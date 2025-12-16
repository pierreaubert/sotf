//! Upmixer Plugin UI Component
//!
//! Controls for the Upmixer plugin with:
//! - Speaker configuration selector
//! - Rotary knobs for gains and frequency controls
//! - Toggles for processing modes

use super::common::{render_knob, render_vertical_slider_with_ticks};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Divider, HStack, Select, SelectOption, SelectSize, StackAlign, StackSpacing, Toggle,
    ToggleStyle, VStack,
};
use sotf_audio_player::param_specs::upmixer::*;

/// Help text for upmixer parameters
mod help_text {
    pub const CONFIG: &str = "Output speaker configuration.\n2.0 = Stereo passthrough\n5.0/5.1 = Standard surround\n7.1 = Extended surround\nAtmos configs add height channels";
    pub const SUBHARMONIC: &str = "Subharmonic Synthesizer adds low bass content\nby generating frequencies one octave below\nthe original signal. Useful for small speakers\nor adding punch to bass-light content.";
    pub const HR_DIRECT: &str = "High Resolution Direct mode preserves\ntransient detail in the center channel\nby using a sharper extraction algorithm.\nMay increase CPU usage slightly.";
    pub const DECORRELATION: &str = "Surround decorrelation method:\n• Velvet: Smooth, natural ambience\n• LFO: Modulated, wider spatial effect";
}

/// Render a config row with label, help icon, and control
/// The help icon shows a popup with help text on hover
fn render_config_row(
    label: &str,
    help_text: &'static str,
    control: impl IntoElement,
    theme: &Theme,
) -> impl IntoElement {
    let label_owned: SharedString = SharedString::from(label.to_string());
    let label_id: SharedString = SharedString::from(format!("help-{}", label));
    let help_text_owned: SharedString = help_text.into();

    div()
        .flex()
        .items_center()
        .justify_between()
        .w_full()
        .py_1()
        // Left side: Label + Help icon
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                // Label
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_secondary)
                        .child(label_owned),
                )
                // Help icon - shows help popup on hover
                .child(
                    div()
                        .id(label_id)
                        .relative()
                        .group("help-icon")
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .w(px(14.0))
                                .h(px(14.0))
                                .rounded_full()
                                .border_1()
                                .border_color(theme.text_muted)
                                .text_size(px(10.0))
                                .text_color(theme.text_muted)
                                .cursor_pointer()
                                .hover(|s| {
                                    s.bg(theme.accent)
                                        .border_color(theme.accent)
                                        .text_color(theme.text_on_accent)
                                })
                                .child("i"),
                        )
                        // Popup shown on hover (uses group_hover)
                        .child(
                            div()
                                .occlude()
                                .invisible()
                                .group_hover("help-icon", |s| s.visible())
                                .absolute()
                                .left_full()
                                .top(px(-8.0))
                                .ml_2()
                                .p_3()
                                .w(px(280.0))
                                .bg(theme.background)
                                .border_1()
                                .border_color(theme.border)
                                .rounded_lg()
                                .shadow_xl()
                                .text_xs()
                                .line_height(relative(1.5))
                                .text_color(theme.text_primary)
                                .child(help_text_owned),
                        ),
                ),
        )
        // Right side: Control
        .child(control)
}

/// State for rendering the Upmixer plugin
pub struct UpmixerRenderState<'a> {
    pub speaker_config: &'a str,
    pub gain_front_direct: f64,
    pub stereo_width: f64,
    pub gain_front_ambient: f64,
    pub gain_rear_ambient: f64,
    pub lfe_cutoff_hz: f64,
    pub bandpass_hz: f64,
    pub height_gain: f64,
    pub lfe_gain: f64,
    pub enable_subharmonic_synth: bool,
    pub subharmonic_gain: f64,
    pub enable_hr_direct: bool,
    pub hr_sharpen: f64,
    pub safety_cap_db: f64,
    pub decorrelation_mode: usize,
    pub is_editing: bool,
    pub selected_param: usize,
    pub config_open: bool,
}

/// Render the upmixer plugin controls
/// Uses Entity<AppState> for direct state updates
pub fn render_upmixer_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    // We need to own the string for the closure
    let speaker_config_owned = state.speaker_config.to_string();
    let config_open = state.config_open;
    let decorrelation_mode = state.decorrelation_mode;

    // Main horizontal layout: Config column | Sliders | Knobs
    HStack::new()
        .spacing(StackSpacing::Md)
        .align(StackAlign::Start)
        // Left config column
        .child(
            VStack::new()
                .spacing(StackSpacing::Xs)
                .build()
                .p_3()
                .bg(theme.background_secondary)
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                // Config select
                .child(render_config_row(
                    "Config",
                    help_text::CONFIG,
                    div().w(px(100.0)).child(
                        Select::new("config-select")
                            .options(
                                [
                                    "2.0", "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4",
                                    "9.1.4", "9.1.6",
                                ]
                                .iter()
                                .map(|c| SelectOption::new(c.to_string(), c.to_string()))
                                .collect(),
                            )
                            .selected(speaker_config_owned.clone())
                            .is_open(config_open)
                            .size(SelectSize::Sm)
                            .on_toggle({
                                let entity = entity.clone();
                                move |is_open, _window, cx| {
                                    entity.update(cx, |state, _| {
                                        state.app.upmixer_config_open = is_open;
                                    });
                                }
                            })
                            .on_change({
                                let entity = entity.clone();
                                move |value, _, cx| {
                                    let configs = [
                                        "2.0", "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2",
                                        "7.1.4", "9.1.4", "9.1.6",
                                    ];
                                    let idx = configs
                                        .iter()
                                        .position(|&c| c == value.as_ref())
                                        .unwrap_or(0);
                                    entity.update(cx, |state, _| {
                                        state.app.set_plugin_param(plugin_idx, 0, idx as f64);
                                        state.app.upmixer_config_open = false;
                                        state.app.update_level_meter_groups();
                                    });
                                }
                            }),
                    ),
                    theme,
                ))
                // Subharmonic toggle
                .child(render_config_row(
                    "SubHarm",
                    help_text::SUBHARMONIC,
                    Toggle::new(("subharm-toggle", plugin_idx))
                        .checked(state.enable_subharmonic_synth)
                        .label(if state.enable_subharmonic_synth {
                            "On"
                        } else {
                            "Off"
                        })
                        .style(ToggleStyle::Segmented)
                        .theme(theme.to_toggle_theme())
                        .on_change({
                            let entity = entity.clone();
                            move |new_value, _, cx| {
                                entity.update(cx, |state, _| {
                                    state
                                        .app
                                        .set_plugin_param(plugin_idx, 9, if new_value { 1.0 } else { 0.0 });
                                });
                            }
                        }),
                    theme,
                ))
                // HR Direct toggle
                .child(render_config_row(
                    "HR Direct",
                    help_text::HR_DIRECT,
                    Toggle::new(("hr-direct-toggle", plugin_idx))
                        .checked(state.enable_hr_direct)
                        .label(if state.enable_hr_direct { "On" } else { "Off" })
                        .style(ToggleStyle::Segmented)
                        .theme(theme.to_toggle_theme())
                        .on_change({
                            let entity = entity.clone();
                            move |new_value, _, cx| {
                                entity.update(cx, |state, _| {
                                    state
                                        .app
                                        .set_plugin_param(plugin_idx, 11, if new_value { 1.0 } else { 0.0 });
                                });
                            }
                        }),
                    theme,
                ))
                // Decorrelation mode toggle
                .child(render_config_row(
                    "Decorr",
                    help_text::DECORRELATION,
                    Toggle::new(("decorr-toggle", plugin_idx))
                        .checked(decorrelation_mode == 1)
                        .label(if decorrelation_mode == 1 {
                            "LFO"
                        } else {
                            "Velvet"
                        })
                        .style(ToggleStyle::Segmented)
                        .theme(theme.to_toggle_theme())
                        .on_change({
                            let entity = entity.clone();
                            move |new_value, _, cx| {
                                entity.update(cx, |state, _| {
                                    state
                                        .app
                                        .set_plugin_param(plugin_idx, 14, if new_value { 1.0 } else { 0.0 });
                                });
                            }
                        }),
                    theme,
                )),
        )
        // Separator
        .child(
            Divider::vertical()
                .color(theme.border)
                .build_simple()
                .h(px(200.0)),
        )
        // Main controls section with vertical sliders for gains and knobs for other params
        .child(
            HStack::new()
                .spacing(StackSpacing::Lg)
                .align(StackAlign::Start)
                .wrap(true)
                // Gain sliders section - vertical sliders for all gain controls
                // Height matches 2 rows of knobs (~200px)
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        // Mains (Front Direct)
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Mains",
                            state.gain_front_direct,
                            GAIN_FRONT_DIRECT_MIN as f64,
                            GAIN_FRONT_DIRECT_MAX as f64,
                            "x",
                            1, // gain_front_direct
                            state.selected_param,
                            state.is_editing,
                            Some('m'),
                            130.0,
                            theme,
                        ))
                        // Center (Front Ambient)
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Center",
                            state.gain_front_ambient,
                            GAIN_FRONT_AMBIENT_MIN as f64,
                            GAIN_FRONT_AMBIENT_MAX as f64,
                            "x",
                            2, // gain_front_ambient
                            state.selected_param,
                            state.is_editing,
                            Some('c'),
                            130.0,
                            theme,
                        ))
                        // LFE
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "LFE",
                            state.lfe_gain,
                            LFE_GAIN_MIN as f64,
                            LFE_GAIN_MAX as f64,
                            "x",
                            8, // lfe_gain
                            state.selected_param,
                            state.is_editing,
                            Some('l'),
                            130.0,
                            theme,
                        ))
                        // Surrounds
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Surr",
                            state.gain_rear_ambient,
                            GAIN_REAR_AMBIENT_MIN as f64,
                            GAIN_REAR_AMBIENT_MAX as f64,
                            "x",
                            3, // gain_rear_ambient
                            state.selected_param,
                            state.is_editing,
                            Some('s'),
                            130.0,
                            theme,
                        ))
                        // Top (Height)
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Top",
                            state.height_gain,
                            HEIGHT_GAIN_MIN as f64,
                            HEIGHT_GAIN_MAX as f64,
                            "x",
                            7, // height_gain
                            state.selected_param,
                            state.is_editing,
                            Some('t'),
                            130.0,
                            theme,
                        ))
                        // Width (Stereo Width)
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Width",
                            state.stereo_width,
                            STEREO_WIDTH_MIN as f64,
                            STEREO_WIDTH_MAX as f64,
                            "",
                            5, // stereo_width
                            state.selected_param,
                            state.is_editing,
                            Some('w'),
                            130.0,
                            theme,
                        ))
                        .build()
                        .p_2()
                        .bg(theme.surface)
                        .rounded_lg(),
                )
                // Separator
                .child(
                    Divider::vertical()
                        .color(theme.border)
                        .build_simple()
                        .h(px(200.0)),
                )
                // Knobs in 2x2 grid layout
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        // Top row: LFE Cut, Bandpass
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Sm)
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "LFE Cut",
                                    state.lfe_cutoff_hz,
                                    LFE_CUTOFF_HZ_MIN as f64,
                                    LFE_CUTOFF_HZ_MAX as f64,
                                    "Hz",
                                    4, // lfe_cutoff_hz
                                    state.selected_param,
                                    state.is_editing,
                                    None,
                                    theme,
                                ))
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "Bandpass",
                                    state.bandpass_hz,
                                    BANDPASS_HZ_MIN as f64,
                                    BANDPASS_HZ_MAX as f64,
                                    "Hz",
                                    6, // bandpass_hz
                                    state.selected_param,
                                    state.is_editing,
                                    None,
                                    theme,
                                ))
                                .build(),
                        )
                        // Bottom row: Safety + conditional knobs
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Sm)
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "Safety",
                                    state.safety_cap_db,
                                    SAFETY_CAP_DB_MIN as f64,
                                    SAFETY_CAP_DB_MAX as f64,
                                    "dB",
                                    13, // safety_cap_db
                                    state.selected_param,
                                    state.is_editing,
                                    None,
                                    theme,
                                ))
                                // SubHarm gain (conditional)
                                .when(state.enable_subharmonic_synth, |el| {
                                    el.child(render_knob(
                                        entity.clone(),
                                        plugin_idx,
                                        "SubGain",
                                        state.subharmonic_gain,
                                        SUBHARMONIC_GAIN_MIN as f64,
                                        SUBHARMONIC_GAIN_MAX as f64,
                                        "",
                                        10, // subharmonic_gain
                                        state.selected_param,
                                        state.is_editing,
                                        None,
                                        theme,
                                    ))
                                })
                                // HR Sharpen (conditional)
                                .when(state.enable_hr_direct, |el| {
                                    el.child(render_knob(
                                        entity.clone(),
                                        plugin_idx,
                                        "Sharpen",
                                        state.hr_sharpen,
                                        HR_SHARPEN_MIN as f64,
                                        HR_SHARPEN_MAX as f64,
                                        "",
                                        12, // hr_sharpen
                                        state.selected_param,
                                        state.is_editing,
                                        None,
                                        theme,
                                    ))
                                })
                                .build(),
                        )
                        .build(),
                )
                .build()
                .p_2(),
        )
        .build()
        .p_3()
        .w_full()
        .h_full()
}
