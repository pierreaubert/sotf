use crate::app::types::PluginUpdateType;
use crate::app::{AppState, Screen};
use crate::components::plugins::common::param_index_to_engine_param;

// Re-export modules for backward compatibility with crate::ui::components, etc.
pub use crate::components;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::theme::ThemeState as UiKitThemeState;
use gpui_ui_kit::{CollapseDirection, PaneDivider, PaneDividerTheme};
use sotf_audio_player::QueuePlaybackEffect;
use std::time::Duration;

#[cfg(target_os = "ios")]
unsafe extern "C" {
    fn sotf_ios_pop_remote_command() -> i32;
    fn sotf_ios_take_imported_files_json() -> *mut std::ffi::c_char;
    fn sotf_ios_string_free(value: *mut std::ffi::c_char);
}

// Re-export all actions for backward compatibility
pub use crate::app::actions::*;
use crate::components::plugins::actions::{
    OpenAbConfigFile, OpenIrFile, OpenSofaFile, ResetPluginParam, SelectPluginParam, StartKnobDrag,
    UpdatePluginParam,
};
use crate::components::plugins::editing::PluginEditingManager;
use crate::components::plugins::level_meters::LevelMeterManager;

/// Compute the responsive scale factor for a given window size.
/// Reference size: 1200×800 (default window). Uses the smaller axis ratio
/// so the UI never overflows, clamped to 0.55–2.5× for usability.
pub fn compute_responsive_scale(window_width: f32, window_height: f32) -> f32 {
    let width_scale = window_width / 1200.0;
    let height_scale = window_height / 800.0;
    width_scale.min(height_scale).clamp(0.55, 2.5)
}

// Layout constants shared between rendering and pagination calculation.
// Keeping these in one place avoids silent drift between the grid layout
// (album_card.rs, library.rs) and the column/row estimator (recalculate_pagination).

/// Album card width in rems (~140px at default 16px rem). Used in album_card.rs grid rendering
/// and recalculate_pagination column estimation.
pub(crate) const ALBUM_CARD_WIDTH_REMS: f32 = 8.75;

/// Album card height in rems (~180px at 16px rem, thumbnail + text below).
pub(crate) const ALBUM_CARD_HEIGHT_REMS: f32 = 11.25;

/// Gap between album cards in rems (matches gap_4 = 1rem in library.rs grid).
pub(crate) const ALBUM_CARD_GAP_REMS: f32 = 1.0;

/// Footer height in rems (~100px at 16px rem). Used for footer sizing and positioning
/// popups (device popup, studio menu) above the footer.
pub(crate) const FOOTER_HEIGHT_REMS: f32 = 6.25;

/// Total vertical chrome height in rems, used by recalculate_pagination to estimate the
/// available grid area. Breakdown:
///   Header ~2.5rem + Stats ~6.25rem + Filter ~2.5rem + Pagination ~3.125rem + Footer ~3.625rem
pub(crate) const CHROME_HEIGHT_REMS: f32 = 18.0;

/// Estimate the number of album grid columns and rows that fit in the given window.
///
/// This is the pure computation behind `recalculate_pagination` — extracted so it can be
/// unit-tested without constructing a full `PlayerView`.
pub fn estimate_grid_dimensions(
    window_width: f32,
    window_height: f32,
    font_scale: f32,
    min_font_size_px: Option<f32>,
    max_font_size_px: Option<f32>,
) -> (usize, usize) {
    let responsive_scale = compute_responsive_scale(window_width, window_height);
    let (scale_min, scale_max) = combined_scale_bounds(min_font_size_px, max_font_size_px);
    let combined_scale = (font_scale * responsive_scale).clamp(scale_min, scale_max);
    let effective_rem = 16.0 * combined_scale;

    let card_with_gap = (ALBUM_CARD_WIDTH_REMS + ALBUM_CARD_GAP_REMS) * effective_rem;
    // Approximate total horizontal chrome: grid p_2 (0.5rem × 2 sides) + parent padding
    let available_width = window_width - 2.0 * effective_rem;
    let columns = (available_width / card_with_gap).floor().max(1.0) as usize;

    let chrome_height = CHROME_HEIGHT_REMS * effective_rem;
    let available_height = (window_height - chrome_height).max(16.0 * effective_rem);
    let card_height = ALBUM_CARD_HEIGHT_REMS * effective_rem;
    let rows = (available_height / card_height).floor().max(1.0) as usize;

    (columns, rows)
}

/// Default minimum font size in pixels.
pub const DEFAULT_MIN_FONT_SIZE_PX: f32 = 8.0;

/// Default maximum font size in pixels.
pub const DEFAULT_MAX_FONT_SIZE_PX: f32 = 32.0;

/// Convert min/max font size in pixels to combined scale bounds.
/// Uses defaults when `None` is provided.
pub fn combined_scale_bounds(min_px: Option<f32>, max_px: Option<f32>) -> (f32, f32) {
    let min = min_px.unwrap_or(DEFAULT_MIN_FONT_SIZE_PX) / 16.0;
    let max = max_px.unwrap_or(DEFAULT_MAX_FONT_SIZE_PX) / 16.0;
    (min, max)
}

/// `PlayerView` is GPUI's view type. The code-review (`reviews/review-app-gpui.md`)
/// flags `impl PlayerView { … }` as a god-class spread across ~12 files (ui/mod.rs,
/// components/recording/capture.rs, components/plugins/ui_rack.rs,
/// components/room_eq/step_3_configure.rs, components/spinorama_eq/mod.rs, etc.).
/// The long-term fix is to extract `RoomEqController`, `SpinoramaEqController`,
/// `RecordingController`, and `PluginRackController` as `Entity<>` sub-views.
///
/// This hotfix moves the red-review tick/engine logic into `ui/tick.rs` so the
/// central file no longer owns every timer concern. The larger controller
/// extraction remains tracked alongside the existing Plugin Controller Phase-3
/// effort (see auto-memory: "Controller Consolidation — Phase 3").
pub struct PlayerView {
    pub state: Entity<AppState>,
    pub focus_handle: FocusHandle,
    pub(crate) search_focus_handle: FocusHandle,
    pub(crate) volume_focus_handle: FocusHandle,
    last_saved_window_bounds: Option<Bounds<Pixels>>,
    /// Scroll handle for library grid view
    pub(crate) grid_scroll_handle: ScrollHandle,
    /// Track if we've done initial focus (for macOS menu activation)
    needs_initial_focus: bool,
    /// Frame counter for throttling updates (increments every 100ms)
    update_frame_count: u64,
    /// Task for debounced window geometry saving. Only one task is in
    /// flight at a time (gated by `geometry_save_pending`); the task drains
    /// `pending_geometry_save` right before writing so the latest known
    /// geometry is what gets persisted.
    geometry_save_task: Option<Task<()>>,
    /// `true` while a `geometry_save_task` is in flight. Render skips
    /// spawning a new task while this is set — without this guard, every
    /// frame the window moved ≥1 px would schedule a fresh 1 s timer task.
    geometry_save_pending: bool,
    /// Monotonic sequence for geometry changes. The debounce task saves
    /// only when this sequence has stayed stable for a full debounce period.
    geometry_save_sequence: u64,
    /// Latest geometry the user moved to; the in-flight save task drains
    /// this when its timer fires.
    pending_geometry_save: std::sync::Arc<parking_lot::Mutex<Option<PendingGeometrySave>>>,
    /// Cached engine index of the compressor plugin. Outer `Option` tracks
    /// cache validity; inner `Option<usize>` is the lookup result (`None`
    /// when the rack has no compressor). Invalidated on structural plugin
    /// graph changes.
    compressor_engine_idx_cache: Option<Option<usize>>,
    /// Snapshot of the last published tick state — used to suppress
    /// `cx.notify()` when nothing observable changed in the tick.
    last_tick_snapshot: Option<TickSnapshot>,
    /// OS media controls (MPRIS / MediaPlayer) — not available on iOS/tvOS
    #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
    media_controls: Option<crate::media_controls::GpuiMediaControls>,
    /// Optional MIDI hardware bridge. Set when a supported controller was
    /// detected at startup; `None` when no device matched.
    midi_input: Option<crate::app::midi_input::MidiInputService>,
    /// Plugin index the engine was last focused on, so we know when to
    /// rebuild the auto-map for a different plugin.
    midi_focused_plugin: Option<usize>,
}

impl PlayerView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let search_focus_handle = cx.focus_handle();
        let volume_focus_handle = cx.focus_handle();

        // Initialize OS media controls (MPRIS on Linux, MediaPlayer on macOS/Windows)
        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        let media_controls = match crate::media_controls::GpuiMediaControls::new() {
            Ok(mc) => {
                log::info!("OS media controls initialized");
                Some(mc)
            }
            Err(e) => {
                log::warn!("Failed to initialize OS media controls: {}", e);
                None
            }
        };

        // Subscribe to layout changes for granular re-renders
        let layout = state.read(cx).layout.clone();
        cx.subscribe(&layout, |_view, _, _, cx| {
            cx.notify();
        })
        .detach();

        // Note: We don't use cx.observe() + cx.notify() here because it can cause
        // re-entrant update issues when state is updated during effect processing.
        // The periodic timer below handles state updates and notify() calls.
        // Event handlers that update state should call cx.notify() directly.

        // Set up periodic update timer for playback position and loudness
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let result = this.update(cx, |view, cx| {
                    // Increment frame counter for throttling
                    view.update_frame_count = view.update_frame_count.wrapping_add(1);

                    // Collect data needed for infinite scroll check before state update
                    let scroll_check_data =
                        if view.state.read(cx).app.ui_state.current_screen == Screen::Library {
                            let scroll_y: f32 = view.grid_scroll_handle.offset().y.into();
                            let state = view.state.read(cx);
                            let item_count = state.app.library_state.items_per_page;
                            let total_albums = state.app.filtered_albums().len();
                            let columns = state.app.library_state.library_columns.max(1);
                            let rows = item_count.div_ceil(columns);
                            let card_height = 220.0;
                            let estimated_height = rows as f32 * card_height;
                            let window_height = state.app.ui_state.window_height;
                            let scroll_position = scroll_y.abs();
                            let scrollable_distance = (estimated_height - window_height).max(0.0);
                            let remaining_scroll = scrollable_distance - scroll_position;
                            let needs_more_content = estimated_height < window_height * 2.0;
                            let near_bottom = remaining_scroll < 1000.0;
                            let should_load =
                                item_count < total_albums && (needs_more_content || near_bottom);
                            Some(should_load)
                        } else {
                            None
                        };

                    // Collect pending media control events (before state borrow)
                    #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
                    let media_events: Vec<
                        sotf_media_controls::MediaControlEvent,
                    > = view
                        .media_controls
                        .as_ref()
                        .map(|mc| std::iter::from_fn(|| mc.poll_event()).collect())
                        .unwrap_or_default();
                    #[cfg(any(target_os = "ios", target_os = "tvos"))]
                    let media_events: Vec<()> = vec![];

                    // Drain any pending hardware MIDI messages before the
                    // main state update so resulting param changes ride
                    // through the same `pending_plugin_update` path as
                    // user-initiated edits.
                    if let Some(svc) = view.midi_input.as_ref() {
                        let messages = svc.drain();
                        if !messages.is_empty() {
                            let layout = svc.layout();
                            let last_focus = view.midi_focused_plugin;
                            let new_focus = view.state.update(cx, |state, _cx| {
                                Self::dispatch_midi_messages(state, layout, last_focus, messages)
                            });
                            view.midi_focused_plugin = new_focus;
                        }
                    }

                    // Consolidate all state updates into a single update call
                    // to avoid multiple observer triggers.
                    let frame_count = view.update_frame_count;
                    let compressor_cache = &mut view.compressor_engine_idx_cache;
                    view.state.update(cx, |state, _cx| {
                        let (playback_state, was_playing) =
                            Self::sync_playback_data(state, frame_count, compressor_cache);

                        if let Some(update_type) =
                            state.app.plugin_state.pending_plugin_update.take()
                        {
                            log::warn!("[GPUI] Applying pending plugin update: {:?}", update_type);
                            // Structural plugin graph changes invalidate
                            // the compressor index cache.
                            *compressor_cache = None;
                            Self::apply_plugin_update(state, update_type);
                        }

                        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
                        for event in &media_events {
                            Self::handle_media_control_event(state, event);
                        }

                        #[cfg(target_os = "ios")]
                        Self::drain_ios_remote_commands(state);

                        if state.app.playback.is_playing && playback_state.is_playing {
                            state.app.check_and_record_play();
                        }

                        Self::handle_engine_state(state, &playback_state, was_playing);
                        Self::handle_gapless_prequeue(state, &playback_state);
                        Self::tick_background_tasks(state);
                        state.app.refresh_scheduled_theme();
                    });

                    // Background stats computation (outside state update)
                    let (needs_stats, is_stats_computing) = {
                        let state = view.state.read(cx);
                        (
                            !state.app.library_stats.valid,
                            state.app.library_stats_computing,
                        )
                    };
                    if needs_stats && !is_stats_computing {
                        view.compute_library_stats_async(cx);
                    }

                    // Infinite scroll - load more albums if needed (outside state update)
                    if scroll_check_data == Some(true) {
                        view.load_more_albums(cx);
                    }

                    // Update OS media controls metadata (outside state update)
                    #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
                    if let Some(mc) = view.media_controls.as_mut() {
                        let state = view.state.read(cx);
                        crate::media_controls::update_media_controls(
                            mc,
                            &state.app,
                            state.app.playback.position_secs,
                        );
                    }

                    // Only notify if observable state actually changed.
                    // `Render::render` re-runs the entire view tree on every
                    // notify, so idle screens at 100 ms tick rate would
                    // otherwise re-render 10×/s for nothing.
                    let new_snapshot = Self::tick_snapshot(view, cx);
                    if view.last_tick_snapshot.as_ref() != Some(&new_snapshot) {
                        view.last_tick_snapshot = Some(new_snapshot);
                        cx.notify();
                    }
                });
                // Exit the loop if the view is no longer valid
                if result.is_err() {
                    break;
                }
            }
        })
        .detach();

        // Best-effort hardware MIDI bridge. iOS/tvOS use the manager_stub
        // which always returns no devices, so this is harmless on those
        // targets.
        let midi_input = crate::app::midi_input::try_start();

        Self {
            state,
            focus_handle,
            search_focus_handle,
            volume_focus_handle,
            last_saved_window_bounds: None,
            grid_scroll_handle: ScrollHandle::new(),
            needs_initial_focus: true,
            update_frame_count: 0,
            geometry_save_task: None,
            geometry_save_pending: false,
            geometry_save_sequence: 0,
            pending_geometry_save: std::sync::Arc::new(parking_lot::Mutex::new(None)),
            compressor_engine_idx_cache: None,
            last_tick_snapshot: None,
            #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
            media_controls,
            midi_input,
            midi_focused_plugin: None,
        }
    }

    /// Spawn a background task to compute library statistics
    pub(crate) fn compute_library_stats_async(&self, cx: &mut Context<Self>) {
        let (albums, pending_stats) = {
            let state = self.state.read(cx);
            (
                state.app.library_state.library.albums.clone(),
                state.app.pending_library_stats.clone(),
            )
        };

        // Mark as computing
        self.state.update(cx, |state, _cx| {
            state.app.library_stats_computing = true;
        });

        // Spawn background task
        cx.background_executor()
            .spawn(async move {
                log::info!("[Stats] Starting background stats computation...");
                let start = std::time::Instant::now();

                // Run expensive O(N) computation
                let stats = crate::app::App::compute_library_stats_static(&albums);

                let duration = start.elapsed();
                log::info!(
                    "[Stats] Background stats computation complete in {:?}",
                    duration
                );

                // Store result for main loop to pick up
                *pending_stats.lock() = Some(stats);
            })
            .detach();
    }

    /// Drain MIDI messages from the hardware bridge into plugin parameter
    /// updates via `MidiMappingEngine`. Returns the plugin index the engine
    /// is now focused on (for cache-invalidation in the caller).
    ///
    /// Skips when no plugin is focused (engine has nothing to map onto)
    /// and silently drops messages when the focused plugin has no params.
    fn dispatch_midi_messages(
        state: &mut AppState,
        layout: sotf_audio_player_midi::ControllerLayout,
        last_focus: Option<usize>,
        messages: Vec<sotf_audio_player_midi::MidiMessage>,
    ) -> Option<usize> {
        use sotf_audio_player_midi::MappingAction;

        // Install the layout exactly once. set_layout() is idempotent on
        // the inner Option but we avoid the clone on every tick.
        if state.app.plugin_state.midi_mapping.layout().is_none() {
            state.app.plugin_state.midi_mapping.set_layout(layout);
        }

        let focused = state.app.plugin_state.editing_plugin_index?;

        // Snapshot the focused plugin's params so we can release the borrow
        // on `plugin_state.graph` before mutably borrowing `midi_mapping`.
        let (plugin_type_name, params) = {
            let plugins = state.app.plugin_state.graph.plugins();
            let plugin = plugins.get(focused)?;
            (
                plugin.settings.plugin_type().name().to_string(),
                plugin.settings.param_specs().to_vec(),
            )
        };

        // Re-focus the engine when the plugin selection changed so the
        // auto-mapped bindings target the new param set.
        if last_focus != Some(focused) {
            state.app.plugin_state.midi_mapping.on_plugin_focus(
                &plugin_type_name,
                &params,
                focused,
            );
        }

        for msg in messages {
            let action = state
                .app
                .plugin_state
                .midi_mapping
                .handle_midi(&msg, &params);
            match action {
                MappingAction::SetParam {
                    plugin_index,
                    param_index,
                    value,
                } => {
                    state.app.set_plugin_param(plugin_index, param_index, value);
                }
                MappingAction::AdjustParam { .. } => {
                    // Relative encoders aren't auto-mapped on the supported
                    // controllers (Xone:K2 / LCXL pots and faders are
                    // absolute). Drop relative deltas with a debug log so
                    // we notice if a future controller surfaces them.
                    log::debug!("MIDI: AdjustParam dropped (relative encoders unsupported)");
                }
                MappingAction::PagePrev | MappingAction::PageNext => {
                    // Engine has already mutated `mapping.current_page`.
                    // Re-render is implicit via cx.notify() in the tick.
                }
                MappingAction::LearnComplete { .. } | MappingAction::Unmapped => {}
            }
        }

        Some(focused)
    }

    #[cfg(target_os = "ios")]
    fn drain_ios_remote_commands(state: &mut AppState) {
        for _ in 0..32 {
            // SAFETY: implemented by the iOS crate in the final app binary.
            // It returns one small integer command and never hands out Rust
            // references across the FFI boundary.
            match unsafe { sotf_ios_pop_remote_command() } {
                0 => break,
                1 => Self::handle_ios_queue_navigation(state, true),
                2 => Self::handle_ios_queue_navigation(state, false),
                3 => {
                    Self::handle_ios_imported_files(state);
                }
                other => {
                    log::warn!("[iOS] unknown remote command code: {other}");
                    break;
                }
            }
        }
    }

    #[cfg(target_os = "ios")]
    fn handle_ios_imported_files(state: &mut AppState) {
        // SAFETY: implemented by app-ios in the final binary. It returns an
        // owned C string allocated by Rust, or NULL on serialization failure.
        let raw = unsafe { sotf_ios_take_imported_files_json() };
        if raw.is_null() {
            return;
        }
        let json = {
            // SAFETY: `raw` is non-null and points to a NUL-terminated string
            // until we release it below.
            unsafe { std::ffi::CStr::from_ptr(raw) }
                .to_string_lossy()
                .into_owned()
        };
        // SAFETY: release the string returned by `sotf_ios_take_imported_files_json`.
        unsafe {
            sotf_ios_string_free(raw);
        }

        let paths: Vec<String> = match serde_json::from_str(&json) {
            Ok(paths) => paths,
            Err(err) => {
                log::warn!("[iOS] failed to parse imported file paths: {err}");
                return;
            }
        };
        if paths.is_empty() {
            return;
        }

        let mut added = 0usize;
        for path in paths.iter().map(std::path::PathBuf::from) {
            let looks_like_file = path.is_file() || path.extension().is_some();
            let library_path = if looks_like_file {
                path.parent()
                    .map(std::path::Path::to_path_buf)
                    .unwrap_or(path)
            } else {
                path
            };
            match state.app.library_state.library.add_directory(library_path) {
                Ok(true) => added += 1,
                Ok(false) => {}
                Err(err) => log::debug!("[iOS] imported file library path skipped: {err}"),
            }
        }

        if added > 0 {
            state.app.needs_rescan = true;
            match state.app.rescan_library() {
                Ok(()) => {
                    state.app.ui_state.toast_message = Some(crate::app::ToastMessage::success(
                        format!("Imported files queued {added} library location(s) for scanning."),
                    ));
                }
                Err(err) => {
                    state.app.ui_state.toast_message = Some(crate::app::ToastMessage::warning(
                        format!("Imported files added, but scan could not start: {err}"),
                    ));
                }
            }
        }
    }

    #[cfg(target_os = "ios")]
    fn handle_ios_queue_navigation(state: &mut AppState, next: bool) {
        if let Err(e) = state.player.lock().cancel_next() {
            log::warn!("Player cancel_next failed: {e}");
        }

        let from_index = state.app.playback.current_queue_index;
        let source = if next {
            state.app.next_track()
        } else {
            state.app.previous_track()
        };

        if let Some(source) = source {
            Self::play_track(state, source);
            if let Some(to_index) = state.app.playback.current_queue_index {
                let trigger = if next {
                    crate::app::state::TrackChangeTrigger::NextTrack
                } else {
                    crate::app::state::TrackChangeTrigger::PrevTrack
                };
                state
                    .app
                    .record_track_changed(from_index, to_index, trigger);
            }
        } else if next {
            state.app.playback.is_playing = false;
        }
    }

    fn open_config(&mut self, _: &OpenConfig, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen(Screen::Settings, cx);
    }

    pub(crate) fn quit_app(&mut self, _: &QuitApp, window: &mut Window, cx: &mut Context<Self>) {
        log::info!("Quit requested - saving config and stopping services...");

        // Save window geometry before quitting
        let window_bounds = window.bounds();
        let geometry = crate::config::WindowGeometry {
            x: window_bounds.origin.x.into(),
            y: window_bounds.origin.y.into(),
            width: window_bounds.size.width.into(),
            height: window_bounds.size.height.into(),
        };

        self.state.update(cx, |state, cx| {
            let layout = state.layout.read(cx);
            // Save configuration
            if let Err(e) = state.app.save_config_with_geometry(layout, Some(geometry)) {
                log::error!("Failed to save config on quit: {}", e);
            }

            // Stop background managers
            state.app.scan_ctrl.stop_all();

            // Stop audio playback - this stops the audio engine threads
            if let Err(e) = state.player.lock().stop() {
                log::error!("Failed to stop player on quit: {}", e);
            }
        });

        // On iOS, apps don't quit — save state and let iOS manage lifecycle.
        // Audio continues in background via UIBackgroundModes=audio.
        #[cfg(any(target_os = "ios", target_os = "tvos"))]
        {
            log::info!("iOS/tvOS: state saved, returning to background");
            return;
        }

        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            log::info!("Services stopped, requesting GPUI quit...");

            // Request GPUI to quit
            cx.quit();

            // Give GPUI and background threads a very short time to clean up
            // before forcing exit (to ensure we don't hang indefinitely)
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_millis(500));
                log::info!("Cleanup timeout reached, forcing exit");
                std::process::exit(0);
            });
        }
    }

    fn cycle_theme(&mut self, _: &CycleTheme, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.app.next_theme();
            let layout = state.layout.read(cx);
            if let Err(e) = state.app.save_config(layout) {
                log::error!("Failed to save config: {}", e);
            }
        });
        cx.notify();
    }

    fn set_design_system(
        &mut self,
        name: &str,
        system: gpui_design::DesignSystem,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.set_global(gpui_design::DesignSystemState::with_system(system));
        self.state.update(cx, |state, cx| {
            state.app.ui_state.design_language = Some(name.to_string());
            let layout = state.layout.read(cx);
            if let Err(e) = state.app.save_config(layout) {
                log::error!("Failed to save config: {}", e);
            }
        });
        cx.notify();
    }

    fn set_design_neutral(
        &mut self,
        _: &SetDesignNeutral,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_design_system("neutral", gpui_design::DesignSystem::neutral(), window, cx);
    }

    fn set_design_apple_hig(
        &mut self,
        _: &SetDesignAppleHig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_design_system(
            "apple_hig",
            gpui_design::DesignSystem::apple_hig(),
            window,
            cx,
        );
    }

    fn set_design_material3(
        &mut self,
        _: &SetDesignMaterial3,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_design_system(
            "material3",
            gpui_design::DesignSystem::material3(),
            window,
            cx,
        );
    }

    fn set_design_fluent(
        &mut self,
        _: &SetDesignFluent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_design_system("fluent", gpui_design::DesignSystem::fluent(), window, cx);
    }

    fn cycle_language(&mut self, _: &CycleLanguage, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.app.next_language();
            let layout = state.layout.read(cx);
            if let Err(e) = state.app.save_config(layout) {
                log::error!("Failed to save config: {}", e);
            }
        });
        cx.notify();
    }

    fn increase_font_size(&mut self, _: &IncreaseFontSize, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            // Increase by 10%, max 2.0 (200%)
            state.app.ui_state.font_scale = (state.app.ui_state.font_scale * 1.1).min(2.0);
            let layout = state.layout.read(cx);
            if let Err(e) = state.app.save_config(layout) {
                log::error!("Failed to save config: {}", e);
            }
        });
        cx.notify();
    }

    fn decrease_font_size(&mut self, _: &DecreaseFontSize, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            // Decrease by 10%, min 0.5 (50%)
            state.app.ui_state.font_scale = (state.app.ui_state.font_scale / 1.1).max(0.5);
            let layout = state.layout.read(cx);
            if let Err(e) = state.app.save_config(layout) {
                log::error!("Failed to save config: {}", e);
            }
        });
        cx.notify();
    }

    fn reset_font_size(&mut self, _: &ResetFontSize, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.app.ui_state.font_scale = 1.0;
            let layout = state.layout.read(cx);
            if let Err(e) = state.app.save_config(layout) {
                log::error!("Failed to save config: {}", e);
            }
        });
        cx.notify();
    }

    fn toggle_search(&mut self, _: &ToggleSearch, window: &mut Window, cx: &mut Context<Self>) {
        let mut should_focus = false;
        self.state.update(cx, |state, _cx| {
            if state.app.ui_state.input_mode == crate::app::InputMode::Search {
                log::info!("toggle_search: exiting search mode");
                state.app.ui_state.input_mode = crate::app::InputMode::Normal;
            } else {
                log::info!("toggle_search: entering search mode");
                state.app.ui_state.input_mode = crate::app::InputMode::Search;
                state.app.library_state.search_query.clear();
                should_focus = true;
            }
        });

        if should_focus {
            self.search_focus_handle.focus(window, cx);
            #[cfg(any(target_os = "ios", target_os = "tvos"))]
            gpui_ios::show_keyboard();
        } else {
            #[cfg(any(target_os = "ios", target_os = "tvos"))]
            gpui_ios::hide_keyboard();
        }
        cx.notify();
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        let was_search =
            self.state.read(cx).app.ui_state.input_mode == crate::app::InputMode::Search;
        self.state.update(cx, |state, _cx| {
            state.app.ui_state.input_mode = crate::app::InputMode::Normal;
            state.app.library_state.search_query.clear();
            state.app.input_state.directory_input.clear();
            state.app.input_state.apo_file_input.clear();
            state.app.input_state.sofa_file_input.clear();
            state.app.clear_autocomplete();
            state.app.dismiss_toast();
            state.app.ui_state.context_menu = None; // Close context menu
            state.app.ui_state.active_menu = crate::app::ActiveMenu::None; // Close dropdown menus
        });
        if was_search {
            #[cfg(any(target_os = "ios", target_os = "tvos"))]
            gpui_ios::hide_keyboard();
        }
        cx.notify();
    }

    fn toggle_library_view(
        &mut self,
        _: &ToggleLibraryView,
        _: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        // Grid view is the only view mode now, toggle is a no-op
    }

    fn toggle_help(&mut self, _: &ToggleHelp, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            use crate::app::InputMode;
            if state.app.ui_state.input_mode == InputMode::Help {
                state.app.ui_state.input_mode = InputMode::Normal;
            } else {
                state.app.ui_state.input_mode = InputMode::Help;
            }
        });
        cx.notify();
    }

    fn toggle_help_support(
        &mut self,
        _: &ToggleHelpSupport,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            use crate::app::InputMode;
            if state.app.ui_state.input_mode == InputMode::HelpSupport {
                state.app.ui_state.input_mode = InputMode::Normal;
            } else {
                state.app.ui_state.input_mode = InputMode::HelpSupport;
            }
        });
        cx.notify();
    }

    fn toggle_screen_guide(
        &mut self,
        _: &ToggleScreenGuide,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            use crate::app::InputMode;
            if state.app.ui_state.input_mode == InputMode::ScreenGuide {
                state.app.ui_state.input_mode = InputMode::Normal;
            } else {
                state.app.ui_state.input_mode = InputMode::ScreenGuide;
            }
        });
        cx.notify();
    }

    fn about(&mut self, _: &About, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            use crate::app::InputMode;
            if state.app.ui_state.input_mode == InputMode::About {
                state.app.ui_state.input_mode = InputMode::Normal;
            } else {
                state.app.ui_state.input_mode = InputMode::About;
            }
        });
        cx.notify();
    }

    fn toggle_expand(&mut self, _: &ToggleExpand, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if state.app.ui_state.current_screen == Screen::Queue {
                state.app.toggle_queue_item_expansion();
            }
        });
        cx.notify();
    }

    fn remove_item(&mut self, _: &RemoveItem, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Block if in text input mode
            if Self::is_text_input_mode(state.app.ui_state.input_mode) {
                return;
            }
            if state.app.ui_state.current_screen == Screen::Queue {
                let effect = state
                    .app
                    .remove_from_queue(state.app.queue_state.selected_index);
                match effect {
                    QueuePlaybackEffect::Reload(source) | QueuePlaybackEffect::Play(source) => {
                        Self::play_track(state, source);
                    }
                    QueuePlaybackEffect::Stop => {
                        if let Err(e) = state.player.lock().stop() {
                            log::warn!("[UI] Failed to stop player after queue removal: {}", e);
                        }
                    }
                    QueuePlaybackEffect::None => {}
                }
            }
        });
        cx.notify();
    }

    fn clear_queue(&mut self, _: &ClearQueue, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.clear_queue();
        });
        cx.notify();
    }

    fn fill_queue_magic(&mut self, _: &FillQueueMagic, _: &mut Window, cx: &mut Context<Self>) {
        log::info!("[UI] FillQueueMagic action handler triggered");
        self.state
            .update(cx, |state, _cx| match state.app.fill_queue_magic() {
                Ok(count) => {
                    log::info!("[UI] fill_queue_magic added {} tracks", count);
                }
                Err(e) => {
                    log::error!("[UI] fill_queue_magic error: {}", e);
                }
            });
        cx.notify();
    }

    fn add_to_queue(&mut self, _: &AddToQueue, _: &mut Window, cx: &mut Context<Self>) {
        log::info!("[UI] AddToQueue action handler triggered");
        self.state
            .update(cx, |state, _cx| match state.app.add_album_to_queue() {
                Ok(Some(path)) => Self::play_track(state, path),
                Err(e) => {
                    log::warn!("[UI] Cannot add to queue: {}", e);
                    state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(e));
                }
                _ => {}
            });
        cx.notify();
    }

    fn play_now(&mut self, _: &PlayNow, _: &mut Window, cx: &mut Context<Self>) {
        log::info!("[UI] PlayNow action handler triggered");
        self.state
            .update(cx, |state, _cx| match state.app.play_album_now() {
                Ok(Some(path)) => Self::play_track(state, path),
                Err(e) => {
                    log::warn!("[UI] Cannot play album: {}", e);
                    state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(e));
                }
                _ => {}
            });
        cx.notify();
    }

    fn add_directory(&mut self, _: &AddDirectory, _: &mut Window, cx: &mut Context<Self>) {
        // On iOS, present the native document picker instead of text input.
        // On tvOS, file import is not available (no document picker).
        #[cfg(target_os = "ios")]
        {
            unsafe extern "C" {
                fn sotf_ios_show_document_picker();
            }
            unsafe { sotf_ios_show_document_picker() };
            return;
        }

        #[cfg(target_os = "tvos")]
        {
            log::info!("tvOS: file import not available");
            return;
        }

        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            self.state.update(cx, |state, _cx| {
                use crate::app::InputMode;
                // Enter add directory mode
                state.app.ui_state.input_mode = InputMode::AddDirectory;
                state.app.input_state.directory_input.clear();
                state.app.clear_autocomplete();
            });
            cx.notify();
        }
    }

    fn scan_library(&mut self, _: &ScanLibrary, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Start scan (this will be async in reality, but for now we do it synchronously)
            if let Err(e) = state.app.scan_library() {
                log::error!("Library scan failed: {}", e);
                state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(format!(
                    "Scan failed: {}",
                    e
                )));
            }
            // Save directories to config after successful scan
            let layout = state.layout.read(_cx);
            if let Err(e) = state.app.save_config(layout) {
                log::warn!("Failed to save config: {}", e);
            }
        });
        cx.notify();
    }

    /// Helper method to start library scan from settings screen
    pub(crate) fn start_library_scan(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if let Err(e) = state.app.scan_library() {
                log::error!("Library scan failed: {}", e);
                state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(format!(
                    "Scan failed: {}",
                    e
                )));
            } else {
                // Show progress modal
                state.app.scan_progress_modal = Some(crate::app::types::ScanProgressModal::new(
                    crate::app::types::ScanType::Library,
                ));
            }
            // Save directories to config after successful scan
            let layout = state.layout.read(_cx);
            if let Err(e) = state.app.save_config(layout) {
                log::warn!("Failed to save config: {}", e);
            }
        });
        cx.notify();
    }

    pub(crate) fn play_track(state: &mut AppState, source: sotf_audio::decoder::AudioSource) {
        Self::play_track_at_with_mode(state, source, None, false);
    }

    pub(crate) fn play_track_smooth(
        state: &mut AppState,
        source: sotf_audio::decoder::AudioSource,
    ) {
        Self::play_track_at_with_mode(state, source, None, true);
    }

    /// Auto-advance variant: auto-suspends incompatible plugins without showing a dialog.
    pub(crate) fn play_track_auto_advance(
        state: &mut AppState,
        source: sotf_audio::decoder::AudioSource,
    ) {
        let track_channels = state
            .app
            .playback
            .current_queue_index
            .and_then(|idx| state.app.queue_state.get(idx))
            .and_then(|item| item.current_track())
            .and_then(|track| track.channels)
            .unwrap_or(2) as usize;

        // Clear suspensions from previous track
        state.app.plugin_state.graph.clear_suspensions();
        state
            .app
            .plugin_state
            .graph
            .update_channel_dependent_plugins();

        let conflicts = state
            .app
            .plugin_state
            .graph
            .find_channel_conflicts(track_channels);
        if !conflicts.is_empty() {
            log::info!(
                "[GPUI] Auto-advance: suspending {} incompatible plugin(s) for {}ch track",
                conflicts.len(),
                track_channels
            );
            let indices: Vec<usize> = conflicts.iter().map(|c| c.index).collect();
            state.app.plugin_state.graph.suspend_plugins(&indices);
            state
                .app
                .plugin_state
                .graph
                .update_channel_dependent_plugins();
        }

        Self::play_track_at_inner(state, source, None, track_channels, false);
    }

    pub(crate) fn play_track_at(
        state: &mut AppState,
        source: sotf_audio::decoder::AudioSource,
        position: Option<f64>,
    ) {
        Self::play_track_at_with_mode(state, source, position, false);
    }

    fn play_track_at_with_mode(
        state: &mut AppState,
        source: sotf_audio::decoder::AudioSource,
        position: Option<f64>,
        prefer_smooth_switch: bool,
    ) {
        let track_channels = state
            .app
            .playback
            .current_queue_index
            .and_then(|idx| state.app.queue_state.get(idx))
            .and_then(|item| item.current_track())
            .and_then(|track| track.channels)
            .unwrap_or(2) as usize;

        // Clear suspensions from previous track
        state.app.plugin_state.graph.clear_suspensions();
        state
            .app
            .plugin_state
            .graph
            .update_channel_dependent_plugins();

        // Check for channel conflicts with all fixed-channel plugins
        let conflicts = state
            .app
            .plugin_state
            .graph
            .find_channel_conflicts(track_channels);
        if !conflicts.is_empty() {
            log::info!(
                "[GPUI] Channel conflict: {}ch file with {} incompatible plugin(s)",
                track_channels,
                conflicts.len()
            );
            state.app.channel_conflicts = conflicts;
            state.app.channel_conflict_path = Some(source);
            state.app.channel_conflict_track_channels = track_channels;
            state.app.ui_state.input_mode = crate::app::InputMode::ChannelConflict;
            return;
        }

        Self::play_track_at_inner(
            state,
            source,
            position,
            track_channels,
            prefer_smooth_switch,
        );
    }

    /// Inner play logic after conflict resolution. Called by both play_track_at and
    /// play_track_auto_advance after suspensions are handled.
    fn play_track_at_inner(
        state: &mut AppState,
        source: sotf_audio::decoder::AudioSource,
        position: Option<f64>,
        track_channels: usize,
        prefer_smooth_switch: bool,
    ) {
        let track_sample_rate = state
            .app
            .playback
            .current_queue_index
            .and_then(|idx| state.app.queue_state.get(idx))
            .and_then(|item| item.current_track())
            .and_then(|track| track.sample_rate)
            .unwrap_or(48000);

        let device_name = state
            .app
            .audio_device_state
            .current_output_device_name
            .clone()
            .or_else(|| {
                state
                    .app
                    .audio_device_state
                    .selected_output_device()
                    .map(|d| d.name.clone())
            });

        // Determine target sample rate based on track's native rate and device capabilities
        let sample_rate =
            sotf_audio::select_output_sample_rate(track_sample_rate, device_name.as_deref()) as f64;

        state
            .app
            .plugin_state
            .graph
            .adapt_matrix_to_input(track_channels);
        let mut output_channels = state
            .app
            .plugin_state
            .graph
            .output_channels_for_input(track_channels);

        // Clamp output channels to device max — the playback thread will
        // downmix automatically when the processing chain outputs more
        // channels than the hardware supports.
        if let Some(max_ch) = state.app.get_device_max_channels()
            && output_channels > max_ch
        {
            log::info!(
                "[GPUI] Clamping output from {} to {} channels (device limit)",
                output_channels,
                max_ch
            );
            output_channels = max_ch;
        }

        // Apply ReplayGain correction to the permanent Gain plugin
        let rg_gain = state
            .app
            .playback
            .current_queue_index
            .and_then(|idx| state.app.queue_state.get(idx))
            .and_then(|item| item.current_track())
            .and_then(|track| state.app.playback.get_replay_gain_adjustment(track));
        state.app.plugin_state.graph.set_replay_gain(rg_gain);

        let plugins = state.app.plugin_state.graph.to_plugin_configs(sample_rate);

        let play_result = {
            let mut player = state.player.lock();
            if prefer_smooth_switch && position.is_none() {
                match player.switch_to_source_at(
                    source.clone(),
                    plugins.clone(),
                    output_channels,
                    device_name.clone(),
                    position,
                ) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        log::warn!(
                            "[GPUI] Smooth track switch unavailable, falling back to restart: {}",
                            e
                        );
                        player.load_and_play_source_at(
                            source,
                            plugins,
                            output_channels,
                            device_name,
                            position,
                        )
                    }
                }
            } else {
                player.load_and_play_source_at(
                    source,
                    plugins,
                    output_channels,
                    device_name,
                    position,
                )
            }
        };

        if let Err(e) = play_result {
            log::error!("Failed to play track: {}", e);
            state.app.playback.is_playing = false;
            state
                .app
                .record_playback_error(format!("Play track failed: {}", e));
        } else {
            state.app.playback.is_playing = true;
            if let Some(queue_index) = state.app.playback.current_queue_index {
                state.app.record_playback_started(
                    queue_index,
                    state.app.queue_state.current_track_path(),
                );
            }
            if let Some(path) = state.app.queue_state.current_track_path() {
                state.app.start_track_tracking(path);
            }
        }
    }
    // Plugin parameter handling
    pub(crate) fn on_update_plugin_param(
        &mut self,
        action: &UpdatePluginParam,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_plugin_param(action.plugin_idx, action.param_idx, action.value);
        });
        cx.notify();
    }

    pub(crate) fn on_select_plugin_param(
        &mut self,
        action: &SelectPluginParam,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state.app.plugin_state.editing_plugin_index = Some(action.plugin_idx);
            state.app.plugin_state.plugin_param_selection = action.param_idx;
        });
        cx.notify();
    }

    pub(crate) fn on_reset_plugin_param(
        &mut self,
        action: &ResetPluginParam,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .reset_plugin_param(action.plugin_idx, action.param_idx);
        });
        cx.notify();
    }

    pub(crate) fn on_start_knob_drag(
        &mut self,
        action: &StartKnobDrag,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state.app.knob_drag = Some(crate::app::state::app::KnobDragState {
                plugin_idx: action.plugin_idx,
                param_idx: action.param_idx,
                start_y: action.start_y,
                start_value: action.start_value,
                min: action.min,
                max: action.max,
            });
        });
        cx.notify();
    }

    pub(crate) fn on_open_sofa_file(
        &mut self,
        action: &OpenSofaFile,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            let plugin_idx = action.plugin_idx;
            let weak_state = self.state.downgrade();
            cx.spawn(async move |_, cx| {
                let file = rfd::AsyncFileDialog::new()
                    .add_filter("SOFA Files", &["sofa"])
                    .set_title("Select SOFA File")
                    .pick_file()
                    .await;

                if let Some(file) = file {
                    let Some(state_entity) = weak_state.upgrade() else {
                        return;
                    };
                    let path = file.path().to_string_lossy().to_string();
                    state_entity.update(&mut cx.clone(), |state, cx| {
                        if let Err(e) = state.app.set_plugin_param_string(plugin_idx, 0, path) {
                            log::error!("[GPUI] Invalid SOFA file path: {}", e);
                            state.app.ui_state.toast_message =
                                Some(crate::app::ToastMessage::error(e));
                        }
                        cx.notify();
                    });
                }
            })
            .detach();
        }
    }

    pub(crate) fn on_open_ir_file(
        &mut self,
        action: &OpenIrFile,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            let plugin_idx = action.plugin_idx;
            let weak_state = self.state.downgrade();
            cx.spawn(async move |_, cx| {
                let file = rfd::AsyncFileDialog::new()
                    .add_filter("Audio Files", &["wav", "flac", "aif", "aiff"])
                    .set_title("Select IR File")
                    .pick_file()
                    .await;

                if let Some(file) = file {
                    let Some(state_entity) = weak_state.upgrade() else {
                        return;
                    };
                    let path = file.path().to_string_lossy().to_string();
                    state_entity.update(&mut cx.clone(), |state, cx| {
                        if let Err(e) = state.app.set_plugin_param_string(plugin_idx, 0, path) {
                            log::error!("[GPUI] Invalid IR file path: {}", e);
                            state.app.ui_state.toast_message =
                                Some(crate::app::ToastMessage::error(e));
                        }
                        cx.notify();
                    });
                }
            })
            .detach();
        }
    }

    pub(crate) fn on_open_ab_config_file(
        &mut self,
        action: &OpenAbConfigFile,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            let plugin_idx = action.plugin_idx;
            let path_id = action.path_id.clone();
            let weak_state = self.state.downgrade();
            cx.spawn(async move |_, cx| {
                let file = rfd::AsyncFileDialog::new()
                    .add_filter("JSON Config Files", &["json"])
                    .set_title("Select Config File")
                    .pick_file()
                    .await;

                if let Some(file) = file {
                    let Some(state_entity) = weak_state.upgrade() else {
                        return;
                    };
                    let file_path = file.path().to_string_lossy().to_string();
                    // Read the JSON content from file (blocking I/O is fine here —
                    // config files are tiny and we're already in a spawned task)
                    match std::fs::read_to_string(&file_path) {
                        Ok(content) => {
                            // Validate it is an AB Compare path config before applying it.
                            if serde_json::from_str::<
                                sotf_audio_player::controllers::ab_compare_path::PathConfig,
                            >(&content)
                            .is_ok()
                            {
                                let plugins =
                                    sotf_audio_player::controllers::ab_compare_path::parse_path_config(
                                        &content,
                                    );
                                let param_idx = if path_id == "a" { 9 } else { 10 };
                                state_entity.update(&mut cx.clone(), |state, cx| {
                                    // AB Compare configs are JSON, not file paths — error won't occur
                                    let _ = state
                                        .app
                                        .set_plugin_param_string(plugin_idx, param_idx, content);
                                    // Store the source file path for display
                                    if path_id == "a" {
                                        state.app.plugin_state.ab_compare_file_a = Some(file_path);
                                        state.app.plugin_state.ab_path_a = plugins;
                                    } else {
                                        state.app.plugin_state.ab_compare_file_b = Some(file_path);
                                        state.app.plugin_state.ab_path_b = plugins;
                                    }
                                    state.app.plugin_state.ab_add_menu_target = None;
                                    cx.notify();
                                });
                            } else {
                                log::warn!(
                                    "AB Compare: file is not a valid path config: {file_path}"
                                );
                            }
                        }
                        Err(e) => {
                            log::error!("AB Compare: failed to read config file: {e}");
                        }
                    }
                }
            })
            .detach();
        }
    }

    pub(crate) fn on_ab_path_add_plugin(
        &mut self,
        action: &crate::components::plugins::actions::ABPathAddPlugin,
        _: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        use sotf_audio_player::controllers::ab_compare_path::{
            add_path_plugin, encode_path_config,
        };
        let plugin_idx = action.plugin_idx;
        let param_idx: usize = if action.path == 0 { 9 } else { 10 };

        self.state.update(cx, |state, _cx| {
            let plugins = if action.path == 0 {
                state.app.plugin_state.ab_compare_file_a = None;
                &mut state.app.plugin_state.ab_path_a
            } else {
                state.app.plugin_state.ab_compare_file_b = None;
                &mut state.app.plugin_state.ab_path_b
            };
            add_path_plugin(plugins, &action.plugin_type);
            let json = encode_path_config(plugins);
            // AB Compare configs are JSON, not file paths — validation won't reject
            let _ = state
                .app
                .set_plugin_param_string(plugin_idx, param_idx, json);
            state.app.plugin_state.ab_add_menu_target = None;
        });
        cx.notify();
    }

    pub(crate) fn on_ab_path_remove_plugin(
        &mut self,
        action: &crate::components::plugins::actions::ABPathRemovePlugin,
        _: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        use sotf_audio_player::controllers::ab_compare_path::{
            encode_path_config, remove_path_plugin,
        };
        let plugin_idx = action.plugin_idx;
        let param_idx: usize = if action.path == 0 { 9 } else { 10 };

        self.state.update(cx, |state, _cx| {
            let plugins = if action.path == 0 {
                state.app.plugin_state.ab_compare_file_a = None;
                &mut state.app.plugin_state.ab_path_a
            } else {
                state.app.plugin_state.ab_compare_file_b = None;
                &mut state.app.plugin_state.ab_path_b
            };
            remove_path_plugin(plugins, action.sub_idx);
            let json = encode_path_config(plugins);
            let _ = state
                .app
                .set_plugin_param_string(plugin_idx, param_idx, json);
        });
        cx.notify();
    }

    pub(crate) fn on_ab_path_move_plugin(
        &mut self,
        action: &crate::components::plugins::actions::ABPathMovePlugin,
        _: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        use sotf_audio_player::controllers::ab_compare_path::{
            encode_path_config, move_path_plugin,
        };
        let plugin_idx = action.plugin_idx;
        let param_idx: usize = if action.path == 0 { 9 } else { 10 };

        self.state.update(cx, |state, _cx| {
            let plugins = if action.path == 0 {
                state.app.plugin_state.ab_compare_file_a = None;
                &mut state.app.plugin_state.ab_path_a
            } else {
                state.app.plugin_state.ab_compare_file_b = None;
                &mut state.app.plugin_state.ab_path_b
            };
            move_path_plugin(plugins, action.from, action.to);
            let json = encode_path_config(plugins);
            let _ = state
                .app
                .set_plugin_param_string(plugin_idx, param_idx, json);
        });
        cx.notify();
    }

    pub(crate) fn on_ab_path_toggle_add_menu(
        &mut self,
        action: &crate::components::plugins::actions::ABPathToggleAddMenu,
        _: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        use crate::app::state::plugin::ABPathTarget;
        self.state.update(cx, |state, _cx| {
            let target = if action.path == 0 {
                ABPathTarget::A
            } else {
                ABPathTarget::B
            };
            state.app.plugin_state.ab_add_menu_target =
                if state.app.plugin_state.ab_add_menu_target == Some(target) {
                    None
                } else {
                    Some(target)
                };
        });
        cx.notify();
    }

    /// Recalculate library pagination based on current layout.
    /// Uses the responsive scale to compute card sizes that match rem-based rendering.
    pub(crate) fn recalculate_pagination(&self, cx: &mut Context<Self>, force_reset: bool) {
        let (window_width, window_height, font_scale, min_font_px, max_font_px) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.window_width,
                state.app.ui_state.window_height,
                state.app.ui_state.font_scale,
                state.app.ui_state.min_font_size_px,
                state.app.ui_state.max_font_size_px,
            )
        };

        let (columns, rows) = estimate_grid_dimensions(
            window_width,
            window_height,
            font_scale,
            min_font_px,
            max_font_px,
        );

        self.state.update(cx, |state, _cx| {
            let app = &mut state.app;
            app.library_state.library_columns = columns;

            // Initial load: 3 screens worth of items
            let new_items_per_page = columns * rows * 3;

            // Only update if we are initializing, resizing significantly, or forcing reset
            if force_reset || app.library_state.items_per_page < new_items_per_page {
                app.library_state.items_per_page = new_items_per_page;
            }
        });
    }

    /// Load more albums (infinite scroll)
    pub(crate) fn load_more_albums(&self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            let app = &mut state.app;
            let total = app.filtered_albums().len();
            if app.library_state.items_per_page < total {
                // Add 5 rows worth of items
                let more = app.library_state.library_columns * 5;
                app.library_state.items_per_page =
                    (app.library_state.items_per_page + more).min(total);
            }
        });
    }
}

pub(crate) mod layout_tree;

// Split impl blocks for PlayerView
include!("handle.rs");
include!("playback.rs");
include!("plugin.rs");
include!("render.rs");
include!("search.rs");
include!("select.rs");
include!("split_view.rs");
include!("switch.rs");
include!("tick.rs");
include!("three_panel_layout.rs");
include!("volume.rs");
