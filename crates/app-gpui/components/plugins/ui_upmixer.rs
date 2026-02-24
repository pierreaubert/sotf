//! Upmixer Plugin UI Component
//!
//! Controls for the Upmixer plugin with:
//! - Speaker configuration selector
//! - Rotary knobs for gains and frequency controls
//! - Toggles for processing modes

use super::common::{render_knob, render_toggle, render_vertical_slider_with_ticks};
use crate::app::AppState;
use crate::components::plugins::editing::PluginEditingManager;
use crate::components::plugins::level_meters::LevelMeterManager;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    HStack, Select, SelectOption, SelectSize, StackAlign, StackSpacing, Toggle, ToggleStyle, VStack,
};
use sotf_audio_player::param_specs::upmixer::*;

/// Render a section header
fn render_section_header(label: &str, theme: &Theme) -> impl IntoElement {
    div()
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .text_color(theme.text_muted)
        .pb_1()
        .child(label.to_string())
}

/// State for rendering the Upmixer plugin
pub struct UpmixerRenderState<'a> {
    pub speaker_config: &'a str,
    // Gains (vertical sliders)
    pub gain_front_direct: f64,
    pub gain_front_ambient: f64,
    pub gain_rear_ambient: f64,
    pub height_gain: f64,
    pub stereo_width: f64,
    pub center_spread: f64,
    pub surround_direct_bleed: f64,
    pub rear_late_reflection: f64,
    // LFE parameters
    pub lfe_cutoff_hz: f64,
    pub lfe_gain: f64,
    pub bandpass_hz: f64,
    // Sub-harmonic parameters
    pub enable_subharmonic_synth: bool,
    pub subharmonic_gain: f64,
    pub subharmonic_freq_hz: f64,
    pub subharmonic_attack_ms: f64,
    pub subharmonic_release_ms: f64,
    // Decorrelation parameters
    pub decorrelation_mode: usize,
    pub decorrelation_lfo_rate_hz: f64,
    pub velvet_noise_duration_ms: f64,
    pub velvet_noise_density: f64,
    // Height parameters
    pub enable_hr_direct: bool,
    pub hr_sharpen: f64,
    pub height_hf_cap_hz: f64,
    pub height_transient_reduction: f64,
    pub height_direct_leak: f64,
    // Ambient parameters
    pub ambient_boost: f64,
    pub safety_cap_db: f64,
    pub rear_ambient_boost: f64,
    // Dialogue parameters
    pub dialogue_weight: f64,
    pub voice_freq_min_hz: f64,
    pub voice_freq_max_hz: f64,
    pub dialogue_centroid_weight: f64,
    pub dialogue_variance_weight: f64,
    pub dialogue_coherence_weight: f64,
    // Bypasses
    pub bypass_decorrelation: bool,
    pub bypass_transient_detection: bool,
    pub bypass_all_processing: bool,
    // ML vocal detection
    pub enable_ml_detection: bool,
    // UI state
    pub is_editing: bool,
    pub selected_param: usize,
    pub config_open: bool,
}

/// Parameter indices for set_plugin_param calls
mod param_idx {
    pub const SPEAKER_CONFIG: usize = 0;
    pub const GAIN_FRONT_DIRECT: usize = 1;
    pub const GAIN_FRONT_AMBIENT: usize = 2;
    pub const GAIN_REAR_AMBIENT: usize = 3;
    pub const HEIGHT_GAIN: usize = 4;
    pub const LFE_GAIN: usize = 5;
    pub const LFE_CUTOFF_HZ: usize = 6;
    pub const STEREO_WIDTH: usize = 7;
    pub const CENTER_SPREAD: usize = 8;
    pub const _BANDPASS_HZ: usize = 9; // Reserved but currently unused in UI
    pub const ENABLE_SUBHARMONIC_SYNTH: usize = 10;
    pub const SUBHARMONIC_GAIN: usize = 11;
    pub const ENABLE_HR_DIRECT: usize = 12;
    pub const HR_SHARPEN: usize = 13;
    pub const SAFETY_CAP_DB: usize = 14;
    pub const DECORRELATION_MODE: usize = 15;
    pub const SUBHARMONIC_FREQ_HZ: usize = 16;
    pub const SUBHARMONIC_ATTACK_MS: usize = 17;
    pub const SUBHARMONIC_RELEASE_MS: usize = 18;
    pub const DECORRELATION_LFO_RATE_HZ: usize = 19;
    pub const VELVET_NOISE_DURATION_MS: usize = 20;
    pub const VELVET_NOISE_DENSITY: usize = 21;
    pub const HEIGHT_HF_CAP_HZ: usize = 22;
    pub const HEIGHT_TRANSIENT_REDUCTION: usize = 23;
    pub const HEIGHT_DIRECT_LEAK: usize = 24;
    pub const SURROUND_DIRECT_BLEED: usize = 25;
    pub const REAR_AMBIENT_BOOST: usize = 26;
    pub const REAR_LATE_REFLECTION: usize = 27;
    pub const AMBIENT_BOOST: usize = 28;
    pub const DIALOGUE_WEIGHT: usize = 29;
    pub const VOICE_FREQ_MIN_HZ: usize = 30;
    pub const VOICE_FREQ_MAX_HZ: usize = 31;
    pub const DIALOGUE_CENTROID_WEIGHT: usize = 32;
    pub const DIALOGUE_VARIANCE_WEIGHT: usize = 33;
    pub const DIALOGUE_COHERENCE_WEIGHT: usize = 34;
    pub const BYPASS_DECORRELATION: usize = 35;
    pub const BYPASS_TRANSIENT_DETECTION: usize = 36;
    pub const BYPASS_ALL_PROCESSING: usize = 37;
    pub const ENABLE_ML_DETECTION: usize = 38;
}

/// Render the upmixer plugin controls
/// Uses Entity<AppState> for direct state updates
pub fn render_upmixer_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let speaker_config_owned = state.speaker_config.to_string();
    let config_open = state.config_open;
    let decorrelation_mode = state.decorrelation_mode;

    // Main layout: 2 rows
    VStack::new()
        .spacing(StackSpacing::Xs)
        // Top bar: Output config selector on the right
        .child(
            div().flex().justify_end().w_full().child(
                HStack::new()
                    .spacing(StackSpacing::Xs)
                    .align(StackAlign::Center)
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_secondary)
                            .child("Output"),
                    )
                    .child(
                        div().w(px(80.0)).child(
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
                                .size(SelectSize::Xs)
                                .theme(theme.to_select_theme())
                                .on_toggle({
                                    let entity = entity.clone();
                                    move |is_open, _window, cx| {
                                        entity.update(cx, |state, cx| {
                                            state.app.upmixer_config_open = is_open;
                                            cx.notify();
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
                                            state.app.set_plugin_param(
                                                plugin_idx,
                                                param_idx::SPEAKER_CONFIG,
                                                idx as f64,
                                            );
                                            state.app.upmixer_config_open = false;
                                            state.app.update_level_meter_groups();
                                        });
                                    }
                                }),
                        ),
                    )
                    .build(),
            ),
        )
        // Row 1: Crossovers, SubHarmonic, 8 Sliders
        .child(
            HStack::new()
                .spacing(StackSpacing::Sm)
                .align(StackAlign::Stretch)
                .child(render_crossovers_box(
                    entity.clone(),
                    plugin_idx,
                    &state,
                    theme,
                ))
                .child(render_subharmonic_box(
                    entity.clone(),
                    plugin_idx,
                    &state,
                    theme,
                ))
                .child(render_gains_row(entity.clone(), plugin_idx, &state, theme))
                .build(),
        )
        // Row 2: Dialogue, Ambient, Height, Decorrelation
        .child(
            HStack::new()
                .spacing(StackSpacing::Sm)
                .align(StackAlign::Start)
                .child(render_dialogue_box(
                    entity.clone(),
                    plugin_idx,
                    &state,
                    theme,
                ))
                .child(render_ambient_box(
                    entity.clone(),
                    plugin_idx,
                    &state,
                    theme,
                ))
                .child(render_height_box(entity.clone(), plugin_idx, &state, theme))
                .child(render_decorrelation_box(
                    entity.clone(),
                    plugin_idx,
                    &state,
                    decorrelation_mode,
                    theme,
                ))
                .child(render_diagnostic_box(
                    entity.clone(),
                    plugin_idx,
                    &state,
                    theme,
                ))
                .build(),
        )
        .build()
        .p_3()
        .w_full()
}

/// Crossovers box: LFE Cut, Bandpass Hz
fn render_crossovers_box(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(render_section_header("Crossovers", theme))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "LFE Cut",
            state.lfe_cutoff_hz,
            LFE_CUTOFF_HZ_MIN as f64,
            LFE_CUTOFF_HZ_MAX as f64,
            "Hz",
            param_idx::LFE_CUTOFF_HZ,
            state.selected_param,
            state.is_editing,
            None,
            theme,
        ))
        .build()
        .h_full()
        .p_2()
        .bg(theme.background_secondary)
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
}

/// SubHarmonic box: LFE Gain, toggle, and subharmonic params
fn render_subharmonic_box(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(render_section_header("SubHarmonic", theme))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "LFE Gain",
            state.lfe_gain,
            LFE_GAIN_MIN as f64,
            LFE_GAIN_MAX as f64,
            "x",
            param_idx::LFE_GAIN,
            state.selected_param,
            state.is_editing,
            None,
            theme,
        ))
        .child(
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
                            state.app.set_plugin_param(
                                plugin_idx,
                                param_idx::ENABLE_SUBHARMONIC_SYNTH,
                                if new_value { 1.0 } else { 0.0 },
                            );
                        });
                    }
                }),
        )
        .when(state.enable_subharmonic_synth, |el| {
            el.child(render_knob(
                entity.clone(),
                plugin_idx,
                "SH Gain",
                state.subharmonic_gain,
                SUBHARMONIC_GAIN_MIN as f64,
                SUBHARMONIC_GAIN_MAX as f64,
                "",
                param_idx::SUBHARMONIC_GAIN,
                state.selected_param,
                state.is_editing,
                None,
                theme,
            ))
            .child(render_knob(
                entity.clone(),
                plugin_idx,
                "Attack",
                state.subharmonic_attack_ms,
                SUBHARMONIC_ATTACK_MS_MIN as f64,
                SUBHARMONIC_ATTACK_MS_MAX as f64,
                "ms",
                param_idx::SUBHARMONIC_ATTACK_MS,
                state.selected_param,
                state.is_editing,
                None,
                theme,
            ))
            .child(render_knob(
                entity.clone(),
                plugin_idx,
                "Release",
                state.subharmonic_release_ms,
                SUBHARMONIC_RELEASE_MS_MIN as f64,
                SUBHARMONIC_RELEASE_MS_MAX as f64,
                "ms",
                param_idx::SUBHARMONIC_RELEASE_MS,
                state.selected_param,
                state.is_editing,
                None,
                theme,
            ))
            .child(render_knob(
                entity.clone(),
                plugin_idx,
                "SH Freq",
                state.subharmonic_freq_hz,
                SUBHARMONIC_FREQ_HZ_MIN as f64,
                SUBHARMONIC_FREQ_HZ_MAX as f64,
                "Hz",
                param_idx::SUBHARMONIC_FREQ_HZ,
                state.selected_param,
                state.is_editing,
                None,
                theme,
            ))
        })
        .build()
        .h_full()
        .p_2()
        .bg(theme.background_secondary)
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
}

/// Gains row: 8 vertical sliders in a single row
fn render_gains_row(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    HStack::new()
        .spacing(StackSpacing::Xs)
        .child(render_vertical_slider_with_ticks(
            entity.clone(),
            plugin_idx,
            "Mains",
            state.gain_front_direct,
            GAIN_FRONT_DIRECT_MIN as f64,
            GAIN_FRONT_DIRECT_MAX as f64,
            "x",
            param_idx::GAIN_FRONT_DIRECT,
            state.selected_param,
            state.is_editing,
            Some('m'),
            220.0,
            theme,
        ))
        .child(render_vertical_slider_with_ticks(
            entity.clone(),
            plugin_idx,
            "Center",
            state.gain_front_ambient,
            GAIN_FRONT_AMBIENT_MIN as f64,
            GAIN_FRONT_AMBIENT_MAX as f64,
            "x",
            param_idx::GAIN_FRONT_AMBIENT,
            state.selected_param,
            state.is_editing,
            Some('c'),
            220.0,
            theme,
        ))
        .child(render_vertical_slider_with_ticks(
            entity.clone(),
            plugin_idx,
            "Surr",
            state.gain_rear_ambient,
            GAIN_REAR_AMBIENT_MIN as f64,
            GAIN_REAR_AMBIENT_MAX as f64,
            "x",
            param_idx::GAIN_REAR_AMBIENT,
            state.selected_param,
            state.is_editing,
            Some('s'),
            220.0,
            theme,
        ))
        .child(render_vertical_slider_with_ticks(
            entity.clone(),
            plugin_idx,
            "Top",
            state.height_gain,
            GAIN_HEIGHT_MIN as f64,
            GAIN_HEIGHT_MAX as f64,
            "x",
            param_idx::HEIGHT_GAIN,
            state.selected_param,
            state.is_editing,
            Some('t'),
            220.0,
            theme,
        ))
        .child(render_vertical_slider_with_ticks(
            entity.clone(),
            plugin_idx,
            "Width",
            state.stereo_width,
            STEREO_WIDTH_MIN as f64,
            STEREO_WIDTH_MAX as f64,
            "",
            param_idx::STEREO_WIDTH,
            state.selected_param,
            state.is_editing,
            Some('w'),
            220.0,
            theme,
        ))
        .child(render_vertical_slider_with_ticks(
            entity.clone(),
            plugin_idx,
            "Spread",
            state.center_spread,
            CENTER_SPREAD_MIN as f64,
            CENTER_SPREAD_MAX as f64,
            "",
            param_idx::CENTER_SPREAD,
            state.selected_param,
            state.is_editing,
            None,
            220.0,
            theme,
        ))
        .child(render_vertical_slider_with_ticks(
            entity.clone(),
            plugin_idx,
            "Bleed",
            state.surround_direct_bleed,
            SURROUND_DIRECT_BLEED_MIN as f64,
            SURROUND_DIRECT_BLEED_MAX as f64,
            "",
            param_idx::SURROUND_DIRECT_BLEED,
            state.selected_param,
            state.is_editing,
            None,
            220.0,
            theme,
        ))
        .child(render_vertical_slider_with_ticks(
            entity.clone(),
            plugin_idx,
            "Reflect",
            state.rear_late_reflection,
            REAR_LATE_REFLECTION_MIN as f64,
            REAR_LATE_REFLECTION_MAX as f64,
            "",
            param_idx::REAR_LATE_REFLECTION,
            state.selected_param,
            state.is_editing,
            None,
            220.0,
            theme,
        ))
        .build()
        .h_full()
        .p_2()
        .bg(theme.surface)
        .rounded_lg()
}

/// Dialogue box
fn render_dialogue_box(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(render_section_header("Dialogue", theme))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "Weight",
            state.dialogue_weight,
            DIALOGUE_WEIGHT_MIN as f64,
            DIALOGUE_WEIGHT_MAX as f64,
            "",
            param_idx::DIALOGUE_WEIGHT,
            state.selected_param,
            state.is_editing,
            None,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "Voice Lo",
            state.voice_freq_min_hz,
            VOICE_FREQ_MIN_HZ_MIN as f64,
            VOICE_FREQ_MIN_HZ_MAX as f64,
            "Hz",
            param_idx::VOICE_FREQ_MIN_HZ,
            state.selected_param,
            state.is_editing,
            None,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "Voice Hi",
            state.voice_freq_max_hz,
            VOICE_FREQ_MAX_HZ_MIN as f64,
            VOICE_FREQ_MAX_HZ_MAX as f64,
            "Hz",
            param_idx::VOICE_FREQ_MAX_HZ,
            state.selected_param,
            state.is_editing,
            None,
            theme,
        ))
        .child(
            HStack::new()
                .spacing(StackSpacing::Xs)
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "W-Cent",
                    state.dialogue_centroid_weight,
                    DIALOGUE_CENTROID_WEIGHT_MIN as f64,
                    DIALOGUE_CENTROID_WEIGHT_MAX as f64,
                    "",
                    param_idx::DIALOGUE_CENTROID_WEIGHT,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "W-Var",
                    state.dialogue_variance_weight,
                    DIALOGUE_VARIANCE_WEIGHT_MIN as f64,
                    DIALOGUE_VARIANCE_WEIGHT_MAX as f64,
                    "",
                    param_idx::DIALOGUE_VARIANCE_WEIGHT,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "W-Coh",
                    state.dialogue_coherence_weight,
                    DIALOGUE_COHERENCE_WEIGHT_MIN as f64,
                    DIALOGUE_COHERENCE_WEIGHT_MAX as f64,
                    "",
                    param_idx::DIALOGUE_COHERENCE_WEIGHT,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                .build(),
        )
        .build()
        .p_2()
        .bg(theme.background_secondary)
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
}

/// Ambient box
fn render_ambient_box(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(render_section_header("Ambient", theme))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "Boost",
            state.ambient_boost,
            AMBIENT_BOOST_MIN as f64,
            AMBIENT_BOOST_MAX as f64,
            "x",
            param_idx::AMBIENT_BOOST,
            state.selected_param,
            state.is_editing,
            None,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "Rear Boost",
            state.rear_ambient_boost,
            REAR_AMBIENT_BOOST_MIN as f64,
            REAR_AMBIENT_BOOST_MAX as f64,
            "x",
            param_idx::REAR_AMBIENT_BOOST,
            state.selected_param,
            state.is_editing,
            None,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "Safety",
            state.safety_cap_db,
            SAFETY_CAP_DB_MIN as f64,
            SAFETY_CAP_DB_MAX as f64,
            "dB",
            param_idx::SAFETY_CAP_DB,
            state.selected_param,
            state.is_editing,
            None,
            theme,
        ))
        .build()
        .p_2()
        .bg(theme.background_secondary)
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
}

/// Height box
fn render_height_box(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(render_section_header("Height", theme))
        .child(
            HStack::new()
                .spacing(StackSpacing::Xs)
                .align(StackAlign::Center)
                .child(
                    Toggle::new(("hr-direct-toggle", plugin_idx))
                        .checked(state.enable_hr_direct)
                        .label(if state.enable_hr_direct {
                            "HR On"
                        } else {
                            "HR Off"
                        })
                        .style(ToggleStyle::Segmented)
                        .theme(theme.to_toggle_theme())
                        .on_change({
                            let entity = entity.clone();
                            move |new_value, _, cx| {
                                entity.update(cx, |state, _| {
                                    state.app.set_plugin_param(
                                        plugin_idx,
                                        param_idx::ENABLE_HR_DIRECT,
                                        if new_value { 1.0 } else { 0.0 },
                                    );
                                });
                            }
                        }),
                )
                .when(state.enable_hr_direct, |el| {
                    el.child(render_knob(
                        entity.clone(),
                        plugin_idx,
                        "Sharpen",
                        state.hr_sharpen,
                        HR_SHARPEN_MIN as f64,
                        HR_SHARPEN_MAX as f64,
                        "",
                        param_idx::HR_SHARPEN,
                        state.selected_param,
                        state.is_editing,
                        None,
                        theme,
                    ))
                })
                .build(),
        )
        .child(
            HStack::new()
                .spacing(StackSpacing::Xs)
                .align(StackAlign::Center)
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "HF Cap",
                    state.height_hf_cap_hz,
                    HEIGHT_HF_CAP_HZ_MIN as f64,
                    HEIGHT_HF_CAP_HZ_MAX as f64,
                    "Hz",
                    param_idx::HEIGHT_HF_CAP_HZ,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Trans Red",
                    state.height_transient_reduction,
                    HEIGHT_TRANSIENT_REDUCTION_MIN as f64,
                    HEIGHT_TRANSIENT_REDUCTION_MAX as f64,
                    "",
                    param_idx::HEIGHT_TRANSIENT_REDUCTION,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Dir Leak",
                    state.height_direct_leak,
                    HEIGHT_DIRECT_LEAK_MIN as f64,
                    HEIGHT_DIRECT_LEAK_MAX as f64,
                    "",
                    param_idx::HEIGHT_DIRECT_LEAK,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                .build(),
        )
        .build()
        .p_2()
        .bg(theme.background_secondary)
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
}

/// Decorrelation box
fn render_decorrelation_box(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    decorrelation_mode: usize,
    theme: &Theme,
) -> impl IntoElement {
    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(render_section_header("Decorrelation", theme))
        .child(
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
                            state.app.set_plugin_param(
                                plugin_idx,
                                param_idx::DECORRELATION_MODE,
                                if new_value { 1.0 } else { 0.0 },
                            );
                        });
                    }
                }),
        )
        .when(decorrelation_mode == 1, |el| {
            el.child(render_knob(
                entity.clone(),
                plugin_idx,
                "LFO Rate",
                state.decorrelation_lfo_rate_hz,
                DECORRELATION_LFO_RATE_HZ_MIN as f64,
                DECORRELATION_LFO_RATE_HZ_MAX as f64,
                "Hz",
                param_idx::DECORRELATION_LFO_RATE_HZ,
                state.selected_param,
                state.is_editing,
                None,
                theme,
            ))
        })
        .when(decorrelation_mode == 0, |el| {
            el.child(render_knob(
                entity.clone(),
                plugin_idx,
                "Duration",
                state.velvet_noise_duration_ms,
                VELVET_NOISE_DURATION_MS_MIN as f64,
                VELVET_NOISE_DURATION_MS_MAX as f64,
                "ms",
                param_idx::VELVET_NOISE_DURATION_MS,
                state.selected_param,
                state.is_editing,
                None,
                theme,
            ))
            .child(render_knob(
                entity.clone(),
                plugin_idx,
                "Density",
                state.velvet_noise_density,
                VELVET_NOISE_DENSITY_MIN as f64,
                VELVET_NOISE_DENSITY_MAX as f64,
                "/s",
                param_idx::VELVET_NOISE_DENSITY,
                state.selected_param,
                state.is_editing,
                None,
                theme,
            ))
        })
        .build()
        .p_2()
        .bg(theme.background_secondary)
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
}

/// Diagnostic box
fn render_diagnostic_box(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(render_section_header("Diagnostic", theme))
        .child(render_toggle(
            entity.clone(),
            plugin_idx,
            "Bypass Decor",
            state.bypass_decorrelation,
            param_idx::BYPASS_DECORRELATION,
            state.selected_param,
            state.is_editing,
            theme,
        ))
        .child(render_toggle(
            entity.clone(),
            plugin_idx,
            "Bypass Trans",
            state.bypass_transient_detection,
            param_idx::BYPASS_TRANSIENT_DETECTION,
            state.selected_param,
            state.is_editing,
            theme,
        ))
        .child(render_toggle(
            entity.clone(),
            plugin_idx,
            "Bypass All",
            state.bypass_all_processing,
            param_idx::BYPASS_ALL_PROCESSING,
            state.selected_param,
            state.is_editing,
            theme,
        ))
        .child(render_toggle(
            entity.clone(),
            plugin_idx,
            "ML Detection",
            state.enable_ml_detection,
            param_idx::ENABLE_ML_DETECTION,
            state.selected_param,
            state.is_editing,
            theme,
        ))
        .build()
        .p_2()
        .bg(theme.background_secondary)
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
}
