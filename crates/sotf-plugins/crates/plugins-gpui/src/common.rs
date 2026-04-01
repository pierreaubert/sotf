//! Common utilities for plugin UI components

use crate::PluginViewHost;
use crate::PluginViewTheme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Potentiometer, PotentiometerScale, PotentiometerSize, Toggle, ToggleStyle, VerticalSlider,
    VerticalSliderTheme,
};
use sotf_audio_player_midi::PhysicalControlKind;
use sotf_audio_player_midi::mapping::{MidiOverlay, ParamAssignment};

/// Render a parameter row with name, value, and optional range hint.
///
/// When `range_hint` is `Some("0.0 — 100.0")` and the row is selected,
/// the range is displayed as muted text beneath the value.
pub fn render_param_row(
    name: &str,
    value: &str,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &PluginViewTheme,
    range_hint: Option<&str>,
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
        // Value + optional range hint
        .child(
            div()
                .flex()
                .flex_col()
                .items_end()
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
                .when(is_selected && range_hint.is_some(), |d| {
                    d.child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .px_2()
                            .child(range_hint.unwrap_or("").to_string()),
                    )
                }),
        )
}

/// Render a parameter row with name, value, and optional MIDI assignment badge
pub fn render_param_row_with_midi(
    name: &str,
    value: &str,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &PluginViewTheme,
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
            PluginViewTheme::with_opacity(theme.warning, 0.2)
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
pub fn render_midi_badge(
    assignment: &ParamAssignment,
    theme: &PluginViewTheme,
) -> impl IntoElement {
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
        .bg(PluginViewTheme::with_opacity(badge_color, 0.2))
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
    theme: &PluginViewTheme,
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
pub fn render_section_header(title: &str, theme: &PluginViewTheme) -> impl IntoElement {
    div()
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .text_color(theme.text_primary)
        .mb_2()
        .child(title.to_string())
}

/// Render a compact section title with a ruled line extending to the right edge.
///
/// ```text
/// DYNAMICS ─────────────────
/// ```
pub fn render_section_title(title: &str, theme: &PluginViewTheme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_secondary)
                .flex_shrink_0()
                .child(title.to_string()),
        )
        .child(div().flex_1().h(px(1.0)).bg(theme.border))
}

/// Trait extension for applying parameter section styling to any Div
pub trait ParamSectionStyle {
    /// Apply base param section styling (rounded, background, border) without padding
    fn param_section_base(self, theme: &PluginViewTheme) -> Self;
    /// Apply param section styling with standard p_3 padding
    fn param_section_style(self, theme: &PluginViewTheme) -> Self;
    /// Apply param section styling with larger p_4 padding
    fn param_section_style_lg(self, theme: &PluginViewTheme) -> Self;
}

impl ParamSectionStyle for Div {
    fn param_section_base(self, theme: &PluginViewTheme) -> Self {
        self.rounded_xl()
            .bg(theme.background_secondary)
            .border_1()
            .border_color(theme.border)
    }

    fn param_section_style(self, theme: &PluginViewTheme) -> Self {
        self.param_section_base(theme).p_3()
    }

    fn param_section_style_lg(self, theme: &PluginViewTheme) -> Self {
        self.param_section_base(theme).p_4()
    }
}

/// Create a new parameter section container with flex column layout
pub fn render_param_section(theme: &PluginViewTheme) -> Div {
    div().flex().flex_col().gap_2().param_section_style(theme)
}

/// Create a new parameter section container with flex column layout and larger padding
pub fn render_param_section_lg(theme: &PluginViewTheme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .param_section_style_lg(theme)
}

/// Render keyboard hints for edit mode
pub fn render_edit_hints(theme: &PluginViewTheme) -> impl IntoElement {
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
/// Uses `Entity<H>` for direct state updates via `PluginViewHost`
pub fn render_toggle<H: PluginViewHost>(
    entity: Entity<H>,
    plugin_idx: usize,
    label: &str,
    enabled: bool,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &PluginViewTheme,
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
                entity.update(cx, |host, _| {
                    host.set_plugin_param(plugin_idx, idx, if new_value { 1.0 } else { 0.0 });
                });
            }
        })
}

/// Render a toggle button without label (just the switch)
/// Uses `Entity<H>` for direct state updates via `PluginViewHost`
pub fn render_toggle_button<H: PluginViewHost>(
    entity: Entity<H>,
    plugin_idx: usize,
    enabled: bool,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &PluginViewTheme,
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
                entity.update(cx, |host, _| {
                    host.set_plugin_param(plugin_idx, idx, if new_value { 1.0 } else { 0.0 });
                });
            }
        })
}

/// Render a value with unit and color coding
pub fn render_colored_value(
    value: f64,
    unit: &str,
    zero_is_neutral: bool,
    theme: &PluginViewTheme,
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

/// Convert PluginViewTheme to VerticalSliderTheme for gpui-ui-kit VerticalSlider
fn theme_to_vertical_slider_theme(theme: &PluginViewTheme) -> VerticalSliderTheme {
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
/// Uses `Entity<H>` for direct state updates via `PluginViewHost`
pub fn render_vertical_slider<H: PluginViewHost>(
    entity: Entity<H>,
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
    theme: &PluginViewTheme,
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
/// Uses `Entity<H>` for direct state updates via `PluginViewHost`
#[allow(clippy::too_many_arguments)]
pub fn render_vertical_slider_sized<H: PluginViewHost>(
    entity: Entity<H>,
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
    theme: &PluginViewTheme,
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
        .design_tokens(theme.design_tokens.clone())
        .on_change({
            let entity = entity.clone();
            move |new_value, _, cx| {
                entity.update(cx, |host, _| {
                    host.set_plugin_param(plugin_idx, idx, new_value);
                });
            }
        })
        .on_drag_start({
            let entity = entity.clone();
            move |start_y, start_value, _, cx| {
                entity.update(cx, |host, _| {
                    host.on_knob_drag_start(plugin_idx, idx, start_y, start_value, min, max);
                });
            }
        })
        .on_select({
            let entity = entity.clone();
            move |_, cx| {
                entity.update(cx, |host, _| {
                    host.set_editing_plugin(plugin_idx);
                    host.set_selected_param(plugin_idx, idx);
                });
            }
        })
        .on_reset({
            let entity = entity.clone();
            move |_, cx| {
                entity.update(cx, |host, _| {
                    host.reset_plugin_param(plugin_idx, idx);
                });
            }
        });

    if let Some(key) = shortcut_key {
        slider = slider.shortcut_key(key);
    }

    div().key_context("plugin-control").child(slider)
}

/// Render a vertical slider with tick marks, custom height, and enhanced visual feedback
/// Uses `Entity<H>` for direct state updates via `PluginViewHost`
#[allow(clippy::too_many_arguments)]
pub fn render_vertical_slider_with_ticks<H: PluginViewHost>(
    entity: Entity<H>,
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
    theme: &PluginViewTheme,
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
        .design_tokens(theme.design_tokens.clone())
        .on_change({
            let entity = entity.clone();
            move |new_value, _, cx| {
                entity.update(cx, |host, _| {
                    host.set_plugin_param(plugin_idx, idx, new_value);
                });
            }
        })
        .on_drag_start({
            let entity = entity.clone();
            move |start_y, start_value, _, cx| {
                entity.update(cx, |host, _| {
                    host.on_knob_drag_start(plugin_idx, idx, start_y, start_value, min, max);
                });
            }
        })
        .on_select({
            let entity = entity.clone();
            move |_, cx| {
                entity.update(cx, |host, _| {
                    host.set_editing_plugin(plugin_idx);
                    host.set_selected_param(plugin_idx, idx);
                });
            }
        })
        .on_reset({
            let entity = entity.clone();
            move |_, cx| {
                entity.update(cx, |host, _| {
                    host.reset_plugin_param(plugin_idx, idx);
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
    theme: &PluginViewTheme,
) -> impl IntoElement {
    render_transfer_curve_sized(threshold_db, ratio, knee_db, is_limiter, 200.0, theme)
}

/// Compute the output dB for a given input dB on the transfer curve
fn compute_transfer(
    input_db: f64,
    threshold_db: f64,
    ratio: f64,
    knee_db: f64,
    is_limiter: bool,
) -> f64 {
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
    theme: &PluginViewTheme,
) -> impl IntoElement {
    render_transfer_curve_with_level(threshold_db, ratio, knee_db, is_limiter, width, None, theme)
}

/// Render a transfer curve with optional input level indicator.
///
/// Uses a custom paint element for smooth curve rendering instead of bars.
/// When `input_level_db` is provided, draws an animated operating point dot
/// on the curve.
#[allow(clippy::too_many_arguments)]
pub fn render_transfer_curve_with_level(
    threshold_db: f64,
    ratio: f64,
    knee_db: f64,
    is_limiter: bool,
    width: f32,
    input_level_db: Option<f64>,
    theme: &PluginViewTheme,
) -> impl IntoElement {
    let curve_width = width.max(200.0);
    let curve_height: f32 = 140.0;

    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .w(px(curve_width))
                .h(px(curve_height))
                .rounded_lg()
                .overflow_hidden()
                .child(TransferCurveElement {
                    width: curve_width,
                    height: curve_height,
                    threshold_db,
                    ratio,
                    knee_db,
                    is_limiter,
                    input_level_db,
                    accent: theme.accent,
                    compressed_color: theme.meter_clip,
                    operating_point_color: theme.warning,
                    bg: theme.background,
                    grid_color: theme.border,
                    text_color: theme.text_muted,
                }),
        )
        // X-axis labels
        .child(
            div()
                .flex()
                .justify_between()
                .w(px(curve_width))
                .text_xs()
                .text_color(theme.text_muted)
                .child("-60 dB")
                .child("0 dB"),
        )
}

// ============================================================================
// TransferCurveElement — custom-painted smooth transfer curve
// ============================================================================

/// Custom element that paints a smooth dynamics transfer curve using PathBuilder.
///
/// Features:
/// - Smooth curve (64 sample points, no staircase)
/// - Filled area under curve with gradient opacity
/// - Grid lines with dB scale
/// - Unity gain reference line (diagonal)
/// - Animated operating point dot
struct TransferCurveElement {
    width: f32,
    height: f32,
    threshold_db: f64,
    ratio: f64,
    knee_db: f64,
    is_limiter: bool,
    input_level_db: Option<f64>,
    accent: Rgba,
    compressed_color: Rgba,
    operating_point_color: Rgba,
    bg: Rgba,
    grid_color: Rgba,
    text_color: Rgba,
}

impl IntoElement for TransferCurveElement {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TransferCurveElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = window.request_layout(
            Style {
                size: Size {
                    width: px(self.width).into(),
                    height: px(self.height).into(),
                },
                ..Default::default()
            },
            [],
            cx,
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        let ox = bounds.origin.x;
        let oy = bounds.origin.y;
        let w = self.width;
        let h = self.height;
        let pad = 4.0; // Inner padding

        // Background
        window.paint_quad(PaintQuad {
            bounds,
            corner_radii: Corners::all(px(8.0)),
            background: self.bg.into(),
            border_widths: Edges::all(px(1.0)),
            border_color: self.grid_color.into(),
            border_style: BorderStyle::default(),
        });

        // Helper: dB to pixel coordinates
        // Input range: -60 to 0 dB → x: pad to w-pad
        // Output range: -60 to 0 dB → y: h-pad (bottom) to pad (top)
        let db_to_x = |db: f64| -> f32 {
            let norm = ((db + 60.0) / 60.0).clamp(0.0, 1.0) as f32;
            pad + norm * (w - 2.0 * pad)
        };
        let db_to_y = |db: f64| -> f32 {
            let norm = ((db + 60.0) / 60.0).clamp(0.0, 1.0) as f32;
            (h - pad) - norm * (h - 2.0 * pad)
        };

        // Grid lines (-48, -36, -24, -12, 0 dB)
        let grid_line_color = Rgba {
            r: self.grid_color.r,
            g: self.grid_color.g,
            b: self.grid_color.b,
            a: 0.3,
        };
        for &db in &[-48.0, -36.0, -24.0, -12.0] {
            let y_pos = db_to_y(db);
            // Horizontal grid line
            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(ox + px(pad), oy + px(y_pos)),
                    size: size(px(w - 2.0 * pad), px(1.0)),
                },
                corner_radii: Corners::default(),
                background: grid_line_color.into(),
                border_widths: Edges::default(),
                border_color: gpui::transparent_black(),
                border_style: BorderStyle::default(),
            });
            // Vertical grid line
            let x_pos = db_to_x(db);
            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(ox + px(x_pos), oy + px(pad)),
                    size: size(px(1.0), px(h - 2.0 * pad)),
                },
                corner_radii: Corners::default(),
                background: grid_line_color.into(),
                border_widths: Edges::default(),
                border_color: gpui::transparent_black(),
                border_style: BorderStyle::default(),
            });
        }

        // Unity gain reference line (diagonal from -60,-60 to 0,0)
        let unity_color = Rgba {
            r: self.text_color.r,
            g: self.text_color.g,
            b: self.text_color.b,
            a: 0.2,
        };
        {
            let mut builder = PathBuilder::fill();
            let line_w = 1.0_f32;
            // Draw diagonal as a thin parallelogram
            builder.move_to(point(ox + px(db_to_x(-60.0)), oy + px(db_to_y(-60.0))));
            builder.line_to(point(
                ox + px(db_to_x(-60.0) + line_w),
                oy + px(db_to_y(-60.0)),
            ));
            builder.line_to(point(ox + px(db_to_x(0.0) + line_w), oy + px(db_to_y(0.0))));
            builder.line_to(point(ox + px(db_to_x(0.0)), oy + px(db_to_y(0.0))));
            if let Ok(path) = builder.build() {
                window.paint_path(path, unity_color);
            }
        }

        // Transfer curve — smooth line with filled area underneath
        let num_points = 64;
        let mut curve_points: Vec<(f32, f32)> = Vec::with_capacity(num_points + 1);

        for i in 0..=num_points {
            let input_db = -60.0 + (i as f64 / num_points as f64) * 60.0;
            let output_db = compute_transfer(
                input_db,
                self.threshold_db,
                self.ratio,
                self.knee_db,
                self.is_limiter,
            );
            curve_points.push((db_to_x(input_db), db_to_y(output_db)));
        }

        // Filled area under curve (accent color at low opacity)
        {
            let fill_color = Rgba {
                r: self.accent.r,
                g: self.accent.g,
                b: self.accent.b,
                a: 0.15,
            };
            let mut builder = PathBuilder::fill();
            // Start at bottom-left
            builder.move_to(point(ox + px(curve_points[0].0), oy + px(h - pad)));
            // Up to curve start
            builder.line_to(point(
                ox + px(curve_points[0].0),
                oy + px(curve_points[0].1),
            ));
            // Along the curve
            for &(cx_pt, cy_pt) in &curve_points[1..] {
                builder.line_to(point(ox + px(cx_pt), oy + px(cy_pt)));
            }
            // Down to bottom-right
            let last = curve_points.last().unwrap();
            builder.line_to(point(ox + px(last.0), oy + px(h - pad)));
            if let Ok(path) = builder.build() {
                window.paint_path(path, fill_color);
            }
        }

        // Curve stroke (thicker line on top of the fill)
        {
            let stroke_width = 2.0_f32;
            // Draw the curve as a thin filled band (PathBuilder doesn't have stroke)
            let mut builder = PathBuilder::fill();

            // Forward pass (top edge of the band)
            builder.move_to(point(
                ox + px(curve_points[0].0),
                oy + px(curve_points[0].1 - stroke_width / 2.0),
            ));
            for &(cx_pt, cy_pt) in &curve_points[1..] {
                builder.line_to(point(ox + px(cx_pt), oy + px(cy_pt - stroke_width / 2.0)));
            }
            // Backward pass (bottom edge of the band)
            for &(cx_pt, cy_pt) in curve_points.iter().rev() {
                builder.line_to(point(ox + px(cx_pt), oy + px(cy_pt + stroke_width / 2.0)));
            }

            if let Ok(path) = builder.build() {
                window.paint_path(path, self.accent);
            }
        }

        // Threshold indicator — vertical dashed line at threshold
        {
            let thresh_x = db_to_x(self.threshold_db);
            let thresh_color = Rgba {
                r: self.compressed_color.r,
                g: self.compressed_color.g,
                b: self.compressed_color.b,
                a: 0.5,
            };
            // Draw as a series of small rectangles (dashed effect)
            let dash_len = 4.0_f32;
            let gap_len = 3.0_f32;
            let mut y_cur = pad;
            while y_cur < h - pad {
                let seg_h = dash_len.min(h - pad - y_cur);
                window.paint_quad(PaintQuad {
                    bounds: Bounds {
                        origin: point(ox + px(thresh_x), oy + px(y_cur)),
                        size: size(px(1.0), px(seg_h)),
                    },
                    corner_radii: Corners::default(),
                    background: thresh_color.into(),
                    border_widths: Edges::default(),
                    border_color: gpui::transparent_black(),
                    border_style: BorderStyle::default(),
                });
                y_cur += dash_len + gap_len;
            }
        }

        // Operating point dot
        if let Some(input_db) = self.input_level_db {
            let output_db = compute_transfer(
                input_db,
                self.threshold_db,
                self.ratio,
                self.knee_db,
                self.is_limiter,
            );
            let dot_x = db_to_x(input_db);
            let dot_y = db_to_y(output_db);
            let dot_r = 5.0_f32;

            // Outer glow
            let glow_color = Rgba {
                r: self.operating_point_color.r,
                g: self.operating_point_color.g,
                b: self.operating_point_color.b,
                a: 0.3,
            };
            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(ox + px(dot_x - dot_r - 2.0), oy + px(dot_y - dot_r - 2.0)),
                    size: size(px((dot_r + 2.0) * 2.0), px((dot_r + 2.0) * 2.0)),
                },
                corner_radii: Corners::all(px(dot_r + 2.0)),
                background: glow_color.into(),
                border_widths: Edges::default(),
                border_color: gpui::transparent_black(),
                border_style: BorderStyle::default(),
            });

            // Inner dot
            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(ox + px(dot_x - dot_r), oy + px(dot_y - dot_r)),
                    size: size(px(dot_r * 2.0), px(dot_r * 2.0)),
                },
                corner_radii: Corners::all(px(dot_r)),
                background: self.operating_point_color.into(),
                border_widths: Edges::all(px(1.5)),
                border_color: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.8,
                }
                .into(),
                border_style: BorderStyle::default(),
            });

            // Crosshair lines from dot to axes
            let crosshair_color = Rgba {
                r: self.operating_point_color.r,
                g: self.operating_point_color.g,
                b: self.operating_point_color.b,
                a: 0.25,
            };
            // Vertical line (to x-axis) — only draw if there's space below the dot
            let vert_line_h = (h - pad) - dot_y - dot_r;
            if vert_line_h > 1.0 {
                window.paint_quad(PaintQuad {
                    bounds: Bounds {
                        origin: point(ox + px(dot_x), oy + px(dot_y + dot_r)),
                        size: size(px(1.0), px(vert_line_h)),
                    },
                    corner_radii: Corners::default(),
                    background: crosshair_color.into(),
                    border_widths: Edges::default(),
                    border_color: gpui::transparent_black(),
                    border_style: BorderStyle::default(),
                });
            }
            // Horizontal line (to y-axis) — only draw if there's space left of the dot
            let horiz_line_w = dot_x - pad - dot_r;
            if horiz_line_w > 1.0 {
                window.paint_quad(PaintQuad {
                    bounds: Bounds {
                        origin: point(ox + px(pad), oy + px(dot_y)),
                        size: size(px(horiz_line_w), px(1.0)),
                    },
                    corner_radii: Corners::default(),
                    background: crosshair_color.into(),
                    border_widths: Edges::default(),
                    border_color: gpui::transparent_black(),
                    border_style: BorderStyle::default(),
                });
            }
        }
    }
}

/// Render an interactive transfer curve where the user can drag to adjust threshold and ratio.
///
/// - **Vertical drag**: adjusts threshold (param at `threshold_param_idx`)
/// - **Horizontal drag**: adjusts ratio (param at `ratio_param_idx`)
/// - **Scroll wheel**: adjusts threshold
///
/// The curve itself is rendered by `TransferCurveElement` and wrapped in
/// a div with drag event handlers.
#[allow(clippy::too_many_arguments)]
pub fn render_interactive_transfer_curve<H: PluginViewHost>(
    entity: Entity<H>,
    plugin_idx: usize,
    threshold_db: f64,
    ratio: f64,
    knee_db: f64,
    is_limiter: bool,
    width: f32,
    input_level_db: Option<f64>,
    threshold_param_idx: usize,
    ratio_param_idx: usize,
    threshold_min: f64,
    threshold_max: f64,
    ratio_min: f64,
    ratio_max: f64,
    theme: &PluginViewTheme,
) -> impl IntoElement {
    let curve_width = width.max(200.0);
    let curve_height: f32 = 140.0;

    // Capture for drag closures
    let entity_drag = entity.clone();
    let entity_scroll = entity.clone();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .id(ElementId::Name(SharedString::from(format!(
                    "xfer-curve-{plugin_idx}"
                ))))
                .w(px(curve_width))
                .h(px(curve_height))
                .rounded_lg()
                .overflow_hidden()
                .cursor_pointer()
                // Drag to adjust threshold (horizontal) and ratio (vertical)
                .on_mouse_down(MouseButton::Left, {
                    let entity = entity.clone();
                    move |event, _window, cx| {
                        cx.stop_propagation();
                        // Store start position for drag delta calculation
                        let start_x: f32 = event.position.x.into();
                        let start_y: f32 = event.position.y.into();
                        entity.update(cx, |host, _| {
                            // Reuse knob_drag fields as scratch storage for the drag:
                            // min = start_x, max = start ratio
                            host.on_knob_drag_start(
                                plugin_idx,
                                0, // param_idx unused for transfer curve drag
                                start_y,
                                threshold_db,
                                start_x as f64,
                                ratio,
                            );
                        });
                    }
                })
                .on_mouse_move(move |event, _window, cx| {
                    if event.pressed_button != Some(MouseButton::Left) {
                        return;
                    }
                    entity_drag.update(cx, |host, _| {
                        let (
                            is_dragging,
                            drag_plugin_idx,
                            start_y,
                            start_threshold,
                            start_x_f64,
                            start_ratio,
                        ) = host.knob_drag_state();
                        if !is_dragging || drag_plugin_idx != plugin_idx {
                            return;
                        }
                        let start_x = start_x_f64 as f32;

                        let current_x: f32 = event.position.x.into();
                        let current_y: f32 = event.position.y.into();

                        let dy = current_y - start_y;
                        let dx = current_x - start_x;

                        // Vertical drag → threshold (drag up = higher threshold)
                        let new_threshold =
                            (start_threshold - dy as f64 * 0.3).clamp(threshold_min, threshold_max);

                        host.set_plugin_param(plugin_idx, threshold_param_idx, new_threshold);

                        // Horizontal drag → ratio (drag right = higher ratio)
                        if !is_limiter {
                            let new_ratio =
                                (start_ratio + dx as f64 * 0.05).clamp(ratio_min, ratio_max);
                            host.set_plugin_param(plugin_idx, ratio_param_idx, new_ratio);
                        }
                    });
                })
                .on_mouse_up(MouseButton::Left, {
                    let entity = entity.clone();
                    move |_, _, cx| {
                        entity.update(cx, |host, _| {
                            host.on_knob_drag_end();
                        });
                    }
                })
                // Scroll wheel adjusts threshold
                .on_scroll_wheel(move |event, _window, cx| {
                    cx.stop_propagation();
                    entity_scroll.update(cx, |host, _| {
                        let delta: f32 = match event.delta {
                            ScrollDelta::Pixels(d) => {
                                let y_px: f32 = d.y.into();
                                -y_px * 0.1
                            }
                            ScrollDelta::Lines(d) => -(d.y) * 1.0,
                        };
                        let new_threshold =
                            (threshold_db + delta as f64).clamp(threshold_min, threshold_max);
                        host.set_plugin_param(plugin_idx, threshold_param_idx, new_threshold);
                    });
                })
                .child(TransferCurveElement {
                    width: curve_width,
                    height: curve_height,
                    threshold_db,
                    ratio,
                    knee_db,
                    is_limiter,
                    input_level_db,
                    accent: theme.accent,
                    compressed_color: theme.meter_clip,
                    operating_point_color: theme.warning,
                    bg: theme.background,
                    grid_color: theme.border,
                    text_color: theme.text_muted,
                }),
        )
        // X-axis labels
        .child(
            div()
                .flex()
                .justify_between()
                .w(px(curve_width))
                .text_xs()
                .text_color(theme.text_muted)
                .child("-60 dB")
                .child("0 dB"),
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
    div().flex().flex_col().gap_4().child(
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
/// Uses `Entity<H>` for direct state updates via `PluginViewHost`
pub fn render_knob<H: PluginViewHost>(
    entity: Entity<H>,
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
    theme: &PluginViewTheme,
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
pub fn render_knob_sized<H: PluginViewHost>(
    entity: Entity<H>,
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
    theme: &PluginViewTheme,
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
        .design_tokens(theme.design_tokens.clone())
        .on_change({
            let entity = entity.clone();
            move |new_value, _, cx| {
                entity.update(cx, |host, _| {
                    host.set_plugin_param(plugin_idx, idx, new_value);
                });
            }
        })
        .on_drag_start({
            let entity = entity.clone();
            move |start_y, start_value, _, cx| {
                entity.update(cx, |host, _| {
                    host.on_knob_drag_start(plugin_idx, idx, start_y, start_value, min, max);
                });
            }
        })
        .on_select({
            let entity = entity.clone();
            move |_, cx| {
                entity.update(cx, |host, _| {
                    host.set_editing_plugin(plugin_idx);
                    host.set_selected_param(plugin_idx, idx);
                });
            }
        })
        .on_reset({
            let entity = entity.clone();
            move |_, cx| {
                entity.update(cx, |host, _| {
                    host.reset_plugin_param(plugin_idx, idx);
                });
            }
        });

    if let Some(key) = shortcut_key {
        knob = knob.shortcut_key(key);
    }

    div().key_context("plugin-control").child(knob)
}
