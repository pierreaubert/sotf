//! Dynamic EQ plugin UI.

use super::ui_layout_renderer::{
    render_compact_choice_param, render_compact_selector_param, render_horizontal_param,
};
use crate::app::AppState;
#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;
use crate::components::design::Ds;
use crate::components::graphs::common::rgba_to_u32;
use crate::components::graphs::response_graphs::{ChartConfig, Series, render_line_chart};
use crate::components::plugins::editing::PluginEditingManager;
use crate::components::plugins::render_gr_meter;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::{AnyElement, App, Entity, FontWeight, SharedString, div};
use gpui_px::ScaleType;
use gpui_ui_kit::{
    Accordion, AccordionItem, AccordionMode, Button, ButtonSize, ButtonVariant, NumberInput,
    NumberInputSize, Text,
};
use sotf_audio_player::PluginSettings;
use sotf_plugins::DynamicEqData;
use sotf_plugins::param_specs::dynamic_eq as specs;
use std::any::Any;
use std::sync::Arc;

const BAND_PARAM_BASE: usize = 100;
const BAND_PARAM_STRIDE: usize = 10;

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Bounded UI controls deliberately round parameter values and navigation indices"
)]
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
        num_bands, bands, ..
    } = settings
    else {
        return div().into_any_element();
    };

    let num_bands = (*num_bands as usize).clamp(1, 8);
    let selected_band_idx = selected_band_idx.min(num_bands.saturating_sub(1));
    let wide = available_width / layout_scale.max(0.01) >= 800.0;
    let gr = plugin_data
        .and_then(|data| data.downcast::<DynamicEqData>().ok())
        .map(|data| std::sync::Arc::clone(&data.gain_reduction_db));

    div()
        .flex()
        .flex_col()
        .gap(d.grid)
        .w_full()
        .min_w_0()
        .child(render_band_tabs(
            &d,
            entity.clone(),
            plugin_idx,
            bands,
            num_bands,
            selected_band_idx,
            gr.as_deref(),
            text,
            theme,
        ))
        .child(render_selected_band(
            &d,
            entity.clone(),
            plugin_idx,
            bands,
            selected_band_idx,
            selected_param,
            is_editing,
            wide,
            settings.param_value(7).unwrap_or(1.0),
            text,
            theme,
            cx,
        ))
        .child(render_dynamic_eq_response(
            &d,
            bands,
            num_bands,
            selected_band_idx,
            gr.as_deref(),
            f64::from(sample_rate),
            available_width,
            text,
            theme,
        ))
        .child(render_global_controls(
            &d,
            entity,
            plugin_idx,
            settings,
            selected_param,
            is_editing,
            text,
            theme,
            cx,
        ))
        .into_any_element()
}

/// Render the static response of the active bands and a live GR meter for the
/// selected band. The response is intentionally derived from the same
/// coefficient designers used by the DSP, including shelves and tilt. Routing
/// is shown on the selected band; a combined target is only meaningful when
/// every active band uses Stereo routing.
#[allow(
    clippy::too_many_arguments,
    reason = "GPUI rendering and callback interfaces accept the complete UI context"
)]
#[expect(
    clippy::too_many_lines,
    reason = "Keep the declarative layout and its interaction handlers together"
)]
#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
#[expect(
    clippy::cast_precision_loss,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
#[expect(
    clippy::cast_sign_loss,
    reason = "Bounded UI controls deliberately round parameter values and navigation indices"
)]
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

    let stereo_combination = active_bands
        .iter()
        .all(|band| band.placement == sotf_plugins::plugin_dynamic_eq::DynEqPlacement::Stereo);
    let combined: Vec<f64> = frequencies
        .iter()
        .map(|frequency| {
            active_bands
                .iter()
                .map(|band| {
                    sotf_plugins::plugin_dynamic_eq::target_band_response_db(
                        band,
                        *frequency,
                        sample_rate,
                    )
                    .unwrap_or(0.0)
                })
                .sum()
        })
        .collect();

    let selected_gain = bands
        .get(selected_band_idx)
        .map_or(0.0, |band| f64::from(band.gain).abs());
    let y_limit = (combined
        .iter()
        .map(|value| value.abs())
        .fold(selected_gain, f64::max)
        + 3.0)
        .clamp(12.0, 36.0);
    let chart_width = available_width.max(280.0);
    let chart_height = (chart_width * 0.34).clamp(130.0, 210.0);

    let mut series = Vec::new();
    if stereo_combination {
        series.push(
            Series::new(
                "Combined target",
                rgba_to_u32(theme.accent),
                frequencies.clone(),
                combined,
            )
            .with_width(2.5),
        );
    }

    if let Some(selected_band) = bands.get(selected_band_idx) {
        let selected_response = frequencies
            .iter()
            .map(|frequency| {
                sotf_plugins::plugin_dynamic_eq::target_band_response_db(
                    selected_band,
                    *frequency,
                    sample_rate,
                )
                .unwrap_or(0.0)
            })
            .collect();
        series.push(
            Series::new(
                format!(
                    "{} {} · {}",
                    text.label("Band"),
                    selected_band_idx + 1,
                    specs::ROUTING_PARAMS[0]
                        .choice_labels()
                        .get(selected_band.placement.choice_index() as usize)
                        .copied()
                        .unwrap_or("Stereo")
                ),
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
            y_label: None,
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
        .map_or(0.0, f64::from);

    div()
        .flex()
        .flex_col()
        .gap(d.gap)
        .w_full()
        .min_w_0()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_size(d.text_sm)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_primary)
                .child(text.label("Target response"))
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

/// Global dynamics defaults stay below the selected-band editor.
#[allow(
    clippy::too_many_arguments,
    reason = "GPUI rendering and callback interfaces accept the complete UI context"
)]
#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
fn render_global_controls(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    selected_param: usize,
    is_editing: bool,
    text: PluginCommonTranslations,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    let app = &entity.read(cx).app;
    let instance_id = app
        .plugin_state
        .graph_state
        .editing_graph_node_uuid
        .and_then(|id| app.plugin_state.routing_controller().graph.nodes.get(&id))
        .map(|node| node.plugin.id)
        .or_else(|| {
            app.plugin_state
                .graph
                .get_plugin(plugin_idx)
                .map(|plugin| plugin.id)
        })
        .unwrap_or(plugin_idx);
    let section = "dynamic-eq-global";
    let expanded = app
        .plugin_state
        .plugin_ui_state
        .plugin_sections
        .get(&instance_id)
        .is_some_and(|sections| sections.iter().any(|id| id == section));
    let mut controls = div().flex().flex_col().gap(d.grid).w_full();
    for index in 1..=5 {
        let mut param = specs::PARAMS[index];
        param.name = text.label(param.name);
        controls = controls.child(render_horizontal_param(
            d,
            entity.clone(),
            plugin_idx,
            index,
            &param,
            settings.param_value(index).unwrap_or(param.default_f64()),
            is_editing,
            selected_param,
            true,
            theme,
        ));
    }
    let accordion = Accordion::new()
        .items(vec![
            AccordionItem::new(section, text.label("Advanced")).content(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.grid)
                    .child(Text::caption(text.global).color(theme.text_muted))
                    .child(controls),
            ),
        ])
        .mode(AccordionMode::Multiple)
        .expanded(if expanded {
            vec![SharedString::from(section)]
        } else {
            Vec::new()
        })
        .bordered(false)
        .rounded(false)
        .theme(theme.to_accordion_theme());
    #[cfg(feature = "dev-api")]
    let accordion = accordion
        .header_wrapper(move |_, header| header.dev_track("dyneq.advanced").into_any_element());
    accordion
        .on_change(move |id, open, _, cx| {
            entity.update(cx, |state, cx| {
                let sections = state
                    .app
                    .plugin_state
                    .plugin_ui_state
                    .plugin_sections
                    .entry(instance_id)
                    .or_default();
                sections.retain(|section| section != id.as_ref());
                if open {
                    sections.push(id.to_string());
                }
                cx.notify();
            });
        })
        .into_any_element()
}

/// Compact structural controls live between the title and Active.
#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
pub(crate) fn render_dynamic_eq_header_controls(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    text: PluginCommonTranslations,
    theme: &Theme,
) -> AnyElement {
    let param = specs::PARAMS[0];
    let count_entity = entity.clone();
    let count = NumberInput::new(("dyneq-band-count", plugin_idx))
        .value(settings.param_value(0).unwrap_or(param.default_f64()))
        .min(param.min_f64())
        .max(param.max_f64())
        .step(1.0)
        .decimals(0)
        .size(NumberInputSize::Xs)
        .theme(theme.to_number_input_theme())
        .aria_label(text.label("Bands"))
        .on_change(move |value, _, cx| {
            count_entity.update(cx, |state, cx| {
                state.app.set_plugin_param(plugin_idx, 0, value);
                cx.notify();
            });
        });
    #[cfg(feature = "dev-api")]
    let count = count.dev_track(format!("plugin.param.{plugin_idx}.0.exact"));
    let mut link = specs::PARAMS[6];
    link.name = text.label(link.name);
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(d.grid)
        .child(Text::label(text.label("Bands")))
        .child(count)
        .child(render_compact_choice_param(
            d,
            entity,
            plugin_idx,
            6,
            &link,
            settings.param_value(6).unwrap_or(1.0),
            theme,
        ))
        .into_any_element()
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
fn render_band_tabs(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    bands: &[sotf_plugins::DynEqBandParams],
    num_bands: usize,
    selected_band_idx: usize,
    _gr: Option<&Vec<f32>>,
    text: PluginCommonTranslations,
    theme: &Theme,
) -> impl IntoElement {
    let strip = div()
        .id(("dyneq-band-strip", plugin_idx))
        .flex()
        .items_center()
        .flex_1()
        .min_w_0()
        .overflow_x_scroll()
        // The strip handles wheel scrolling before this bubble listener.
        // Keep its wheel event from scrolling the surrounding rack as well.
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .gap(d.grid)
        .children((0..num_bands).map(|band_idx| {
            let band = bands.get(band_idx).cloned().unwrap_or_default();
            let entity = entity.clone();
            let button = Button::new(
                SharedString::from(format!("dyneq-band-{band_idx}")),
                format!("#{} {:.0} Hz", band_idx + 1, band.frequency),
            )
            .aria_label(format!("Dynamic EQ band {}", band_idx + 1))
            .size(ButtonSize::Xs)
            .variant(if selected_band_idx == band_idx {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Ghost
            })
            .theme(theme.to_button_theme())
            .on_click_event(move |_, _, cx| {
                entity.update(cx, |state, cx| {
                    state.app.plugin_state.selected_eq_band = band_idx;
                    state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                    cx.notify();
                });
            });
            #[cfg(feature = "dev-api")]
            let button = button.dev_track(format!("dyneq.band.{band_idx}.select"));
            div()
                .flex_none()
                .opacity(if band.active { 1.0 } else { 0.5 })
                .child(button)
        }));
    let band = bands.get(selected_band_idx).cloned().unwrap_or_default();
    let base = BAND_PARAM_BASE + selected_band_idx * BAND_PARAM_STRIDE;
    let mut controls = div().flex().flex_none().items_center().gap(d.grid);
    for (index, value) in [(5, band.active), (6, band.solo)] {
        let mut param = specs::BAND_PARAMS[index];
        param.name = text.label(if index == 5 { "Band" } else { "Solo" });
        controls = controls.child(render_compact_choice_param(
            d,
            entity.clone(),
            plugin_idx,
            base + index,
            &param,
            if value { 1.0 } else { 0.0 },
            theme,
        ));
    }
    div()
        .flex()
        .items_center()
        .gap(d.grid)
        .w_full()
        .min_w_0()
        .child(strip)
        .child(controls)
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
fn render_selected_band(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    bands: &[sotf_plugins::DynEqBandParams],
    selected_band_idx: usize,
    selected_param: usize,
    is_editing: bool,
    wide: bool,
    mix: f64,
    text: PluginCommonTranslations,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    let band = bands.get(selected_band_idx).cloned().unwrap_or_default();
    let base = BAND_PARAM_BASE + selected_band_idx * BAND_PARAM_STRIDE;
    let values = [
        f64::from(band.frequency),
        f64::from(band.q),
        f64::from(band.gain),
        f64::from(band.band_threshold),
        f64::from(band.band_ratio),
    ];
    let mut choices = div().flex().flex_wrap().items_center().gap(d.grid);
    for (index, spec, value) in [
        (
            7,
            specs::SHELF_PARAMS[0],
            f64::from(band.shape.choice_index()),
        ),
        (
            9,
            specs::ROUTING_PARAMS[0],
            f64::from(band.placement.choice_index()),
        ),
    ] {
        let mut spec = spec;
        spec.name = text.label(spec.name);
        choices = choices.child(render_compact_selector_param(
            d,
            entity.clone(),
            plugin_idx,
            base + index,
            &spec,
            value,
            theme,
            cx,
        ));
    }
    let render_rows = |indices: &[usize]| {
        let mut rows = div().flex().flex_col().gap(d.grid).flex_1().min_w_0();
        for &index in indices {
            let (index, mut param, value) =
                if index == 1 && band.shape != sotf_plugins::plugin_dynamic_eq::DynEqShape::Peak {
                    if band.shape == sotf_plugins::plugin_dynamic_eq::DynEqShape::Tilt {
                        continue;
                    }
                    (8, specs::SHELF_PARAMS[1], f64::from(band.shelf_slope))
                } else {
                    (index, specs::BAND_PARAMS[index], values[index])
                };
            if index == 0 {
                param.name = match band.shape {
                    sotf_plugins::plugin_dynamic_eq::DynEqShape::Tilt => "Pivot",
                    sotf_plugins::plugin_dynamic_eq::DynEqShape::Peak => "Frequency",
                    _ => "Cutoff",
                };
            }
            param.name = text.label(param.name);
            rows = rows.child(render_horizontal_param(
                d,
                entity.clone(),
                plugin_idx,
                base + index,
                &param,
                value,
                is_editing,
                selected_param,
                true,
                theme,
            ));
        }
        rows
    };
    let mut mix_param = specs::PARAMS[7];
    mix_param.name = text.label(mix_param.name);
    let rows = div()
        .flex()
        .gap(d.grid)
        .w_full()
        .min_w_0()
        .when(!wide, gpui::Styled::flex_col)
        .child(render_rows(&[0, 1, 2]))
        .child(render_rows(&[3, 4]).child(render_horizontal_param(
            d,
            entity.clone(),
            plugin_idx,
            7,
            &mix_param,
            mix,
            is_editing,
            selected_param,
            true,
            theme,
        )));
    div()
        .flex()
        .flex_col()
        .gap(d.grid)
        .w_full()
        .min_w_0()
        .child(choices)
        .child(rows)
        .into_any_element()
}
use crate::app::i18n::PluginCommonTranslations;
