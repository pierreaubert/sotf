//! Dynamic EQ plugin UI.

use super::common::{render_knob_sized, render_toggle};
use crate::app::AppState;
use crate::components::design::Ds;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_audio_kit::PotentiometerSize;
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
    _available_width: f32,
    theme: &Theme,
    cx: &mut Context<PlayerView>,
) -> impl IntoElement {
    let d = Ds::from_cx(cx);
    let text = PluginCommonTranslations::for_language(entity.read(cx).app.ui_state.language);
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
            text,
            theme,
        ))
        .into_any_element()
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
    text: PluginCommonTranslations,
    theme: &Theme,
) -> impl IntoElement {
    let params = specs::PARAMS;
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
                    PotentiometerSize::Xs,
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
                    PotentiometerSize::Xs,
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
                    PotentiometerSize::Xs,
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
                    PotentiometerSize::Xs,
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
                    PotentiometerSize::Xs,
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
                    PotentiometerSize::Xs,
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
                    PotentiometerSize::Xs,
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
        .gap(d.gap)
        .p(d.grid)
        .rounded(d.r_lg)
        .bg(theme.surface);

    for band_idx in 0..num_bands {
        let band = bands.get(band_idx).cloned().unwrap_or_default();
        let is_selected = band_idx == selected_band_idx;
        let gain_reduction = gr.and_then(|g| g.get(band_idx)).copied().unwrap_or(0.0);
        let entity = entity.clone();
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
                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                    entity.update(cx, |state, _| {
                        state.app.plugin_state.selected_eq_band = band_idx;
                        state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                    });
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
    text: PluginCommonTranslations,
    theme: &Theme,
) -> impl IntoElement {
    let band = bands.get(selected_band_idx).cloned().unwrap_or_default();
    let bt = specs::BAND_PARAMS;
    let base = BAND_PARAM_BASE + selected_band_idx * BAND_PARAM_STRIDE;

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
                .child(format!("Band {}", selected_band_idx + 1)),
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
                    PotentiometerSize::Sm,
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
                    PotentiometerSize::Sm,
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
                    PotentiometerSize::Sm,
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
                    PotentiometerSize::Sm,
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
                    PotentiometerSize::Sm,
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
