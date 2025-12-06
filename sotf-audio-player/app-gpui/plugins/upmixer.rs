use super::actions::UpdatePluginParam;
use super::common::{
    render_edit_hints, render_knob, render_param_row, render_toggle, render_vertical_slider,
};
use crate::theme::Theme;
use gpui::*;

/// Render the upmixer plugin controls
///
/// Layout:
/// - Top: Config selector, Toggles (Sub, HR)
/// - Main: Input L/R, Gains (C, LFE, S, T), Knobs (LFE Cutoff, Bandpass, Safety), Decorrelation
pub fn render_upmixer_plugin(
    plugin_idx: usize,
    speaker_config: &str,
    _gain_front_direct: f64,
    _stereo_width: f64,
    gain_front_ambient: f64,
    gain_rear_ambient: f64,
    lfe_cutoff_hz: f64,
    bandpass_hz: f64,
    height_gain: f64,
    lfe_gain: f64,
    enable_subharmonic_synth: bool,
    _subharmonic_gain: f64,
    enable_hr_direct: bool,
    _hr_sharpen: f64,
    safety_cap_db: f64,
    decorrelation_mode: usize,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
) -> impl IntoElement {
    // Parameter Indices:
    // 0: Speaker Config
    // 1: Center Gain
    // 2: LFE Gain
    // 3: Surround Gain
    // 4: Top Gain
    // 5: LFE Cutoff
    // 6: Bandpass Center
    // 7: Safety Cap
    // 8: Input Level L (read-only)
    // 9: Subharmonic Synth (0/1)
    // 10: Input Level R (read-only)
    // 11: HR Direct (0/1)
    // 12: Output Level L
    // 13: Output Level R
    // 14: Decorrelation Mode (0=Velvet/1=LFO)

    // Note: Some params like gain_front_direct might be used for future visualizers
    // so we keep them in signature but underscore if unused for now.

    let _config_idx = match speaker_config {
        "2.0" => 0,
        "5.0" => 1,
        "5.1" => 2,
        "7.1" => 3,
        "5.1.2" => 4,
        "5.1.4" => 5,
        "7.1.2" => 6,
        "7.1.4" => 7,
        "9.1.4" => 8,
        "9.1.6" => 9,
        _ => 0,
    };

    // We need to own the string for the closure
    let speaker_config_owned = speaker_config.to_string();

    div()
        .flex()
        .flex_col()
        .w_full()
        .h_full()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px_3()
                .py_2()
                .bg(theme.background_secondary)
                .border_b_1()
                .border_color(theme.border)
                // Speaker Config
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .items_center()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            let configs = [
                                "2.0", "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4",
                                "9.1.4", "9.1.6",
                            ];
                            let current_idx = configs
                                .iter()
                                .position(|&c| c == speaker_config_owned)
                                .unwrap_or(0);
                            let next_idx = (current_idx + 1) % configs.len();
                            cx.dispatch_action(&UpdatePluginParam {
                                plugin_idx,
                                param_idx: 0,
                                value: next_idx as f64,
                            });
                        })
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_secondary)
                                .child("Config:"),
                        )
                        .child(render_param_row(
                            "",
                            speaker_config,
                            0,
                            selected_param,
                            is_editing,
                            theme,
                        )),
                )
                // Separator
                .child(div().w(px(1.0)).h(px(16.0)).bg(theme.border))
                // Subharmonic Synth Toggle
                .child(render_toggle(
                    plugin_idx,
                    "SubHarm",
                    enable_subharmonic_synth,
                    9,
                    selected_param,
                    is_editing,
                    theme,
                ))
                // HR Direct Toggle
                .child(render_toggle(
                    plugin_idx,
                    "HR Direct",
                    enable_hr_direct,
                    11,
                    selected_param,
                    is_editing,
                    theme,
                ))
                // Decorrelation Mode
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .items_center()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            cx.dispatch_action(&UpdatePluginParam {
                                plugin_idx,
                                param_idx: 14,
                                value: if decorrelation_mode == 0 { 1.0 } else { 0.0 },
                            });
                        })
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_secondary)
                                .child("Decorrelation:"),
                        )
                        .child(render_param_row(
                            "",
                            if decorrelation_mode == 0 {
                                "Velvet"
                            } else {
                                "LFO"
                            },
                            14,
                            selected_param,
                            is_editing,
                            theme,
                        )),
                ),
        )
        // Main Row Container
        .child(
            div()
                .flex()
                .items_start() // Make all children same height
                .gap_4()
                .p_2()
                // 1. Input Levels (Stereo) - Same size as Gains
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_2()
                        .rounded_lg()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_secondary)
                                .child("Input"),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .h_full()
                                // Placeholder for input levels as they are not available in PluginSettings directly
                                .child(render_level_meter("L", -60.0, theme))
                                .child(render_level_meter("R", -60.0, theme)),
                        ),
                )
                // 2. Gains (Center, LFE, Surround, Top)
                // Use Some('c') for shortcut display. Need to map others.
                .child(render_vertical_slider(
                    plugin_idx,
                    "Center",
                    gain_front_ambient,
                    -12.0,
                    12.0,
                    "dB",
                    1,
                    selected_param,
                    is_editing,
                    Some('c'),
                    theme,
                ))
                .child(render_vertical_slider(
                    plugin_idx,
                    "LFE",
                    lfe_gain,
                    -12.0,
                    12.0,
                    "dB",
                    2,
                    selected_param,
                    is_editing,
                    Some('l'),
                    theme,
                ))
                .child(render_vertical_slider(
                    plugin_idx,
                    "Surr",
                    gain_rear_ambient,
                    -12.0,
                    12.0,
                    "dB",
                    3,
                    selected_param,
                    is_editing,
                    Some('s'),
                    theme,
                ))
                .child(render_vertical_slider(
                    plugin_idx,
                    "Top",
                    height_gain,
                    -12.0,
                    12.0,
                    "dB",
                    4,
                    selected_param,
                    is_editing,
                    Some('t'),
                    theme,
                ))
                // 3. Knobs
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(render_knob(
                            plugin_idx,
                            "LFE Cut",
                            lfe_cutoff_hz,
                            20.0,
                            180.0,
                            "Hz",
                            5,
                            selected_param,
                            is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            plugin_idx,
                            "Bandpass",
                            bandpass_hz,
                            150.0,
                            350.0,
                            "Hz",
                            6,
                            selected_param,
                            is_editing,
                            None,
                            theme,
                        )),
                )
                .child(div().flex().flex_col().gap_4().child(render_knob(
                    plugin_idx,
                    "Safety",
                    safety_cap_db,
                    0.0,
                    3.0,
                    "dB",
                    7,
                    selected_param,
                    is_editing,
                    None,
                    theme,
                )))
                // 4. Output Levels
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_2()
                        .rounded_lg()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_secondary)
                                .child("Output"),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .h_full()
                                .child(render_level_meter("L", -60.0, theme))
                                .child(render_level_meter("R", -60.0, theme)),
                        ),
                ),
        )
        // Edit Hint Bar
        .child(render_edit_hints(theme))
}

fn render_level_meter(label: &str, level_db: f64, theme: &Theme) -> impl IntoElement {
    let clamped = (level_db + 60.0) / 60.0; // Map -60..0 to 0..1
    let pct = clamped.clamp(0.0, 1.0) as f32; // 0..1

    div()
        .flex()
        .flex_col()
        .items_center()
        .gap_1()
        .h_full()
        .child(
            div()
                .w(px(8.0))
                .h(px(100.0)) // Fixed height matching sliders
                .bg(theme.background)
                .rounded_sm()
                .child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(relative(pct)) // Relative height (0.0 - 1.0)
                        .bg(if level_db > -0.1 {
                            theme.error
                        } else {
                            theme.success
                        })
                        .rounded_sm(),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.text_muted)
                .child(label.to_string()),
        )
}
