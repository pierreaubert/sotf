//! Common utilities for plugin UI components

use crate::app::{AppState, InputMode};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Potentiometer, PotentiometerTheme, Toggle, ToggleStyle, ToggleTheme, VerticalSlider,
    VerticalSliderTheme,
};

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
        .bg(if is_selected {
            theme.accent_muted
        } else {
            theme.surface
        })
        .border_l_4()
        .border_color(if is_selected {
            theme.accent
        } else {
            theme.surface
        })
        // Parameter name
        .child(
            div()
                .text_sm()
                .text_color(if is_selected {
                    theme.text_primary
                } else {
                    theme.text_secondary
                })
                .font_weight(if is_selected {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .child(name.to_string()),
        )
        // Value
        .child(
            div()
                .min_w(px(80.0))
                .px_2()
                .py_1()
                .rounded_md()
                .bg(if is_selected {
                    theme.background
                } else {
                    theme.background_secondary
                })
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

/// Convert Theme to ToggleTheme for gpui-ui-kit Toggle
fn theme_to_toggle_theme(theme: &Theme) -> ToggleTheme {
    ToggleTheme {
        checked_bg: theme.accent,
        unchecked_bg: theme.surface,
        knob: rgba(0xffffffff),
        label: theme.text_secondary,
        accent: theme.accent,
        accent_muted: theme.accent_muted,
        success: theme.success,
        border: theme.border,
        text_on_accent: theme.text_on_accent,
        text_muted: theme.text_muted,
    }
}

/// Render a toggle button with [OFF | ON] display
/// The active state is highlighted, inactive is dimmed
/// Uses Entity<AppState> for direct state updates
pub fn render_toggle(
    entity: Entity<AppState>,
    plugin_idx: usize,
    label: &str,
    enabled: bool,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &Theme,
) -> impl IntoElement {
    let is_selected = selected_param == idx && is_editing;

    Toggle::new(("toggle", plugin_idx * 1000 + idx))
        .checked(enabled)
        .label(label.to_string())
        .selected(is_selected)
        .style(ToggleStyle::Segmented)
        .theme(theme_to_toggle_theme(theme))
        .on_change(move |new_checked, _, cx| {
            entity.update(cx, |state, _| {
                state
                    .app
                    .set_plugin_param(plugin_idx, idx, if new_checked { 1.0 } else { 0.0 });
            });
        })
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
                .bg(if !enabled {
                    theme.surface_hover
                } else {
                    theme.background
                })
                .text_color(if !enabled {
                    theme.text_primary
                } else {
                    theme.text_muted
                })
                .child("OFF"),
        )
        // Separator
        .child(div().w(px(1.0)).h_full().bg(theme.border))
        // ON button
        .child(
            div()
                .px_2()
                .py_1()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .bg(if enabled {
                    theme.success
                } else {
                    theme.background
                })
                .text_color(if enabled {
                    theme.text_on_accent
                } else {
                    theme.text_muted
                })
                .child("ON"),
        )
}

/// Render a compact toggle button (just the switch, no row styling)
pub fn render_compact_toggle(label: &str, enabled: bool, theme: &Theme) -> impl IntoElement {
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
pub fn render_colored_value(
    value: f64,
    unit: &str,
    zero_is_neutral: bool,
    theme: &Theme,
) -> impl IntoElement {
    let color = if zero_is_neutral {
        if value > 0.5 {
            theme.success // Green for positive
        } else if value < -0.5 {
            theme.error // Red for negative
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

/// Convert Theme to VerticalSliderTheme for gpui-ui-kit VerticalSlider
fn theme_to_vertical_slider_theme(theme: &Theme) -> VerticalSliderTheme {
    VerticalSliderTheme {
        surface: theme.surface,
        surface_hover: theme.surface_hover,
        track_bg: theme.background,
        accent: theme.accent,
        accent_muted: theme.accent_muted,
        border: theme.border,
        text_secondary: theme.text_secondary,
        text_primary: theme.text_primary,
        text_muted: theme.text_muted,
        text_on_accent: theme.text_on_accent,
        background_secondary: theme.background_secondary,
    }
}

/// Render a vertical slider with label, value, drag support and enhanced visual feedback
/// Uses Entity<AppState> for direct state updates
pub fn render_vertical_slider(
    entity: Entity<AppState>,
    plugin_idx: usize,
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

    let mut slider = VerticalSlider::new(("slider", plugin_idx * 1000 + idx))
        .value(value)
        .min(min)
        .max(max)
        .unit(unit.to_string())
        .label(label.to_string())
        .selected(is_selected)
        .theme(theme_to_vertical_slider_theme(theme))
        .on_change({
            let entity = entity.clone();
            move |new_value, _, cx| {
                entity.update(cx, |state, _| {
                    state.app.set_plugin_param(plugin_idx, idx, new_value);
                });
            }
        })
        .on_drag_start({
            let entity = entity.clone();
            move |start_y, start_value, _, cx| {
                entity.update(cx, |state, _| {
                    state.app.is_dragging_knob = true;
                    state.app.knob_drag_plugin_idx = plugin_idx;
                    state.app.knob_drag_param_idx = idx;
                    state.app.knob_drag_start_y = Some(start_y);
                    state.app.knob_drag_start_value = start_value;
                    state.app.knob_drag_min = min;
                    state.app.knob_drag_max = max;
                });
            }
        })
        .on_select({
            let entity = entity.clone();
            move |_, cx| {
                entity.update(cx, |state, _| {
                    state.app.editing_plugin_index = Some(plugin_idx);
                    state.app.plugin_param_selection = idx;
                    state.app.input_mode = InputMode::EditPlugin;
                });
            }
        })
        .on_reset({
            let entity = entity.clone();
            move |_, cx| {
                entity.update(cx, |state, _| {
                    state.app.reset_plugin_param(plugin_idx, idx);
                });
            }
        });

    if let Some(key) = shortcut_key {
        slider = slider.shortcut_key(key);
    }

    slider
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
            theme.meter_clip // Red for compressed region
        } else {
            theme.accent // Blue for linear region
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
                    div().flex_1().h(relative(height)).bg(color).rounded_t_sm()
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

/// Convert Theme to PotentiometerTheme for gpui-ui-kit Potentiometer
fn theme_to_potentiometer_theme(theme: &Theme) -> PotentiometerTheme {
    PotentiometerTheme {
        surface: theme.surface,
        surface_hover: theme.surface_hover,
        knob_bg: theme.background,
        accent: theme.accent,
        accent_muted: theme.accent_muted,
        border: theme.border,
        text_secondary: theme.text_secondary,
        text_primary: theme.text_primary,
        text_muted: theme.text_muted,
        text_on_accent: theme.text_on_accent,
        background_secondary: theme.background_secondary,
    }
}

/// Render a rotary knob control with drag support and enhanced visual feedback
/// Uses Entity<AppState> for direct state updates instead of action dispatch
pub fn render_knob(
    entity: Entity<AppState>,
    plugin_idx: usize,
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

    let mut knob = Potentiometer::new(("knob", plugin_idx * 1000 + idx))
        .value(value)
        .min(min)
        .max(max)
        .unit(unit.to_string())
        .label(label.to_string())
        .selected(is_selected)
        .theme(theme_to_potentiometer_theme(theme))
        .on_change({
            let entity = entity.clone();
            move |new_value, _, cx| {
                entity.update(cx, |state, _| {
                    state.app.set_plugin_param(plugin_idx, idx, new_value);
                });
            }
        })
        .on_drag_start({
            let entity = entity.clone();
            move |start_y, start_value, _, cx| {
                entity.update(cx, |state, _| {
                    state.app.is_dragging_knob = true;
                    state.app.knob_drag_plugin_idx = plugin_idx;
                    state.app.knob_drag_param_idx = idx;
                    state.app.knob_drag_start_y = Some(start_y);
                    state.app.knob_drag_start_value = start_value;
                    state.app.knob_drag_min = min;
                    state.app.knob_drag_max = max;
                });
            }
        })
        .on_select({
            let entity = entity.clone();
            move |_, cx| {
                entity.update(cx, |state, _| {
                    state.app.editing_plugin_index = Some(plugin_idx);
                    state.app.plugin_param_selection = idx;
                    state.app.input_mode = InputMode::EditPlugin;
                });
            }
        })
        .on_reset({
            let entity = entity.clone();
            move |_, cx| {
                entity.update(cx, |state, _| {
                    state.app.reset_plugin_param(plugin_idx, idx);
                });
            }
        });

    if let Some(key) = shortcut_key {
        knob = knob.shortcut_key(key);
    }

    knob
}
