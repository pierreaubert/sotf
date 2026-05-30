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
            tint: Rgba {
                a: 0.42,
                ..theme.accent
            },
            tint_hover: theme.accent,
        };

        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(theme.background)
            // Global mouse move handler for all divider dragging.
            //
            // Delta-based: each divider's `on_drag_start` records the mouse
            // position and the *current* ratio. Here we compute (mouse_x -
            // anchor_pos) as a pixel delta and convert to a ratio delta using
            // the appropriate denominator. This eliminates the dead-zone bug
            // where deriving the ratio from the raw mouse position
            // disagreed with the solved layout (which clamps + adjusts the
            // configured ratios) and snapped the divider on the first
            // movement.
            .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, window, cx| {
                let (
                    is_dragging_lib_queue,
                    is_dragging_queue_rack,
                    is_dragging_queue_list,
                    is_dragging_meters,
                    is_dragging_lufs,
                    anchor_pos,
                    anchor_lib,
                    anchor_rack,
                    anchor_queue_list,
                    anchor_meters,
                    anchor_lufs,
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
                        layout.is_dragging_lufs_divider,
                        layout.drag_anchor_pos,
                        layout.drag_anchor_library_h_ratio,
                        layout.drag_anchor_rack_h_ratio,
                        layout.drag_anchor_queue_list_ratio,
                        layout.drag_anchor_meters_ratio,
                        layout.drag_anchor_lufs_ratio,
                        layout.library_h_ratio,
                        layout.rack_h_ratio,
                        layout.rack_panel_collapsed,
                    )
                };

                let window_size = window.bounds().size;
                let mouse_pos = event.position;
                let window_width: f32 = window_size.width.into();
                let mouse_x: f32 = mouse_pos.x.into();
                let dx = mouse_x - anchor_pos;
                let window_height: f32 = window_size.height.into();
                let mouse_y: f32 = mouse_pos.y.into();
                let dy = mouse_y - anchor_pos;

                if is_dragging_lib_queue && window_width > 0.0 {
                    let new_ratio = (anchor_lib + dx / window_width).clamp(0.15, 0.50);
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            layout.library_h_ratio = new_ratio;
                        });
                    });
                }

                if is_dragging_queue_rack && window_width > 0.0 {
                    // Rack grows leftward, so positive dx shrinks the ratio.
                    let new_ratio = (anchor_rack - dx / window_width).clamp(0.15, 0.50);
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            layout.rack_h_ratio = new_ratio;
                        });
                    });
                }

                // Inner queue dividers: deltas converted using queue width
                // (not window width). Queue width derives from current
                // library/rack ratios, which is fine because the user
                // shouldn't be dragging two dividers simultaneously.
                if is_dragging_queue_list || is_dragging_meters {
                    let queue_start = library_h_ratio * window_width;
                    let rack_width = if rack_collapsed {
                        0.0
                    } else {
                        rack_h_ratio * window_width
                    };
                    let queue_width = window_width - queue_start - rack_width;

                    if queue_width > 0.0 {
                        if is_dragging_queue_list {
                            let new_ratio = (anchor_queue_list + dx / queue_width).clamp(0.1, 0.5);
                            view.state.update(cx, |state, cx| {
                                state.layout.update(cx, |layout, _| {
                                    layout.queue_list_ratio = new_ratio;
                                });
                            });
                        }

                        if is_dragging_meters {
                            // Meters panel grows leftward — positive dx shrinks it.
                            let new_ratio = (anchor_meters - dx / queue_width).clamp(0.1, 0.5);
                            view.state.update(cx, |state, cx| {
                                state.layout.update(cx, |layout, _| {
                                    layout.meters_panel_ratio = new_ratio;
                                });
                            });
                        }
                    }
                }

                if is_dragging_lufs && window_height > 0.0 {
                    let new_ratio = (anchor_lufs + dy / window_height).clamp(0.20, 0.82);
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            layout.lufs_panel_ratio = new_ratio;
                        });
                    });
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
                                || layout.is_dragging_meters_divider
                                || layout.is_dragging_lufs_divider;

                            layout.is_dragging_library_queue_divider = false;
                            layout.is_dragging_queue_rack_divider = false;
                            layout.is_dragging_queue_list_divider = false;
                            layout.is_dragging_meters_divider = false;
                            layout.is_dragging_lufs_divider = false;

                            if any_dragging && let Err(e) = state.app.save_config(layout) {
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
                                    if let Err(e) = state.app.save_config(layout) {
                                        log::debug!("Config save failed: {e}");
                                    }
                                });
                            });
                        }
                    })
                    .on_drag_start({
                        let state_handle = self.state.clone();
                        move |pos, _window, cx| {
                            state_handle.update(cx, |state, cx| {
                                state.layout.update(cx, |layout, _| {
                                    layout.is_dragging_library_queue_divider = true;
                                    layout.drag_anchor_pos = pos;
                                    layout.drag_anchor_library_h_ratio = layout.library_h_ratio;
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
                                        if let Err(e) = state.app.save_config(layout) {
                                            log::debug!("Config save failed: {e}");
                                        }
                                    });
                                });
                            }
                        })
                        .on_drag_start({
                            let state_handle = self.state.clone();
                            move |pos, _window, cx| {
                                state_handle.update(cx, |state, cx| {
                                    state.layout.update(cx, |layout, _| {
                                        layout.is_dragging_queue_rack_divider = true;
                                        layout.drag_anchor_pos = pos;
                                        layout.drag_anchor_rack_h_ratio = layout.rack_h_ratio;
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
            tint: Rgba {
                a: 0.42,
                ..theme.accent
            },
            tint_hover: theme.accent,
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            // Global mouse move handler for all divider dragging (vertical layout).
            // Delta-based — see horizontal-layout handler comment.
            .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, window, cx| {
                let (
                    is_dragging_lib_queue,
                    is_dragging_queue_rack,
                    is_dragging_queue_list,
                    is_dragging_meters,
                    is_dragging_lufs,
                    anchor_pos,
                    anchor_lib_v,
                    anchor_rack_v,
                    anchor_queue_list,
                    anchor_meters,
                    anchor_lufs,
                ) = {
                    let layout = view.state.read(cx).layout.read(cx);
                    (
                        layout.is_dragging_library_queue_divider,
                        layout.is_dragging_queue_rack_divider,
                        layout.is_dragging_queue_list_divider,
                        layout.is_dragging_meters_divider,
                        layout.is_dragging_lufs_divider,
                        layout.drag_anchor_pos,
                        layout.drag_anchor_library_v_ratio,
                        layout.drag_anchor_rack_v_ratio,
                        layout.drag_anchor_queue_list_ratio,
                        layout.drag_anchor_meters_ratio,
                        layout.drag_anchor_lufs_ratio,
                    )
                };

                let window_size = window.bounds().size;
                let mouse_pos = event.position;

                if is_dragging_lib_queue {
                    let window_height: f32 = window_size.height.into();
                    if window_height > 0.0 {
                        let mouse_y: f32 = mouse_pos.y.into();
                        let dy = mouse_y - anchor_pos;
                        let new_ratio = (anchor_lib_v + dy / window_height).clamp(0.15, 0.50);
                        view.state.update(cx, |state, cx| {
                            state.layout.update(cx, |layout, _| {
                                layout.library_v_ratio = new_ratio;
                            });
                        });
                    }
                }

                if is_dragging_queue_rack {
                    let window_height: f32 = window_size.height.into();
                    if window_height > 0.0 {
                        let mouse_y: f32 = mouse_pos.y.into();
                        let dy = mouse_y - anchor_pos;
                        // Rack grows upward — positive dy shrinks it.
                        let new_ratio = (anchor_rack_v - dy / window_height).clamp(0.15, 0.50);
                        view.state.update(cx, |state, cx| {
                            state.layout.update(cx, |layout, _| {
                                layout.rack_v_ratio = new_ratio;
                            });
                        });
                    }
                }

                // Inner queue dividers: vertical layout, queue panel spans full window width.
                if is_dragging_queue_list || is_dragging_meters {
                    let window_width: f32 = window_size.width.into();
                    if window_width > 0.0 {
                        let mouse_x: f32 = mouse_pos.x.into();
                        let dx = mouse_x - anchor_pos;

                        if is_dragging_queue_list {
                            let new_ratio = (anchor_queue_list + dx / window_width).clamp(0.1, 0.5);
                            view.state.update(cx, |state, cx| {
                                state.layout.update(cx, |layout, _| {
                                    layout.queue_list_ratio = new_ratio;
                                });
                            });
                        }

                        if is_dragging_meters {
                            let new_ratio = (anchor_meters - dx / window_width).clamp(0.1, 0.5);
                            view.state.update(cx, |state, cx| {
                                state.layout.update(cx, |layout, _| {
                                    layout.meters_panel_ratio = new_ratio;
                                });
                            });
                        }
                    }
                }

                if is_dragging_lufs {
                    let window_height: f32 = window_size.height.into();
                    if window_height > 0.0 {
                        let mouse_y: f32 = mouse_pos.y.into();
                        let dy = mouse_y - anchor_pos;
                        let new_ratio = (anchor_lufs + dy / window_height).clamp(0.20, 0.82);
                        view.state.update(cx, |state, cx| {
                            state.layout.update(cx, |layout, _| {
                                layout.lufs_panel_ratio = new_ratio;
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
                                || layout.is_dragging_meters_divider
                                || layout.is_dragging_lufs_divider;

                            layout.is_dragging_library_queue_divider = false;
                            layout.is_dragging_queue_rack_divider = false;
                            layout.is_dragging_queue_list_divider = false;
                            layout.is_dragging_meters_divider = false;
                            layout.is_dragging_lufs_divider = false;

                            if any_dragging && let Err(e) = state.app.save_config(layout) {
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
                                    if let Err(e) = state.app.save_config(layout) {
                                        log::debug!("Config save failed: {e}");
                                    }
                                });
                            });
                        }
                    })
                    .on_drag_start({
                        let state_handle = self.state.clone();
                        move |pos, _window, cx| {
                            state_handle.update(cx, |state, cx| {
                                state.layout.update(cx, |layout, _| {
                                    layout.is_dragging_library_queue_divider = true;
                                    layout.drag_anchor_pos = pos;
                                    layout.drag_anchor_library_v_ratio = layout.library_v_ratio;
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
                                        if let Err(e) = state.app.save_config(layout) {
                                            log::debug!("Config save failed: {e}");
                                        }
                                    });
                                });
                            }
                        })
                        .on_drag_start({
                            let state_handle = self.state.clone();
                            move |pos, _window, cx| {
                                state_handle.update(cx, |state, cx| {
                                    state.layout.update(cx, |layout, _| {
                                        layout.is_dragging_queue_rack_divider = true;
                                        layout.drag_anchor_pos = pos;
                                        layout.drag_anchor_rack_v_ratio = layout.rack_v_ratio;
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
            .child(div().flex_1().p(d.pad_y).child(self.render_side_meter(
                cx,
                output_channels,
                "",
                true,
                false,
            )))
    }

    /// Render queue content for 3-panel layout
    /// Meters visibility is controlled by hide_queue_meters_for_rack state field
    pub fn render_queue_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_queue_screen(cx)
    }
}
