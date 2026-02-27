// Three-panel layout implementation for responsive Library | Queue | Rack display
// Horizontal mode (width > height): panels side-by-side
// Vertical mode (height >= width): panels stacked

impl PlayerView {
    /// Render horizontal 3-panel layout: Library | Queue | Rack
    pub fn render_horizontal_3panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, library_ratio, rack_ratio, rack_mode, library_collapsed, rack_collapsed) = {
            let state = self.state.read(cx);
            let layout = state.layout.read(cx);
            (
                state.app.ui_state.theme.clone(),
                layout.library_h_ratio,
                layout.rack_h_ratio,
                state.app.rack_display_mode,
                layout.library_panel_collapsed,
                layout.rack_panel_collapsed,
            )
        };

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
            // Global mouse move handler for 3-panel divider dragging
            .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, window, cx| {
                let (is_dragging_lib_queue, is_dragging_queue_rack) = {
                    let layout = view.state.read(cx).layout.read(cx);
                    (
                        layout.is_dragging_library_queue_divider,
                        layout.is_dragging_queue_rack_divider,
                    )
                };

                let window_size = window.bounds().size;
                let mouse_pos = event.position;

                if is_dragging_lib_queue {
                    let window_width: f32 = window_size.width.into();
                    let mouse_x: f32 = mouse_pos.x.into();
                    let new_ratio = (mouse_x / window_width).clamp(0.15, 0.50);
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            layout.library_h_ratio = new_ratio;
                        });
                    });
                }

                if is_dragging_queue_rack {
                    let window_width: f32 = window_size.width.into();
                    let mouse_x: f32 = mouse_pos.x.into();
                    // Rack ratio is from the right edge
                    let new_ratio = (1.0 - (mouse_x / window_width)).clamp(0.15, 0.50);
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            layout.rack_h_ratio = new_ratio;
                        });
                    });
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _event: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            if layout.is_dragging_library_queue_divider
                                || layout.is_dragging_queue_rack_divider
                            {
                                layout.is_dragging_library_queue_divider = false;
                                layout.is_dragging_queue_rack_divider = false;
                                // Save panel layout
                                if let Err(e) = state.app.save_config(layout) {
                                    log::warn!("Failed to save panel layout: {}", e);
                                }
                            }
                        });
                    });
                }),
            )
            // Library panel (left)
            .when(!library_collapsed, |d| {
                d.child(
                    div()
                        .w(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                            library_ratio,
                        )))
                        .h_full()
                        .overflow_hidden()
                        .child(self.render_library_screen(cx)),
                )
            })
            // Library-Queue divider
            .child({
                PaneDivider::vertical("lib-queue-h-divider", CollapseDirection::Left)
                    .label("Library")
                    .collapsed(library_collapsed)
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
                    .h_full()
                    .overflow_hidden()
                    .child(self.render_queue_content(cx)),
            )
            // Queue-Rack divider
            .child({
                PaneDivider::vertical("queue-rack-h-divider", CollapseDirection::Right)
                    .label("Rack")
                    .collapsed(rack_collapsed)
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
            // Rack panel (right)
            .when(!rack_collapsed, |d| {
                d.child(
                    div()
                        .w(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                            rack_ratio,
                        )))
                        .h_full()
                        .overflow_hidden()
                        .child(self.render_rack_for_mode(rack_mode, cx)),
                )
            })
    }

    /// Render vertical 3-panel layout: Library / Queue / Rack stacked
    pub fn render_vertical_3panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, library_ratio, rack_ratio, rack_mode, library_collapsed, rack_collapsed) = {
            let state = self.state.read(cx);
            let layout = state.layout.read(cx);
            (
                state.app.ui_state.theme.clone(),
                layout.library_v_ratio,
                layout.rack_v_ratio,
                state.app.rack_display_mode,
                layout.library_panel_collapsed,
                layout.rack_panel_collapsed,
            )
        };

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
            // Global mouse move handler for 3-panel divider dragging (vertical)
            .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, window, cx| {
                let (is_dragging_lib_queue, is_dragging_queue_rack) = {
                    let layout = view.state.read(cx).layout.read(cx);
                    (
                        layout.is_dragging_library_queue_divider,
                        layout.is_dragging_queue_rack_divider,
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
                    // Rack ratio is from the bottom edge
                    let new_ratio = (1.0 - (mouse_y / window_height)).clamp(0.15, 0.50);
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            layout.rack_v_ratio = new_ratio;
                        });
                    });
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _event: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            if layout.is_dragging_library_queue_divider
                                || layout.is_dragging_queue_rack_divider
                            {
                                layout.is_dragging_library_queue_divider = false;
                                layout.is_dragging_queue_rack_divider = false;
                                // Save panel layout
                                if let Err(e) = state.app.save_config(layout) {
                                    log::warn!("Failed to save panel layout: {}", e);
                                }
                            }
                        });
                    });
                }),
            )
            // Library panel (top)
            .when(!library_collapsed, |d| {
                d.child(
                    div()
                        .h(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                            library_ratio,
                        )))
                        .w_full()
                        .overflow_hidden()
                        .child(self.render_library_screen(cx)),
                )
            })
            // Library-Queue divider (horizontal)
            .child({
                PaneDivider::horizontal("lib-queue-v-divider", CollapseDirection::Up)
                    .label("Library")
                    .collapsed(library_collapsed)
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
            // Queue-Rack divider (horizontal)
            .child({
                PaneDivider::horizontal("queue-rack-v-divider", CollapseDirection::Down)
                    .label("Rack")
                    .collapsed(rack_collapsed)
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
            // Rack panel (bottom)
            .when(!rack_collapsed, |d| {
                d.child(
                    div()
                        .h(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                            rack_ratio,
                        )))
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
        let (theme, output_channels) = {
            let state = self.state.read(cx);
            let channels = state.app.plugin_state.chain.output_channels();
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
                    .px_2()
                    .py_1()
                    .border_b_1()
                    .border_color(theme.border)
                    .text_xs()
                    .text_color(theme.text_muted)
                    .child("OUTPUT"),
            )
            // Output meters
            .child(
                div()
                    .flex_1()
                    .p_2()
                    .child(self.render_side_meter(cx, output_channels, "", true, false)),
            )
    }

    /// Render queue content for 3-panel layout
    /// Meters visibility is controlled by hide_queue_meters_for_rack state field
    pub fn render_queue_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_queue_screen(cx)
    }
}
