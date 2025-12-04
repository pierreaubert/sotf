//! Headphone settings content

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;

impl PlayerView {
    pub(crate) fn render_headphone_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (
            theme,
            headphone_curve_path,
            headphone_target,
            headphone_params,
            optimization_running,
            optimization_progress,
        ) = {
            let state = self.state.read(cx);
            (
                state.app.theme.clone(),
                state.app.headphone_curve_path.clone(),
                state.app.headphone_target.clone(),
                state.app.headphone_params.clone(),
                state.app.headphone_optimization_running,
                state.app.headphone_optimization_progress.clone(),
            )
        };

        div()
            .flex()
            .flex_col()
            .gap_4()
            // Headphone EQ intro
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_secondary)
                    .child("Generate EQ curves for headphones using industry-standard target curves"),
            )
            // Headphone curve file selection
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Headphone Measurement File"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .bg(theme.background_secondary)
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child(if headphone_curve_path.is_empty() {
                                        String::from("No file selected")
                                    } else {
                                        headphone_curve_path.clone()
                                    }),
                            )
                            .child(
                                div()
                                    .id("browse-headphone-curve")
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .text_xs()
                                    .bg(theme.surface_hover)
                                    .hover(|style| style.bg(theme.background_tertiary))
                                    .cursor_pointer()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                            view.browse_headphone_curve(cx);
                                        }),
                                    )
                                    .child("📁 Browse"),
                            ),
                    ),
            )
            // Target curve selection
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Target Curve"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .children(
                                [
                                    ("harman-over-ear-2018", "Harman Over-Ear 2018"),
                                    ("harman-over-ear-2015", "Harman Over-Ear 2015"),
                                    ("harman-over-ear-2013", "Harman Over-Ear 2013"),
                                    ("harman-in-ear-2019", "Harman In-Ear 2019"),
                                ]
                                .iter()
                                .map(|(value, label)| {
                                    let is_selected = headphone_target == *value;
                                    let value = value.to_string();
                                    let theme = theme.clone();
                                    div()
                                        .id(SharedString::from(format!(
                                            "headphone-target-{}",
                                            value
                                        )))
                                        .px_3()
                                        .py_2()
                                        .rounded_md()
                                        .text_xs()
                                        .cursor_pointer()
                                        .when(is_selected, |d| {
                                            d.bg(theme.accent).text_color(theme.text_primary)
                                        })
                                        .when(!is_selected, |d| {
                                            d.bg(theme.surface_hover)
                                                .text_color(theme.text_secondary)
                                                .hover(|style| {
                                                    style.bg(theme.background_tertiary)
                                                })
                                        })
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(
                                                move |view, _: &MouseUpEvent, _window, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state.app.headphone_target =
                                                            value.clone();
                                                    });
                                                    cx.notify();
                                                },
                                            ),
                                        )
                                        .child(div().text_xs().child(*label))
                                }),
                            ),
                    ),
            )
            // Loss function selection
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Optimization Goal"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .children(
                                crate::optimization_params::HEADPHONE_LOSS_OPTIONS
                                    .iter()
                                    .map(|(value, label)| {
                                        let is_selected = headphone_params.loss == *value;
                                        let value = value.to_string();
                                        let theme = theme.clone();
                                        div()
                                            .id(SharedString::from(format!(
                                                "headphone-loss-{}",
                                                value
                                            )))
                                            .px_3()
                                            .py_2()
                                            .rounded_md()
                                            .text_xs()
                                            .cursor_pointer()
                                            .when(is_selected, |d| {
                                                d.bg(theme.accent).text_color(theme.text_primary)
                                            })
                                            .when(!is_selected, |d| {
                                                d.bg(theme.surface_hover)
                                                    .text_color(theme.text_secondary)
                                                    .hover(|style| {
                                                        style.bg(theme.background_tertiary)
                                                    })
                                            })
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(
                                                    move |view, _: &MouseUpEvent, _window, cx| {
                                                        view.state.update(cx, |state, _cx| {
                                                            state.app.headphone_params.loss =
                                                                value.clone();
                                                        });
                                                        cx.notify();
                                                    },
                                                ),
                                            )
                                            .child(div().text_xs().child(*label))
                                    }),
                            ),
                    ),
            )
            // EQ Design Parameters (reusable component)
            .child(self.render_eq_design_params(&headphone_params, "headphone", &theme, cx))
            // Optimization Fine Tuning Parameters (reusable component)
            .child(self.render_optimization_tuning_params(&headphone_params, "headphone", &theme, cx))
            // Generate EQ button
            .child(
                div()
                    .id("generate-headphone-eq")
                    .w_full()
                    .px_4()
                    .py_3()
                    .rounded_md()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .bg(theme.accent)
                    .text_color(theme.text_primary)
                    .cursor_pointer()
                    .hover(|style| style.opacity(0.9))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                            view.run_headphone_optimization(cx);
                        }),
                    )
                    .child("🎧 Generate Headphone EQ"),
            )
            // Optimization progress (if running)
            .when(optimization_running, |div| {
                let progress = optimization_progress.clone();
                div.child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .p_4()
                        .bg(theme.surface)
                        .rounded_lg()
                        .child(
                            gpui::div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Optimization Progress"),
                        )
                        .child(
                            gpui::div()
                                .text_xs()
                                .text_color(theme.text_secondary)
                                .child(if progress.is_empty() {
                                    "Starting optimization...".to_string()
                                } else {
                                    format!(
                                        "Iteration: {} | Loss: {:.4}",
                                        progress.last().map(|(i, _)| *i).unwrap_or(0),
                                        progress.last().map(|(_, f)| *f).unwrap_or(0.0)
                                    )
                                }),
                        )
                        .child(
                            // Simple progress indicator
                            gpui::div()
                                .h(px(4.0))
                                .bg(theme.background_secondary)
                                .rounded_full()
                                .overflow_hidden()
                                .child(
                                    gpui::div()
                                        .h_full()
                                        .w(px(100.0))
                                        .bg(theme.accent)
                                        .rounded_full(),
                                ),
                        ),
                )
            })
            // Saved EQ files section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .mt_4()
                    .pt_4()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Saved EQ Curves"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child("EQ files are stored in ~/Library/Application Support/org.spinorama.sotf/EQ"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .mt_2()
                            .children({
                                let eq_files = self.list_saved_eq_files();
                                if eq_files.is_empty() {
                                    vec![div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .child("No saved EQ files")]
                                } else {
                                    eq_files
                                        .into_iter()
                                        .map(|path| {
                                            let filename = path
                                                .file_name()
                                                .and_then(|n| n.to_str())
                                                .unwrap_or("Unknown")
                                                .to_string();
                                            let path_clone = path.clone();
                                            let path_clone2 = path.clone();
                                            let theme = theme.clone();

                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .px_3()
                                                .py_2()
                                                .rounded_md()
                                                .bg(theme.surface_hover)
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .text_xs()
                                                        .text_color(theme.text_primary)
                                                        .child(filename),
                                                )
                                                .child(
                                                    div()
                                                        .px_2()
                                                        .py_1()
                                                        .rounded_sm()
                                                        .text_xs()
                                                        .bg(theme.accent)
                                                        .cursor_pointer()
                                                        .hover(|style| style.opacity(0.8))
                                                        .on_mouse_up(
                                                            MouseButton::Left,
                                                            cx.listener(
                                                                move |view,
                                                                      _: &MouseUpEvent,
                                                                      _window,
                                                                      cx| {
                                                                    view.load_headphone_eq(
                                                                        path_clone.clone(),
                                                                        cx,
                                                                    );
                                                                },
                                                            ),
                                                        )
                                                        .child("Load"),
                                                )
                                                .child(
                                                    div()
                                                        .px_2()
                                                        .py_1()
                                                        .rounded_sm()
                                                        .text_xs()
                                                        .bg(theme.error)
                                                        .cursor_pointer()
                                                        .hover(|style| style.opacity(0.8))
                                                        .on_mouse_up(
                                                            MouseButton::Left,
                                                            cx.listener(
                                                                move |view,
                                                                      _: &MouseUpEvent,
                                                                      _window,
                                                                      cx| {
                                                                    view.delete_headphone_eq(
                                                                        path_clone2.clone(),
                                                                        cx,
                                                                    );
                                                                },
                                                            ),
                                                        )
                                                        .child("Delete")
                                                )
                                        })
                                        .collect()
                                }
                            }),
                    ),
            )
    }
}
