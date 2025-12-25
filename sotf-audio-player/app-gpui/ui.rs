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
    last_saved_window_bounds: Option<Bounds<Pixels>>,
    /// Scroll handle for library grid view
    pub(crate) grid_scroll_handle: ScrollHandle,
    /// Track if we've done initial focus (for macOS menu activation)
    needs_initial_focus: bool,
}

impl PlayerView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        // Register plugin interactions
        // Register plugin interactions - moved to render

        // Observe state changes to trigger re-renders when state is updated
        // from callbacks (like Select toggles in AutoEqForm)
        cx.observe(&state, |_, _, cx| {
            cx.notify();
        })
        .detach();

        // Set up periodic update timer for playback position and loudness
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let result = this.update(cx, |view, cx| {
                    view.update_playback_state(cx);

                    // Update waveform scanner and check startup database state
                    view.state.update(cx, |state, _| {
                        // Perform deferred database check on first update
                        state.app.check_library_on_startup();

                        state.app.waveform_manager.update();
                        state.app.replay_gain_manager.update();
                        state.app.bliss_manager.update();
                        state.app.update_library_scan();
                        state.app.update_toast();
                    });

                    // Infinite scroll check
                    if view.state.read(cx).app.current_screen == Screen::Library {
                        let scroll_y: f32 = view.grid_scroll_handle.offset().y.into();
                        let state = view.state.read(cx);
                        let item_count = state.app.library_items_per_page;
                        let total_albums = state.app.filtered_albums().len();
                        let columns = state.app.library_columns.max(1);
                        let rows = (item_count + columns - 1) / columns;
                        let card_height = 220.0; // Card (180px) + gap (16px) + margin
                        let estimated_height = rows as f32 * card_height;
                        let window_height = state.app.window_height;

                        // scroll_y is negative when scrolling down
                        let scroll_position = scroll_y.abs();

                        // Calculate how far the user can scroll (content beyond viewport)
                        let scrollable_distance = (estimated_height - window_height).max(0.0);
                        // How much scroll room remains below current position
                        let remaining_scroll = scrollable_distance - scroll_position;

                        // Load more if:
                        // 1. Content doesn't fill viewport + 1 screen buffer (preload ahead)
                        // 2. User has scrolled and is within 1000px of the bottom
                        let needs_more_content = estimated_height < window_height * 2.0;
                        let near_bottom = remaining_scroll < 1000.0;
                        let should_load =
                            item_count < total_albums && (needs_more_content || near_bottom);

                        // If we should load more, do it
                        if should_load {
                            view.state
                                .update(cx, |state, _| state.app.load_more_albums());
                        }
                    }
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
            last_saved_window_bounds: None,
            grid_scroll_handle: ScrollHandle::new(),
            needs_initial_focus: true,
        }
    }

    pub(crate) fn toggle_playback(
        &mut self,
        _: &PlayPause,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            if state.app.is_playing {
                let _ = state.player.lock().pause();
                state.app.is_playing = false;
            } else {
                let _ = state.player.lock().resume();
                state.app.is_playing = true;
            }
        });
        cx.notify();
    }

    fn stop_playback(&mut self, _: &Stop, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            let _ = state.player.lock().stop();
            state.app.is_playing = false;
            state.app.current_queue_index = None;
        });
        cx.notify();
    }

    pub(crate) fn next_track(&mut self, _: &NextTrack, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Block if in text input mode
            if Self::is_text_input_mode(state.app.input_mode) {
                return;
            }
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
                    log::error!("Failed to play next track: {}", e);
                    state.app.is_playing = false;
                }
            } else {
                state.app.is_playing = false;
            }
        });
        cx.notify();
    }

    pub(crate) fn prev_track(&mut self, _: &PrevTrack, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Block if in text input mode
            if Self::is_text_input_mode(state.app.input_mode) {
                return;
            }
            if let Some(path) = state.app.previous_track() {
                let sample_rate = 48000.0;
                let plugins = state.app.plugin_chain.to_plugin_configs(sample_rate);
                let output_channels = state.app.plugin_chain.output_channels();

                if let Err(e) = state.player.lock().load_and_play(
                    path,
                    plugins,
                    output_channels,
                    state.app.current_output_device_name.clone(),
                ) {
                    log::error!("Failed to play previous track: {}", e);
                    state.app.is_playing = false;
                }
            } else {
                state.app.is_playing = false;
            }
        });
        cx.notify();
    }

    fn adjust_volume(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.volume = (state.app.volume + delta).clamp(0.0, 1.0);
            let _ = state.player.lock().set_volume(state.app.volume);
        });
        cx.notify();
    }

    fn volume_up(&mut self, _: &VolumeUp, _: &mut Window, cx: &mut Context<Self>) {
        self.adjust_volume(0.05, cx);
    }

    fn volume_down(&mut self, _: &VolumeDown, _: &mut Window, cx: &mut Context<Self>) {
        self.adjust_volume(-0.05, cx);
    }

    fn volume_up_small(&mut self, _: &VolumeUpSmall, _: &mut Window, cx: &mut Context<Self>) {
        self.adjust_volume(0.01, cx);
    }

    fn volume_down_small(&mut self, _: &VolumeDownSmall, _: &mut Window, cx: &mut Context<Self>) {
        self.adjust_volume(-0.01, cx);
    }

    pub(crate) fn switch_screen(&mut self, screen: Screen, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if state.app.current_screen != screen {
                state.app.last_screen = state.app.current_screen;
                state.app.current_screen = screen;
            }
        });
        cx.notify();
    }

    fn switch_to_library(&mut self, _: &SwitchToLibrary, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen(Screen::Library, cx);
    }

    fn switch_to_queue(&mut self, _: &SwitchToQueue, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen(Screen::Queue, cx);
    }

    fn switch_to_plugins(&mut self, _: &SwitchToPlugins, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.current_screen = Screen::Settings;
            state.app.active_settings_tab = crate::app::SettingsTab::AudioDevice;
        });
        cx.notify();
    }

    fn switch_to_studio(&mut self, _: &SwitchToStudio, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen(Screen::Studio, cx);
    }

    fn switch_to_plugin_graph(
        &mut self,
        _: &SwitchToPluginGraph,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen(Screen::PluginGraph, cx);
    }

    fn switch_to_spinorma(&mut self, _: &SwitchToSpinorma, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen(Screen::Spinorama, cx);
    }

    fn switch_to_devices(&mut self, _: &SwitchToDevices, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.current_screen = Screen::Settings;
            state.app.active_settings_tab = crate::app::SettingsTab::AudioDevice;
        });
        cx.notify();
    }

    fn switch_to_directory_manager(
        &mut self,
        _: &SwitchToDirectoryManager,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen(Screen::DirectoryManager, cx);
    }

    fn switch_to_settings(&mut self, _: &SwitchToSettings, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen(Screen::Settings, cx);
    }

    fn switch_to_recording(
        &mut self,
        _: &SwitchToRecording,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen(Screen::Recording, cx);
    }

    fn switch_to_room_eq(&mut self, _: &SwitchToRoomEQ, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen(Screen::RoomEq, cx);
    }

    fn switch_to_headphone_eq(
        &mut self,
        _: &SwitchToHeadphoneEQ,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen(Screen::HeadphoneEq, cx);
    }

    fn open_config(&mut self, _: &OpenConfig, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen(Screen::Settings, cx);
    }

    pub(crate) fn quit_app(&mut self, _: &QuitApp, window: &mut Window, cx: &mut Context<Self>) {
        // Save window geometry before quitting
        let window_bounds = window.bounds();
        let geometry = crate::config::WindowGeometry {
            x: window_bounds.origin.x.into(),
            y: window_bounds.origin.y.into(),
            width: window_bounds.size.width.into(),
            height: window_bounds.size.height.into(),
        };

        self.state.update(cx, |state, _cx| {
            if let Err(e) = state.app.save_config_with_geometry(Some(geometry)) {
                log::error!("Failed to save config on quit: {}", e);
            }
        });

        cx.quit();
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
                state.app.input_mode = crate::app::InputMode::Normal;
            } else {
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

    /// Render split view with Library on top and Queue on bottom (expanded mode)
    fn render_split_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, queue_ratio) = {
            let state = self.state.read(cx);
            (state.app.theme.clone(), state.app.queue_panel_ratio)
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            // Global mouse move handler for divider and volume dragging
            .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, window, cx| {
                let (
                    is_dragging_divider,
                    is_dragging_queue_list,
                    is_dragging_meters,
                    is_dragging_lufs,
                    is_dragging_volume,
                    volume_start_y,
                    volume_start_value,
                    window_height,
                    meters_ratio,
                ) = {
                    let state = view.state.read(cx);
                    (
                        state.app.is_dragging_queue_divider,
                        state.app.is_dragging_queue_list_divider,
                        state.app.is_dragging_meters_divider,
                        state.app.is_dragging_lufs_divider,
                        state.app.is_dragging_volume,
                        state.app.volume_drag_start_y,
                        state.app.volume_drag_start_value,
                        state.app.window_height,
                        state.app.meters_panel_ratio,
                    )
                };

                let window_size = window.bounds().size;
                let mouse_pos = event.position;
                let is_compact_height = window_height < 600.0;

                if is_dragging_divider {
                    let window_height = window_size.height;
                    let mouse_y: f32 = mouse_pos.y.into();
                    let window_h: f32 = window_height.into();
                    // Calculate new ratio (inverted because queue is at bottom)
                    let new_ratio = (1.0 - (mouse_y / window_h)).clamp(0.15, 0.6);
                    view.state.update(cx, |state, _cx| {
                        state.app.queue_panel_ratio = new_ratio;
                    });
                    cx.notify();
                }

                if is_dragging_queue_list {
                    let window_width: f32 = window_size.width.into();
                    let mouse_x: f32 = mouse_pos.x.into();
                    let new_ratio = (mouse_x / window_width).clamp(0.1, 0.5);
                    view.state.update(cx, |state, _cx| {
                        state.app.queue_list_ratio = new_ratio;
                    });
                    cx.notify();
                }

                if is_dragging_meters {
                    let window_width: f32 = window_size.width.into();
                    let mouse_x: f32 = mouse_pos.x.into();
                    // Meters are on the right, so ratio is from the right edge
                    let right_edge_ratio = (1.0 - (mouse_x / window_width)).clamp(0.1, 0.8);

                    view.state.update(cx, |state, _cx| {
                        if is_compact_height {
                            // In 4-col mode, Divider 2 controls total right width (LUFS + Meters)
                            // lufs_ratio = total - meters_ratio
                            let new_lufs = (right_edge_ratio - meters_ratio).max(0.05);
                            state.app.lufs_panel_ratio = new_lufs;
                        } else {
                            // Standard mode: controls combined panel width
                            state.app.meters_panel_ratio = right_edge_ratio.clamp(0.1, 0.5);
                        }
                    });
                    cx.notify();
                }

                if is_dragging_lufs {
                    let window_width: f32 = window_size.width.into();
                    let mouse_x: f32 = mouse_pos.x.into();
                    // Divider 3 (LUFS <-> Meters) controls meters_panel_ratio
                    let new_meters = (1.0 - (mouse_x / window_width)).clamp(0.05, 0.5);
                    view.state.update(cx, |state, _cx| {
                        state.app.meters_panel_ratio = new_meters;
                    });
                    cx.notify();
                }

                // Handle volume dragging (drag up = increase, drag down = decrease)
                if is_dragging_volume {
                    if let Some(start_y) = volume_start_y {
                        let mouse_y: f32 = mouse_pos.y.into();
                        let delta_y = start_y - mouse_y; // Inverted: up = positive
                        // Scale: 100px drag = full volume range
                        let volume_delta = delta_y / 100.0;
                        let new_volume = (volume_start_value + volume_delta).clamp(0.0, 1.0);
                        view.state.update(cx, |state, _cx| {
                            state.app.volume = new_volume;
                            let _ = state.player.lock().set_volume(new_volume);
                        });
                        cx.notify();
                    }
                }
            }))
            // Global mouse up handler to stop dragging even if mouse is outside divider
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        if state.app.is_dragging_queue_divider {
                            state.app.is_dragging_queue_divider = false;
                            // Save the new layout
                            if let Err(e) = state.app.save_config() {
                                log::warn!("Failed to save panel layout: {}", e);
                            }
                        }
                        if state.app.is_dragging_queue_list_divider {
                            // Check for click vs drag
                            let was_click = state
                                .app
                                .divider_click_start
                                .map(|start| start.elapsed().as_millis() < 200)
                                .unwrap_or(false);

                            if was_click {
                                if state.app.queue_list_ratio > 0.05 {
                                    state.app.queue_list_ratio = 0.0;
                                } else {
                                    state.app.queue_list_ratio = 0.30; // Restore default
                                }
                            }

                            state.app.is_dragging_queue_list_divider = false;
                            if let Err(e) = state.app.save_config() {
                                log::warn!("Failed to save panel layout: {}", e);
                            }
                        }
                        if state.app.is_dragging_meters_divider {
                            // Check for click vs drag
                            let was_click = state
                                .app
                                .divider_click_start
                                .map(|start| start.elapsed().as_millis() < 200)
                                .unwrap_or(false);

                            if was_click {
                                if state.app.meters_panel_ratio > 0.05 {
                                    state.app.meters_panel_ratio = 0.0;
                                } else {
                                    state.app.meters_panel_ratio = 0.25; // Restore default
                                }
                            }

                            state.app.is_dragging_meters_divider = false;
                            if let Err(e) = state.app.save_config() {
                                log::warn!("Failed to save panel layout: {}", e);
                            }
                        }
                        if state.app.is_dragging_lufs_divider {
                            state.app.is_dragging_lufs_divider = false;
                            if let Err(e) = state.app.save_config() {
                                log::warn!("Failed to save panel layout: {}", e);
                            }
                        }
                        if state.app.is_dragging_volume {
                            state.app.is_dragging_volume = false;
                            state.app.volume_drag_start_y = None;
                        }
                    });
                }),
            )
            // Top section: Library (takes remaining space)
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.render_library_screen(cx)),
            )
            // Resize handle
            .child({
                let library_collapsed = queue_ratio > 0.9;
                let divider_theme = PaneDividerTheme {
                    background: theme.background,
                    background_hover: theme.surface_hover,
                    background_collapsed: theme.surface,
                    foreground: theme.text_muted,
                    foreground_hover: theme.text_secondary,
                    border: theme.border,
                };
                PaneDivider::horizontal("library-queue-divider", CollapseDirection::Up)
                    .label("Library")
                    .collapsed(library_collapsed)
                    .theme(divider_theme)
                    .on_toggle({
                        let state = self.state.clone();
                        move |collapsed, _window, cx| {
                            state.update(cx, |state, _| {
                                state.app.queue_panel_ratio = if collapsed { 0.95 } else { 0.35 };
                                let _ = state.app.save_config();
                            });
                        }
                    })
                    .on_drag_start({
                        let state = self.state.clone();
                        move |_pos, _window, cx| {
                            state.update(cx, |state, _| {
                                state.app.is_dragging_queue_divider = true;
                                state.app.divider_click_start = Some(std::time::Instant::now());
                            });
                        }
                    })
            })
            // Bottom section: Queue (configurable height ratio)
            .child(
                div()
                    .h(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                        queue_ratio,
                    )))
                    .border_t_1()
                    .border_color(theme.border)
                    .overflow_hidden()
                    .child(self.render_queue_screen(cx)),
            )
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

    fn cycle_sort_order(&mut self, _: &CycleSortOrder, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Block if in text input mode
            if Self::is_text_input_mode(state.app.input_mode) {
                return;
            }
            use crate::app::LibrarySortOrder;
            let next_order = match state.app.library_sort_order {
                LibrarySortOrder::Year => LibrarySortOrder::Genre,
                LibrarySortOrder::Genre => LibrarySortOrder::Artist,
                LibrarySortOrder::Artist => LibrarySortOrder::Album,
                LibrarySortOrder::Album => LibrarySortOrder::Tracks,
                LibrarySortOrder::Tracks => LibrarySortOrder::Composer,
                LibrarySortOrder::Composer => LibrarySortOrder::Popularity,
                LibrarySortOrder::Popularity => LibrarySortOrder::Year,
            };
            state.app.set_library_sort_order(next_order);
        });
        cx.notify();
    }

    fn set_sort_artist(&mut self, _: &SetSortArtist, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_library_sort_order(crate::app::LibrarySortOrder::Artist);
        });
        cx.notify();
    }

    fn set_sort_album(&mut self, _: &SetSortAlbum, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_library_sort_order(crate::app::LibrarySortOrder::Album);
        });
        cx.notify();
    }

    fn set_sort_title(&mut self, _: &SetSortTitle, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Title sorting now uses Album sort (sorts by album title)
            state
                .app
                .set_library_sort_order(crate::app::LibrarySortOrder::Album);
        });
        cx.notify();
    }

    fn set_sort_year(&mut self, _: &SetSortYear, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_library_sort_order(crate::app::LibrarySortOrder::Year);
        });
        cx.notify();
    }

    fn cycle_channel_filter(
        &mut self,
        _: &CycleChannelFilter,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            // Block if in text input mode
            if Self::is_text_input_mode(state.app.input_mode) {
                return;
            }
            state.app.cycle_channel_filter();
        });
        cx.notify();
    }

    fn set_filter_all(&mut self, _: &SetFilterAll, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.set_channel_filter(crate::app::ChannelFilter::All);
        });
        cx.notify();
    }

    fn set_filter_mono(&mut self, _: &SetFilterMono, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_channel_filter(crate::app::ChannelFilter::Mono);
        });
        cx.notify();
    }

    fn set_filter_stereo(&mut self, _: &SetFilterStereo, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_channel_filter(crate::app::ChannelFilter::Stereo);
        });
        cx.notify();
    }

    fn set_filter_surround(
        &mut self,
        _: &SetFilterSurround,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_channel_filter(crate::app::ChannelFilter::Surround);
        });
        cx.notify();
    }

    fn set_filter_surround71(
        &mut self,
        _: &SetFilterSurround71,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_channel_filter(crate::app::ChannelFilter::Surround71);
        });
        cx.notify();
    }

    fn set_filter_surround_plus(
        &mut self,
        _: &SetFilterSurroundPlus,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_channel_filter(crate::app::ChannelFilter::SurroundPlus);
        });
        cx.notify();
    }

    fn set_filter_mixed(&mut self, _: &SetFilterMixed, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_channel_filter(crate::app::ChannelFilter::Mixed);
        });
        cx.notify();
    }

    /// Check if we're in a text input mode where actions should be blocked
    fn is_text_input_mode(input_mode: crate::app::InputMode) -> bool {
        use crate::app::InputMode;
        matches!(
            input_mode,
            InputMode::Search
                | InputMode::AddDirectory
                | InputMode::SavePlugins
                | InputMode::LoadPlugins
                | InputMode::LoadApoFile
                | InputMode::LoadSofaFile
                | InputMode::SpinoramaSpeakerSearch
        )
    }

    fn handle_search_input(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // Handle text input for search mode
        match event.keystroke.key.as_str() {
            "backspace" => {
                self.state.update(cx, |state, _cx| {
                    state.app.search_query.pop();
                    state.app.selected_album_index = 0;
                    state.app.reset_page();
                });
                cx.notify();
            }
            "escape" => {
                // Exit search mode and clear search
                self.state.update(cx, |state, _cx| {
                    state.app.input_mode = crate::app::InputMode::Normal;
                    state.app.search_query.clear();
                    state.app.selected_album_index = 0;
                    state.app.reset_page();
                });
                cx.notify();
            }
            "enter" => {
                // Exit search mode but keep search results
                self.state.update(cx, |state, _cx| {
                    state.app.input_mode = crate::app::InputMode::Normal;
                });
                cx.notify();
            }
            _ => {
                // Add character to search query
                if let Some(text) = event.keystroke.key_char.as_ref() {
                    self.state.update(cx, |state, _cx| {
                        state.app.search_query.push_str(text);
                        state.app.selected_album_index = 0;
                        state.app.reset_page();
                    });
                    cx.notify();
                }
            }
        }
    }

    fn handle_directory_input(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // Handle text input for add directory mode
        match event.keystroke.key.as_str() {
            "backspace" => {
                self.state.update(cx, |state, _cx| {
                    state.app.directory_input.pop();
                    state.app.clear_autocomplete();
                });
                cx.notify();
            }
            "tab" => {
                // Tab autocomplete
                self.state.update(cx, |state, _cx| {
                    if state.app.autocomplete_suggestions.is_empty() {
                        state.app.generate_autocomplete_suggestions();
                    } else {
                        state.app.next_autocomplete();
                    }
                });
                cx.notify();
            }
            "escape" => {
                // Already handled by Cancel action
            }
            "enter" => {
                // Already handled by Enter action (adds directory)
            }
            _ => {
                // Add character to directory input
                if let Some(text) = event.keystroke.key_char.as_ref() {
                    self.state.update(cx, |state, _cx| {
                        state.app.directory_input.push_str(text);
                        state.app.clear_autocomplete();
                    });
                    cx.notify();
                }
            }
        }
    }

    fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Block if in text input mode
            if Self::is_text_input_mode(state.app.input_mode) {
                return;
            }
            match state.app.current_screen {
                Screen::Library => {
                    state.app.select_next_album();
                }
                Screen::Queue => {
                    state.app.select_next_queue_item();
                }
                Screen::DirectoryManager => {
                    state.app.select_next_directory();
                }
                _ => {}
            }
        });
        cx.notify();
    }

    fn select_prev(&mut self, _: &SelectPrev, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Block if in text input mode
            if Self::is_text_input_mode(state.app.input_mode) {
                return;
            }
            match state.app.current_screen {
                Screen::Library => {
                    state.app.select_previous_album();
                }
                Screen::Queue => {
                    state.app.select_previous_queue_item();
                }
                Screen::DirectoryManager => {
                    state.app.select_previous_directory();
                }
                _ => {}
            }
        });
        cx.notify();
    }

    fn select_next_page(&mut self, _: &SelectNextPage, _: &mut Window, cx: &mut Context<Self>) {
        // Grid uses rows × columns for page size
        const GRID_COLUMNS: usize = 7;
        const GRID_PAGE_ROWS: usize = 3;
        const LIST_PAGE_SIZE: usize = 20;

        self.state
            .update(cx, |state, _cx| match state.app.current_screen {
                Screen::Library => {
                    // Grid view: move by full rows
                    state.app.page_down_albums(GRID_COLUMNS * GRID_PAGE_ROWS);
                }
                Screen::Queue => {
                    state.app.page_down_queue(LIST_PAGE_SIZE);
                }
                Screen::DirectoryManager => {
                    state.app.page_down_directories(LIST_PAGE_SIZE);
                }
                _ => {}
            });
        cx.notify();
    }

    fn select_prev_page(&mut self, _: &SelectPrevPage, _: &mut Window, cx: &mut Context<Self>) {
        // Grid uses rows × columns for page size
        const GRID_COLUMNS: usize = 7;
        const GRID_PAGE_ROWS: usize = 3;
        const LIST_PAGE_SIZE: usize = 20;

        self.state
            .update(cx, |state, _cx| match state.app.current_screen {
                Screen::Library => {
                    // Grid view: move by full rows
                    state.app.page_up_albums(GRID_COLUMNS * GRID_PAGE_ROWS);
                }
                Screen::Queue => {
                    state.app.page_up_queue(LIST_PAGE_SIZE);
                }
                Screen::DirectoryManager => {
                    state.app.page_up_directories(LIST_PAGE_SIZE);
                }
                _ => {}
            });
        cx.notify();
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if state.app.current_screen == Screen::Library {
                state.app.select_grid_left();
            }
        });
        cx.notify();
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if state.app.current_screen == Screen::Library {
                state.app.select_grid_right();
            }
        });
        cx.notify();
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if state.app.current_screen == Screen::Library {
                state.app.select_grid_up();
            }
        });
        cx.notify();
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if state.app.current_screen == Screen::Library {
                state.app.select_grid_down();
            }
        });
        cx.notify();
    }

    fn toggle_expand(&mut self, _: &ToggleExpand, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if state.app.current_screen == Screen::Queue {
                state.app.toggle_queue_item_expansion();
            } else if state.app.current_screen == Screen::DirectoryManager {
                state.app.toggle_directory_expansion();
            }
        });
        cx.notify();
    }

    fn update_playback_state(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Get playback state (include spectrum data when on spectrum screen)
            let include_spectrum =
                state.app.spectrum_visible || state.app.current_screen == Screen::Spectrum;
            let playback_state = state.player.lock().get_playback_state(include_spectrum);

            state.app.position_secs = playback_state.position_secs;
            state.app.loudness_info = playback_state.loudness;
            state.app.duration_secs = state.app.get_current_track_duration();

            // Update level meter groups based on channel count
            state.app.update_level_meter_groups();

            if include_spectrum {
                state.app.spectrum_info = playback_state.spectrum;
            }

            // Apply pending plugin updates to audio engine
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
        });
        cx.notify();
    }

    /// Apply a pending plugin update to the audio engine.
    /// Called from update_playback_state when there's a pending update.
    fn apply_plugin_update(state: &mut AppState, update_type: PluginUpdateType) {
        let result = match update_type {
            PluginUpdateType::Parameter {
                plugin_index,
                param_index,
            } => {
                // Zero-dropout individual parameter update
                if let Some(plugin) = state.app.plugin_chain.get_plugin(plugin_index) {
                    // We must map the UI index to the Engine index because the Engine reorders plugins
                    // (analyzers moved to the end) and filters out disabled ones.
                    if let Some(engine_index) =
                        state.app.plugin_chain.get_engine_index(plugin_index)
                    {
                        if let Some((param_id, value)) =
                            param_index_to_engine_param(&plugin.settings, param_index)
                        {
                            state
                                .player
                                .lock()
                                .set_plugin_parameter(engine_index, param_id, value)
                        } else {
                            // Parameter not supported for individual update, fall back to structural
                            let sample_rate = 48000.0;
                            let plugins = state.app.plugin_chain.to_plugin_configs(sample_rate);
                            state.player.lock().update_plugins(plugins)
                        }
                    } else {
                        // Plugin is disabled or not found in engine map - ignore or full update
                        Ok(())
                    }
                } else {
                    Ok(()) // Plugin not found, ignore
                }
            }
            PluginUpdateType::Structural => {
                // Full plugin chain rebuild
                let sample_rate = 48000.0;
                let plugins = state.app.plugin_chain.to_plugin_configs(sample_rate);
                log::warn!(
                    "[GPUI] Structural update: sending {} plugins to engine (expected output: {} channels)",
                    plugins.len(),
                    state.app.plugin_chain.output_channels()
                );
                state.player.lock().update_plugins(plugins)
            }
        };

        if let Err(e) = result {
            log::warn!("Failed to apply plugin update: {}", e);
        }
    }

    fn remove_item(&mut self, _: &RemoveItem, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Block if in text input mode
            if Self::is_text_input_mode(state.app.input_mode) {
                return;
            }
            match state.app.current_screen {
                Screen::Queue => {
                    state.app.remove_from_queue(state.app.selected_queue_index);
                }
                Screen::DirectoryManager => {
                    state.app.remove_selected_directory();
                }
                _ => {}
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
        self.state.update(cx, |state, _cx| {
            match state.app.fill_queue_magic() {
                Ok(count) => {
                    log::info!("[UI] fill_queue_magic added {} tracks", count);
                }
                Err(e) => {
                    log::error!("[UI] fill_queue_magic error: {}", e);
                }
            }
        });
        cx.notify();
    }

    fn move_plugin_up(&mut self, _: &MovePluginUp, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.move_plugin_up(state.app.selected_plugin_index);
        });
        cx.notify();
    }

    fn move_plugin_down(&mut self, _: &MovePluginDown, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.move_plugin_down(state.app.selected_plugin_index);
        });
        cx.notify();
    }

    fn toggle_plugin(&mut self, _: &TogglePlugin, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.toggle_plugin(state.app.selected_plugin_index);
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

    // Quick plugin add shortcuts
    fn quick_add_eq(&mut self, _: &QuickAddEQ, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.add_plugin(&sotf_audio_player::PluginType::EQ);
        });
        cx.notify();
    }

    fn quick_add_upmixer(&mut self, _: &QuickAddUpmixer, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .add_plugin(&sotf_audio_player::PluginType::Upmixer);
        });
        cx.notify();
    }

    fn quick_add_compressor(
        &mut self,
        _: &QuickAddCompressor,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .add_plugin(&sotf_audio_player::PluginType::Compressor);
        });
        cx.notify();
    }

    fn quick_add_gate(&mut self, _: &QuickAddGate, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.add_plugin(&sotf_audio_player::PluginType::Gate);
        });
        cx.notify();
    }

    fn quick_add_limiter(&mut self, _: &QuickAddLimiter, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .add_plugin(&sotf_audio_player::PluginType::Limiter);
        });
        cx.notify();
    }

    fn quick_add_loudness(&mut self, _: &QuickAddLoudness, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .add_plugin(&sotf_audio_player::PluginType::LoudnessCompensation);
        });
        cx.notify();
    }

    fn quick_add_binaural(&mut self, _: &QuickAddBinaural, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .add_plugin(&sotf_audio_player::PluginType::BinauralDecoder);
        });
        cx.notify();
    }

    // Level meter actions
    fn select_next_meter_group(
        &mut self,
        _: &SelectNextMeterGroup,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state.app.select_next_level_meter_group();
        });
        cx.notify();
    }

    fn select_prev_meter_group(
        &mut self,
        _: &SelectPrevMeterGroup,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state.app.select_previous_level_meter_group();
        });
        cx.notify();
    }

    fn toggle_meter_mute(&mut self, _: &ToggleMeterMute, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.toggle_level_meter_mute();
        });
        cx.notify();
    }

    fn toggle_meter_solo(&mut self, _: &ToggleMeterSolo, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.toggle_level_meter_solo();
        });
        cx.notify();
    }

    fn toggle_meter_dim(&mut self, _: &ToggleMeterDim, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.toggle_level_meter_dim();
        });
        cx.notify();
    }

    fn clear_meter_mutes_solos(
        &mut self,
        _: &ClearMeterMutesSolos,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state.app.clear_level_meter_mutes_and_solos();
        });
        cx.notify();
    }

    fn handle_apo_file_input(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // Handle text input for APO file loading mode
        match event.keystroke.key.as_str() {
            "backspace" => {
                self.state.update(cx, |state, _cx| {
                    state.app.apo_file_input.pop();
                });
                cx.notify();
            }
            "tab" => {
                // TODO: Add file autocomplete support
            }
            "escape" => {
                // Already handled by Cancel action
            }
            "enter" => {
                // Load the APO file
                self.state
                    .update(cx, |state, _cx| match state.app.load_apo_file() {
                        Ok(()) => {
                            state.app.toast_message = Some(crate::app::ToastMessage::success(
                                "APO file loaded successfully",
                            ));
                            state.app.apo_file_input.clear();
                            state.app.input_mode = crate::app::InputMode::Normal;
                        }
                        Err(e) => {
                            state.app.toast_message = Some(crate::app::ToastMessage::error(
                                format!("Failed to load APO file: {}", e),
                            ));
                        }
                    });
                cx.notify();
            }
            _ => {
                // Add character to input
                if let Some(text) = event.keystroke.key_char.as_ref() {
                    self.state.update(cx, |state, _cx| {
                        state.app.apo_file_input.push_str(text);
                    });
                    cx.notify();
                }
            }
        }
    }

    fn handle_sofa_file_input(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // Handle text input for SOFA file loading mode
        match event.keystroke.key.as_str() {
            "backspace" => {
                self.state.update(cx, |state, _cx| {
                    state.app.sofa_file_input.pop();
                });
                cx.notify();
            }
            "tab" => {
                // TODO: Add file autocomplete support
            }
            "escape" => {
                // Already handled by Cancel action
            }
            "enter" => {
                // Load the SOFA file
                self.state
                    .update(cx, |state, _cx| match state.app.load_sofa_file() {
                        Ok(()) => {
                            state.app.toast_message = Some(crate::app::ToastMessage::success(
                                "SOFA file loaded successfully",
                            ));
                            state.app.sofa_file_input.clear();
                            state.app.input_mode = crate::app::InputMode::Normal;
                        }
                        Err(e) => {
                            state.app.toast_message = Some(crate::app::ToastMessage::error(
                                format!("Failed to load SOFA file: {}", e),
                            ));
                        }
                    });
                cx.notify();
            }
            _ => {
                // Add character to input
                if let Some(text) = event.keystroke.key_char.as_ref() {
                    self.state.update(cx, |state, _cx| {
                        state.app.sofa_file_input.push_str(text);
                    });
                    cx.notify();
                }
            }
        }
    }

    pub(crate) fn handle_spinorama_speaker_search_input(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) {
        log::info!(
            "[SPINORAMA] handle_spinorama_speaker_search_input called, key={}",
            event.keystroke.key
        );
        // Handle text input for spinorama speaker search mode
        match event.keystroke.key.as_str() {
            "backspace" => {
                log::info!("[SPINORAMA] Backspace pressed");
                self.state.update(cx, |state, _cx| {
                    state.app.spinorama_eq_state.speaker_search.pop();
                    state.app.spinorama_eq_state.update_suggestions();
                });
                cx.notify();
            }
            "escape" => {
                log::info!("[SPINORAMA] Escape pressed - exiting search mode");
                // Exit search mode
                self.state.update(cx, |state, _cx| {
                    state.app.input_mode = crate::app::InputMode::Normal;
                });
                cx.notify();
            }
            "enter" => {
                log::info!("[SPINORAMA] Enter pressed - exiting search mode");
                // Exit search mode, keep current search results
                self.state.update(cx, |state, _cx| {
                    state.app.input_mode = crate::app::InputMode::Normal;
                });
                cx.notify();
            }
            _ => {
                // Add character to search query using key_char (handles all printable chars including space)
                if let Some(text) = event.keystroke.key_char.as_ref() {
                    log::info!("[SPINORAMA] Character typed: '{}'", text);
                    self.state.update(cx, |state, _cx| {
                        state.app.spinorama_eq_state.speaker_search.push_str(text);
                        state.app.spinorama_eq_state.update_suggestions();
                    });
                    cx.notify();
                }
            }
        }
    }

    fn handle_save_plugins_input(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // Handle text input for save plugins mode
        match event.keystroke.key.as_str() {
            "backspace" => {
                self.state.update(cx, |state, _cx| {
                    state.app.plugin_file_input.pop();
                    state.app.clear_autocomplete();
                });
                cx.notify();
            }
            "tab" => {
                // Autocomplete from available presets
                self.state.update(cx, |state, _cx| {
                    if state.app.autocomplete_suggestions.is_empty() {
                        state
                            .app
                            .generate_autocomplete_suggestions_for_save_preset();
                        if !state.app.autocomplete_suggestions.is_empty() {
                            state.app.apply_autocomplete_to_plugin_file();
                        }
                    } else {
                        state.app.next_autocomplete_for_plugin_file();
                    }
                });
                cx.notify();
            }
            "escape" => {
                self.state.update(cx, |state, _cx| {
                    state.app.input_mode = crate::app::InputMode::Normal;
                    state.app.plugin_file_input.clear();
                    state.app.clear_autocomplete();
                    state.app.pending_studio_close = false; // Cancel close if save cancelled
                });
                cx.notify();
            }
            "enter" => {
                self.state.update(cx, |state, _cx| {
                    // If there are presets shown and input is empty, use selected preset (overwrite)
                    if state.app.plugin_file_input.is_empty()
                        && !state.app.available_plugin_presets.is_empty()
                    {
                        state.app.save_selected_preset();
                    } else if !state.app.plugin_file_input.is_empty() {
                        state.app.save_plugin_chain();
                    }
                    state.app.input_mode = crate::app::InputMode::Normal;
                    state.app.clear_autocomplete();
                    state.app.plugin_chain_modified = false;

                    if state.app.pending_studio_close {
                        state.app.pending_studio_close = false;
                        state.app.current_screen = state.app.last_screen;
                    }
                });
                cx.notify();
            }
            "up" => {
                // Navigate preset list when input is empty
                self.state.update(cx, |state, _cx| {
                    if state.app.plugin_file_input.is_empty()
                        && !state.app.available_plugin_presets.is_empty()
                    {
                        state.app.select_previous_preset();
                    }
                });
                cx.notify();
            }
            "down" => {
                // Navigate preset list when input is empty
                self.state.update(cx, |state, _cx| {
                    if state.app.plugin_file_input.is_empty()
                        && !state.app.available_plugin_presets.is_empty()
                    {
                        state.app.select_next_preset();
                    }
                });
                cx.notify();
            }
            _ => {
                // Add character to input
                if let Some(text) = event.keystroke.key_char.as_ref() {
                    self.state.update(cx, |state, _cx| {
                        state.app.plugin_file_input.push_str(text);
                        state.app.clear_autocomplete();
                    });
                    cx.notify();
                }
            }
        }
    }

    fn handle_load_plugins_input(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // Handle text input for load plugins mode
        match event.keystroke.key.as_str() {
            "backspace" => {
                self.state.update(cx, |state, _cx| {
                    state.app.plugin_file_input.pop();
                    state.app.clear_autocomplete();
                });
                cx.notify();
            }
            "tab" => {
                // Autocomplete file path
                self.state.update(cx, |state, _cx| {
                    if !state.app.plugin_file_input.is_empty() {
                        if state.app.autocomplete_suggestions.is_empty() {
                            state
                                .app
                                .generate_autocomplete_suggestions_for_plugin_file();
                            if !state.app.autocomplete_suggestions.is_empty() {
                                state.app.apply_autocomplete_to_plugin_file();
                            }
                        } else {
                            state.app.next_autocomplete_for_plugin_file();
                        }
                    }
                });
                cx.notify();
            }
            "escape" => {
                self.state.update(cx, |state, _cx| {
                    state.app.input_mode = crate::app::InputMode::Normal;
                    state.app.plugin_file_input.clear();
                    state.app.clear_autocomplete();
                });
                cx.notify();
            }
            "enter" => {
                self.state.update(cx, |state, _cx| {
                    // If there are presets shown and input is empty, load selected preset
                    if state.app.plugin_file_input.is_empty()
                        && !state.app.available_plugin_presets.is_empty()
                    {
                        state.app.load_selected_preset();
                    } else if !state.app.plugin_file_input.is_empty() {
                        state.app.load_plugin_chain();
                    }
                    state.app.input_mode = crate::app::InputMode::Normal;
                    state.app.clear_autocomplete();
                });
                cx.notify();
            }
            "up" | "k" => {
                // Navigate through presets
                self.state.update(cx, |state, _cx| {
                    if state.app.plugin_file_input.is_empty() {
                        state.app.select_previous_preset();
                    }
                });
                cx.notify();
            }
            "down" | "j" => {
                // Navigate through presets
                self.state.update(cx, |state, _cx| {
                    if state.app.plugin_file_input.is_empty() {
                        state.app.select_next_preset();
                    }
                });
                cx.notify();
            }
            _ => {
                // Add character to input
                if let Some(text) = event.keystroke.key_char.as_ref() {
                    self.state.update(cx, |state, _cx| {
                        state.app.plugin_file_input.push_str(text);
                        state.app.clear_autocomplete();
                    });
                    cx.notify();
                }
            }
        }
    }

    fn handle_enter(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            use crate::app::InputMode;

            // Block action if in text input modes (where typing should take priority)
            match state.app.input_mode {
                InputMode::Search
                | InputMode::SavePlugins
                | InputMode::LoadPlugins
                | InputMode::LoadApoFile
                | InputMode::LoadSofaFile => {
                    // Don't execute action - these modes handle Enter themselves via keyboard handlers
                    return;
                }
                InputMode::AddDirectory => {
                    // Add the directory
                    if !state.app.directory_input.is_empty() {
                        let path = std::path::PathBuf::from(&state.app.directory_input);
                        state.app.add_directory(path);
                        state.app.directory_input.clear();
                        state.app.clear_autocomplete();
                    }
                    state.app.input_mode = InputMode::Normal;
                    return;
                }
                _ => {
                    // Continue to handle screen-specific actions
                }
            }

            // Handle screen-specific actions in Normal mode
            match state.app.current_screen {
                Screen::Library => {
                    // Add selected album to queue
                    if let Some(path) = state.app.add_album_to_queue() {
                        Self::play_track(state, path);
                    }
                }
                Screen::Queue => {
                    // Play selected track in queue
                    // TODO: Implement playing specific track from queue
                }
                Screen::Settings => {
                    // Enter key in Settings screen - no action needed
                }
                _ => {}
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

impl Render for PlayerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Focus view on first render to activate macOS menu bar
        if self.needs_initial_focus {
            self.needs_initial_focus = false;
            self.focus_handle.focus(window, cx);
            window.activate_window();
            cx.activate(true);
        }

        // Update layout mode based on window height
        let window_bounds = window.bounds();
        let window_height: f32 = window_bounds.size.height.into();
        let window_width: f32 = window_bounds.size.width.into();
        self.state.update(cx, |state, _cx| {
            state.app.window_height = window_height;
            state.app.window_width = window_width;
            state.app.layout_mode = if window_height >= 800.0 {
                crate::app::LayoutMode::Expanded
            } else {
                crate::app::LayoutMode::Compact
            };

            // Recalculate pagination based on new window size
            state.app.recalculate_pagination(false);
        });

        // Save window geometry if it has changed (debounced by checking if different)
        let should_save = match self.last_saved_window_bounds {
            None => true,
            Some(last_bounds) => {
                let pos_changed = (last_bounds.origin.x - window_bounds.origin.x).abs() > px(1.0)
                    || (last_bounds.origin.y - window_bounds.origin.y).abs() > px(1.0);
                let size_changed = (last_bounds.size.width - window_bounds.size.width).abs()
                    > px(1.0)
                    || (last_bounds.size.height - window_bounds.size.height).abs() > px(1.0);
                pos_changed || size_changed
            }
        };

        if should_save {
            let geometry = crate::config::WindowGeometry {
                x: window_bounds.origin.x.into(),
                y: window_bounds.origin.y.into(),
                width: window_bounds.size.width.into(),
                height: window_bounds.size.height.into(),
            };

            self.state.update(cx, |state, _cx| {
                if let Err(e) = state.app.save_config_with_geometry(Some(geometry)) {
                    log::warn!("Failed to save window geometry: {}", e);
                }
            });

            self.last_saved_window_bounds = Some(window_bounds);
        }

        let (current_screen, input_mode, theme, layout_mode, active_menu) = {
            let state = self.state.read(cx);
            (
                state.app.current_screen,
                state.app.input_mode,
                state.app.theme.clone(),
                state.app.layout_mode,
                state.app.active_menu,
            )
        };

        // Keep gpui-ui-kit global theme in sync with app theme so components get consistent defaults.
        // This allows builder overrides but ensures out-of-the-box colors match the app theme.
        let ui_kit_theme = theme.to_ui_kit_theme(self.state.read(cx).app.theme_id);
        cx.set_global(UiKitThemeState {
            theme: ui_kit_theme,
        });

        // Determine key context based on input mode
        // Use "TextInput" context when typing to disable single-letter keybindings
        let key_context = {
            let state = self.state.read(cx);
            if Self::is_text_input_mode(state.app.input_mode) {
                "TextInput"
            } else {
                "PlayerView"
            }
        };

        div()
            .key_context(key_context)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::toggle_playback))
            .on_action(cx.listener(Self::stop_playback))
            .on_action(cx.listener(Self::next_track))
            .on_action(cx.listener(Self::prev_track))
            .on_action(cx.listener(Self::volume_up))
            .on_action(cx.listener(Self::volume_down))
            .on_action(cx.listener(Self::volume_up_small))
            .on_action(cx.listener(Self::volume_down_small))
            .on_action(cx.listener(Self::switch_to_library))
            .on_action(cx.listener(Self::switch_to_queue))
            .on_action(cx.listener(Self::switch_to_plugins))
            .on_action(cx.listener(Self::switch_to_studio))
            .on_action(cx.listener(Self::switch_to_plugin_graph))
            .on_action(cx.listener(Self::switch_to_devices))
            .on_action(cx.listener(Self::switch_to_directory_manager))
            .on_action(cx.listener(Self::switch_to_settings))
            .on_action(cx.listener(Self::switch_to_recording))
            .on_action(cx.listener(Self::switch_to_room_eq))
            .on_action(cx.listener(Self::switch_to_headphone_eq))
            .on_action(cx.listener(Self::switch_to_spinorma))
            .on_action(cx.listener(Self::open_config))
            .on_action(cx.listener(Self::quit_app))
            .on_action(cx.listener(Self::cycle_theme))
            .on_action(cx.listener(Self::cycle_language))
            .on_action(cx.listener(Self::toggle_search))
            .on_action(cx.listener(Self::toggle_library_view))
            .on_action(cx.listener(Self::toggle_help))
            .on_action(cx.listener(Self::toggle_help_support))
            .on_action(cx.listener(Self::about))
            .on_action(cx.listener(Self::cycle_sort_order))
            .on_action(cx.listener(Self::set_sort_artist))
            .on_action(cx.listener(Self::set_sort_album))
            .on_action(cx.listener(Self::set_sort_title))
            .on_action(cx.listener(Self::set_sort_year))
            .on_action(cx.listener(Self::cycle_channel_filter))
            .on_action(cx.listener(Self::set_filter_all))
            .on_action(cx.listener(Self::set_filter_mono))
            .on_action(cx.listener(Self::set_filter_stereo))
            .on_action(cx.listener(Self::set_filter_surround))
            .on_action(cx.listener(Self::set_filter_surround71))
            .on_action(cx.listener(Self::set_filter_surround_plus))
            .on_action(cx.listener(Self::set_filter_mixed))
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_prev))
            .on_action(cx.listener(Self::select_next_page))
            .on_action(cx.listener(Self::select_prev_page))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::toggle_expand))
            .on_action(cx.listener(Self::handle_enter))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::remove_item))
            .on_action(cx.listener(Self::clear_queue))
            .on_action(cx.listener(Self::fill_queue_magic))
            .on_action(cx.listener(Self::move_plugin_up))
            .on_action(cx.listener(Self::move_plugin_down))
            .on_action(cx.listener(Self::toggle_plugin))
            .on_action(cx.listener(Self::add_directory))
            .on_action(cx.listener(Self::scan_library))
            .on_action(cx.listener(Self::quick_add_eq))
            .on_action(cx.listener(Self::quick_add_upmixer))
            .on_action(cx.listener(Self::quick_add_compressor))
            .on_action(cx.listener(Self::quick_add_gate))
            .on_action(cx.listener(Self::quick_add_limiter))
            .on_action(cx.listener(Self::quick_add_loudness))
            .on_action(cx.listener(Self::quick_add_binaural))
            .on_action(cx.listener(Self::quick_add_binaural))
            // Plugin parameter actions
            .on_action(cx.listener(Self::on_update_plugin_param))
            .on_action(cx.listener(Self::on_select_plugin_param))
            .on_action(cx.listener(Self::on_reset_plugin_param))
            .on_action(cx.listener(Self::on_start_knob_drag))
            // Level meter actions
            .on_action(cx.listener(Self::select_next_meter_group))
            .on_action(cx.listener(Self::select_prev_meter_group))
            .on_action(cx.listener(Self::toggle_meter_mute))
            .on_action(cx.listener(Self::toggle_meter_solo))
            .on_action(cx.listener(Self::toggle_meter_dim))
            .on_action(cx.listener(Self::clear_meter_mutes_solos))
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, _window, cx| {
                // Handle text input for search mode and add directory mode
                let input_mode = view.state.read(cx).app.input_mode;
                let current_screen = view.state.read(cx).app.current_screen;

                match input_mode {
                    crate::app::InputMode::Search => {
                        cx.stop_propagation(); // Prevent actions from processing this keystroke
                        view.handle_search_input(event, cx);
                    }
                    crate::app::InputMode::AddDirectory => {
                        cx.stop_propagation(); // Prevent actions from processing this keystroke
                        view.handle_directory_input(event, cx);
                    }
                    crate::app::InputMode::LoadApoFile => {
                        cx.stop_propagation(); // Prevent actions from processing this keystroke
                        view.handle_apo_file_input(event, cx);
                    }
                    crate::app::InputMode::LoadSofaFile => {
                        cx.stop_propagation(); // Prevent actions from processing this keystroke
                        view.handle_sofa_file_input(event, cx);
                    }
                    crate::app::InputMode::SavePlugins => {
                        cx.stop_propagation(); // Prevent actions from processing this keystroke
                        view.handle_save_plugins_input(event, cx);
                    }
                    crate::app::InputMode::LoadPlugins => {
                        cx.stop_propagation(); // Prevent actions from processing this keystroke
                        view.handle_load_plugins_input(event, cx);
                    }
                    crate::app::InputMode::EditingParam => {
                        // Stepper-based editing doesn't need keyboard input
                    }
                    crate::app::InputMode::SpinoramaSpeakerSearch => {
                        cx.stop_propagation();
                        view.handle_spinorama_speaker_search_input(event, cx);
                    }
                    crate::app::InputMode::Normal => {
                        // Handle screen-specific shortcuts in Normal mode
                        if current_screen == crate::app::Screen::Settings
                            && view
                                .state
                                .read(cx)
                                .app
                                .expanded_settings_sections
                                .contains(&"plugins".to_string())
                        {
                            match event.keystroke.key.as_str() {
                                "S" => {
                                    // Enter save plugins mode (Shift-S)
                                    view.state.update(cx, |state, _cx| {
                                        state.app.refresh_plugin_presets();
                                        state.app.plugin_file_input.clear();
                                        state.app.input_mode = crate::app::InputMode::SavePlugins;
                                    });
                                    cx.notify();
                                }
                                "l" => {
                                    // Enter load plugins mode
                                    view.state.update(cx, |state, _cx| {
                                        state.app.refresh_plugin_presets();
                                        state.app.plugin_file_input.clear();
                                        state.app.input_mode = crate::app::InputMode::LoadPlugins;
                                    });
                                    cx.notify();
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }))
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .text_color(theme.text_primary)
            .when(!cfg!(target_os = "macos"), |div| {
                div.child(self.render_menu_bar(cx))
            })
            .child(self.render_header(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(match layout_mode {
                        crate::app::LayoutMode::Expanded => {
                            // Split view: Library on bottom, Queue on top
                            match current_screen {
                                Screen::Spectrum => {
                                    self.render_spectrum_screen(cx).into_any_element()
                                }
                                Screen::DirectoryManager => {
                                    self.render_directory_screen(cx).into_any_element()
                                }
                                Screen::Settings => {
                                    self.render_settings_screen(cx).into_any_element()
                                }
                                Screen::Studio => self.render_plugins_screen(cx).into_any_element(),
                                Screen::Recording => {
                                    self.render_recording_screen(cx).into_any_element()
                                }
                                Screen::RoomEq => self.render_room_eq_screen(cx).into_any_element(),
                                Screen::HeadphoneEq => {
                                    self.render_headphone_eq_screen(cx).into_any_element()
                                }
                                Screen::Spinorama => {
                                    self.render_spinorama_eq_screen(cx).into_any_element()
                                }
                                Screen::PluginGraph => {
                                    self.render_plugin_graph_screen(cx).into_any_element()
                                }
                                // Default: split Library/Queue view
                                Screen::Library | Screen::Queue => {
                                    self.render_split_view(cx).into_any_element()
                                }
                            }
                        }
                        crate::app::LayoutMode::Compact => {
                            // Single view based on current screen
                            match current_screen {
                                Screen::Library => {
                                    self.render_library_screen(cx).into_any_element()
                                }
                                Screen::Queue => self.render_queue_screen(cx).into_any_element(),
                                Screen::Spectrum => {
                                    self.render_spectrum_screen(cx).into_any_element()
                                }
                                Screen::DirectoryManager => {
                                    self.render_directory_screen(cx).into_any_element()
                                }
                                Screen::Settings => {
                                    self.render_settings_screen(cx).into_any_element()
                                }
                                Screen::Studio => self.render_plugins_screen(cx).into_any_element(),
                                Screen::Recording => {
                                    self.render_recording_screen(cx).into_any_element()
                                }
                                Screen::RoomEq => self.render_room_eq_screen(cx).into_any_element(),
                                Screen::HeadphoneEq => {
                                    self.render_headphone_eq_screen(cx).into_any_element()
                                }
                                Screen::Spinorama => {
                                    self.render_spinorama_eq_screen(cx).into_any_element()
                                }
                                Screen::PluginGraph => {
                                    self.render_plugin_graph_screen(cx).into_any_element()
                                }
                            }
                        }
                    }),
            )
            .child(self.render_footer(cx))
            .when(input_mode == crate::app::InputMode::Help, |div| {
                div.child(self.render_help_modal(cx))
            })
            .when(input_mode == crate::app::InputMode::LoadApoFile, |div| {
                div.child(self.render_apo_file_dialog(cx))
            })
            .when(input_mode == crate::app::InputMode::LoadSofaFile, |div| {
                div.child(self.render_sofa_file_dialog(cx))
            })
            .when(input_mode == crate::app::InputMode::SavePlugins, |div| {
                div.child(self.render_save_plugins_dialog(cx))
            })
            .when(input_mode == crate::app::InputMode::LoadPlugins, |div| {
                div.child(self.render_load_plugins_dialog(cx))
            })
            .when(
                input_mode == crate::app::InputMode::KeyboardShortcuts,
                |div| div.child(self.render_keyboard_shortcuts_dialog(cx)),
            )
            .when(input_mode == crate::app::InputMode::About, |div| {
                div.child(self.render_about_dialog(cx))
            })
            .when(input_mode == crate::app::InputMode::HelpSupport, |div| {
                div.child(self.render_help_support_dialog(cx))
            })
            .when(
                input_mode == crate::app::InputMode::EmptyLibraryPrompt,
                |div| div.child(self.render_empty_library_prompt(cx)),
            )
            .when(
                input_mode == crate::app::InputMode::EditingPluginNode,
                |div| div.child(self.render_plugin_node_modal(cx)),
            )
            // Scan progress modal
            .child(self.render_scan_progress_modal(cx))
            .child(self.render_toast(cx))
            .when(self.state.read(cx).app.context_menu.is_some(), |div| {
                div.child(self.render_context_menu(cx))
            })
            // Studio menu overlay (click outside to close)
            .when(self.state.read(cx).app.show_studio_menu, |div| {
                div.child(self.render_studio_menu_overlay(cx))
            })
            // Device popup overlay (click outside to close)
            .when(self.state.read(cx).app.show_device_popup, |div| {
                div.child(self.render_device_popup_overlay(cx))
            })
            // Device popup (rendered here to be above overlay)
            .when(self.state.read(cx).app.show_device_popup, |div| {
                let translations = &self.state.read(cx).app.translations;
                div.child(self.render_device_popup(translations.playback_output_devices, cx))
            })
            // Menu dropdowns rendered last for z-ordering
            .when(active_menu != crate::app::ActiveMenu::None, |div| {
                div.child(self.render_menu_dropdowns(cx))
            })
    }
}
