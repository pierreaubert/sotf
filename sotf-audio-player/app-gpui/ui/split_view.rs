impl PlayerView {
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


}
