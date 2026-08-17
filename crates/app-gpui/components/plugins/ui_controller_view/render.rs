// intentional-file: fixed pixel values here are graph and plugin control geometry.
use super::super::editing::PluginEditingManager;
use super::consts::BUTTON_SIZE;
use super::consts::CELL_H;
use super::consts::CELL_W;
use super::consts::FADER_CELL_H;
use super::consts::empty_cell;
use super::misc::cell_inner_theme;
use super::misc::continuous_range;
use super::misc::layout_for_controller_id;
use super::misc::mapping_for_view;
use super::static_::static_button;
use super::static_::static_fader;
use super::static_::static_knob;
use super::types::ContinuousKind;
use crate::app::AppState;
use crate::components::design::Ds;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_audio_kit::{PotentiometerSize, VerticalSlider, VerticalSliderSize};
use gpui_ui_kit::{Toggle, ToggleStyle};
use sotf_audio_player::PluginSettings;
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
        ((logical_width - logical_gap * (columns - 1.0)) / columns).clamp(72.0, CELL_W);

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
    theme: &Theme,
) -> AnyElement {
    let Some(layout) = layout_for_controller_id(controller_id) else {
        return div()
            .p(d.section)
            .text_color(theme.text_muted)
            .child(format!("Unknown controller: {controller_id}"))
            .into_any_element();
    };

    let mapping = mapping_for_view(&layout, settings, plugin_index, engine);
    let params = settings.param_specs();
    let current_page = mapping.current_page;
    let total_pages = mapping.total_pages.max(1);

    // Page indicator only when the device actually paginates — single-page
    // controllers (Xone:K2 / LCXL on small layouts) drop the header entirely
    // since the controller name already appears in the View dropdown above.
    let header: Option<AnyElement> = if total_pages > 1 {
        Some(render_page_indicator(d, current_page, total_pages, theme).into_any_element())
    } else {
        None
    };

    let grid = render_grid(
        d,
        &layout,
        &mapping,
        params,
        settings,
        entity,
        plugin_index,
        is_editing,
        selected_param,
        available_width,
        layout_scale,
        theme,
    );
    let legend = render_legend(d, theme);

    let mut root = div().flex().flex_col().gap(d.section).p(d.pad_x);
    if let Some(h) = header {
        root = root.child(h);
    }
    root.child(
        div()
            .id("controller-grid-scroll")
            .w_full()
            .overflow_x_scroll()
            .child(grid),
    )
    .child(legend)
    .into_any_element()
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

#[allow(clippy::too_many_arguments)]
fn render_grid(
    d: &Ds,
    layout: &ControllerLayout,
    mapping: &MidiMapping,
    params: &[ParamSpec],
    settings: &PluginSettings,
    entity: Entity<AppState>,
    plugin_index: usize,
    is_editing: bool,
    selected_param: usize,
    available_width: f32,
    layout_scale: f32,
    theme: &Theme,
) -> impl IntoElement {
    let rows = layout.grid_rows.max(1);
    let cols = layout.grid_columns.max(1);
    let geometry = controller_geometry(available_width, layout_scale, cols as usize);

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
                    params,
                    settings,
                    entity.clone(),
                    plugin_index,
                    is_editing,
                    selected_param,
                    theme,
                    row_height,
                    geometry.cell_width,
                ),
                None => empty_cell(geometry.cell_width, row_height).into_any_element(),
            };
            cells.push(cell);
        }
        row_elements.push(
            div()
                .flex()
                .gap(d.gap_md)
                .h(px(row_height))
                .flex_shrink_0()
                .children(cells)
                .into_any_element(),
        );
    }

    div().flex().flex_col().gap(d.gap_md).children(row_elements)
}

#[allow(clippy::too_many_arguments)]
fn render_cell(
    d: &Ds,
    ctrl: &PhysicalControl,
    layout: &ControllerLayout,
    mapping: &MidiMapping,
    params: &[ParamSpec],
    settings: &PluginSettings,
    entity: Entity<AppState>,
    plugin_index: usize,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
    row_height: f32,
    cell_width: f32,
) -> AnyElement {
    let is_reserved = layout.reserved_control_ids.contains(&ctrl.id);
    let binding = if is_reserved {
        None
    } else {
        mapping.binding_for_control(&ctrl.id)
    };

    // Inner widgets render with `LABEL_UNDERLINED` so the cell's outer card
    // is the only chassis (no double-border).
    let inner_theme = cell_inner_theme(theme);
    let widget_scale = (cell_width / CELL_W).clamp(0.75, 1.5);

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
                ctrl,
                b,
                params,
                settings,
                entity.clone(),
                plugin_index,
                is_editing,
                selected_param,
                &inner_theme,
                widget_scale,
                ContinuousKind::Fader,
            )
            .map(|(el, label)| (el, label, theme.text_primary))
            .unwrap_or_else(|| {
                (
                    static_fader(theme, widget_scale).into_any_element(),
                    ctrl.label.clone(),
                    theme.text_muted,
                )
            }),
            // Pot / encoder on a continuous param.
            (Some(b), false, PhysicalControlKind::Pot)
            | (Some(b), false, PhysicalControlKind::Encoder)
            | (Some(b), false, PhysicalControlKind::EncoderWithButton) => render_continuous_widget(
                ctrl,
                b,
                params,
                settings,
                entity.clone(),
                plugin_index,
                is_editing,
                selected_param,
                &inner_theme,
                widget_scale,
                ContinuousKind::Knob,
            )
            .map(|(el, label)| (el, label, theme.text_primary))
            .unwrap_or_else(|| {
                (
                    static_knob(theme, theme.accent, widget_scale).into_any_element(),
                    ctrl.label.clone(),
                    theme.text_muted,
                )
            }),
            // Button on a discrete param.
            (Some(b), false, PhysicalControlKind::Button) => render_button_widget(
                d,
                ctrl,
                b,
                params,
                settings,
                entity.clone(),
                plugin_index,
                is_editing,
                selected_param,
                theme,
                widget_scale,
            )
            .map(|(el, label)| (el, label, theme.text_primary))
            .unwrap_or_else(|| {
                (
                    static_button(theme, theme.text_secondary, widget_scale).into_any_element(),
                    ctrl.label.clone(),
                    theme.text_muted,
                )
            }),
            // Unmapped controls: static placeholder + physical id label.
            (None, false, PhysicalControlKind::Fader) => (
                static_fader(theme, widget_scale).into_any_element(),
                ctrl.label.clone(),
                theme.text_muted,
            ),
            (None, false, PhysicalControlKind::Pot) => (
                static_knob(theme, theme.accent, widget_scale).into_any_element(),
                ctrl.label.clone(),
                theme.text_muted,
            ),
            (None, false, PhysicalControlKind::Encoder)
            | (None, false, PhysicalControlKind::EncoderWithButton) => (
                static_knob(theme, theme.warning, widget_scale).into_any_element(),
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
                .flex()
                .items_center()
                .justify_center()
                .child(widget),
        )
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_continuous_widget(
    _ctrl: &PhysicalControl,
    binding: &ControlBinding,
    params: &[ParamSpec],
    settings: &PluginSettings,
    entity: Entity<AppState>,
    plugin_index: usize,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
    widget_scale: f32,
    kind: ContinuousKind,
) -> Option<(AnyElement, String)> {
    let spec = params.get(binding.param_index)?;
    let (min, max) = continuous_range(spec)?;
    if max <= min {
        return None;
    }
    let value = settings.param_value(binding.param_index).unwrap_or(min);

    let element: AnyElement = match kind {
        ContinuousKind::Knob => super::super::common::render_knob_sized(
            entity,
            plugin_index,
            "",
            value,
            min,
            max,
            spec.unit,
            binding.param_index,
            selected_param,
            is_editing,
            None,
            match widget_scale {
                scale if scale >= 1.35 => PotentiometerSize::Md,
                scale if scale >= 1.1 => PotentiometerSize::Sm,
                _ => PotentiometerSize::Xs,
            },
            theme,
        )
        .into_any_element(),
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
            theme,
            widget_scale,
        )
        .into_any_element(),
    };

    Some((element, spec.name.to_string()))
}

#[allow(clippy::too_many_arguments)]
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
    theme: &Theme,
    widget_scale: f32,
) -> impl IntoElement {
    let is_selected = selected_param == idx && is_editing;
    VerticalSlider::new(("hw-fader", plugin_idx * 1000 + idx))
        .value(value)
        .min(min)
        .max(max)
        .unit(spec.unit.to_string())
        .label("".to_string())
        .size(if widget_scale >= 1.35 {
            VerticalSliderSize::Md
        } else {
            VerticalSliderSize::Sm
        })
        .selected(is_selected)
        .theme(super::super::common::theme_to_vertical_slider_theme(theme))
        .design_tokens(theme.layout.design_tokens.clone())
        .on_change({
            let entity = entity.clone();
            move |new_value, _, cx| {
                entity.update(cx, |state, _| {
                    state.app.set_plugin_param(plugin_idx, idx, new_value);
                });
            }
        })
}

#[allow(clippy::too_many_arguments)]
fn render_button_widget(
    d: &Ds,
    _ctrl: &PhysicalControl,
    binding: &ControlBinding,
    params: &[ParamSpec],
    settings: &PluginSettings,
    entity: Entity<AppState>,
    plugin_index: usize,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
    widget_scale: f32,
) -> Option<(AnyElement, String)> {
    let spec = params.get(binding.param_index)?;
    let value = settings.param_value(binding.param_index).unwrap_or(0.0);

    let element: AnyElement = match spec.param_type {
        ParamType::Bool { .. } => {
            let enabled = value > 0.5;
            let toggle = Toggle::new(("hw-toggle", plugin_index * 1000 + binding.param_index))
                .checked(enabled)
                .label("".to_string())
                .style(ToggleStyle::Segmented)
                .selected(selected_param == binding.param_index && is_editing)
                .theme(theme.to_toggle_theme())
                .on_change({
                    let entity = entity.clone();
                    let param_idx = binding.param_index;
                    move |new_value, _, cx| {
                        entity.update(cx, |state, _| {
                            state.app.set_plugin_param(
                                plugin_index,
                                param_idx,
                                if new_value { 1.0 } else { 0.0 },
                            );
                        });
                    }
                });
            div()
                .w(px(BUTTON_SIZE * 1.6 * widget_scale))
                .h(px(BUTTON_SIZE * widget_scale))
                .child(toggle)
                .into_any_element()
        }
        ParamType::Choice { labels, .. } => {
            let count = labels.len().max(1);
            let cur = (value as usize).min(count - 1);
            let next = (cur + 1) % count;
            let label_for_cell = labels.get(cur).copied().unwrap_or_default().to_string();
            let next_label = labels.get(next).copied().unwrap_or_default();
            let display = format!("{} → {}", label_for_cell, next_label);
            let element = div()
                .id(("hw-choice-btn", plugin_index * 1000 + binding.param_index))
                .w(px(BUTTON_SIZE * 1.6 * widget_scale))
                .h(px(BUTTON_SIZE * widget_scale))
                .rounded(px(4.0))
                .bg(theme.background_secondary)
                .border_1()
                .border_color(theme.accent)
                .flex()
                .items_center()
                .justify_center()
                .text_size(d.text_xs)
                .text_color(theme.text_primary)
                .child(display)
                .on_mouse_down(MouseButton::Left, {
                    let entity = entity.clone();
                    let param_idx = binding.param_index;
                    move |_, _, cx| {
                        entity.update(cx, |state, _| {
                            state
                                .app
                                .set_plugin_param(plugin_index, param_idx, next as f64);
                        });
                    }
                });
            // Append the current choice value to the assignment label.
            return Some((
                element.into_any_element(),
                format!("{} = {}", spec.name, label_for_cell),
            ));
        }
        ParamType::Int { min, max, .. } => {
            // Step the int value upward, wrapping at max.
            let cur = value as i64;
            let next = if cur >= max { min } else { cur + 1 };
            let element = div()
                .id(("hw-int-btn", plugin_index * 1000 + binding.param_index))
                .w(px(BUTTON_SIZE * 1.6 * widget_scale))
                .h(px(BUTTON_SIZE * widget_scale))
                .rounded(px(4.0))
                .bg(theme.background_secondary)
                .border_1()
                .border_color(theme.accent)
                .flex()
                .items_center()
                .justify_center()
                .text_size(d.text_xs)
                .text_color(theme.text_primary)
                .child(format!("{cur}"))
                .on_mouse_down(MouseButton::Left, {
                    let entity = entity.clone();
                    let param_idx = binding.param_index;
                    move |_, _, cx| {
                        entity.update(cx, |state, _| {
                            state
                                .app
                                .set_plugin_param(plugin_index, param_idx, next as f64);
                        });
                    }
                });
            return Some((element.into_any_element(), format!("{} = {cur}", spec.name)));
        }
        // Float / FilePath bound to a button: render as a static placeholder.
        ParamType::Float { .. } | ParamType::FilePath => {
            return Some((
                static_button(theme, theme.text_secondary, widget_scale).into_any_element(),
                spec.name.to_string(),
            ));
        }
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
        assert!(geometry.cell_width >= 72.0);
    }

    #[test]
    fn controller_geometry_scales_with_zoom() {
        let geometry = controller_geometry(1200.0, 1.5, 8);

        assert_eq!(geometry.cell_width, CELL_W * 1.5);
        assert_eq!(geometry.cell_height, CELL_H * 1.5);
        assert_eq!(geometry.fader_cell_height, FADER_CELL_H * 1.5);
    }
}
