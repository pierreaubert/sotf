use super::actions::{ToggleUpmixerConfig, UpdatePluginParam};
use super::common::{
    render_edit_hints, render_knob, render_param_row, render_toggle, render_vertical_slider,
};
use crate::theme::Theme;
use gpui::*;
use gpui_ui_kit::{
    Divider, HStack, Select, SelectOption, SelectSize, StackAlign, StackSpacing, Text, TextSize,
    TextWeight, VStack, Toggle, ToggleStyle,
};

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
    config_open: bool,
) -> impl IntoElement {
    // Parameter Indices (must match plugin_editing.rs):
    // 0: Speaker Config
    // 1: gain_front_direct (not shown in UI)
    // 2: gain_front_ambient (Center Gain)
    // 3: gain_rear_ambient (Surround Gain)
    // 4: lfe_cutoff_hz (LFE Cutoff)
    // 5: stereo_width (not shown in UI)
    // 6: bandpass_hz (Bandpass Center)
    // 7: height_gain (Top Gain)
    // 8: lfe_gain (LFE Gain)
    // 9: enable_subharmonic_synth (toggle)
    // 10: subharmonic_gain (not shown in UI)
    // 11: enable_hr_direct (toggle)
    // 12: hr_sharpen (not shown in UI)
    // 13: safety_cap_db (Safety Cap)
    // 14: decorrelation_mode (0=Velvet/1=LFO)

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

    VStack::new()
        .spacing(StackSpacing::None)
        .child(
            // Top toolbar row
            HStack::new()
                .spacing(StackSpacing::Md)
                .build()
                .px_3()
                .py_2()
                .bg(theme.background_secondary)
                .border_b_1()
                .border_color(theme.border)
                .justify_between()
                // Speaker Config
                .child(
                    div().w(px(140.0)).child(
                        Select::new("config-select")
                            .options(
                                [
                                    "2.0", "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2",
                                    "7.1.4", "9.1.4", "9.1.6",
                                ]
                                .iter()
                                .map(|c| SelectOption::new(c.to_string(), c.to_string()))
                                .collect(),
                            )
                            .selected(speaker_config_owned.clone())
                            .is_open(config_open)
                            .label("Config")
                            .size(SelectSize::Sm)
                            .on_toggle(move |is_open, _window, cx| {
                                cx.dispatch_action(&ToggleUpmixerConfig { open: is_open })
                            })
                            .on_change(move |value, _, cx| {
                                let configs = [
                                    "2.0", "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2",
                                    "7.1.4", "9.1.4", "9.1.6",
                                ];
                                let idx = configs.iter().position(|&c| c == value.as_ref()).unwrap_or(0);
                                cx.dispatch_action(&UpdatePluginParam {
                                    plugin_idx,
                                    param_idx: 0,
                                    value: idx as f64,
                                });
                                cx.dispatch_action(&ToggleUpmixerConfig { open: false });
                            }),
                    ),
                )
                // Separator
                .child(Divider::vertical().color(theme.border).build_simple().h(px(16.0)))
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
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .child(Text::new("Decorrelation:").size(TextSize::Xs).color(theme.text_secondary))
                        .child(render_param_row(
                            "",
                            if decorrelation_mode == 0 { "Velvet" } else { "LFO" },
                            14,
                            selected_param,
                            is_editing,
                            theme,
                        ))
                        .build()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            cx.dispatch_action(&UpdatePluginParam {
                                plugin_idx,
                                param_idx: 14,
                                value: if decorrelation_mode == 0 { 1.0 } else { 0.0 },
                            });
                        }),
                ),
        )
        // Main Row Container
        .child(
            HStack::new()
                .spacing(StackSpacing::Lg)
                .align(StackAlign::Start)

                // 2. Gains (Center, LFE, Surround, Top)
                // Indices must match plugin_editing.rs
                .child(render_vertical_slider(
                    plugin_idx,
                    "Center",
                    gain_front_ambient,
                    -12.0,
                    12.0,
                    "dB",
                    2, // gain_front_ambient
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
                    8, // lfe_gain
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
                    3, // gain_rear_ambient
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
                    7, // height_gain
                    selected_param,
                    is_editing,
                    Some('t'),
                    theme,
                ))
                // 3. Knobs
                // Indices must match plugin_editing.rs
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Lg)
                        .child(render_knob(
                            plugin_idx,
                            "LFE Cut",
                            lfe_cutoff_hz,
                            20.0,
                            180.0,
                            "Hz",
                            4, // lfe_cutoff_hz
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
                            6, // bandpass_hz
                            selected_param,
                            is_editing,
                            None,
                            theme,
                        )),
                )
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Lg)
                        .child(render_knob(
                            plugin_idx,
                            "Safety",
                            safety_cap_db,
                            0.0,
                            3.0,
                            "dB",
                            13, // safety_cap_db
                            selected_param,
                            is_editing,
                            None,
                            theme,
                        )),
                )

                .build()
                .p_2(),
        )
        // Edit Hint Bar
        .child(render_edit_hints(theme))
        .build()
        .w_full()
        .h_full()
}


