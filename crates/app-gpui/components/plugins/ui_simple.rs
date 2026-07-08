//! Simple text-based parameter list view for plugins
//!
//! Alternative to the graphical plugin views, showing all parameters
//! as a grouped table using the gpui-ui-kit Table component.
//! Left column (parameter name) is right-justified.
//! Right column (value) is editable:
//!   - Float/Int: NumberInput with direct text entry
//!   - Bool: Toggle switch
//!   - Choice: clickable ◄/► buttons to cycle through options

use super::common::render_section_title;
use super::editing::PluginEditingManager;
use crate::app::AppState;
use crate::components::design::Ds;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Column, NumberInput, NumberInputSize, SelectionMode, Table, TableTheme, Toggle, ToggleSize,
};
use sotf_audio_player::PluginSettings;
use sotf_audio_player::ui_params::{TuiEditablePlugin, TuiParamType};
use sotf_audio_player_midi::mapping::MidiOverlay;
use std::collections::HashSet;

/// Row data for the parameter table
#[derive(Clone)]
struct ParamRow {
    name: String,
    value_str: String,
    value_f64: f64,
    param_type: TuiParamType,
    unit: String,
    global_param_idx: usize,
    /// For Bool params: whether currently true
    bool_value: bool,
    /// MIDI assignment info (if mapped)
    midi_assignment: Option<sotf_audio_player_midi::mapping::ParamAssignment>,
    /// Whether this param is the MIDI learn target
    is_learn_target: bool,
    /// Documentation string for this parameter
    doc: String,
}

/// Render a simple table-based parameter list for any plugin
pub fn render_simple_plugin_view(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
    midi_overlay: Option<&MidiOverlay>,
) -> impl IntoElement {
    let descriptors = settings.get_descriptors();
    let params = settings.get_params();

    // Group params by their group name
    let mut groups: Vec<(String, Vec<ParamRow>)> = Vec::new();
    let mut current_group = String::new();

    for (i, (desc, param)) in descriptors.iter().zip(params.iter()).enumerate() {
        if desc.group != current_group {
            current_group = desc.group.clone();
            groups.push((current_group.clone(), Vec::new()));
        }

        let value_str = if param.unit.is_empty() {
            param.value.clone()
        } else {
            format!("{} {}", param.value, param.unit)
        };

        let value_f64 = param.value.parse::<f64>().unwrap_or(0.0);
        let bool_value = matches!(
            param.value.to_lowercase().as_str(),
            "true" | "on" | "yes" | "1"
        );

        let midi_assignment = midi_overlay.and_then(|o| o.assignments.get(&i).cloned());
        let is_learn_target = midi_overlay
            .and_then(|o| o.learn_target)
            .is_some_and(|t| t == i);

        if let Some(last) = groups.last_mut() {
            last.1.push(ParamRow {
                name: param.name.clone(),
                value_str,
                value_f64,
                param_type: desc.param_type,
                unit: desc.unit.clone(),
                global_param_idx: i,
                bool_value,
                midi_assignment,
                is_learn_target,
                doc: desc.doc.clone(),
            });
        }
    }

    let table_theme = TableTheme {
        header_bg: theme.background_secondary,
        header_text: theme.text_muted,
        header_border: theme.border,
        row_bg: theme.surface,
        row_alt_bg: theme.background_secondary,
        row_hover_bg: theme.surface_hover,
        row_selected_bg: Theme::opacity_20pct(theme.accent),
        cell_text: theme.text_secondary,
        cell_border: theme.border,
        sort_icon_color: theme.accent,
        pagination_text: theme.text_muted,
        footer_bg: theme.background_secondary,
        footer_text: theme.text_secondary,
    };

    let accent = theme.accent;
    let text_primary = theme.text_primary;
    let text_muted = theme.text_muted;
    let warning_color = theme.warning;

    let mut container = div().flex().flex_col().gap(d.gap);

    for (group_name, rows) in groups {
        // Section title above each group table
        container = container.child(div().mt(d.gap).mb(d.grid).child(render_section_title(
            d,
            &group_name.to_uppercase(),
            theme,
        )));

        // Determine which row in this group is selected
        let selected_in_group: HashSet<usize> = if is_editing {
            rows.iter()
                .position(|row| row.global_param_idx == selected_param)
                .into_iter()
                .collect()
        } else {
            HashSet::new()
        };

        // Capture for the selection change closure
        let entity_for_handler = entity.clone();
        let rows_for_handler: Vec<usize> = rows.iter().map(|r| r.global_param_idx).collect();

        // Track which rows are selected for conditional styling in cell_render
        let selected_in_group_for_name = selected_in_group.clone();
        let selected_in_group_for_value = selected_in_group.clone();
        let theme_for_name = theme.clone();

        let entity_for_value = entity.clone();
        let ds_for_name = *d;
        let ds_for_value = *d;
        let ds_for_doc = *d;

        let table = Table::new(
            SharedString::from(format!("simple-params-{}", group_name)),
            rows,
        )
        .column(
            Column::new("name", "Parameter")
                .width(px(220.0)) // intentional: fixed plugin-control layout width
                .sortable(false)
                .resizable(false)
                .cell_render(move |row: &ParamRow, row_idx, _, _| {
                    let is_sel = selected_in_group_for_name.contains(&row_idx);
                    let mut name_row = div()
                        .w_full()
                        .flex()
                        .justify_end()
                        .items_center()
                        .gap(ds_for_name.grid);

                    // MIDI badge (before param name)
                    if let Some(ref assignment) = row.midi_assignment {
                        name_row = name_row.child(super::common::render_midi_badge(
                            &ds_for_name,
                            assignment,
                            &theme_for_name,
                        ));
                    }

                    let name_color = if row.is_learn_target {
                        warning_color
                    } else if is_sel {
                        accent
                    } else {
                        text_primary
                    };

                    name_row.child(
                        div()
                            .text_size(ds_for_name.text_sm)
                            .text_color(name_color)
                            .child(row.name.clone()),
                    )
                }),
        )
        .column(
            Column::new("value", "Value")
                .sortable(false)
                .resizable(false)
                .cell_render(move |row: &ParamRow, row_idx, _, _| {
                    let is_sel = selected_in_group_for_value.contains(&row_idx);

                    match row.param_type {
                        TuiParamType::Float { min, max, step } if is_sel => {
                            let dec = if step < 0.01 {
                                3
                            } else if step < 0.1 {
                                2
                            } else if step < 1.0 {
                                1
                            } else {
                                0
                            };

                            let param_idx = row.global_param_idx;
                            let entity_clone = entity_for_value.clone();

                            let mut input = NumberInput::new(SharedString::from(format!(
                                "simple-param-{}-{}",
                                plugin_idx, param_idx
                            )))
                            .value(row.value_f64)
                            .min(min)
                            .max(max)
                            .step(step)
                            .decimals(dec)
                            .size(NumberInputSize::Xs)
                            .on_change(move |value, _window, cx| {
                                entity_clone.update(cx, |state, cx| {
                                    state.app.set_plugin_param(plugin_idx, param_idx, value);
                                    cx.notify();
                                });
                            });

                            if !row.unit.is_empty() {
                                input = input.unit(row.unit.clone());
                            }

                            input.into_any_element()
                        }
                        TuiParamType::Int { min, max, step } if is_sel => {
                            let param_idx = row.global_param_idx;
                            let entity_clone = entity_for_value.clone();

                            let mut input = NumberInput::new(SharedString::from(format!(
                                "simple-param-{}-{}",
                                plugin_idx, param_idx
                            )))
                            .value(row.value_f64)
                            .min(min as f64)
                            .max(max as f64)
                            .step(step as f64)
                            .decimals(0)
                            .size(NumberInputSize::Xs)
                            .on_change(move |value, _window, cx| {
                                entity_clone.update(cx, |state, cx| {
                                    state.app.set_plugin_param(plugin_idx, param_idx, value);
                                    cx.notify();
                                });
                            });

                            if !row.unit.is_empty() {
                                input = input.unit(row.unit.clone());
                            }

                            input.into_any_element()
                        }
                        TuiParamType::Bool if is_sel => {
                            let param_idx = row.global_param_idx;
                            let entity_clone = entity_for_value.clone();

                            Toggle::new(SharedString::from(format!(
                                "simple-toggle-{}-{}",
                                plugin_idx, param_idx
                            )))
                            .checked(row.bool_value)
                            .size(ToggleSize::Sm)
                            .on_change(move |checked, _window, cx| {
                                entity_clone.update(cx, |state, cx| {
                                    state.app.set_plugin_param(
                                        plugin_idx,
                                        param_idx,
                                        if checked { 1.0 } else { 0.0 },
                                    );
                                    cx.notify();
                                });
                            })
                            .into_any_element()
                        }
                        TuiParamType::Choice { .. } if is_sel => {
                            let param_idx = row.global_param_idx;
                            let entity_prev = entity_for_value.clone();
                            let entity_next = entity_for_value.clone();

                            render_choice_buttons(
                                ds_for_value,
                                &row.value_str,
                                accent,
                                move |_window, cx| {
                                    entity_prev.update(cx, |state, cx| {
                                        state.app.adjust_selected_param(-1.0);
                                        cx.notify();
                                    });
                                },
                                move |_window, cx| {
                                    entity_next.update(cx, |state, cx| {
                                        state.app.adjust_selected_param(1.0);
                                        cx.notify();
                                    });
                                },
                                plugin_idx,
                                param_idx,
                            )
                            .into_any_element()
                        }
                        _ => {
                            // Static text for non-selected rows
                            div()
                                .flex()
                                .items_center()
                                .gap(ds_for_value.gap)
                                .child(
                                    div()
                                        .text_size(ds_for_value.text_sm)
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(if is_sel { accent } else { text_primary })
                                        .child(row.value_str.clone()),
                                )
                                .into_any_element()
                        }
                    }
                }),
        )
        .column(
            Column::new("doc", "Description")
                .sortable(false)
                .resizable(false)
                .cell_render(move |row: &ParamRow, _row_idx, _, _| {
                    div()
                        .text_size(ds_for_doc.text_xs)
                        .text_color(text_muted)
                        .child(row.doc.clone())
                }),
        )
        .selection_mode(SelectionMode::Single)
        .selected_indices(selected_in_group)
        .on_selection_change(move |indices: &HashSet<usize>, _window, cx| {
            if let Some(&row_idx) = indices.iter().next()
                && let Some(&param_idx) = rows_for_handler.get(row_idx)
            {
                entity_for_handler.update(cx, |state, _| {
                    state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                    state.app.plugin_state.plugin_param_selection = param_idx;
                });
            }
        })
        .alternating_rows(true)
        .theme(table_theme.clone());

        container = container.child(table);
    }

    container
}

/// Render clickable ◄ value ► buttons for Choice parameters
fn render_choice_buttons(
    d: Ds,
    value_str: &str,
    accent: Rgba,
    on_prev: impl Fn(&mut Window, &mut App) + 'static,
    on_next: impl Fn(&mut Window, &mut App) + 'static,
    plugin_idx: usize,
    param_idx: usize,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(d.grid)
        .child(
            div()
                .id(SharedString::from(format!(
                    "choice-prev-{}-{}",
                    plugin_idx, param_idx
                )))
                .text_size(d.text_sm)
                .text_color(accent)
                .cursor_pointer()
                .on_click(move |_, window, cx| on_prev(window, cx))
                .child("<"),
        )
        .child(
            div()
                .text_size(d.text_sm)
                .font_weight(FontWeight::MEDIUM)
                .text_color(accent)
                .mx(d.grid)
                .child(value_str.to_string()),
        )
        .child(
            div()
                .id(SharedString::from(format!(
                    "choice-next-{}-{}",
                    plugin_idx, param_idx
                )))
                .text_size(d.text_sm)
                .text_color(accent)
                .cursor_pointer()
                .on_click(move |_, window, cx| on_next(window, cx))
                .child(">"),
        )
}
