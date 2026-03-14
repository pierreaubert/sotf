//! Generic 3-column plugin UI layout renderer.
//!
//! Renders plugin parameters automatically based on `ParamCategory` annotations
//! in the `ParamSpec` array. Plugins that use this renderer don't need custom
//! UI code — they declare categories on their params and get a consistent layout.
//!
//! Layout:
//! ```text
//! +------------------+--------------------------------------------+------------------+
//! | LEFT (Setup)     | CENTER-TOP (Primary)                       | RIGHT (Output)   |
//! |                  +--------------------------------------------+                  |
//! |                  | [Tab1] [Tab2] ...                          |                  |
//! |                  | CENTER-BOTTOM (Secondary / Diagnostic)      |                  |
//! +------------------+--------------------------------------------+------------------+
//! ```

use super::actions::{OpenIrFile, OpenSofaFile};
use super::common::{render_knob, render_section_title, render_toggle};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::PluginSettings;
use sotf_plugins::param_specs::{ParamCategory, ParamSpec, ParamType};
use std::collections::HashMap;

/// Input for the auto layout renderer.
pub struct AutoLayoutInput<'a> {
    pub entity: Entity<AppState>,
    pub plugin_idx: usize,
    pub params: &'a [ParamSpec],
    /// Current raw values for each param index.
    pub values: Vec<f64>,
    /// String values for FilePath params (param index → current path).
    pub file_paths: HashMap<usize, String>,
    pub is_editing: bool,
    pub selected_param: usize,
    /// Which secondary/diagnostic tab is currently active (index into tab list).
    pub active_tab: usize,
    pub theme: &'a Theme,
}

/// Render a single parameter control based on its ParamType.
fn render_param(
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    spec: &ParamSpec,
    value: f64,
    file_path: Option<&str>,
    selected_param: usize,
    is_editing: bool,
    theme: &Theme,
) -> AnyElement {
    match spec.param_type {
        ParamType::Bool {
            true_label,
            false_label,
            ..
        } => {
            let is_on = value > 0.5;
            let label = if true_label != "On" || false_label != "Off" {
                // Show state label for labeled bools (e.g., "Learn Noise: Active")
                let state_label = if is_on { true_label } else { false_label };
                format!("{}: {}", spec.name, state_label)
            } else {
                spec.name.to_string()
            };
            render_toggle(
                entity,
                plugin_idx,
                &label,
                is_on,
                idx,
                selected_param,
                is_editing,
                theme,
            )
            .into_any_element()
        }
        ParamType::Float { min, max, .. } => {
            let display_min = min * spec.display_scale;
            let display_max = max * spec.display_scale;
            let display_val = value * spec.display_scale;
            render_knob(
                entity,
                plugin_idx,
                spec.name,
                display_val,
                display_min,
                display_max,
                spec.unit,
                idx,
                selected_param,
                is_editing,
                None,
                theme,
            )
            .into_any_element()
        }
        ParamType::Int { min, max, .. } => render_knob(
            entity,
            plugin_idx,
            spec.name,
            value,
            min as f64,
            max as f64,
            spec.unit,
            idx,
            selected_param,
            is_editing,
            None,
            theme,
        )
        .into_any_element(),
        ParamType::Choice { labels, .. } => {
            let label = labels.get(value as usize).copied().unwrap_or("?");
            render_toggle(
                entity,
                plugin_idx,
                &format!("{}: {}", spec.name, label),
                true,
                idx,
                selected_param,
                is_editing,
                theme,
            )
            .into_any_element()
        }
        ParamType::FilePath => {
            let has_file = file_path.is_some_and(|p| !p.is_empty());
            let display_name = file_path
                .filter(|p| !p.is_empty())
                .and_then(|p| p.rsplit('/').next())
                .unwrap_or("None");
            let text_color = if has_file {
                theme.text_primary
            } else {
                theme.text_muted
            };
            let engine_key = spec.engine_key;
            div()
                .flex()
                .items_center()
                .justify_between()
                .px_3()
                .py_2()
                .rounded_lg()
                .bg(theme.background_secondary)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child(spec.name),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(text_color)
                                .overflow_hidden()
                                .text_ellipsis()
                                .max_w(px(120.0))
                                .child(display_name.to_string()),
                        ),
                )
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(theme.surface)
                        .border_1()
                        .border_color(theme.border)
                        .text_xs()
                        .id(("load-file-btn", plugin_idx * 1000 + idx))
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.surface_hover))
                        .on_click(move |_, _, cx| {
                            match engine_key {
                                "sofa_file" => {
                                    cx.dispatch_action(&OpenSofaFile { plugin_idx });
                                }
                                "ir_file" => {
                                    cx.dispatch_action(&OpenIrFile { plugin_idx });
                                }
                                _ => {
                                    log::warn!(
                                        "No file open action for engine_key: {}",
                                        engine_key
                                    );
                                }
                            }
                        })
                        .child("Load"),
                )
                .into_any_element()
        }
    }
}

/// Collect tab names from Secondary/Diagnostic params.
fn collect_tabs(params: &[ParamSpec]) -> Vec<&'static str> {
    let mut tabs: Vec<&'static str> = Vec::new();
    for spec in params {
        let tab_name = match spec.category {
            ParamCategory::Secondary(tab) => tab,
            ParamCategory::Diagnostic => "Diagnostic",
            _ => continue,
        };
        if !tabs.contains(&tab_name) {
            tabs.push(tab_name);
        }
    }
    tabs
}

/// Render the full 3-column auto layout for a plugin.
pub fn render_auto_layout(input: AutoLayoutInput) -> impl IntoElement {
    let AutoLayoutInput {
        entity,
        plugin_idx,
        params,
        values,
        file_paths,
        is_editing,
        selected_param,
        active_tab,
        theme,
    } = input;

    // Collect params by category
    let setup_params: Vec<(usize, &ParamSpec)> = params
        .iter()
        .enumerate()
        .filter(|(_, s)| s.category == ParamCategory::Setup)
        .collect();

    let primary_params: Vec<(usize, &ParamSpec)> = params
        .iter()
        .enumerate()
        .filter(|(_, s)| s.category == ParamCategory::Primary)
        .collect();

    let output_params: Vec<(usize, &ParamSpec)> = params
        .iter()
        .enumerate()
        .filter(|(_, s)| s.category == ParamCategory::Output)
        .collect();

    let tabs = collect_tabs(params);

    // Build the 3-column layout
    let mut row = div().flex().gap_6().items_start();

    // LEFT COLUMN: Setup params
    if !setup_params.is_empty() {
        let mut col = div().flex().flex_col().gap_2();
        col = col.child(render_section_title("SETUP", theme));
        for (idx, spec) in &setup_params {
            col = col.child(render_param(
                entity.clone(),
                plugin_idx,
                *idx,
                spec,
                values[*idx],
                file_paths.get(idx).map(|s| s.as_str()),
                selected_param,
                is_editing,
                theme,
            ));
        }
        row = row.child(col);
    }

    // CENTER COLUMN
    let mut center = div().flex().flex_col().gap_4().flex_1();

    // Center-top: Primary params grouped by `group` field
    if !primary_params.is_empty() {
        let mut groups: Vec<(&str, Vec<(usize, &ParamSpec)>)> = Vec::new();
        for (idx, spec) in &primary_params {
            if let Some(g) = groups.iter_mut().find(|(name, _)| *name == spec.group) {
                g.1.push((*idx, spec));
            } else {
                groups.push((spec.group, vec![(*idx, spec)]));
            }
        }

        let mut primary_row = div().flex().gap_6().items_start();
        for (group_name, group_params) in &groups {
            let mut col = div().flex().flex_col().gap_2();
            col = col.child(render_section_title(&group_name.to_uppercase(), theme));
            for (idx, spec) in group_params {
                col = col.child(render_param(
                    entity.clone(),
                    plugin_idx,
                    *idx,
                    spec,
                    values[*idx],
                    file_paths.get(idx).map(|s| s.as_str()),
                    selected_param,
                    is_editing,
                    theme,
                ));
            }
            primary_row = primary_row.child(col);
        }
        center = center.child(primary_row);
    }

    // Center-bottom: Tabs for Secondary/Diagnostic params
    if !tabs.is_empty() {
        let clamped_tab = active_tab.min(tabs.len().saturating_sub(1));

        // Tab bar
        let mut tab_bar = div().flex().gap_2();
        for (i, tab_name) in tabs.iter().enumerate() {
            let is_active = i == clamped_tab;
            let tab_entity = entity.clone();
            let tab_plugin_idx = plugin_idx;
            let tab_idx = i;
            tab_bar = tab_bar.child(
                div()
                    .text_xs()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .when(is_active, |d| {
                        d.bg(theme.background_secondary)
                            .text_color(theme.text_primary)
                    })
                    .when(!is_active, |d| d.text_color(theme.text_secondary))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        tab_entity.update(cx, |state, _| {
                            state.app.plugin_auto_tab.insert(tab_plugin_idx, tab_idx);
                        });
                    })
                    .child(tab_name.to_string()),
            );
        }
        center = center.child(tab_bar);

        // Active tab content
        let active_tab_name = tabs[clamped_tab];
        let tab_params: Vec<(usize, &ParamSpec)> = params
            .iter()
            .enumerate()
            .filter(|(_, s)| match s.category {
                ParamCategory::Secondary(tab) => tab == active_tab_name,
                ParamCategory::Diagnostic => active_tab_name == "Diagnostic",
                _ => false,
            })
            .collect();

        if !tab_params.is_empty() {
            let mut tab_content = div().flex().flex_wrap().gap_4();
            for (idx, spec) in &tab_params {
                tab_content = tab_content.child(render_param(
                    entity.clone(),
                    plugin_idx,
                    *idx,
                    spec,
                    values[*idx],
                    file_paths.get(idx).map(|s| s.as_str()),
                    selected_param,
                    is_editing,
                    theme,
                ));
            }
            center = center.child(tab_content);
        }
    }

    row = row.child(center);

    // RIGHT COLUMN: Output params
    if !output_params.is_empty() {
        let mut col = div().flex().flex_col().gap_2();
        col = col.child(render_section_title("OUTPUT", theme));
        for (idx, spec) in &output_params {
            col = col.child(render_param(
                entity.clone(),
                plugin_idx,
                *idx,
                spec,
                values[*idx],
                file_paths.get(idx).map(|s| s.as_str()),
                selected_param,
                is_editing,
                theme,
            ));
        }
        row = row.child(col);
    }

    div().flex().flex_col().gap_4().child(row)
}

/// Convenience: render a plugin using auto layout by extracting values from PluginSettings.
pub fn render_plugin_auto(
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    is_editing: bool,
    selected_param: usize,
    active_tab: usize,
    theme: &Theme,
) -> impl IntoElement {
    let params = settings.param_specs();
    let values: Vec<f64> = (0..params.len())
        .map(|i| settings.param_value(i).unwrap_or(0.0))
        .collect();

    // Extract file path strings for FilePath params
    let mut file_paths = HashMap::new();
    for (i, spec) in params.iter().enumerate() {
        if matches!(spec.param_type, ParamType::FilePath) {
            let path = match settings {
                PluginSettings::BinauralDecoder { sofa_file, .. } if spec.engine_key == "sofa_file" => {
                    sofa_file.clone()
                }
                PluginSettings::Convolution { ir_file, .. } if spec.engine_key == "ir_file" => {
                    ir_file.clone()
                }
                _ => String::new(),
            };
            file_paths.insert(i, path);
        }
    }

    render_auto_layout(AutoLayoutInput {
        entity,
        plugin_idx,
        params,
        values,
        file_paths,
        is_editing,
        selected_param,
        active_tab,
        theme,
    })
}
