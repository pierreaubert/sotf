// intentional-file: fixed pixel values here are graph and plugin control geometry.
use super::super::editing::PluginEditingManager;
use super::consts::CELL_H;
use super::consts::CELL_W;
use super::consts::FADER_CELL_H;
use super::consts::MIN_CELL_W;
use super::consts::empty_cell;
use super::misc::cell_inner_theme;
use super::misc::continuous_range;
use super::misc::layout_for_controller_id;
use super::misc::mapping_for_view;
use super::static_::static_button;
use super::static_::static_fader;
use super::static_::static_slider;
use super::types::ContinuousKind;
use crate::app::AppState;
use crate::components::design::Ds;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::{AnyElement, App, Empty, Entity, FontWeight, Rgba, ScrollHandle, div, px, rems};
use gpui_audio_kit::{VerticalSlider, VerticalSliderSize};
use gpui_ui_kit::{
    ButtonSet, ButtonSetOption, ButtonSetSize, NumberInput, NumberInputSize, Slider, SliderSize,
};
use sotf_audio_player::PluginSettings;
use sotf_audio_player::controllers::plugin::{
    desktop_eq_param_value, desktop_param_enabled, desktop_param_spec, desktop_param_value,
};
use sotf_audio_player_midi::MidiMappingEngine;
use sotf_audio_player_midi::layout::{ControllerLayout, PhysicalControl, PhysicalControlKind};
use sotf_audio_player_midi::mapping::{ControlBinding, MidiMapping};
use sotf_plugins::param_specs::{ParamSpec, ParamType};

#[derive(Clone, Copy)]
struct ControllerGeometry {
    cell_width: f32,
    cell_height: f32,
    fader_cell_height: f32,
}

#[expect(
    clippy::cast_precision_loss,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
fn controller_geometry(
    available_width: f32,
    layout_scale: f32,
    columns: usize,
) -> ControllerGeometry {
    let scale = layout_scale.max(0.01);
    let columns = columns.max(1) as f32;
    let logical_width = available_width.max(CELL_W * scale) / scale;
    let logical_gap = 8.0;
    let logical_cell_width =
        ((logical_width - logical_gap * (columns - 1.0)) / columns).clamp(MIN_CELL_W, CELL_W);

    ControllerGeometry {
        cell_width: logical_cell_width * scale,
        cell_height: CELL_H * scale,
        fader_cell_height: FADER_CELL_H * scale,
    }
}

/// Render the interactive hardware view for a specific controller.
pub fn render_controller_view(
    d: &Ds,
    controller_id: &str,
    settings: &PluginSettings,
    plugin_index: usize,
    engine: &MidiMappingEngine,
    entity: Entity<AppState>,
    is_editing: bool,
    selected_param: usize,
    available_width: f32,
    layout_scale: f32,
    scroll_handle: &ScrollHandle,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    let Some(layout) = layout_for_controller_id(controller_id) else {
        return div()
            .p(d.section)
            .text_color(theme.text_muted)
            .child(format!("Unknown controller: {controller_id}"))
            .into_any_element();
    };

    let mapping = mapping_for_view(&layout, settings, plugin_index, engine);
    let current_page = mapping.current_page;
    let total_pages = mapping.total_pages.max(1);

    // Page indicator only when the device actually paginates — single-page
    // controllers (Xone:K2 / LCXL on small layouts) drop the header entirely
    // since the controller name already appears in the View dropdown above.
    let header: Option<AnyElement> = (total_pages > 1)
        .then(|| render_page_indicator(d, current_page, total_pages, theme).into_any_element());

    let grid = render_grid(
        d,
        &layout,
        &mapping,
        settings,
        entity,
        plugin_index,
        is_editing,
        selected_param,
        available_width,
        layout_scale,
        theme,
        cx,
    );
    let legend = render_legend(d, theme);

    let mut root = div().flex().flex_col().gap(d.section).p(d.pad_x);
    if let Some(h) = header {
        root = root.child(h);
    }
    let scrollport = div()
        .id("controller-grid-scroll")
        .w_full()
        .min_w_0()
        .overflow_x_scroll()
        .track_scroll(scroll_handle)
        .child(grid);
    #[cfg(feature = "dev-api")]
    let scrollport = {
        use crate::app::dev_api::DevTrackExt;
        scrollport.dev_track(format!("plugin.controller.scroll.{plugin_index}"))
    };
    root.child(scrollport).child(legend).into_any_element()
}

fn render_page_indicator(
    d: &Ds,
    current_page: usize,
    total_pages: usize,
    theme: &Theme,
) -> impl IntoElement {
    div().flex().justify_end().child(
        div()
            .text_size(d.text_xs)
            .text_color(theme.text_muted)
            .child(format!("Page {} / {}", current_page + 1, total_pages)),
    )
}

fn render_legend(d: &Ds, theme: &Theme) -> impl IntoElement {
    let chip = |label: &str, color| {
        div()
            .flex()
            .items_center()
            .gap(d.gap)
            .child(
                div()
                    .w(rems(0.625))
                    .h(rems(0.625))
                    .rounded_full()
                    .bg(color)
                    .border_1()
                    .border_color(theme.border),
            )
            .child(
                div()
                    .text_size(d.text_xs)
                    .text_color(theme.text_secondary)
                    .child(label.to_string()),
            )
    };

    div()
        .flex()
        .items_center()
        .gap(d.section)
        .child(chip("Pot", theme.accent))
        .child(chip("Encoder", theme.warning))
        .child(chip("Fader", theme.success))
        .child(chip("Button", theme.text_secondary))
        .child(chip("Reserved", theme.error))
}

#[allow(
    clippy::too_many_arguments,
    reason = "GPUI rendering and callback interfaces accept the complete UI context"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
fn render_grid(
    d: &Ds,
    layout: &ControllerLayout,
    mapping: &MidiMapping,
    settings: &PluginSettings,
    entity: Entity<AppState>,
    plugin_index: usize,
    is_editing: bool,
    selected_param: usize,
    available_width: f32,
    layout_scale: f32,
    theme: &Theme,
    cx: &App,
) -> impl IntoElement {
    let rows = layout.grid_rows.max(1);
    let cols = layout.grid_columns.max(1);
    let geometry = controller_geometry(available_width, layout_scale, cols as usize);
    // Match the eight-pixel logical gap used by the responsive cell solver.
    let column_gap = 8.0 * layout_scale.max(0.01);

    let mut row_elements: Vec<AnyElement> = Vec::with_capacity(rows as usize);
    for row in 0..rows {
        // Row height adapts to its tallest control so faders don't look squashed.
        let has_fader_in_row = layout
            .controls
            .iter()
            .any(|c| c.row == row && c.kind == PhysicalControlKind::Fader);
        let row_height = if has_fader_in_row {
            geometry.fader_cell_height
        } else {
            geometry.cell_height
        };

        let mut cells: Vec<AnyElement> = Vec::with_capacity(cols as usize);
        for col in 0..cols {
            let control = layout
                .controls
                .iter()
                .find(|c| c.row == row && c.column == col);

            let cell = match control {
                Some(ctrl) => render_cell(
                    d,
                    ctrl,
                    layout,
                    mapping,
                    settings,
                    entity.clone(),
                    plugin_index,
                    is_editing,
                    selected_param,
                    theme,
                    row_height,
                    geometry.cell_width,
                    cx,
                ),
                None => empty_cell(geometry.cell_width, row_height).into_any_element(),
            };
            cells.push(cell);
        }
        row_elements.push(
            div()
                .flex()
                .gap(px(column_gap))
                .h(px(row_height))
                .flex_shrink_0()
                .children(cells)
                .into_any_element(),
        );
    }

    div()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .w(px(
            geometry.cell_width * f32::from(cols) + column_gap * f32::from(cols - 1)
        ))
        .gap(d.gap_md)
        .children(row_elements)
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
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
fn render_cell(
    d: &Ds,
    ctrl: &PhysicalControl,
    layout: &ControllerLayout,
    mapping: &MidiMapping,
    settings: &PluginSettings,
    entity: Entity<AppState>,
    plugin_index: usize,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
    row_height: f32,
    cell_width: f32,
    cx: &App,
) -> AnyElement {
    let is_reserved = layout.reserved_control_ids.contains(&ctrl.id);
    let binding = if is_reserved {
        None
    } else {
        mapping
            .binding_for_control(&ctrl.id)
            .filter(|binding| binding.plugin_index == plugin_index)
    };

    // Inner widgets render with `LABEL_UNDERLINED` so the cell's outer card
    // is the only chassis (no double-border).
    let inner_theme = cell_inner_theme(theme);
    let widget_scale = (cell_width / CELL_W).clamp(0.01, 1.5);

    let (widget, header_text, header_color): (AnyElement, String, Rgba) =
        match (binding, is_reserved, ctrl.kind) {
            // Reserved nav button: show a non-interactive marker, keep the
            // physical id as the label so the user can find it on the device.
            (_, true, _) => (
                static_button(theme, theme.error, widget_scale).into_any_element(),
                ctrl.label.clone(),
                theme.error,
            ),
            // Fader on a continuous param.
            (Some(b), false, PhysicalControlKind::Fader) => render_continuous_widget(
                d,
                ctrl,
                b,
                settings,
                entity.clone(),
                plugin_index,
                is_editing,
                selected_param,
                &inner_theme,
                widget_scale,
                ContinuousKind::Fader,
                cx,
            )
            .map_or_else(
                || {
                    (
                        static_fader(theme, widget_scale).into_any_element(),
                        ctrl.label.clone(),
                        theme.text_muted,
                    )
                },
                |(el, label)| (el, label, theme.text_primary),
            ),
            // Pot / encoder on a continuous param.
            (
                Some(b),
                false,
                PhysicalControlKind::Pot
                | PhysicalControlKind::Encoder
                | PhysicalControlKind::EncoderWithButton,
            ) => render_continuous_widget(
                d,
                ctrl,
                b,
                settings,
                entity.clone(),
                plugin_index,
                is_editing,
                selected_param,
                &inner_theme,
                widget_scale,
                ContinuousKind::Knob,
                cx,
            )
            .map_or_else(
                || {
                    (
                        static_slider(theme, theme.accent, widget_scale).into_any_element(),
                        ctrl.label.clone(),
                        theme.text_muted,
                    )
                },
                |(el, label)| (el, label, theme.text_primary),
            ),
            // Button on a discrete param.
            (Some(b), false, PhysicalControlKind::Button) => render_button_widget(
                d,
                ctrl,
                b,
                settings,
                entity.clone(),
                plugin_index,
                is_editing,
                selected_param,
                theme,
                widget_scale,
                cx,
            )
            .map_or_else(
                || {
                    (
                        static_button(theme, theme.text_secondary, widget_scale).into_any_element(),
                        ctrl.label.clone(),
                        theme.text_muted,
                    )
                },
                |(el, label)| (el, label, theme.text_primary),
            ),
            // Unmapped controls: static placeholder + physical id label.
            (None, false, PhysicalControlKind::Fader) => (
                static_fader(theme, widget_scale).into_any_element(),
                ctrl.label.clone(),
                theme.text_muted,
            ),
            (None, false, PhysicalControlKind::Pot) => (
                static_slider(theme, theme.accent, widget_scale).into_any_element(),
                ctrl.label.clone(),
                theme.text_muted,
            ),
            (
                None,
                false,
                PhysicalControlKind::Encoder | PhysicalControlKind::EncoderWithButton,
            ) => (
                static_slider(theme, theme.warning, widget_scale).into_any_element(),
                ctrl.label.clone(),
                theme.text_muted,
            ),
            (None, false, PhysicalControlKind::Button) => (
                static_button(theme, theme.text_secondary, widget_scale).into_any_element(),
                ctrl.label.clone(),
                theme.text_muted,
            ),
        };

    // Cell — title-with-rule on top, widget below. No surrounding chassis:
    // the title + thin rule + bare widget read as one unit, and the row's
    // gap_md keeps neighbours visually separated. The rule color tracks the
    // header (binding ⇒ theme.border, unmapped ⇒ background_secondary,
    // reserved ⇒ theme.error) so the same hint of state still survives.
    let rule_color = if binding.is_some() {
        theme.border
    } else if is_reserved {
        theme.error
    } else {
        theme.background_secondary
    };

    let is_unmapped = binding.is_none() && !is_reserved;

    div()
        .w(px(cell_width))
        .h(px(row_height))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .items_center()
        .gap(d.gap)
        .opacity(if is_unmapped { 0.5 } else { 1.0 })
        // Keep unassigned physical controls discoverable without making them
        // look interactive: the dashed boundary and muted surface distinguish
        // them from mapped controls while preserving the device geometry.
        .when(is_unmapped, |el| {
            el.bg(Theme::opacity_20pct(theme.text_muted))
                .border_1()
                .border_dashed()
                .border_color(Theme::opacity_50pct(theme.text_muted))
        })
        // Header row: param name + thin underline rule sitting flush below.
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .w_full()
                .gap(d.grid)
                .child(
                    div()
                        .text_size(d.text_xs)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(header_color)
                        .text_ellipsis()
                        .max_w(px((cell_width - 8.0).max(24.0)))
                        .child(header_text),
                )
                // intentional: 1px hairline rule (visual element, not spacing)
                .child(div().h(px(1.0)).w(px(cell_width * 0.85)).bg(rule_color)),
        )
        .child(
            div()
                .flex_1()
                .w_full()
                .min_w_0()
                .flex()
                .items_center()
                .justify_center()
                .child(widget),
        )
        .into_any_element()
}

#[allow(
    clippy::too_many_arguments,
    reason = "GPUI rendering and callback interfaces accept the complete UI context"
)]
fn render_continuous_widget(
    d: &Ds,
    ctrl: &PhysicalControl,
    binding: &ControlBinding,
    settings: &PluginSettings,
    entity: Entity<AppState>,
    plugin_index: usize,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
    widget_scale: f32,
    kind: ContinuousKind,
    cx: &App,
) -> Option<(AnyElement, String)> {
    let spec = desktop_param_spec(settings, binding.param_index)?;
    if matches!(
        spec.param_type,
        ParamType::Bool { .. } | ParamType::Choice { .. }
    ) {
        return render_button_widget(
            d,
            ctrl,
            binding,
            settings,
            entity,
            plugin_index,
            is_editing,
            selected_param,
            theme,
            widget_scale,
            cx,
        );
    }
    let (min, max) = continuous_range(spec)?;
    if max <= min {
        return None;
    }
    let value = controller_param_value(&entity, settings, plugin_index, binding.param_index, cx)?;

    let element: AnyElement = match kind {
        ContinuousKind::Knob => render_controller_slider(
            entity,
            plugin_index,
            binding.param_index,
            spec,
            value,
            min,
            max,
            desktop_param_enabled(settings, binding.param_index),
            theme,
            widget_scale,
        ),
        ContinuousKind::Fader => render_compact_fader(
            entity,
            plugin_index,
            value,
            min,
            max,
            spec,
            binding.param_index,
            selected_param,
            is_editing,
            desktop_param_enabled(settings, binding.param_index),
            theme,
            widget_scale,
        )
        .into_any_element(),
    };

    Some((element, spec.name.to_string()))
}

/// Render a mapped continuous control with its actual display-space metadata.
#[expect(
    clippy::too_many_arguments,
    reason = "Controller cell geometry and binding are supplied by the existing renderer"
)]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
fn render_controller_slider(
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    spec: &ParamSpec,
    value: f64,
    min: f64,
    max: f64,
    interactive: bool,
    theme: &Theme,
    widget_scale: f32,
) -> AnyElement {
    let scale = spec.display_scale;
    let (min, max, value) = (min * scale, max * scale, value * scale);
    let step = match spec.param_type {
        ParamType::Float { step, .. } => step,
        ParamType::Int { step, .. } => step as f64,
        ParamType::Bool { .. } | ParamType::Choice { .. } => 1.0,
        ParamType::FilePath => return Empty.into_any_element(),
    } * scale.abs();
    // Leave an inset inside each physical controller cell, including its narrow layout.
    let width = (CELL_W * widget_scale - 12.0).max(60.0);
    let slider = Slider::new(("hw-slider", plugin_idx * 1000 + idx))
        .scroll_requires_alt(true)
        .value(value as f32)
        .range(min as f32, max as f32)
        .step(step as f32)
        .logarithmic(min > 0.0 && matches!(spec.unit, "Hz" | "ms"))
        .width(width)
        .size(SliderSize::Sm)
        .disabled(!interactive)
        .theme(theme.to_slider_theme())
        .aria_label(spec.name)
        .on_change({
            let entity = entity.clone();
            move |value, _, cx| {
                entity.update(cx, |state, cx| {
                    state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                    state.app.plugin_state.plugin_param_selection = idx;
                    state
                        .app
                        .set_plugin_param(plugin_idx, idx, f64::from(value));
                    cx.notify();
                });
            }
        })
        .on_reset({
            let entity = entity.clone();
            move |_, cx| {
                entity.update(cx, |state, cx| {
                    state.app.reset_plugin_param(plugin_idx, idx);
                    cx.notify();
                });
            }
        });
    let exact = render_controller_number(
        entity,
        plugin_idx,
        idx,
        spec,
        value,
        min,
        max,
        step,
        width,
        interactive,
        theme,
    );
    #[cfg(feature = "dev-api")]
    let slider = {
        use crate::app::dev_api::DevTrackExt;
        slider.dev_track(format!("plugin.controller.slider.{plugin_idx}.{idx}"))
    };
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap_1()
        .child(slider)
        .child(exact)
        .into_any_element()
}

/// Render exact entry in display units for either controller slider orientation.
#[expect(
    clippy::too_many_arguments,
    reason = "Canonical metadata and controller geometry are explicit"
)]
fn render_controller_number(
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    spec: &ParamSpec,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    width: f32,
    interactive: bool,
    theme: &Theme,
) -> AnyElement {
    let exact = NumberInput::new(("hw-number", plugin_idx * 1000 + idx))
        .scroll_requires_alt(true)
        .value(value)
        .min(min)
        .max(max)
        .step(step)
        .decimals(spec.precision())
        .unit(spec.unit)
        .width(width)
        .size(NumberInputSize::Xs)
        .disabled(!interactive)
        .theme(theme.to_number_input_theme())
        .aria_label(format!("{} value", spec.name))
        .on_change(move |value, _, cx| {
            entity.update(cx, |state, cx| {
                state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                state.app.plugin_state.plugin_param_selection = idx;
                state.app.set_plugin_param(plugin_idx, idx, value);
                cx.notify();
            });
        });
    #[cfg(feature = "dev-api")]
    let exact = {
        use crate::app::dev_api::DevTrackExt;
        exact.dev_track(format!("plugin.controller.exact.{plugin_idx}.{idx}"))
    };
    exact.into_any_element()
}

#[allow(
    clippy::too_many_arguments,
    reason = "GPUI rendering and callback interfaces accept the complete UI context"
)]
#[expect(
    clippy::cast_precision_loss,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
fn render_compact_fader(
    entity: Entity<AppState>,
    plugin_idx: usize,
    value: f64,
    min: f64,
    max: f64,
    spec: &ParamSpec,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    interactive: bool,
    theme: &Theme,
    widget_scale: f32,
) -> impl IntoElement {
    let is_selected = selected_param == idx && is_editing;
    // The setter accepts display-space values, as do the other numeric widgets.
    let display_scale = spec.display_scale;
    let slider = VerticalSlider::new(("hw-fader", plugin_idx * 1000 + idx))
        .scroll_requires_alt(true)
        .value(value * display_scale)
        .min(min * display_scale)
        .max(max * display_scale)
        .unit(spec.unit.to_string())
        .label(String::new())
        .size(if widget_scale >= 1.35 {
            VerticalSliderSize::Md
        } else {
            VerticalSliderSize::Sm
        })
        .selected(is_selected)
        .disabled(!interactive)
        .theme(super::super::common::theme_to_vertical_slider_theme(theme))
        .design_tokens(theme.layout.design_tokens.clone())
        .on_change({
            let entity = entity.clone();
            move |new_value, _, cx| {
                entity.update(cx, |state, cx| {
                    state.app.set_plugin_param(plugin_idx, idx, new_value);
                    cx.notify();
                });
            }
        })
        .on_reset({
            let entity = entity.clone();
            move |_, cx| {
                entity.update(cx, |state, cx| {
                    state.app.reset_plugin_param(plugin_idx, idx);
                    cx.notify();
                });
            }
        });
    let step = match spec.param_type {
        ParamType::Float { step, .. } => step,
        ParamType::Int { step, .. } => step as f64,
        _ => 1.0,
    } * display_scale.abs();
    let exact = render_controller_number(
        entity,
        plugin_idx,
        idx,
        spec,
        value * display_scale,
        min * display_scale,
        max * display_scale,
        step,
        (CELL_W * widget_scale - 12.0).max(60.0),
        interactive,
        theme,
    );
    #[cfg(feature = "dev-api")]
    let slider = {
        use crate::app::dev_api::DevTrackExt;
        slider.dev_track(format!("plugin.controller.fader.{plugin_idx}.{idx}"))
    };
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap_1()
        .child(slider)
        .child(exact)
}

/// Read the same EQ bank that the desktop editing route will modify.
fn controller_param_value(
    entity: &Entity<AppState>,
    settings: &PluginSettings,
    plugin_index: usize,
    parameter_index: usize,
    cx: &App,
) -> Option<f64> {
    if matches!(settings, PluginSettings::EQ { .. }) {
        let target =
            super::super::editing::active_eq_edit_target(&entity.read(cx).app, plugin_index)?;
        desktop_eq_param_value(settings, target, parameter_index)
    } else {
        desktop_param_value(settings, parameter_index)
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "Existing controller cell supplies its binding and geometry"
)]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Bounded UI controls deliberately round parameter values and navigation indices"
)]
#[expect(
    clippy::cast_precision_loss,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
fn render_button_widget(
    d: &Ds,
    _ctrl: &PhysicalControl,
    binding: &ControlBinding,
    settings: &PluginSettings,
    entity: Entity<AppState>,
    plugin_index: usize,
    _is_editing: bool,
    _selected_param: usize,
    theme: &Theme,
    widget_scale: f32,
    cx: &App,
) -> Option<(AnyElement, String)> {
    let index = binding.param_index;
    let spec = desktop_param_spec(settings, index)?;
    let value = controller_param_value(&entity, settings, plugin_index, index, cx)?;
    let interactive = desktop_param_enabled(settings, index);
    let labels = match spec.param_type {
        ParamType::Bool { .. } => &["Off", "On"][..],
        ParamType::Choice { labels, .. } if !labels.is_empty() => labels,
        ParamType::Float { .. } | ParamType::Int { .. } => {
            let (min, max) = continuous_range(spec)?;
            return Some((
                render_controller_slider(
                    entity,
                    plugin_index,
                    index,
                    spec,
                    value,
                    min,
                    max,
                    interactive,
                    theme,
                    widget_scale,
                ),
                spec.name.to_string(),
            ));
        }
        ParamType::Choice { .. } | ParamType::FilePath => return None,
    };
    // Long labels use a dropdown rather than widening the physical cell.
    let element =
        if labels.len() <= 3 && labels.iter().map(|label| label.len()).sum::<usize>() <= 12 {
            let count = labels.len();
            let choices = ButtonSet::new(("hw-choice", plugin_index * 1000 + index))
                .options(
                    labels
                        .iter()
                        .enumerate()
                        .map(|(index, label)| {
                            ButtonSetOption::new(index.to_string(), label.to_string())
                        })
                        .collect(),
                )
                .selected((value as usize).min(count - 1).to_string())
                .size(ButtonSetSize::Xs)
                .disabled(!interactive)
                .theme(theme.to_button_set_theme())
                .on_change(move |selected, _, cx| {
                    if let Ok(selected) = selected.parse::<usize>()
                        && selected < count
                    {
                        entity.update(cx, |state, cx| {
                            state
                                .app
                                .set_plugin_param(plugin_index, index, selected as f64);
                            cx.notify();
                        });
                    }
                });
            #[cfg(feature = "dev-api")]
            let choices = {
                use crate::app::dev_api::DevTrackExt;
                choices
                    .option_wrapper(move |option, button| {
                        button
                            .dev_track(format!(
                                "plugin.controller.choice.{plugin_index}.{index}.{option}"
                            ))
                            .into_any_element()
                    })
                    .dev_track(format!("plugin.controller.choice.{plugin_index}.{index}"))
            };
            choices.into_any_element()
        } else {
            super::super::ui_layout_renderer::render_compact_selector_value_width(
                d,
                entity,
                plugin_index,
                index,
                spec,
                value,
                interactive,
                theme,
                cx,
                (CELL_W * widget_scale - 12.0).max(60.0),
            )
        };
    Some((element, spec.name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controller_cells_shrink_before_the_grid_scrolls() {
        let geometry = controller_geometry(700.0, 1.0, 8);

        assert!(geometry.cell_width < CELL_W);
        assert!(geometry.cell_width >= MIN_CELL_W);
    }

    #[test]
    fn controller_geometry_scales_with_zoom() {
        let geometry = controller_geometry(2400.0, 1.5, 8);

        assert_eq!(geometry.cell_width, CELL_W * 1.5);
        assert_eq!(geometry.cell_height, CELL_H * 1.5);
        assert_eq!(geometry.fader_cell_height, FADER_CELL_H * 1.5);
    }
}
