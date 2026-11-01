//! Dense FIR EQ editor with stable legacy band addresses and sparse globals.

// Rust guideline compliant 2026-02-21
use super::render::{
    EqBandAction, EqBandIndexing, render_eq_band_action_button, render_eq_visualization_sized,
};
use super::types::{EqRenderState, EqViewMode};
use crate::app::AppState;
#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;
use crate::components::design::Ds;
use crate::components::plugins::editing::PluginEditingManager;
use crate::components::plugins::ui_layout_renderer::{
    render_compact_choice_param, render_compact_selector_param, render_horizontal_param,
};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSet, ButtonSetOption, ButtonSetSize, ButtonSize, ButtonVariant, NumberInput,
    NumberInputSize, Text,
};
use sotf_audio_player::{EQFilter, PluginSettings};
use sotf_plugins::param_specs::linear_phase_eq as specs;
use sotf_plugins::plugin_eq::EqBandPlacement;

fn placement_value(placement: Option<EqBandPlacement>) -> f64 {
    match placement {
        None => 0.0,
        Some(EqBandPlacement::Stereo) => 1.0,
        Some(EqBandPlacement::Left) => 2.0,
        Some(EqBandPlacement::Right) => 3.0,
        Some(EqBandPlacement::Mid) => 4.0,
        Some(EqBandPlacement::Side) => 5.0,
    }
}

pub(crate) fn render_header(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    let text =
        crate::app::i18n::EqViewTranslations::for_language(entity.read(cx).app.ui_state.language);
    let mut fir = specs::PARAMS[1];
    let mut phase = specs::PARAMS[2];
    phase.name = text.phase;
    let mut auto = specs::PARAMS[3];
    auto.name = text.auto_gain;
    let PluginSettings::LinearPhaseEq {
        num_filters,
        fir_length,
        phase_mode,
        auto_gain,
        accuracy_mode,
        ..
    } = settings
    else {
        return Empty.into_any_element();
    };
    fir.name = if *accuracy_mode {
        "Saved Manual FIR Length"
    } else {
        text.fir_length
    };
    let change_entity = entity.clone();
    let count = NumberInput::new(("fir-eq-count", plugin_idx))
        .value(*num_filters)
        .min(1.0)
        .max(specs::MAX_FILTERS as f64)
        .step(1.0)
        .decimals(0)
        .size(NumberInputSize::Xs)
        .aria_label(text.filters)
        .theme(theme.to_number_input_theme())
        .on_change(move |value, _, cx| {
            change_entity.update(cx, |state, cx| {
                state.app.set_plugin_param(plugin_idx, 100, value);
                cx.notify();
            });
        });
    #[cfg(feature = "dev-api")]
    let count = count.dev_track(format!("plugin.param.{plugin_idx}.100.exact"));
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(d.grid)
        .child(Text::label(text.filters))
        .child(count)
        .child(render_compact_selector_param(
            d,
            entity.clone(),
            plugin_idx,
            101,
            &fir,
            *fir_length,
            theme,
            cx,
        ))
        .child(render_compact_choice_param(
            d,
            entity.clone(),
            plugin_idx,
            102,
            &phase,
            *phase_mode,
            theme,
        ))
        .child(render_compact_choice_param(
            d,
            entity.clone(),
            plugin_idx,
            103,
            &auto,
            if *auto_gain { 1.0 } else { 0.0 },
            theme,
        ))
        .child(render_compact_choice_param(
            d,
            entity.clone(),
            plugin_idx,
            105,
            &specs::PARAMS[5],
            f64::from(*accuracy_mode),
            theme,
        ))
        .into_any_element()
}

pub(super) fn render(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &EqRenderState,
    filters: &[EQFilter],
    selected: usize,
    indexing: EqBandIndexing,
    theme: &Theme,
    focus: FocusHandle,
    cx: &App,
) -> AnyElement {
    let language = entity.read(cx).app.ui_state.language;
    let text = crate::app::i18n::EqViewTranslations::for_language(language);
    let mut strip = div()
        .id(("fir-eq-bands", plugin_idx))
        .flex()
        .items_center()
        .w_full()
        .min_w_0()
        .overflow_x_scroll()
        // The strip handles wheel scrolling before this bubble listener.
        // Keep its wheel event from scrolling the surrounding rack as well.
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .gap(d.grid);
    for (index, filter) in filters.iter().enumerate() {
        let change_entity = entity.clone();
        let button = Button::new(
            ("fir-eq-band", index),
            format!(
                "#{} {} {:.0} Hz",
                index + 1,
                filter.filter_type.short_name(),
                filter.frequency
            ),
        )
        .size(ButtonSize::Xs)
        .variant(if index == selected {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Ghost
        })
        .theme(theme.to_button_theme())
        .on_click(move |_, cx| {
            change_entity.update(cx, |state, cx| {
                state.app.plugin_state.selected_eq_band = index;
                state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                cx.notify();
            });
        });
        #[cfg(feature = "dev-api")]
        let button = button.dev_track(format!("fir.eq.band.{index}.select"));
        strip = strip.child(
            div()
                .flex_none()
                .opacity(if filter.muted { 0.5 } else { 1.0 })
                .child(button),
        );
    }
    strip = strip.child(render_eq_band_action_button(
        entity.clone(),
        plugin_idx,
        EqBandAction::Add,
        "+",
        filters.len() < specs::MAX_FILTERS,
        theme,
    ));
    let mut root = div()
        .flex()
        .flex_col()
        .w_full()
        .min_w_0()
        .gap(d.grid)
        .child(strip);
    if let Some(filter) = filters.get(selected) {
        let base = selected * 5;
        let solo = filter.solo;
        let solo_entity = entity.clone();
        let solo_buttons = ButtonSet::new(("fir-eq-solo", plugin_idx))
            .options(vec![
                ButtonSetOption::new("0", text.off),
                ButtonSetOption::new("1", text.on),
            ])
            .selected(if solo { "1" } else { "0" })
            .size(ButtonSetSize::Xs)
            .theme(theme.to_button_set_theme())
            .on_change(move |value, _, cx| {
                if (value.as_ref() == "1") != solo {
                    solo_entity.update(cx, |state, cx| {
                        state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                        if let Err(error) = state.app.toggle_eq_band_solo(selected) {
                            log::warn!("Unable to change FIR band solo: {error}");
                        }
                        cx.notify();
                    });
                }
            });
        #[cfg(feature = "dev-api")]
        let solo_buttons = solo_buttons.option_wrapper(move |option, button| {
            button
                .dev_track(format!("fir.eq.band.{selected}.solo.{option}"))
                .into_any_element()
        });
        let solo_control = div()
            .flex()
            .items_center()
            .gap(d.grid)
            .child(Text::label(text.solo))
            .child(solo_buttons);

        let kind = match filter.filter_type {
            math_audio_iir_fir::BiquadFilterType::Lowshelf => 1.0,
            math_audio_iir_fir::BiquadFilterType::Highshelf => 2.0,
            math_audio_iir_fir::BiquadFilterType::Lowpass => 3.0,
            math_audio_iir_fir::BiquadFilterType::Highpass => 4.0,
            _ => 0.0,
        };
        let mut active = specs::BAND_TEMPLATE[4];
        active.name = text.active;
        let mut kind_spec = specs::BAND_TEMPLATE[0];
        kind_spec.name = text.type_label;
        root = root.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(d.grid)
                .child(render_compact_selector_param(
                    d,
                    entity.clone(),
                    plugin_idx,
                    base,
                    &kind_spec,
                    kind,
                    theme,
                    cx,
                ))
                .child(render_compact_selector_param(
                    d,
                    entity.clone(),
                    plugin_idx,
                    200 + selected,
                    &specs::BAND_TEMPLATE[5],
                    placement_value(filter.placement),
                    theme,
                    cx,
                ))
                .child(render_compact_choice_param(
                    d,
                    entity.clone(),
                    plugin_idx,
                    base + 4,
                    &active,
                    if filter.muted { 0.0 } else { 1.0 },
                    theme,
                ))
                .child(solo_control)
                .child(render_eq_band_action_button(
                    entity.clone(),
                    plugin_idx,
                    EqBandAction::Remove(selected),
                    text.band_actions.remove,
                    filters.len() > 1,
                    theme,
                ))
                .child(render_eq_band_action_button(
                    entity.clone(),
                    plugin_idx,
                    EqBandAction::Reset {
                        band_idx: selected,
                        indexing,
                    },
                    text.band_actions.reset,
                    true,
                    theme,
                )),
        );
        let mut controls = div().flex().flex_wrap().items_center().gap(d.grid);
        for (local, value) in [(1, filter.frequency), (2, filter.q), (3, filter.gain_db)] {
            if local == 3 && kind >= 3.0 {
                continue;
            }
            let mut param = specs::BAND_TEMPLATE[local];
            param.name = match local {
                1 => text.frequency,
                2 => text.quality_factor,
                _ => text.gain,
            };
            let control = render_horizontal_param(
                d,
                entity.clone(),
                plugin_idx,
                base + local,
                &param,
                value,
                state.is_editing,
                state.selected_param,
                true,
                theme,
            );
            let assignment = state
                .midi_overlay
                .and_then(|overlay| overlay.assignments.get(&(base + local)));
            controls = controls.child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.grid)
                    .child(control)
                    .children(assignment.map(|assignment| {
                        super::super::common::render_midi_badge(d, assignment, theme)
                    })),
            );
        }
        root = root.child(controls);
    }
    if let EqViewMode::LinearPhase { mix, .. } = &state.mode {
        let mut mix_spec = specs::PARAMS[4];
        mix_spec.name = text.mix;
        root = root.child(render_horizontal_param(
            d,
            entity.clone(),
            plugin_idx,
            104,
            &mix_spec,
            *mix,
            state.is_editing,
            state.selected_param,
            true,
            theme,
        ));
    }
    let height = if state.available_width / state.layout_scale.max(0.01) >= 900.0 {
        260.0
    } else {
        180.0
    } * state.layout_scale;
    root = root
        .child(render_eq_visualization_sized(
            entity,
            plugin_idx,
            filters,
            Some(selected),
            indexing,
            theme,
            state.available_width.max(1.0),
            height,
            state.layout_scale,
            state.sample_rate,
            focus,
        ))
        .child(
            div()
                .w_full()
                .text_center()
                .text_size(d.text_sm)
                .text_color(theme.text_secondary)
                .child(crate::app::i18n::EqViewTranslations::frequency_axis(
                    language,
                )),
        )
        .child(
            Text::caption(crate::app::i18n::EqViewTranslations::wheel_hint(language))
                .color(theme.text_muted),
        );
    if let EqViewMode::LinearPhase {
        accuracy_mode,
        latency_samples,
        latency_ms,
        fir_length,
        phase_mode,
        ..
    } = &state.mode
    {
        let summary = if *accuracy_mode {
            format!(
                "{phase_mode} phase · Automatic FIR support · Resolved length and latency pending"
            )
        } else {
            format!(
                "{phase_mode} phase · {fir_length} taps · Latency {latency_samples} samples ({latency_ms:.2} ms)"
            )
        };
        root = root.child(Text::caption(summary).color(theme.text_secondary));
    }
    if let Some(overlay) = state
        .midi_overlay
        .filter(|overlay| overlay.has_controller())
    {
        root = root.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(d.grid)
                .children(
                    overlay
                        .controller_name
                        .as_ref()
                        .map(|name| Text::caption(name.clone()).color(theme.text_secondary)),
                )
                .child(super::super::common::render_midi_page_indicator(
                    d,
                    overlay.current_page,
                    overlay.total_pages,
                    theme,
                )),
        );
    }
    root.into_any_element()
}
