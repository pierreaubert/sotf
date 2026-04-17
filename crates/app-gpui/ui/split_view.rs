impl PlayerView {
    /// Render split view with Library on top and Queue on bottom (expanded mode)
    #[allow(dead_code)]
    fn render_split_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, queue_ratio) = {
            let state = self.state.read(cx);
            let layout = state.layout.read(cx);
            (state.app.ui_state.theme.clone(), layout.queue_panel_ratio)
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
                    volume_drag,
                    window_height,
                ) = {
                    let state = view.state.read(cx);
                    let layout = state.layout.read(cx);
                    (
                        layout.is_dragging_queue_divider,
                        layout.is_dragging_queue_list_divider,
                        layout.is_dragging_meters_divider,
                        layout.is_dragging_lufs_divider,
                        state.app.volume_drag,
                        state.app.ui_state.window_height,
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
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            layout.queue_panel_ratio = new_ratio;
                        });
                    });
                }

                if is_dragging_queue_list {
                    let window_width: f32 = window_size.width.into();
                    let mouse_x: f32 = mouse_pos.x.into();
                    let new_ratio = (mouse_x / window_width).clamp(0.1, 0.5);
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            layout.queue_list_ratio = new_ratio;
                        });
                    });
                }

                if is_dragging_meters {
                    let window_width: f32 = window_size.width.into();
                    let mouse_x: f32 = mouse_pos.x.into();
                    // Meters are on the right, so ratio is from the right edge
                    let right_edge_ratio: f32 = (1.0 - (mouse_x / window_width)).clamp(0.1, 0.8);

                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            if is_compact_height {
                                // In 4-col mode, Divider 2 controls total right width (LUFS + Meters)
                                // lufs_ratio = total - meters_ratio
                                let new_lufs = (right_edge_ratio - layout.meters_panel_ratio).max(0.05);
                                layout.lufs_panel_ratio = new_lufs;
                            } else {
                                // Standard mode: controls combined panel width
                                layout.meters_panel_ratio = right_edge_ratio.clamp(0.1, 0.5);
                            }
                        });
                    });
                }

                if is_dragging_lufs {
                    let window_width: f32 = window_size.width.into();
                    let mouse_x: f32 = mouse_pos.x.into();
                    // Divider 3 (LUFS <-> Meters) controls meters_panel_ratio
                    let new_meters = (1.0 - (mouse_x / window_width)).clamp(0.05, 0.5);
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            layout.meters_panel_ratio = new_meters;
                        });
                    });
                }

                // Handle volume dragging (drag up = increase, drag down = decrease)
                if let Some(vd) = volume_drag {
                        let mouse_y: f32 = mouse_pos.y.into();
                        let delta_y = vd.start_y - mouse_y; // Inverted: up = positive
                        // Scale: 100px drag = full volume range
                        let volume_delta = delta_y / 100.0;
                        let new_volume: f32 = (vd.start_value + volume_delta).clamp(0.0, 1.0);
                        view.state.update(cx, |state, _cx| {
                            state.app.playback.volume = new_volume;
                            let _ = state.player.lock().set_volume(new_volume);
                        });
                        cx.notify();
                    }
            }))
            // Global mouse up handler to stop dragging even if mouse is outside divider
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                    let mut needs_save = false;
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            if layout.is_dragging_queue_divider {
                                layout.is_dragging_queue_divider = false;
                                needs_save = true;
                            }
                            if layout.is_dragging_queue_list_divider {
                                // Check for click vs drag
                                let was_click = state
                                    .app
                                    .divider_click_start
                                    .map(|start| start.elapsed().as_millis() < 200)
                                    .unwrap_or(false);

                                if was_click {
                                    if layout.queue_list_ratio > 0.05 {
                                        layout.queue_list_ratio = 0.0;
                                    } else {
                                        layout.queue_list_ratio = 0.30; // Restore default
                                    }
                                }

                                layout.is_dragging_queue_list_divider = false;
                                needs_save = true;
                            }
                            if layout.is_dragging_meters_divider {
                                // Check for click vs drag
                                let was_click = state
                                    .app
                                    .divider_click_start
                                    .map(|start| start.elapsed().as_millis() < 200)
                                    .unwrap_or(false);

                                if was_click {
                                    if layout.meters_panel_ratio > 0.05 {
                                        layout.meters_panel_ratio = 0.0;
                                    } else {
                                        layout.meters_panel_ratio = 0.25; // Restore default
                                    }
                                }

                                layout.is_dragging_meters_divider = false;
                                needs_save = true;
                            }
                            if layout.is_dragging_lufs_divider {
                                layout.is_dragging_lufs_divider = false;
                                needs_save = true;
                            }
                        });

                        if state.app.volume_drag.is_some() {
                            state.app.volume_drag = None;
                        }

                        if needs_save {
                            let layout = state.layout.read(cx);
                            if let Err(e) = state.app.save_config(layout) {
                                log::warn!("Failed to save panel layout: {}", e);
                            }
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
                        let state_handle = self.state.clone();
                        move |collapsed, _window, cx| {
                            state_handle.update(cx, |state, cx| {
                                state.layout.update(cx, |layout, _| {
                                    layout.queue_panel_ratio = if collapsed { 0.95 } else { 0.35 };
                                    let _ = state.app.save_config(layout);
                                });
                            });
                        }
                    })
                    .on_drag_start({
                        let state_handle = self.state.clone();
                        move |_pos, _window, cx| {
                            state_handle.update(cx, |state, cx| {
                                state.layout.update(cx, |layout, _| {
                                    layout.is_dragging_queue_divider = true;
                                });
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
