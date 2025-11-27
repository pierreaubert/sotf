//! Common utilities for plugin UI components

use crate::theme::Theme;
use super::ticks::{TickConfig, render_tick_row};
use gpui::prelude::*;
use gpui::*;

/// Render a parameter row with name and value
pub fn render_param_row(
    name: &str,
    value: &str,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &Theme,
) -> impl IntoElement {
    let is_selected = selected_param == idx && is_editing;

    div()
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .py_2()
        .rounded_lg()
        .bg(if is_selected { theme.accent_muted } else { theme.surface })
        .border_l_4()
        .border_color(if is_selected { theme.accent } else { theme.surface })
        // Parameter name
        .child(
            div()
                .text_sm()
                .text_color(if is_selected { theme.text_primary } else { theme.text_secondary })
                .font_weight(if is_selected { FontWeight::MEDIUM } else { FontWeight::NORMAL })
                .child(name.to_string()),
        )
        // Value
        .child(
            div()
                .min_w(px(80.0))
                .px_2()
                .py_1()
                .rounded_md()
                .bg(if is_selected { theme.background } else { theme.background_secondary })
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_primary)
                .child(value.to_string()),
        )
}

/// Render a section header
pub fn render_section_header(title: &str, theme: &Theme) -> impl IntoElement {
    div()
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .text_color(theme.text_primary)
        .mb_2()
        .child(title.to_string())
}

/// Render a parameter section container
pub fn render_param_section(theme: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .rounded_xl()
        .bg(theme.background_secondary)
        .border_1()
        .border_color(theme.border)
        .p_3()
}

/// Render keyboard hints for edit mode
pub fn render_edit_hints(theme: &Theme) -> impl IntoElement {
    div()
        .mt_4()
        .p_3()
        .rounded_lg()
        .bg(theme.background_secondary)
        .border_1()
        .border_color(theme.border)
        .flex()
        .gap_4()
        .text_xs()
        .text_color(theme.text_muted)
        .child("↑/↓: Select")
        .child("←/→: Adjust")
        .child("[/]: Large step")
        .child("Enter: Done")
}

/// Render a meter/gauge visualization (simple, linear scale)
pub fn render_meter(
    value: f32, // 0.0 to 1.0
    label: &str,
    color: Rgba,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(theme.text_muted)
                .child(label.to_string()),
        )
        .child(
            div()
                .h(px(8.0))
                .w_full()
                .bg(theme.surface)
                .rounded_full()
                .overflow_hidden()
                .child(
                    div()
                        .h_full()
                        .w(relative(value.clamp(0.0, 1.0)))
                        .bg(color)
                        .rounded_full(),
                ),
        )
}

/// Configuration for horizontal meter styling
pub struct HorizontalMeterConfig {
    pub label_width: f32,
    pub value_width: f32,
    pub bar_height: f32,
    pub show_ticks: bool,
    pub show_legend: bool,
}

impl Default for HorizontalMeterConfig {
    fn default() -> Self {
        Self {
            label_width: 32.0,
            value_width: 50.0,
            bar_height: 20.0,
            show_ticks: true,
            show_legend: true,
        }
    }
}

/// Render a horizontal meter with proper scaling, ticks, and legend
///
/// Uses TickConfig for consistent bar fill, tick marks, and legend alignment.
/// The bar fill, ticks, and legend all use the same scale transformation.
pub fn render_horizontal_meter(
    label: &str,
    value: f64,
    value_format: &str, // e.g., "{:.1} dB" or "{:.0}%"
    tick_config: &TickConfig,
    bar_color: Rgba,
    meter_config: &HorizontalMeterConfig,
    theme: &Theme,
) -> impl IntoElement {
    let ratio = tick_config.value_to_position(value);
    let value_str = format_value(value, value_format);
    let gap = 4.0;

    div()
        .flex()
        .flex_col()
        .gap_1()
        // Bar row: [label] [bar] [value]
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(gap))
                // Label
                .child(
                    div()
                        .w(px(meter_config.label_width))
                        .text_xs()
                        .text_color(theme.text_secondary)
                        .child(label.to_string()),
                )
                // Bar with border
                .child(
                    div()
                        .flex_1()
                        .h(px(meter_config.bar_height))
                        .rounded(px(4.0))
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.background)
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .w(relative(ratio))
                                .bg(bar_color),
                        ),
                )
                // Value display
                .child(
                    div()
                        .w(px(meter_config.value_width))
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_primary)
                        .text_align(TextAlign::Right)
                        .child(value_str),
                ),
        )
        // Tick marks (if enabled)
        .when(meter_config.show_ticks, |d| {
            d.child(render_tick_row(
                tick_config,
                meter_config.label_width,
                meter_config.value_width,
            ))
        })
        // Legend (if enabled)
        .when(meter_config.show_legend, |d| {
            d.child(render_meter_legend(
                tick_config,
                meter_config.label_width,
                meter_config.value_width,
                theme,
            ))
        })
}

/// Render legend labels aligned with meter bar
fn render_meter_legend(
    tick_config: &TickConfig,
    label_width: f32,
    value_width: f32,
    theme: &Theme,
) -> impl IntoElement {
    let gap = 4.0;

    div()
        .flex()
        .gap(px(gap))
        // Label spacer
        .child(div().w(px(label_width)))
        // Legend area (flex-1, justify_between for labels)
        .child(
            div()
                .flex_1()
                .flex()
                .justify_between()
                .text_xs()
                .text_color(theme.text_muted)
                .children(tick_config.major_values.iter().map(|v| {
                    let label = format_legend_value(*v);
                    div().child(label)
                })),
        )
        // Value spacer
        .child(div().w(px(value_width)))
}

/// Format a value for display based on format string
fn format_value(value: f64, format: &str) -> String {
    // Simple format string parsing
    if format.contains("{:.1}") {
        format.replace("{:.1}", &format!("{:.1}", value))
    } else if format.contains("{:.0}") {
        format.replace("{:.0}", &format!("{:.0}", value))
    } else if format.contains("{:.2}") {
        format.replace("{:.2}", &format!("{:.2}", value))
    } else if format.contains("{:+.1}") {
        format.replace("{:+.1}", &format!("{:+.1}", value))
    } else {
        format!("{:.1}", value)
    }
}

/// Format legend value (handles positive values with + sign for dB scales)
fn format_legend_value(value: f64) -> String {
    if value > 0.0 {
        format!("+{}", value as i32)
    } else {
        format!("{}", value as i32)
    }
}

/// Render a toggle button with [OFF | ON] display
/// The active state is highlighted, inactive is dimmed
pub fn render_toggle(
    label: &str,
    enabled: bool,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &Theme,
) -> impl IntoElement {
    let is_selected = selected_param == idx && is_editing;

    div()
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .py_2()
        .rounded_lg()
        .bg(if is_selected { theme.accent_muted } else { theme.surface })
        .border_l_4()
        .border_color(if is_selected { theme.accent } else { theme.surface })
        // Label
        .child(
            div()
                .text_sm()
                .text_color(if is_selected { theme.text_primary } else { theme.text_secondary })
                .font_weight(if is_selected { FontWeight::MEDIUM } else { FontWeight::NORMAL })
                .child(label.to_string()),
        )
        // Toggle switch: [OFF | ON]
        .child(render_toggle_switch(enabled, theme))
}

/// Render just the toggle switch part: [OFF | ON]
/// Can be used standalone without label
pub fn render_toggle_switch(enabled: bool, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
        // OFF button
        .child(
            div()
                .px_2()
                .py_1()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .bg(if !enabled { theme.surface_hover } else { theme.background })
                .text_color(if !enabled { theme.text_primary } else { theme.text_muted })
                .child("OFF"),
        )
        // Separator
        .child(
            div()
                .w(px(1.0))
                .h_full()
                .bg(theme.border),
        )
        // ON button
        .child(
            div()
                .px_2()
                .py_1()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .bg(if enabled { theme.success } else { theme.background })
                .text_color(if enabled { rgb(0xffffff) } else { theme.text_muted })
                .child("ON"),
        )
}

/// Render a compact toggle button (just the switch, no row styling)
pub fn render_compact_toggle(
    label: &str,
    enabled: bool,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        // Label
        .child(
            div()
                .text_xs()
                .text_color(theme.text_secondary)
                .child(label.to_string()),
        )
        // Toggle switch
        .child(render_toggle_switch(enabled, theme))
}

/// Render a value with unit and color coding
pub fn render_colored_value(value: f64, unit: &str, zero_is_neutral: bool, theme: &Theme) -> impl IntoElement {
    let color = if zero_is_neutral {
        if value > 0.5 {
            rgb(0x22c55e) // Green for positive
        } else if value < -0.5 {
            rgb(0xef4444) // Red for negative
        } else {
            theme.text_muted
        }
    } else {
        theme.text_primary
    };

    div()
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .text_color(color)
        .child(format!("{:+.1}{}", value, unit))
}

/// Format a label with keyboard shortcut indicator
/// e.g., "Threshold" with key 't' -> "[T]hreshold"
pub fn format_shortcut_label(label: &str, shortcut_key: Option<char>) -> String {
    match shortcut_key {
        Some(key) => {
            let key_lower = key.to_ascii_lowercase();
            let label_lower = label.to_lowercase();
            if let Some(pos) = label_lower.find(key_lower) {
                format!(
                    "{}[{}]{}",
                    &label[..pos],
                    label.chars().nth(pos).unwrap().to_ascii_uppercase(),
                    &label[pos + 1..]
                )
            } else {
                format!("[{}] {}", key.to_ascii_uppercase(), label)
            }
        }
        None => label.to_string(),
    }
}

/// Render a vertical slider with label and value
pub fn render_vertical_slider(
    label: &str,
    value: f64,
    min: f64,
    max: f64,
    unit: &str,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    shortcut_key: Option<char>,
    theme: &Theme,
) -> impl IntoElement {
    let is_selected = selected_param == idx && is_editing;
    let normalized = ((value - min) / (max - min)).clamp(0.0, 1.0) as f32;
    let formatted_label = format_shortcut_label(label, shortcut_key);

    // Format value display
    let value_str = if unit == ":1" {
        format!("{:.1}{}", value, unit)
    } else if unit == "%" {
        format!("{:.0}{}", value * 100.0, unit)
    } else {
        format!("{:.1} {}", value, unit)
    };

    div()
        .flex()
        .flex_col()
        .items_center()
        .gap_2()
        .p_2()
        .rounded_lg()
        .bg(if is_selected { theme.accent_muted } else { theme.surface })
        .border_2()
        .border_color(if is_selected { theme.accent } else { theme.border })
        .min_w(px(70.0))
        // Label with keyboard shortcut
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if is_selected { theme.text_primary } else { theme.text_secondary })
                .text_center()
                .child(formatted_label),
        )
        // Current value tag
        .child(
            div()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(if is_selected { theme.success } else { theme.background_secondary })
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(if is_selected { rgb(0xffffff) } else { theme.text_primary })
                .child(value_str),
        )
        // Vertical slider track
        .child(
            div()
                .w(px(16.0))
                .h(px(120.0))
                .bg(theme.background)
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                .relative()
                .overflow_hidden()
                // Filled portion (from bottom)
                .child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(relative(normalized))
                        .bg(if is_selected { theme.accent } else { rgb(0x4f46e5) })
                        .rounded_b_lg(),
                )
                // Thumb indicator
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(relative(normalized))
                        .h(px(4.0))
                        .bg(if is_selected { theme.accent } else { rgb(0x818cf8) })
                        .rounded_sm(),
                ),
        )
        // Scale markers
        .child(
            div()
                .flex()
                .justify_between()
                .w_full()
                .text_xs()
                .text_color(theme.text_muted)
                .child(format!("{:.0}", min))
                .child(format!("{:.0}", max)),
        )
}

/// Render a horizontal gain reduction meter
/// Uses TickConfig for consistent bar fill, ticks, and legend alignment
pub fn render_gr_meter(
    gain_reduction_db: f64, // Should be negative or 0
    max_db: f64,            // e.g., -30.0
    theme: &Theme,
) -> impl IntoElement {
    let gr_abs = gain_reduction_db.abs();
    let tick_config = TickConfig::gain_reduction(max_db);

    // Color gradient: green -> yellow -> orange -> red based on amount
    let color = gr_color(gr_abs);

    let meter_config = HorizontalMeterConfig {
        label_width: 0.0,  // No label column for GR meter
        value_width: 0.0,  // No value column (shown in header)
        bar_height: 12.0,
        show_ticks: true,
        show_legend: true,
    };

    div()
        .flex()
        .flex_col()
        .gap_1()
        .w_full()
        // Header row with label and value
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_secondary)
                        .child("Gain Reduction"),
                )
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(theme.surface)
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(color)
                        .child(format!("{:.1} dB", gr_abs)),
                ),
        )
        // Meter bar (full width, no label/value columns)
        .child(
            div()
                .h(px(meter_config.bar_height))
                .w_full()
                .bg(theme.background)
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .overflow_hidden()
                .child(
                    div()
                        .h_full()
                        .w(relative(tick_config.value_to_position(gr_abs)))
                        .bg(color)
                        .rounded_l_md(),
                ),
        )
        // Tick marks (full width)
        .child(render_tick_row(&tick_config, 0.0, 0.0))
        // Legend (full width)
        .child(
            div()
                .flex()
                .justify_between()
                .text_xs()
                .text_color(theme.text_muted)
                .children(tick_config.major_values.iter().map(|v| {
                    div().child(format!("{:.0}", v))
                })),
        )
}

/// Get color for gain reduction amount
fn gr_color(gr_abs: f64) -> Rgba {
    if gr_abs < 3.0 {
        rgb(0x22c55e) // Green
    } else if gr_abs < 10.0 {
        rgb(0xeab308) // Yellow
    } else if gr_abs < 20.0 {
        rgb(0xf59e0b) // Orange
    } else {
        rgb(0xef4444) // Red
    }
}

/// Render a simple transfer curve visualization (input vs output)
pub fn render_transfer_curve(
    threshold_db: f64,
    ratio: f64,
    knee_db: f64,
    is_limiter: bool,
    theme: &Theme,
) -> impl IntoElement {
    let curve_size = 100.0;
    let num_points = 20;

    // Generate curve points
    let mut bars: Vec<(f32, Rgba)> = Vec::new();

    for i in 0..num_points {
        let input_db = -60.0 + (i as f64 / num_points as f64) * 60.0; // -60 to 0 dB
        let output_db = if is_limiter {
            // Limiter: hard clip at threshold
            input_db.min(threshold_db)
        } else {
            // Compressor: soft knee compression
            if input_db < threshold_db - knee_db / 2.0 {
                input_db
            } else if input_db > threshold_db + knee_db / 2.0 {
                threshold_db + (input_db - threshold_db) / ratio
            } else {
                // Knee region
                let knee_input = input_db - (threshold_db - knee_db / 2.0);
                let knee_ratio = knee_input / knee_db;
                input_db + (knee_ratio * knee_ratio / 2.0) * (1.0 / ratio - 1.0) * knee_db
            }
        };

        let height = ((output_db + 60.0) / 60.0).clamp(0.0, 1.0) as f32;
        let is_compressed = output_db < input_db - 0.5;
        let color = if is_compressed {
            rgb(0xef4444) // Red for compressed region
        } else {
            rgb(0x4f46e5) // Blue for linear region
        };
        bars.push((height, color));
    }

    div()
        .flex()
        .flex_col()
        .gap_2()
        // Title
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_secondary)
                .text_center()
                .child("Transfer Curve"),
        )
        // Curve visualization
        .child(
            div()
                .w(px(curve_size))
                .h(px(curve_size))
                .bg(theme.background)
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                .p_1()
                .flex()
                .items_end()
                .gap_px()
                .children(bars.into_iter().map(|(height, color)| {
                    div()
                        .flex_1()
                        .h(relative(height))
                        .bg(color)
                        .rounded_t_sm()
                })),
        )
        // Axis labels
        .child(
            div()
                .flex()
                .justify_between()
                .text_xs()
                .text_color(theme.text_muted)
                .child("-60")
                .child("Input (dB)")
                .child("0"),
        )
}

/// Render a peak meter (vertical)
pub fn render_peak_meter(
    peak_db: f64,
    ceiling_db: f64,
    theme: &Theme,
) -> impl IntoElement {
    let min_db = -60.0;
    let normalized = ((peak_db - min_db) / (0.0 - min_db)).clamp(0.0, 1.0) as f32;
    let ceiling_normalized = ((ceiling_db - min_db) / (0.0 - min_db)).clamp(0.0, 1.0) as f32;

    // Color based on level
    let color = if peak_db > ceiling_db {
        rgb(0xef4444) // Red - clipping
    } else if peak_db > ceiling_db - 3.0 {
        rgb(0xf59e0b) // Orange - near ceiling
    } else if peak_db > -12.0 {
        rgb(0xeab308) // Yellow - moderate
    } else {
        rgb(0x22c55e) // Green - safe
    };

    div()
        .flex()
        .flex_col()
        .items_center()
        .gap_1()
        // Label
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_secondary)
                .child("Peak"),
        )
        // Meter
        .child(
            div()
                .w(px(20.0))
                .h(px(80.0))
                .bg(theme.background)
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .relative()
                .overflow_hidden()
                // Level bar
                .child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(relative(normalized))
                        .bg(color),
                )
                // Ceiling marker
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(relative(ceiling_normalized))
                        .h(px(2.0))
                        .bg(rgb(0xef4444)),
                ),
        )
        // Value
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(color)
                .child(if peak_db <= min_db {
                    "-∞".to_string()
                } else {
                    format!("{:.1}", peak_db)
                }),
        )
}
