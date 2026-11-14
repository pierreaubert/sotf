//! Compact controls for live hosted parameters with stable graph ownership.

use super::external_numeric::NumericControl;
use crate::app::{AppState, ToastMessage};
use crate::components::design::Ds;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::{AnyElement, App, Empty, Entity, SharedString, div, rems};
use gpui_ui_kit::{
    ButtonSet, ButtonSetOption, ButtonSetSize, NumberInput, NumberInputSize, Select, SelectOption,
    SelectSize, Slider, SliderSize, Text, WithTooltip,
};
use sotf_plugins::{ExternalPluginEditorData, Parameter, ParameterId, ParameterValue};

pub(super) fn external_engine_index(
    graph: &sotf_audio_player::PluginGraph,
    instance_id: usize,
) -> Option<usize> {
    graph.nodes.values().find_map(|node| {
        (node.plugin.id == instance_id
            && matches!(
                node.plugin.settings,
                sotf_audio_player::PluginSettings::External { .. }
            ))
        .then(|| graph.get_engine_index(node.id))
        .flatten()
    })
}

#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;

#[expect(
    clippy::needless_pass_by_value,
    reason = "Parameter submission accepts an owned parameter identifier from the control callback"
)]
fn submit(
    entity: &Entity<AppState>,
    instance_id: usize,
    parameter_id: ParameterId,
    value: ParameterValue,
    cx: &mut App,
) {
    entity.update(cx, |state, cx| {
        let graph = &state.app.plugin_state.graph;
        let engine_index = external_engine_index(graph, instance_id);
        if state.app.plugin_state.graph_state.draft.is_some()
            || state.app.plugin_state.update_state.pending_ack.is_some()
            || state
                .app
                .plugin_state
                .update_state
                .pending_plugin_update
                .is_some()
            || !state
                .app
                .plugin_state
                .update_state
                .pending_actions
                .is_empty()
            || engine_index.is_none()
        {
            state.app.ui_state.toast_message = Some(ToastMessage::error(
                "Apply the routing graph before editing a hosted parameter.",
            ));
            cx.notify();
            return;
        }
        let Some(engine_index) = engine_index else {
            return;
        };
        let encoded = match value {
            // Debug retains a decimal point for integral floats. The engine's
            // typed parser must not reinterpret 1.0 as an integer.
            ParameterValue::Float(value) => format!("{value:?}"),
            ParameterValue::Int(value) => value.to_string(),
            ParameterValue::Bool(value) => value.to_string(),
            ParameterValue::String(value) => value,
        };
        state
            .app
            .plugin_state
            .update_state
            .profile_observation_revision = state
            .app
            .plugin_state
            .update_state
            .profile_observation_revision
            .wrapping_add(1);
        if let Err(error) =
            state
                .player
                .set_plugin_action(engine_index, parameter_id.as_str().to_owned(), encoded)
        {
            state.app.ui_state.toast_message = Some(ToastMessage::error(error.to_string()));
        }
        cx.notify();
    });
}

/// Render compact controls from authoritative native parameter readback.
///
/// Missing or invalid values remain visibly unavailable. Changes are submitted
/// through the owning player actor; this component never edits saved state.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
pub fn render_parameters(
    entity: Entity<AppState>,
    data: &ExternalPluginEditorData,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    let text = crate::app::i18n::PluginCommonTranslations::for_language(
        entity.read(cx).app.ui_state.language,
    );
    let d = Ds::from_cx(cx);
    let Some(instance_id) = data.plugin_instance_id else {
        return Empty.into_any_element();
    };
    let state = &entity.read(cx).app.plugin_state;
    let interactive = data.native_state.is_ok()
        && external_engine_index(&state.graph, instance_id).is_some()
        && state.graph_state.draft.is_none()
        && state.update_state.pending_ack.is_none()
        && state.update_state.pending_plugin_update.is_none()
        && state.update_state.pending_actions.is_empty();
    let mut panel = div().flex().flex_col().gap(d.grid).w_full().min_w_0();
    if let Err(error) = &data.native_state {
        panel = panel.child(
            Text::caption(format!("Native state capture failed: {error}")).color(theme.error),
        );
    }
    let mut last_group = None;
    for (index, parameter) in data.parameters.iter().enumerate() {
        if !parameter.group.is_empty() && last_group != Some(&parameter.group) {
            panel = panel.child(Text::label(parameter.group.clone()));
            last_group = Some(&parameter.group);
        }
        panel = panel.child(render_parameter(
            &d,
            entity.clone(),
            instance_id,
            index,
            parameter,
            data.parameter_values.get(&parameter.id),
            interactive,
            state.external_plugin_ui.choice_menu == Some((instance_id, index)),
            state.external_plugin_ui.choice_highlight,
            theme,
            text,
        ));
    }
    panel.into_any_element()
}

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
    clippy::cast_possible_truncation,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
fn render_parameter(
    d: &Ds,
    entity: Entity<AppState>,
    instance_id: usize,
    index: usize,
    parameter: &Parameter,
    value: Option<&ParameterValue>,
    interactive: bool,
    choice_open: bool,
    choice_highlight: Option<usize>,
    theme: &Theme,
    text: crate::app::i18n::PluginCommonTranslations,
) -> AnyElement {
    let mut row = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(d.grid)
        .w_full()
        .min_w_0()
        .child(
            WithTooltip::new(
                div()
                    .w(rems(10.0))
                    .min_w_0()
                    .child(Text::label(parameter.name.clone()).truncate(true)),
                parameter.description.as_ref().map_or_else(
                    || parameter.name.clone(),
                    |description| format!("{}: {description}", parameter.name),
                ),
            )
            .id(("external-label", instance_id * 1000 + index)),
        );
    let Some(value) = value else {
        return row
            .child(Text::caption(text.label("Value unavailable")))
            .into_any_element();
    };
    if parameter.read_only {
        let display = parameter
            .choices
            .iter()
            .find(|choice| &choice.value == value)
            .map_or_else(
                || format!("{value} {}", parameter.unit).trim().to_owned(),
                |choice| choice.label.clone(),
            );
        let text = format!("{display} · Read only");
        let caption = WithTooltip::new(
            div()
                .w(rems(12.0))
                .min_w_0()
                .child(Text::caption(text.clone()).truncate(true)),
            text.clone(),
        )
        .id(("external-readonly", instance_id * 1000 + index));
        #[cfg(feature = "dev-api")]
        let caption = caption.dev_track_with_state(
            format!("external.param.{instance_id}.{index}.readonly"),
            crate::app::dev_api::DevElementState::default()
                .enabled(false)
                .text(text),
        );
        return row.child(caption).into_any_element();
    }
    if !parameter.choices.is_empty() {
        let Some(selected) = parameter
            .choices
            .iter()
            .position(|choice| &choice.value == value)
        else {
            return row
                .child(
                    Text::caption(format!("{value} · Native choice unavailable"))
                        .color(theme.warning),
                )
                .into_any_element();
        };
        let choices = parameter.choices.clone();
        let id = parameter.id.clone();
        if choices.len() <= 3
            && choices
                .iter()
                .map(|choice| choice.label.len())
                .sum::<usize>()
                <= 24
        {
            let buttons = ButtonSet::new(("external-native-choices", instance_id * 1000 + index))
                .options(
                    choices
                        .iter()
                        .enumerate()
                        .map(|(i, choice)| {
                            ButtonSetOption::new(i.to_string(), choice.label.clone())
                        })
                        .collect(),
                )
                .selected(selected.to_string())
                .size(ButtonSetSize::Xs)
                .theme(theme.to_button_set_theme())
                .disabled(!interactive)
                .on_change(move |key, _, cx| {
                    if let Ok(i) = key.parse::<usize>()
                        && let Some(choice) = choices.get(i)
                    {
                        submit(&entity, instance_id, id.clone(), choice.value.clone(), cx);
                    }
                });
            #[cfg(feature = "dev-api")]
            let buttons = buttons.option_wrapper(move |i, option| {
                option
                    .dev_track_with_state(
                        format!("external.param.{instance_id}.{index}.choice.{i}"),
                        crate::app::dev_api::DevElementState::default()
                            .enabled(interactive)
                            .selected(i == selected),
                    )
                    .into_any_element()
            });
            return row.child(buttons).into_any_element();
        }
        // Select retains its blur callback; it must not retain the application model.
        let toggle = entity.downgrade();
        let highlight = entity.downgrade();
        let selection = entity.downgrade();
        let select = Select::new(("external-native-select", instance_id * 1000 + index))
            .options(
                choices
                    .iter()
                    .enumerate()
                    .map(|(i, choice)| SelectOption::new(i.to_string(), choice.label.clone()))
                    .collect(),
            )
            .selected(selected.to_string())
            .is_open(choice_open && interactive)
            .highlighted_index(choice_highlight)
            .size(SelectSize::Xs)
            .theme(theme.to_select_theme())
            .disabled(!interactive)
            .on_toggle(move |open, _, cx| {
                let _ = toggle.update(cx, |state, cx| {
                    let ui = &mut state.app.plugin_state.external_plugin_ui;
                    ui.choice_menu = open.then_some((instance_id, index));
                    ui.choice_highlight = open.then_some(selected);
                    cx.notify();
                });
            })
            .on_highlight(move |value, _, cx| {
                let _ = highlight.update(cx, |state, cx| {
                    state.app.plugin_state.external_plugin_ui.choice_highlight = value;
                    cx.notify();
                });
            })
            .on_change(move |key, _, cx| {
                if let Ok(i) = key.parse::<usize>()
                    && let Some(choice) = choices.get(i)
                    && let Some(selection) = selection.upgrade()
                {
                    submit(
                        &selection,
                        instance_id,
                        id.clone(),
                        choice.value.clone(),
                        cx,
                    );
                }
                let _ = selection.update(cx, |state, cx| {
                    state.app.plugin_state.external_plugin_ui.choice_menu = None;
                    state.app.plugin_state.external_plugin_ui.choice_highlight = None;
                    cx.notify();
                });
            });
        #[cfg(feature = "dev-api")]
        let select = select.dev_track_with_state(
            format!("external.param.{instance_id}.{index}.select"),
            crate::app::dev_api::DevElementState::default().enabled(interactive),
        );
        return row
            .child(div().w(rems(12.0)).min_w_0().child(select))
            .into_any_element();
    }
    match value {
        ParameterValue::Bool(value) => {
            let id = parameter.id.clone();
            let buttons = ButtonSet::new(("external-bool", instance_id * 1000 + index))
                .options(vec![
                    ButtonSetOption::new("0", text.label("Off")),
                    ButtonSetOption::new("1", text.label("On")),
                ])
                .selected(if *value { "1" } else { "0" })
                .size(ButtonSetSize::Xs)
                .theme(theme.to_button_set_theme())
                .disabled(!interactive)
                .on_change(move |value, _, cx| {
                    submit(
                        &entity,
                        instance_id,
                        id.clone(),
                        ParameterValue::Bool(value.as_ref() == "1"),
                        cx,
                    );
                });
            #[cfg(feature = "dev-api")]
            let buttons = buttons
                .option_wrapper(move |option_index, option| {
                    use crate::app::dev_api::DevElementState;
                    option
                        .dev_track_with_state(
                            format!("external.param.{instance_id}.{index}.choice.{option_index}"),
                            DevElementState::default().enabled(interactive),
                        )
                        .into_any_element()
                })
                .dev_track(format!("external.param.{instance_id}.{index}.choice"));
            row = row.child(buttons);
        }
        ParameterValue::Float(_) | ParameterValue::Int(_) => {
            let control = match NumericControl::from_native(
                value,
                parameter.min_value.as_ref(),
                parameter.max_value.as_ref(),
            ) {
                Ok(control) => control,
                Err(error) => {
                    return row
                        .child(Text::caption(format!("{value:?} · {error}")).color(theme.warning))
                        .into_any_element();
                }
            };
            let interactive = interactive && control.min < control.max;
            let slider_entity = entity.clone();
            let reset_entity = entity.clone();
            let slider_id = parameter.id.clone();
            let reset_id = parameter.id.clone();
            let default = parameter.default_value.clone();
            if let Some((min, max)) = control.slider_range {
                let slider = Slider::new(("external-slider", instance_id * 1000 + index))
                    .value(control.current as f32)
                    .range(min, max)
                    .step(parameter.step.filter(|step| step.is_finite() && *step > 0.0).map_or(if control.integer { 1.0 } else { 0.0 }, |step| step as f32))
                    .logarithmic(parameter.logarithmic && min > 0.0)
                    .width(142.0)
                    .size(SliderSize::Sm)
                    .theme(theme.to_slider_theme())
                    .disabled(!interactive)
                    .on_change(move |value, _, cx| {
                        if let Some(value) = control.encode(f64::from(value)) {
                            submit(&slider_entity, instance_id, slider_id.clone(), value, cx);
                        }
                    })
                    .on_reset(move |_, cx| {
                        let valid = matches!(
                            NumericControl::from_native(&default, Some(&ParameterValue::Float(min)), Some(&ParameterValue::Float(max))),
                            Ok(reset) if reset.integer == control.integer
                        );
                        if valid {
                            submit(&reset_entity, instance_id, reset_id.clone(), default.clone(), cx);
                        }
                    });
                #[cfg(feature = "dev-api")]
                let slider = slider.dev_track_with_state(
                    format!("external.param.{instance_id}.{index}.slider"),
                    crate::app::dev_api::DevElementState::default().enabled(interactive),
                );
                row = row.child(slider);
            }
            let id = parameter.id.clone();
            let exact = NumberInput::new(SharedString::from(format!(
                "external-exact-{instance_id}-{index}"
            )))
            .value(control.current)
            .min(control.min)
            .max(control.max)
            .step(
                parameter
                    .step
                    .filter(|step| step.is_finite() && *step > 0.0)
                    .unwrap_or(if control.integer { 1.0 } else { 0.001 }),
            )
            .decimals(if control.integer { 0 } else { 6 })
            .unit(parameter.unit.clone())
            .size(NumberInputSize::Xs)
            .theme(theme.to_number_input_theme())
            .disabled(!interactive)
            .aria_label(format!("{} value", parameter.name))
            .on_change(move |value, _, cx| {
                if let Some(value) = control.encode(value) {
                    submit(&entity, instance_id, id.clone(), value, cx);
                }
            });
            #[cfg(feature = "dev-api")]
            let exact = exact.dev_track_with_state(
                format!("external.param.{instance_id}.{index}.exact"),
                crate::app::dev_api::DevElementState::default().enabled(interactive),
            );
            row = row.child(div().w(rems(10.0)).flex_shrink_0().child(exact));
        }
        ParameterValue::String(value) => {
            row = row.child(Text::caption(value.clone()));
        }
    }
    row.into_any_element()
}
