//! Simple text-based parameter list view for plugins
//!
//! Alternative to the graphical plugin views, showing all parameters
//! as a grouped table using the gpui-ui-kit Table component.
//! Left column (parameter name) is right-justified.
//! Right column (value) is editable via click-to-select + arrow keys.

use super::common::render_section_title;
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Column, SelectionMode, Table, TableTheme};
use sotf_audio_player::tui_params::TuiEditablePlugin;
use sotf_audio_player::PluginSettings;
use std::collections::HashSet;

/// Row data for the parameter table
#[derive(Clone)]
struct ParamRow {
    name: String,
    value_str: String,
}

/// Render a simple table-based parameter list for any plugin
pub fn render_simple_plugin_view(
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
) -> impl IntoElement {
    let descriptors = settings.get_descriptors();
    let params = settings.get_params();

    // Group params by their group name, tracking global param indices
    let mut groups: Vec<(String, Vec<ParamRow>, Vec<usize>)> = Vec::new();
    let mut current_group = String::new();

    for (i, (desc, param)) in descriptors.iter().zip(params.iter()).enumerate() {
        if desc.group != current_group {
            current_group = desc.group.clone();
            groups.push((current_group.clone(), Vec::new(), Vec::new()));
        }

        let value_str = if param.unit.is_empty() {
            param.value.clone()
        } else {
            format!("{} {}", param.value, param.unit)
        };

        if let Some(last) = groups.last_mut() {
            last.1.push(ParamRow {
                name: param.name.clone(),
                value_str,
            });
            last.2.push(i);
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

    let mut container = div().flex().flex_col().gap_2();

    for (group_name, rows, param_indices) in groups {
        // Section title above each group table
        container = container.child(
            div()
                .mt_2()
                .mb_1()
                .child(render_section_title(&group_name.to_uppercase(), theme)),
        );

        // Determine which row in this group is selected
        let selected_in_group: HashSet<usize> = if is_editing {
            param_indices
                .iter()
                .position(|&pi| pi == selected_param)
                .into_iter()
                .collect()
        } else {
            HashSet::new()
        };

        // Capture values for the selection change closure
        let param_indices_for_handler = param_indices.clone();
        let entity_for_handler = entity.clone();

        // Track which rows are selected for conditional styling in cell_render
        let selected_in_group_for_name = selected_in_group.clone();
        let selected_in_group_for_value = selected_in_group.clone();

        let table = Table::new(
            SharedString::from(format!("simple-params-{}", group_name)),
            rows,
        )
        .column(
            Column::new("name", "Parameter")
                .width(px(180.0))
                .sortable(false)
                .resizable(false)
                .cell_render(move |row: &ParamRow, row_idx, _, _| {
                    let is_sel = selected_in_group_for_name.contains(&row_idx);
                    div()
                        .w_full()
                        .flex()
                        .justify_end()
                        .child(
                            div()
                                .text_sm()
                                .text_color(if is_sel { accent } else { text_primary })
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
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if is_sel { accent } else { text_primary })
                                .child(row.value_str.clone()),
                        )
                        .when(is_sel, |d| {
                            d.child(
                                div()
                                    .text_xs()
                                    .text_color(accent)
                                    .child("◄ ►"),
                            )
                        })
                }),
        )
        .selection_mode(SelectionMode::Single)
        .selected_indices(selected_in_group)
        .on_selection_change(move |indices: &HashSet<usize>, _window, cx| {
            if let Some(&row_idx) = indices.iter().next()
                && let Some(&param_idx) = param_indices_for_handler.get(row_idx)
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
