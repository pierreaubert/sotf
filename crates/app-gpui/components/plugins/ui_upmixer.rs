//! Upmixer Plugin UI Component
//!
//! Controls for the Upmixer plugin organized into tabbed sections:
//! - Main Gains: front/rear/center/height faders + stereo width/spread
//! - LFE & Bass: LFE cutoff/gain, sub-harmonic synth controls
//! - Advanced: decorrelation, height, ambient, dialogue, diagnostics

use super::common::{render_knob, render_toggle, render_vertical_slider_with_ticks};
use crate::app::AppState;
use crate::components::plugins::editing::PluginEditingManager;
use crate::components::plugins::level_meters::LevelMeterManager;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    HStack, Select, SelectOption, SelectSize, StackAlign, StackSpacing, TabItem, TabVariant, Tabs,
    Toggle, ToggleStyle, VStack,
};
use sotf_plugins::param_specs::{find_by_key as pk, upmixer::PARAMS as UP};

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
    pub upmixer_tab: usize,
}

/// Parameter indices matching PARAMS ordering in param_specs::upmixer::PARAMS
mod param_idx {
    pub const SPEAKER_CONFIG: usize = 0;
    pub const GAIN_FRONT_DIRECT: usize = 1;
    pub const GAIN_FRONT_AMBIENT: usize = 2;
    pub const GAIN_REAR_AMBIENT: usize = 3;
    pub const HEIGHT_GAIN: usize = 4;
    pub const LFE_GAIN: usize = 5;
    pub const LFE_CUTOFF_HZ: usize = 6;
    pub const ENABLE_SUBHARMONIC_SYNTH: usize = 7;
    pub const SUBHARMONIC_GAIN: usize = 8;
    pub const SUBHARMONIC_FREQ_HZ: usize = 9;
    pub const SUBHARMONIC_ATTACK_MS: usize = 10;
    pub const SUBHARMONIC_RELEASE_MS: usize = 11;
    pub const STEREO_WIDTH: usize = 12;
    pub const CENTER_SPREAD: usize = 13;
    pub const _BANDPASS_HZ: usize = 14; // Reserved but currently unused in UI
    pub const ENABLE_HR_DIRECT: usize = 15;
    pub const HR_SHARPEN: usize = 16;
    pub const AMBIENT_BOOST: usize = 17;
    pub const DECORRELATION_MODE: usize = 18;
    pub const DECORRELATION_LFO_RATE_HZ: usize = 19;
    pub const VELVET_NOISE_DURATION_MS: usize = 20;
    pub const VELVET_NOISE_DENSITY: usize = 21;
    pub const HEIGHT_HF_CAP_HZ: usize = 22;
    pub const HEIGHT_TRANSIENT_REDUCTION: usize = 23;
    pub const HEIGHT_DIRECT_LEAK: usize = 24;
    pub const SURROUND_DIRECT_BLEED: usize = 25;
    pub const REAR_AMBIENT_BOOST: usize = 26;
    pub const REAR_LATE_REFLECTION: usize = 27;
    pub const DIALOGUE_WEIGHT: usize = 28;
    pub const VOICE_FREQ_MIN_HZ: usize = 29;
    pub const VOICE_FREQ_MAX_HZ: usize = 30;
    pub const DIALOGUE_CENTROID_WEIGHT: usize = 31;
    pub const DIALOGUE_VARIANCE_WEIGHT: usize = 32;
    pub const DIALOGUE_COHERENCE_WEIGHT: usize = 33;
    pub const SAFETY_CAP_DB: usize = 34;
    pub const BYPASS_DECORRELATION: usize = 35;
    pub const BYPASS_TRANSIENT_DETECTION: usize = 36;
    pub const BYPASS_ALL_PROCESSING: usize = 37;
    pub const ENABLE_ML_DETECTION: usize = 38;
}

/// Render the upmixer plugin controls
pub fn render_upmixer_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let speaker_config_owned = state.speaker_config.to_string();
    let config_open = state.config_open;
    let selected_tab = state.upmixer_tab;

    // Tab bar
    let tab_bar = Tabs::new("upmixer-tabs")
        .tabs(vec![
            TabItem::new("gains", "Main Gains"),
            TabItem::new("lfe", "LFE & Bass"),
            TabItem::new("advanced", "Advanced"),
        ])
        .selected_index(selected_tab)
        .variant(TabVariant::Pills)
        .theme(theme.to_tabs_theme())
        .on_change({
            let entity = entity.clone();
            move |index, _window, cx| {
                entity.update(cx, |state, cx| {
                    state.app.upmixer_tab = index;
                    cx.notify();
                });
            }
        });

    // Tab content
    let tab_content: AnyElement = match selected_tab {
        0 => render_tab_gains(entity.clone(), plugin_idx, &state, theme).into_any_element(),
        1 => render_tab_lfe(entity.clone(), plugin_idx, &state, theme).into_any_element(),
        2 => render_tab_advanced(entity.clone(), plugin_idx, &state, theme).into_any_element(),
        _ => render_tab_gains(entity.clone(), plugin_idx, &state, theme).into_any_element(),
    };

    // Main layout
    VStack::new()
        .spacing(StackSpacing::Xs)
        // Top bar: tabs on left, output config on right
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .w_full()
                .child(tab_bar)
                .child(
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
                                            "2.0", "5.0", "5.1", "7.1", "5.1.2", "5.1.4",
                                            "7.1.2", "7.1.4", "9.1.4", "9.1.6",
                                        ]
                                        .iter()
                                        .map(|c| {
                                            SelectOption::new(c.to_string(), c.to_string())
                                        })
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
                                                "2.0", "5.0", "5.1", "7.1", "5.1.2", "5.1.4",
                                                "7.1.2", "7.1.4", "9.1.4", "9.1.6",
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
        // Tab content
        .child(tab_content)
        .build()
        .p_3()
        .w_full()
}

/// Tab 0: Main Gains — channel faders (row 1) + spatial controls (row 2)
fn render_tab_gains(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    VStack::new()
        .spacing(StackSpacing::Sm)
        // Row 1: Channel gains (the 4 main faders)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_section_header("Channel Gains", theme))
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(), plugin_idx, "Mains", state.gain_front_direct,
                            pk(UP, "gain_front_direct").min_f64(), pk(UP, "gain_front_direct").max_f64(),
                            "x", param_idx::GAIN_FRONT_DIRECT, state.selected_param, state.is_editing,
                            Some('m'), 180.0, theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(), plugin_idx, "Center", state.gain_front_ambient,
                            pk(UP, "gain_front_ambient").min_f64(), pk(UP, "gain_front_ambient").max_f64(),
                            "x", param_idx::GAIN_FRONT_AMBIENT, state.selected_param, state.is_editing,
                            Some('c'), 180.0, theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(), plugin_idx, "Surr", state.gain_rear_ambient,
                            pk(UP, "gain_rear_ambient").min_f64(), pk(UP, "gain_rear_ambient").max_f64(),
                            "x", param_idx::GAIN_REAR_AMBIENT, state.selected_param, state.is_editing,
                            Some('s'), 180.0, theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(), plugin_idx, "Top", state.height_gain,
                            pk(UP, "height_gain").min_f64(), pk(UP, "height_gain").max_f64(),
                            "x", param_idx::HEIGHT_GAIN, state.selected_param, state.is_editing,
                            Some('t'), 180.0, theme,
                        ))
                        .build(),
                )
                .p_2()
                .bg(theme.surface)
                .rounded_lg(),
        )
        // Row 2: Spatial controls (width, spread, bleed, reflect)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_section_header("Spatial Controls", theme))
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(), plugin_idx, "Width", state.stereo_width,
                            pk(UP, "stereo_width").min_f64(), pk(UP, "stereo_width").max_f64(),
                            "", param_idx::STEREO_WIDTH, state.selected_param, state.is_editing,
                            Some('w'), 180.0, theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(), plugin_idx, "Spread", state.center_spread,
                            pk(UP, "center_spread").min_f64(), pk(UP, "center_spread").max_f64(),
                            "", param_idx::CENTER_SPREAD, state.selected_param, state.is_editing,
                            None, 180.0, theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(), plugin_idx, "Bleed", state.surround_direct_bleed,
                            pk(UP, "surround_direct_bleed").min_f64(), pk(UP, "surround_direct_bleed").max_f64(),
                            "", param_idx::SURROUND_DIRECT_BLEED, state.selected_param, state.is_editing,
                            None, 180.0, theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(), plugin_idx, "Reflect", state.rear_late_reflection,
                            pk(UP, "rear_late_reflection").min_f64(), pk(UP, "rear_late_reflection").max_f64(),
                            "", param_idx::REAR_LATE_REFLECTION, state.selected_param, state.is_editing,
                            None, 180.0, theme,
                        ))
                        .build(),
                )
                .p_2()
                .bg(theme.surface)
                .rounded_lg(),
        )
        .build()
}

/// Tab 1: LFE & Bass — row 1: crossover knobs, row 2: subharmonic controls
fn render_tab_lfe(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    VStack::new()
        .spacing(StackSpacing::Sm)
        // Row 1: LFE Crossover — knobs side by side
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_section_header("LFE Crossover", theme))
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .child(render_knob(
                            entity.clone(), plugin_idx, "LFE Cut", state.lfe_cutoff_hz,
                            pk(UP, "lfe_cutoff_hz").min_f64(), pk(UP, "lfe_cutoff_hz").max_f64(),
                            "Hz", param_idx::LFE_CUTOFF_HZ, state.selected_param, state.is_editing,
                            None, theme,
                        ))
                        .child(render_knob(
                            entity.clone(), plugin_idx, "LFE Gain", state.lfe_gain,
                            pk(UP, "lfe_gain").min_f64(), pk(UP, "lfe_gain").max_f64(),
                            "x", param_idx::LFE_GAIN, state.selected_param, state.is_editing,
                            None, theme,
                        ))
                        .build(),
                )
                .p_3()
                .bg(theme.background_secondary)
                .rounded_lg()
                .border_1()
                .border_color(theme.border),
        )
        // Row 2: SubHarmonic Synth — toggle + knobs in a row
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(render_section_header("SubHarmonic Synth", theme))
                        .child(
                            Toggle::new(("subharm-toggle", plugin_idx))
                                .checked(state.enable_subharmonic_synth)
                                .label(if state.enable_subharmonic_synth { "On" } else { "Off" })
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
                        ),
                )
                .when(state.enable_subharmonic_synth, |el| {
                    el.child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(render_knob(
                                entity.clone(), plugin_idx, "SH Gain", state.subharmonic_gain,
                                pk(UP, "subharmonic_gain").min_f64(), pk(UP, "subharmonic_gain").max_f64(),
                                "", param_idx::SUBHARMONIC_GAIN, state.selected_param, state.is_editing,
                                None, theme,
                            ))
                            .child(render_knob(
                                entity.clone(), plugin_idx, "SH Freq", state.subharmonic_freq_hz,
                                pk(UP, "subharmonic_freq_hz").min_f64(), pk(UP, "subharmonic_freq_hz").max_f64(),
                                "Hz", param_idx::SUBHARMONIC_FREQ_HZ, state.selected_param, state.is_editing,
                                None, theme,
                            ))
                            .child(render_knob(
                                entity.clone(), plugin_idx, "Attack", state.subharmonic_attack_ms,
                                pk(UP, "subharmonic_attack_ms").min_f64(), pk(UP, "subharmonic_attack_ms").max_f64(),
                                "ms", param_idx::SUBHARMONIC_ATTACK_MS, state.selected_param, state.is_editing,
                                None, theme,
                            ))
                            .child(render_knob(
                                entity.clone(), plugin_idx, "Release", state.subharmonic_release_ms,
                                pk(UP, "subharmonic_release_ms").min_f64(), pk(UP, "subharmonic_release_ms").max_f64(),
                                "ms", param_idx::SUBHARMONIC_RELEASE_MS, state.selected_param, state.is_editing,
                                None, theme,
                            ))
                            .build(),
                    )
                })
                .p_3()
                .bg(theme.background_secondary)
                .rounded_lg()
                .border_1()
                .border_color(theme.border),
        )
        .build()
}

/// Tab 2: Advanced — 2 rows: dialogue/ambient/height on top, decorrelation/diagnostic on bottom
fn render_tab_advanced(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let decorrelation_mode = state.decorrelation_mode;

    VStack::new()
        .spacing(StackSpacing::Sm)
        // Row 1: Dialogue, Ambient, Height
        .child(
            HStack::new()
                .spacing(StackSpacing::Sm)
                .align(StackAlign::Start)
                .child(render_dialogue_box(entity.clone(), plugin_idx, state, theme))
                .child(render_ambient_box(entity.clone(), plugin_idx, state, theme))
                .child(render_height_box(entity.clone(), plugin_idx, state, theme))
                .build(),
        )
        // Row 2: Decorrelation, Diagnostic
        .child(
            HStack::new()
                .spacing(StackSpacing::Sm)
                .align(StackAlign::Start)
                .child(render_decorrelation_box(
                    entity.clone(), plugin_idx, state, decorrelation_mode, theme,
                ))
                .child(render_diagnostic_box(entity.clone(), plugin_idx, state, theme))
                .build(),
        )
        .build()
}

/// Dialogue box
fn render_dialogue_box(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    VStack::new()
        .spacing(StackSpacing::Sm)
        .child(render_section_header("Dialogue", theme))
        .child(render_knob(
            entity.clone(), plugin_idx, "Weight", state.dialogue_weight,
            pk(UP, "dialogue_weight").min_f64(), pk(UP, "dialogue_weight").max_f64(),
            "", param_idx::DIALOGUE_WEIGHT, state.selected_param, state.is_editing,
            None, theme,
        ))
        .child(render_knob(
            entity.clone(), plugin_idx, "Voice Lo", state.voice_freq_min_hz,
            pk(UP, "voice_freq_min_hz").min_f64(), pk(UP, "voice_freq_min_hz").max_f64(),
            "Hz", param_idx::VOICE_FREQ_MIN_HZ, state.selected_param, state.is_editing,
            None, theme,
        ))
        .child(render_knob(
            entity.clone(), plugin_idx, "Voice Hi", state.voice_freq_max_hz,
            pk(UP, "voice_freq_max_hz").min_f64(), pk(UP, "voice_freq_max_hz").max_f64(),
            "Hz", param_idx::VOICE_FREQ_MAX_HZ, state.selected_param, state.is_editing,
            None, theme,
        ))
        .child(
            HStack::new()
                .spacing(StackSpacing::Sm)
                .child(render_knob(
                    entity.clone(), plugin_idx, "W-Cent", state.dialogue_centroid_weight,
                    pk(UP, "dialogue_centroid_weight").min_f64(), pk(UP, "dialogue_centroid_weight").max_f64(),
                    "", param_idx::DIALOGUE_CENTROID_WEIGHT, state.selected_param, state.is_editing,
                    None, theme,
                ))
                .child(render_knob(
                    entity.clone(), plugin_idx, "W-Var", state.dialogue_variance_weight,
                    pk(UP, "dialogue_variance_weight").min_f64(), pk(UP, "dialogue_variance_weight").max_f64(),
                    "", param_idx::DIALOGUE_VARIANCE_WEIGHT, state.selected_param, state.is_editing,
                    None, theme,
                ))
                .child(render_knob(
                    entity.clone(), plugin_idx, "W-Coh", state.dialogue_coherence_weight,
                    pk(UP, "dialogue_coherence_weight").min_f64(), pk(UP, "dialogue_coherence_weight").max_f64(),
                    "", param_idx::DIALOGUE_COHERENCE_WEIGHT, state.selected_param, state.is_editing,
                    None, theme,
                ))
                .build(),
        )
        .build()
        .p_3()
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
        .spacing(StackSpacing::Sm)
        .child(render_section_header("Ambient", theme))
        .child(render_knob(
            entity.clone(), plugin_idx, "Boost", state.ambient_boost,
            pk(UP, "ambient_boost").min_f64(), pk(UP, "ambient_boost").max_f64(),
            "x", param_idx::AMBIENT_BOOST, state.selected_param, state.is_editing,
            None, theme,
        ))
        .child(render_knob(
            entity.clone(), plugin_idx, "Rear Boost", state.rear_ambient_boost,
            pk(UP, "rear_ambient_boost").min_f64(), pk(UP, "rear_ambient_boost").max_f64(),
            "x", param_idx::REAR_AMBIENT_BOOST, state.selected_param, state.is_editing,
            None, theme,
        ))
        .child(render_knob(
            entity.clone(), plugin_idx, "Safety", state.safety_cap_db,
            pk(UP, "safety_cap_db").min_f64(), pk(UP, "safety_cap_db").max_f64(),
            "dB", param_idx::SAFETY_CAP_DB, state.selected_param, state.is_editing,
            None, theme,
        ))
        .build()
        .p_3()
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
        .spacing(StackSpacing::Sm)
        .child(render_section_header("Height", theme))
        .child(
            HStack::new()
                .spacing(StackSpacing::Sm)
                .align(StackAlign::Center)
                .child(
                    Toggle::new(("hr-direct-toggle", plugin_idx))
                        .checked(state.enable_hr_direct)
                        .label(if state.enable_hr_direct { "HR On" } else { "HR Off" })
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
                        entity.clone(), plugin_idx, "Sharpen", state.hr_sharpen,
                        pk(UP, "hr_sharpen").min_f64(), pk(UP, "hr_sharpen").max_f64(),
                        "", param_idx::HR_SHARPEN, state.selected_param, state.is_editing,
                        None, theme,
                    ))
                })
                .build(),
        )
        .child(
            HStack::new()
                .spacing(StackSpacing::Sm)
                .align(StackAlign::Center)
                .child(render_knob(
                    entity.clone(), plugin_idx, "HF Cap", state.height_hf_cap_hz,
                    pk(UP, "height_hf_cap_hz").min_f64(), pk(UP, "height_hf_cap_hz").max_f64(),
                    "Hz", param_idx::HEIGHT_HF_CAP_HZ, state.selected_param, state.is_editing,
                    None, theme,
                ))
                .child(render_knob(
                    entity.clone(), plugin_idx, "Trans Red", state.height_transient_reduction,
                    pk(UP, "height_transient_reduction").min_f64(), pk(UP, "height_transient_reduction").max_f64(),
                    "", param_idx::HEIGHT_TRANSIENT_REDUCTION, state.selected_param, state.is_editing,
                    None, theme,
                ))
                .child(render_knob(
                    entity.clone(), plugin_idx, "Dir Leak", state.height_direct_leak,
                    pk(UP, "height_direct_leak").min_f64(), pk(UP, "height_direct_leak").max_f64(),
                    "", param_idx::HEIGHT_DIRECT_LEAK, state.selected_param, state.is_editing,
                    None, theme,
                ))
                .build(),
        )
        .build()
        .p_3()
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
        .spacing(StackSpacing::Sm)
        .child(render_section_header("Decorrelation", theme))
        .child(
            Toggle::new(("decorr-toggle", plugin_idx))
                .checked(decorrelation_mode == 1)
                .label(if decorrelation_mode == 1 { "LFO" } else { "Velvet" })
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
                entity.clone(), plugin_idx, "LFO Rate", state.decorrelation_lfo_rate_hz,
                pk(UP, "decorrelation_lfo_rate_hz").min_f64(), pk(UP, "decorrelation_lfo_rate_hz").max_f64(),
                "Hz", param_idx::DECORRELATION_LFO_RATE_HZ, state.selected_param, state.is_editing,
                None, theme,
            ))
        })
        .when(decorrelation_mode == 0, |el| {
            el.child(render_knob(
                entity.clone(), plugin_idx, "Duration", state.velvet_noise_duration_ms,
                pk(UP, "velvet_noise_duration_ms").min_f64(), pk(UP, "velvet_noise_duration_ms").max_f64(),
                "ms", param_idx::VELVET_NOISE_DURATION_MS, state.selected_param, state.is_editing,
                None, theme,
            ))
            .child(render_knob(
                entity.clone(), plugin_idx, "Density", state.velvet_noise_density,
                pk(UP, "velvet_noise_density").min_f64(), pk(UP, "velvet_noise_density").max_f64(),
                "/s", param_idx::VELVET_NOISE_DENSITY, state.selected_param, state.is_editing,
                None, theme,
            ))
        })
        .build()
        .p_3()
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
        .spacing(StackSpacing::Sm)
        .child(render_section_header("Diagnostic", theme))
        .child(render_toggle(
            entity.clone(), plugin_idx, "Bypass Decor", state.bypass_decorrelation,
            param_idx::BYPASS_DECORRELATION, state.selected_param, state.is_editing, theme,
        ))
        .child(render_toggle(
            entity.clone(), plugin_idx, "Bypass Trans", state.bypass_transient_detection,
            param_idx::BYPASS_TRANSIENT_DETECTION, state.selected_param, state.is_editing, theme,
        ))
        .child(render_toggle(
            entity.clone(), plugin_idx, "Bypass All", state.bypass_all_processing,
            param_idx::BYPASS_ALL_PROCESSING, state.selected_param, state.is_editing, theme,
        ))
        .child(render_toggle(
            entity.clone(), plugin_idx, "ML Detection", state.enable_ml_detection,
            param_idx::ENABLE_ML_DETECTION, state.selected_param, state.is_editing, theme,
        ))
        .build()
        .p_3()
        .bg(theme.background_secondary)
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
}
