//! Limiter Plugin UI Component
//!
//! Brick-wall limiter with:
//! - Transfer curve display
//! - Gain reduction meter
//! - Peak meter with ceiling indicator
//! - Rotary knob controls

use super::common::{render_edit_hints, render_knob, render_section_header, render_transfer_curve};
use super::level_meters::{render_gr_meter, render_peak_meter};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Divider, HStack, StackAlign, StackSpacing, Text, TextSize, VStack};
use sotf_audio_player::param_specs::limiter::*;

/// State for rendering the Limiter plugin
pub struct LimiterRenderState {
    pub threshold_db: f64,
    pub release_ms: f64,
    pub mix: f64,
    pub is_editing: bool,
    pub selected_param: usize,
}

/// Render the Limiter plugin
pub fn render_limiter_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: LimiterRenderState,
    theme: &Theme,
) -> impl IntoElement {
    // Simulated values (in real implementation, these would come from the audio engine)
    let simulated_gr = if state.threshold_db < -1.0 {
        (state.threshold_db + 1.0) * 2.0
    } else {
        0.0
    };
    let simulated_peak = state.threshold_db - 3.0; // Simulated peak level below ceiling

    // Cache theme colors for closures
    let peak_color = if simulated_peak > state.threshold_db {
        theme.error
    } else {
        theme.success
    };
    let border_color = theme.border;

    VStack::new()
        .spacing(StackSpacing::Lg)
        // Main section - Knobs, Transfer Curve and Peak Meter
        .child(
            HStack::new()
                .spacing(StackSpacing::Lg)
                .align(StackAlign::Start)
                // Transfer curve
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(render_transfer_curve(
                            state.threshold_db,
                            f64::INFINITY,
                            0.0,
                            true,
                            theme,
                        ))
                        .build()
                        .rounded_xl()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .p_4(),
                )
                // Peak meter
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(render_peak_meter(simulated_peak, state.threshold_db, theme))
                        .build()
                        .rounded_xl()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .p_4(),
                )
                // Parameters section with knobs
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .child(render_section_header("LIMITER SETTINGS", theme))
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Md)
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "Ceiling",
                                    state.threshold_db,
                                    THRESHOLD_MIN as f64,
                                    THRESHOLD_MAX as f64,
                                    "dB",
                                    0,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('c'),
                                    theme,
                                ))
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "Release",
                                    state.release_ms,
                                    RELEASE_MIN as f64,
                                    RELEASE_MAX as f64,
                                    "ms",
                                    1,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('r'),
                                    theme,
                                ))
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "Mix",
                                    state.mix,
                                    MIX_MIN as f64,
                                    MIX_MAX as f64,
                                    "%",
                                    2,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('m'),
                                    theme,
                                ))
                                .build()
                                .justify_center(),
                        )
                        .build()
                        .flex_1()
                        .rounded_xl()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .p_4(),
                ),
        )
        // Large ceiling display
        .child(
            HStack::new()
                .spacing(StackSpacing::Xl)
                .child(
                    VStack::new()
                        .align(StackAlign::Center)
                        .child(
                            Text::new("CEILING")
                                .size(TextSize::Xs)
                                .color(theme.text_muted),
                        )
                        .child(
                            div()
                                .text_3xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.warning)
                                .child(format!("{:.2} dB", state.threshold_db)),
                        ),
                )
                .child(
                    Divider::vertical()
                        .color(border_color)
                        .build_simple()
                        .h(px(40.0)),
                )
                .child(
                    VStack::new()
                        .align(StackAlign::Center)
                        .child(
                            Text::new("TRUE PEAK")
                                .size(TextSize::Xs)
                                .color(theme.text_muted),
                        )
                        .child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(peak_color)
                                .child(format!("{:.1} dB", simulated_peak)),
                        ),
                )
                .build()
                .justify_center()
                .p_4()
                .rounded_xl()
                .bg(theme.background_secondary)
                .border_1()
                .border_color(theme.border),
        )
        // Gain reduction meter
        .child(
            div()
                .rounded_xl()
                .bg(theme.background_secondary)
                .border_1()
                .border_color(theme.border)
                .p_4()
                .child(render_gr_meter(simulated_gr, -20.0, theme)),
        )
}
