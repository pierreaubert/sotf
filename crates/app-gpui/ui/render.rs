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
        // Use defer to avoid re-entrant state updates during render
        let window_bounds = window.bounds();
        let window_height: f32 = window_bounds.size.height.into();
        let window_width: f32 = window_bounds.size.width.into();

        // Check if dimensions actually changed to avoid unnecessary updates
        let needs_dimension_update = {
            let state = self.state.read(cx);
            (state.app.ui_state.window_height - window_height).abs() > 0.5
                || (state.app.ui_state.window_width - window_width).abs() > 0.5
        };

        if needs_dimension_update {
            let view_handle = cx.entity().clone();
            cx.defer(move |cx| {
                view_handle.update(cx, |view, cx| {
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _cx| {
                            state.app.ui_state.window_height = window_height;
                            state.app.ui_state.window_width = window_width;
                            // Use Expanded layout when both dimensions are large enough
                            // for multi-panel view. Compact for small screens (phones/tablets).
                            state.app.ui_state.layout_mode =
                                if window_height >= 500.0 && window_width >= 600.0 {
                                    crate::app::LayoutMode::Expanded
                                } else {
                                    crate::app::LayoutMode::Compact
                                };

                            // Update 3-panel layout orientation based on aspect ratio
                            state.app.layout_orientation = if window_width > window_height {
                                crate::app::LayoutOrientation::Horizontal
                            } else {
                                crate::app::LayoutOrientation::Vertical
                            };

                            // Determine rack display mode based on available space
                            let rack_dimension = match state.app.layout_orientation {
                                crate::app::LayoutOrientation::Horizontal => {
                                    window_width * layout.rack_h_ratio
                                }
                                crate::app::LayoutOrientation::Vertical => {
                                    window_height * layout.rack_v_ratio
                                }
                            };
                            state.app.rack_display_mode = if layout.rack_panel_collapsed {
                                crate::app::RackDisplayMode::Collapsed
                            } else if rack_dimension < 100.0 {
                                crate::app::RackDisplayMode::Collapsed
                            } else if rack_dimension < 200.0 {
                                crate::app::RackDisplayMode::Mini
                            } else {
                                crate::app::RackDisplayMode::Full
                            };

                            // Hide queue meters when rack is visible to avoid duplicate meters
                            state.app.hide_queue_meters_for_rack = matches!(
                                state.app.rack_display_mode,
                                crate::app::RackDisplayMode::Full | crate::app::RackDisplayMode::Mini
                            );
                        });
                    });
                    
                    // Recalculate pagination based on new window size
                    view.recalculate_pagination(cx, false);
                });
            });
        }

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

            // Debounce saving to avoid disk IO pressure during active resizing
            self.geometry_save_task = Some(cx.spawn(async move |this, cx| {
                // Wait for a period of stability (1 second) before saving
                cx.background_executor().timer(Duration::from_secs(1)).await;
                
                let _ = this.update(cx, |view, cx| {
                    view.state.update(cx, |state, cx| {
                        let layout = state.layout.read(cx);
                        if let Err(e) = state.app.save_config_with_geometry(&layout, Some(geometry)) {
                            log::warn!("Failed to save window geometry: {}", e);
                        } else {
                            log::debug!("Debounced window geometry saved successfully");
                        }
                    });
                });
            }));

            self.last_saved_window_bounds = Some(window_bounds);
        }

        // Batch all state reads into a single scope to minimize locking overhead
        let (
            current_screen,
            input_mode,
            theme,
            layout_mode,
            active_menu,
            font_scale,
            theme_id,
            show_migration_modal,
            context_menu,
            show_studio_menu,
            show_device_popup,
            playback_output_devices,
        ) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.current_screen,
                state.app.ui_state.input_mode,
                state.app.ui_state.theme.clone(),
                state.app.ui_state.layout_mode,
                state.app.ui_state.active_menu,
                state.app.ui_state.font_scale,
                state.app.ui_state.theme_id,
                state.app.measurement_state.recording_state.migration_modal_open,
                state.app.ui_state.context_menu.is_some(),
                state.app.ui_state.show_studio_menu,
                state.app.ui_state.show_device_popup,
                state.app.ui_state.translations.playback_output_devices,
            )
        };

        // Compute responsive scale based on window dimensions.
        // Reference size: 1200x800 (default window). Scale adapts smoothly from
        // phone-size windows (~400px) through 4K displays (~3840px logical).
        let responsive_scale = {
            let width_scale = window_width / 1200.0;
            let height_scale = window_height / 800.0;
            // Use the smaller axis to avoid overflow, clamp to usable range
            width_scale.min(height_scale).clamp(0.55, 2.5)
        };

        // Apply combined font scale (user preference * responsive auto-scale) to rem size.
        // All rem-based sizes (text, padding, gaps) scale automatically.
        window.set_rem_size(px(16.0 * font_scale * responsive_scale));

        // Keep gpui-ui-kit global theme in sync with app theme so components get consistent defaults.
        // This allows builder overrides but ensures out-of-the-box colors match the app theme.
        let ui_kit_theme: gpui_ui_kit::Theme = theme.to_ui_kit_theme(theme_id);
        cx.set_global(UiKitThemeState {
            theme: ui_kit_theme,
        });

        // Determine key context based on input mode
        // Use "TextInput" context when typing to disable single-letter keybindings
        let key_context = if Self::is_text_input_mode(input_mode) {
            "TextInput"
        } else {
            "PlayerView"
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
            .on_action(cx.listener(Self::volume_up_large))
            .on_action(cx.listener(Self::volume_down_large))
            .on_action(cx.listener(Self::volume_max))
            .on_action(cx.listener(Self::volume_min))
            .on_action(cx.listener(Self::toggle_mute))
            .on_action(cx.listener(Self::switch_to_library))
            .on_action(cx.listener(Self::switch_to_queue))
            .on_action(cx.listener(Self::switch_to_plugins))
            .on_action(cx.listener(Self::switch_to_studio))
            .on_action(cx.listener(Self::switch_to_plugin_graph))
            .on_action(cx.listener(Self::switch_to_devices))
            .on_action(cx.listener(Self::switch_to_settings))
            .on_action(cx.listener(Self::switch_to_recording))
            .on_action(cx.listener(Self::switch_to_room_eq))
            .on_action(cx.listener(Self::switch_to_headphone_eq))
            .on_action(cx.listener(Self::switch_to_spinorama))
            .on_action(cx.listener(Self::open_config))
            .on_action(cx.listener(Self::quit_app))
            .on_action(cx.listener(Self::cycle_theme))
            .on_action(cx.listener(Self::cycle_language))
            // Font size actions
            .on_action(cx.listener(Self::increase_font_size))
            .on_action(cx.listener(Self::decrease_font_size))
            .on_action(cx.listener(Self::reset_font_size))
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
            .on_action(cx.listener(Self::toggle_simple_view))
            .on_action(cx.listener(Self::add_directory))
            .on_action(cx.listener(Self::scan_library))
            .on_action(cx.listener(Self::quick_add_eq))
            .on_action(cx.listener(Self::quick_add_gain))
            .on_action(cx.listener(Self::quick_add_upmixer))
            .on_action(cx.listener(Self::quick_add_compressor))
            .on_action(cx.listener(Self::quick_add_gate))
            .on_action(cx.listener(Self::quick_add_limiter))
            .on_action(cx.listener(Self::quick_add_expander))
            .on_action(cx.listener(Self::quick_add_mbcomp))
            .on_action(cx.listener(Self::quick_add_mbexp))
            .on_action(cx.listener(Self::quick_add_loudness))
            .on_action(cx.listener(Self::quick_add_fletcher))
            .on_action(cx.listener(Self::quick_add_binaural))
            .on_action(cx.listener(Self::quick_add_convolution))
            .on_action(cx.listener(Self::quick_add_loudness_monitor))
            .on_action(cx.listener(Self::quick_add_spectrum))
            .on_action(cx.listener(Self::quick_add_mutesolo))
            .on_action(cx.listener(Self::quick_add_xtc))
            .on_action(cx.listener(Self::quick_add_denoiser))
            .on_action(cx.listener(Self::quick_add_pnd))
            .on_action(cx.listener(Self::quick_add_ab_compare))
            .on_action(cx.listener(Self::quick_add_downmix))
            .on_action(cx.listener(Self::quick_add_mono_to_stereo))
            .on_action(cx.listener(Self::quick_add_band_split))
            .on_action(cx.listener(Self::quick_add_band_merge))
            .on_action(cx.listener(Self::quick_add_crossfeed))
            // Plugin parameter actions
            .on_action(cx.listener(Self::on_update_plugin_param))
            .on_action(cx.listener(Self::on_select_plugin_param))
            .on_action(cx.listener(Self::on_reset_plugin_param))
            .on_action(cx.listener(Self::on_start_knob_drag))
            .on_action(cx.listener(Self::increment_plugin_param))
            .on_action(cx.listener(Self::decrement_plugin_param))
            .on_action(cx.listener(Self::increment_plugin_param_large))
            .on_action(cx.listener(Self::decrement_plugin_param_large))
            .on_action(cx.listener(Self::increment_plugin_param_small))
            .on_action(cx.listener(Self::decrement_plugin_param_small))
            // Level meter actions
            .on_action(cx.listener(Self::select_next_meter_group))
            .on_action(cx.listener(Self::select_prev_meter_group))
            .on_action(cx.listener(Self::toggle_meter_mute))
            .on_action(cx.listener(Self::toggle_meter_solo))
            .on_action(cx.listener(Self::toggle_meter_dim))
            .on_action(cx.listener(Self::clear_meter_mutes_solos))
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, _window, cx| {
                // Handle text input for search mode and add directory mode
                let (input_mode, current_screen) = {
                    let state = view.state.read(cx);
                    (state.app.ui_state.input_mode, state.app.ui_state.current_screen)
                };

                log::debug!(
                    "on_key_down: key='{}', input_mode={:?}",
                    event.keystroke.key,
                    input_mode
                );

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
                                        state.app.input_state.plugin_file_input.clear();
                                        state.app.ui_state.input_mode = crate::app::InputMode::SavePlugins;
                                    });
                                    cx.notify();
                                }
                                "l" => {
                                    // Enter load plugins mode
                                    view.state.update(cx, |state, _cx| {
                                        state.app.refresh_plugin_presets();
                                        state.app.input_state.plugin_file_input.clear();
                                        state.app.ui_state.input_mode = crate::app::InputMode::LoadPlugins;
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
            .font_family(theme.font_family.clone())
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
                    .child(self.render_current_screen(current_screen, layout_mode, cx)),
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
            // Migration modal for recording format conversion
            .when(show_migration_modal, |div| div.child(self.render_migration_modal(cx)))
            .child(self.render_toast(cx))
            .when(context_menu, |div| div.child(self.render_context_menu(cx)))
            // Studio menu overlay (click outside to close)
            .when(show_studio_menu, |div| {
                div.child(self.render_studio_menu_overlay(cx))
            })
            // Device popup overlay (click outside to close)
            .when(show_device_popup, |div| {
                div.child(self.render_device_popup_overlay(cx))
            })
            // Device popup (rendered here to be above overlay)
            .when(show_device_popup, |div| {
                div.child(self.render_device_popup(playback_output_devices, cx))
            })
            // Menu dropdowns rendered last for z-ordering
            .when(active_menu != crate::app::ActiveMenu::None, |div| {
                div.child(self.render_menu_dropdowns(cx))
            })
    }
}

// Screen rendering helper
impl PlayerView {
    /// Render the current screen based on layout mode.
    /// Most screens render identically regardless of layout mode.
    /// Only Library/Queue differ: Expanded uses split view, Compact uses separate screens.
    fn render_current_screen(
        &mut self,
        screen: Screen,
        layout_mode: crate::app::LayoutMode,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match screen {
            // These screens render the same regardless of layout mode
            Screen::Spectrum => self.render_spectrum_screen(cx).into_any_element(),
            Screen::Settings => self.render_settings_screen(cx).into_any_element(),
            Screen::Studio => self.render_plugins_screen(cx).into_any_element(),
            Screen::Recording => self.render_recording_screen(cx).into_any_element(),
            Screen::RoomEq => self.render_room_eq_screen(cx).into_any_element(),
            Screen::HeadphoneEq => self.render_headphone_eq_screen(cx).into_any_element(),
            Screen::Spinorama => self.render_spinorama_eq_screen(cx).into_any_element(),
            Screen::PluginGraph => self.render_plugin_graph_screen(cx).into_any_element(),
            // Library/Queue use 3-panel layout in Expanded mode, individual screens in Compact
            Screen::Library | Screen::Queue => {
                let layout_orientation = self.state.read(cx).app.layout_orientation;
                match layout_mode {
                    crate::app::LayoutMode::Expanded => match layout_orientation {
                        crate::app::LayoutOrientation::Horizontal => {
                            self.render_horizontal_3panel(cx).into_any_element()
                        }
                        crate::app::LayoutOrientation::Vertical => {
                            self.render_vertical_3panel(cx).into_any_element()
                        }
                    }
                    crate::app::LayoutMode::Compact => {
                        if screen == Screen::Library {
                            self.render_library_screen(cx).into_any_element()
                        } else {
                            self.render_queue_screen(cx).into_any_element()
                        }
                    }
                }
            }
        }
    }
}
