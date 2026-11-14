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
use gpui::{AnyElement, Entity, FontWeight, SharedString, div, rems};
use gpui_ui_kit::{
    Accordion, AccordionItem, AccordionMode, Button, ButtonSize, ButtonVariant, NumberInput,
    NumberInputSize, Slider, SliderSize, StackSpacing, VStack,
};
use sotf_plugins::param_specs::{ParamType, find_by_key as pk, upmixer::PARAMS as UP};

// Every lane uses the canonical numeric increment.
#[expect(
    clippy::cast_precision_loss,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
fn numeric_step(param_idx: usize) -> f64 {
    match UP[param_idx].param_type {
        ParamType::Float { step, .. } => step,
        ParamType::Int { step, .. } => step as f64,
        _ => unreachable!("upmixer numeric lane requires a numeric parameter"),
    }
}

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
            const IDS: [&str; 4] = [
                "cfg-bass-output",
                "cfg-spatial",
                "cfg-analysis",
                "cfg-diagnostics",
            ];
            CONFIG_ITEMS
                .iter()
                .zip(IDS)
                .enumerate()
                .map(|(i, (label, id))| UpmixerConfigSpec {
                    label,
                    id,
                    config_idx: i + 10,
                })
                .collect()
        })
        .as_slice()
}

// intentional: editor lanes use fixed rems min-widths so multi-lane
// sections cannot collapse at narrow widths; spacing and padding come
// from the Ds design tokens.
/// Render the upmixer plugin controls
#[must_use]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
pub fn render_upmixer_plugin(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: UpmixerRenderState,
    available_width: f32,
    layout_scale: f32,
    text: PluginCommonTranslations,
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
    let layout = UpmixerLayout::from_width(available_width / layout_scale.max(0.01));

    let main_area = match layout {
        UpmixerLayout::Wide => render_main_area(
            d,
            entity.clone(),
            plugin_idx,
            &state,
            layout_scale,
            text,
            theme,
        )
        .into_any_element(),
        UpmixerLayout::Medium => render_medium_area(
            d,
            entity.clone(),
            plugin_idx,
            selected_config,
            &state,
            layout_scale,
            text,
            theme,
        )
        .into_any_element(),
        UpmixerLayout::Narrow => render_narrow_area(
            d,
            entity.clone(),
            plugin_idx,
            selected_config,
            &state,
            layout_scale,
            text,
            theme,
        )
        .into_any_element(),
    };

    let tab_bar = render_tab_bar(
        d,
        entity.clone(),
        selected_config,
        layout,
        false,
        text,
        theme,
    );

    // Configuration row: conditional on one of the visible configuration tabs.
    let config_row = render_compact_config_content(
        d,
        entity.clone(),
        plugin_idx,
        focused_config_index(selected_config),
        &state,
        layout_scale,
        text,
        theme,
    );

    div()
        .w_full()
        .min_w_0()
        .bg(app_background)
        .rounded(d.r_lg)
        .flex()
        .justify_start()
        .p(d.pad_x)
        .id("upmixer-surface-scroll")
        // Keep the dense control surface usable when a zoomed or narrow
        // window cannot fit the minimum lane widths. The rack shell still
        // owns vertical scrolling; this local surface provides the missing
        // horizontal escape hatch instead of silently clipping controls.
        .overflow_x_scroll()
        .child(
            div().w_full().min_w_0().child(
                VStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(main_area)
                    .when(layout == UpmixerLayout::Wide, |el| el.child(tab_bar))
                    .when(
                        layout == UpmixerLayout::Wide
                            && (10..10 + upmixer_config_specs().len()).contains(&selected_config),
                        |el| el.child(config_row),
                    )
                    .build(),
            ),
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

/// Render a focusable tab-style button using the toolkit's keyboard and focus behavior.
fn render_tab_button(
    _d: &Ds,
    id: &'static str,
    label: &str,
    is_active: bool,
    theme: &Theme,
) -> Button {
    Button::new(id, label.to_string())
        .size(ButtonSize::Sm)
        .variant(if is_active {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Ghost
        })
        .selected(is_active)
        .aria_label(label.to_string())
        .theme(theme.to_button_theme())
}

fn select_upmixer_param(state: &mut AppState, plugin_idx: usize, param_idx: usize) {
    state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
    state.app.plugin_state.plugin_param_selection = param_idx;
}

/// Render the tab bar below the main blocks
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
fn render_tab_bar(
    d: &Ds,
    entity: Entity<AppState>,
    selected_config: usize,
    layout: UpmixerLayout,
    allow_toggle_off: bool,
    text: PluginCommonTranslations,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_wrap()
        .items_end()
        .justify_center()
        .w_full()
        .when(layout == UpmixerLayout::Narrow, gpui::Styled::justify_start)
        .border_b_1()
        .border_color(theme.border)
        .children(upmixer_config_specs().iter().map(|spec| {
            let config_idx = spec.config_idx;
            let is_active = selected_config == config_idx;
            let entity = entity.clone();
            let button = render_tab_button(d, spec.id, text.label(spec.label), is_active, theme)
                .on_click(move |_window, cx| {
                    entity.update(cx, |state, cx| {
                        state.app.plugin_ui.upmixer_tab =
                            if allow_toggle_off && state.app.plugin_ui.upmixer_tab == config_idx {
                                0
                            } else {
                                config_idx
                            };
                        cx.notify();
                    });
                });
            #[cfg(feature = "dev-api")]
            let button = {
                use crate::app::dev_api::DevTrackExt;
                button.dev_track(spec.id)
            };
            button
        }))
}

/// Render the main area: spatial field first, compact primary controls below.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
fn render_main_area(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    layout_scale: f32,
    text: PluginCommonTranslations,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_wrap()
        .items_start()
        .gap(d.gap_md)
        .pl(d.pad_x)
        .child(
            div()
                .flex_1()
                .min_w(rems(32.0))
                .flex()
                .flex_col()
                .child(render_primary_control_strip(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    layout_scale,
                    theme,
                ))
                .p(d.pad_y)
                .bg(theme.surface)
                .rounded(d.r_lg),
        )
        .when(
            state.spatial_spider.view_mode != super::super::spatial_spider::SpiderViewMode::Off,
            |row| {
                row.child(
                    div()
                        .flex_1()
                        .min_w(rems(18.0))
                        .p(d.pad_y)
                        .bg(theme.surface)
                        .rounded(d.r_lg)
                        .child(render_spider_graph_row(
                            d,
                            entity.clone(),
                            plugin_idx,
                            state,
                            text,
                            theme,
                        )),
                )
            },
        )
}

/// Render the medium main area: speaker field with a side inspector.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
fn render_medium_area(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    selected_config: usize,
    state: &UpmixerRenderState,
    layout_scale: f32,
    text: PluginCommonTranslations,
    theme: &Theme,
) -> impl IntoElement {
    let focused_config = focused_config_index(selected_config);

    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(d.gap_md)
        .child(render_primary_control_strip(
            d,
            entity.clone(),
            plugin_idx,
            state,
            layout_scale,
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
                        .child(render_spider_graph_row(
                            d,
                            entity.clone(),
                            plugin_idx,
                            state,
                            text,
                            theme,
                        )),
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
                        .child(render_tab_bar(
                            d,
                            entity.clone(),
                            focused_config,
                            UpmixerLayout::Medium,
                            false,
                            text,
                            theme,
                        ))
                        .child(render_compact_config_content(
                            d,
                            entity.clone(),
                            plugin_idx,
                            focused_config,
                            state,
                            layout_scale,
                            text,
                            theme,
                        )),
                ),
        )
        .child(render_upmixer_summary_strip(d, state, text, theme))
}

/// Render the narrow main area: stacked field and immediately reachable primary lanes.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
fn render_narrow_area(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    selected_config: usize,
    state: &UpmixerRenderState,
    layout_scale: f32,
    text: PluginCommonTranslations,
    theme: &Theme,
) -> impl IntoElement {
    let focused_config = focused_config_index(selected_config);

    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(d.gap)
        .child(render_primary_control_strip(
            d,
            entity.clone(),
            plugin_idx,
            state,
            layout_scale,
            theme,
        ))
        .child(
            div()
                .w_full()
                .min_w_0()
                .p(d.pad_y)
                .bg(theme.surface)
                .rounded(d.r_lg)
                .child(render_spider_graph_row(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    text,
                    theme,
                )),
        )
        .child(render_tab_bar(
            d,
            entity.clone(),
            focused_config,
            UpmixerLayout::Narrow,
            false,
            text,
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
                    layout_scale,
                    text,
                    theme,
                )),
        )
        .child(render_upmixer_summary_strip(d, state, text, theme))
}

fn focused_config_index(selected_config: usize) -> usize {
    if (10..10 + CONFIG_ITEMS.len()).contains(&selected_config) {
        selected_config
    } else {
        10
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "Keep the declarative layout and its interaction handlers together"
)]
#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
fn render_compact_config_content(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    selected_config: usize,
    state: &UpmixerRenderState,
    layout_scale: f32,
    text: PluginCommonTranslations,
    theme: &Theme,
) -> AnyElement {
    match selected_config {
        10 => render_upmixer_section_accordion(
            entity.clone(),
            theme,
            &state.expanded_sections,
            text.configuration,
            vec![
                (
                    "upmixer-lfe",
                    text.label("LFE & Bass"),
                    render_compact_config_content(
                        d,
                        entity.clone(),
                        plugin_idx,
                        1,
                        state,
                        layout_scale,
                        text,
                        theme,
                    ),
                ),
                (
                    "upmixer-output",
                    text.label("Output"),
                    render_compact_lane_group(
                        d,
                        "Output",
                        vec![
                            render_compact_toggle_row(
                                d,
                                entity.clone(),
                                plugin_idx,
                                text.label("Auto Gain"),
                                state.auto_gain_enabled,
                                param_idx::AUTO_GAIN_ENABLED,
                                theme,
                                text,
                            ),
                            compact_lane_enabled(
                                d,
                                entity.clone(),
                                plugin_idx,
                                state,
                                layout_scale,
                                "Auto Max",
                                state.auto_gain_max_db,
                                "auto_gain_max_db",
                                param_idx::AUTO_GAIN_MAX_DB,
                                "dB",
                                state.auto_gain_enabled,
                                theme,
                            ),
                            compact_lane_enabled(
                                d,
                                entity,
                                plugin_idx,
                                state,
                                layout_scale,
                                "Smooth",
                                state.auto_gain_smoothing_ms,
                                "auto_gain_smoothing_ms",
                                param_idx::AUTO_GAIN_SMOOTHING_MS,
                                "ms",
                                state.auto_gain_enabled,
                                theme,
                            ),
                        ],
                        theme,
                    )
                    .into_any_element(),
                ),
            ],
        ),
        11 => render_upmixer_section_accordion(
            entity.clone(),
            theme,
            &state.expanded_sections,
            text.configuration,
            vec![
                (
                    "upmixer-dialogue",
                    text.label("Dialogue"),
                    render_compact_config_content(
                        d,
                        entity.clone(),
                        plugin_idx,
                        2,
                        state,
                        layout_scale,
                        text,
                        theme,
                    ),
                ),
                (
                    "upmixer-ambient",
                    text.label("Ambient"),
                    render_compact_config_content(
                        d,
                        entity.clone(),
                        plugin_idx,
                        3,
                        state,
                        layout_scale,
                        text,
                        theme,
                    ),
                ),
                (
                    "upmixer-height",
                    text.label("Height"),
                    render_compact_config_content(
                        d,
                        entity.clone(),
                        plugin_idx,
                        4,
                        state,
                        layout_scale,
                        text,
                        theme,
                    ),
                ),
                (
                    "upmixer-hr-direct",
                    text.label("HR Direct"),
                    render_compact_config_content(
                        d,
                        entity.clone(),
                        plugin_idx,
                        5,
                        state,
                        layout_scale,
                        text,
                        theme,
                    ),
                ),
                (
                    "upmixer-decorrelation",
                    text.label("Decorrelation"),
                    render_compact_config_content(
                        d,
                        entity,
                        plugin_idx,
                        6,
                        state,
                        layout_scale,
                        text,
                        theme,
                    ),
                ),
            ],
        ),
        12 => render_compact_config_content(
            d,
            entity,
            plugin_idx,
            7,
            state,
            layout_scale,
            text,
            theme,
        ),
        13 => render_compact_config_content(
            d,
            entity,
            plugin_idx,
            8,
            state,
            layout_scale,
            text,
            theme,
        ),
        1 => render_compact_lane_group(
            d,
            "LFE & Bass",
            vec![
                render_compact_toggle_row(
                    d,
                    entity.clone(),
                    plugin_idx,
                    text.label("SubHarmonic"),
                    state.enable_subharmonic_synth,
                    param_idx::ENABLE_SUBHARMONIC_SYNTH,
                    theme,
                    text,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    layout_scale,
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
                    layout_scale,
                    "Bandpass",
                    state.bandpass_hz,
                    "bandpass_hz",
                    param_idx::BANDPASS_HZ,
                    "Hz",
                    theme,
                ),
                compact_lane_enabled(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    layout_scale,
                    "Sub Gain",
                    state.subharmonic_gain,
                    "subharmonic_gain",
                    param_idx::SUBHARMONIC_GAIN,
                    "x",
                    state.enable_subharmonic_synth,
                    theme,
                ),
                compact_lane_enabled(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    layout_scale,
                    "Sub Freq",
                    state.subharmonic_freq_hz,
                    "subharmonic_freq_hz",
                    param_idx::SUBHARMONIC_FREQ_HZ,
                    "Hz",
                    state.enable_subharmonic_synth,
                    theme,
                ),
                compact_lane_enabled(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    layout_scale,
                    "Attack",
                    state.subharmonic_attack_ms,
                    "subharmonic_attack_ms",
                    param_idx::SUBHARMONIC_ATTACK_MS,
                    "ms",
                    state.enable_subharmonic_synth,
                    theme,
                ),
                compact_lane_enabled(
                    d,
                    entity,
                    plugin_idx,
                    state,
                    layout_scale,
                    "Release",
                    state.subharmonic_release_ms,
                    "subharmonic_release_ms",
                    param_idx::SUBHARMONIC_RELEASE_MS,
                    "ms",
                    state.enable_subharmonic_synth,
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
                    layout_scale,
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
                    layout_scale,
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
                    layout_scale,
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
                    layout_scale,
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
                    layout_scale,
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
                    layout_scale,
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
                    layout_scale,
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
                    layout_scale,
                    text.label("Bleed"),
                    state.surround_direct_bleed,
                    "surround_direct_bleed",
                    param_idx::SURROUND_DIRECT_BLEED,
                    "",
                    theme,
                ),
                compact_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    layout_scale,
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
                    layout_scale,
                    text.label("Reflection"),
                    state.rear_late_reflection,
                    "rear_late_reflection",
                    param_idx::REAR_LATE_REFLECTION,
                    "",
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
                    layout_scale,
                    text.label("Top Gain"),
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
                    layout_scale,
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
                    layout_scale,
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
                    layout_scale,
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
                    text.label("HR Direct"),
                    state.enable_hr_direct,
                    param_idx::ENABLE_HR_DIRECT,
                    theme,
                    text,
                ),
                compact_lane_enabled(
                    d,
                    entity,
                    plugin_idx,
                    state,
                    layout_scale,
                    "Sharpen",
                    state.hr_sharpen,
                    "hr_sharpen",
                    param_idx::HR_SHARPEN,
                    "",
                    state.enable_hr_direct,
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
                render_compact_decorrelation_choices(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state.decorrelation_mode,
                    theme,
                ),
                compact_lane_enabled(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    layout_scale,
                    "LFO",
                    state.decorrelation_lfo_rate_hz,
                    "decorrelation_lfo_rate_hz",
                    param_idx::DECORRELATION_LFO_RATE_HZ,
                    "Hz",
                    state.decorrelation_mode == 1,
                    theme,
                ),
                compact_lane_enabled(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    layout_scale,
                    "Velvet",
                    state.velvet_noise_duration_ms,
                    "velvet_noise_duration_ms",
                    param_idx::VELVET_NOISE_DURATION_MS,
                    "ms",
                    state.decorrelation_mode == 0,
                    theme,
                ),
                compact_lane_enabled(
                    d,
                    entity,
                    plugin_idx,
                    state,
                    layout_scale,
                    "Density",
                    state.velvet_noise_density,
                    "velvet_noise_density",
                    param_idx::VELVET_NOISE_DENSITY,
                    "",
                    state.decorrelation_mode == 0,
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
                    text.label("Low Latency"),
                    state.low_latency,
                    param_idx::LOW_LATENCY,
                    theme,
                    text,
                ),
                render_compact_toggle_row(
                    d,
                    entity.clone(),
                    plugin_idx,
                    text.label("Binaural"),
                    state.binaural_preview,
                    param_idx::BINAURAL_PREVIEW,
                    theme,
                    text,
                ),
                render_compact_resolution_choices(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state.frequency_resolution,
                    theme,
                ),
                render_compact_toggle_row(
                    d,
                    entity.clone(),
                    plugin_idx,
                    text.label("Multi Source"),
                    state.multi_source_extraction,
                    param_idx::MULTI_SOURCE_EXTRACTION,
                    theme,
                    text,
                ),
                compact_lane_enabled(
                    d,
                    entity.clone(),
                    plugin_idx,
                    state,
                    layout_scale,
                    "Multi Src",
                    state.multi_source_threshold,
                    "multi_source_threshold",
                    param_idx::MULTI_SOURCE_THRESHOLD,
                    "",
                    state.multi_source_extraction,
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
                    text.label("Bypass All"),
                    diagnostic_param_value(state, param_idx::BYPASS_ALL_PROCESSING),
                    param_idx::BYPASS_ALL_PROCESSING,
                    theme,
                    text,
                ),
                render_compact_toggle_row(
                    d,
                    entity.clone(),
                    plugin_idx,
                    text.label("Bypass Decorr"),
                    diagnostic_param_value(state, param_idx::BYPASS_DECORRELATION),
                    param_idx::BYPASS_DECORRELATION,
                    theme,
                    text,
                ),
                render_compact_toggle_row(
                    d,
                    entity.clone(),
                    plugin_idx,
                    text.label("Bypass Trans"),
                    diagnostic_param_value(state, param_idx::BYPASS_TRANSIENT_DETECTION),
                    param_idx::BYPASS_TRANSIENT_DETECTION,
                    theme,
                    text,
                ),
                render_compact_toggle_row(
                    d,
                    entity.clone(),
                    plugin_idx,
                    text.label("ML Detect"),
                    diagnostic_param_value(state, param_idx::ENABLE_ML_DETECTION),
                    param_idx::ENABLE_ML_DETECTION,
                    theme,
                    text,
                ),
            ],
            theme,
        )
        .into_any_element(),
        _ => div().into_any_element(),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "GPUI rendering and callback interfaces accept the complete UI context"
)]
fn render_compact_resolution_choices(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    selected: usize,
    theme: &Theme,
) -> AnyElement {
    render_compact_choice_row(
        d,
        entity,
        plugin_idx,
        selected,
        param_idx::FREQUENCY_RESOLUTION,
        "Resolution",
        theme,
    )
}

fn render_compact_decorrelation_choices(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    selected: usize,
    theme: &Theme,
) -> AnyElement {
    render_compact_choice_row(
        d,
        entity,
        plugin_idx,
        selected,
        param_idx::DECORRELATION_MODE,
        "Mode",
        theme,
    )
}

#[expect(
    clippy::cast_precision_loss,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
fn render_compact_choice_row(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    selected: usize,
    param_idx: usize,
    label: &str,
    theme: &Theme,
) -> AnyElement {
    use gpui_ui_kit::{ButtonSet, ButtonSetOption, ButtonSetSize};
    let choices = ButtonSet::new(("upmixer-compact-choice", plugin_idx * 1000 + param_idx))
        .options(
            UP[param_idx]
                .choice_labels()
                .iter()
                .enumerate()
                .map(|(index, label)| ButtonSetOption::new(index.to_string(), *label))
                .collect(),
        )
        .selected(selected.to_string())
        .size(ButtonSetSize::Xs)
        .theme(theme.to_button_set_theme())
        .on_change(move |value, _, cx| {
            if let Ok(value) = value.parse::<usize>() {
                entity.update(cx, |state, cx| {
                    select_upmixer_param(state, plugin_idx, param_idx);
                    state
                        .app
                        .set_plugin_param(plugin_idx, param_idx, value as f64);
                    cx.notify();
                });
            }
        });
    #[cfg(feature = "dev-api")]
    let choices = {
        use crate::app::dev_api::DevTrackExt;
        choices.option_wrapper(move |index, option| {
            let option = option.dev_track(format!(
                "plugin.upmixer.choice.{plugin_idx}.{param_idx}.{index}"
            ));
            if param_idx == param_idx::FREQUENCY_RESOLUTION {
                option
                    .dev_track(match index {
                        0 => "compact-freq-erb",
                        1 => "compact-freq-fine",
                        _ => "compact-freq-bin",
                    })
                    .into_any_element()
            } else {
                option.into_any_element()
            }
        })
    };
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(d.grid)
        .min_w(rems(14.0))
        .px(d.pad_y)
        .py(d.pad_y_half)
        .rounded(d.r_sm)
        .bg(theme.surface)
        .border_1()
        .border_color(theme.border)
        .child(
            div()
                .min_w(rems(3.8))
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_secondary)
                .child(label.to_string()),
        )
        .child(choices)
        .into_any_element()
}

fn compact_lane(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    layout_scale: f32,
    label: &'static str,
    value: f64,
    param_key: &'static str,
    param_idx: usize,
    unit: &'static str,
    theme: &Theme,
) -> AnyElement {
    compact_lane_enabled(
        d,
        entity,
        plugin_idx,
        state,
        layout_scale,
        label,
        value,
        param_key,
        param_idx,
        unit,
        true,
        theme,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "GPUI rendering and callback interfaces accept the complete UI context"
)]
fn compact_lane_enabled(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    layout_scale: f32,
    label: &'static str,
    value: f64,
    param_key: &'static str,
    param_idx: usize,
    unit: &'static str,
    interactive: bool,
    theme: &Theme,
) -> AnyElement {
    render_control_lane_enabled(
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
        interactive,
        layout_scale,
        theme,
    )
}

fn render_compact_lane_group(
    d: &Ds,
    _label: &'static str,
    lanes: Vec<AnyElement>,
    _theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(d.grid)
        .min_w_0()
        .child(div().flex().flex_col().gap(d.grid).children(lanes))
}

#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
fn render_compact_toggle_row(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    label: &str,
    checked: bool,
    param_idx: usize,
    theme: &Theme,
    text: PluginCommonTranslations,
) -> AnyElement {
    let toggle =
        gpui_ui_kit::ButtonSet::new(("upmix-compact-toggle", plugin_idx * 1000 + param_idx))
            .options(vec![
                gpui_ui_kit::ButtonSetOption::new("0", text.label("Off")),
                gpui_ui_kit::ButtonSetOption::new("1", text.label("On")),
            ])
            .selected(if checked { "1" } else { "0" })
            .size(gpui_ui_kit::ButtonSetSize::Xs)
            .theme(theme.to_button_set_theme())
            .on_change(move |value, _, cx| {
                entity.update(cx, |state, cx| {
                    select_upmixer_param(state, plugin_idx, param_idx);
                    state.app.set_plugin_param(
                        plugin_idx,
                        param_idx,
                        if value.as_ref() == "1" { 1.0 } else { 0.0 },
                    );
                    cx.notify();
                });
            });
    #[cfg(feature = "dev-api")]
    let toggle = {
        use crate::app::dev_api::DevTrackExt;
        toggle
            .dev_track(format!("plugin.upmixer.control.{plugin_idx}.{param_idx}"))
            .dev_track(format!("plugin.upmixer.toggle.{plugin_idx}.{param_idx}"))
    };
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
                .min_w(rems(3.8))
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_secondary)
                .child(label.to_string()),
        )
        .child(toggle)
        .into_any_element()
}

#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
fn render_upmixer_summary_strip(
    d: &Ds,
    state: &UpmixerRenderState,
    text: PluginCommonTranslations,
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
            text.upmixer_summary.output,
            state.speaker_config.to_string(),
            theme,
        ))
        .child(render_summary_chip(
            d,
            text.upmixer_summary.width,
            format_compact_value(state.stereo_width, ""),
            theme,
        ))
        .child(render_summary_chip(
            d,
            text.upmixer_summary.center,
            format_compact_value(state.center_spread, ""),
            theme,
        ))
        .child(render_summary_chip(
            d,
            text.upmixer_summary.rear,
            format_compact_value(state.gain_rear_ambient, "x"),
            theme,
        ))
        .child(render_summary_chip(
            d,
            text.upmixer_summary.height,
            format_compact_value(state.height_gain, "x"),
            theme,
        ))
        .child(render_summary_chip(
            d,
            text.upmixer_summary.lfe,
            format_compact_value(state.lfe_gain, "x"),
            theme,
        ))
        .child(render_summary_chip(
            d,
            text.upmixer_summary.cap,
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
        .gap(d.grid)
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

#[expect(
    clippy::too_many_lines,
    reason = "Keep the declarative layout and its interaction handlers together"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
fn render_primary_control_strip(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    layout_scale: f32,
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
                        label: "Direct",
                        value: state.gain_front_direct,
                        min: pk(UP, "gain_front_direct").min_f64(),
                        max: pk(UP, "gain_front_direct").max_f64(),
                        unit: "x",
                        param_idx: param_idx::GAIN_FRONT_DIRECT,
                        selected_param: state.selected_param,
                        is_editing: state.is_editing,
                    },
                    layout_scale,
                    theme,
                ),
                render_control_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    ControlLaneSpec {
                        label: "Front Ambient",
                        value: state.gain_front_ambient,
                        min: pk(UP, "gain_front_ambient").min_f64(),
                        max: pk(UP, "gain_front_ambient").max_f64(),
                        unit: "x",
                        param_idx: param_idx::GAIN_FRONT_AMBIENT,
                        selected_param: state.selected_param,
                        is_editing: state.is_editing,
                    },
                    layout_scale,
                    theme,
                ),
                render_control_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    ControlLaneSpec {
                        label: "Rear",
                        value: state.gain_rear_ambient,
                        min: pk(UP, "gain_rear_ambient").min_f64(),
                        max: pk(UP, "gain_rear_ambient").max_f64(),
                        unit: "x",
                        param_idx: param_idx::GAIN_REAR_AMBIENT,
                        selected_param: state.selected_param,
                        is_editing: state.is_editing,
                    },
                    layout_scale,
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
                    layout_scale,
                    theme,
                ),
            ],
            theme,
        ))
        .child(render_control_group(
            d,
            "Image",
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
                    layout_scale,
                    theme,
                ),
                render_control_lane(
                    d,
                    entity.clone(),
                    plugin_idx,
                    ControlLaneSpec {
                        label: "Center",
                        value: state.center_spread,
                        min: pk(UP, "center_spread").min_f64(),
                        max: pk(UP, "center_spread").max_f64(),
                        unit: "",
                        param_idx: param_idx::CENTER_SPREAD,
                        selected_param: state.selected_param,
                        is_editing: state.is_editing,
                    },
                    layout_scale,
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
                    layout_scale,
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

// Fit label, slider and value inside the 16-rem compact lane minimum.
const CONTROL_LANE_SLIDER_WIDTH_AT_BASE_SCALE: f32 = 96.0;

fn scaled_control_lane_slider_width(layout_scale: f32) -> f32 {
    CONTROL_LANE_SLIDER_WIDTH_AT_BASE_SCALE * layout_scale.max(0.01)
}

#[derive(Clone, Copy)]
struct ControlLaneRange {
    value: f64,
    min: f64,
    max: f64,
}

#[expect(
    clippy::float_cmp,
    reason = "Exact parameter choices and changed-value detection preserve control semantics"
)]
fn sanitize_control_lane_range(spec: &ControlLaneSpec) -> ControlLaneRange {
    let (mut min, mut max) = if spec.min.is_finite() && spec.max.is_finite() {
        (spec.min.min(spec.max), spec.min.max(spec.max))
    } else if spec.value.is_finite() {
        (spec.value - 1.0, spec.value + 1.0)
    } else {
        (0.0, 1.0)
    };

    if min == max {
        let pad = min.abs().max(1.0) * 0.01;
        min -= pad;
        max += pad;
    }

    let value = if spec.value.is_finite() {
        spec.value.clamp(min, max)
    } else {
        min
    };

    if value != spec.value || min != spec.min || max != spec.max {
        log::warn!(
            "Invalid upmixer control range for '{}': value={}, min={}, max={}; using value={}, min={}, max={}",
            spec.label,
            spec.value,
            spec.min,
            spec.max,
            value,
            min,
            max
        );
    }

    ControlLaneRange { value, min, max }
}

fn render_control_lane(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    spec: ControlLaneSpec,
    layout_scale: f32,
    theme: &Theme,
) -> AnyElement {
    render_control_lane_enabled(d, entity, plugin_idx, spec, true, layout_scale, theme)
}

#[expect(
    clippy::too_many_lines,
    reason = "Keep the declarative layout and its interaction handlers together"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
fn render_control_lane_enabled(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    spec: ControlLaneSpec,
    interactive: bool,
    layout_scale: f32,
    theme: &Theme,
) -> AnyElement {
    let is_selected = spec.selected_param == spec.param_idx && spec.is_editing;
    let range = sanitize_control_lane_range(&spec);
    let value = range.value;
    let exact_input = NumberInput::new(SharedString::from(format!(
        "upmix-number-input-{plugin_idx}-{}",
        spec.param_idx
    )))
    .value(value)
    .min(range.min)
    .max(range.max)
    .step(numeric_step(spec.param_idx))
    .decimals(UP[spec.param_idx].precision())
    .unit(spec.unit)
    .size(NumberInputSize::Xs)
    .theme(theme.to_number_input_theme())
    .disabled(!interactive)
    .aria_label(format!("{} value", spec.label))
    .on_change({
        let entity = entity.clone();
        move |new_value, _window, cx| {
            entity.update(cx, |state, _| {
                state
                    .app
                    .set_plugin_param(plugin_idx, spec.param_idx, new_value);
            });
        }
    });
    #[cfg(feature = "dev-api")]
    let exact_input = {
        use crate::app::dev_api::DevTrackExt;
        exact_input.dev_track(format!(
            "plugin.upmixer.exact.{plugin_idx}.{}",
            spec.param_idx
        ))
    };
    let slider_width = scaled_control_lane_slider_width(layout_scale);
    let value_label = format_compact_value(value, spec.unit);
    let slider = Slider::new(("upmix-lane", plugin_idx * 1000 + spec.param_idx))
        .value(value as f32)
        .range(range.min as f32, range.max as f32)
        .step(numeric_step(spec.param_idx) as f32)
        .logarithmic(range.min > 0.0 && matches!(spec.unit, "Hz" | "ms"))
        .width(slider_width)
        .size(SliderSize::Sm)
        .theme(theme.to_slider_theme())
        .disabled(!interactive)
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
                    select_upmixer_param(state, plugin_idx, spec.param_idx);
                    state
                        .app
                        .set_plugin_param(plugin_idx, spec.param_idx, f64::from(new_value));
                });
            }
        })
        .on_reset(move |_, cx| {
            entity.update(cx, |state, _| {
                select_upmixer_param(state, plugin_idx, spec.param_idx);
                state.app.reset_plugin_param(plugin_idx, spec.param_idx);
            });
        });
    #[cfg(feature = "dev-api")]
    let slider = {
        use crate::app::dev_api::DevTrackExt;
        slider.dev_track(format!(
            "plugin.upmixer.slider.{plugin_idx}.{}",
            spec.param_idx
        ))
    };

    let row = div()
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
        .when(!interactive, |el| el.opacity(0.45))
        .child(
            div()
                .min_w(rems(3.8))
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_secondary)
                .child(spec.label),
        )
        .child(slider)
        .child(exact_input);
    #[cfg(feature = "dev-api")]
    let row = {
        use crate::app::dev_api::DevTrackExt;
        row.dev_track(format!(
            "plugin.upmixer.lane.{plugin_idx}.{}",
            spec.param_idx
        ))
    };
    row.into_any_element()
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

fn render_upmixer_section_accordion(
    entity: Entity<AppState>,
    theme: &Theme,
    expanded_sections: &[String],
    aria_label: &'static str,
    sections: Vec<(&'static str, &'static str, AnyElement)>,
) -> AnyElement {
    let expanded_ids = expanded_sections
        .iter()
        .cloned()
        .map(SharedString::from)
        .collect();
    let items = sections
        .into_iter()
        .map(|(id, title, content)| AccordionItem::new(id, title).content(content))
        .collect::<Vec<_>>();
    let state_handle = entity;

    let accordion = Accordion::new()
        .items(items)
        .mode(AccordionMode::Multiple)
        .expanded(expanded_ids)
        .bordered(false)
        .rounded(false)
        .theme(theme.to_accordion_theme())
        .aria_label(aria_label);
    #[cfg(feature = "dev-api")]
    let accordion = accordion.header_wrapper(move |id, header| {
        use crate::app::dev_api::DevTrackExt;
        header
            .dev_track(format!("upmixer.section.{id}"))
            .into_any_element()
    });
    accordion
        .on_change(move |id, is_expanded, _window, cx| {
            let id = id.to_string();
            state_handle.update(cx, |state, _| {
                state
                    .app
                    .plugin_ui
                    .upmixer_expanded_sections
                    .retain(|expanded| expanded != &id);
                if is_expanded {
                    state.app.plugin_ui.upmixer_expanded_sections.push(id);
                }
            });
        })
        .into_any_element()
}

/// Render a section header
fn render_section_header(d: &Ds, label: &str, theme: &Theme) -> impl IntoElement {
    div()
        .text_size(d.text_sm)
        .font_weight(FontWeight::BOLD)
        .text_color(theme.text_muted)
        .pb(d.pad_y_half)
        .child(label.to_string())
}

/// Permanent spider graph row below the tab bar. Always visible regardless
/// of which tab is selected, so the user can watch the spatial field while
/// editing parameters in any tab.
#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
fn render_spider_graph_row(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &UpmixerRenderState,
    text: PluginCommonTranslations,
    theme: &Theme,
) -> AnyElement {
    use crate::components::plugins::spatial_spider::{
        SpatialSpiderSnapshot, render_spatial_spider_graph, render_spatial_spider_metric_controls,
        resolve_speaker_config,
    };

    if state.spatial_spider.view_mode == super::super::spatial_spider::SpiderViewMode::Off {
        return div().into_any_element();
    }
    let snapshot = SpatialSpiderSnapshot {
        loudness: state.loudness_info.clone(),
        ui: state.spatial_spider.clone(),
    };
    let cfg_opt = resolve_speaker_config(&snapshot, Some(state.speaker_config));
    div()
        .w_full()
        .pt(d.pad_y)
        .child(render_spatial_spider_metric_controls(
            d, entity, plugin_idx, &snapshot, cfg_opt, text, theme,
        ))
        .child(render_spatial_spider_graph(
            d, &snapshot, cfg_opt, text, theme,
        ))
        .into_any_element()
}
use crate::app::i18n::PluginCommonTranslations;
