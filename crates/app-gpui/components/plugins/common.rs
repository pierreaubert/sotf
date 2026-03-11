//! Common utilities for plugin UI components

use crate::app::AppState;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Potentiometer, PotentiometerScale, PotentiometerSize, Toggle, ToggleStyle, VerticalSlider,
    VerticalSliderTheme,
};
pub use sotf_audio_player::param_index_to_engine_param;
use sotf_audio_player_midi::mapping::{MidiOverlay, ParamAssignment};
use sotf_audio_player_midi::PhysicalControlKind;

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
                .min_w(rems(5.0))
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

/// Render a parameter row with name, value, and optional MIDI assignment badge
pub fn render_param_row_with_midi(
    name: &str,
    value: &str,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &Theme,
    midi_overlay: Option<&MidiOverlay>,
) -> impl IntoElement {
    let is_selected = selected_param == idx && is_editing;
    let is_learn_target = midi_overlay
        .and_then(|o| o.learn_target)
        .is_some_and(|t| t == idx);

    div()
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .py_2()
        .rounded_lg()
        .bg(if is_learn_target {
            Theme::with_opacity(theme.warning, 0.2)
        } else if is_selected {
            theme.accent_muted
        } else {
            theme.surface
        })
        .border_l_4()
        .border_color(if is_learn_target {
            theme.warning
        } else if is_selected {
            theme.accent
        } else {
            theme.surface
        })
        // Parameter name + MIDI badge
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
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
                .children(
                    midi_overlay
                        .and_then(|o| o.assignments.get(&idx))
                        .map(|assignment| render_midi_badge(assignment, theme)),
                ),
        )
        // Value
        .child(
            div()
                .min_w(rems(5.0))
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

/// Render a small MIDI control badge (e.g., "K1", "F3") next to a parameter name
pub fn render_midi_badge(assignment: &ParamAssignment, theme: &Theme) -> impl IntoElement {
    let icon = match assignment.control_kind {
        PhysicalControlKind::Fader => "▏",
        PhysicalControlKind::Pot => "◎",
        PhysicalControlKind::Encoder | PhysicalControlKind::EncoderWithButton => "↻",
        PhysicalControlKind::Button => "◻",
    };

    let badge_color = if assignment.is_override {
        theme.warning
    } else {
        theme.accent
    };

    div()
        .flex()
        .items_center()
        .gap(px(2.0))
        .px(px(4.0))
        .py(px(1.0))
        .rounded(px(3.0))
        .bg(Theme::with_opacity(badge_color, 0.2))
        .child(
            div()
                .text_xs()
                .text_color(badge_color)
                .child(icon.to_string()),
        )
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(badge_color)
                .child(assignment.control_label.clone()),
        )
}

/// Render a MIDI page indicator (e.g., "Page 1/3")
pub fn render_midi_page_indicator(
    current_page: usize,
    total_pages: usize,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_1()
        .px_2()
        .py_1()
        .rounded_md()
        .bg(theme.surface)
        .child(div().text_xs().text_color(theme.text_muted).child(format!(
            "MIDI {}/{}",
            current_page + 1,
            total_pages
        )))
}

/// Render a section header (with bottom margin - use for bordered sections)
pub fn render_section_header(title: &str, theme: &Theme) -> impl IntoElement {
    div()
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .text_color(theme.text_primary)
        .mb_2()
        .child(title.to_string())
}

/// Render a compact section title (no margin - use for borderless layouts)
pub fn render_section_title(title: &str, theme: &Theme) -> impl IntoElement {
    div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.text_secondary)
        .child(title.to_string())
}

/// Trait extension for applying parameter section styling to any Div
pub trait ParamSectionStyle {
    /// Apply base param section styling (rounded, background, border) without padding
    fn param_section_base(self, theme: &Theme) -> Self;
    /// Apply param section styling with standard p_3 padding
    fn param_section_style(self, theme: &Theme) -> Self;
    /// Apply param section styling with larger p_4 padding
    fn param_section_style_lg(self, theme: &Theme) -> Self;
}

impl ParamSectionStyle for Div {
    fn param_section_base(self, theme: &Theme) -> Self {
        self.rounded_xl()
            .bg(theme.background_secondary)
            .border_1()
            .border_color(theme.border)
    }

    fn param_section_style(self, theme: &Theme) -> Self {
        self.param_section_base(theme).p_3()
    }

    fn param_section_style_lg(self, theme: &Theme) -> Self {
        self.param_section_base(theme).p_4()
    }
}

/// Create a new parameter section container with flex column layout
pub fn render_param_section(theme: &Theme) -> Div {
    div().flex().flex_col().gap_2().param_section_style(theme)
}

/// Create a new parameter section container with flex column layout and larger padding
pub fn render_param_section_lg(theme: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .param_section_style_lg(theme)
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

/// Render a toggle button using gpui-ui-kit Toggle component
/// Uses `Entity<AppState>` for direct state updates
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
        .style(ToggleStyle::Segmented)
        .selected(is_selected)
        .theme(theme.to_toggle_theme())
        .on_change({
            let entity = entity.clone();
            move |new_value, _, cx| {
                entity.update(cx, |state, _| {
                    state
                        .app
                        .set_plugin_param(plugin_idx, idx, if new_value { 1.0 } else { 0.0 });
                });
            }
        })
}

/// Render a toggle button without label (just the switch)
/// Uses `Entity<AppState>` for direct state updates
pub fn render_toggle_button(
    entity: Entity<AppState>,
    plugin_idx: usize,
    enabled: bool,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &Theme,
) -> impl IntoElement {
    let is_selected = selected_param == idx && is_editing;

    Toggle::new(("toggle-btn", plugin_idx * 1000 + idx))
        .checked(enabled)
        .style(ToggleStyle::Segmented)
        .selected(is_selected)
        .theme(theme.to_toggle_theme())
        .on_change({
            let entity = entity.clone();
            move |new_value, _, cx| {
                entity.update(cx, |state, _| {
                    state
                        .app
                        .set_plugin_param(plugin_idx, idx, if new_value { 1.0 } else { 0.0 });
                });
            }
        })
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
/// e.g., "Threshold" with key 't' -> "\[T\]hreshold"
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
        peak_marker: theme.meter_colors.peak,
    }
}

/// Render a vertical slider with label, value, drag support and enhanced visual feedback
/// Uses `Entity<AppState>` for direct state updates
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
    render_vertical_slider_sized(
        entity,
        plugin_idx,
        label,
        value,
        min,
        max,
        unit,
        idx,
        selected_param,
        is_editing,
        shortcut_key,
        None,
        theme,
    )
}

/// Render a vertical slider with custom height
/// Uses `Entity<AppState>` for direct state updates
#[allow(clippy::too_many_arguments)]
pub fn render_vertical_slider_sized(
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
    _height: Option<f32>,
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
                    state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                    state.app.plugin_state.plugin_param_selection = idx;
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

    div().key_context("plugin-control").child(slider)
}

/// Render a vertical slider with tick marks, custom height, and enhanced visual feedback
/// Uses `Entity<AppState>` for direct state updates
#[allow(clippy::too_many_arguments)]
pub fn render_vertical_slider_with_ticks(
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
    height: f32,
    theme: &Theme,
) -> impl IntoElement {
    let is_selected = selected_param == idx && is_editing;

    let mut slider = VerticalSlider::new(("slider-ticks", plugin_idx * 1000 + idx))
        .value(value)
        .min(min)
        .max(max)
        .unit(unit.to_string())
        .label(label.to_string())
        .height(height)
        .with_ticks()
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
                    state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                    state.app.plugin_state.plugin_param_selection = idx;
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

    div().key_context("plugin-control").child(slider)
}

/// Render a simple transfer curve visualization (input vs output)
pub fn render_transfer_curve(
    threshold_db: f64,
    ratio: f64,
    knee_db: f64,
    is_limiter: bool,
    theme: &Theme,
) -> impl IntoElement {
    render_transfer_curve_sized(threshold_db, ratio, knee_db, is_limiter, 200.0, theme)
}

/// Compute the output dB for a given input dB on the transfer curve
fn compute_transfer(input_db: f64, threshold_db: f64, ratio: f64, knee_db: f64, is_limiter: bool) -> f64 {
    if is_limiter {
        input_db.min(threshold_db)
    } else if input_db < threshold_db - knee_db / 2.0 {
        input_db
    } else if input_db > threshold_db + knee_db / 2.0 {
        threshold_db + (input_db - threshold_db) / ratio
    } else {
        let knee_input = input_db - (threshold_db - knee_db / 2.0);
        let knee_ratio = knee_input / knee_db;
        input_db + (knee_ratio * knee_ratio / 2.0) * (1.0 / ratio - 1.0) * knee_db
    }
}

/// Render a transfer curve visualization with custom width
pub fn render_transfer_curve_sized(
    threshold_db: f64,
    ratio: f64,
    knee_db: f64,
    is_limiter: bool,
    width: f32,
    theme: &Theme,
) -> impl IntoElement {
    render_transfer_curve_with_level(threshold_db, ratio, knee_db, is_limiter, width, None, theme)
}

/// Render a transfer curve with optional input level indicator
///
/// When `input_level_db` is provided, draws a vertical + horizontal crosshair
/// showing the current operating point on the curve.
#[allow(clippy::too_many_arguments)]
pub fn render_transfer_curve_with_level(
    threshold_db: f64,
    ratio: f64,
    knee_db: f64,
    is_limiter: bool,
    width: f32,
    input_level_db: Option<f64>,
    theme: &Theme,
) -> impl IntoElement {
    let curve_width = width.max(200.0);
    let num_points = 30;

    // Generate curve points
    let mut bars: Vec<(f32, Rgba, bool)> = Vec::new();

    // Compute input level position (normalized 0..1 in the -60..0 range)
    let input_pos = input_level_db.map(|db| ((db + 60.0) / 60.0).clamp(0.0, 1.0) as f32);
    let output_pos = input_level_db.map(|db| {
        let out = compute_transfer(db, threshold_db, ratio, knee_db, is_limiter);
        ((out + 60.0) / 60.0).clamp(0.0, 1.0) as f32
    });

    for i in 0..num_points {
        let input_db = -60.0 + (i as f64 / num_points as f64) * 60.0;
        let output_db = compute_transfer(input_db, threshold_db, ratio, knee_db, is_limiter);

        let height = ((output_db + 60.0) / 60.0).clamp(0.0, 1.0) as f32;
        let is_compressed = output_db < input_db - 0.5;

        // Highlight the bar at the current input level
        let at_input = input_pos.is_some_and(|pos| {
            let bar_pos = i as f32 / num_points as f32;
            (bar_pos - pos).abs() < (1.0 / num_points as f32)
        });

        let color = if at_input {
            theme.warning // Yellow for current operating point
        } else if is_compressed {
            theme.meter_clip // Red for compressed region
        } else {
            theme.accent // Blue for linear region
        };
        bars.push((height, color, at_input));
    }

    div()
        .flex()
        .flex_col()
        .flex_1()
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
                .w(px(curve_width))
                .flex_1()
                .min_h(px(120.0))
                .bg(theme.background)
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                .p_1()
                .relative()
                .flex()
                .items_end()
                .gap_px()
                .children(bars.into_iter().map(|(height, color, at_input)| {
                    let mut bar = div().flex_1().h(relative(height)).bg(color).rounded_t_sm();
                    if at_input {
                        bar = bar.w(px(3.0));
                    }
                    bar
                }))
                // Horizontal output level indicator line
                .children(output_pos.map(|pos| {
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(relative(pos))
                        .h(px(1.0))
                        .bg(Theme::opacity_20pct(theme.warning))
                })),
        )
        // Axis labels
        .child(
            div()
                .flex()
                .justify_between()
                .text_xs()
                .text_color(theme.text_muted)
                .child("-60 dB")
                .child("Input")
                .child("0 dB"),
        )
        // Y-axis label
        .child(
            div()
                .flex()
                .justify_between()
                .text_xs()
                .text_color(theme.text_muted)
                .child("Output"),
        )
}

/// Standard 3-column layout for dynamics plugins (compressor, gate, limiter, expander).
///
/// ```text
/// ┌──────────────┬──────────────────┬──────────────┐
/// │              │  [Param sliders] │   GR Meter   │
/// │  Transfer    │  [Param knobs]   │              │
/// │  Curve       │  [Toggles]       │  [Extra      │
/// │              │                  │   controls]  │
/// └──────────────┴──────────────────┴──────────────┘
/// ```
pub fn render_dynamics_layout(
    transfer_curve: impl IntoElement,
    controls: impl IntoElement,
    meter_section: impl IntoElement,
    meter_width: f32,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .gap_4()
                // Column 1: Transfer curve
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .w(px(meter_width))
                        .child(transfer_curve),
                )
                // Column 2: Controls
                .child(div().flex().flex_1().child(controls))
                // Column 3: Meters
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .w(px(meter_width))
                        .child(meter_section),
                ),
        )
}

/// Render a rotary knob control using gpui-ui-kit Potentiometer
/// Uses `Entity<AppState>` for direct state updates
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
    render_knob_sized(
        entity,
        plugin_idx,
        label,
        value,
        min,
        max,
        unit,
        idx,
        selected_param,
        is_editing,
        shortcut_key,
        PotentiometerSize::Sm,
        theme,
    )
}

/// Render a rotary knob control with custom size
pub fn render_knob_sized(
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
    size: PotentiometerSize,
    theme: &Theme,
) -> impl IntoElement {
    let is_selected = selected_param == idx && is_editing;

    // Determine scale type based on unit (Hz parameters use logarithmic scale)
    let scale = if unit == "Hz" {
        PotentiometerScale::Logarithmic
    } else {
        PotentiometerScale::Linear
    };

    let mut knob = Potentiometer::new(("knob", plugin_idx * 1000 + idx))
        .value(value)
        .min(min)
        .max(max)
        .unit(unit.to_string())
        .label(label.to_string())
        .size(size)
        .scale(scale)
        .selected(is_selected)
        .theme(theme.to_potentiometer_theme())
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
                    state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                    state.app.plugin_state.plugin_param_selection = idx;
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

    div().key_context("plugin-control").child(knob)
}
