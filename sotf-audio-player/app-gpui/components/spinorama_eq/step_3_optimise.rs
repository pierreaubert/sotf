use crate::ui::PlayerView;
use d3rs::prelude::{render_line, CurveType, D3Color, LineConfig, LinePoint, LinearScale};
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, Progress, ProgressSize, StackSpacing, Text,
    TextSize, TextWeight, VStack,
};

impl PlayerView {

    // ========================================================================
    // Step 3: Optimize
    // ========================================================================

    pub(crate) fn render_spinorama_optimize(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let spinorama = &state.app.spinorama_eq_state;

        let progress = spinorama.progress;
        let status_msg = spinorama.status_message.clone();
        let error_msg = spinorama.error_message.clone();
        let is_optimizing = spinorama.is_optimizing();
        let selected_speaker = spinorama.selected_speaker.clone().unwrap_or_default();
        let mode = spinorama.optimizer_config.mode;
        let progress_history = spinorama.progress_history.clone();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Run Optimization")
                    .color(theme.text_primary)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Generate optimized EQ filters for your speaker.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("Configuration Summary")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(
                        VStack::new().spacing(StackSpacing::Sm).child(
                            HStack::new()
                                .spacing(StackSpacing::Lg)
                                .child(
                                    VStack::new()
                                        .spacing(StackSpacing::Xs)
                                        .child(
                                            Text::new("Speaker")
                                                .size(TextSize::Xs)
                                                .color(theme.text_muted),
                                        )
                                        .child(
                                            Text::new(selected_speaker)
                                                .weight(TextWeight::Bold)
                                                .color(theme.accent),
                                        ),
                                )
                                .child(
                                    VStack::new()
                                        .spacing(StackSpacing::Xs)
                                        .child(
                                            Text::new("Mode")
                                                .size(TextSize::Xs)
                                                .color(theme.text_muted),
                                        )
                                        .child(
                                            Text::new(mode.as_str())
                                                .color(theme.text_primary)
                                                .weight(TextWeight::Bold),
                                        ),
                                )
                                .child(
                                    VStack::new()
                                        .spacing(StackSpacing::Xs)
                                        .child(
                                            Text::new("Filters")
                                                .size(TextSize::Xs)
                                                .color(theme.text_muted),
                                        )
                                        .child(
                                            Text::new(format!(
                                                "{}",
                                                spinorama.optimizer_config.num_filters
                                            ))
                                            .color(theme.text_primary)
                                            .weight(TextWeight::Bold),
                                        ),
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
                        Text::new("Generate Speaker EQ")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Button::new(
                                    "start_spinorama_optimization",
                                    if is_optimizing {
                                        "Optimizing..."
                                    } else {
                                        "Generate Speaker EQ"
                                    },
                                )
                                .variant(ButtonVariant::Primary)
                                .size(ButtonSize::Lg)
                                .full_width(true)
                                .disabled(is_optimizing)
                                .build()
                                .when(!is_optimizing, |btn| {
                                    btn.on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            view.start_spinorama_optimization(cx);
                                        }),
                                    )
                                }),
                            )
                            .when(is_optimizing || progress > 0.0, |vstack| {
                                vstack
                                    .child(
                                        Text::new(format!("Progress: {:.0}%", progress * 100.0))
                                            .size(TextSize::Sm)
                                            .color(theme.text_primary),
                                    )
                                    .child(Progress::new(progress * 100.0).size(ProgressSize::Md))
                                    .child(
                                        Text::new(status_msg)
                                            .size(TextSize::Sm)
                                            .color(theme.text_secondary),
                                    )
                            })
                            .when_some(error_msg, |vstack, err| {
                                vstack.child(Text::new(err).size(TextSize::Sm).color(theme.error))
                            }),
                    ),
            )
            // Loss vs Iterations graph
            .when(!progress_history.is_empty(), |vstack| {
                let theme = theme.clone();
                let history = progress_history.clone();

                // Calculate bounds for the graph
                let max_iter = history.iter().map(|(i, _)| *i).max().unwrap_or(1) as f64;
                let min_loss = history.iter().map(|(_, l)| *l).fold(f64::INFINITY, f64::min);
                let max_loss = history.iter().map(|(_, l)| *l).fold(f64::NEG_INFINITY, f64::max);

                // Add some padding to the loss range
                let loss_range = (max_loss - min_loss).max(0.1);
                let loss_padding = loss_range * 0.1;
                let y_min = min_loss - loss_padding;
                let y_max = max_loss + loss_padding;

                // Get current loss for display
                let current_loss = history.last().map(|(_, l)| *l).unwrap_or(0.0);
                let best_loss = min_loss;

                vstack.child(
                    Card::new()
                        .background(theme.surface)
                        .header_background(theme.background_secondary)
                        .border(theme.border)
                        .header(
                            HStack::new()
                                .spacing(StackSpacing::Lg)
                                .child(
                                    Text::new("Optimization Progress")
                                        .color(theme.text_primary)
                                        .weight(TextWeight::Semibold),
                                )
                                .child(
                                    Text::new(format!("Current: {:.4}", current_loss))
                                        .size(TextSize::Sm)
                                        .color(theme.text_secondary),
                                )
                                .child(
                                    Text::new(format!("Best: {:.4}", best_loss))
                                        .size(TextSize::Sm)
                                        .color(theme.success),
                                ),
                        )
                        .content(
                            div()
                                .h(px(200.0))
                                .w_full()
                                .relative()
                                .bg(theme.background)
                                .rounded_md()
                                .overflow_hidden()
                                .child({
                                    // Create scales
                                    let x_scale = LinearScale::new()
                                        .domain(0.0, max_iter)
                                        .range(0.0, 1.0);
                                    let y_scale = LinearScale::new()
                                        .domain(y_min, y_max)
                                        .range(1.0, 0.0); // Inverted for screen coords

                                    // Convert history to line points
                                    let line_points: Vec<LinePoint> = history
                                        .iter()
                                        .map(|(iter, loss)| LinePoint::new(*iter as f64, *loss))
                                        .collect();

                                    // Configure line style
                                    let line_config = LineConfig::new()
                                        .stroke_color(D3Color::from_hex(0x4CAF50)) // Green
                                        .stroke_width(2.0)
                                        .curve(CurveType::Linear)
                                        .show_points(false);

                                    render_line(&x_scale, &y_scale, &line_points, &line_config)
                                })
                                // Y-axis labels
                                .child(
                                    div()
                                        .absolute()
                                        .left_1()
                                        .top_1()
                                        .child(
                                            Text::new(format!("{:.3}", y_max))
                                                .size(TextSize::Xs)
                                                .color(theme.text_muted),
                                        ),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .left_1()
                                        .bottom_1()
                                        .child(
                                            Text::new(format!("{:.3}", y_min))
                                                .size(TextSize::Xs)
                                                .color(theme.text_muted),
                                        ),
                                )
                                // X-axis labels
                                .child(
                                    div()
                                        .absolute()
                                        .right_1()
                                        .bottom_1()
                                        .child(
                                            Text::new(format!("{}", max_iter as usize))
                                                .size(TextSize::Xs)
                                                .color(theme.text_muted),
                                        ),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .left(px(50.0))
                                        .bottom_1()
                                        .child(
                                            Text::new("Iterations →")
                                                .size(TextSize::Xs)
                                                .color(theme.text_muted),
                                        ),
                                ),
                        ),
                )
            })
    }

}
