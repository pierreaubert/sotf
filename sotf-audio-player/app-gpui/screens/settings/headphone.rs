//! Headphone settings content

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Accordion, AccordionItem, AccordionMode, AccordionTheme, Button, ButtonSize, ButtonTheme,
    ButtonVariant, Card, HStack, Progress, ProgressSize, StackSpacing, Text, TextSize, TextWeight,
    VStack,
};

/// Target curve options for headphone EQ
pub const TARGET_CURVE_OPTIONS: &[(&str, &str)] = &[
    ("harman-over-ear-2018", "Harman Over-Ear 2018"),
    ("harman-over-ear-2015", "Harman Over-Ear 2015"),
    ("harman-over-ear-2013", "Harman Over-Ear 2013"),
    ("harman-in-ear-2019", "Harman In-Ear 2019"),
    ("custom", "Custom File..."),
];

impl PlayerView {
    pub(crate) fn render_headphone_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (
            theme,
            headphone_curve_path,
            headphone_target,
            headphone_target_custom_path,
            headphone_params,
            optimization_running,
            optimization_progress,
            headphone_optimization_result,
            headphone_export_format,
            headphone_eq_save_name,
            expanded_sections,
            headphone_opt_ui,
        ) = {
            let state = self.state.read(cx);
            (
                state.app.theme.clone(),
                state.app.headphone_curve_path.clone(),
                state.app.headphone_target.clone(),
                state.app.headphone_target_custom_path.clone(),
                state.app.headphone_params.clone(),
                state.app.headphone_optimization_running,
                state.app.headphone_optimization_progress.clone(),
                state.app.headphone_optimization_result.clone(),
                state.app.headphone_export_format.clone(),
                state.app.headphone_eq_save_name.clone(),
                state.app.headphone_expanded_sections.clone(),
                state.app.headphone_opt_ui.clone(),
            )
        };

        let button_theme = ButtonTheme {
            accent: theme.accent,
            accent_hover: theme.accent,
            surface: theme.surface_hover,
            surface_hover: theme.background_tertiary,
            text_primary: theme.text_primary,
            text_secondary: theme.text_secondary,
            error: theme.error,
            border: theme.border,
        };

        let accordion_theme = AccordionTheme {
            header_bg: theme.surface,
            header_hover_bg: theme.surface_hover,
            content_bg: theme.background,
            border: theme.border,
            title_color: theme.text_primary,
            indicator_color: theme.text_muted,
        };

        let view = cx.entity().clone();

        div()
            .id("headphone-settings-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                div()
                    .id("headphone-settings-content")
                    .flex()
                    .flex_col()
                    .gap_4()
                    .pb_4()
                    // Intro text
                    .child(
                        Text::new(
                            "Generate EQ curves for headphones using industry-standard target curves",
                        )
                        .size(TextSize::Xs)
                        .muted(true),
                    )
                    // Accordion sections for optimization parameters
                    .child(
                        Accordion::new()
                            .mode(AccordionMode::Multiple)
                            .theme(accordion_theme.clone())
                            .expanded(expanded_sections.clone())
                            .item(
                                AccordionItem::new("measurement", "Measurement File")
                                    .content(self.render_file_selection_section(
                                        "Headphone Measurement File",
                                        &headphone_curve_path,
                                        "No file selected",
                                        "browse-headphone-curve",
                                        "Browse",
                                        &theme,
                                        cx,
                                    )),
                            )
                            .item(
                                AccordionItem::new("target", "Target Curve").content(
                                    self.render_target_selection(
                                        &headphone_target,
                                        &headphone_target_custom_path,
                                        &theme,
                                        cx,
                                    ),
                                ),
                            )
                            .item(
                                AccordionItem::new("goal", "Optimization Goal").content(
                                    self.render_option_chips(
                                        "Optimization Goal",
                                        &headphone_params.loss,
                                        &[
                                            ("headphone-flat", "Target"),
                                            ("headphone-score", "Harman Score"),
                                        ],
                                        "headphone-loss",
                                        &theme,
                                        cx,
                                    ),
                                ),
                            )
                            .item(
                                AccordionItem::new("eq-design", "EQ Design Parameters").content(
                                    self.render_eq_design_params(
                                        &headphone_params,
                                        &headphone_opt_ui,
                                        "headphone",
                                        &theme,
                                        cx,
                                    ),
                                ),
                            )
                            .item(
                                AccordionItem::new("tuning", "Optimization Fine Tuning").content(
                                    self.render_optimization_tuning_params(
                                        &headphone_params,
                                        &headphone_opt_ui,
                                        "headphone",
                                        &theme,
                                        cx,
                                    ),
                                ),
                            )
                            .on_change({
                                let view = view.clone();
                                move |id, expanded, _window, cx| {
                                    view.update(cx, |view, cx| {
                                        view.toggle_headphone_section(id.as_ref(), expanded, cx);
                                    });
                                }
                            }),
                    )
                    // Generate button
                    .child(
                        Button::new(
                            "generate-headphone-eq",
                            if optimization_running {
                                "Optimizing..."
                            } else {
                                "Generate Headphone EQ"
                            },
                        )
                        .variant(ButtonVariant::Primary)
                        .size(ButtonSize::Lg)
                        .full_width(true)
                        .disabled(optimization_running)
                        .theme(button_theme.clone())
                        .build()
                        .when(!optimization_running, |d| {
                            d.on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.run_headphone_optimization(cx);
                                }),
                            )
                        }),
                    )
                    // Progress section
                    .when(optimization_running, |d| {
                        d.child(self.render_optimization_progress(
                            &optimization_progress,
                            headphone_params.maxeval,
                            &theme,
                        ))
                    })
                    // Results section
                    .when_some(headphone_optimization_result.as_ref(), |d, result| {
                        d.child(
                            Card::new()
                                .header(
                                    Text::new("Optimization Results").weight(TextWeight::Semibold),
                                )
                                .content(
                                    self.render_optimization_result_graphs(result, &theme, 1000.0),
                                ),
                        )
                    })
                    // Listen & Save EQ accordion (below results)
                    .child(
                        Accordion::new()
                            .mode(AccordionMode::Multiple)
                            .theme(accordion_theme)
                            .expanded(expanded_sections)
                            .item(
                                AccordionItem::new("listen", "Listen").content(
                                    self.render_listen_section(
                                        headphone_optimization_result.as_ref(),
                                        &theme,
                                        cx,
                                    ),
                                ),
                            )
                            .item(
                                AccordionItem::new("save", "Save EQ").content(
                                    self.render_save_eq_section(
                                        &headphone_export_format,
                                        &headphone_eq_save_name,
                                        headphone_optimization_result.is_some(),
                                        &theme,
                                        cx,
                                    ),
                                ),
                            )
                            .on_change(move |id, expanded, _window, cx| {
                                view.update(cx, |view, cx| {
                                    view.toggle_headphone_section(id.as_ref(), expanded, cx);
                                });
                            }),
                    ),
            )
    }

    /// Toggle a headphone accordion section
    pub fn toggle_headphone_section(
        &mut self,
        section_id: &str,
        expanded: bool,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            let sections = &mut state.app.headphone_expanded_sections;
            let id = SharedString::from(section_id.to_string());
            if expanded {
                if !sections.contains(&id) {
                    sections.push(id);
                }
            } else {
                sections.retain(|s| s != &id);
            }
        });
        cx.notify();
    }

    /// Render the Listen section with EQ preview and apply to playback
    fn render_listen_section(
        &self,
        result: Option<&crate::autoeq::HeadphoneOptimizationResult>,
        theme: &crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = theme.clone();

        VStack::new()
            .spacing(StackSpacing::Md)
            .when(result.is_none(), |vstack| {
                vstack.child(
                    Text::new("Run optimization first to preview the EQ")
                        .size(TextSize::Xs)
                        .muted(true),
                )
            })
            .when_some(result, |vstack, result| {
                let num_filters = result.biquads.len();
                let biquads = result.biquads.clone();

                vstack
                    // EQ Filters summary
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(
                                Text::new(format!("EQ Preview ({} filters)", num_filters))
                                    .size(TextSize::Sm)
                                    .weight(TextWeight::Medium),
                            )
                            // Filter list
                            .child(
                                div()
                                    .id("filter-list-scroll")
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .p_2()
                                    .rounded_md()
                                    .bg(theme.surface)
                                    .max_h(px(200.0))
                                    .overflow_y_scroll()
                                    .children(biquads.iter().enumerate().map(|(i, biquad)| {
                                        let filter_type = format!("{:?}", biquad.filter_type);
                                        let freq = biquad.freq;
                                        let q = biquad.q;
                                        let gain = biquad.db_gain;

                                        div()
                                            .flex()
                                            .justify_between()
                                            .items_center()
                                            .px_2()
                                            .py_1()
                                            .rounded(px(4.0))
                                            .bg(theme.background)
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.accent)
                                                            .child(format!("#{}", i + 1)),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.text_secondary)
                                                            .child(filter_type),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_3()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.text_primary)
                                                            .child(format!("{:.0} Hz", freq)),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.text_muted)
                                                            .child(format!("Q {:.2}", q)),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(if gain >= 0.0 {
                                                                theme.success
                                                            } else {
                                                                theme.error
                                                            })
                                                            .child(format!("{:+.1} dB", gain)),
                                                    ),
                                            )
                                    })),
                            ),
                    )
                    // Apply to playback button
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(
                                Button::new("apply-eq-to-playback", "Apply to Playback")
                                    .variant(ButtonVariant::Primary)
                                    .size(ButtonSize::Md)
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                            view.apply_headphone_eq_to_playback(cx);
                                        }),
                                    ),
                            )
                            .child(
                                Button::new("clear-eq-from-playback", "Clear EQ")
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Md)
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                            view.clear_headphone_eq_from_playback(cx);
                                        }),
                                    ),
                            ),
                    )
                    .child(
                        Text::new("Applies the computed EQ filters to the current playback chain")
                            .size(TextSize::Xs)
                            .muted(true),
                    )
            })
    }

    /// Render a file selection row with label, path display, and browse button
    fn render_file_selection_section(
        &self,
        label: &str,
        path: &str,
        placeholder: &str,
        button_id: &str,
        button_label: &str,
        theme: &crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = theme.clone();
        let display_path = if path.is_empty() {
            placeholder.to_string()
        } else {
            path.to_string()
        };
        let label = SharedString::from(label.to_string());
        let button_id = SharedString::from(button_id.to_string());
        let button_label = SharedString::from(button_label.to_string());

        VStack::new()
            .spacing(gpui_ui_kit::StackSpacing::Sm)
            .child(
                Text::new(label)
                    .size(TextSize::Xs)
                    .weight(TextWeight::Medium),
            )
            .child(
                HStack::new()
                    .spacing(gpui_ui_kit::StackSpacing::Sm)
                    .child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(theme.background_secondary)
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child(display_path),
                    )
                    .child(
                        Button::new(button_id, button_label)
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Sm)
                            .build()
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.browse_headphone_curve(cx);
                                }),
                            ),
                    ),
            )
    }

    /// Render target curve selection with chips
    fn render_target_selection(
        &self,
        current_target: &str,
        custom_path: &str,
        theme: &crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = theme.clone();
        let current_target = current_target.to_string();
        let custom_path = custom_path.to_string();

        VStack::new()
            .spacing(gpui_ui_kit::StackSpacing::Sm)
            .child(
                HStack::new()
                    .spacing(gpui_ui_kit::StackSpacing::Sm)
                    .wrap(true)
                    .children(TARGET_CURVE_OPTIONS.iter().map(|(value, label)| {
                        let is_selected = current_target == *value;
                        let value = value.to_string();
                        let is_custom = value == "custom";

                        Button::new(
                            SharedString::from(format!("headphone-target-{}", value)),
                            *label,
                        )
                        .variant(if is_selected {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(ButtonSize::Sm)
                        .build()
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                if is_custom {
                                    view.browse_target_curve(cx);
                                } else {
                                    view.state.update(cx, |state, _cx| {
                                        state.app.headphone_target = value.clone();
                                    });
                                    cx.notify();
                                }
                            }),
                        )
                    })),
            )
            .when(current_target == "custom", |vstack| {
                let theme = theme.clone();
                vstack.child(
                    HStack::new()
                        .spacing(gpui_ui_kit::StackSpacing::Sm)
                        .child(
                            div()
                                .flex_1()
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .bg(theme.background_secondary)
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child(if custom_path.is_empty() {
                                    "No custom target file selected".to_string()
                                } else {
                                    custom_path.clone()
                                }),
                        )
                        .child(
                            Button::new("browse-custom-target", "Change")
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Sm)
                                .build()
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                        view.browse_target_curve(cx);
                                    }),
                                ),
                        ),
                )
            })
    }

    /// Render option chips (for loss function, etc.)
    fn render_option_chips(
        &self,
        _label: &str,
        current_value: &str,
        options: &[(&str, &str)],
        id_prefix: &str,
        _theme: &crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id_prefix = id_prefix.to_string();
        let current_value = current_value.to_string();
        // Convert options to owned strings
        let options: Vec<(String, String)> = options
            .iter()
            .map(|(v, l)| (v.to_string(), l.to_string()))
            .collect();

        HStack::new()
            .spacing(gpui_ui_kit::StackSpacing::Sm)
            .wrap(true)
            .children(options.into_iter().map(|(value, display_label)| {
                let is_selected = current_value == value;
                let id_prefix = id_prefix.clone();
                let value_clone = value.clone();

                Button::new(
                    SharedString::from(format!("{}-{}", id_prefix, value)),
                    SharedString::from(display_label),
                )
                .variant(if is_selected {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Secondary
                })
                .size(ButtonSize::Sm)
                .build()
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                        view.state.update(cx, |state, _cx| {
                            state.app.headphone_params.loss = value_clone.clone();
                        });
                        cx.notify();
                    }),
                )
            }))
    }

    /// Render save EQ section with format selection and save button
    fn render_save_eq_section(
        &self,
        current_format: &str,
        save_name: &str,
        has_result: bool,
        theme: &crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = theme.clone();
        let eq_files = self.list_saved_eq_files();
        let view = cx.entity().downgrade();

        VStack::new()
            .spacing(StackSpacing::Md)
            // Name input
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new("EQ Name")
                            .size(TextSize::Xs)
                            .weight(TextWeight::Medium),
                    )
                    .child(
                        div()
                            .id("headphone-eq-name-input")
                            .flex()
                            .items_center()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.surface)
                            .text_sm()
                            .text_color(if save_name.is_empty() {
                                theme.text_muted
                            } else {
                                theme.text_primary
                            })
                            .child(if save_name.is_empty() {
                                SharedString::from("Enter a name for your EQ preset (optional)")
                            } else {
                                SharedString::from(format!("{}|", save_name))
                            })
                            .on_mouse_up(MouseButton::Left, {
                                let view = view.clone();
                                move |_, _window, cx| {
                                    // Start editing the EQ name
                                    if let Some(view) = view.upgrade() {
                                        view.update(cx, |view, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                state.app.editing_param =
                                                    Some("headphone_eq_save_name".to_string());
                                                state.app.editing_value =
                                                    state.app.headphone_eq_save_name.clone();
                                            });
                                        });
                                    }
                                }
                            }),
                    ),
            )
            // Format selection
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new("Export Format")
                            .size(TextSize::Xs)
                            .weight(TextWeight::Medium),
                    )
                    .child(HStack::new().spacing(StackSpacing::Sm).wrap(true).children(
                        crate::autoeq::EQ_EXPORT_FORMAT_OPTIONS.iter().map(
                            |(value, label, _ext)| {
                                let is_selected = current_format == *value;
                                let value = value.to_string();

                                Button::new(
                                    SharedString::from(format!("export-format-{}", value)),
                                    *label,
                                )
                                .variant(if is_selected {
                                    ButtonVariant::Primary
                                } else {
                                    ButtonVariant::Secondary
                                })
                                .size(ButtonSize::Sm)
                                .build()
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                        view.state.update(cx, |state, _cx| {
                                            state.app.headphone_export_format = value.clone();
                                        });
                                        cx.notify();
                                    }),
                                )
                            },
                        ),
                    )),
            )
            // Save button
            .child(
                Button::new("save-headphone-eq", "Save Current EQ")
                    .variant(ButtonVariant::Primary)
                    .size(ButtonSize::Md)
                    .full_width(true)
                    .disabled(!has_result)
                    .build()
                    .when(has_result, |btn| {
                        btn.on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                view.save_headphone_eq(cx);
                            }),
                        )
                    }),
            )
            .when(!has_result, |vstack| {
                vstack.child(
                    Text::new("Run optimization first to generate an EQ curve")
                        .size(TextSize::Xs)
                        .muted(true),
                )
            })
            // Saved files list
            .when(!eq_files.is_empty(), |vstack| {
                vstack
                    .child(div().h(px(1.0)).w_full().bg(theme.border).my_2())
                    .child(
                        Text::new("Saved EQ Files")
                            .size(TextSize::Xs)
                            .weight(TextWeight::Medium),
                    )
                    .child(
                        Text::new("~/Library/Application Support/org.spinorama.sotf/EQ")
                            .size(TextSize::Xs)
                            .muted(true),
                    )
                    .children(eq_files.into_iter().map(|path| {
                        let filename = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Unknown")
                            .to_string();
                        let path_clone = path.clone();
                        let path_clone2 = path.clone();
                        let theme = theme.clone();

                        HStack::new()
                            .spacing(gpui_ui_kit::StackSpacing::Sm)
                            .child(
                                div()
                                    .flex_1()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .bg(theme.surface_hover)
                                    .text_xs()
                                    .text_color(theme.text_primary)
                                    .child(filename),
                            )
                            .child(
                                Button::new(
                                    SharedString::from(format!("load-eq-{}", path.display())),
                                    "Load",
                                )
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Xs)
                                .build()
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                        view.load_headphone_eq(path_clone.clone(), cx);
                                    }),
                                ),
                            )
                            .child(
                                Button::new(
                                    SharedString::from(format!("delete-eq-{}", path.display())),
                                    "Delete",
                                )
                                .variant(ButtonVariant::Destructive)
                                .size(ButtonSize::Xs)
                                .build()
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                        view.delete_headphone_eq(path_clone2.clone(), cx);
                                    }),
                                ),
                            )
                    }))
            })
    }

    /// Render optimization progress card with live loss curve
    fn render_optimization_progress(
        &self,
        progress: &[(usize, f64)],
        maxeval: usize,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        let current_iter = progress.last().map(|(i, _)| *i).unwrap_or(0);
        let progress_pct = if maxeval > 0 {
            ((current_iter as f32 / maxeval as f32) * 100.0).min(100.0)
        } else {
            0.0
        };

        let status_text = if progress.is_empty() {
            "Starting optimization...".to_string()
        } else {
            format!(
                "Iteration: {} / {} | Loss: {:.4}",
                current_iter,
                maxeval,
                progress.last().map(|(_, f)| *f).unwrap_or(0.0)
            )
        };

        Card::new()
            .header(Text::new("Optimization Progress").weight(TextWeight::Semibold))
            .content(
                VStack::new()
                    .spacing(gpui_ui_kit::StackSpacing::Sm)
                    .child(Text::new(status_text).size(TextSize::Xs).muted(true))
                    .child(Progress::new(progress_pct).size(ProgressSize::Sm))
                    // Live loss curve graph
                    .when(!progress.is_empty(), |vstack| {
                        vstack.child(self.render_live_loss_graph(progress, maxeval, theme))
                    }),
            )
    }

    /// Render the live loss curve during optimization
    fn render_live_loss_graph(
        &self,
        progress: &[(usize, f64)],
        maxeval: usize,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        use d3rs::color::D3Color;
        use d3rs::scale::LinearScale;
        use d3rs::shape::{LineConfig, LinePoint, render_line};

        let graph_width = 400.0_f32;
        let graph_height = 120.0_f32;

        if progress.is_empty() {
            return div().w(px(graph_width)).h(px(graph_height));
        }

        // Calculate scales
        let max_iter = maxeval.max(progress.last().map(|x| x.0).unwrap_or(1)) as f64;
        let losses: Vec<f64> = progress.iter().map(|x| x.1).collect();
        let min_loss = losses.iter().copied().fold(f64::INFINITY, f64::min);
        let max_loss = losses.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let loss_range = (max_loss - min_loss).max(0.1);
        let min_loss_padded = min_loss - loss_range * 0.1;
        let max_loss_padded = max_loss + loss_range * 0.1;

        let iter_scale = LinearScale::new()
            .domain(0.0, max_iter)
            .range(0.0, graph_width as f64);
        let loss_scale = LinearScale::new()
            .domain(min_loss_padded, max_loss_padded)
            .range(graph_height as f64, 0.0);

        // Create line points
        let points: Vec<LinePoint> = progress
            .iter()
            .map(|&(i, loss)| LinePoint::new(i as f64, loss))
            .collect();

        let config = LineConfig::new()
            .stroke_width(2.0)
            .stroke_color(D3Color::from_rgba(gpui::rgb(0x8b5cf6))); // Violet

        let curve = render_line(&iter_scale, &loss_scale, &points, &config);

        // Grid lines
        let grid_color = gpui::rgba(0xffffff15);

        div()
            .w(px(graph_width))
            .h(px(graph_height))
            .bg(theme.surface)
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .relative()
            .overflow_hidden()
            // Horizontal grid lines
            .child(
                div()
                    .absolute()
                    .top(px(graph_height * 0.25))
                    .left_0()
                    .right_0()
                    .h(px(1.0))
                    .bg(grid_color),
            )
            .child(
                div()
                    .absolute()
                    .top(px(graph_height * 0.5))
                    .left_0()
                    .right_0()
                    .h(px(1.0))
                    .bg(grid_color),
            )
            .child(
                div()
                    .absolute()
                    .top(px(graph_height * 0.75))
                    .left_0()
                    .right_0()
                    .h(px(1.0))
                    .bg(grid_color),
            )
            // Vertical grid lines
            .child(
                div()
                    .absolute()
                    .left(px(graph_width * 0.25))
                    .top_0()
                    .bottom_0()
                    .w(px(1.0))
                    .bg(grid_color),
            )
            .child(
                div()
                    .absolute()
                    .left(px(graph_width * 0.5))
                    .top_0()
                    .bottom_0()
                    .w(px(1.0))
                    .bg(grid_color),
            )
            .child(
                div()
                    .absolute()
                    .left(px(graph_width * 0.75))
                    .top_0()
                    .bottom_0()
                    .w(px(1.0))
                    .bg(grid_color),
            )
            .child(curve)
    }
}
