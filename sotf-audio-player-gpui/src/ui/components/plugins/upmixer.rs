//! Upmixer Plugin UI Component
//!
//! Provides spatial audio upmixing visualization with:
//! - Speaker configuration display
//! - Level meters per channel
//! - Vertical slider controls for main parameters
//! - Advanced parameter controls

use super::common::{
    render_edit_hints, render_param_row, render_section_header, render_toggle, render_vertical_slider,
};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

/// Render a simple vertical level meter
fn render_level_meter(label: &str, level: f32, color: Rgba, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap_1()
        .child(
            div()
                .w(px(12.0))
                .h(px(60.0))
                .bg(theme.background)
                .rounded_sm()
                .border_1()
                .border_color(theme.border)
                .relative()
                .overflow_hidden()
                .child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(relative(level))
                        .bg(color),
                ),
        )
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme.text_muted)
                .child(label.to_string()),
        )
}

/// Render the Upmixer plugin
#[allow(clippy::too_many_arguments)]
pub fn render_upmixer_plugin(
    speaker_config: &str,
    gain_front_direct: f64,
    gain_front_ambient: f64,
    gain_rear_ambient: f64,
    lfe_cutoff_hz: f64,
    stereo_width: f64,
    bandpass_hz: f64,
    height_gain: f64,
    lfe_gain: f64,
    enable_subharmonic_synth: bool,
    subharmonic_gain: f64,
    enable_hr_direct: bool,
    hr_sharpen: f64,
    safety_cap_db: f64,
    decorrelation_mode: usize,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
) -> impl IntoElement {
    // Simulated output levels (would come from audio engine in real implementation)
    let fl_level = (gain_front_direct as f32 * 0.7).clamp(0.0, 1.0);
    let fr_level = (gain_front_direct as f32 * 0.75).clamp(0.0, 1.0);
    let c_level = (gain_front_direct as f32 * 0.5).clamp(0.0, 1.0);
    let lfe_level = (lfe_gain as f32 * 0.6).clamp(0.0, 1.0);
    let rl_level = (gain_rear_ambient as f32 * 0.4).clamp(0.0, 1.0);
    let rr_level = (gain_rear_ambient as f32 * 0.45).clamp(0.0, 1.0);

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Main section - Speaker Layout and Level Meters
        .child(
            div()
                .flex()
                .gap_4()
                // Speaker layout visualization
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .rounded_xl()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .p_4()
                        .child(render_section_header("SPEAKER LAYOUT", theme))
                        .child(
                            div()
                                .h(px(140.0))
                                .w(px(180.0))
                                .bg(theme.surface)
                                .rounded_lg()
                                .border_1()
                                .border_color(theme.border)
                                .relative()
                                .flex()
                                .items_center()
                                .justify_center()
                                // Center label
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(0x7c3aed))
                                        .child(speaker_config.to_string()),
                                )
                                // Front Left
                                .child(
                                    div()
                                        .absolute()
                                        .top(px(10.0))
                                        .left(px(15.0))
                                        .w(px(22.0))
                                        .h(px(22.0))
                                        .rounded_full()
                                        .bg(rgb(0x22c55e))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_xs()
                                        .text_color(rgb(0xffffff))
                                        .font_weight(FontWeight::BOLD)
                                        .child("FL"),
                                )
                                // Front Right
                                .child(
                                    div()
                                        .absolute()
                                        .top(px(10.0))
                                        .right(px(15.0))
                                        .w(px(22.0))
                                        .h(px(22.0))
                                        .rounded_full()
                                        .bg(rgb(0x22c55e))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_xs()
                                        .text_color(rgb(0xffffff))
                                        .font_weight(FontWeight::BOLD)
                                        .child("FR"),
                                )
                                // Center
                                .child(
                                    div()
                                        .absolute()
                                        .top(px(10.0))
                                        .left_0()
                                        .right_0()
                                        .flex()
                                        .justify_center()
                                        .child(
                                            div()
                                                .w(px(22.0))
                                                .h(px(22.0))
                                                .rounded_full()
                                                .bg(rgb(0x3b82f6))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_xs()
                                                .text_color(rgb(0xffffff))
                                                .font_weight(FontWeight::BOLD)
                                                .child("C"),
                                        ),
                                )
                                // Rear Left
                                .child(
                                    div()
                                        .absolute()
                                        .bottom(px(10.0))
                                        .left(px(15.0))
                                        .w(px(22.0))
                                        .h(px(22.0))
                                        .rounded_full()
                                        .bg(rgb(0xf59e0b))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_xs()
                                        .text_color(rgb(0xffffff))
                                        .font_weight(FontWeight::BOLD)
                                        .child("RL"),
                                )
                                // Rear Right
                                .child(
                                    div()
                                        .absolute()
                                        .bottom(px(10.0))
                                        .right(px(15.0))
                                        .w(px(22.0))
                                        .h(px(22.0))
                                        .rounded_full()
                                        .bg(rgb(0xf59e0b))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_xs()
                                        .text_color(rgb(0xffffff))
                                        .font_weight(FontWeight::BOLD)
                                        .child("RR"),
                                )
                                // LFE
                                .child(
                                    div()
                                        .absolute()
                                        .bottom(px(10.0))
                                        .left_0()
                                        .right_0()
                                        .flex()
                                        .justify_center()
                                        .child(
                                            div()
                                                .w(px(28.0))
                                                .h(px(22.0))
                                                .rounded_md()
                                                .bg(rgb(0xdc2626))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_xs()
                                                .text_color(rgb(0xffffff))
                                                .font_weight(FontWeight::BOLD)
                                                .child("LFE"),
                                        ),
                                ),
                        ),
                )
                // Level meters
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .rounded_xl()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .p_4()
                        .child(render_section_header("OUTPUT LEVELS", theme))
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .justify_center()
                                .child(render_level_meter("FL", fl_level, rgb(0x22c55e), theme))
                                .child(render_level_meter("C", c_level, rgb(0x3b82f6), theme))
                                .child(render_level_meter("FR", fr_level, rgb(0x22c55e), theme))
                                .child(render_level_meter("LFE", lfe_level, rgb(0xdc2626), theme))
                                .child(render_level_meter("RL", rl_level, rgb(0xf59e0b), theme))
                                .child(render_level_meter("RR", rr_level, rgb(0xf59e0b), theme)),
                        ),
                ),
        )
        // Main gain controls with vertical sliders
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .rounded_xl()
                .bg(theme.background_secondary)
                .border_1()
                .border_color(theme.border)
                .p_4()
                .child(render_section_header("CHANNEL GAINS", theme))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .justify_center()
                        .flex_wrap()
                        .child(render_vertical_slider(
                            "Direct",
                            gain_front_direct,
                            0.0,
                            2.0,
                            "",
                            1,
                            selected_param,
                            is_editing,
                            Some('d'),
                            theme,
                        ))
                        .child(render_vertical_slider(
                            "Ambient",
                            gain_front_ambient,
                            0.0,
                            2.0,
                            "",
                            2,
                            selected_param,
                            is_editing,
                            Some('f'),
                            theme,
                        ))
                        .child(render_vertical_slider(
                            "Rear",
                            gain_rear_ambient,
                            0.0,
                            2.0,
                            "",
                            3,
                            selected_param,
                            is_editing,
                            Some('r'),
                            theme,
                        ))
                        .child(render_vertical_slider(
                            "Width",
                            stereo_width,
                            0.0,
                            2.0,
                            "",
                            5,
                            selected_param,
                            is_editing,
                            Some('w'),
                            theme,
                        ))
                        .child(render_vertical_slider(
                            "LFE",
                            lfe_gain,
                            0.0,
                            2.0,
                            "",
                            7,
                            selected_param,
                            is_editing,
                            Some('l'),
                            theme,
                        ))
                        .child(render_vertical_slider(
                            "Height",
                            height_gain,
                            0.0,
                            2.0,
                            "",
                            6,
                            selected_param,
                            is_editing,
                            Some('h'),
                            theme,
                        )),
                ),
        )
        // LFE and frequency settings
        .child(
            div()
                .flex()
                .gap_4()
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .rounded_xl()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .p_3()
                        .child(render_section_header("FREQUENCY", theme))
                        .child(render_param_row(
                            "LFE Cutoff",
                            &format!("{:.0} Hz", lfe_cutoff_hz),
                            4,
                            selected_param,
                            is_editing,
                            theme,
                        ))
                        .child(render_param_row(
                            "Bandpass",
                            &format!("{:.0} Hz", bandpass_hz),
                            8,
                            selected_param,
                            is_editing,
                            theme,
                        )),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .rounded_xl()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .p_3()
                        .child(render_section_header("SAFETY", theme))
                        .child(render_param_row(
                            "Safety Cap",
                            &format!("{:.1} dB", safety_cap_db),
                            13,
                            selected_param,
                            is_editing,
                            theme,
                        ))
                        .child(render_param_row(
                            "Decorrelation",
                            &match decorrelation_mode {
                                0 => "None".to_string(),
                                1 => "Light".to_string(),
                                2 => "Medium".to_string(),
                                3 => "Heavy".to_string(),
                                _ => format!("{}", decorrelation_mode),
                            },
                            14,
                            selected_param,
                            is_editing,
                            theme,
                        )),
                ),
        )
        // Advanced toggles
        .child(
            div()
                .flex()
                .gap_4()
                .children([
                    div()
                        .flex_1()
                        .child(render_toggle(
                            "Subharmonic Synth",
                            enable_subharmonic_synth,
                            9,
                            selected_param,
                            is_editing,
                            theme,
                        )),
                    div()
                        .flex_1()
                        .child(render_toggle(
                            "HR Direct",
                            enable_hr_direct,
                            11,
                            selected_param,
                            is_editing,
                            theme,
                        )),
                ]),
        )
        // Additional parameters when subharmonic or HR is enabled
        .when(enable_subharmonic_synth || enable_hr_direct, |d| {
            d.child(
                div()
                    .flex()
                    .gap_4()
                    .children([
                        div()
                            .flex_1()
                            .when(enable_subharmonic_synth, |d| {
                                d.child(
                                    div()
                                        .rounded_xl()
                                        .bg(theme.background_secondary)
                                        .border_1()
                                        .border_color(theme.border)
                                        .p_3()
                                        .child(render_param_row(
                                            "Subharm Gain",
                                            &format!("{:.2}", subharmonic_gain),
                                            10,
                                            selected_param,
                                            is_editing,
                                            theme,
                                        )),
                                )
                            }),
                        div()
                            .flex_1()
                            .when(enable_hr_direct, |d| {
                                d.child(
                                    div()
                                        .rounded_xl()
                                        .bg(theme.background_secondary)
                                        .border_1()
                                        .border_color(theme.border)
                                        .p_3()
                                        .child(render_param_row(
                                            "HR Sharpen",
                                            &format!("{:.2}", hr_sharpen),
                                            12,
                                            selected_param,
                                            is_editing,
                                            theme,
                                        )),
                                )
                            }),
                    ]),
            )
        })
        // Keyboard hints
        .child(
            div()
                .p_3()
                .rounded_lg()
                .bg(theme.accent_muted)
                .border_1()
                .border_color(theme.accent)
                .flex()
                .flex_wrap()
                .gap_3()
                .text_xs()
                .text_color(theme.text_secondary)
                .child("[D]irect")
                .child("[F] Ambient")
                .child("[R]ear")
                .child("[W]idth")
                .child("[L]FE")
                .child("[H]eight")
                .child("1-6: Quick select"),
        )
        .when(is_editing, |d| d.child(render_edit_hints(theme)))
}
