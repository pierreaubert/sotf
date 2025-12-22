use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, StackSpacing, Text, TextSize, TextWeight, VStack,
};

// === Free functions for channel configuration UI ===

/// Render a single channel configuration row
pub(crate) fn render_channel_config_row(
    idx: usize,
    config: &crate::app::types::RoomEqSpeakerConfig,
    theme: &crate::theme::Theme,
    view: &Entity<PlayerView>,
) -> impl IntoElement {
    use crate::app::types::SpeakerConfigType;

    let channel_name = config.channel_name.clone();
    let is_multi = config.config_type == SpeakerConfigType::MultiDriver;
    let crossover_type = config.crossover_type;

    div()
        .flex()
        .gap_4()
        .items_center()
        .w_full()
        .p_3()
        .bg(theme.surface)
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
        // Channel name
        .child(
            div().w(px(80.0)).child(
                Text::new(channel_name)
                    .weight(TextWeight::Bold)
                    .color(theme.text_primary),
            ),
        )
        // Speaker type toggle
        .child(
            div()
                .flex()
                .gap_2()
                .items_center()
                .child(
                    Text::new("Type:")
                        .size(TextSize::Sm)
                        .color(theme.text_secondary),
                )
                .child(
                    Button::new(SharedString::from(format!("single-{}", idx)), "Single")
                        .variant(if !is_multi {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(ButtonSize::Sm)
                        .theme(theme.to_button_theme())
                        .on_click({
                            let view = view.clone();
                            move |_, cx| {
                                view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        if let Some(cfg) =
                                            state.app.room_eq_state.speaker_configs.get_mut(idx)
                                        {
                                            cfg.config_type = SpeakerConfigType::Single;
                                        }
                                    });
                                    cx.notify();
                                });
                            }
                        }),
                )
                .child(
                    Button::new(SharedString::from(format!("multi-{}", idx)), "Multi-Driver")
                        .variant(if is_multi {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(ButtonSize::Sm)
                        .theme(theme.to_button_theme())
                        .on_click({
                            let view = view.clone();
                            move |_, cx| {
                                view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        if let Some(cfg) =
                                            state.app.room_eq_state.speaker_configs.get_mut(idx)
                                        {
                                            cfg.config_type = SpeakerConfigType::MultiDriver;
                                        }
                                    });
                                    cx.notify();
                                });
                            }
                        }),
                ),
        )
        // Crossover type selector (only shown for multi-driver)
        .when(is_multi, |el| {
            el.child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Text::new("Crossover:")
                            .size(TextSize::Sm)
                            .color(theme.text_secondary),
                    )
                    .child(render_crossover_dropdown(idx, crossover_type, view)),
            )
        })
}

/// Render crossover type dropdown as a cycling button
fn render_crossover_dropdown(
    channel_idx: usize,
    current: crate::app::types::CrossoverType,
    view: &Entity<PlayerView>,
) -> impl IntoElement {
    use crate::app::types::CrossoverType;

    let crossover_types = CrossoverType::all();
    let current_label = current.as_str();

    Button::new(
        SharedString::from(format!("xover-{}", channel_idx)),
        current_label,
    )
    .variant(ButtonVariant::Secondary)
    .size(ButtonSize::Sm)
    .on_click({
        let view = view.clone();
        let crossover_types = crossover_types.to_vec();
        move |_, cx| {
            view.update(cx, |this, cx| {
                this.state.update(cx, |state, _| {
                    if let Some(cfg) = state.app.room_eq_state.speaker_configs.get_mut(channel_idx)
                    {
                        // Find current index and cycle to next
                        let current_idx = crossover_types
                            .iter()
                            .position(|&ct| ct == cfg.crossover_type)
                            .unwrap_or(0);
                        let next_idx = (current_idx + 1) % crossover_types.len();
                        cfg.crossover_type = crossover_types[next_idx];
                    }
                });
                cx.notify();
            });
        }
    })
}

// === Review Step UI Free Functions ===

/// Render a single channel result card with plots and filter details
pub(crate) fn render_channel_result_card(
    result: &crate::app::types::ChannelOptResult,
    theme: &crate::theme::Theme,
) -> impl IntoElement {
    use crate::components::graphs::format_frequency;

    let channel_name = result.channel_name.clone();
    let score_improvement = result.pre_score - result.post_score;
    let has_response_data =
        result.original_response.is_some() && result.corrected_response.is_some();

    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .bg(theme.surface)
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
        // Header with channel name and scores
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    Text::new(channel_name)
                        .weight(TextWeight::Bold)
                        .size(TextSize::Lg)
                        .color(theme.text_primary),
                )
                .child(
                    div()
                        .flex()
                        .gap_4()
                        .child(
                            Text::new(format!("Before: {:.2}", result.pre_score))
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                        )
                        .child(
                            Text::new(format!("After: {:.2}", result.post_score))
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                        )
                        .child(
                            Text::new(format!("{:+.2}", score_improvement))
                                .weight(TextWeight::Bold)
                                .color(if score_improvement > 0.0 {
                                    theme.success
                                } else {
                                    theme.error
                                }),
                        ),
                ),
        )
        // Frequency response plot (if available)
        .when(has_response_data, |div| {
            let original = result.original_response.as_ref().unwrap();
            let corrected = result.corrected_response.as_ref().unwrap();
            div.child(render_response_comparison_graph(original, corrected, theme))
        })
        // EQ Filter details
        .child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Text::new("EQ Filters")
                        .weight(TextWeight::Semibold)
                        .size(TextSize::Sm)
                        .color(theme.text_primary),
                )
                .child(render_filter_table(&result.eq_filters, theme)),
        )
        // Crossover info (if multi-driver)
        .when(result.crossover_freqs.is_some(), |el| {
            let xover_freqs = result.crossover_freqs.as_ref().unwrap();
            el.child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new("Crossover Frequencies")
                            .weight(TextWeight::Semibold)
                            .size(TextSize::Sm)
                            .color(theme.text_primary),
                    )
                    .child(
                        gpui::div()
                            .flex()
                            .gap_2()
                            .children(xover_freqs.iter().map(|f| {
                                gpui::div()
                                    .px_2()
                                    .py_1()
                                    .bg(theme.background_secondary)
                                    .rounded_md()
                                    .child(
                                        Text::new(format_frequency(*f))
                                            .size(TextSize::Sm)
                                            .color(theme.text_primary),
                                    )
                            })),
                    ),
            )
        })
}

/// Render the frequency response comparison graph
fn render_response_comparison_graph(
    original: &[(f64, f64)],
    corrected: &[(f64, f64)],
    theme: &crate::theme::Theme,
) -> impl IntoElement {
    use d3rs::color::D3Color;
    use d3rs::scale::{LinearScale, LogScale, Scale};
    use d3rs::shape::{LineConfig, LinePoint, render_line};

    const GRAPH_WIDTH: f32 = 400.0;
    const GRAPH_HEIGHT: f32 = 150.0;
    const Y_AXIS_WIDTH: f32 = 32.0;
    const X_AXIS_HEIGHT: f32 = 16.0;
    const MIN_FREQ: f64 = 20.0;
    const MAX_FREQ: f64 = 20000.0;

    // Calculate dB range
    let all_values: Vec<f64> = original
        .iter()
        .chain(corrected.iter())
        .map(|(_, db)| *db)
        .collect();
    let min_db = all_values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min)
        .max(-24.0);
    let max_db = all_values
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max)
        .min(24.0);

    // Add padding
    let range = max_db - min_db;
    let padding = range * 0.1;
    let min_db = ((min_db - padding) / 6.0).floor() * 6.0;
    let max_db = ((max_db + padding) / 6.0).ceil() * 6.0;

    let freq_scale = LogScale::new()
        .domain(MIN_FREQ, MAX_FREQ)
        .range(0.0, GRAPH_WIDTH as f64);
    let db_scale = LinearScale::new()
        .domain(min_db, max_db)
        .range(GRAPH_HEIGHT as f64, 0.0);

    // Create line points
    let original_points: Vec<LinePoint> = original
        .iter()
        .map(|(f, db)| LinePoint::new(*f, *db))
        .collect();
    let corrected_points: Vec<LinePoint> = corrected
        .iter()
        .map(|(f, db)| LinePoint::new(*f, *db))
        .collect();

    let original_config = LineConfig::new()
        .stroke_width(1.5)
        .stroke_color(D3Color::from_rgba(theme.text_muted));
    let corrected_config = LineConfig::new()
        .stroke_width(2.0)
        .stroke_color(D3Color::from_rgba(theme.info));

    let original_line = render_line(&freq_scale, &db_scale, &original_points, &original_config);
    let corrected_line = render_line(&freq_scale, &db_scale, &corrected_points, &corrected_config);

    div()
        .w(px(GRAPH_WIDTH + Y_AXIS_WIDTH))
        .h(px(GRAPH_HEIGHT + X_AXIS_HEIGHT + 24.0))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                // Graph area
                .child(
                    div()
                        .w(px(GRAPH_WIDTH))
                        .h(px(GRAPH_HEIGHT))
                        .bg(theme.background)
                        .rounded_md()
                        .border_1()
                        .border_color(theme.border)
                        .relative()
                        .overflow_hidden()
                        // Zero line
                        .when(min_db <= 0.0 && max_db >= 0.0, |el| {
                            let zero_y = db_scale.scale(0.0) as f32;
                            el.child(
                                div()
                                    .absolute()
                                    .top(px(zero_y))
                                    .left_0()
                                    .right_0()
                                    .h(px(1.0))
                                    .bg(theme.text_muted)
                                    .opacity(0.3),
                            )
                        })
                        .child(original_line)
                        .child(corrected_line),
                ),
        )
        // Legend
        .child(
            div()
                .flex()
                .gap_4()
                .justify_center()
                .pt_2()
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .items_center()
                        .child(div().w(px(12.0)).h(px(2.0)).bg(theme.text_muted))
                        .child(
                            Text::new("Original")
                                .size(TextSize::Xs)
                                .color(theme.text_muted),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .items_center()
                        .child(div().w(px(12.0)).h(px(2.0)).bg(theme.info))
                        .child(
                            Text::new("Corrected")
                                .size(TextSize::Xs)
                                .color(theme.text_muted),
                        ),
                ),
        )
}

/// Render the EQ filter table
fn render_filter_table(
    filters: &[crate::app::types::EqFilterConfig],
    theme: &crate::theme::Theme,
) -> impl IntoElement {
    use crate::components::graphs::format_frequency;

    if filters.is_empty() {
        return div()
            .child(
                Text::new("No filters")
                    .size(TextSize::Sm)
                    .color(theme.text_muted),
            )
            .into_any_element();
    }

    div()
        .flex()
        .flex_wrap()
        .gap_2()
        .children(filters.iter().enumerate().map(|(i, f)| {
            let gain_color = if f.gain_db > 0.5 {
                theme.success
            } else if f.gain_db < -0.5 {
                theme.error
            } else {
                theme.text_muted
            };

            div()
                .px_3()
                .py_2()
                .bg(theme.background_secondary)
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .flex()
                .flex_col()
                .gap_1()
                .min_w(px(80.0))
                // Filter number and type
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .items_center()
                        .child(
                            Text::new(format!("{}", i + 1))
                                .weight(TextWeight::Bold)
                                .size(TextSize::Xs)
                                .color(theme.text_primary),
                        )
                        .child(
                            Text::new(&f.filter_type)
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        ),
                )
                // Frequency
                .child(
                    Text::new(format_frequency(f.frequency))
                        .weight(TextWeight::Semibold)
                        .size(TextSize::Sm)
                        .color(theme.text_primary),
                )
                // Gain and Q
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            Text::new(format!("{:+.1}dB", f.gain_db))
                                .weight(TextWeight::Bold)
                                .size(TextSize::Sm)
                                .color(gain_color),
                        )
                        .child(
                            Text::new(format!("Q:{:.1}", f.q))
                                .size(TextSize::Xs)
                                .color(theme.text_muted),
                        ),
                )
        }))
        .into_any_element()
}
