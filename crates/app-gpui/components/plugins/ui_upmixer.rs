//! Upmixer Plugin UI Component
//!
//! Layout:
//! - Main area: Channel Gains (4 faders) + Spatial Controls (4 faders) side by side
//! - Tab bar: LFE & Bass | Dialogue | Ambient | Height | Decorrelation | Config
//! - Tab content: Expandable panel for the selected tab

use super::common::{render_knob, render_vertical_slider_with_ticks};
use crate::app::AppState;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{HStack, StackAlign, StackSpacing, Toggle, ToggleStyle, VStack};
use sotf_plugins::param_specs::{find_by_key as pk, upmixer::PARAMS as UP};

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
    /// 0=none, 1=LFE, 2=Dialogue, 3=Ambient, 4=Height, 5=Decorrelation
    pub upmixer_tab: usize,
}

/// Parameter indices matching PARAMS ordering in param_specs::upmixer::PARAMS
mod param_idx {
    pub const _SPEAKER_CONFIG: usize = 0;
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

/// Configuration menu items
const CONFIG_ITEMS: [&str; 6] = [
    "LFE & Bass",
    "Dialogue",
    "Ambient",
    "Height",
    "Decorrelation",
    "Config",
];

/// Render the upmixer plugin controls
pub fn render_upmixer_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let selected_config = state.upmixer_tab; // 0=none, 1-6=config panels

    // Main area: Channel Gains + Spatial Controls side by side
    let main_area = render_main_area(entity.clone(), plugin_idx, &state, theme);

    // Tab bar below main area
    let tab_bar = render_tab_bar(entity.clone(), selected_config, theme);

    // Configuration row: conditional on selected_config
    let config_row = render_config_row(entity.clone(), plugin_idx, selected_config, &state, theme);

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(main_area)
        .child(tab_bar)
        .when((1..=6).contains(&selected_config), |el| {
            el.child(config_row)
        })
        .build()
        .p_3()
        .w_full()
}

/// Render an underline tab button
fn render_tab_button(
    id: &'static str,
    label: &str,
    is_active: bool,
    theme: &Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .cursor_pointer()
        .px_3()
        .pb(px(6.0))
        .pt(px(4.0))
        .text_xs()
        .font_weight(if is_active {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        })
        .text_color(if is_active {
            theme.accent
        } else {
            theme.text_muted
        })
        // Active underline
        .border_b_2()
        .border_color(if is_active {
            theme.accent
        } else {
            gpui::rgba(0x00000000)
        })
        .hover(|s| {
            s.text_color(theme.text_primary).border_color(if is_active {
                theme.accent
            } else {
                theme.text_muted
            })
        })
        .child(label.to_string())
}

/// Render the tab bar below the main blocks
fn render_tab_bar(
    entity: Entity<AppState>,
    selected_config: usize,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .items_end()
        .w_full()
        .border_b_1()
        .border_color(theme.border)
        .children(CONFIG_ITEMS.iter().enumerate().map(|(i, label)| {
            let config_idx = i + 1; // 1-indexed
            let is_active = selected_config == config_idx;
            let entity = entity.clone();
            render_tab_button(
                match i {
                    0 => "cfg-lfe",
                    1 => "cfg-dialogue",
                    2 => "cfg-ambient",
                    3 => "cfg-height",
                    4 => "cfg-decorr",
                    5 => "cfg-config",
                    _ => "cfg-unknown",
                },
                label,
                is_active,
                theme,
            )
            .on_click(move |_, _window, cx| {
                entity.update(cx, |state, cx| {
                    state.app.upmixer_tab = if state.app.upmixer_tab == config_idx {
                        0
                    } else {
                        config_idx
                    };
                    cx.notify();
                });
            })
        }))
}

/// Render the main area: Channel Gains + Spatial Controls side by side
fn render_main_area(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    HStack::new()
        .spacing(StackSpacing::Sm)
        .align(StackAlign::Start)
        // Channel Gains
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
                            entity.clone(),
                            plugin_idx,
                            "Mains",
                            state.gain_front_direct,
                            pk(UP, "gain_front_direct").min_f64(),
                            pk(UP, "gain_front_direct").max_f64(),
                            "x",
                            param_idx::GAIN_FRONT_DIRECT,
                            state.selected_param,
                            state.is_editing,
                            Some('m'),
                            180.0,
                            theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Center",
                            state.gain_front_ambient,
                            pk(UP, "gain_front_ambient").min_f64(),
                            pk(UP, "gain_front_ambient").max_f64(),
                            "x",
                            param_idx::GAIN_FRONT_AMBIENT,
                            state.selected_param,
                            state.is_editing,
                            Some('c'),
                            180.0,
                            theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Surr",
                            state.gain_rear_ambient,
                            pk(UP, "gain_rear_ambient").min_f64(),
                            pk(UP, "gain_rear_ambient").max_f64(),
                            "x",
                            param_idx::GAIN_REAR_AMBIENT,
                            state.selected_param,
                            state.is_editing,
                            Some('s'),
                            180.0,
                            theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Top",
                            state.height_gain,
                            pk(UP, "height_gain").min_f64(),
                            pk(UP, "height_gain").max_f64(),
                            "x",
                            param_idx::HEIGHT_GAIN,
                            state.selected_param,
                            state.is_editing,
                            Some('t'),
                            180.0,
                            theme,
                        ))
                        .build(),
                )
                .p_2()
                .bg(theme.surface)
                .rounded_lg(),
        )
        // Spatial Controls
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
                            entity.clone(),
                            plugin_idx,
                            "Width",
                            state.stereo_width,
                            pk(UP, "stereo_width").min_f64(),
                            pk(UP, "stereo_width").max_f64(),
                            "",
                            param_idx::STEREO_WIDTH,
                            state.selected_param,
                            state.is_editing,
                            Some('w'),
                            180.0,
                            theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Spread",
                            state.center_spread,
                            pk(UP, "center_spread").min_f64(),
                            pk(UP, "center_spread").max_f64(),
                            "",
                            param_idx::CENTER_SPREAD,
                            state.selected_param,
                            state.is_editing,
                            None,
                            180.0,
                            theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Bleed",
                            state.surround_direct_bleed,
                            pk(UP, "surround_direct_bleed").min_f64(),
                            pk(UP, "surround_direct_bleed").max_f64(),
                            "",
                            param_idx::SURROUND_DIRECT_BLEED,
                            state.selected_param,
                            state.is_editing,
                            None,
                            180.0,
                            theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Reflect",
                            state.rear_late_reflection,
                            pk(UP, "rear_late_reflection").min_f64(),
                            pk(UP, "rear_late_reflection").max_f64(),
                            "",
                            param_idx::REAR_LATE_REFLECTION,
                            state.selected_param,
                            state.is_editing,
                            None,
                            180.0,
                            theme,
                        ))
                        .build(),
                )
                .p_2()
                .bg(theme.surface)
                .rounded_lg(),
        )
        .build()
}

/// Render the configuration row based on selected config menu item
fn render_config_row(
    entity: Entity<AppState>,
    plugin_idx: usize,
    selected_config: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let content: AnyElement = match selected_config {
        1 => render_config_lfe(entity, plugin_idx, state, theme).into_any_element(),
        2 => render_config_dialogue(entity, plugin_idx, state, theme).into_any_element(),
        3 => render_config_ambient(entity, plugin_idx, state, theme).into_any_element(),
        4 => render_config_height(entity, plugin_idx, state, theme).into_any_element(),
        5 => render_config_decorrelation(entity, plugin_idx, state, theme).into_any_element(),
        6 => render_config_diagnostic(entity, plugin_idx, state, theme).into_any_element(),
        _ => div().into_any_element(),
    };

    div()
        .w_full()
        .p_3()
        .bg(theme.background_secondary)
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
        .child(content)
}

/// Render a section header
fn render_section_header(label: &str, theme: &Theme) -> impl IntoElement {
    div()
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .text_color(theme.text_muted)
        .pb_1()
        .child(label.to_string())
}

// ─────────────────────────────────────────────────────────────────────────
// Configuration row panels
// ─────────────────────────────────────────────────────────────────────────

/// LFE & Bass configuration row
fn render_config_lfe(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let subharm_enabled = state.enable_subharmonic_synth;

    VStack::new()
        .spacing(StackSpacing::Xs)
        // Header row: "LFE & Bass" label + "SubHarmonic" label + toggle
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(render_section_header("LFE & Bass", theme))
                .child(div().w(px(1.0)).h(px(14.0)).bg(theme.border))
                .child(render_section_header("SubHarmonic", theme))
                .child(
                    Toggle::new(("subharm-toggle", plugin_idx))
                        .checked(subharm_enabled)
                        .label(if subharm_enabled { "On" } else { "Off" })
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
        // Knobs row: all 6 knobs on the same baseline
        .child(
            HStack::new()
                .spacing(StackSpacing::Md)
                // LFE knobs
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "LFE Cut",
                    state.lfe_cutoff_hz,
                    pk(UP, "lfe_cutoff_hz").min_f64(),
                    pk(UP, "lfe_cutoff_hz").max_f64(),
                    "Hz",
                    param_idx::LFE_CUTOFF_HZ,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "LFE Gain",
                    state.lfe_gain,
                    pk(UP, "lfe_gain").min_f64(),
                    pk(UP, "lfe_gain").max_f64(),
                    "x",
                    param_idx::LFE_GAIN,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                // Separator
                .child(div().w(px(1.0)).h(px(80.0)).bg(theme.border))
                // SubHarmonic knobs (dimmed when disabled)
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .when(!subharm_enabled, |d| d.opacity(0.3))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Gain",
                            state.subharmonic_gain,
                            pk(UP, "subharmonic_gain").min_f64(),
                            pk(UP, "subharmonic_gain").max_f64(),
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
                            "Freq",
                            state.subharmonic_freq_hz,
                            pk(UP, "subharmonic_freq_hz").min_f64(),
                            pk(UP, "subharmonic_freq_hz").max_f64(),
                            "Hz",
                            param_idx::SUBHARMONIC_FREQ_HZ,
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
                            pk(UP, "subharmonic_attack_ms").min_f64(),
                            pk(UP, "subharmonic_attack_ms").max_f64(),
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
                            pk(UP, "subharmonic_release_ms").min_f64(),
                            pk(UP, "subharmonic_release_ms").max_f64(),
                            "ms",
                            param_idx::SUBHARMONIC_RELEASE_MS,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                )
                .build(),
        )
        .build()
}

/// Dialogue configuration row
fn render_config_dialogue(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(render_section_header("Dialogue", theme))
        .child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Weight",
                    state.dialogue_weight,
                    pk(UP, "dialogue_weight").min_f64(),
                    pk(UP, "dialogue_weight").max_f64(),
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
                    pk(UP, "voice_freq_min_hz").min_f64(),
                    pk(UP, "voice_freq_min_hz").max_f64(),
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
                    pk(UP, "voice_freq_max_hz").min_f64(),
                    pk(UP, "voice_freq_max_hz").max_f64(),
                    "Hz",
                    param_idx::VOICE_FREQ_MAX_HZ,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                // Separator
                .child(div().w(px(1.0)).h(px(40.0)).bg(theme.border))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Centroid",
                    state.dialogue_centroid_weight,
                    pk(UP, "dialogue_centroid_weight").min_f64(),
                    pk(UP, "dialogue_centroid_weight").max_f64(),
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
                    "Variance",
                    state.dialogue_variance_weight,
                    pk(UP, "dialogue_variance_weight").min_f64(),
                    pk(UP, "dialogue_variance_weight").max_f64(),
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
                    "Coherence",
                    state.dialogue_coherence_weight,
                    pk(UP, "dialogue_coherence_weight").min_f64(),
                    pk(UP, "dialogue_coherence_weight").max_f64(),
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
}

/// Ambient configuration row
fn render_config_ambient(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(render_section_header("Ambient", theme))
        .child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Boost",
                    state.ambient_boost,
                    pk(UP, "ambient_boost").min_f64(),
                    pk(UP, "ambient_boost").max_f64(),
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
                    pk(UP, "rear_ambient_boost").min_f64(),
                    pk(UP, "rear_ambient_boost").max_f64(),
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
                    pk(UP, "safety_cap_db").min_f64(),
                    pk(UP, "safety_cap_db").max_f64(),
                    "dB",
                    param_idx::SAFETY_CAP_DB,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                .build(),
        )
        .build()
}

/// Height configuration row
fn render_config_height(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(render_section_header("Height", theme))
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
                ),
        )
        .child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    div()
                        .when(!state.enable_hr_direct, |d| d.opacity(0.3))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Sharpen",
                            state.hr_sharpen,
                            pk(UP, "hr_sharpen").min_f64(),
                            pk(UP, "hr_sharpen").max_f64(),
                            "",
                            param_idx::HR_SHARPEN,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                )
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "HF Cap",
                    state.height_hf_cap_hz,
                    pk(UP, "height_hf_cap_hz").min_f64(),
                    pk(UP, "height_hf_cap_hz").max_f64(),
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
                    pk(UP, "height_transient_reduction").min_f64(),
                    pk(UP, "height_transient_reduction").max_f64(),
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
                    pk(UP, "height_direct_leak").min_f64(),
                    pk(UP, "height_direct_leak").max_f64(),
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
}

/// Decorrelation configuration row
fn render_config_decorrelation(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let decorrelation_mode = state.decorrelation_mode;
    let decorrelation_modes = [(0usize, "Velvet"), (1usize, "LFO")];

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(render_section_header("Decorrelation", theme))
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Xs)
                        .children(decorrelation_modes.into_iter().map(|(mode, label)| {
                            let is_active = decorrelation_mode == mode;
                            let entity = entity.clone();
                            render_tab_button(
                                if mode == 0 {
                                    "decorr-velvet"
                                } else {
                                    "decorr-lfo"
                                },
                                label,
                                is_active,
                                theme,
                            )
                            .on_click(move |_, _window, cx| {
                                entity.update(cx, |state, _| {
                                    state.app.set_plugin_param(
                                        plugin_idx,
                                        param_idx::DECORRELATION_MODE,
                                        mode as f64,
                                    );
                                });
                            })
                        })),
                ),
        )
        .child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .when(decorrelation_mode == 1, |el| {
                    el.child(render_knob(
                        entity.clone(),
                        plugin_idx,
                        "LFO Rate",
                        state.decorrelation_lfo_rate_hz,
                        pk(UP, "decorrelation_lfo_rate_hz").min_f64(),
                        pk(UP, "decorrelation_lfo_rate_hz").max_f64(),
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
                        pk(UP, "velvet_noise_duration_ms").min_f64(),
                        pk(UP, "velvet_noise_duration_ms").max_f64(),
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
                        pk(UP, "velvet_noise_density").min_f64(),
                        pk(UP, "velvet_noise_density").max_f64(),
                        "/s",
                        param_idx::VELVET_NOISE_DENSITY,
                        state.selected_param,
                        state.is_editing,
                        None,
                        theme,
                    ))
                })
                .build(),
        )
        .build()
}

/// Config tab: diagnostic toggles in a row
fn render_config_diagnostic(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(render_section_header("Configuration", theme))
        .child(
            HStack::new()
                .spacing(StackSpacing::Lg)
                .child(render_diag_toggle(
                    entity.clone(),
                    plugin_idx,
                    "Bypass Decorrelation",
                    state.bypass_decorrelation,
                    param_idx::BYPASS_DECORRELATION,
                    theme,
                ))
                .child(render_diag_toggle(
                    entity.clone(),
                    plugin_idx,
                    "Bypass Transients",
                    state.bypass_transient_detection,
                    param_idx::BYPASS_TRANSIENT_DETECTION,
                    theme,
                ))
                .child(render_diag_toggle(
                    entity.clone(),
                    plugin_idx,
                    "Bypass All Processing",
                    state.bypass_all_processing,
                    param_idx::BYPASS_ALL_PROCESSING,
                    theme,
                ))
                .child(render_diag_toggle(
                    entity,
                    plugin_idx,
                    "ML Detection",
                    state.enable_ml_detection,
                    param_idx::ENABLE_ML_DETECTION,
                    theme,
                ))
                .build(),
        )
        .build()
}

/// Render a single diagnostic toggle with label
fn render_diag_toggle(
    entity: Entity<AppState>,
    plugin_idx: usize,
    label: &str,
    value: bool,
    param_id: usize,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .text_xs()
                .text_color(theme.text_secondary)
                .child(label.to_string()),
        )
        .child(
            Toggle::new((SharedString::from(format!("diag-{param_id}")), plugin_idx))
                .checked(value)
                .label(if value { "On" } else { "Off" })
                .style(ToggleStyle::Segmented)
                .theme(theme.to_toggle_theme())
                .on_change({
                    move |new_value, _, cx| {
                        entity.update(cx, |state, _| {
                            state.app.set_plugin_param(
                                plugin_idx,
                                param_id,
                                if new_value { 1.0 } else { 0.0 },
                            );
                        });
                    }
                }),
        )
}
