use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName, IconSize};

#[derive(Debug)]
pub(crate) struct PendingGeometrySave {
    geometry: crate::config::WindowGeometry,
    sequence: u64,
}

impl Render for PlayerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Focus view on first render to activate macOS menu bar
        if self.needs_initial_focus {
            self.needs_initial_focus = false;
            self.focus_handle.focus(window, cx);
            window.activate_window();
            cx.activate(true);

            // When the window close button (red X) is clicked, dispatch QuitApp
            // so that cleanup (save config/geometry, stop player) runs the same
            // path as Cmd-Q.
            window.on_window_should_close(cx, |window, cx| {
                window.dispatch_action(Box::new(QuitApp), cx);
                false // prevent default close — QuitApp calls cx.quit()
            });
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
                            state.app.ui_state.layout_mode = state
                                .app
                                .ui_state
                                .density_mode
                                .layout_mode_for_window(window_width, window_height);

                            // Derive layout orientation, rack display mode, and
                            // queue-meter visibility from the constraint solver.
                            let solved = crate::ui::layout_tree::solve_app_layout(
                                window_width, window_height, layout,
                            );
                            state.app.layout_orientation =
                                if crate::ui::layout_tree::solved_is_horizontal(&solved) {
                                    crate::app::LayoutOrientation::Horizontal
                                } else {
                                    crate::app::LayoutOrientation::Vertical
                                };
                            state.app.rack_display_mode =
                                crate::ui::layout_tree::solved_rack_display_mode(&solved);
                            state.app.hide_queue_meters_for_rack =
                                crate::ui::layout_tree::solved_hide_queue_meters(&solved);
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
            self.last_saved_window_bounds = Some(window_bounds);
            self.geometry_save_sequence = self.geometry_save_sequence.wrapping_add(1);
            let geometry_sequence = self.geometry_save_sequence;
            *self.pending_geometry_save.lock() = Some(PendingGeometrySave {
                geometry,
                sequence: geometry_sequence,
            });

            // Only spawn a new debounce task if none is in flight. Without
            // this guard, every render frame the window moved ≥1 px would
            // schedule a fresh 1 s timer task. The running task waits until
            // the sequence is stable for a full debounce interval, so a long
            // drag produces one save after the final move instead of periodic
            // writes during the drag.
            if !self.geometry_save_pending {
                self.geometry_save_pending = true;
                let pending = self.pending_geometry_save.clone();
                let mut last_seen_sequence = geometry_sequence;
                self.geometry_save_task = Some(cx.spawn(async move |this, cx| {
                    loop {
                        cx.background_executor().timer(Duration::from_secs(1)).await;

                        let current_sequence = pending.lock().as_ref().map(|save| save.sequence);
                        let Some(current_sequence) = current_sequence else {
                            let _ = this.update(cx, |view, _cx| {
                                view.geometry_save_pending = false;
                            });
                            break;
                        };

                        if current_sequence != last_seen_sequence {
                            last_seen_sequence = current_sequence;
                            continue;
                        }

                        let geometry = {
                            let mut pending = pending.lock();
                            match pending.as_ref() {
                                Some(save) if save.sequence == current_sequence => {
                                    pending.take().map(|save| save.geometry)
                                }
                                _ => None,
                            }
                        };

                        let Some(geometry) = geometry else {
                            continue;
                        };

                        let _ = this.update(cx, |view, cx| {
                            view.geometry_save_pending = false;
                            view.state.update(cx, |state, cx| {
                                let layout = state.layout.read(cx);
                                if let Err(e) =
                                    state.app.save_config_with_geometry(layout, Some(geometry))
                                {
                                    log::warn!("Failed to save window geometry: {}", e);
                                } else {
                                    log::debug!(
                                        "Debounced window geometry saved successfully"
                                    );
                                }
                            });
                        });
                        break;
                    }
                }));
            }
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
            show_move_position_modal,
            context_menu,
            min_font_size_px,
            max_font_size_px,
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
                state
                    .app
                    .measurement_state
                    .recording_state
                    .move_position_modal_open,
                state.app.ui_state.context_menu.is_some(),
                state.app.ui_state.min_font_size_px,
                state.app.ui_state.max_font_size_px,
            )
        };

        // Apply combined font scale (user preference × responsive auto-scale) to rem size.
        // All rem-based sizes (text, padding, gaps) scale automatically.
        let responsive_scale = compute_responsive_scale(window_width, window_height);
        let (scale_min, scale_max) = combined_scale_bounds(min_font_size_px, max_font_size_px);
        let combined_scale = (font_scale * responsive_scale).clamp(scale_min, scale_max);
        window.set_rem_size(px(16.0 * combined_scale));

        // Keep gpui-ui-kit global theme in sync with app theme so components get consistent defaults.
        // This allows builder overrides but ensures out-of-the-box colors match the app theme.
        let ui_kit_theme: gpui_ui_kit::Theme = theme.to_ui_kit_theme(theme_id, cx);
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
            .on_action(cx.listener(Self::switch_to_playlists))
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
            // Design system actions
            .on_action(cx.listener(Self::set_design_neutral))
            .on_action(cx.listener(Self::set_design_apple_hig))
            .on_action(cx.listener(Self::set_design_material3))
            .on_action(cx.listener(Self::set_design_fluent))
            // Font size actions
            .on_action(cx.listener(Self::increase_font_size))
            .on_action(cx.listener(Self::decrease_font_size))
            .on_action(cx.listener(Self::reset_font_size))
            .on_action(cx.listener(Self::toggle_search))
            .on_action(cx.listener(Self::toggle_library_view))
            .on_action(cx.listener(Self::toggle_help))
            .on_action(cx.listener(Self::toggle_help_support))
            .on_action(cx.listener(Self::toggle_screen_guide))
            .on_action(cx.listener(Self::about))
            .on_action(cx.listener(Self::cycle_sort_order))
            .on_action(cx.listener(Self::set_sort_artist))
            .on_action(cx.listener(Self::set_sort_album))
            .on_action(cx.listener(Self::set_sort_title))
            .on_action(cx.listener(Self::set_sort_year))
            .on_action(cx.listener(Self::cycle_channel_filter))
            .on_action(cx.listener(Self::toggle_favorites_filter))
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
            .on_action(cx.listener(Self::add_to_queue))
            .on_action(cx.listener(Self::play_now))
            .on_action(cx.listener(Self::move_plugin_up))
            .on_action(cx.listener(Self::move_plugin_down))
            .on_action(cx.listener(Self::toggle_plugin))
            .on_action(cx.listener(Self::toggle_simple_view))
            .on_action(cx.listener(Self::add_directory))
            .on_action(cx.listener(Self::scan_library))
            .on_action(cx.listener(Self::quick_add_eq))
            .on_action(cx.listener(Self::quick_add_gain))
            .on_action(cx.listener(Self::quick_add_upmixer))
            .on_action(cx.listener(Self::quick_add_aae))
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
            // Plugin file picker actions
            .on_action(cx.listener(Self::on_open_sofa_file))
            .on_action(cx.listener(Self::on_open_ir_file))
            .on_action(cx.listener(Self::on_open_ab_config_file))
            .on_action(cx.listener(Self::on_ab_path_add_plugin))
            .on_action(cx.listener(Self::on_ab_path_remove_plugin))
            .on_action(cx.listener(Self::on_ab_path_move_plugin))
            .on_action(cx.listener(Self::on_ab_path_toggle_add_menu))
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
            // Band selection actions (multiband plugins)
            .on_action(cx.listener(Self::select_band_global))
            .on_action(cx.listener(Self::select_band_1))
            .on_action(cx.listener(Self::select_band_2))
            .on_action(cx.listener(Self::select_band_3))
            .on_action(cx.listener(Self::select_band_4))
            .on_action(cx.listener(Self::select_band_5))
            // EQ band navigation
            .on_action(cx.listener(Self::select_next_eq_band))
            .on_action(cx.listener(Self::select_prev_eq_band))
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
                    // Search mode now uses Input component's native text handling
                    // No global keyboard interception needed
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
                        // Let the GPUI Input component handle text natively via on_text_change.
                        // Only intercept non-text keys like escape/enter (handled by actions).
                    }
                    crate::app::InputMode::ContextMenu => {
                        cx.stop_propagation();
                        match event.keystroke.key.as_str() {
                            "a" => {
                                // Add to queue
                                view.state.update(cx, |state, _cx| {
                                    state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                                    state.app.ui_state.context_menu = None;
                                    match state.app.add_album_to_queue() {
                                        Ok(Some(path)) => PlayerView::play_track(state, path),
                                        Err(e) => {
                                            state.app.ui_state.toast_message =
                                                Some(crate::app::ToastMessage::error(e));
                                        }
                                        _ => {}
                                    }
                                });
                            }
                            "enter" => {
                                // Play now
                                view.state.update(cx, |state, _cx| {
                                    state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                                    state.app.ui_state.context_menu = None;
                                    match state.app.play_album_now() {
                                        Ok(Some(path)) => PlayerView::play_track(state, path),
                                        Err(e) => {
                                            state.app.ui_state.toast_message =
                                                Some(crate::app::ToastMessage::error(e));
                                        }
                                        _ => {}
                                    }
                                });
                            }
                            "escape" => {
                                // Close context menu
                                view.state.update(cx, |state, _cx| {
                                    state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                                    state.app.ui_state.context_menu = None;
                                });
                            }
                            _ => {}
                        }
                    }
                    crate::app::InputMode::Tutorial => {
                        cx.stop_propagation();
                        view.handle_tutorial_key(event, cx);
                    }
                    crate::app::InputMode::ScreenGuide => {
                        cx.stop_propagation();
                        if matches!(event.keystroke.key.as_str(), "escape" | "f1") {
                            view.state.update(cx, |state, _| {
                                state.app.ui_state.input_mode =
                                    crate::app::InputMode::Normal;
                            });
                        }
                    }
                    crate::app::InputMode::ChannelConflict => {
                        cx.stop_propagation();
                        match event.keystroke.key.as_str() {
                            "enter" => {
                                // Default action: suspend incompatible plugins and play
                                view.state.update(cx, |state, _| {
                                    let conflicts =
                                        std::mem::take(&mut state.app.channel_conflicts);
                                    let indices: Vec<usize> =
                                        conflicts.iter().map(|c| c.index).collect();
                                    state.app.plugin_state.graph.suspend_plugins(&indices);
                                    state
                                        .app
                                        .plugin_state
                                        .graph
                                        .update_channel_dependent_plugins();
                                    state.app.ui_state.input_mode =
                                        crate::app::InputMode::Normal;
                                });
                                let (path, track_channels) = view
                                    .state
                                    .update(cx, |state, _| {
                                        let p = state.app.channel_conflict_path.take();
                                        let ch = state.app.channel_conflict_track_channels;
                                        (p, ch)
                                    });
                                if let Some(path) = path {
                                    view.state.update(cx, |state, _| {
                                        // Call play_track_at_inner directly — conflict
                                        // resolution already happened above, so we must
                                        // skip play_track/play_track_at which would
                                        // clear_suspensions and re-detect conflicts.
                                        PlayerView::play_track_at_inner(
                                            state,
                                            path,
                                            None,
                                            track_channels,
                                            false,
                                        );
                                    });
                                }
                                cx.notify();
                            }
                            "escape" => {
                                view.state.update(cx, |state, _| {
                                    state.app.channel_conflict_path = None;
                                    state.app.channel_conflicts.clear();
                                    state.app.ui_state.input_mode =
                                        crate::app::InputMode::Normal;
                                    state.app.playback.is_playing = false;
                                });
                                cx.notify();
                            }
                            _ => {}
                        }
                    }
                    crate::app::InputMode::Normal
                        if current_screen == crate::app::Screen::Settings
                            && view
                                .state
                                .read(cx)
                                .app
                                .expanded_settings_sections
                                .contains(&"plugins".to_string()) =>
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
                    _ => {}
                }
            }))
            .flex()
            .flex_col()
            .size_full()
            .font_family(theme.resolved_font_family(cx))
            .bg(theme.background)
            .text_color(theme.text_primary)
            // Apply safe area insets on iOS/tvOS to avoid the notch / overscan
            .map(|div| {
                #[cfg(any(target_os = "ios", target_os = "tvos"))]
                {
                    let (top, _left, bottom, _right) = gpui_ios::safe_area_insets();
                    div.pt(px(top)).pb(px(bottom))
                }
                #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
                { div }
            })
            .when(!cfg!(target_os = "macos") && !cfg!(target_os = "ios") && !cfg!(target_os = "tvos"), |div| {
                div.child(self.render_menu_bar(cx))
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.render_app_sidebar(cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex()
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_hidden()
                                    .child(self.render_current_screen(current_screen, layout_mode, cx)),
                            )
                            .when(
                                self.state.read(cx).app.federation.scan_progress.is_some(),
                                |div| div.child(self.render_federation_scan_progress(cx)),
                            )
                            .child(self.render_footer(cx)),
                    ),
            )
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
                input_mode == crate::app::InputMode::Tutorial,
                |div| div.child(self.render_tutorial_dialog(cx)),
            )
            .when(
                input_mode == crate::app::InputMode::ScreenGuide,
                |div| div.child(self.render_screen_guide_dialog(cx)),
            )
            .when(
                input_mode == crate::app::InputMode::EditingPluginNode,
                |div| div.child(self.render_plugin_node_modal(cx)),
            )
            .when(
                input_mode == crate::app::InputMode::ChannelConflict,
                |div| div.child(self.render_channel_conflict_dialog(cx)),
            )
            // Scan progress modal
            .child(self.render_scan_progress_modal(cx))
            // Migration modal for recording format conversion
            .when(show_migration_modal, |div| div.child(self.render_migration_modal(cx)))
            // Move-microphones-to-next-position modal (multi-position recording)
            .when(show_move_position_modal, |div| {
                div.child(self.render_move_position_modal(cx))
            })
            .child(self.render_toast(cx))
            .when(context_menu, |div| div.child(self.render_context_menu(cx)))
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
            Screen::Home => self.render_home_screen(cx).into_any_element(),
            Screen::Streams => self.render_streams_screen(cx).into_any_element(),
            Screen::Spectrum => self.render_spectrum_screen(cx).into_any_element(),
            Screen::Settings => self.render_settings_screen(cx).into_any_element(),
            Screen::Studio => self.render_plugins_screen(cx).into_any_element(),
            Screen::Recording => self.render_recording_screen(cx).into_any_element(),
            Screen::RoomEq => self.render_room_eq_screen(cx).into_any_element(),
            Screen::HeadphoneEq => self.render_headphone_eq_screen(cx).into_any_element(),
            Screen::Spinorama => self.render_spinorama_eq_screen(cx).into_any_element(),
            Screen::PluginGraph => self.render_plugin_graph_screen(cx).into_any_element(),
            Screen::Playlists => div().into_any_element(),
            // Library/Queue use 3-panel layout in Expanded mode, individual screens in Compact
            Screen::NowPlaying | Screen::Library | Screen::Queue => {
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
                        match screen {
                            Screen::Library => self.render_library_screen(cx).into_any_element(),
                            Screen::NowPlaying | Screen::Queue => {
                                self.render_queue_screen(cx).into_any_element()
                            }
                            _ => unreachable!("handled by outer match"),
                        }
                    }
                }
            }
        }
    }

    fn render_app_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (
            theme,
            current_screen,
            input_mode,
            collapsed,
            release_channel,
            translations,
            output_devices,
            selected_output_device_index,
            cast_devices,
            selected_cast_device,
            cast_discovery_running,
        ) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.current_screen,
                state.app.ui_state.input_mode,
                state.app.ui_state.primary_nav_collapsed,
                state.app.ui_state.release_channel,
                state.app.ui_state.translations.clone(),
                state.app.audio_device_state.output_devices.clone(),
                state.app.audio_device_state.selected_output_device_index,
                state.app.audio_device_state.cast_devices.clone(),
                state.app.audio_device_state.selected_cast_device,
                state.app.audio_device_state.cast_discovery_running,
            )
        };

        let rail_width = if collapsed { rems(3.75) } else { rems(12.0) };
        let toggle_icon = if collapsed {
            IconName::ChevronRight
        } else {
            IconName::ChevronLeft
        };
        let state_for_toggle = self.state.clone();

        div()
            .id("app-sidebar")
            .flex()
            .flex_col()
            .flex_none()
            .w(rail_width)
            .h_full()
            .min_h_0()
            .bg(theme.surface)
            .border_r_1()
            .border_color(theme.border)
            .p(d.pad_y_half)
            .gap(d.grid)
            .child(
                div()
                    .id("app-sidebar-toggle")
                    .h(rems(2.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(d.r_md)
                    .cursor_pointer()
                    .hover({
                        let theme = theme.clone();
                        move |s| s.bg(theme.surface_hover)
                    })
                    .child(
                        Icon::new(toggle_icon)
                            .size(IconSize::Sm)
                            .color(theme.text_muted),
                    )
                    .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                        state_for_toggle.update(cx, |state, _cx| {
                            state.app.ui_state.primary_nav_collapsed =
                                !state.app.ui_state.primary_nav_collapsed;
                        });
                    }),
            )
            .child(self.render_sidebar_screen_item(
                "nav-home",
                "Home",
                IconName::Home,
                Screen::Home,
                current_screen == Screen::Home,
                collapsed,
                &theme,
                &d,
            ))
            .child(self.render_sidebar_search_item(
                input_mode == crate::app::InputMode::Search,
                collapsed,
                &theme,
                &d,
            ))
            .child(self.render_sidebar_screen_item(
                "nav-playing",
                "Playing",
                IconName::Music,
                Screen::NowPlaying,
                matches!(current_screen, Screen::NowPlaying | Screen::Queue),
                collapsed,
                &theme,
                &d,
            ))
            .child(self.render_sidebar_screen_item(
                "nav-streams",
                "Streams",
                IconName::ListMusic,
                Screen::Streams,
                current_screen == Screen::Streams,
                collapsed,
                &theme,
                &d,
            ))
            .child(self.render_sidebar_screen_item(
                "nav-studio",
                translations.screen_studio,
                IconName::SlidersHorizontal,
                Screen::Studio,
                current_screen == Screen::Studio,
                collapsed,
                &theme,
                &d,
            ))
            .child(self.render_sidebar_separator(&theme))
            .child(self.render_sidebar_group_label("Tools", collapsed, &theme, &d))
            .when(release_channel.allows(Screen::Recording.maturity()), |el| {
                el.child(self.render_sidebar_screen_item(
                    "nav-recording",
                    translations.screen_recording,
                    IconName::Disc,
                    Screen::Recording,
                    current_screen == Screen::Recording,
                    collapsed,
                    &theme,
                    &d,
                ))
            })
            .when(release_channel.allows(Screen::RoomEq.maturity()), |el| {
                el.child(self.render_sidebar_screen_item(
                    "nav-room-eq",
                    translations.screen_room_eq,
                    IconName::AudioWaveform,
                    Screen::RoomEq,
                    current_screen == Screen::RoomEq,
                    collapsed,
                    &theme,
                    &d,
                ))
            })
            .when(release_channel.allows(Screen::HeadphoneEq.maturity()), |el| {
                el.child(self.render_sidebar_screen_item(
                    "nav-headphone-eq",
                    translations.screen_headphone_eq,
                    IconName::Speaker,
                    Screen::HeadphoneEq,
                    current_screen == Screen::HeadphoneEq,
                    collapsed,
                    &theme,
                    &d,
                ))
            })
            .when(release_channel.allows(Screen::Spinorama.maturity()), |el| {
                el.child(self.render_sidebar_screen_item(
                    "nav-spinorama",
                    translations.screen_spinorama,
                    IconName::AudioWaveform,
                    Screen::Spinorama,
                    current_screen == Screen::Spinorama,
                    collapsed,
                    &theme,
                    &d,
                ))
            })
            .child(div().flex_1())
            .child(self.render_sidebar_separator(&theme))
            .child(self.render_sidebar_group_label("Devices", collapsed, &theme, &d))
            .child(self.render_sidebar_devices_item(collapsed, &theme, &d))
            .when(!collapsed, |el| {
                el.child(self.render_sidebar_device_actions(
                    cast_discovery_running,
                    &theme,
                    &d,
                ))
                .children(output_devices.iter().take(4).enumerate().map(|(idx, device)| {
                    self.render_sidebar_output_device_item(
                        idx,
                        &device.name,
                        idx == selected_output_device_index && selected_cast_device.is_none(),
                        &theme,
                        &d,
                    )
                }))
                .child(self.render_sidebar_cast_group_label(
                    cast_discovery_running,
                    &theme,
                    &d,
                ))
                .when(cast_devices.is_empty() && !cast_discovery_running, |el| {
                    el.child(
                        div()
                            .px(d.pad_y)
                            .py(d.grid)
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child("No Cast devices"),
                    )
                })
                .children(cast_devices.iter().take(4).enumerate().map(|(idx, device)| {
                    self.render_sidebar_cast_device_item(
                        idx,
                        &device.name,
                        &device.device_type,
                        selected_cast_device == Some(idx),
                        &theme,
                        &d,
                    )
                }))
            })
            .into_any_element()
    }

    fn render_sidebar_separator(&self, theme: &crate::theme::Theme) -> AnyElement {
        div()
            .h(px(1.0))
            .mx(rems(0.25))
            .my(rems(0.25))
            .bg(theme.border)
            .into_any_element()
    }

    fn render_sidebar_group_label(
        &self,
        label: &'static str,
        collapsed: bool,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        div()
            .h(if collapsed { rems(0.25) } else { rems(1.5) })
            .px(d.pad_y)
            .flex()
            .items_center()
            .when(!collapsed, |el| {
                el.child(
                    div()
                        .text_size(d.text_xs)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_muted)
                        .child(label),
                )
            })
            .into_any_element()
    }

    fn render_sidebar_screen_item(
        &self,
        id: &'static str,
        label: &str,
        icon: IconName,
        screen: Screen,
        selected: bool,
        collapsed: bool,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let label = label.to_string();
        let state_entity = self.state.clone();

        self.render_sidebar_item_base(id, &label, icon, selected, collapsed, theme, d)
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                state_entity.update(cx, |state, _cx| {
                    state.app.set_screen(screen, "SidebarNav");
                    state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                });
            })
            .into_any_element()
    }

    fn render_sidebar_search_item(
        &self,
        selected: bool,
        collapsed: bool,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let state_entity = self.state.clone();
        let search_focus = self.search_focus_handle.clone();

        self.render_sidebar_item_base(
            "nav-search",
            "Search",
            IconName::Search,
            selected,
            collapsed,
            theme,
            d,
        )
        .on_mouse_up(MouseButton::Left, move |_event, window, cx| {
            state_entity.update(cx, |state, _cx| {
                state.app.set_screen(Screen::Library, "SidebarSearch");
                state.app.ui_state.input_mode = crate::app::InputMode::Search;
                state.app.library_state.search_query.clear();
            });
            search_focus.focus(window, cx);
            #[cfg(any(target_os = "ios", target_os = "tvos"))]
            gpui_ios::show_keyboard();
        })
        .into_any_element()
    }

    fn render_sidebar_devices_item(
        &self,
        collapsed: bool,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let state_entity = self.state.clone();

        self.render_sidebar_item_base(
            "nav-devices",
            "Devices",
            IconName::Speaker,
            false,
            collapsed,
            theme,
            d,
        )
        .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
            state_entity.update(cx, |state, _cx| {
                state.app.set_screen(Screen::Settings, "SidebarDevices");
                state.app.ui_state.active_settings_tab = crate::app::SettingsTab::AudioDevice;
                state.app.ui_state.input_mode = crate::app::InputMode::Normal;
            });
        })
        .into_any_element()
    }

    fn render_sidebar_device_actions(
        &self,
        cast_discovery_running: bool,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let state_for_refresh = self.state.clone();
        let state_for_cast = self.state.clone();

        div()
            .id("nav-device-actions")
            .flex()
            .items_center()
            .gap(d.grid)
            .px(d.pad_y)
            .py(d.grid)
            .child(
                div()
                    .id("nav-refresh-devices")
                    .flex_1()
                    .h(rems(1.75))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(d.r_sm)
                    .border_1()
                    .border_color(theme.border)
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text_secondary)
                    .cursor_pointer()
                    .hover({
                        let theme = theme.clone();
                        move |style| style.bg(theme.surface_hover).text_color(theme.text_primary)
                    })
                    .child("Refresh")
                    .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                        state_for_refresh.update(cx, |state, _cx| {
                            state.app.load_audio_devices();
                        });
                    }),
            )
            .child(
                div()
                    .id("nav-scan-cast")
                    .flex_1()
                    .h(rems(1.75))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(d.r_sm)
                    .border_1()
                    .border_color(theme.border)
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if cast_discovery_running {
                        theme.accent
                    } else {
                        theme.text_secondary
                    })
                    .cursor_pointer()
                    .hover({
                        let theme = theme.clone();
                        move |style| style.bg(theme.surface_hover).text_color(theme.text_primary)
                    })
                    .child(if cast_discovery_running {
                        "Scanning"
                    } else {
                        "Scan Cast"
                    })
                    .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                        state_for_cast.update(cx, |state, _cx| {
                            state.app.start_cast_discovery();
                        });
                    }),
            )
            .into_any_element()
    }

    fn render_sidebar_cast_group_label(
        &self,
        cast_discovery_running: bool,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        div()
            .h(rems(1.5))
            .px(d.pad_y)
            .flex()
            .items_center()
            .child(
                div()
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if cast_discovery_running {
                        theme.accent
                    } else {
                        theme.text_muted
                    })
                    .child(if cast_discovery_running {
                        "Cast Devices (scanning...)"
                    } else {
                        "Cast Devices"
                    }),
            )
            .into_any_element()
    }

    fn render_sidebar_output_device_item(
        &self,
        index: usize,
        name: &str,
        selected: bool,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let device_name = name.to_string();
        let display_name = if device_name.chars().count() > 18 {
            format!("{}...", device_name.chars().take(15).collect::<String>())
        } else {
            device_name.clone()
        };
        let state_entity = self.state.clone();

        div()
            .id(SharedString::from(format!("nav-device-{index}")))
            .flex()
            .items_center()
            .gap(d.grid)
            .h(rems(1.75))
            .px(d.pad_y)
            .rounded(d.r_sm)
            .cursor_pointer()
            .text_size(d.text_xs)
            .when(selected, |el| {
                el.bg(theme.surface_selected)
                    .text_color(theme.text_primary)
                    .font_weight(FontWeight::MEDIUM)
            })
            .when(!selected, |el| {
                el.text_color(theme.text_secondary).hover({
                    let theme = theme.clone();
                    move |s| s.bg(theme.surface_hover).text_color(theme.text_primary)
                })
            })
            .child(
                div()
                    .w(rems(0.75))
                    .text_color(if selected {
                        theme.accent
                    } else {
                        theme.text_muted
                    })
                    .child(if selected { "✓" } else { "›" }),
            )
            .child(
                div()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(display_name),
            )
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                state_entity.update(cx, |state, _cx| {
                    let was_playing = state.app.playback.is_playing;
                    let current_path = state.app.queue_state.current_track_source();
                    let current_pos = state.app.playback.position_secs;

                    state.app.audio_device_state.selected_output_device_index = index;
                    state.app.audio_device_state.current_output_device_name =
                        Some(device_name.clone());
                    state.app.deselect_cast_device();

                    let mut player = state.player.lock();
                    if let Err(e) = player.set_output_device(device_name.clone()) {
                        log::error!("Failed to set output device: {}", e);
                    } else if was_playing && let Some(path) = current_path {
                        drop(player);
                        Self::play_track_at(state, path, Some(current_pos));
                    }
                });
            })
            .into_any_element()
    }

    fn render_sidebar_cast_device_item(
        &self,
        index: usize,
        name: &str,
        device_type: &str,
        selected: bool,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let device_name = name.to_string();
        let display_name = if device_name.chars().count() > 15 {
            format!("{}...", device_name.chars().take(12).collect::<String>())
        } else {
            device_name.clone()
        };
        let display_type = device_type.to_string();
        let state_entity = self.state.clone();

        div()
            .id(SharedString::from(format!("nav-cast-device-{index}")))
            .flex()
            .items_center()
            .gap(d.grid)
            .h(rems(1.75))
            .px(d.pad_y)
            .rounded(d.r_sm)
            .cursor_pointer()
            .text_size(d.text_xs)
            .when(selected, |el| {
                el.bg(theme.surface_selected)
                    .text_color(theme.text_primary)
                    .font_weight(FontWeight::MEDIUM)
            })
            .when(!selected, |el| {
                el.text_color(theme.text_secondary).hover({
                    let theme = theme.clone();
                    move |s| s.bg(theme.surface_hover).text_color(theme.text_primary)
                })
            })
            .child(
                div()
                    .w(rems(0.75))
                    .text_color(if selected {
                        theme.accent
                    } else {
                        theme.text_muted
                    })
                    .child(if selected { "✓" } else { "›" }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(display_name),
            )
            .child(
                div()
                    .text_size(d.text_xs)
                    .text_color(theme.text_muted)
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(display_type),
            )
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                state_entity.update(cx, |state, _cx| {
                    if state.app.audio_device_state.selected_cast_device == Some(index) {
                        state.app.deselect_cast_device();
                    } else {
                        state.app.select_cast_device(index);
                    }
                });
            })
            .into_any_element()
    }

    fn render_sidebar_item_base(
        &self,
        id: &'static str,
        label: &str,
        icon: IconName,
        selected: bool,
        collapsed: bool,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> Stateful<Div> {
        let icon_color = if selected {
            theme.icon_on_accent
        } else {
            theme.text_secondary
        };
        let label_color = if selected {
            theme.text_on_accent
        } else {
            theme.text_secondary
        };

        div()
            .id(id)
            .flex()
            .items_center()
            .justify_center()
            .gap(d.gap)
            .h(rems(2.25))
            .px(if collapsed { d.grid } else { d.pad_y })
            .rounded(d.r_md)
            .cursor_pointer()
            .when(selected, |el| {
                el.bg(theme.accent)
                    .text_color(theme.text_on_accent)
                    .font_weight(FontWeight::MEDIUM)
            })
            .when(!selected, |el| {
                el.text_color(theme.text_secondary).hover({
                    let theme = theme.clone();
                    move |s| s.bg(theme.surface_hover).text_color(theme.text_primary)
                })
            })
            .child(Icon::new(icon).size(IconSize::Sm).color(icon_color))
            .when(!collapsed, |el| {
                el.justify_start().child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(d.text_sm)
                        .text_color(label_color)
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(label.to_string()),
                )
            })
    }
}
