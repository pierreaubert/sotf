//! Dense table view using the same parameter addresses as the desktop editors.
//!
//! Numeric cells pair a slider with exact entry. Short choices use connected buttons;
//! longer choices use dropdowns. Name cells retain MIDI assignments and selection.

use super::common::render_section_title;
use super::editing::PluginEditingManager;
use crate::app::AppState;
use crate::app::i18n::PluginCommonTranslations;
use crate::components::design::Ds;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::{AnyElement, App, Empty, Entity, SharedString, div, px};
use gpui_ui_kit::{
    ButtonSet, ButtonSetOption, ButtonSetSize, Column, SelectionMode, Table, TableTheme,
};
use sotf_audio_player::PluginSettings;
use sotf_audio_player::controllers::plugin::{
    desktop_eq_param_value, desktop_param_enabled, desktop_param_indices, desktop_param_spec,
    desktop_param_value,
};
use sotf_audio_player_midi::mapping::MidiOverlay;
use sotf_plugins::param_specs::{ParamSpec, ParamType};
use std::collections::HashSet;

/// Row data for the parameter table
#[derive(Clone)]
struct ParamRow {
    name: String,
    value_f64: Option<f64>,
    spec: &'static ParamSpec,
    file_path: Option<String>,
    interactive: bool,
    global_param_idx: usize,
    /// MIDI assignment info (if mapped)
    midi_assignment: Option<sotf_audio_player_midi::mapping::ParamAssignment>,
    /// Whether this param is the MIDI learn target
    is_learn_target: bool,
    /// Documentation string for this parameter
    doc: String,
}

/// Whether table and controller editors have real parameter addresses.
pub(super) fn supports_parameter_views(settings: &PluginSettings) -> bool {
    !desktop_param_indices(settings).is_empty()
}

/// Render a simple table-based parameter list for any plugin
#[expect(
    clippy::too_many_lines,
    reason = "Keep the declarative layout and its interaction handlers together"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
pub fn render_simple_plugin_view(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    is_editing: bool,
    selected_param: usize,
    available_width: f32,
    layout_scale: f32,
    theme: &Theme,
    midi_overlay: Option<&MidiOverlay>,
    text: PluginCommonTranslations,
    cx: &App,
) -> impl IntoElement {
    let eq_target = super::editing::active_eq_edit_target(&entity.read(cx).app, plugin_idx);
    let file_paths =
        super::ui_layout_renderer::extract_file_paths(settings.param_specs(), settings);
    let shell_owned = super::ui_layout_renderer::shell_owned_param_indices(settings);
    let mut groups: Vec<(String, Vec<ParamRow>)> = Vec::new();
    for index in desktop_param_indices(settings) {
        if shell_owned.contains(&index) {
            continue;
        }
        let Some(spec) = desktop_param_spec(settings, index) else {
            continue;
        };
        let group = match settings {
            PluginSettings::EQ { .. } => format!("Filter {}", index / 4 + 1),
            PluginSettings::LinearPhaseEq { .. } if index < 50 => {
                format!("Filter {}", index / 5 + 1)
            }
            PluginSettings::LinearPhaseEq { .. } if index >= 200 => {
                format!("Filter {}", index - 200 + 1)
            }
            PluginSettings::DynamicEq { .. } if index >= 100 => {
                format!("Band {}", (index - 100) / 10 + 1)
            }
            PluginSettings::MultibandCompressor { .. }
            | PluginSettings::MultibandExpander { .. }
                if index >= 100 =>
            {
                format!("Band {}", index / 100)
            }
            _ => spec.group.to_string(),
        };
        let row = ParamRow {
            name: text.label(spec.name).to_string(),
            value_f64: if matches!(settings, PluginSettings::EQ { .. }) {
                eq_target.and_then(|target| desktop_eq_param_value(settings, target, index))
            } else {
                desktop_param_value(settings, index)
            },
            interactive: desktop_param_enabled(settings, index),
            spec,
            file_path: file_paths.get(&index).cloned(),
            global_param_idx: index,
            midi_assignment: midi_overlay
                .and_then(|overlay| overlay.assignments.get(&index).cloned()),
            is_learn_target: midi_overlay.and_then(|overlay| overlay.learn_target) == Some(index),
            doc: spec.doc.to_string(),
        };
        if let Some((_, rows)) = groups.iter_mut().find(|(name, _)| *name == group) {
            rows.push(row);
        } else {
            groups.push((group, vec![row]));
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
    // Below this content width, three columns force the slider and exact entry
    // onto separate lines. Reserve space for editing and expose docs on hover.
    // Column widths follow the measured rack slot, including font zoom.
    let content_width = (available_width - 2.0 * d.card.0 * 16.0 * layout_scale).max(1.0);
    let show_descriptions = content_width >= 720.0 * layout_scale;
    let name_width = content_width * if show_descriptions { 0.24 } else { 0.30 };
    let description_width = content_width * 0.30;

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
        let theme_for_name = theme.clone();

        let entity_for_value = entity.clone();
        let ds_for_name = *d;
        let ds_for_value = *d;
        let ds_for_doc = *d;
        let theme_for_value = theme.clone();

        let mut table = Table::new(
            SharedString::from(format!("simple-params-{group_name}")),
            rows,
        )
        .column(
            Column::new("name", "Parameter")
                .width(px(name_width))
                .sortable(false)
                .resizable(false)
                .cell_render(move |row: &ParamRow, row_idx, _, _| {
                    let is_sel = selected_in_group_for_name.contains(&row_idx);
                    let mut name_row = div()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .flex_wrap()
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

                    let name_row = name_row.child(
                        div()
                            .text_size(ds_for_name.text_sm)
                            .text_color(name_color)
                            .child(row.name.clone()),
                    );
                    #[cfg(feature = "dev-api")]
                    let name_row = {
                        use crate::app::dev_api::DevTrackExt;
                        name_row.dev_track(format!(
                            "plugin.simple.row.{plugin_idx}.{}",
                            row.global_param_idx
                        ))
                    };
                    let help = row.doc.clone();
                    let help_theme = theme_for_name.clone();
                    div()
                        .id((
                            "simple-param-help",
                            plugin_idx * 1000 + row.global_param_idx,
                        ))
                        .w_full()
                        .min_w_0()
                        .tooltip(move |_, cx| {
                            crate::components::themed_tooltip(help.clone(), &help_theme, cx)
                        })
                        .child(name_row)
                }),
        )
        .column(
            Column::new("value", "Value")
                .sortable(false)
                .resizable(false)
                .cell_render(move |row: &ParamRow, _row_idx, _, cx| {
                    let mut spec = *row.spec;
                    spec.name = text.label(spec.name);
                    match spec.param_type {
                        ParamType::Float { .. } | ParamType::Int { .. } => {
                            let Some(value) = row.value_f64 else {
                                return Empty.into_any_element();
                            };
                            super::ui_layout_renderer::render_horizontal_param_value(
                                &ds_for_value,
                                entity_for_value.clone(),
                                plugin_idx,
                                row.global_param_idx,
                                &spec,
                                value,
                                is_editing,
                                selected_param,
                                row.interactive,
                                &theme_for_value,
                            )
                        }
                        ParamType::Choice { labels, .. } if labels.len() > 3 => {
                            let Some(value) = row.value_f64 else {
                                return Empty.into_any_element();
                            };
                            super::ui_layout_renderer::render_compact_selector_value(
                                &ds_for_value,
                                entity_for_value.clone(),
                                plugin_idx,
                                row.global_param_idx,
                                &spec,
                                value,
                                row.interactive,
                                &theme_for_value,
                                cx,
                            )
                        }
                        ParamType::Bool {
                            false_label,
                            true_label,
                            ..
                        } => render_table_choices(
                            entity_for_value.clone(),
                            plugin_idx,
                            row.global_param_idx,
                            spec.name,
                            row.value_f64,
                            row.interactive,
                            &[false_label, true_label],
                            &theme_for_value,
                        ),
                        ParamType::Choice { labels, .. } => render_table_choices(
                            entity_for_value.clone(),
                            plugin_idx,
                            row.global_param_idx,
                            spec.name,
                            row.value_f64,
                            row.interactive,
                            labels,
                            &theme_for_value,
                        ),
                        ParamType::FilePath => super::ui_layout_renderer::render_file_picker(
                            &ds_for_value,
                            plugin_idx,
                            row.global_param_idx,
                            &spec,
                            row.file_path.as_deref(),
                            row.interactive,
                            &theme_for_value,
                        ),
                    }
                }),
        );
        if show_descriptions {
            table = table.column(
                Column::new("doc", "Description")
                    .width(px(description_width))
                    .sortable(false)
                    .resizable(false)
                    .cell_render(move |row: &ParamRow, _row_idx, _, _| {
                        let description = div()
                            .text_size(ds_for_doc.text_xs)
                            .text_color(text_muted)
                            .child(row.doc.clone());
                        #[cfg(feature = "dev-api")]
                        let description = {
                            use crate::app::dev_api::DevTrackExt;
                            description.dev_track(format!(
                                "plugin.simple.description.{plugin_idx}.{}",
                                row.global_param_idx
                            ))
                        };
                        description
                    }),
            );
        }
        let table = table
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

/// Render short choices without repeating the table's name-column label.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Bounded UI controls deliberately round parameter values and navigation indices"
)]
#[expect(
    clippy::cast_precision_loss,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
fn render_table_choices(
    entity: Entity<AppState>,
    plugin_idx: usize,
    param_idx: usize,
    _name: &'static str,
    value: Option<f64>,
    interactive: bool,
    labels: &[&str],
    theme: &Theme,
) -> AnyElement {
    let Some(value) = value else {
        return Empty.into_any_element();
    };
    let count = labels.len();
    let choices = ButtonSet::new(("simple-choice", plugin_idx * 1000 + param_idx))
        .options(
            labels
                .iter()
                .enumerate()
                .map(|(index, label)| ButtonSetOption::new(index.to_string(), label.to_string()))
                .collect(),
        )
        .selected((value as usize).min(count.saturating_sub(1)).to_string())
        .size(ButtonSetSize::Xs)
        .disabled(!interactive)
        .theme(theme.to_button_set_theme())
        .on_change(move |value, _, cx| {
            if let Ok(index) = value.parse::<usize>()
                && index < count
            {
                entity.update(cx, |state, cx| {
                    state
                        .app
                        .set_plugin_param(plugin_idx, param_idx, index as f64);
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
                        "plugin.simple.choice.{plugin_idx}.{param_idx}.{option}"
                    ))
                    .into_any_element()
            })
            .dev_track(format!("plugin.simple.choice.{plugin_idx}.{param_idx}"))
    };
    choices.into_any_element()
}
