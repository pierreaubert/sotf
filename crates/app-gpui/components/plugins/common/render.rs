use super::TransferCurveElement;
use super::param_section_style::ParamSectionStyle;
use crate::app::AppState;
use crate::app::i18n::PluginCommonTranslations;
use crate::app::state::app::KnobDragState;
use crate::components::design::Ds;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::{
    AnyElement, Div, ElementId, Entity, FocusHandle, FontWeight, MouseButton, ScrollDelta,
    SharedString, div, px, rems,
};
use gpui_audio_kit::PotentiometerSize;
use gpui_ui_kit::{
    ButtonSet, ButtonSetOption, ButtonSetSize, NumberInput, NumberInputSize, Slider, SliderSize,
};
use sotf_audio_player_midi::PhysicalControlKind;
use sotf_audio_player_midi::mapping::{MidiOverlay, ParamAssignment};
use std::sync::atomic::{AtomicBool, Ordering};

static WARNED_INVALID_AUDIO_CONTROL_RANGE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
struct SanitizedControlRange {
    value: f64,
    min: f64,
    max: f64,
}

#[expect(
    clippy::float_cmp,
    reason = "Exact parameter choices and changed-value detection preserve control semantics"
)]
fn sanitize_audio_control_range(
    label: &str,
    value: f64,
    min: f64,
    max: f64,
) -> SanitizedControlRange {
    let (mut normalized_min, mut normalized_max) = if min.is_finite() && max.is_finite() {
        (min.min(max), min.max(max))
    } else if value.is_finite() {
        (value - 1.0, value + 1.0)
    } else {
        (0.0, 1.0)
    };

    if normalized_min == normalized_max {
        let pad = normalized_min.abs().max(1.0) * 0.01;
        normalized_min -= pad;
        normalized_max += pad;
    }

    let normalized_value = if value.is_finite() {
        value.clamp(normalized_min, normalized_max)
    } else {
        normalized_min
    };

    let changed = normalized_value != value || normalized_min != min || normalized_max != max;
    if changed && !WARNED_INVALID_AUDIO_CONTROL_RANGE.swap(true, Ordering::Relaxed) {
        log::warn!(
            "Invalid audio control range for '{label}': value={value}, min={min}, max={max}; using value={normalized_value}, min={normalized_min}, max={normalized_max}"
        );
    }

    SanitizedControlRange {
        value: normalized_value,
        min: normalized_min,
        max: normalized_max,
    }
}

/// Render a parameter row with name, value, and optional range hint.
///
/// When `range_hint` is `Some("0.0 — 100.0")` and the row is selected,
/// the range is displayed as muted text beneath the value.
#[must_use]
pub fn render_param_row(
    d: &Ds,
    name: &str,
    value: &str,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &Theme,
    range_hint: Option<&str>,
) -> impl IntoElement {
    let is_selected = selected_param == idx && is_editing;

    div()
        .flex()
        .items_center()
        .justify_between()
        .px(d.pad_x)
        .py(d.pad_y)
        .rounded(d.r_lg)
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
                .text_size(d.text_sm)
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
                        .px(d.pad_y)
                        .py(d.pad_y_half)
                        .rounded(d.r_md)
                        .bg(if is_selected {
                            theme.background
                        } else {
                            theme.background_secondary
                        })
                        .text_size(d.text_sm)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child(value.to_string()),
                )
                .when(is_selected && range_hint.is_some(), |el| {
                    el.child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .px(d.pad_y)
                            .child(range_hint.unwrap_or("").to_string()),
                    )
                }),
        )
}

/// Render a parameter row with name, value, and optional MIDI assignment badge
#[must_use]
pub fn render_param_row_with_midi(
    d: &Ds,
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
        .px(d.pad_x)
        .py(d.pad_y)
        .rounded(d.r_lg)
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
                .gap(d.gap)
                .child(
                    div()
                        .text_size(d.text_sm)
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
                        .map(|assignment| render_midi_badge(d, assignment, theme)),
                ),
        )
        // Value
        .child(
            div()
                .min_w(rems(5.0))
                .px(d.pad_y)
                .py(d.pad_y_half)
                .rounded(d.r_md)
                .bg(if is_selected {
                    theme.background
                } else {
                    theme.background_secondary
                })
                .text_size(d.text_sm)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_primary)
                .child(value.to_string()),
        )
}

/// Render a small MIDI control badge (e.g., "K1", "F3") next to a parameter name
#[must_use]
pub fn render_midi_badge(d: &Ds, assignment: &ParamAssignment, theme: &Theme) -> impl IntoElement {
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
        .gap(d.half_grid)
        .px(d.grid)
        // intentional: 1px vertical inset for compact badge — do not scale
        .py(px(1.0))
        .rounded(d.r_sm)
        .bg(Theme::with_opacity(badge_color, 0.2))
        .child(
            div()
                .text_size(d.text_xs)
                .text_color(badge_color)
                .child(icon.to_string()),
        )
        .child(
            div()
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(badge_color)
                .child(assignment.control_label.clone()),
        )
}

/// Render a MIDI page indicator (e.g., "Page 1/3")
#[must_use]
pub fn render_midi_page_indicator(
    d: &Ds,
    current_page: usize,
    total_pages: usize,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(d.grid)
        .px(d.pad_y)
        .py(d.pad_y_half)
        .rounded(d.r_md)
        .bg(theme.surface)
        .child(
            div()
                .text_size(d.text_xs)
                .text_color(theme.text_muted)
                .child(format!("MIDI {}/{}", current_page + 1, total_pages)),
        )
}

/// Render a section header (with bottom margin - use for bordered sections)
#[must_use]
pub fn render_section_header(d: &Ds, title: &str, theme: &Theme) -> impl IntoElement {
    div()
        .text_size(d.text_sm)
        .font_weight(FontWeight::BOLD)
        .text_color(theme.text_primary)
        .mb(d.gap)
        .child(title.to_string())
}

/// Render a compact section title with a ruled line extending to the right edge.
///
/// ```text
/// DYNAMICS ─────────────────
/// ```
#[must_use]
pub fn render_section_title(d: &Ds, title: &str, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(d.gap)
        .child(
            div()
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_secondary)
                .flex_shrink_0()
                .child(title.to_string()),
        )
        .child(div().flex_1().h(px(1.0)).bg(theme.border)) // intentional: one-pixel separator rule
}

/// Create a new parameter section container with flex column layout
#[must_use]
pub fn render_param_section(d: &Ds, theme: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(d.gap)
        .param_section_style(d, theme)
}

/// Create a new parameter section container with flex column layout and larger padding
#[must_use]
pub fn render_param_section_lg(d: &Ds, theme: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(d.gap)
        .param_section_style_lg(d, theme)
}

/// Render keyboard hints for edit mode
#[must_use]
pub fn render_edit_hints(d: &Ds, theme: &Theme) -> impl IntoElement {
    div()
        .mt(d.section)
        .p(d.pad_x)
        .rounded(d.r_lg)
        .bg(theme.background_secondary)
        .border_1()
        .border_color(theme.border)
        .flex()
        .gap(d.section)
        .text_size(d.text_xs)
        .text_color(theme.text_muted)
        .child("↑/↓")
        .child("←/→")
        .child("[/]")
        .child("↵")
}

/// Render a toggle button using gpui-ui-kit Toggle component
/// Uses `Entity<AppState>` for direct state updates
#[must_use]
pub fn render_toggle(
    entity: Entity<AppState>,
    plugin_idx: usize,
    label: &str,
    enabled: bool,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &Theme,
    text: PluginCommonTranslations,
) -> impl IntoElement {
    render_toggle_enabled(
        entity,
        plugin_idx,
        label,
        enabled,
        true,
        idx,
        selected_param,
        is_editing,
        theme,
        text,
    )
}

/// Render a toggle whose interaction can be disabled by layout metadata.
#[allow(
    clippy::too_many_arguments,
    reason = "GPUI rendering and callback interfaces accept the complete UI context"
)]
#[must_use]
pub fn render_toggle_enabled(
    entity: Entity<AppState>,
    plugin_idx: usize,
    label: &str,
    checked: bool,
    interactive: bool,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &Theme,
    text: PluginCommonTranslations,
) -> impl IntoElement {
    let _ = (selected_param, is_editing);
    let toggle = ButtonSet::new(("plugin-toggle", plugin_idx * 1000 + idx))
        .options(vec![
            ButtonSetOption::new("0", text.label("Off")),
            ButtonSetOption::new("1", text.label("On")),
        ])
        .selected(if checked { "1" } else { "0" })
        .size(ButtonSetSize::Xs)
        .disabled(!interactive)
        .theme(theme.to_button_set_theme())
        .on_change(move |value, _, cx| {
            if let Ok(value) = value.parse::<f64>() {
                entity.update(cx, |state, cx| {
                    state.app.set_plugin_param(plugin_idx, idx, value);
                    cx.notify();
                });
            }
        });
    #[cfg(feature = "dev-api")]
    let toggle = {
        use crate::app::dev_api::{DevElementState, DevTrackExt};
        toggle.dev_track_with_state(
            format!("plugin.toggle.{plugin_idx}.{idx}"),
            DevElementState::default()
                .enabled(interactive)
                .selected(checked),
        )
    };
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_1()
        .child(gpui_ui_kit::Text::label(label.to_string()))
        .child(toggle)
}

/// Render an unlabeled compact exclusive Off/On control.
#[must_use]
pub fn render_toggle_button(
    entity: Entity<AppState>,
    plugin_idx: usize,
    enabled: bool,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &Theme,
    text: PluginCommonTranslations,
) -> impl IntoElement {
    render_toggle_enabled(
        entity,
        plugin_idx,
        "",
        enabled,
        true,
        idx,
        selected_param,
        is_editing,
        theme,
        text,
    )
}

/// Render a value with unit and color coding
#[must_use]
pub fn render_colored_value(
    d: &Ds,
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
        .text_size(d.text_sm)
        .font_weight(FontWeight::BOLD)
        .text_color(color)
        .child(format!("{value:+.1}{unit}"))
}

/// Render a vertical slider with label, value, drag support and enhanced visual feedback
/// Uses `Entity<AppState>` for direct state updates
#[must_use]
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
#[allow(
    clippy::too_many_arguments,
    reason = "GPUI rendering and callback interfaces accept the complete UI context"
)]
#[must_use]
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
    height: Option<f32>,
    theme: &Theme,
) -> AnyElement {
    let _ = height;
    render_knob_sized_enabled_with_focus(
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
        PotentiometerSize::Xs,
        true,
        None,
        theme,
    )
}

/// Render a vertical slider with tick marks, custom height, and enhanced visual feedback
/// Uses `Entity<AppState>` for direct state updates
#[allow(
    clippy::too_many_arguments,
    reason = "GPUI rendering and callback interfaces accept the complete UI context"
)]
#[must_use]
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
    render_vertical_slider_with_ticks_enabled(
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
        height,
        true,
        theme,
    )
}

/// Render a ticked slider whose interaction can be disabled by layout metadata.
#[allow(
    clippy::too_many_arguments,
    reason = "GPUI rendering and callback interfaces accept the complete UI context"
)]
#[must_use]
pub fn render_vertical_slider_with_ticks_enabled(
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
    interactive: bool,
    theme: &Theme,
) -> AnyElement {
    let _ = height;
    render_knob_sized_enabled_with_focus(
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
        PotentiometerSize::Xs,
        interactive,
        None,
        theme,
    )
}

/// Render a simple transfer curve visualization (input vs output)
#[must_use]
pub fn render_transfer_curve(
    d: &Ds,
    threshold_db: f64,
    ratio: f64,
    knee_db: f64,
    is_limiter: bool,
    theme: &Theme,
) -> impl IntoElement {
    render_transfer_curve_sized(d, threshold_db, ratio, knee_db, is_limiter, 200.0, theme)
}

/// Render a transfer curve visualization with custom width
#[must_use]
pub fn render_transfer_curve_sized(
    d: &Ds,
    threshold_db: f64,
    ratio: f64,
    knee_db: f64,
    is_limiter: bool,
    width: f32,
    theme: &Theme,
) -> impl IntoElement {
    render_transfer_curve_with_level(
        d,
        threshold_db,
        ratio,
        knee_db,
        is_limiter,
        width,
        None,
        theme,
    )
}

/// Render a transfer curve with optional input level indicator.
///
/// Uses a custom paint element for smooth curve rendering instead of bars.
/// When `input_level_db` is provided, draws an animated operating point dot
/// on the curve.
#[allow(
    clippy::too_many_arguments,
    reason = "GPUI rendering and callback interfaces accept the complete UI context"
)]
#[must_use]
pub fn render_transfer_curve_with_level(
    d: &Ds,
    threshold_db: f64,
    ratio: f64,
    knee_db: f64,
    is_limiter: bool,
    width: f32,
    input_level_db: Option<f64>,
    theme: &Theme,
) -> impl IntoElement {
    let curve_width = width.max(200.0);
    // Preserve the baseline 200x140 aspect ratio as callers scale the width
    // for zoom and available layout space.
    let curve_height = (curve_width * 0.7).clamp(120.0, 240.0);

    div()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(
            div()
                .w(px(curve_width))
                .h(px(curve_height))
                .rounded(d.r_lg)
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
                    compressed_color: theme.feedback.meter_clip,
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
                .text_size(d.text_xs)
                .text_color(theme.text_muted)
                .child("-60 dB")
                .child("0 dB"),
        )
}

/// Render an interactive transfer curve where the user can drag to adjust threshold and ratio.
///
/// - **Vertical drag**: adjusts threshold (param at `threshold_param_idx`)
/// - **Horizontal drag**: adjusts ratio (param at `ratio_param_idx`)
/// - **Scroll wheel**: adjusts threshold
///
/// The curve itself is rendered by `TransferCurveElement` and wrapped in
/// a div with drag event handlers.
#[allow(
    clippy::too_many_arguments,
    reason = "GPUI rendering and callback interfaces accept the complete UI context"
)]
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "Keep the declarative layout and its interaction handlers together"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
pub fn render_interactive_transfer_curve(
    d: &Ds,
    entity: Entity<AppState>,
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
    theme: &Theme,
) -> impl IntoElement {
    let curve_width = width.max(200.0);
    let curve_height = (curve_width * 0.7).clamp(120.0, 240.0);

    // Capture for drag closures
    let entity_drag = entity.clone();
    let entity_scroll = entity.clone();

    div()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(
            div()
                .id(ElementId::Name(SharedString::from(format!(
                    "xfer-curve-{plugin_idx}"
                ))))
                .w(px(curve_width))
                .h(px(curve_height))
                .rounded(d.r_lg)
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
                        entity.update(cx, |state, _| {
                            state.app.drag.knob_drag = Some(KnobDragState {
                                plugin_idx,
                                param_idx: 0,
                                start_y,
                                start_value: threshold_db,
                                min: f64::from(start_x),
                                max: ratio,
                            });
                        });
                    }
                })
                .on_mouse_move(move |event, _window, cx| {
                    if event.pressed_button != Some(MouseButton::Left) {
                        return;
                    }
                    entity_drag.update(cx, |state, _| {
                        let Some(ref drag) = state.app.drag.knob_drag else {
                            return;
                        };
                        if drag.plugin_idx != plugin_idx {
                            return;
                        }
                        let start_y = drag.start_y;
                        let start_x = drag.min as f32;
                        let start_threshold = drag.start_value;
                        let start_ratio = drag.max;

                        let current_x: f32 = event.position.x.into();
                        let current_y: f32 = event.position.y.into();

                        let dy = current_y - start_y;
                        let dx = current_x - start_x;

                        // Vertical drag → threshold (drag up = higher threshold)
                        let new_threshold = (start_threshold - f64::from(dy) * 0.3)
                            .clamp(threshold_min, threshold_max);

                        state
                            .app
                            .set_plugin_param(plugin_idx, threshold_param_idx, new_threshold);

                        // Horizontal drag → ratio (drag right = higher ratio)
                        if !is_limiter {
                            let new_ratio =
                                (start_ratio + f64::from(dx) * 0.05).clamp(ratio_min, ratio_max);
                            state
                                .app
                                .set_plugin_param(plugin_idx, ratio_param_idx, new_ratio);
                        }
                    });
                })
                .on_mouse_up(MouseButton::Left, {
                    let entity = entity.clone();
                    move |_, _, cx| {
                        entity.update(cx, |state, _| {
                            state.app.drag.knob_drag = None;
                        });
                    }
                })
                // Scroll wheel adjusts threshold
                .on_scroll_wheel(move |event, _window, cx| {
                    cx.stop_propagation();
                    entity_scroll.update(cx, |state, _| {
                        let delta: f32 = match event.delta {
                            ScrollDelta::Pixels(d) => {
                                let y_px: f32 = d.y.into();
                                -y_px * 0.1
                            }
                            ScrollDelta::Lines(d) => -(d.y) * 1.0,
                        };
                        let new_threshold =
                            (threshold_db + f64::from(delta)).clamp(threshold_min, threshold_max);
                        state
                            .app
                            .set_plugin_param(plugin_idx, threshold_param_idx, new_threshold);
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
                    compressed_color: theme.feedback.meter_clip,
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
                .text_size(d.text_xs)
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
    d: &Ds,
    transfer_curve: impl IntoElement,
    controls: impl IntoElement,
    meter_section: impl IntoElement,
    meter_width: f32,
) -> impl IntoElement {
    div().flex().flex_col().gap(d.section).child(
        div()
            .flex()
            .gap(d.section)
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

/// Render a rotary knob control using gpui-audio-kit Potentiometer
/// Uses `Entity<AppState>` for direct state updates
#[must_use]
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
    render_knob_enabled(
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
        true,
        theme,
    )
}

/// Render a standard-size knob with an explicit interaction state.
#[allow(
    clippy::too_many_arguments,
    reason = "GPUI rendering and callback interfaces accept the complete UI context"
)]
#[must_use]
pub fn render_knob_enabled(
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
    interactive: bool,
    theme: &Theme,
) -> impl IntoElement {
    render_knob_sized_enabled(
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
        interactive,
        theme,
    )
}

/// Render a rotary knob control with custom size
#[must_use]
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
    render_knob_sized_enabled(
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
        size,
        true,
        theme,
    )
}

/// Render a custom-size knob whose selected exact editor owns a stable focus
/// handle supplied by the parent view.
#[allow(
    clippy::too_many_arguments,
    reason = "GPUI rendering and callback interfaces accept the complete UI context"
)]
#[must_use]
pub fn render_knob_sized_with_focus(
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
    exact_entry_focus_handle: FocusHandle,
    theme: &Theme,
) -> impl IntoElement {
    render_knob_sized_enabled_with_focus(
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
        size,
        true,
        Some(exact_entry_focus_handle),
        theme,
    )
}

/// Render a potentiometer whose interaction can be disabled by layout metadata.
#[allow(
    clippy::too_many_arguments,
    reason = "GPUI rendering and callback interfaces accept the complete UI context"
)]
#[must_use]
pub fn render_knob_sized_enabled(
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
    interactive: bool,
    theme: &Theme,
) -> AnyElement {
    render_knob_sized_enabled_with_focus(
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
        size,
        interactive,
        None,
        theme,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "GPUI rendering and callback interfaces accept the complete UI context"
)]
#[expect(
    clippy::too_many_lines,
    reason = "Keep the declarative layout and its interaction handlers together"
)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
fn render_knob_sized_enabled_with_focus(
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
    interactive: bool,
    exact_entry_focus_handle: Option<FocusHandle>,
    theme: &Theme,
) -> AnyElement {
    let is_selected = selected_param == idx && is_editing;
    let control_range = sanitize_audio_control_range(label, value, min, max);
    let value = control_range.value;
    let min = control_range.min;
    let max = control_range.max;
    let logarithmic = min > 0.0 && matches!(unit, "Hz" | "ms");
    let slider = Slider::new(("plugin-custom-slider", plugin_idx * 1000 + idx))
        .value(value as f32)
        .range(min as f32, max as f32)
        .logarithmic(logarithmic)
        .width(120.0)
        .size(SliderSize::Sm)
        .disabled(!interactive)
        .theme(theme.to_slider_theme())
        .aria_label(format!("{label}: {value} {unit}"))
        .on_drag_start({
            let entity = entity.clone();
            move |_, _, _, cx| {
                entity.update(cx, |state, cx| {
                    state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                    state.app.plugin_state.plugin_param_selection = idx;
                    cx.notify();
                });
            }
        })
        .on_change({
            let entity = entity.clone();
            move |value, _, cx| {
                let value = f64::from(value);
                entity.update(cx, |state, cx| {
                    state
                        .app
                        .set_plugin_param(plugin_idx, idx, value.clamp(min, max));
                    cx.notify();
                });
            }
        })
        .on_reset({
            let entity = entity.clone();
            move |_, cx| {
                entity.update(cx, |state, cx| {
                    state.app.reset_plugin_param(plugin_idx, idx);
                    cx.notify();
                });
            }
        });
    #[cfg(feature = "dev-api")]
    let slider = {
        use crate::app::dev_api::DevTrackExt;
        slider.dev_track(format!("plugin.slider.{plugin_idx}.{idx}"))
    };
    // Custom band editors supply display-space bounds rather than ParamSpec.
    // Preserve their existing range and use a fine, bounded exact-entry increment.
    let exact = NumberInput::new(SharedString::from(format!(
        "plugin-custom-number-{plugin_idx}-{idx}"
    )))
    .value(value)
    .min(min)
    .max(max)
    .step(((max - min) / 1000.0).max(0.0001))
    .decimals(if unit == "Hz" { 1 } else { 3 })
    .unit(unit.to_string())
    .size(NumberInputSize::Xs)
    .disabled(!interactive)
    .theme(theme.to_number_input_theme())
    .aria_label(format!("{label} value"))
    .on_change(move |value, _, cx| {
        entity.update(cx, |state, cx| {
            state.app.set_plugin_param(plugin_idx, idx, value);
            cx.notify();
        });
    });
    let exact = if is_selected {
        if let Some(handle) = exact_entry_focus_handle {
            exact.focus_handle(handle)
        } else {
            exact
        }
    } else {
        exact
    };
    #[cfg(feature = "dev-api")]
    let exact = {
        use crate::app::dev_api::DevTrackExt;
        exact.dev_track(format!("plugin.param.{plugin_idx}.{idx}.exact"))
    };
    let label = if let Some(key) = shortcut_key {
        format!("{label} ({key})")
    } else {
        label.to_string()
    };
    // Legacy callers retain their size argument while every plugin adopts the same row height.
    let _ = size;
    div()
        .flex()
        .flex_col()
        .gap_1()
        .w(rems(16.0))
        .max_w_full()
        .child(gpui_ui_kit::Text::label(label))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_1()
                .child(slider)
                .child(exact),
        )
        .into_any_element()
}
