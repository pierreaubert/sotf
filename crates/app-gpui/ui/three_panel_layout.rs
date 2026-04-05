// Three-panel layout implementation for responsive Library | Queue | Rack display
// Horizontal mode (width > height): panels side-by-side
// Vertical mode (height >= width): panels stacked

impl PlayerView {
    /// Render horizontal 3-panel layout: Library | Queue | Rack
    pub fn render_horizontal_3panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, solved) = {
            let state = self.state.read(cx);
            let layout = state.layout.read(cx);
            let w = state.app.ui_state.window_width;
            let h = state.app.ui_state.window_height;
            (
                state.app.ui_state.theme.clone(),
                crate::ui::layout_tree::solve_app_layout(w, h, layout),
            )
        };
        let library_node = solved.find("library").unwrap();
        let rack_node = solved.find("rack").unwrap();
        let library_visible = library_node.visible;
        let rack_visible = rack_node.visible;
        let library_width = library_node.width;
        let rack_width = rack_node.width;
        let rack_mode = crate::ui::layout_tree::solved_rack_display_mode(&solved);

        let divider_theme = PaneDividerTheme {
            background: theme.background,
            background_hover: theme.surface_hover,
            background_collapsed: theme.surface,
            foreground: theme.text_muted,
            foreground_hover: theme.text_secondary,
            border: theme.border,
        };

        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(theme.background)
            // Global mouse move handler for all divider dragging
            .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, window, cx| {
                let (
                    is_dragging_lib_queue,
                    is_dragging_queue_rack,
                    is_dragging_queue_list,
                    is_dragging_meters,
                    library_h_ratio,
                    rack_h_ratio,
                    rack_collapsed,
                ) = {
                    let state = view.state.read(cx);
                    let layout = state.layout.read(cx);
                    (
                        layout.is_dragging_library_queue_divider,
                        layout.is_dragging_queue_rack_divider,
                        layout.is_dragging_queue_list_divider,
                        layout.is_dragging_meters_divider,
                        layout.library_h_ratio,
                        layout.rack_h_ratio,
                        layout.rack_panel_collapsed,
                    )
                };

                let window_size = window.bounds().size;
                let mouse_pos = event.position;
                let window_width: f32 = window_size.width.into();
                let mouse_x: f32 = mouse_pos.x.into();

                if is_dragging_lib_queue {
                    let new_ratio = (mouse_x / window_width).clamp(0.15, 0.50);
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            layout.library_h_ratio = new_ratio;
                        });
                    });
                }

                if is_dragging_queue_rack {
                    let new_ratio = (1.0 - (mouse_x / window_width)).clamp(0.15, 0.50);
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            layout.rack_h_ratio = new_ratio;
                        });
                    });
                }

                // Inner queue dividers: compute position relative to queue panel
                if is_dragging_queue_list || is_dragging_meters {
                    // Queue panel spans from library_h_ratio to (1.0 - rack_h_ratio) in window coords
                    let queue_start = library_h_ratio * window_width;
                    let rack_width = if rack_collapsed { 0.0 } else { rack_h_ratio * window_width };
                    let queue_width = window_width - queue_start - rack_width;

                    if queue_width > 0.0 {
                        let local_x = mouse_x - queue_start;

                        if is_dragging_queue_list {
                            let new_ratio = (local_x / queue_width).clamp(0.1, 0.5);
                            view.state.update(cx, |state, cx| {
                                state.layout.update(cx, |layout, _| {
                                    layout.queue_list_ratio = new_ratio;
                                });
                            });
                        }

                        if is_dragging_meters {
                            let new_ratio = (1.0 - (local_x / queue_width)).clamp(0.1, 0.5);
                            view.state.update(cx, |state, cx| {
                                state.layout.update(cx, |layout, _| {
                                    layout.meters_panel_ratio = new_ratio;
                                });
                            });
                        }
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _event: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            let any_dragging = layout.is_dragging_library_queue_divider
                                || layout.is_dragging_queue_rack_divider
                                || layout.is_dragging_queue_list_divider
                                || layout.is_dragging_meters_divider;

                            layout.is_dragging_library_queue_divider = false;
                            layout.is_dragging_queue_rack_divider = false;
                            layout.is_dragging_queue_list_divider = false;
                            layout.is_dragging_meters_divider = false;

                            if any_dragging
                                && let Err(e) = state.app.save_config(layout)
                            {
                                log::warn!("Failed to save panel layout: {}", e);
                            }
                        });
                    });
                }),
            )
            // Library panel (left)
            .when(library_visible, |d| {
                d.child(
                    div()
                        .w(px(library_width))
                        .h_full()
                        .overflow_hidden()
                        .child(self.render_library_screen(cx)),
                )
            })
            // Library-Queue divider
            .child({
                PaneDivider::vertical("lib-queue-h-divider", CollapseDirection::Left)
                    .label("Library")
                    .collapsed(!library_visible)
                    .theme(divider_theme.clone())
                    .on_toggle({
                        let state_handle = self.state.clone();
                        move |collapsed, _window, cx| {
                            state_handle.update(cx, |state, cx| {
                                state.layout.update(cx, |layout, _| {
                                    layout.library_panel_collapsed = collapsed;
                                    // When opening library, clamp ratio so meters panel still fits
                                    if !collapsed {
                                        let window_w = state.app.ui_state.window_width;
                                        // Meters panel needs ~400px + queue list + center space
                                        let min_queue_width = 700.0_f32;
                                        if window_w > 0.0 {
                                            let max_lib_ratio = 1.0 - (min_queue_width / window_w);
                                            layout.library_h_ratio =
                                                layout.library_h_ratio.min(max_lib_ratio.max(0.15));
                                        }
                                    }
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
                                    layout.is_dragging_library_queue_divider = true;
                                });
                            });
                        }
                    })
            })
            // Queue panel (center) - takes remaining space
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .overflow_hidden()
                    .child(self.render_queue_content(cx)),
            )
            // Queue-Rack divider + Rack panel (only when rack is open)
            .when(rack_visible, |d| {
                d.child({
                    PaneDivider::vertical("queue-rack-h-divider", CollapseDirection::Right)
                        .label("Rack")
                        .collapsed(!rack_visible)
                        .theme(divider_theme)
                        .on_toggle({
                            let state_handle = self.state.clone();
                            move |collapsed, _window, cx| {
                                state_handle.update(cx, |state, cx| {
                                    state.layout.update(cx, |layout, _| {
                                        layout.rack_panel_collapsed = collapsed;
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
                                        layout.is_dragging_queue_rack_divider = true;
                                    });
                                });
                            }
                        })
                })
                .child(
                    div()
                        .w(px(rack_width))
                        .h_full()
                        .overflow_hidden()
                        .child(self.render_rack_for_mode(rack_mode, cx)),
                )
            })
    }

    /// Render vertical 3-panel layout: Library / Queue / Rack stacked
    pub fn render_vertical_3panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, solved) = {
            let state = self.state.read(cx);
            let layout = state.layout.read(cx);
            let w = state.app.ui_state.window_width;
            let h = state.app.ui_state.window_height;
            (
                state.app.ui_state.theme.clone(),
                crate::ui::layout_tree::solve_app_layout(w, h, layout),
            )
        };
        let library_node = solved.find("library").unwrap();
        let rack_node = solved.find("rack").unwrap();
        let library_visible = library_node.visible;
        let rack_visible = rack_node.visible;
        let library_height = library_node.height;
        let rack_height = rack_node.height;
        let rack_mode = crate::ui::layout_tree::solved_rack_display_mode(&solved);

        let divider_theme = PaneDividerTheme {
            background: theme.background,
            background_hover: theme.surface_hover,
            background_collapsed: theme.surface,
            foreground: theme.text_muted,
            foreground_hover: theme.text_secondary,
            border: theme.border,
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            // Global mouse move handler for all divider dragging (vertical layout)
            .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, window, cx| {
                let (
                    is_dragging_lib_queue,
                    is_dragging_queue_rack,
                    is_dragging_queue_list,
                    is_dragging_meters,
                ) = {
                    let layout = view.state.read(cx).layout.read(cx);
                    (
                        layout.is_dragging_library_queue_divider,
                        layout.is_dragging_queue_rack_divider,
                        layout.is_dragging_queue_list_divider,
                        layout.is_dragging_meters_divider,
                    )
                };

                let window_size = window.bounds().size;
                let mouse_pos = event.position;

                if is_dragging_lib_queue {
                    let window_height: f32 = window_size.height.into();
                    let mouse_y: f32 = mouse_pos.y.into();
                    let new_ratio = (mouse_y / window_height).clamp(0.15, 0.50);
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            layout.library_v_ratio = new_ratio;
                        });
                    });
                }

                if is_dragging_queue_rack {
                    let window_height: f32 = window_size.height.into();
                    let mouse_y: f32 = mouse_pos.y.into();
                    let new_ratio = (1.0 - (mouse_y / window_height)).clamp(0.15, 0.50);
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            layout.rack_v_ratio = new_ratio;
                        });
                    });
                }

                // Inner queue dividers: queue panel spans full width in vertical mode
                if is_dragging_queue_list || is_dragging_meters {
                    let window_width: f32 = window_size.width.into();
                    let mouse_x: f32 = mouse_pos.x.into();

                    if is_dragging_queue_list {
                        let new_ratio = (mouse_x / window_width).clamp(0.1, 0.5);
                        view.state.update(cx, |state, cx| {
                            state.layout.update(cx, |layout, _| {
                                layout.queue_list_ratio = new_ratio;
                            });
                        });
                    }

                    if is_dragging_meters {
                        let new_ratio = (1.0 - (mouse_x / window_width)).clamp(0.1, 0.5);
                        view.state.update(cx, |state, cx| {
                            state.layout.update(cx, |layout, _| {
                                layout.meters_panel_ratio = new_ratio;
                            });
                        });
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _event: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            let any_dragging = layout.is_dragging_library_queue_divider
                                || layout.is_dragging_queue_rack_divider
                                || layout.is_dragging_queue_list_divider
                                || layout.is_dragging_meters_divider;

                            layout.is_dragging_library_queue_divider = false;
                            layout.is_dragging_queue_rack_divider = false;
                            layout.is_dragging_queue_list_divider = false;
                            layout.is_dragging_meters_divider = false;

                            if any_dragging
                                && let Err(e) = state.app.save_config(layout)
                            {
                                log::warn!("Failed to save panel layout: {}", e);
                            }
                        });
                    });
                }),
            )
            // Library panel (top)
            .when(library_visible, |d| {
                d.child(
                    div()
                        .h(px(library_height))
                        .w_full()
                        .overflow_hidden()
                        .child(self.render_library_screen(cx)),
                )
            })
            // Library-Queue divider (horizontal)
            .child({
                PaneDivider::horizontal("lib-queue-v-divider", CollapseDirection::Up)
                    .label("Library")
                    .collapsed(!library_visible)
                    .theme(divider_theme.clone())
                    .on_toggle({
                        let state_handle = self.state.clone();
                        move |collapsed, _window, cx| {
                            state_handle.update(cx, |state, cx| {
                                state.layout.update(cx, |layout, _| {
                                    layout.library_panel_collapsed = collapsed;
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
                                    layout.is_dragging_library_queue_divider = true;
                                });
                            });
                        }
                    })
            })
            // Queue panel (center) - takes remaining space
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .child(self.render_queue_content(cx)),
            )
            // Queue-Rack divider + Rack panel (only when rack is open)
            .when(rack_visible, |d| {
                d.child({
                    PaneDivider::horizontal("queue-rack-v-divider", CollapseDirection::Down)
                        .label("Rack")
                        .collapsed(!rack_visible)
                        .theme(divider_theme)
                        .on_toggle({
                            let state_handle = self.state.clone();
                            move |collapsed, _window, cx| {
                                state_handle.update(cx, |state, cx| {
                                    state.layout.update(cx, |layout, _| {
                                        layout.rack_panel_collapsed = collapsed;
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
                                        layout.is_dragging_queue_rack_divider = true;
                                    });
                                });
                            }
                        })
                })
                .child(
                    div()
                        .h(px(rack_height))
                        .w_full()
                        .overflow_hidden()
                        .child(self.render_rack_for_mode(rack_mode, cx)),
                )
            })
    }

    /// Render rack panel based on display mode
    fn render_rack_for_mode(
        &self,
        mode: crate::app::RackDisplayMode,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match mode {
            crate::app::RackDisplayMode::Full => self.render_plugins_screen(cx).into_any_element(),
            crate::app::RackDisplayMode::Mini => self.render_mini_rack(cx).into_any_element(),
            crate::app::RackDisplayMode::Collapsed => div().into_any_element(),
        }
    }

    /// Render mini rack with output level meters only
    fn render_mini_rack(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = crate::components::design::Ds::from_cx(cx);
        let (theme, output_channels) = {
            let state = self.state.read(cx);
            let channels = state.app.plugin_state.graph.output_channels();
            (state.app.ui_state.theme.clone(), channels)
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background_secondary)
            // Header
            .child(
                div()
                    .px(d.pad_y)
                    .py(d.pad_y_half)
                    .border_b_1()
                    .border_color(theme.border)
                    .text_size(d.text_xs)
                    .text_color(theme.text_muted)
                    .child("OUTPUT"),
            )
            // Output meters
            .child(
                div()
                    .flex_1()
                    .p(d.pad_y)
                    .child(self.render_side_meter(cx, output_channels, "", true, false)),
            )
    }

    /// Render queue content for 3-panel layout
    /// Meters visibility is controlled by hide_queue_meters_for_rack state field
    pub fn render_queue_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_queue_screen(cx)
    }
}
