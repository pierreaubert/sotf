//! Dynamic EQ plugin UI.

use super::common::{render_knob_sized, render_toggle};
use crate::app::AppState;
use crate::components::design::Ds;
use crate::components::graphs::common::rgba_to_u32;
use crate::components::graphs::response_graphs::{ChartConfig, Series, render_line_chart};
use crate::components::plugins::render_gr_meter;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_audio_kit::PotentiometerSize;
use gpui_px::ScaleType;
use sotf_audio_player::PluginSettings;
use sotf_plugins::DynamicEqData;
use sotf_plugins::param_specs::{dynamic_eq as specs, find_by_key as pk};
use std::any::Any;
use std::sync::Arc;

const BAND_PARAM_BASE: usize = 100;
const BAND_PARAM_STRIDE: usize = 10;

pub fn render_dynamic_eq_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    is_editing: bool,
    selected_param: usize,
    selected_band_idx: usize,
    plugin_data: Option<Arc<dyn Any + Send + Sync>>,
    available_width: f32,
    layout_scale: f32,
    theme: &Theme,
    cx: &mut Context<PlayerView>,
) -> impl IntoElement {
    let d = Ds::from_cx(cx);
    let text = PluginCommonTranslations::for_language(entity.read(cx).app.ui_state.language);
    let sample_rate = entity
        .read(cx)
        .app
        .audio_device_state
        .hal_config
        .sample_rate;
    let PluginSettings::DynamicEq {
        num_bands,
        threshold,
        ratio,
        attack,
        release,
        knee,
        link_channels,
        mix,
        bands,
    } = settings
    else {
        return div().into_any_element();
    };

    let num_bands = (*num_bands as usize).clamp(1, 8);
    let selected_band_idx = selected_band_idx.min(num_bands.saturating_sub(1));
    let compact = available_width / layout_scale.max(0.01) < 720.0;
    let gr = plugin_data
        .and_then(|data| data.downcast::<DynamicEqData>().ok())
        .map(|data| data.gain_reduction_db.clone());

    div()
        .flex()
        .flex_col()
        .gap(d.section_lg)
        .w_full()
        .child(render_global_controls(
            &d,
            entity.clone(),
            plugin_idx,
            num_bands as f64,
            *threshold,
            *ratio,
            *attack,
            *release,
            *knee,
            *link_channels,
            *mix,
            selected_param,
            is_editing,
            compact,
            text,
            theme,
        ))
        .child(render_dynamic_eq_response(
            &d,
            bands,
            num_bands,
            selected_band_idx,
            gr.as_deref(),
            sample_rate as f64,
            available_width,
            text,
            theme,
        ))
        .child(render_band_tabs(
            &d,
            entity.clone(),
            plugin_idx,
            bands,
            num_bands,
            selected_band_idx,
            gr.as_deref(),
            theme,
        ))
        .child(render_selected_band(
            &d,
            entity,
            plugin_idx,
            bands,
            selected_band_idx,
            selected_param,
            is_editing,
            compact,
            text,
            theme,
        ))
        .into_any_element()
}

/// Render the static response of the active bands and a live GR meter for the
/// selected band. The response is intentionally derived from the same
/// peaking-EQ coefficients used by the DSP, rather than from a decorative
/// bell-shaped approximation, so changing frequency, Q, or gain is visible in
/// the chart immediately.
#[allow(clippy::too_many_arguments)]
fn render_dynamic_eq_response(
    d: &Ds,
    bands: &[sotf_plugins::DynEqBandParams],
    num_bands: usize,
    selected_band_idx: usize,
    gain_reduction_db: Option<&Vec<f32>>,
    sample_rate: f64,
    available_width: f32,
    text: PluginCommonTranslations,
    theme: &Theme,
) -> AnyElement {
    const POINTS: usize = 160;
    const MIN_FREQ: f64 = 20.0;
    const DEFAULT_SAMPLE_RATE: f64 = 48_000.0;

    let sample_rate = if sample_rate.is_finite() && sample_rate >= 8_000.0 {
        sample_rate
    } else {
        DEFAULT_SAMPLE_RATE
    };
    let max_freq = (sample_rate * 0.475).clamp(MIN_FREQ * 2.0, 20_000.0);
    let frequencies: Vec<f64> = (0..POINTS)
        .map(|index| {
            let position = index as f64 / (POINTS - 1) as f64;
            (MIN_FREQ.ln() + position * (max_freq.ln() - MIN_FREQ.ln())).exp()
        })
        .collect();

    let has_solo = bands
        .iter()
        .take(num_bands)
        .any(|band| band.active && band.solo);
    let audible_band =
        |band: &&sotf_plugins::DynEqBandParams| band.active && (!has_solo || band.solo);
    let active_bands: Vec<&sotf_plugins::DynEqBandParams> =
        bands.iter().take(num_bands).filter(audible_band).collect();

    let combined: Vec<f64> = frequencies
        .iter()
        .map(|frequency| {
            active_bands
                .iter()
                .map(|band| {
                    peaking_eq_response_db(
                        *frequency,
                        f64::from(band.frequency),
                        f64::from(band.q),
                        f64::from(band.gain),
                        sample_rate,
                    )
                })
                .sum()
        })
        .collect();

    let y_limit = (combined
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max)
        + 3.0)
        .clamp(12.0, 36.0);
    let chart_width = available_width.clamp(280.0, 900.0);
    let chart_height = (chart_width * 0.34).clamp(130.0, 210.0);

    let mut series = vec![
        Series::new(
            "Combined",
            rgba_to_u32(theme.accent),
            frequencies.clone(),
            combined,
        )
        .with_width(2.5),
    ];

    if let Some(selected_band) = bands.get(selected_band_idx).filter(audible_band) {
        let selected_response = frequencies
            .iter()
            .map(|frequency| {
                peaking_eq_response_db(
                    *frequency,
                    f64::from(selected_band.frequency),
                    f64::from(selected_band.q),
                    f64::from(selected_band.gain),
                    sample_rate,
                )
            })
            .collect();
        series.push(
            Series::new(
                format!("{} {}", text.label("Band"), selected_band_idx + 1),
                rgba_to_u32(theme.warning),
                frequencies,
                selected_response,
            )
            .with_width(1.5)
            .with_opacity(0.85),
        );
    }

    let chart = render_line_chart(
        series,
        ChartConfig {
            title: None,
            x_label: Some("Hz".to_string()),
            y_label: Some("dB".to_string()),
            x_range: (MIN_FREQ, max_freq),
            y_range: (-y_limit, y_limit),
            x_scale: ScaleType::Log,
            width: chart_width,
            height: chart_height,
        },
        theme,
        None,
    )
    .into_any_element();

    let selected_gr = gain_reduction_db
        .and_then(|values| values.get(selected_band_idx))
        .copied()
        .filter(|value| value.is_finite())
        .map(f64::from)
        .unwrap_or(0.0);

    div()
        .flex()
        .flex_col()
        .gap(d.gap)
        .p(d.card)
        .rounded(d.r_lg)
        .bg(theme.surface)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_size(d.text_sm)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_primary)
                .child(text.label("Response"))
                .child(
                    div()
                        .text_size(d.text_xs)
                        .font_weight(FontWeight::NORMAL)
                        .text_color(theme.text_muted)
                        .child(format!(
                            "{} {} · {}",
                            text.label("Band"),
                            selected_band_idx + 1,
                            text.label("live GR")
                        )),
                ),
        )
        .child(div().w_full().overflow_hidden().child(chart))
        .child(render_gr_meter(d, selected_gr, -30.0, theme))
        .into_any_element()
}

/// Magnitude response of a peaking biquad at `frequency` in Hz.
fn peaking_eq_response_db(
    frequency: f64,
    center_frequency: f64,
    q: f64,
    gain_db: f64,
    sample_rate: f64,
) -> f64 {
    if !frequency.is_finite()
        || !center_frequency.is_finite()
        || !q.is_finite()
        || !gain_db.is_finite()
        || !sample_rate.is_finite()
        || frequency <= 0.0
        || center_frequency <= 0.0
        || q <= 0.0
        || sample_rate <= 0.0
    {
        return 0.0;
    }

    let center_omega = 2.0 * std::f64::consts::PI * center_frequency / sample_rate;
    let alpha = center_omega.sin() / (2.0 * q);
    let amplitude = 10.0_f64.powf(gain_db / 40.0);
    let center_cos = center_omega.cos();

    let b0 = 1.0 + alpha * amplitude;
    let b1 = -2.0 * center_cos;
    let b2 = 1.0 - alpha * amplitude;
    let a0 = 1.0 + alpha / amplitude;
    let a1 = -2.0 * center_cos;
    let a2 = 1.0 - alpha / amplitude;

    let omega = 2.0 * std::f64::consts::PI * frequency / sample_rate;
    let cos_omega = omega.cos();
    let sin_omega = omega.sin();
    let cos_2omega = (2.0 * omega).cos();
    let sin_2omega = (2.0 * omega).sin();
    let numerator =
        (b0 + b1 * cos_omega + b2 * cos_2omega).hypot(-b1 * sin_omega - b2 * sin_2omega);
    let denominator =
        (a0 + a1 * cos_omega + a2 * cos_2omega).hypot(-a1 * sin_omega - a2 * sin_2omega);

    if denominator > f64::EPSILON && numerator.is_finite() && denominator.is_finite() {
        (20.0 * (numerator / denominator).log10()).clamp(-60.0, 60.0)
    } else {
        0.0
    }
}

#[allow(clippy::too_many_arguments)]
fn render_global_controls(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    num_bands: f64,
    threshold: f64,
    ratio: f64,
    attack: f64,
    release: f64,
    knee: f64,
    link_channels: bool,
    mix: f64,
    selected_param: usize,
    is_editing: bool,
    compact: bool,
    text: PluginCommonTranslations,
    theme: &Theme,
) -> impl IntoElement {
    let params = specs::PARAMS;
    let knob_size = if compact {
        PotentiometerSize::Xs
    } else {
        PotentiometerSize::Sm
    };
    div()
        .flex()
        .flex_col()
        .gap(d.gap)
        .p(d.card)
        .rounded(d.r_lg)
        .bg(theme.surface)
        .child(
            div()
                .text_size(d.text_sm)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_primary)
                .child(text.global),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(d.section)
                .items_center()
                .child(render_knob_sized(
                    entity.clone(),
                    plugin_idx,
                    text.label("Bands"),
                    num_bands,
                    pk(params, "num_bands").min_f64(),
                    pk(params, "num_bands").max_f64(),
                    "",
                    0,
                    selected_param,
                    is_editing,
                    None,
                    knob_size,
                    theme,
                ))
                .child(render_knob_sized(
                    entity.clone(),
                    plugin_idx,
                    text.label("Thresh"),
                    threshold,
                    pk(params, "threshold").min_f64(),
                    pk(params, "threshold").max_f64(),
                    "dB",
                    1,
                    selected_param,
                    is_editing,
                    None,
                    knob_size,
                    theme,
                ))
                .child(render_knob_sized(
                    entity.clone(),
                    plugin_idx,
                    text.label("Ratio"),
                    ratio,
                    pk(params, "ratio").min_f64(),
                    pk(params, "ratio").max_f64(),
                    ":1",
                    2,
                    selected_param,
                    is_editing,
                    None,
                    knob_size,
                    theme,
                ))
                .child(render_knob_sized(
                    entity.clone(),
                    plugin_idx,
                    text.label("Attack"),
                    attack,
                    pk(params, "attack").min_f64(),
                    pk(params, "attack").max_f64(),
                    "ms",
                    3,
                    selected_param,
                    is_editing,
                    None,
                    knob_size,
                    theme,
                ))
                .child(render_knob_sized(
                    entity.clone(),
                    plugin_idx,
                    text.label("Release"),
                    release,
                    pk(params, "release").min_f64(),
                    pk(params, "release").max_f64(),
                    "ms",
                    4,
                    selected_param,
                    is_editing,
                    None,
                    knob_size,
                    theme,
                ))
                .child(render_knob_sized(
                    entity.clone(),
                    plugin_idx,
                    text.label("Knee"),
                    knee,
                    pk(params, "knee").min_f64(),
                    pk(params, "knee").max_f64(),
                    "dB",
                    5,
                    selected_param,
                    is_editing,
                    None,
                    knob_size,
                    theme,
                ))
                .child(render_toggle(
                    entity.clone(),
                    plugin_idx,
                    text.label("Link"),
                    link_channels,
                    6,
                    selected_param,
                    is_editing,
                    theme,
                ))
                .child(render_knob_sized(
                    entity,
                    plugin_idx,
                    text.label("Mix"),
                    mix,
                    pk(params, "mix").min_f64(),
                    pk(params, "mix").max_f64(),
                    "%",
                    7,
                    selected_param,
                    is_editing,
                    None,
                    knob_size,
                    theme,
                )),
        )
}

fn render_band_tabs(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    bands: &[sotf_plugins::DynEqBandParams],
    num_bands: usize,
    selected_band_idx: usize,
    gr: Option<&Vec<f32>>,
    theme: &Theme,
) -> impl IntoElement {
    let mut row = div()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_center()
        .w_full()
        .min_w_0()
        .gap(d.gap)
        .p(d.grid)
        .rounded(d.r_lg)
        .bg(theme.surface);

    for band_idx in 0..num_bands {
        let band = bands.get(band_idx).cloned().unwrap_or_default();
        let is_selected = band_idx == selected_band_idx;
        let gain_reduction = gr.and_then(|g| g.get(band_idx)).copied().unwrap_or(0.0);
        let entity = entity.clone();
        let key_entity = entity.clone();
        let focus_color = theme.border_focused;
        row = row.child(
            div()
                .id(("dyneq-band", band_idx))
                .flex()
                .flex_col()
                .items_center()
                .gap(d.grid)
                .px(d.pad_x)
                .py(d.pad_y)
                .rounded(d.r_md)
                .cursor_pointer()
                .bg(if is_selected {
                    theme.accent
                } else {
                    theme.background_secondary
                })
                .text_color(if is_selected {
                    theme.text_on_accent
                } else {
                    theme.text_secondary
                })
                .opacity(if band.active { 1.0 } else { 0.5 })
                .focusable()
                .focus_visible(move |s| s.border_1().border_color(focus_color))
                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                    entity.update(cx, |state, _| {
                        state.app.plugin_state.selected_eq_band = band_idx;
                        state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                    });
                })
                .on_key_down(move |event, _window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        key_entity.update(cx, |state, _| {
                            state.app.plugin_state.selected_eq_band = band_idx;
                            state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                        });
                        cx.stop_propagation();
                    }
                })
                .child(
                    div()
                        .text_size(d.text_sm)
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("B{}", band_idx + 1)),
                )
                .child(
                    div()
                        .text_size(d.text_xs)
                        .child(format!("{:.0} Hz", band.frequency)),
                )
                .child(
                    div()
                        .text_size(d.text_xs)
                        .child(format!("{gain_reduction:.1} dB GR")),
                )
                .child(
                    div()
                        .w(rems(4.0))
                        .h(rems(0.25))
                        .rounded_full()
                        .overflow_hidden()
                        .bg(theme.background)
                        .child(
                            div()
                                .h_full()
                                .w(relative((gain_reduction.abs() / 24.0).clamp(0.0, 1.0)))
                                .bg(if is_selected {
                                    theme.text_on_accent
                                } else {
                                    theme.accent
                                }),
                        ),
                ),
        );
    }

    row
}

fn render_selected_band(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    bands: &[sotf_plugins::DynEqBandParams],
    selected_band_idx: usize,
    selected_param: usize,
    is_editing: bool,
    compact: bool,
    text: PluginCommonTranslations,
    theme: &Theme,
) -> impl IntoElement {
    let band = bands.get(selected_band_idx).cloned().unwrap_or_default();
    let bt = specs::BAND_PARAMS;
    let base = BAND_PARAM_BASE + selected_band_idx * BAND_PARAM_STRIDE;
    let knob_size = if compact {
        PotentiometerSize::Xs
    } else {
        PotentiometerSize::Sm
    };

    div()
        .flex()
        .flex_col()
        .gap(d.gap)
        .p(d.card)
        .rounded(d.r_lg)
        .bg(theme.background_secondary)
        .child(
            div()
                .text_size(d.text_sm)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_primary)
                .child(format!("{} {}", text.label("Band"), selected_band_idx + 1)),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(d.section)
                .items_center()
                .child(render_knob_sized(
                    entity.clone(),
                    plugin_idx,
                    text.label("Freq"),
                    band.frequency as f64,
                    pk(bt, "frequency").min_f64(),
                    pk(bt, "frequency").max_f64(),
                    "Hz",
                    base,
                    selected_param,
                    is_editing,
                    None,
                    knob_size,
                    theme,
                ))
                .child(render_knob_sized(
                    entity.clone(),
                    plugin_idx,
                    text.label("Q"),
                    band.q as f64,
                    pk(bt, "q").min_f64(),
                    pk(bt, "q").max_f64(),
                    "",
                    base + 1,
                    selected_param,
                    is_editing,
                    None,
                    knob_size,
                    theme,
                ))
                .child(render_knob_sized(
                    entity.clone(),
                    plugin_idx,
                    text.label("Gain"),
                    band.gain as f64,
                    pk(bt, "gain").min_f64(),
                    pk(bt, "gain").max_f64(),
                    "dB",
                    base + 2,
                    selected_param,
                    is_editing,
                    None,
                    knob_size,
                    theme,
                ))
                .child(render_knob_sized(
                    entity.clone(),
                    plugin_idx,
                    text.label("Thresh"),
                    band.band_threshold as f64,
                    pk(bt, "band_threshold").min_f64(),
                    pk(bt, "band_threshold").max_f64(),
                    "dB",
                    base + 3,
                    selected_param,
                    is_editing,
                    None,
                    knob_size,
                    theme,
                ))
                .child(render_knob_sized(
                    entity.clone(),
                    plugin_idx,
                    text.label("Ratio"),
                    band.band_ratio as f64,
                    pk(bt, "band_ratio").min_f64(),
                    pk(bt, "band_ratio").max_f64(),
                    ":1",
                    base + 4,
                    selected_param,
                    is_editing,
                    None,
                    knob_size,
                    theme,
                ))
                .child(render_toggle(
                    entity.clone(),
                    plugin_idx,
                    text.label("Active"),
                    band.active,
                    base + 5,
                    selected_param,
                    is_editing,
                    theme,
                ))
                .child(render_toggle(
                    entity,
                    plugin_idx,
                    text.label("Solo"),
                    band.solo,
                    base + 6,
                    selected_param,
                    is_editing,
                    theme,
                )),
        )
}
use crate::app::i18n::PluginCommonTranslations;
