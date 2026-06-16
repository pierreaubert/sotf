use super::misc::result_to_spinorama_curves;
use crate::app::types::SpinoramaCurves;
use crate::components::design::Ds;
use crate::components::graphs::common::{
    colors, render_empty_state, rgba_to_u32, theme_to_chart_theme,
};
use crate::components::icons::IconName;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_px::{BarTheme, LegendPosition, ScaleType, bar, line};
use sotf_audio_player::autoeq::SpeakerOptimizationResult;

impl PlayerView {
    /// Render Spinorama speaker optimization result graphs
    pub fn render_speaker_optimization_result_graphs(
        &self,
        d: &Ds,
        result: &SpeakerOptimizationResult,
        theme: &Theme,
        available_width: f32,
    ) -> impl IntoElement {
        let gap = 8.0;
        let graph_ratio = 0.75;
        let graph_width = ((available_width - gap) / 2.0).max(600.0);
        let legend_width = 150.0;
        let graph_height = 300.0_f32.max((graph_width - legend_width) * graph_ratio);

        div()
            .flex()
            .flex_col()
            .w_full()
            .gap(d.section)
            // Row 1: CEA2034 with and without EQ
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(d.section)
                    .child(render_cea2034_from_result(
                        result,
                        theme,
                        graph_width,
                        graph_height,
                        false,
                    ))
                    .child(render_cea2034_from_result(
                        result,
                        theme,
                        graph_width,
                        graph_height,
                        true,
                    )),
            )
            // Row 2: Main Response | Filter Response
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(d.section)
                    .child(render_spinorama_main_response_plot(
                        result,
                        theme,
                        graph_width,
                        graph_height,
                    ))
                    .child(render_speaker_filter_response_plot(
                        result,
                        theme,
                        graph_width,
                        graph_height,
                    )),
            )
            // Row 4: Tonal Balance ON | Tonal Balance LW
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(d.section)
                    .child(render_tonal_balance_plot(
                        d,
                        result,
                        "ON",
                        theme,
                        graph_width,
                        graph_height,
                    ))
                    .child(render_tonal_balance_plot(
                        d,
                        result,
                        "LW",
                        theme,
                        graph_width,
                        graph_height,
                    )),
            )
            // Row 5: Tonal Balance ER | Tonal Balance SP
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(d.section)
                    .child(render_tonal_balance_plot(
                        d,
                        result,
                        "ER",
                        theme,
                        graph_width,
                        graph_height,
                    ))
                    .child(render_tonal_balance_plot(
                        d,
                        result,
                        "SP",
                        theme,
                        graph_width,
                        graph_height,
                    )),
            )
    }
}

/// Render Main Response Plot (Input, Target, Corrected) using gpui-px
fn render_spinorama_main_response_plot(
    result: &SpeakerOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let chart_theme = theme_to_chart_theme(theme);

    // Use the LW curve as "Original" for consistency with CEA2034 plot above.
    // Fall back to input_curve if LW is empty (e.g., headphone mode).
    let original = if !result.lw_curve.is_empty() {
        &result.lw_curve
    } else {
        &result.input_curve
    };

    // Corrected = original + filter response
    let corrected: Vec<f64> = original
        .iter()
        .zip(result.filter_response.iter())
        .map(|(a, b)| a + b)
        .collect();

    let chart = line(&result.frequencies, original)
        .x_scale(ScaleType::Log)
        .y_label("SPL (dB)")
        .y_range(-15.0, 5.0)
        .label("Original")
        .color(rgba_to_u32(colors::input(theme)))
        .stroke_width(1.5)
        .theme(chart_theme)
        .size(width, height)
        .legend_position(LegendPosition::Bottom)
        .add_series(
            &result.target_curve,
            Some("Target"),
            rgba_to_u32(colors::target(theme)),
            2.0,
            1.0,
        )
        .add_series(
            &corrected,
            Some("Corrected"),
            rgba_to_u32(colors::corrected(theme)),
            2.0,
            1.0,
        )
        .build();

    div()
        .w(px(width))
        .flex()
        .flex_col()
        .when_some(chart.ok(), |el, c| el.child(c))
}

/// Render Filter Response Plot using gpui-px with individual filter curves
fn render_speaker_filter_response_plot(
    result: &SpeakerOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let chart_theme = theme_to_chart_theme(theme);

    // Start with the total filter response (thicker line)
    let mut chart_builder = line(&result.frequencies, &result.filter_response)
        .x_scale(ScaleType::Log)
        .y_label("dB")
        .label("Total")
        .legend_position(LegendPosition::Bottom)
        .color(rgba_to_u32(colors::filter(theme)))
        .stroke_width(2.5)
        .theme(chart_theme)
        .size(width, height);

    // Add individual filter responses if available
    if !result.individual_filter_responses.is_empty() {
        for (i, filter_response) in result.individual_filter_responses.iter().enumerate() {
            // Use band colors from theme, cycling through them
            let color = if i < theme.plugin_palette.band_colors.len() {
                rgba_to_u32(theme.plugin_palette.band_colors[i])
            } else {
                rgba_to_u32(
                    theme.plugin_palette.band_colors[i % theme.plugin_palette.band_colors.len()],
                )
            };

            chart_builder = chart_builder.add_series(
                filter_response,
                Some(&format!("F{}", i + 1)),
                color,
                1.2,
                0.7,
            );
        }
    }

    let chart = chart_builder.build();

    div()
        .w(px(width))
        .flex()
        .flex_col()
        .when_some(chart.ok(), |el, c| el.child(c))
}

/// Render measurement preview graph showing On Axis measurement and optional target curve
/// Used in spinorama_eq Step 1 (Select Speaker) for previewing the measurement before optimization
pub fn render_speaker_preview_graph(
    frequencies: &[f64],
    on_axis_curve: &[f64],
    target_curve: Option<&[f64]>,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let chart_theme = theme_to_chart_theme(theme);

    let mut chart_builder = line(frequencies, on_axis_curve)
        .x_scale(ScaleType::Log)
        .label("On Axis")
        .color(rgba_to_u32(colors::input(theme)))
        .stroke_width(2.0)
        .theme(chart_theme)
        .size(width, height);

    // Add target curve if available
    if let Some(target) = target_curve
        && !target.is_empty()
    {
        chart_builder = chart_builder.add_series(
            target,
            Some("Target"),
            rgba_to_u32(colors::target(theme)),
            1.5,
            0.8,
        );
    }

    let chart = chart_builder.build();

    div()
        .w(px(width))
        .flex()
        .flex_col()
        .when_some(chart.ok(), |el, c| el.child(c))
}

/// Render CEA2034 Spinorama plot with dual y-axis
/// Left axis: ON (On Axis), LW (Listening Window), ER (Early Reflections), SP (Sound Power)
/// Right axis: ERDI (Early Reflections DI), SPDI (Sound Power DI)
pub fn render_spinorama_cea2034_graph(
    curves: &SpinoramaCurves,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    if !curves.is_valid() {
        return div().child(render_empty_state(
            IconName::AudioWaveform,
            "No data available",
            theme,
        ));
    }

    let chart_theme = theme_to_chart_theme(theme);

    // CEA2034 standard colors (matching spinorama.org)
    const ON_AXIS_COLOR: u32 = 0x1f77b4; // Blue
    const LISTENING_WINDOW_COLOR: u32 = 0xff7f0e; // Orange
    const EARLY_REFLECTIONS_COLOR: u32 = 0x2ca02c; // Green
    const SOUND_POWER_COLOR: u32 = 0xd62728; // Red
    const ERDI_COLOR: u32 = 0x9467bd; // Purple (dashed style would be nice)
    const SPDI_COLOR: u32 = 0x8c564b; // Brown (dashed style would be nice)

    let chart = line(&curves.frequencies, &curves.on_axis)
        .x_scale(ScaleType::Log)
        .x_range(20.0, 20000.0)
        .y_label("SPL (dB)")
        .y_range(-40.0, 10.0)
        .y2_range(0.0, 50.0)
        .y2_label("DI (dB)")
        .label("ON")
        .color(ON_AXIS_COLOR)
        .stroke_width(2.0)
        .theme(chart_theme)
        .size(width, height)
        .legend_position(LegendPosition::Bottom)
        // Primary axis series (SPL curves)
        .add_series(
            &curves.listening_window,
            Some("LW"),
            LISTENING_WINDOW_COLOR,
            2.0,
            1.0,
        )
        .add_series(
            &curves.early_reflections,
            Some("ER"),
            EARLY_REFLECTIONS_COLOR,
            1.5,
            0.8,
        )
        .add_series(&curves.sound_power, Some("SP"), SOUND_POWER_COLOR, 1.5, 0.8)
        // Secondary axis series (DI curves)
        .add_series_y2(
            &curves.early_reflections_di,
            Some("ER DI"),
            ERDI_COLOR,
            1.5,
            0.9,
        )
        .add_series_y2(&curves.sound_power_di, Some("SP DI"), SPDI_COLOR, 1.5, 0.9)
        .build();

    div()
        .w(px(width))
        .flex()
        .flex_col()
        .when_some(chart.ok(), |el, c| el.child(c))
}

/// Render PIR (Estimated In-Room Response) graph
pub fn render_spinorama_pir_graph(
    curves: &SpinoramaCurves,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    if !curves.has_pir() || curves.frequencies.is_empty() {
        return div().child("PIR data not available");
    }

    let chart_theme = theme_to_chart_theme(theme);

    const PIR_COLOR: u32 = 0x9467bd; // Purple

    let chart = line(&curves.frequencies, &curves.estimated_in_room)
        .x_scale(ScaleType::Log)
        .x_range(20.0, 20000.0)
        .y_label("SPL (dB)")
        .y_range(-40.0, 10.0)
        .label("PIR")
        .color(PIR_COLOR)
        .stroke_width(2.0)
        .theme(chart_theme)
        .size(width, height)
        .legend_position(LegendPosition::Bottom)
        .build();

    div()
        .w(px(width))
        .flex()
        .flex_col()
        .when_some(chart.ok(), |el, c| el.child(c))
}

/// Render horizontal directivity (SPL Horizontal) graph showing multiple angle curves
pub fn render_spinorama_horizontal_graph(
    curves: &SpinoramaCurves,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    if !curves.has_horizontal() {
        return div().child("Horizontal directivity data not available");
    }

    let chart_theme = theme_to_chart_theme(theme);

    // Color palette for different angles
    let angle_colors: Vec<u32> = vec![
        0x1f77b4, // Blue (0°)
        0xff7f0e, // Orange
        0x2ca02c, // Green
        0xd62728, // Red
        0x9467bd, // Purple
        0x8c564b, // Brown
        0xe377c2, // Pink
        0x7f7f7f, // Gray
        0xbcbd22, // Yellow-green
        0x17becf, // Cyan
    ];

    // Standard horizontal angles for spinorama display
    let display_angles: &[f64] = &[0.0, 10.0, -10.0, 20.0, -20.0, 30.0, -30.0];

    // Filter and sort curves to standard display angles
    let mut sorted_curves: Vec<_> = curves
        .horizontal_directivity
        .iter()
        .filter(|c| display_angles.iter().any(|a| (c.angle - a).abs() < 0.5))
        .collect();
    sorted_curves.sort_by(|a, b| {
        a.angle
            .partial_cmp(&b.angle)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Use On Axis (0°) if available, otherwise first curve
    let base_curve = sorted_curves
        .iter()
        .find(|c| (c.angle - 0.0).abs() < 0.5)
        .or(sorted_curves.first());

    let Some(base) = base_curve else {
        return div().child("No horizontal curves found");
    };

    let mut chart_builder = line(&base.frequencies, &base.spl)
        .x_scale(ScaleType::Log)
        .x_range(20.0, 20000.0)
        .y_label("SPL (dB)")
        .y_range(-40.0, 10.0)
        .label(format!("{:.0}°", base.angle))
        .legend_position(LegendPosition::Bottom)
        .color(angle_colors[0])
        .stroke_width(2.0)
        .theme(chart_theme)
        .size(width, height);

    // Add other curves
    for (i, curve) in sorted_curves.iter().enumerate().skip(1) {
        if i < angle_colors.len() {
            chart_builder = chart_builder.add_series(
                &curve.spl,
                Some(&format!("{:.0}°", curve.angle)),
                angle_colors[i % angle_colors.len()],
                1.5,
                0.8,
            );
        }
    }

    let chart = chart_builder.build();

    div()
        .w(px(width))
        .flex()
        .flex_col()
        .when_some(chart.ok(), |el, c| el.child(c))
}

/// Render vertical directivity (SPL Vertical) graph showing multiple angle curves
pub fn render_spinorama_vertical_graph(
    curves: &SpinoramaCurves,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    if !curves.has_vertical() {
        return div().child("Vertical directivity data not available");
    }

    let chart_theme = theme_to_chart_theme(theme);

    // Color palette for different angles
    let angle_colors: Vec<u32> = vec![
        0x1f77b4, // Blue (0°)
        0xff7f0e, // Orange
        0x2ca02c, // Green
        0xd62728, // Red
        0x9467bd, // Purple
        0x8c564b, // Brown
        0xe377c2, // Pink
        0x7f7f7f, // Gray
        0xbcbd22, // Yellow-green
        0x17becf, // Cyan
    ];

    // Standard vertical angles for spinorama display
    let display_angles: &[f64] = &[0.0, 10.0, -10.0, 20.0, -20.0, 30.0, -30.0];

    // Filter and sort curves to standard display angles
    let mut sorted_curves: Vec<_> = curves
        .vertical_directivity
        .iter()
        .filter(|c| display_angles.iter().any(|a| (c.angle - a).abs() < 0.5))
        .collect();
    sorted_curves.sort_by(|a, b| {
        a.angle
            .partial_cmp(&b.angle)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Use On Axis (0°) if available, otherwise first curve
    let base_curve = sorted_curves
        .iter()
        .find(|c| (c.angle - 0.0).abs() < 0.5)
        .or(sorted_curves.first());

    let Some(base) = base_curve else {
        return div().child("No vertical curves found");
    };

    let mut chart_builder = line(&base.frequencies, &base.spl)
        .x_scale(ScaleType::Log)
        .x_range(20.0, 20000.0)
        .y_label("SPL (dB)")
        .y_range(-40.0, 10.0)
        .label(format!("{:.0}°", base.angle))
        .color(angle_colors[0])
        .stroke_width(2.0)
        .theme(chart_theme)
        .legend_position(LegendPosition::Bottom)
        .size(width, height);

    // Add other curves
    for (i, curve) in sorted_curves.iter().enumerate().skip(1) {
        if i < angle_colors.len() {
            chart_builder = chart_builder.add_series(
                &curve.spl,
                Some(&format!("{:.0}°", curve.angle)),
                angle_colors[i % angle_colors.len()],
                1.5,
                0.8,
            );
        }
    }

    let chart = chart_builder.build();

    div()
        .w(px(width))
        .flex()
        .flex_col()
        .when_some(chart.ok(), |el, c| el.child(c))
}

/// Render CEA2034 (Original or Corrected) - reuses render_spinorama_cea2034_graph
fn render_cea2034_from_result(
    result: &SpeakerOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
    corrected: bool,
) -> Div {
    let curves = result_to_spinorama_curves(result, corrected);
    render_spinorama_cea2034_graph(&curves, theme, width, height)
}

/// Render Tonal Balance Trend Lines (Before vs After)
fn render_tonal_balance_plot(
    d: &Ds,
    result: &SpeakerOptimizationResult,
    curve_type: &str,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let chart_theme = theme_to_chart_theme(theme);

    let apply_filter = |curve: &[f64]| -> Vec<f64> {
        curve
            .iter()
            .zip(result.filter_response.iter())
            .map(|(a, b)| a + b)
            .collect()
    };

    // Select curve - use actual On Axis curve for "ON", not input_curve
    let (original_curve, corrected_curve) = match curve_type {
        "ON" => (
            result.on_axis_curve.clone(),
            apply_filter(&result.on_axis_curve),
        ),
        "LW" => (result.lw_curve.clone(), apply_filter(&result.lw_curve)),
        "ER" => (result.er_curve.clone(), apply_filter(&result.er_curve)),
        "SP" => (result.sp_curve.clone(), apply_filter(&result.sp_curve)),
        _ => (vec![], vec![]),
    };

    if original_curve.is_empty() {
        return div().child(render_empty_state(
            IconName::AudioWaveform,
            "No data",
            theme,
        ));
    }

    // Manual linear regression helper
    let calculate_trend = |freqs: &[f64], values: &[f64]| -> Option<(f64, f64)> {
        let min_freq = 100.0;
        let max_freq = 10000.0;
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_xy = 0.0;
        let mut sum_xx = 0.0;
        let mut count = 0.0;

        for (i, &f) in freqs.iter().enumerate() {
            if f >= min_freq
                && f <= max_freq
                && let Some(&y) = values.get(i)
            {
                let x = f.log10();
                sum_x += x;
                sum_y += y;
                sum_xy += x * y;
                sum_xx += x * x;
                count += 1.0;
            }
        }

        if count < 2.0 {
            return None;
        }

        let mean_x = sum_x / count;
        let mean_y = sum_y / count;

        let denominator = sum_xx - count * mean_x * mean_x;
        if denominator.abs() < 1e-10 {
            return None;
        }

        let slope = (sum_xy - count * mean_x * mean_y) / denominator;
        let intercept = mean_y - slope * mean_x;

        Some((slope, intercept))
    };

    let orig_trend = calculate_trend(&result.frequencies, &original_curve);
    let corr_trend = calculate_trend(&result.frequencies, &corrected_curve);

    // CEA2034 standard colors for consistency
    const BLUE: u32 = 0x1f77b4;
    const ORANGE: u32 = 0xff7f0e;

    let mut chart_builder = line(&result.frequencies, &original_curve)
        .x_scale(ScaleType::Log)
        .x_range(20.0, 20000.0)
        .y_range(-15.0, 5.0)
        .label(format!("{} Orig", curve_type))
        .y_label("SPL (dB)")
        .legend_position(LegendPosition::Bottom)
        .color(BLUE)
        .stroke_width(2.0)
        .opacity(1.0)
        .theme(chart_theme)
        .size(width, height)
        .add_series(
            &corrected_curve,
            Some(&format!("{} EQ", curve_type)),
            ORANGE,
            1.0,
            0.5,
        );

    // Histogram calculation
    let mut hist_chart = None;
    if let (Some((slope_orig, int_orig)), Some((slope_corr, int_corr))) = (orig_trend, corr_trend) {
        let calculate_histogram =
            |freqs: &[f64], values: &[f64], slope: f64, intercept: f64| -> Vec<f64> {
                let min_freq = 100.0;
                let max_freq = 10000.0;
                // Bins: [0, 0.5), [0.5, 1.0), ... [3.5, 4.0), [4.0, inf)
                let mut bins = vec![0.0; 9];

                for (i, &f) in freqs.iter().enumerate() {
                    if f >= min_freq
                        && f <= max_freq
                        && let Some(&y) = values.get(i)
                    {
                        let trend_y = slope * f.log10() + intercept;
                        let deviation = (y - trend_y).abs();

                        let bin_idx = (deviation / 0.5).floor() as usize;
                        if bin_idx < 8 {
                            bins[bin_idx] += 1.0;
                        } else {
                            bins[8] += 1.0; // Overflow bin
                        }
                    }
                }
                bins
            };

        let hist_orig =
            calculate_histogram(&result.frequencies, &original_curve, slope_orig, int_orig);
        let hist_corr =
            calculate_histogram(&result.frequencies, &corrected_curve, slope_corr, int_corr);

        let labels = vec![
            "0-0.5", "0.5-1", "1-1.5", "1.5-2", "2-2.5", "2.5-3", "3-3.5", "3.5-4", ">4",
        ];

        let bar_theme = BarTheme {
            plot_background: theme.surface,
            title_color: theme.text_primary,
            legend_text_color: theme.text_secondary,
        };

        // Use a grouped bar chart
        let chart = bar(&labels, &hist_orig)
            .color(BLUE)
            .label(format!("{} Original", curve_type))
            .theme(bar_theme)
            .size(width, height / 2.0)
            .bar_gap(4.0)
            .opacity(0.8)
            .legend_position(LegendPosition::Bottom)
            .add_series(
                &hist_corr,
                Some(&format!("{} Corrected", curve_type)),
                ORANGE,
                0.8,
            );

        hist_chart = chart.build().ok();
    }

    if let Some((slope, intercept)) = orig_trend {
        let trend: Vec<f64> = result
            .frequencies
            .iter()
            .map(|f| slope * f.log10() + intercept)
            .collect();
        chart_builder = chart_builder.add_series(
            &trend,
            Some(&format!("{:.2} dB/oct", slope)),
            BLUE,
            2.5,
            1.0,
        );
    }

    if let Some((slope, intercept)) = corr_trend {
        let trend: Vec<f64> = result
            .frequencies
            .iter()
            .map(|f| slope * f.log10() + intercept)
            .collect();
        chart_builder = chart_builder.add_series(
            &trend,
            Some(&format!("{:.2} dB/oct", slope)),
            ORANGE,
            2.5,
            1.0,
        );
    }

    let chart = chart_builder.build();
    div()
        .w(px(width))
        .flex()
        .flex_col()
        .when_some(chart.ok(), |el, c| el.child(c))
        .gap(d.gap)
        .when_some(hist_chart, |el, c| el.child(c))
}
