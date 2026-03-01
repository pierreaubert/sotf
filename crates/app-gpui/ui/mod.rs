use crate::app::types::PluginUpdateType;
use crate::app::{AppState, Screen};
use crate::components::plugins::common::param_index_to_engine_param;

// Re-export modules for backward compatibility with crate::ui::components, etc.
pub use crate::components;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::theme::ThemeState as UiKitThemeState;
use gpui_ui_kit::{CollapseDirection, PaneDivider, PaneDividerTheme};
use std::time::Duration;

// Re-export all actions for backward compatibility
pub use crate::app::actions::*;
use crate::components::plugins::actions::{
    ResetPluginParam, SelectPluginParam, StartKnobDrag, UpdatePluginParam,
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
    /// Task for debounced window geometry saving
    geometry_save_task: Option<Task<()>>,
}

impl PlayerView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let search_focus_handle = cx.focus_handle();
        let volume_focus_handle = cx.focus_handle();

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
                            let rows = (item_count + columns - 1) / columns;
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

                    // Consolidate all state updates into a single update call
                    // to avoid multiple observer triggers
                    view.state.update(cx, |state, _cx| {
                        // Playback state update (inlined from update_playback_state)
                        let frame_count = view.update_frame_count;
                        let should_update_spectrum = frame_count % 2 == 0;
                        let include_spectrum = should_update_spectrum
                            && (state.app.spectrum_visible
                                || state.app.ui_state.current_screen == Screen::Spectrum);

                        let mut player = state.player.lock();
                        let playback_state = player.get_playback_state();

                        let was_playing = state.app.playback.is_playing;
                        state.app.playback.is_playing = playback_state.is_playing;
                        state.app.playback.position_secs = playback_state.position_secs;
                        state.app.playback.duration_secs = state.app.get_current_track_duration();

                        // Read analyzer data from the shared cache (no audio pipeline blocking)
                        if playback_state.is_playing {
                            let chain = &state.app.plugin_state.chain;

                            if let Some(idx) = chain.input_monitor_engine_index() {
                                if let Some(data) = player.get_cached_plugin_data(idx) {
                                    if let Some(loudness) =
                                        data.downcast_ref::<sotf_audio_player::LoudnessData>()
                                    {
                                        state.app.playback.input_loudness_info =
                                            Some(loudness.clone());
                                    }
                                }
                            }

                            if let Some(idx) = chain.output_monitor_engine_index() {
                                if let Some(data) = player.get_cached_plugin_data(idx) {
                                    if let Some(loudness) =
                                        data.downcast_ref::<sotf_audio_player::LoudnessData>()
                                    {
                                        state.app.playback.loudness_info = Some(loudness.clone());
                                    }
                                }
                            }

                            if include_spectrum {
                                if let Some(idx) = chain.spectrum_engine_index() {
                                    state.app.playback.spectrum_info =
                                        player.get_cached_plugin_data(idx).and_then(|d| {
                                            d.downcast_ref::<sotf_audio_player::SpectrumData>()
                                                .cloned()
                                        });
                                }
                            }

                            if let Some(idx) = chain.compressor_engine_index() {
                                state.app.playback.compressor_info =
                                    player.get_cached_plugin_data(idx).and_then(|d| {
                                        d.downcast_ref::<sotf_plugins::CompressorData>().cloned()
                                    });
                            }
                        }

                        drop(player);

                        state.app.update_level_meter_groups();
                        state.app.update_level_meter_peak_hold();

                        if let Some(update_type) =
                            state.app.plugin_state.pending_plugin_update.take()
                        {
                            log::warn!("[GPUI] Applying pending plugin update: {:?}", update_type);
                            Self::apply_plugin_update(state, update_type);
                        }

                        // Check and record play history (30s threshold)
                        if state.app.playback.is_playing && playback_state.is_playing {
                            state.app.check_and_record_play();
                        }

                        // Engine crash handling (priority: fatal > error > restarted > auto-advance)
                        if playback_state.engine_fatal {
                            log::error!("[GPUI] Engine crashed fatally, cannot auto-restart");
                            state.app.playback.is_playing = false;
                            state.app.ui_state.toast_message =
                                Some(crate::app::ToastMessage::error(
                                    "Audio engine crashed. Please play a new track to restart.",
                                ));
                        } else if let Some(ref err) = playback_state.last_error {
                            log::error!("[GPUI] Playback error: {}", err);
                            state.app.playback.is_playing = false;
                            state.app.ui_state.toast_message = Some(
                                crate::app::ToastMessage::error(format!("Playback error: {}", err)),
                            );
                        } else if playback_state.engine_restarted {
                            log::info!(
                                "[GPUI] Engine auto-restarted after crash, resuming playback"
                            );
                            state.app.ui_state.toast_message =
                                Some(crate::app::ToastMessage::info(
                                    "Engine restarted, resuming playback",
                                ));
                        } else if was_playing
                            && !playback_state.is_playing
                            && state.app.playback.current_queue_index.is_some()
                        {
                            // Check if playback ended and auto-advance
                            state.app.stop_track_tracking();
                            if let Some(path) = state.app.next_track() {
                                Self::play_track(state, path);
                            } else {
                                state.app.playback.is_playing = false;
                            }
                        }

                        // Startup database check
                        state.app.check_library_on_startup();

                        // Managers update
                        state.app.scan_ctrl.update_all();
                        state.app.update_library_scan();
                        state.app.update_toast();

                        // Ensure library cache is valid (recomputes if invalidated by events)
                        state.app.library_state.ensure_cache_valid();

                        // Check for pending stats from background task
                        let pending = state.app.pending_library_stats.lock().take();
                        if let Some(stats) = pending {
                            state.app.library_stats = stats;
                            state.app.library_stats_computing = false;
                        }
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

                    cx.notify();
                });
                // Exit the loop if the view is no longer valid
                if result.is_err() {
                    break;
                }
            }
        })
        .detach();

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
            if let Err(e) = state.app.save_config_with_geometry(&layout, Some(geometry)) {
                log::error!("Failed to save config on quit: {}", e);
            }

            // Stop background managers
            state.app.scan_ctrl.stop_all();

            // Stop audio playback - this stops the audio engine threads
            if let Err(e) = state.player.lock().stop() {
                log::error!("Failed to stop player on quit: {}", e);
            }
        });

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

    fn cycle_theme(&mut self, _: &CycleTheme, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.app.next_theme();
            let layout = state.layout.read(cx);
            if let Err(e) = state.app.save_config(&layout) {
                log::error!("Failed to save config: {}", e);
            }
        });
        cx.notify();
    }

    fn cycle_language(&mut self, _: &CycleLanguage, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.app.next_language();
            let layout = state.layout.read(cx);
            if let Err(e) = state.app.save_config(&layout) {
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
            if let Err(e) = state.app.save_config(&layout) {
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
            if let Err(e) = state.app.save_config(&layout) {
                log::error!("Failed to save config: {}", e);
            }
        });
        cx.notify();
    }

    fn reset_font_size(&mut self, _: &ResetFontSize, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.app.ui_state.font_scale = 1.0;
            let layout = state.layout.read(cx);
            if let Err(e) = state.app.save_config(&layout) {
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
        }
        cx.notify();
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
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
                state.app.remove_from_queue(state.app.selected_queue_index);
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

    fn add_directory(&mut self, _: &AddDirectory, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            use crate::app::InputMode;
            // Enter add directory mode
            state.app.ui_state.input_mode = InputMode::AddDirectory;
            state.app.input_state.directory_input.clear();
            state.app.clear_autocomplete();
        });
        cx.notify();
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
            if let Err(e) = state.app.save_config(&layout) {
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
            if let Err(e) = state.app.save_config(&layout) {
                log::warn!("Failed to save config: {}", e);
            }
        });
        cx.notify();
    }

    pub(crate) fn play_track(state: &mut AppState, path: std::path::PathBuf) {
        Self::play_track_at(state, path, None);
    }

    pub(crate) fn play_track_at(
        state: &mut AppState,
        path: std::path::PathBuf,
        position: Option<f64>,
    ) {
        let track_sample_rate = state
            .app
            .playback
            .current_queue_index
            .and_then(|idx| state.app.queue.get(idx))
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

        let track_channels = state
            .app
            .playback
            .current_queue_index
            .and_then(|idx| state.app.queue.get(idx))
            .and_then(|item| item.current_track())
            .and_then(|track| track.channels)
            .unwrap_or(2) as usize;
        state
            .app
            .plugin_state
            .chain
            .adapt_matrix_to_input(track_channels);
        let mut output_channels = state
            .app
            .plugin_state
            .chain
            .output_channels_for_input(track_channels);

        // Clamp output channels to device max — the playback thread will
        // downmix automatically when the processing chain outputs more
        // channels than the hardware supports.
        if let Some(max_ch) = state.app.get_device_max_channels() {
            if output_channels > max_ch {
                log::info!(
                    "[GPUI] Clamping output from {} to {} channels (device limit)",
                    output_channels,
                    max_ch
                );
                output_channels = max_ch;
            }
        }

        // Apply ReplayGain correction to the permanent Gain plugin
        let rg_gain = state
            .app
            .playback
            .current_queue_index
            .and_then(|idx| state.app.queue.get(idx))
            .and_then(|item| item.current_track())
            .and_then(|track| state.app.playback.get_replay_gain_adjustment(track));
        state.app.plugin_state.chain.set_replay_gain(rg_gain);

        let plugins = state
            .app
            .plugin_state
            .chain
            .to_plugin_configs(sample_rate);

        if let Err(e) = state.player.lock().load_and_play_at(
            path.clone(),
            plugins,
            output_channels,
            device_name,
            position,
        ) {
            log::error!("Failed to play track: {}", e);
            state.app.playback.is_playing = false;
            state
                .app
                .record_playback_error(format!("Play track failed: {}", e));
        } else {
            state.app.playback.is_playing = true;
            if let Some(queue_index) = state.app.playback.current_queue_index {
                state
                    .app
                    .record_playback_started(queue_index, Some(path.clone()));
            }
            state.app.start_track_tracking(path);
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
            state.app.is_dragging_knob = true;
            state.app.knob_drag_plugin_idx = action.plugin_idx;
            state.app.knob_drag_param_idx = action.param_idx;
            state.app.knob_drag_start_y = Some(action.start_y);
            state.app.knob_drag_start_value = action.start_value;
            state.app.knob_drag_min = action.min;
            state.app.knob_drag_max = action.max;
        });
        cx.notify();
    }

    /// Recalculate library pagination based on current layout.
    /// Uses the responsive scale to compute card sizes that match rem-based rendering.
    pub(crate) fn recalculate_pagination(&self, cx: &mut Context<Self>, force_reset: bool) {
        let (window_width, window_height, font_scale) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.window_width,
                state.app.ui_state.window_height,
                state.app.ui_state.font_scale,
            )
        };

        self.state.update(cx, |state, _cx| {
            let app = &mut state.app;

            let responsive_scale = compute_responsive_scale(window_width, window_height);
            let effective_rem = 16.0 * font_scale * responsive_scale;

            // Card width in pixels: 8.75 rem (matching album_card.rs)
            // Plus gap_4 = 1rem gap
            let card_px = 8.75 * effective_rem;
            let gap_px = 1.0 * effective_rem;
            let card_with_gap = card_px + gap_px;

            // Approximate total horizontal chrome: grid p_2 (0.5rem × 2 sides) + parent padding
            let available_width = window_width - 2.0 * effective_rem;
            let columns = (available_width / card_with_gap).floor().max(1.0) as usize;
            app.library_state.library_columns = columns;

            // Estimate available height for grid (header/footer areas scale too)
            let chrome_height = 18.0 * effective_rem; // ~290px at base scale
            let available_height = (window_height - chrome_height).max(16.0 * effective_rem);
            let card_height = 11.25 * effective_rem; // ~180px at base (thumbnail + text)
            let rows = (available_height / card_height).floor().max(1.0) as usize;

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

// Split impl blocks for PlayerView
include!("handle.rs");
include!("playback.rs");
include!("plugin.rs");
include!("render.rs");
include!("search.rs");
include!("select.rs");
include!("split_view.rs");
include!("switch.rs");
include!("three_panel_layout.rs");
include!("volume.rs");
