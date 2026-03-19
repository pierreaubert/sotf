use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, Select, SelectOption, StackSpacing, Text,
    TextSize, TextWeight, Toggle, VStack,
};

use super::render::render_channel_result_card;

impl PlayerView {
    pub(crate) fn render_room_eq_review(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let room_eq = &state.app.measurement_state.room_eq_state;

        let pre_score = room_eq.average_pre_score();
        let post_score = room_eq.average_post_score();
        let smoothing_octaves = room_eq.review_smoothing_octaves;
        let smoothing_dropdown_open = room_eq.dropdowns.review_smoothing_open;
        let selected_channel_idx = room_eq.review_selected_channel;
        let channel_results = room_eq.channel_results.clone();

        let view = cx.entity().clone();

        // Smoothing options
        let smoothing_options = vec![
            SelectOption::new("0", "None"),
            SelectOption::new("0.25", "1/4 Oct"),
            SelectOption::new("0.5", "1/2 Oct"),
            SelectOption::new("1", "1 Oct"),
            SelectOption::new("2", "2 Oct"),
            SelectOption::new("3", "3 Oct"),
        ];

        let selected_smoothing = format!("{}", smoothing_octaves);
        let y_axis_auto = room_eq.review_y_axis_auto;

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Text::new("Review Results")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Md),
            )
            .child(
                Text::new("Review the optimization results before applying.")
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            // Channel selection buttons
            .when(channel_results.len() > 1, |vstack| {
                vstack.child(
                    Card::new()
                        .background(theme.surface)
                        .header_background(theme.background_secondary)
                        .border(theme.border)
                        .header(
                            Text::new("Select Channel")
                                .color(theme.text_primary)
                                .weight(TextWeight::Semibold),
                        )
                        .content(HStack::new().spacing(StackSpacing::Xs).children(
                            channel_results.iter().enumerate().map(|(idx, result)| {
                                let is_selected = idx == selected_channel_idx;
                                let channel_name = result.channel_name.clone();

                                Button::new(
                                    SharedString::from(format!("channel_select_{}", idx)),
                                    channel_name,
                                )
                                .variant(if is_selected {
                                    ButtonVariant::Primary
                                } else {
                                    ButtonVariant::Secondary
                                })
                                .size(ButtonSize::Sm)
                                .theme(theme.to_button_theme())
                                .build()
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |view, _, _, cx| {
                                        view.state.update(cx, |state, _| {
                                            state
                                                .app
                                                .measurement_state
                                                .room_eq_state
                                                .review_selected_channel = idx;
                                        });
                                        cx.notify();
                                    }),
                                )
                            }),
                        )),
                )
            })
            // Graph settings card
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("Graph Settings")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(
                                        Text::new("Smoothing:")
                                            .size(TextSize::Xs)
                                            .color(theme.text_secondary),
                                    )
                                    .child(
                                        Select::new("review_smoothing_select")
                                            .options(smoothing_options)
                                            .selected(selected_smoothing)
                                            .placeholder("Smoothing")
                                            .is_open(smoothing_dropdown_open)
                                            .theme(theme.to_select_theme())
                                            .on_toggle({
                                                let view = view.clone();
                                                move |open, _window, cx| {
                                                    view.update(cx, |this, cx| {
                                                        this.state.update(cx, |state, _| {
                                                            state
                                                                .app
                                                                .measurement_state
                                                                .room_eq_state
                                                                .dropdowns
                                                                .review_smoothing_open = open;
                                                        });
                                                        cx.notify();
                                                    });
                                                }
                                            })
                                            .on_change({
                                                let view = view.clone();
                                                move |value, _window, cx| {
                                                    view.update(cx, |this, cx| {
                                                        this.state.update(cx, |state, _| {
                                                            if let Ok(oct) = value.parse::<f64>() {
                                                                state
                                                                    .app
                                                                    .measurement_state
                                                                    .room_eq_state
                                                                    .review_smoothing_octaves = oct;
                                                            }
                                                            state
                                                                .app
                                                                .measurement_state
                                                                .room_eq_state
                                                                .dropdowns
                                                                .review_smoothing_open = false;
                                                        });
                                                        cx.notify();
                                                    });
                                                }
                                            }),
                                    ),
                            )
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(
                                        Text::new("Y-Axis Auto:")
                                            .size(TextSize::Xs)
                                            .color(theme.text_secondary),
                                    )
                                    .child(
                                        Toggle::new("review_y_axis_auto")
                                            .checked(y_axis_auto)
                                            .theme(theme.to_toggle_theme())
                                            .on_change({
                                                let view = view.clone();
                                                move |checked, _window, cx| {
                                                    view.update(cx, |this, cx| {
                                                        this.state.update(cx, |state, _| {
                                                            state
                                                                .app
                                                                .measurement_state
                                                                .room_eq_state
                                                                .review_y_axis_auto = checked;
                                                        });
                                                        cx.notify();
                                                    });
                                                }
                                            }),
                                    ),
                            ),
                    ),
            )
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("Score Summary")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(
                        VStack::new().spacing(StackSpacing::Xs).child(
                            HStack::new()
                                .spacing(StackSpacing::Md)
                                .child(
                                    Text::new(format!("Before: {:.2}", pre_score))
                                        .color(theme.text_primary),
                                )
                                .child(
                                    Text::new(format!("After: {:.2}", post_score))
                                        .color(theme.text_primary),
                                )
                                .child(
                                    Text::new(format!(
                                        "Improvement: {:.2}",
                                        pre_score - post_score
                                    ))
                                    .color(
                                        if post_score < pre_score {
                                            theme.success
                                        } else {
                                            theme.error
                                        },
                                    ),
                                ),
                        ),
                    ),
            )
            // Selected channel result
            .child(self.render_selected_channel_result(cx))
    }

    /// Render the selected channel's optimization result
    fn render_selected_channel_result(&self, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::app::types::room_eq::{CustomTargetCurve, InteractiveChartStateWrapper};

        // Initialize interactive chart state if needed
        {
            let state = self.state.read(cx);
            if state
                .app
                .measurement_state
                .room_eq_state
                .review_chart_state
                .is_none()
            {
                // Drop read borrow before update
                let _ = state;
                self.state.update(cx, |state, _| {
                    // Create interactive state for frequency response chart
                    // X: 20 Hz to 20 kHz (log scale), Y: -20 to +5 dB (typical range)
                    state.app.measurement_state.room_eq_state.review_chart_state = Some(
                        InteractiveChartStateWrapper::new(20.0, 20000.0, -20.0, 5.0)
                            .with_log_x(true)
                            .with_size(1200.0, 400.0),
                    );
                });
            }
        }

        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let room_eq = &state.app.measurement_state.room_eq_state;
        let channel_results = &room_eq.channel_results;
        let selected_idx = room_eq.review_selected_channel;
        let smoothing_octaves = room_eq.review_smoothing_octaves;
        let y_axis_auto = room_eq.review_y_axis_auto;
        let chart_state = room_eq.review_chart_state.as_ref().map(|w| w.inner());

        if channel_results.is_empty() {
            return VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Card::new()
                        .background(theme.surface)
                        .header_background(theme.background_secondary)
                        .border(theme.border)
                        .header(
                            Text::new("Channel Result")
                                .color(theme.text_primary)
                                .weight(TextWeight::Semibold),
                        )
                        .content(
                            Text::new("No optimization results yet. Run optimization first.")
                                .size(TextSize::Xs)
                                .color(theme.text_muted),
                        ),
                )
                .into_any_element();
        }

        // Clamp selected index to valid range
        let idx = selected_idx.min(channel_results.len().saturating_sub(1));
        let result = &channel_results[idx];

        // Use the backend's effective target curve (mean_spl + tilt) when available.
        // This shows what the optimizer actually aimed for instead of a misleading 0dB line.
        // Falls back to the UI-generated target curve if the backend didn't provide one.
        let target_curve_data = if result.target_curve.is_some() {
            result.target_curve.clone()
        } else if room_eq.optimizer_config.target_curve == "custom" {
            Some(room_eq.custom_target_curve.generate_curve())
        } else if room_eq.optimizer_config.target_curve == "flat" {
            Some(CustomTargetCurve::new_flat().generate_curve())
        } else {
            None
        };

        render_channel_result_card(
            result,
            &theme,
            smoothing_octaves,
            y_axis_auto,
            chart_state,
            target_curve_data.as_deref(),
        )
        .into_any_element()
    }
}
