use crate::app::{AppState, Screen};

// Re-export modules for backward compatibility with crate::ui::components, etc.
pub use crate::components;
pub use crate::plugins;
pub use crate::screens;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::Divider;
use std::time::Duration;

// Re-export all actions for backward compatibility
pub use crate::actions::*;

pub struct PlayerView {
    pub(crate) state: Entity<AppState>,
    pub(crate) focus_handle: FocusHandle,
    last_saved_window_bounds: Option<Bounds<Pixels>>,
    /// Scroll handle for library grid view
    pub(crate) grid_scroll_handle: ScrollHandle,
}

impl PlayerView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        // Set up periodic update timer for playback position and loudness
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let result = this.update(cx, |view, cx| {
                    view.update_playback_state(cx);

                    // Infinite scroll check
                    if view.state.read(cx).app.current_screen == Screen::Library {
                        let scroll_y: f32 = view.grid_scroll_handle.offset().y.into();
                        let state = view.state.read(cx);
                        let item_count = state.app.library_items_per_page;
                        let columns = state.app.library_columns.max(1);
                        let rows = (item_count + columns - 1) / columns;
                        let estimated_height = rows as f32 * 260.0; // Approx card height + gap
                        let window_height = state.app.window_height;

                        // If we are within 1000px of the bottom, load more
                        if scroll_y.abs() + window_height > estimated_height - 1000.0 {
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

    pub(crate) fn switch_screen(&mut self, screen: Screen, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.current_screen = screen;
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
            // Expand the plugins section
            if !state
                .app
                .expanded_settings_sections
                .contains(&"plugins".to_string())
            {
                state
                    .app
                    .expanded_settings_sections
                    .push("plugins".to_string());
            }
        });
        cx.notify();
    }

    fn switch_to_devices(&mut self, _: &SwitchToDevices, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.current_screen = Screen::Settings;
            // Expand the audio-device section
            if !state
                .app
                .expanded_settings_sections
                .contains(&"audio-device".to_string())
            {
                state
                    .app
                    .expanded_settings_sections
                    .push("audio-device".to_string());
            }
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
            .child(self.render_horizontal_divider(theme.clone(), cx))
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

    /// Render a horizontal divider that can be dragged to resize panels
    /// Click to toggle library visibility
    fn render_horizontal_divider(
        &self,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        Divider::new()
            .id("queue-divider")
            .color(theme.border)
            .hover_color(theme.accent)
            .thickness(px(6.0))
            .interactive()
            .build()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _: &MouseDownEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.is_dragging_queue_divider = true;
                        state.app.divider_click_start = Some(std::time::Instant::now());
                    });
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        // Check if this was a click (short duration, no significant drag)
                        let was_click = state
                            .app
                            .divider_click_start
                            .map(|start| start.elapsed().as_millis() < 200)
                            .unwrap_or(false);

                        if was_click && state.app.is_dragging_queue_divider {
                            // Toggle library visibility by adjusting queue ratio
                            if state.app.queue_panel_ratio < 0.9 {
                                // Hide library (maximize queue)
                                state.app.queue_panel_ratio = 0.95;
                            } else {
                                // Show library (restore default)
                                state.app.queue_panel_ratio = 0.35;
                            }
                        }

                        state.app.is_dragging_queue_divider = false;
                        state.app.divider_click_start = None;

                        // Save the new layout
                        if let Err(e) = state.app.save_config() {
                            log::warn!("Failed to save panel layout: {}", e);
                        }
                    });
                    cx.notify();
                }),
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

    fn cycle_sort_order(&mut self, _: &CycleSortOrder, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            use crate::app::LibrarySortOrder;
            let next_order = match state.app.library_sort_order {
                LibrarySortOrder::Artist => LibrarySortOrder::Album,
                LibrarySortOrder::Album => LibrarySortOrder::Title,
                LibrarySortOrder::Title => LibrarySortOrder::Year,
                LibrarySortOrder::Year => LibrarySortOrder::Artist,
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
            state
                .app
                .set_library_sort_order(crate::app::LibrarySortOrder::Title);
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

    fn set_filter_multichannel(
        &mut self,
        _: &SetFilterMultichannel,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_channel_filter(crate::app::ChannelFilter::Multichannel);
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

    fn handle_search_input(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // Handle text input for search mode
        match event.keystroke.key.as_str() {
            "backspace" => {
                self.state.update(cx, |state, _cx| {
                    state.app.search_query.pop();
                    state.app.selected_album_index = 0; // Reset selection when query changes
                });
                cx.notify();
            }
            "escape" => {
                // Already handled by Cancel action
            }
            "enter" => {
                // Already handled by Enter action (exits search mode)
            }
            _ => {
                // Add character to search query
                if let Some(text) = event.keystroke.key_char.as_ref() {
                    self.state.update(cx, |state, _cx| {
                        state.app.search_query.push_str(text);
                        state.app.selected_album_index = 0; // Reset selection when query changes
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
        self.state
            .update(cx, |state, _cx| match state.app.current_screen {
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
            });
        cx.notify();
    }

    fn select_prev(&mut self, _: &SelectPrev, _: &mut Window, cx: &mut Context<Self>) {
        self.state
            .update(cx, |state, _cx| match state.app.current_screen {
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

    fn remove_item(&mut self, _: &RemoveItem, _: &mut Window, cx: &mut Context<Self>) {
        self.state
            .update(cx, |state, _cx| match state.app.current_screen {
                Screen::Queue => {
                    state.app.remove_from_queue(state.app.selected_queue_index);
                }
                Screen::DirectoryManager => {
                    state.app.remove_selected_directory();
                }
                _ => {}
            });
        cx.notify();
    }

    fn clear_queue(&mut self, _: &ClearQueue, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.clear_queue();
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

    fn edit_plugin(&mut self, _: &EditPlugin, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.enter_plugin_edit_mode();
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

    fn handle_plugin_edit_input(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // Handle key input for plugin edit mode
        match event.keystroke.key.as_str() {
            "up" | "k" => {
                self.state.update(cx, |state, _cx| {
                    state.app.select_previous_param();
                });
                cx.notify();
            }
            "down" | "j" => {
                self.state.update(cx, |state, _cx| {
                    state.app.select_next_param();
                });
                cx.notify();
            }
            "left" | "h" => {
                self.state.update(cx, |state, _cx| {
                    state.app.adjust_selected_param(-1.0);
                });
                cx.notify();
            }
            "right" | "l" => {
                self.state.update(cx, |state, _cx| {
                    state.app.adjust_selected_param(1.0);
                });
                cx.notify();
            }
            "a" => {
                // Load APO file (for EQ plugins)
                self.state.update(cx, |state, _cx| {
                    use crate::app::InputMode;
                    use sotf_audio_player::PluginSettings;
                    if let Some(plugin) = state.app.get_editing_plugin() {
                        if matches!(plugin.settings, PluginSettings::EQ { .. }) {
                            state.app.input_mode = InputMode::LoadApoFile;
                            state.app.apo_file_input.clear();
                            state.app.toast_message =
                                Some(crate::app::ToastMessage::info("Enter path to APO file:"));
                        } else {
                            state.app.toast_message = Some(crate::app::ToastMessage::warning(
                                "APO files can only be loaded for EQ plugins",
                            ));
                        }
                    }
                });
                cx.notify();
            }
            "f" => {
                // Load SOFA file (for Binaural Decoder plugins)
                self.state.update(cx, |state, _cx| {
                    use crate::app::InputMode;
                    use sotf_audio_player::PluginSettings;
                    if let Some(plugin) = state.app.get_editing_plugin() {
                        if matches!(plugin.settings, PluginSettings::BinauralDecoder { .. }) {
                            state.app.input_mode = InputMode::LoadSofaFile;
                            state.app.sofa_file_input.clear();
                            state.app.toast_message =
                                Some(crate::app::ToastMessage::info("Enter path to SOFA file:"));
                        } else {
                            state.app.toast_message = Some(crate::app::ToastMessage::warning(
                                "SOFA files can only be loaded for Binaural Decoder plugins",
                            ));
                        }
                    }
                });
                cx.notify();
            }
            "escape" => {
                // Already handled by Cancel action
            }
            _ => {}
        }
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
                            state.app.input_mode = crate::app::InputMode::EditPlugin;
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
                            state.app.input_mode = crate::app::InputMode::EditPlugin;
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

            // Handle input modes first
            if state.app.input_mode == InputMode::AddDirectory {
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
                    // If Plugins section is expanded, enter plugin edit mode
                    if state
                        .app
                        .expanded_settings_sections
                        .contains(&"plugins".to_string())
                    {
                        state.app.enter_plugin_edit_mode();
                    }
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
}

impl Render for PlayerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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

        div()
            .key_context("PlayerView")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::toggle_playback))
            .on_action(cx.listener(Self::stop_playback))
            .on_action(cx.listener(Self::next_track))
            .on_action(cx.listener(Self::prev_track))
            .on_action(cx.listener(Self::volume_up))
            .on_action(cx.listener(Self::volume_down))
            .on_action(cx.listener(Self::switch_to_library))
            .on_action(cx.listener(Self::switch_to_queue))
            .on_action(cx.listener(Self::switch_to_plugins))
            .on_action(cx.listener(Self::switch_to_devices))
            .on_action(cx.listener(Self::switch_to_directory_manager))
            .on_action(cx.listener(Self::switch_to_settings))
            .on_action(cx.listener(Self::open_config))
            .on_action(cx.listener(Self::quit_app))
            .on_action(cx.listener(Self::cycle_theme))
            .on_action(cx.listener(Self::cycle_language))
            .on_action(cx.listener(Self::toggle_search))
            .on_action(cx.listener(Self::toggle_library_view))
            .on_action(cx.listener(Self::toggle_help))
            .on_action(cx.listener(Self::cycle_sort_order))
            .on_action(cx.listener(Self::set_sort_artist))
            .on_action(cx.listener(Self::set_sort_album))
            .on_action(cx.listener(Self::set_sort_title))
            .on_action(cx.listener(Self::set_sort_year))
            .on_action(cx.listener(Self::cycle_channel_filter))
            .on_action(cx.listener(Self::set_filter_all))
            .on_action(cx.listener(Self::set_filter_mono))
            .on_action(cx.listener(Self::set_filter_stereo))
            .on_action(cx.listener(Self::set_filter_multichannel))
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
            .on_action(cx.listener(Self::edit_plugin))
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
                        view.handle_search_input(event, cx);
                    }
                    crate::app::InputMode::AddDirectory => {
                        view.handle_directory_input(event, cx);
                    }
                    crate::app::InputMode::EditPlugin => {
                        view.handle_plugin_edit_input(event, cx);
                    }
                    crate::app::InputMode::LoadApoFile => {
                        view.handle_apo_file_input(event, cx);
                    }
                    crate::app::InputMode::LoadSofaFile => {
                        view.handle_sofa_file_input(event, cx);
                    }
                    crate::app::InputMode::SavePlugins => {
                        view.handle_save_plugins_input(event, cx);
                    }
                    crate::app::InputMode::LoadPlugins => {
                        view.handle_load_plugins_input(event, cx);
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
                            }
                        }
                    }),
            )
            .child(self.render_footer(cx))
            .when(input_mode == crate::app::InputMode::Help, |div| {
                div.child(self.render_help_modal(cx))
            })
            .when(input_mode == crate::app::InputMode::EditPlugin, |div| {
                div.child(self.render_plugin_edit_modal(cx))
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
            .child(self.render_toast(cx))
            .when(self.state.read(cx).app.context_menu.is_some(), |div| {
                div.child(self.render_context_menu(cx))
            })
            // Device popup overlay (click outside to close)
            .when(self.state.read(cx).app.show_device_popup, |div| {
                div.child(self.render_device_popup_overlay(cx))
            })
            // Menu dropdowns rendered last for z-ordering
            .when(active_menu != crate::app::ActiveMenu::None, |div| {
                div.child(self.render_menu_dropdowns(cx))
            })
    }
}
