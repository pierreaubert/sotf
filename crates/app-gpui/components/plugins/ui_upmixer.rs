//! Upmixer Plugin UI Component
//!
//! Layout:
//! - Main area: Channel Gains (4 faders) + Spatial Controls (4 faders) side by side
//! - Tab bar: LFE & Bass | Dialogue | Ambient | Height | HR Direct | Decorrelation | Analysis | Diagnostic
//! - Tab content: Expandable panel for the selected tab

use super::common::{render_knob, render_vertical_slider_with_ticks};
use crate::app::AppState;
use crate::components::design::Ds;
use crate::components::plugins::editing::PluginEditingManager;
use crate::components::plugins::level_meters::LevelMeterManager;
use crate::components::plugins::theme::PluginTheme;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    HStack, Select, SelectOption, SelectSize, StackAlign, StackSpacing, Toggle, ToggleStyle, VStack,
};
use sotf_plugins::param_specs::{ParamCategory, find_by_key as pk, upmixer::PARAMS as UP};

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
    // Analysis parameters
    pub low_latency: bool,
    pub frequency_resolution: usize,
    pub multi_source_extraction: bool,
    pub multi_source_threshold: f64,
    pub binaural_preview: bool,
    pub auto_gain_enabled: bool,
    pub auto_gain_max_db: f64,
    pub auto_gain_smoothing_ms: f64,
    // UI state
    pub is_editing: bool,
    pub selected_param: usize,
    pub config_open: bool,
    /// 0=none, 1=LFE, 2=Dialogue, 3=Ambient, 4=Height, 5=HR Direct,
    /// 6=Decorrelation, 7=Analysis, 8=Diagnostic, 9=Spatial
    pub upmixer_tab: usize,
    /// Live per-channel loudness. Drives both spider modes — the SPL view
    /// reads `true_peaks_dbtp`, the Correlation view reads
    /// `correlation_matrix`. Both are populated by the same LoudnessMonitor
    /// poll path, so no separate correlation field is needed.
    pub loudness_info: Option<sotf_audio_player::LoudnessData>,
    /// Shared spatial-spider UI state (view mode, ref channel, 3D camera).
    pub spatial_spider: crate::components::plugins::spatial_spider::SpatialSpiderUiState,
}

/// Parameter indices derived from param_specs::upmixer::PARAMS at compile time.
/// If a key is renamed or removed, compilation fails.
mod param_idx {
    use sotf_plugins::param_specs::{index_of, upmixer::PARAMS as P};

    pub const _SPEAKER_CONFIG: usize = index_of(P, "speaker_config");
    pub const GAIN_FRONT_DIRECT: usize = index_of(P, "gain_front_direct");
    pub const GAIN_FRONT_AMBIENT: usize = index_of(P, "gain_front_ambient");
    pub const GAIN_REAR_AMBIENT: usize = index_of(P, "gain_rear_ambient");
    pub const HEIGHT_GAIN: usize = index_of(P, "height_gain");
    pub const LFE_GAIN: usize = index_of(P, "lfe_gain");
    pub const LFE_CUTOFF_HZ: usize = index_of(P, "lfe_cutoff_hz");
    pub const ENABLE_SUBHARMONIC_SYNTH: usize = index_of(P, "enable_subharmonic_synth");
    pub const SUBHARMONIC_GAIN: usize = index_of(P, "subharmonic_gain");
    pub const SUBHARMONIC_FREQ_HZ: usize = index_of(P, "subharmonic_freq_hz");
    pub const SUBHARMONIC_ATTACK_MS: usize = index_of(P, "subharmonic_attack_ms");
    pub const SUBHARMONIC_RELEASE_MS: usize = index_of(P, "subharmonic_release_ms");
    pub const STEREO_WIDTH: usize = index_of(P, "stereo_width");
    pub const CENTER_SPREAD: usize = index_of(P, "center_spread");
    pub const _BANDPASS_HZ: usize = index_of(P, "bandpass_hz");
    pub const ENABLE_HR_DIRECT: usize = index_of(P, "enable_hr_direct");
    pub const HR_SHARPEN: usize = index_of(P, "hr_sharpen");
    pub const AMBIENT_BOOST: usize = index_of(P, "ambient_boost");
    pub const DECORRELATION_MODE: usize = index_of(P, "decorrelation_mode");
    pub const DECORRELATION_LFO_RATE_HZ: usize = index_of(P, "decorrelation_lfo_rate_hz");
    pub const VELVET_NOISE_DURATION_MS: usize = index_of(P, "velvet_noise_duration_ms");
    pub const VELVET_NOISE_DENSITY: usize = index_of(P, "velvet_noise_density");
    pub const HEIGHT_HF_CAP_HZ: usize = index_of(P, "height_hf_cap_hz");
    pub const HEIGHT_TRANSIENT_REDUCTION: usize = index_of(P, "height_transient_reduction");
    pub const HEIGHT_DIRECT_LEAK: usize = index_of(P, "height_direct_leak");
    pub const SURROUND_DIRECT_BLEED: usize = index_of(P, "surround_direct_bleed");
    pub const REAR_AMBIENT_BOOST: usize = index_of(P, "rear_ambient_boost");
    pub const REAR_LATE_REFLECTION: usize = index_of(P, "rear_late_reflection");
    pub const DIALOGUE_WEIGHT: usize = index_of(P, "dialogue_weight");
    pub const VOICE_FREQ_MIN_HZ: usize = index_of(P, "voice_freq_min_hz");
    pub const VOICE_FREQ_MAX_HZ: usize = index_of(P, "voice_freq_max_hz");
    pub const DIALOGUE_CENTROID_WEIGHT: usize = index_of(P, "dialogue_centroid_weight");
    pub const DIALOGUE_VARIANCE_WEIGHT: usize = index_of(P, "dialogue_variance_weight");
    pub const DIALOGUE_COHERENCE_WEIGHT: usize = index_of(P, "dialogue_coherence_weight");
    pub const SAFETY_CAP_DB: usize = index_of(P, "safety_cap_db");
    pub const BYPASS_DECORRELATION: usize = index_of(P, "bypass_decorrelation");
    pub const BYPASS_TRANSIENT_DETECTION: usize = index_of(P, "bypass_transient_detection");
    pub const BYPASS_ALL_PROCESSING: usize = index_of(P, "bypass_all_processing");
    pub const ENABLE_ML_DETECTION: usize = index_of(P, "enable_ml_detection");
    pub const LOW_LATENCY: usize = index_of(P, "low_latency");
    pub const FREQUENCY_RESOLUTION: usize = index_of(P, "frequency_resolution");
    pub const MULTI_SOURCE_EXTRACTION: usize = index_of(P, "multi_source_extraction");
    pub const MULTI_SOURCE_THRESHOLD: usize = index_of(P, "multi_source_threshold");
    pub const BINAURAL_PREVIEW: usize = index_of(P, "binaural_preview");
    pub const AUTO_GAIN_ENABLED: usize = index_of(P, "auto_gain_enabled");
    pub const AUTO_GAIN_MAX_DB: usize = index_of(P, "auto_gain_max_db");
    pub const AUTO_GAIN_SMOOTHING_MS: usize = index_of(P, "auto_gain_smoothing_ms");
}

/// Configuration menu items
const CONFIG_ITEMS: [&str; 9] = [
    "LFE & Bass",
    "Dialogue",
    "Ambient",
    "Height",
    "HR Direct",
    "Decorrelation",
    "Analysis",
    "Diagnostic",
    "Spatial",
];

/// Render the upmixer plugin controls
pub fn render_upmixer_plugin(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: UpmixerRenderState,
    theme: &Theme,
    plugin_theme: &PluginTheme,
) -> impl IntoElement {
    let selected_config = state.upmixer_tab;
    let app_background = theme.background_secondary;

    // Overlay the chassis theme onto the global theme so every helper
    // taking `&Theme` (knobs, sliders, toggles, panels) repaints with the
    // chassis palette. Same trick as ui_layout_renderer.
    let chassis_theme = plugin_theme.apply_to(theme);
    let theme = &chassis_theme;

    let config_column = render_config_column_controls(d, entity.clone(), plugin_idx, &state, theme);

    let output_column = render_output_column(d, entity.clone(), plugin_idx, &state, theme);

    // Main area: Channel Gains + Spatial Controls side by side (centered)
    let main_area = render_main_area(d, entity.clone(), plugin_idx, &state, theme);

    // Tab bar below main area
    let tab_bar = render_tab_bar(d, entity.clone(), selected_config, theme);

    // Configuration row: conditional on selected_config (1-7)
    let config_row = render_config_row(
        d,
        entity.clone(),
        plugin_idx,
        selected_config,
        &state,
        theme,
    );

    // Permanent spatial-spider graph row, always visible below the tabs.
    // The Spatial tab itself hosts the controls (mode toggles + ref-channel
    // selector); switching to other tabs leaves the graph in place so the
    // user can keep watching the field while editing parameters elsewhere.
    let spider_graph_row = render_spider_graph_row(d, &state, theme);

    div()
        .w_full()
        .bg(app_background)
        .rounded(d.r_lg)
        .flex()
        .justify_center()
        .p(d.pad_x)
        .child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .align(StackAlign::Start)
                .child(config_column)
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Xs)
                        .child(main_area)
                        .child(tab_bar)
                        .when((1..=9).contains(&selected_config), |el| {
                            el.child(config_row)
                        })
                        .child(spider_graph_row)
                        .build(),
                )
                .child(output_column)
                .build(),
        )
}

fn render_config_column_controls(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let labels = pk(UP, "speaker_config").choice_labels();
    let output_options: Vec<SelectOption> = labels
        .iter()
        .map(|label| SelectOption::new(label.to_string(), label.to_string()))
        .collect();

    div()
        .w(px(180.0))
        .flex_none()
        .flex()
        .flex_col()
        .gap(d.gap)
        .p(d.pad_y)
        .bg(theme.surface)
        .rounded(d.r_lg)
        .child(render_section_header(d, "Configuration", theme))
        .child(
            VStack::new()
                .spacing(StackSpacing::Xs)
                .child(
                    div()
                        .text_size(d.text_xs)
                        .text_color(theme.text_secondary)
                        .child("Output Channels".to_string()),
                )
                .child(
                    div().w_full().child(
                        Select::new(format!("upmixer-output-config-{plugin_idx}"))
                            .options(output_options)
                            .selected(state.speaker_config.to_string())
                            .is_open(state.config_open)
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
                                move |value, _window, cx| {
                                    let idx = labels
                                        .iter()
                                        .position(|label| *label == value.as_ref())
                                        .unwrap_or(0);
                                    entity.update(cx, |state, _| {
                                        state.app.set_plugin_param(
                                            plugin_idx,
                                            param_idx::_SPEAKER_CONFIG,
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
        )
        .child(render_diag_toggle(
            d,
            entity,
            plugin_idx,
            "Binaural Preview",
            state.binaural_preview,
            param_idx::BINAURAL_PREVIEW,
            theme,
        ))
}

fn render_output_column(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .w(px(220.0))
        .flex_none()
        .flex()
        .flex_col()
        .gap(d.gap_md)
        .p(d.pad_y)
        .bg(theme.surface)
        .rounded(d.r_lg)
        .child(render_section_header(d, "Output", theme))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "Safety Cap",
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
        .child(render_diag_toggle(
            d,
            entity.clone(),
            plugin_idx,
            "Auto Gain",
            state.auto_gain_enabled,
            param_idx::AUTO_GAIN_ENABLED,
            theme,
        ))
        .child(
            div()
                .when(!state.auto_gain_enabled, |d| d.opacity(0.3))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "AG Max",
                    state.auto_gain_max_db,
                    pk(UP, "auto_gain_max_db").min_f64(),
                    pk(UP, "auto_gain_max_db").max_f64(),
                    "dB",
                    param_idx::AUTO_GAIN_MAX_DB,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                )),
        )
        .child(
            div()
                .when(!state.auto_gain_enabled, |d| d.opacity(0.3))
                .child(render_knob(
                    entity,
                    plugin_idx,
                    "AG Smooth",
                    state.auto_gain_smoothing_ms,
                    pk(UP, "auto_gain_smoothing_ms").min_f64(),
                    pk(UP, "auto_gain_smoothing_ms").max_f64(),
                    "ms",
                    param_idx::AUTO_GAIN_SMOOTHING_MS,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                )),
        )
}

/// Render an underline tab button
fn render_tab_button(
    d: &Ds,
    id: &'static str,
    label: &str,
    is_active: bool,
    theme: &Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .cursor_pointer()
        .px(d.card)
        // intentional: asymmetric underline-tab padding — 4/6 pair is visually tuned
        .pb(px(6.0))
        .pt(px(4.0))
        .text_size(d.text_xs)
        .text_center()
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
            gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }
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
    d: &Ds,
    entity: Entity<AppState>,
    selected_config: usize,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .items_end()
        .justify_center()
        .w_full()
        .border_b_1()
        .border_color(theme.border)
        .children(CONFIG_ITEMS.iter().enumerate().map(|(i, label)| {
            let config_idx = i + 1; // 1-indexed
            let is_active = selected_config == config_idx;
            let entity = entity.clone();
            render_tab_button(
                d,
                match i {
                    0 => "cfg-lfe",
                    1 => "cfg-dialogue",
                    2 => "cfg-ambient",
                    3 => "cfg-height",
                    4 => "cfg-hr-direct",
                    5 => "cfg-decorr",
                    6 => "cfg-analysis",
                    7 => "cfg-diagnostic",
                    8 => "cfg-spatial",
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
    d: &Ds,
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
                .gap(d.gap)
                .child(render_section_header(d, "Channel Gains", theme))
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
                .p(d.pad_y)
                .bg(theme.surface)
                .rounded(d.r_lg),
        )
        // Spatial Controls
        .child(
            div()
                .flex()
                .flex_col()
                .gap(d.gap)
                .child(render_section_header(d, "Spatial Controls", theme))
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
                .p(d.pad_y)
                .bg(theme.surface)
                .rounded(d.r_lg),
        )
        .build()
}

/// Render the configuration row based on selected config menu item
fn render_config_row(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    selected_config: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let content: AnyElement = match selected_config {
        1 => render_config_lfe(d, entity, plugin_idx, state, theme).into_any_element(),
        2 => render_config_dialogue(d, entity, plugin_idx, state, theme).into_any_element(),
        3 => render_config_ambient(d, entity, plugin_idx, state, theme).into_any_element(),
        4 => render_config_height(d, entity, plugin_idx, state, theme).into_any_element(),
        5 => render_config_hr_direct(d, entity, plugin_idx, state, theme).into_any_element(),
        6 => render_config_decorrelation(d, entity, plugin_idx, state, theme).into_any_element(),
        7 => render_config_analysis(d, entity, plugin_idx, state, theme).into_any_element(),
        8 => render_config_diagnostic(d, entity, plugin_idx, state, theme).into_any_element(),
        9 => render_config_spatial(d, entity, plugin_idx, state, theme).into_any_element(),
        _ => div().into_any_element(),
    };

    div()
        .w_full()
        .p(d.pad_x)
        .bg(theme.background_secondary)
        .rounded(d.r_lg)
        .border_1()
        .border_color(theme.border)
        .child(content)
}

/// Render a section header
fn render_section_header(d: &Ds, label: &str, theme: &Theme) -> impl IntoElement {
    div()
        .text_size(d.text_xs)
        .font_weight(FontWeight::BOLD)
        .text_color(theme.text_muted)
        .pb(d.pad_y_half)
        .child(label.to_string())
}

// ─────────────────────────────────────────────────────────────────────────
// Configuration row panels
// ─────────────────────────────────────────────────────────────────────────

/// LFE & Bass configuration row
fn render_config_lfe(
    d: &Ds,
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
                .gap(d.gap_md)
                .child(render_section_header(d, "LFE & Bass", theme))
                // intentional: pixel-exact 1px vertical divider — do not scale
                .child(div().w(px(1.0)).h(px(14.0)).bg(theme.border))
                .child(render_section_header(d, "SubHarmonic", theme))
                .child(
                    Toggle::new(("subharm-toggle", plugin_idx))
                        .checked(subharm_enabled)
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
                // intentional: pixel-exact 1px vertical divider — do not scale
                .child(div().w(px(1.0)).h(px(80.0)).bg(theme.border))
                // SubHarmonic knobs (dimmed when disabled)
                .child(
                    div()
                        .flex()
                        .gap(d.gap_md)
                        .when(!subharm_enabled, |el| el.opacity(0.3))
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
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(render_section_header(d, "Dialogue", theme))
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
                // intentional: pixel-exact 1px vertical divider — do not scale
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
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(render_section_header(d, "Ambient", theme))
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

/// Height configuration row (pure height channel params, no HR Direct)
fn render_config_height(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(render_section_header(d, "Height", theme))
        .child(
            HStack::new()
                .spacing(StackSpacing::Md)
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

/// HR Direct configuration row
fn render_config_hr_direct(
    d: &Ds,
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
                .gap(d.gap_md)
                .child(render_section_header(d, "HR Direct", theme))
                .child(
                    Toggle::new(("hr-direct-toggle", plugin_idx))
                        .checked(state.enable_hr_direct)
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
                    "Amb Boost",
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
                .build(),
        )
        .build()
}

/// Decorrelation configuration row
fn render_config_decorrelation(
    d: &Ds,
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
                .gap(d.gap_md)
                .child(render_section_header(d, "Decorrelation", theme))
                .child(HStack::new().spacing(StackSpacing::Xs).children(
                    decorrelation_modes.into_iter().map(|(mode, label)| {
                        let is_active = decorrelation_mode == mode;
                        let entity = entity.clone();
                        render_tab_button(
                            d,
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
                    }),
                )),
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

/// Analysis & Source Extraction configuration row
fn render_config_analysis(
    d: &Ds,
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
                .gap(d.gap_md)
                .child(render_section_header(d, "Analysis", theme))
                // intentional: pixel-exact 1px vertical divider — do not scale
                .child(div().w(px(1.0)).h(px(14.0)).bg(theme.border))
                .child(render_section_header(d, "Source Extraction", theme))
                .child(
                    Toggle::new(("multi-source-toggle", plugin_idx))
                        .checked(state.multi_source_extraction)
                        .style(ToggleStyle::Segmented)
                        .theme(theme.to_toggle_theme())
                        .on_change({
                            let entity = entity.clone();
                            move |new_value, _, cx| {
                                entity.update(cx, |state, _| {
                                    state.app.set_plugin_param(
                                        plugin_idx,
                                        param_idx::MULTI_SOURCE_EXTRACTION,
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
                // Analysis: Low Latency toggle
                .child(render_diag_toggle(
                    d,
                    entity.clone(),
                    plugin_idx,
                    "Low Latency",
                    state.low_latency,
                    param_idx::LOW_LATENCY,
                    theme,
                ))
                // Analysis: Freq Resolution selector
                .child({
                    let freq_res = state.frequency_resolution;
                    let labels = pk(UP, "frequency_resolution").choice_labels();
                    div()
                        .flex()
                        .items_center()
                        .gap(d.gap)
                        .child(
                            div()
                                .text_size(d.text_xs)
                                .text_color(theme.text_secondary)
                                .child("Resolution".to_string()),
                        )
                        .children(labels.iter().enumerate().map(|(i, label)| {
                            let is_active = freq_res == i;
                            let entity = entity.clone();
                            render_tab_button(
                                d,
                                match i {
                                    0 => "freq-erb",
                                    1 => "freq-fine",
                                    _ => "freq-bin",
                                },
                                label,
                                is_active,
                                theme,
                            )
                            .on_click(move |_, _window, cx| {
                                entity.update(cx, |state, _| {
                                    state.app.set_plugin_param(
                                        plugin_idx,
                                        param_idx::FREQUENCY_RESOLUTION,
                                        i as f64,
                                    );
                                });
                            })
                        }))
                })
                // Separator
                // intentional: pixel-exact 1px vertical divider — do not scale
                .child(div().w(px(1.0)).h(px(40.0)).bg(theme.border))
                // Source Extraction threshold (dimmed when disabled)
                .child(
                    div()
                        .when(!state.multi_source_extraction, |d| d.opacity(0.3))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Threshold",
                            state.multi_source_threshold,
                            pk(UP, "multi_source_threshold").min_f64(),
                            pk(UP, "multi_source_threshold").max_f64(),
                            "",
                            param_idx::MULTI_SOURCE_THRESHOLD,
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

/// Diagnostic configuration row — auto-discovered from PARAMS.
///
/// Iterates the upmixer's `PARAMS` and renders a toggle for every spec tagged
/// `ParamCategory::Diagnostic`. Adding a `.diagnostic()` param in `params.rs`
/// auto-populates this tab — no edits here required (only a new field/match
/// arm in `UpmixerRenderState` + `diagnostic_param_value`).
fn render_config_diagnostic(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let mut row = HStack::new().spacing(StackSpacing::Md);
    for (idx, spec) in UP.iter().enumerate() {
        if !matches!(spec.category, ParamCategory::Diagnostic) {
            continue;
        }
        let value = diagnostic_param_value(state, idx);
        row = row.child(render_diag_toggle(
            d,
            entity.clone(),
            plugin_idx,
            spec.name,
            value,
            idx,
            theme,
        ));
    }

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(render_section_header(d, "Diagnostic", theme))
        .child(row.build())
        .build()
}

/// Read the bool value for a diagnostic-category param out of the render state.
///
/// Crashes on an unknown index — if a new `.diagnostic()` param is added in
/// `params.rs`, the surface that needs updating is `UpmixerRenderState` and
/// this match arm. The crash makes that dependency visible at runtime.
fn diagnostic_param_value(state: &UpmixerRenderState, idx: usize) -> bool {
    match idx {
        i if i == param_idx::BYPASS_DECORRELATION => state.bypass_decorrelation,
        i if i == param_idx::BYPASS_TRANSIENT_DETECTION => state.bypass_transient_detection,
        i if i == param_idx::BYPASS_ALL_PROCESSING => state.bypass_all_processing,
        i if i == param_idx::ENABLE_ML_DETECTION => state.enable_ml_detection,
        _ => unreachable!(
            "diagnostic_param_value: unmapped param index {} ({}); add a match arm",
            idx, UP[idx].name
        ),
    }
}

/// Render a single diagnostic toggle with label
fn render_diag_toggle(
    d: &Ds,
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
        .gap(d.gap)
        .child(
            div()
                .text_size(d.text_xs)
                .text_color(theme.text_secondary)
                .child(label.to_string()),
        )
        .child(
            Toggle::new((SharedString::from(format!("diag-{param_id}")), plugin_idx))
                .checked(value)
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

// ─────────────────────────────────────────────────────────────────────────
// Spatial tab: SPL / correlation spider visualizer
// ─────────────────────────────────────────────────────────────────────────

/// Spatial tab content — only the controls (view-mode toggle, SPL /
/// Correlation toggle, reference-channel dropdown). The graph itself lives
/// in a permanent row below the tab bar (see `render_spider_graph_row`).
fn render_config_spatial(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    use crate::components::plugins::spatial_spider::{
        SpatialSpiderSnapshot, render_spatial_spider_controls, resolve_speaker_config,
    };

    let snapshot = SpatialSpiderSnapshot {
        loudness: state.loudness_info.clone(),
        ui: state.spatial_spider.clone(),
    };
    let cfg_opt = resolve_speaker_config(&snapshot, Some(state.speaker_config));
    render_spatial_spider_controls(d, entity, plugin_idx, &snapshot, cfg_opt, theme)
}

/// Permanent spider graph row below the tab bar. Always visible regardless
/// of which tab is selected, so the user can watch the spatial field while
/// editing parameters in any tab.
fn render_spider_graph_row(
    d: &Ds,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    use crate::components::plugins::spatial_spider::{
        SpatialSpiderSnapshot, render_spatial_spider_graph, resolve_speaker_config,
    };

    let snapshot = SpatialSpiderSnapshot {
        loudness: state.loudness_info.clone(),
        ui: state.spatial_spider.clone(),
    };
    let cfg_opt = resolve_speaker_config(&snapshot, Some(state.speaker_config));
    div()
        .w_full()
        .pt(d.pad_y)
        .child(render_spatial_spider_graph(d, &snapshot, cfg_opt, theme))
}
