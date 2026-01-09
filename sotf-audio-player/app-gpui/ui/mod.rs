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

pub struct PlayerView {
    pub(crate) state: Entity<AppState>,
    pub(crate) focus_handle: FocusHandle,
    pub(crate) volume_focus_handle: FocusHandle,
    last_saved_window_bounds: Option<Bounds<Pixels>>,
    /// Scroll handle for library grid view
    pub(crate) grid_scroll_handle: ScrollHandle,
    /// Track if we've done initial focus (for macOS menu activation)
    needs_initial_focus: bool,
    /// Frame counter for throttling updates (increments every 100ms)
    update_frame_count: u64,
}

impl PlayerView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let volume_focus_handle = cx.focus_handle();

        // Register plugin interactions
        // Register plugin interactions - moved to render

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
                        if view.state.read(cx).app.current_screen == Screen::Library {
                            let scroll_y: f32 = view.grid_scroll_handle.offset().y.into();
                            let state = view.state.read(cx);
                            let item_count = state.app.library_items_per_page;
                            let total_albums = state.app.filtered_albums().len();
                            let columns = state.app.library_columns.max(1);
                            let rows = (item_count + columns - 1) / columns;
                            let card_height = 220.0;
                            let estimated_height = rows as f32 * card_height;
                            let window_height = state.app.window_height;
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
                                || state.app.current_screen == Screen::Spectrum);

                        let playback_state =
                            state.player.lock().get_playback_state(include_spectrum);

                        state.app.position_secs = playback_state.position_secs;
                        state.app.duration_secs = state.app.get_current_track_duration();

                        if playback_state.input_loudness.is_some() {
                            let _ = state.app.input_loudness_info.take();
                            state.app.input_loudness_info = playback_state.input_loudness;
                        }

                        if playback_state.output_loudness.is_some() {
                            let _ = state.app.loudness_info.take();
                            state.app.loudness_info = playback_state.output_loudness;
                        }

                        state.app.update_level_meter_groups();

                        if include_spectrum {
                            let _ = state.app.spectrum_info.take();
                            state.app.spectrum_info = playback_state.spectrum;
                        }

                        state.app.compressor_info = playback_state.compressor;

                        if let Some(update_type) = state.app.pending_plugin_update.take() {
                            log::warn!("[GPUI] Applying pending plugin update: {:?}", update_type);
                            Self::apply_plugin_update(state, update_type);
                        }

                        // Check if playback ended and auto-advance
                        if state.app.is_playing
                            && !playback_state.is_playing
                            && state.app.current_queue_index.is_some()
                        {
                            if let Some(path) = state.app.next_track() {
                                let sample_rate = 48000.0;
                                let plugins = state.app.plugin_chain.to_plugin_configs(sample_rate);
                                let output_channels = state.app.plugin_chain.output_channels();

                                if let Err(e) = state.player.lock().load_and_play(
                                    path,
                                    plugins,
                                    output_channels,
                                    state.app.current_output_device_name.clone(),
                                ) {
                                    log::error!("Failed to auto-advance: {}", e);
                                    state.app.is_playing = false;
                                }
                            } else {
                                state.app.is_playing = false;
                            }
                        }

                        // Startup database check
                        state.app.check_library_on_startup();

                        // Managers update
                        state.app.waveform_manager.update();
                        state.app.replay_gain_manager.update();
                        state.app.bliss_manager.update();
                        state.app.update_library_scan();
                        state.app.update_toast();

                        // Infinite scroll - load more albums if needed
                        if scroll_check_data == Some(true) {
                            state.app.load_more_albums();
                        }
                    });

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
            volume_focus_handle,
            last_saved_window_bounds: None,
            grid_scroll_handle: ScrollHandle::new(),
            needs_initial_focus: true,
            update_frame_count: 0,
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

        self.state.update(cx, |state, _cx| {
            // Save configuration
            if let Err(e) = state.app.save_config_with_geometry(Some(geometry)) {
                log::error!("Failed to save config on quit: {}", e);
            }

            // Stop background managers
            state.app.waveform_manager.stop();
            state.app.replay_gain_manager.stop();
            state.app.bliss_manager.stop();

            // Stop audio playback - this stops the audio engine threads
            if let Err(e) = state.player.lock().stop() {
                log::error!("Failed to stop player on quit: {}", e);
            }
        });

        log::info!("Services stopped, quitting application...");

        // Request GPUI to quit
        cx.quit();

        // Force immediate process exit
        // cx.quit() may not terminate if background threads (audio engine, etc.) are still running.
        // We've already stopped the services above, so it's safe to exit immediately.
        log::info!("Force exiting process");
        std::process::exit(0);
    }

    fn cycle_theme(&mut self, _: &CycleTheme, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.next_theme();
            if let Err(e) = state.app.save_config() {
                log::error!("Failed to save config: {}", e);
            }
        });
        cx.notify();
    }

    fn cycle_language(&mut self, _: &CycleLanguage, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.next_language();
            if let Err(e) = state.app.save_config() {
                log::error!("Failed to save config: {}", e);
            }
        });
        cx.notify();
    }

    fn toggle_search(&mut self, _: &ToggleSearch, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if state.app.input_mode == crate::app::InputMode::Search {
                log::info!("toggle_search: exiting search mode");
                state.app.input_mode = crate::app::InputMode::Normal;
            } else {
                log::info!("toggle_search: entering search mode");
                state.app.input_mode = crate::app::InputMode::Search;
                state.app.search_query.clear();
            }
        });
        cx.notify();
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.input_mode = crate::app::InputMode::Normal;
            state.app.search_query.clear();
            state.app.directory_input.clear();
            state.app.apo_file_input.clear();
            state.app.sofa_file_input.clear();
            state.app.clear_autocomplete();
            state.app.dismiss_toast();
            state.app.context_menu = None; // Close context menu
            state.app.active_menu = crate::app::ActiveMenu::None; // Close dropdown menus
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
            if state.app.input_mode == InputMode::Help {
                state.app.input_mode = InputMode::Normal;
            } else {
                state.app.input_mode = InputMode::Help;
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
            if state.app.input_mode == InputMode::HelpSupport {
                state.app.input_mode = InputMode::Normal;
            } else {
                state.app.input_mode = InputMode::HelpSupport;
            }
        });
        cx.notify();
    }

    fn about(&mut self, _: &About, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            use crate::app::InputMode;
            if state.app.input_mode == InputMode::About {
                state.app.input_mode = InputMode::Normal;
            } else {
                state.app.input_mode = InputMode::About;
            }
        });
        cx.notify();
    }

    fn toggle_expand(&mut self, _: &ToggleExpand, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if state.app.current_screen == Screen::Queue {
                state.app.toggle_queue_item_expansion();
            }
        });
        cx.notify();
    }

    fn remove_item(&mut self, _: &RemoveItem, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Block if in text input mode
            if Self::is_text_input_mode(state.app.input_mode) {
                return;
            }
            if state.app.current_screen == Screen::Queue {
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
            state.app.input_mode = InputMode::AddDirectory;
            state.app.directory_input.clear();
            state.app.clear_autocomplete();
        });
        cx.notify();
    }

    fn scan_library(&mut self, _: &ScanLibrary, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Start scan (this will be async in reality, but for now we do it synchronously)
            if let Err(e) = state.app.scan_library() {
                log::error!("Library scan failed: {}", e);
                state.app.toast_message = Some(crate::app::ToastMessage::error(format!(
                    "Scan failed: {}",
                    e
                )));
            }
            // Save directories to config after successful scan
            if let Err(e) = state.app.save_config() {
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
                state.app.toast_message = Some(crate::app::ToastMessage::error(format!(
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
            if let Err(e) = state.app.save_config() {
                log::warn!("Failed to save config: {}", e);
            }
        });
        cx.notify();
    }

    pub(crate) fn play_track(state: &mut AppState, path: std::path::PathBuf) {
        let sample_rate = 48000.0;
        let plugins = state.app.plugin_chain.to_plugin_configs(sample_rate);
        let output_channels = state.app.plugin_chain.output_channels();

        log::warn!(
            "[GPUI] play_track: starting with {} plugins, output_channels={}",
            plugins.len(),
            output_channels
        );

        if let Err(e) = state.player.lock().load_and_play(
            path,
            plugins,
            output_channels,
            state.app.current_output_device_name.clone(),
        ) {
            log::error!("Failed to play track: {}", e);
            state.app.is_playing = false;
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
            state.app.editing_plugin_index = Some(action.plugin_idx);
            state.app.plugin_param_selection = action.param_idx;
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
include!("volume.rs");
