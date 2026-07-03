use super::super::common::render_knob;
use super::misc::CONFIG_ITEMS;
use super::misc::param_idx;
use super::types::UpmixerRenderState;
use super::types::diagnostic_param_value;
use crate::app::AppState;
use crate::components::design::Ds;
use crate::components::plugins::editing::PluginEditingManager;
use crate::components::plugins::theme::PluginTheme;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{HStack, Slider, SliderSize, StackSpacing, Toggle, ToggleStyle, VStack};
use sotf_plugins::param_specs::{ParamCategory, find_by_key as pk, upmixer::PARAMS as UP};

struct UpmixerConfigSpec {
    label: &'static str,
    id: &'static str,
    config_idx: usize,
}

static UPMIXER_CONFIG_SPECS: std::sync::OnceLock<Vec<UpmixerConfigSpec>> =
    std::sync::OnceLock::new();

fn upmixer_config_specs() -> &'static [UpmixerConfigSpec] {
    UPMIXER_CONFIG_SPECS
        .get_or_init(|| {
            const IDS: [&str; 9] = [
                "cfg-lfe",
                "cfg-dialogue",
                "cfg-ambient",
                "cfg-height",
                "cfg-hr-direct",
                "cfg-decorr",
                "cfg-analysis",
                "cfg-diagnostic",
                "cfg-spatial",
            ];
            CONFIG_ITEMS
                .iter()
                .zip(IDS)
                .enumerate()
                .map(|(i, (label, id))| UpmixerConfigSpec {
                    label,
                    id,
                    config_idx: i + 1,
                })
                .collect()
        })
        .as_slice()
}

/// Render the upmixer plugin controls
pub fn render_upmixer_plugin(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: UpmixerRenderState,
    available_width: f32,
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
    let layout = UpmixerLayout::from_width(available_width);

    let main_area = match layout {
        UpmixerLayout::Wide => {
            render_main_area(d, entity.clone(), plugin_idx, &state, theme).into_any_element()
        }
        UpmixerLayout::Medium => render_medium_area(
            d,
            entity.clone(),
            plugin_idx,
            selected_config,
            &state,
            theme,
        )
        .into_any_element(),
        UpmixerLayout::Narrow => render_narrow_area(
            d,
            entity.clone(),
            plugin_idx,
            selected_config,
            &state,
            theme,
        )
        .into_any_element(),
    };

    let tab_bar = render_tab_bar(d, entity.clone(), selected_config, layout, true, theme);

    // Configuration row: conditional on selected_config (1-7)
    let config_row = render_config_row(
        d,
        entity.clone(),
        plugin_idx,
        selected_config,
        &state,
        theme,
    );

    div()
        .w_full()
        .min_w_0()
        .bg(app_background)
        .rounded(d.r_lg)
        .flex()
        .justify_center()
        .p(d.pad_x)
        .overflow_hidden()
        .child(
            VStack::new()
                .spacing(StackSpacing::Xs)
                .child(main_area)
                .when(layout == UpmixerLayout::Wide, |el| el.child(tab_bar))
                .when(
                    layout == UpmixerLayout::Wide && (1..=9).contains(&selected_config),
                    |el| el.child(config_row),
                )
                .build(),
        )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum UpmixerLayout {
    Wide,
    Medium,
    Narrow,
}

impl UpmixerLayout {
    fn from_width(width: f32) -> Self {
        if width >= 900.0 {
            Self::Wide
        } else if width >= 600.0 {
            Self::Medium
        } else {
            Self::Narrow
        }
    }
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
    layout: UpmixerLayout,
    allow_toggle_off: bool,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_wrap()
        .items_end()
        .justify_center()
        .w_full()
        .when(layout == UpmixerLayout::Narrow, |el| el.justify_start())
        .border_b_1()
        .border_color(theme.border)
        .children(upmixer_config_specs().iter().map(|spec| {
            let config_idx = spec.config_idx;
            let is_active = selected_config == config_idx;
            let entity = entity.clone();
            render_tab_button(d, spec.id, spec.label, is_active, theme).on_click(
                move |_, _window, cx| {
                    entity.update(cx, |state, cx| {
                        state.app.plugin_ui.upmixer_tab =
                            if allow_toggle_off && state.app.plugin_ui.upmixer_tab == config_idx {
                                0
                            } else {
                                config_idx
                            };
                        cx.notify();
                    });
                },
            )
        }))
}

/// Render the main area: spatial field first, compact primary controls below.
fn render_main_area(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(d.gap_md)
        .child(render_upmixer_header(
            d,
            entity.clone(),
            plugin_idx,
            state,
            theme,
        ))
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(d.gap)
                .child(render_spider_graph_row(d, state, theme))
                .child(render_primary_control_strip(
                    d, entity, plugin_idx, state, theme,
                ))
                .p(d.pad_y)
                .bg(theme.surface)
                .rounded(d.r_lg),
        )
}

/// Render the medium main area: speaker field with a side inspector.
fn render_medium_area(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    selected_config: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let focused_config = focused_config_index(selected_config);

    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(d.gap_md)
        .child(render_upmixer_header(
            d,
            entity.clone(),
            plugin_idx,
            state,
            theme,
        ))
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_wrap()
                .gap(d.gap)
                .child(
                    div()
                        .flex_1()
                        .min_w(rems(22.0))
                        .p(d.pad_y)
                        .bg(theme.surface)
                        .rounded(d.r_lg)
                        .child(render_spider_graph_row(d, state, theme)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(d.gap)
                        .flex_1()
                        .min_w(rems(18.0))
                        .p(d.pad_y)
                        .bg(theme.surface)
                        .rounded(d.r_lg)
                        .child(render_section_header(d, "Primary", theme))
                        .child(render_tab_bar(
                            d,
                            entity.clone(),
                            focused_config,
                            UpmixerLayout::Medium,
                            false,
                            theme,
                        ))
                        .child(render_compact_config_content(
                            d,
                            entity.clone(),
                            plugin_idx,
                            focused_config,
                            state,
                            theme,
                        )),
                ),
        )
        .child(render_upmixer_summary_strip(d, state, theme))
}

/// Render the narrow main area: stacked field and immediately reachable primary lanes.
fn render_narrow_area(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    selected_config: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let focused_config = focused_config_index(selected_config);

    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(d.gap)
        .child(render_speaker_config_selector(
            d,
            entity.clone(),
            plugin_idx,
            state,
            theme,
        ))
        .child(render_spider_controls(
            d,
            entity.clone(),
            plugin_idx,
            state,
            theme,
        ))
        .child(
            div()
                .w_full()
                .min_w_0()
                .p(d.pad_y)
                .bg(theme.surface)
                .rounded(d.r_lg)
                .child(render_spider_graph_row(d, state, theme)),
        )
        .child(render_tab_bar(
            d,
            entity.clone(),
            focused_config,
            UpmixerLayout::Narrow,
            false,
            theme,
        ))
        .child(
            div()
                .w_full()
                .min_w_0()
                .p(d.pad_y)
                .bg(theme.surface)
                .rounded(d.r_lg)
                .child(render_compact_config_content(
                    d,
                    entity.clone(),
                    plugin_idx,
                    focused_config,
                    state,
                    theme,
                )),
        )
        .child(render_upmixer_summary_strip(d, state, theme))
}

fn focused_config_index(selected_config: usize) -> usize {
    if (1..=CONFIG_ITEMS.len()).contains(&selected_config) {
        selected_config
    } else {
        1
    }
}

fn render_compact_config_content(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    selected_config: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> AnyElement {
    match selected_config {
        1 => render_compact_lane_group(
            d,
            "LFE & Bass",
            vec![
                render_compact_toggle_row(
                    d,
                    entity.clone(),
                    plugin_idx,
                    "SubHarmonic",
                    state.enable_subharmonic_synth,
                    param_idx::ENABLE_SUBHARMONIC_SYNTH,
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "LFE Cut",
                    state.lfe_cutoff_hz,
                    "lfe_cutoff_hz",
                    param_idx::LFE_CUTOFF_HZ,
                    "Hz",
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "LFE Gain",
                    state.lfe_gain,
                    "lfe_gain",
                    param_idx::LFE_GAIN,
                    "x",
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "Bandpass",
                    state.bandpass_hz,
                    "bandpass_hz",
                    param_idx::BANDPASS_HZ,
                    "Hz",
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "Sub Gain",
                    state.subharmonic_gain,
                    "subharmonic_gain",
                    param_idx::SUBHARMONIC_GAIN,
                    "x",
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "Sub Freq",
                    state.subharmonic_freq_hz,
                    "subharmonic_freq_hz",
                    param_idx::SUBHARMONIC_FREQ_HZ,
                    "Hz",
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "Attack",
                    state.subharmonic_attack_ms,
                    "subharmonic_attack_ms",
                    param_idx::SUBHARMONIC_ATTACK_MS,
                    "ms",
                    theme,
                ),
                compact_lane(
                    d,
                    entity,
                    plugin_idx,
                    state,
                    "Release",
                    state.subharmonic_release_ms,
                    "subharmonic_release_ms",
                    param_idx::SUBHARMONIC_RELEASE_MS,
                    "ms",
                    theme,
                ),
            ],
            theme,
        )
        .into_any_element(),
        2 => render_compact_lane_group(
            d,
            "Dialogue",
            vec![
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "Weight",
                    state.dialogue_weight,
                    "dialogue_weight",
                    param_idx::DIALOGUE_WEIGHT,
                    "",
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "Voice Lo",
                    state.voice_freq_min_hz,
                    "voice_freq_min_hz",
                    param_idx::VOICE_FREQ_MIN_HZ,
                    "Hz",
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "Voice Hi",
                    state.voice_freq_max_hz,
                    "voice_freq_max_hz",
                    param_idx::VOICE_FREQ_MAX_HZ,
                    "Hz",
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "Centroid",
                    state.dialogue_centroid_weight,
                    "dialogue_centroid_weight",
                    param_idx::DIALOGUE_CENTROID_WEIGHT,
                    "",
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "Variance",
                    state.dialogue_variance_weight,
                    "dialogue_variance_weight",
                    param_idx::DIALOGUE_VARIANCE_WEIGHT,
                    "",
                    theme,
                ),
                compact_lane(
                    d,
                    entity,
                    plugin_idx,
                    state,
                    "Coherence",
                    state.dialogue_coherence_weight,
                    "dialogue_coherence_weight",
                    param_idx::DIALOGUE_COHERENCE_WEIGHT,
                    "",
                    theme,
                ),
            ],
            theme,
        )
        .into_any_element(),
        3 => render_compact_lane_group(
            d,
            "Ambient",
            vec![
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "Ambient",
                    state.ambient_boost,
                    "ambient_boost",
                    param_idx::AMBIENT_BOOST,
                    "x",
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "Rear Amb",
                    state.rear_ambient_boost,
                    "rear_ambient_boost",
                    param_idx::REAR_AMBIENT_BOOST,
                    "x",
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "Reflect",
                    state.rear_late_reflection,
                    "rear_late_reflection",
                    param_idx::REAR_LATE_REFLECTION,
                    "",
                    theme,
                ),
                compact_lane(
                    d,
                    entity,
                    plugin_idx,
                    state,
                    "Safety",
                    state.safety_cap_db,
                    "safety_cap_db",
                    param_idx::SAFETY_CAP_DB,
                    "dB",
                    theme,
                ),
            ],
            theme,
        )
        .into_any_element(),
        4 => render_compact_lane_group(
            d,
            "Height",
            vec![
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "Top Gain",
                    state.height_gain,
                    "height_gain",
                    param_idx::HEIGHT_GAIN,
                    "x",
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "HF Cap",
                    state.height_hf_cap_hz,
                    "height_hf_cap_hz",
                    param_idx::HEIGHT_HF_CAP_HZ,
                    "Hz",
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "Transient",
                    state.height_transient_reduction,
                    "height_transient_reduction",
                    param_idx::HEIGHT_TRANSIENT_REDUCTION,
                    "",
                    theme,
                ),
                compact_lane(
                    d,
                    entity,
                    plugin_idx,
                    state,
                    "Direct Leak",
                    state.height_direct_leak,
                    "height_direct_leak",
                    param_idx::HEIGHT_DIRECT_LEAK,
                    "",
                    theme,
                ),
            ],
            theme,
        )
        .into_any_element(),
        5 => render_compact_lane_group(
            d,
            "HR Direct",
            vec![
                render_compact_toggle_row(
                    d,
                    entity.clone(),
                    plugin_idx,
                    "HR Direct",
                    state.enable_hr_direct,
                    param_idx::ENABLE_HR_DIRECT,
                    theme,
                ),
                compact_lane(
                    d,
                    entity,
                    plugin_idx,
                    state,
                    "Sharpen",
                    state.hr_sharpen,
                    "hr_sharpen",
                    param_idx::HR_SHARPEN,
                    "",
                    theme,
                ),
            ],
            theme,
        )
        .into_any_element(),
        6 => render_compact_lane_group(
            d,
            "Decorrelation",
            vec![
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "Mode",
                    state.decorrelation_mode as f64,
                    "decorrelation_mode",
                    param_idx::DECORRELATION_MODE,
                    "",
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "LFO",
                    state.decorrelation_lfo_rate_hz,
                    "decorrelation_lfo_rate_hz",
                    param_idx::DECORRELATION_LFO_RATE_HZ,
                    "Hz",
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "Velvet",
                    state.velvet_noise_duration_ms,
                    "velvet_noise_duration_ms",
                    param_idx::VELVET_NOISE_DURATION_MS,
                    "ms",
                    theme,
                ),
                compact_lane(
                    d,
                    entity,
                    plugin_idx,
                    state,
                    "Density",
                    state.velvet_noise_density,
                    "velvet_noise_density",
                    param_idx::VELVET_NOISE_DENSITY,
                    "",
                    theme,
                ),
            ],
            theme,
        )
        .into_any_element(),
        7 => render_compact_lane_group(
            d,
            "Analysis",
            vec![
                render_compact_toggle_row(
                    d,
                    entity.clone(),
                    plugin_idx,
                    "Low Latency",
                    state.low_latency,
                    param_idx::LOW_LATENCY,
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "Resolution",
                    state.frequency_resolution as f64,
                    "frequency_resolution",
                    param_idx::FREQUENCY_RESOLUTION,
                    "",
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "Multi Src",
                    state.multi_source_threshold,
                    "multi_source_threshold",
                    param_idx::MULTI_SOURCE_THRESHOLD,
                    "",
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    "Auto Max",
                    state.auto_gain_max_db,
                    "auto_gain_max_db",
                    param_idx::AUTO_GAIN_MAX_DB,
                    "dB",
                    theme,
                ),
                compact_lane(
                    d,
                    entity,
                    plugin_idx,
                    state,
                    "Smooth",
                    state.auto_gain_smoothing_ms,
                    "auto_gain_smoothing_ms",
                    param_idx::AUTO_GAIN_SMOOTHING_MS,
                    "ms",
                    theme,
                ),
            ],
            theme,
        )
        .into_any_element(),
        8 => render_compact_lane_group(
            d,
            "Diagnostic",
            vec![
                render_compact_toggle_row(
                    d,
                    entity.clone(),
                    plugin_idx,
                    "Bypass All",
                    state.bypass_all_processing,
                    param_idx::BYPASS_ALL_PROCESSING,
                    theme,
                ),
                render_compact_toggle_row(
                    d,
                    entity.clone(),
                    plugin_idx,
                    "Bypass Decorr",
                    state.bypass_decorrelation,
                    param_idx::BYPASS_DECORRELATION,
                    theme,
                ),
                render_compact_toggle_row(
                    d,
                    entity.clone(),
                    plugin_idx,
                    "Bypass Trans",
                    state.bypass_transient_detection,
                    param_idx::BYPASS_TRANSIENT_DETECTION,
                    theme,
                ),
                render_compact_toggle_row(
                    d,
                    entity.clone(),
                    plugin_idx,
                    "ML Detect",
                    state.enable_ml_detection,
                    param_idx::ENABLE_ML_DETECTION,
                    theme,
                ),
                render_compact_toggle_row(
                    d,
                    entity.clone(),
                    plugin_idx,
                    "Multi Source",
                    state.multi_source_extraction,
                    param_idx::MULTI_SOURCE_EXTRACTION,
                    theme,
                ),
                render_compact_toggle_row(
                    d,
                    entity.clone(),
                    plugin_idx,
                    "Binaural",
                    state.binaural_preview,
                    param_idx::BINAURAL_PREVIEW,
                    theme,
                ),
                render_compact_toggle_row(
                    d,
                    entity,
                    plugin_idx,
                    "Auto Gain",
                    state.auto_gain_enabled,
                    param_idx::AUTO_GAIN_ENABLED,
                    theme,
                ),
            ],
            theme,
        )
        .into_any_element(),
        9 => div()
            .flex()
            .flex_col()
            .gap(d.gap)
            .child(render_section_header(d, "Spatial", theme))
            .child(render_spider_controls(d, entity, plugin_idx, state, theme))
            .into_any_element(),
        _ => div().into_any_element(),
    }
}

#[allow(clippy::too_many_arguments)]
fn compact_lane(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    label: &'static str,
    value: f64,
    param_key: &'static str,
    param_idx: usize,
    unit: &'static str,
    theme: &Theme,
) -> AnyElement {
    render_control_lane(
        d,
        entity,
        plugin_idx,
        ControlLaneSpec {
            label,
            value,
            min: pk(UP, param_key).min_f64(),
            max: pk(UP, param_key).max_f64(),
            unit,
            param_idx,
            selected_param: state.selected_param,
            is_editing: state.is_editing,
        },
        theme,
    )
}

fn render_compact_lane_group(
    d: &Ds,
    label: &'static str,
    lanes: Vec<AnyElement>,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(d.grid)
        .min_w_0()
        .child(render_section_header(d, label, theme))
        .child(div().flex().flex_col().gap(d.grid).children(lanes))
}

fn render_compact_toggle_row(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    label: &'static str,
    checked: bool,
    param_idx: usize,
    theme: &Theme,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(d.gap)
        .min_w(rems(14.0))
        .px(d.pad_y)
        .py(d.pad_y_half)
        .rounded(d.r_sm)
        .bg(theme.surface)
        .border_1()
        .border_color(theme.border)
        .child(
            div()
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_secondary)
                .child(label),
        )
        .child(
            Toggle::new(("upmix-compact-toggle", plugin_idx * 1000 + param_idx))
                .checked(checked)
                .style(ToggleStyle::Segmented)
                .theme(theme.to_toggle_theme())
                .on_change(move |new_value, _, cx| {
                    entity.update(cx, |state, _| {
                        state.app.set_plugin_param(
                            plugin_idx,
                            param_idx,
                            if new_value { 1.0 } else { 0.0 },
                        );
                    });
                }),
        )
        .into_any_element()
}

fn render_upmixer_header(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap(d.gap)
        .child(render_speaker_config_selector(
            d,
            entity.clone(),
            plugin_idx,
            state,
            theme,
        ))
        .child(render_spider_controls(d, entity, plugin_idx, state, theme))
}

fn render_upmixer_summary_strip(
    d: &Ds,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_wrap()
        .gap(d.grid)
        .px(d.pad_y)
        .py(d.pad_y_half)
        .bg(theme.surface)
        .rounded(d.r_md)
        .child(render_summary_chip(
            d,
            "Out",
            state.speaker_config.to_string(),
            theme,
        ))
        .child(render_summary_chip(
            d,
            "Width",
            format_compact_value(state.stereo_width, ""),
            theme,
        ))
        .child(render_summary_chip(
            d,
            "Center",
            format_compact_value(state.center_spread, ""),
            theme,
        ))
        .child(render_summary_chip(
            d,
            "Rear",
            format_compact_value(state.gain_rear_ambient, "x"),
            theme,
        ))
        .child(render_summary_chip(
            d,
            "Top",
            format_compact_value(state.height_gain, "x"),
            theme,
        ))
        .child(render_summary_chip(
            d,
            "LFE",
            format_compact_value(state.lfe_gain, "x"),
            theme,
        ))
        .child(render_summary_chip(
            d,
            "Cap",
            format_compact_value(state.safety_cap_db, "dB"),
            theme,
        ))
}

fn render_summary_chip(
    d: &Ds,
    label: &'static str,
    value: impl Into<SharedString>,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(3.0))
        .px(d.pad_y)
        .py(d.pad_y_half)
        .rounded(d.r_sm)
        .bg(theme.background_secondary)
        .text_size(d.text_xs)
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .child(label),
        )
        .child(div().text_color(theme.text_primary).child(value.into()))
}

fn render_spider_controls(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> AnyElement {
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

fn render_primary_control_strip(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_wrap()
        .gap(d.gap)
        .child(render_control_group(
            d,
            "Gains",
            vec![
                render_control_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    ControlLaneSpec {
                        label: "Mains",
                        value: state.gain_front_direct,
                        min: pk(UP, "gain_front_direct").min_f64(),
                        max: pk(UP, "gain_front_direct").max_f64(),
                        unit: "x",
                        param_idx: param_idx::GAIN_FRONT_DIRECT,
                        selected_param: state.selected_param,
                        is_editing: state.is_editing,
                    },
                    theme,
                ),
                render_control_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    ControlLaneSpec {
                        label: "Center",
                        value: state.gain_front_ambient,
                        min: pk(UP, "gain_front_ambient").min_f64(),
                        max: pk(UP, "gain_front_ambient").max_f64(),
                        unit: "x",
                        param_idx: param_idx::GAIN_FRONT_AMBIENT,
                        selected_param: state.selected_param,
                        is_editing: state.is_editing,
                    },
                    theme,
                ),
                render_control_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    ControlLaneSpec {
                        label: "Surr",
                        value: state.gain_rear_ambient,
                        min: pk(UP, "gain_rear_ambient").min_f64(),
                        max: pk(UP, "gain_rear_ambient").max_f64(),
                        unit: "x",
                        param_idx: param_idx::GAIN_REAR_AMBIENT,
                        selected_param: state.selected_param,
                        is_editing: state.is_editing,
                    },
                    theme,
                ),
                render_control_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    ControlLaneSpec {
                        label: "Top",
                        value: state.height_gain,
                        min: pk(UP, "height_gain").min_f64(),
                        max: pk(UP, "height_gain").max_f64(),
                        unit: "x",
                        param_idx: param_idx::HEIGHT_GAIN,
                        selected_param: state.selected_param,
                        is_editing: state.is_editing,
                    },
                    theme,
                ),
                render_control_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    ControlLaneSpec {
                        label: "LFE",
                        value: state.lfe_gain,
                        min: pk(UP, "lfe_gain").min_f64(),
                        max: pk(UP, "lfe_gain").max_f64(),
                        unit: "x",
                        param_idx: param_idx::LFE_GAIN,
                        selected_param: state.selected_param,
                        is_editing: state.is_editing,
                    },
                    theme,
                ),
                render_control_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    ControlLaneSpec {
                        label: "Safety",
                        value: state.safety_cap_db,
                        min: pk(UP, "safety_cap_db").min_f64(),
                        max: pk(UP, "safety_cap_db").max_f64(),
                        unit: "dB",
                        param_idx: param_idx::SAFETY_CAP_DB,
                        selected_param: state.selected_param,
                        is_editing: state.is_editing,
                    },
                    theme,
                ),
            ],
            theme,
        ))
        .child(render_control_group(
            d,
            "Space",
            vec![
                render_control_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    ControlLaneSpec {
                        label: "Width",
                        value: state.stereo_width,
                        min: pk(UP, "stereo_width").min_f64(),
                        max: pk(UP, "stereo_width").max_f64(),
                        unit: "",
                        param_idx: param_idx::STEREO_WIDTH,
                        selected_param: state.selected_param,
                        is_editing: state.is_editing,
                    },
                    theme,
                ),
                render_control_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    ControlLaneSpec {
                        label: "Spread",
                        value: state.center_spread,
                        min: pk(UP, "center_spread").min_f64(),
                        max: pk(UP, "center_spread").max_f64(),
                        unit: "",
                        param_idx: param_idx::CENTER_SPREAD,
                        selected_param: state.selected_param,
                        is_editing: state.is_editing,
                    },
                    theme,
                ),
                render_control_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    ControlLaneSpec {
                        label: "Bleed",
                        value: state.surround_direct_bleed,
                        min: pk(UP, "surround_direct_bleed").min_f64(),
                        max: pk(UP, "surround_direct_bleed").max_f64(),
                        unit: "",
                        param_idx: param_idx::SURROUND_DIRECT_BLEED,
                        selected_param: state.selected_param,
                        is_editing: state.is_editing,
                    },
                    theme,
                ),
                render_control_lane(
                    d,
                    entity,
                    plugin_idx,
                    ControlLaneSpec {
                        label: "Reflect",
                        value: state.rear_late_reflection,
                        min: pk(UP, "rear_late_reflection").min_f64(),
                        max: pk(UP, "rear_late_reflection").max_f64(),
                        unit: "",
                        param_idx: param_idx::REAR_LATE_REFLECTION,
                        selected_param: state.selected_param,
                        is_editing: state.is_editing,
                    },
                    theme,
                ),
            ],
            theme,
        ))
}

fn render_control_group(
    d: &Ds,
    label: &'static str,
    lanes: Vec<AnyElement>,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(d.grid)
        .flex_1()
        .min_w(rems(16.0))
        .child(render_section_header(d, label, theme))
        .child(div().flex().flex_wrap().gap(d.grid).children(lanes))
}

struct ControlLaneSpec {
    label: &'static str,
    value: f64,
    min: f64,
    max: f64,
    unit: &'static str,
    param_idx: usize,
    selected_param: usize,
    is_editing: bool,
}

fn render_control_lane(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    spec: ControlLaneSpec,
    theme: &Theme,
) -> AnyElement {
    let is_selected = spec.selected_param == spec.param_idx && spec.is_editing;
    let value = spec.value.clamp(spec.min, spec.max);
    let slider_width = 132.0;
    let value_label = format_compact_value(value, spec.unit);

    div()
        .flex()
        .items_center()
        .gap(d.grid)
        .min_w(rems(14.0))
        .px(d.pad_y)
        .py(d.pad_y_half)
        .rounded(d.r_sm)
        .bg(if is_selected {
            theme.accent_muted
        } else {
            theme.surface
        })
        .border_1()
        .border_color(if is_selected {
            theme.accent
        } else {
            theme.border
        })
        .child(
            div()
                .min_w(rems(3.8))
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_secondary)
                .child(spec.label),
        )
        .child(
            Slider::new(("upmix-lane", plugin_idx * 1000 + spec.param_idx))
                .value(value as f32)
                .range(spec.min as f32, spec.max as f32)
                .width(slider_width)
                .size(SliderSize::Sm)
                .theme(theme.to_slider_theme())
                .aria_label(format!("{} {}", spec.label, value_label))
                .on_drag_start({
                    let entity = entity.clone();
                    move |_, _, _, cx| {
                        entity.update(cx, |state, _| {
                            state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                            state.app.plugin_state.plugin_param_selection = spec.param_idx;
                        });
                    }
                })
                .on_change({
                    let entity = entity.clone();
                    move |new_value, _, cx| {
                        entity.update(cx, |state, _| {
                            state.app.set_plugin_param(
                                plugin_idx,
                                spec.param_idx,
                                new_value as f64,
                            );
                        });
                    }
                })
                .on_reset(move |_, cx| {
                    entity.update(cx, |state, _| {
                        state.app.reset_plugin_param(plugin_idx, spec.param_idx);
                    });
                }),
        )
        .child(
            div()
                .min_w(rems(3.2))
                .text_align(TextAlign::Right)
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_primary)
                .child(value_label),
        )
        .into_any_element()
}

fn format_compact_value(value: f64, unit: &str) -> String {
    if unit == "Hz" {
        format!("{value:.0}Hz")
    } else if unit.is_empty() {
        format!("{value:.2}")
    } else if unit == "dB" {
        format!("{value:+.1}dB")
    } else {
        format!("{value:.2}{unit}")
    }
}

fn render_speaker_config_selector(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let labels = pk(UP, "speaker_config").choice_labels();
    div()
        .flex()
        .flex_col()
        .gap(d.grid)
        .p(d.pad_y)
        .bg(theme.surface)
        .rounded(d.r_lg)
        .child(render_section_header(d, "Output", theme))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(d.grid)
                .children(labels.iter().enumerate().map(|(i, label)| {
                    let is_active = *label == state.speaker_config;
                    let entity = entity.clone();
                    render_tab_button(
                        d,
                        match i {
                            0 => "upmix-speaker-51",
                            1 => "upmix-speaker-714",
                            2 => "upmix-speaker-916",
                            _ => "upmix-speaker-other",
                        },
                        label,
                        is_active,
                        theme,
                    )
                    .on_click(move |_, _, cx| {
                        entity.update(cx, |state, _| {
                            state.app.set_plugin_param(
                                plugin_idx,
                                param_idx::SPEAKER_CONFIG,
                                i as f64,
                            );
                        });
                    })
                })),
        )
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
    let content = render_config_content(d, entity, plugin_idx, selected_config, state, theme);

    div()
        .w_full()
        .p(d.pad_x)
        .bg(theme.background_secondary)
        .rounded(d.r_lg)
        .border_1()
        .border_color(theme.border)
        .child(content)
}

fn render_config_content(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    selected_config: usize,
    state: &UpmixerRenderState,
    theme: &Theme,
) -> AnyElement {
    match selected_config {
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
    }
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
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Bandpass",
                    state.bandpass_hz,
                    pk(UP, "bandpass_hz").min_f64(),
                    pk(UP, "bandpass_hz").max_f64(),
                    "Hz",
                    param_idx::BANDPASS_HZ,
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
                .child(render_diag_toggle(
                    d,
                    entity.clone(),
                    plugin_idx,
                    "Binaural",
                    state.binaural_preview,
                    param_idx::BINAURAL_PREVIEW,
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
                // Separator
                .child(div().w(px(1.0)).h(px(40.0)).bg(theme.border))
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
                            entity.clone(),
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
fn render_spider_graph_row(d: &Ds, state: &UpmixerRenderState, theme: &Theme) -> impl IntoElement {
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
