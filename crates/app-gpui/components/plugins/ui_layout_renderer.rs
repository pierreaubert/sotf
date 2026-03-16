//! Generic Layout Renderer
//!
//! Renders any plugin that has a declarative `PluginLayout` definition.
//! Replaces 20+ hand-coded `render_*_plugin()` functions with a single
//! generic renderer driven by `PluginLayout` data + the constraint solver.
//!
//! Layout:
//! ```text
//! +------------------+--------------------------------------------+------------------+
//! | CONFIG           | MAIN (groups side-by-side or stacked)       | OUTPUT           |
//! |                  +--------------------------------------------+                  |
//! |                  | [Tab1] [Tab2] ...  (+ collapsed columns)   |                  |
//! +------------------+--------------------------------------------+------------------+
//! ```

use super::actions::{OpenIrFile, OpenSofaFile};
use super::common::{
    render_knob_sized, render_section_title, render_toggle,
    render_transfer_curve_with_level, render_vertical_slider_with_ticks,
};
use super::level_meters::render_gr_meter;
use crate::app::AppState;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::audio::potentiometer::PotentiometerSize;
use sotf_audio_player::PluginSettings;
use sotf_plugins::layout_solver::{solve_layout, KnobSize, SolvedLayout};
use sotf_plugins::param_specs::{ParamSpec, ParamType};
use sotf_plugins::plugin_layout::*;
use std::collections::HashMap;

// ============================================================================
// Public API
// ============================================================================

/// Render a plugin using its declarative layout.
///
/// This replaces individual `render_*_plugin()` functions. Call this for any
/// plugin that has a `PluginLayout` definition (i.e., `settings.layout().is_some()`).
pub fn render_from_layout(
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    is_editing: bool,
    selected_param: usize,
    active_tab: usize,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    available_width: f32,
    theme: &Theme,
) -> AnyElement {
    let layout = settings
        .layout()
        .expect("render_from_layout called on plugin without layout");
    let params = settings.param_specs();
    let values: Vec<f64> = (0..params.len())
        .map(|i| settings.param_value(i).unwrap_or(0.0))
        .collect();

    // Extract file paths for FilePath params
    let file_paths = extract_file_paths(params, settings);

    // Run the constraint solver
    let solved = solve_layout(layout.column_constraints, available_width);

    render_solved_layout(
        entity,
        plugin_idx,
        layout,
        params,
        &values,
        &file_paths,
        &solved,
        is_editing,
        selected_param,
        active_tab,
        plugin_data,
        theme,
    )
}

// ============================================================================
// Internal Rendering
// ============================================================================

fn render_solved_layout(
    entity: Entity<AppState>,
    plugin_idx: usize,
    layout: &'static PluginLayout,
    params: &[ParamSpec],
    values: &[f64],
    file_paths: &HashMap<usize, String>,
    solved: &SolvedLayout,
    is_editing: bool,
    selected_param: usize,
    active_tab: usize,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    theme: &Theme,
) -> AnyElement {
    let mut root = div().flex().flex_col().gap_4();

    // Build the main row (columns side-by-side, centered)
    let mut row = div().flex().gap_6().items_start().justify_center();

    for col in &solved.columns {
        match col.role {
            ColumnRole::Config if layout.has_config() => {
                row = row.child(render_config_column(
                    entity.clone(),
                    plugin_idx,
                    layout.config,
                    params,
                    values,
                    file_paths,
                    is_editing,
                    selected_param,
                    col.width,
                    solved.knob_size,
                    theme,
                ));
            }
            ColumnRole::Main => {
                row = row.child(render_main_column(
                    entity.clone(),
                    plugin_idx,
                    layout,
                    params,
                    values,
                    file_paths,
                    solved,
                    is_editing,
                    selected_param,
                    active_tab,
                    plugin_data,
                    theme,
                ));
            }
            ColumnRole::Output if layout.has_output() => {
                row = row.child(render_output_column(
                    entity.clone(),
                    plugin_idx,
                    layout.output,
                    params,
                    values,
                    file_paths,
                    is_editing,
                    selected_param,
                    plugin_data,
                    col.width,
                    solved.knob_size,
                    theme,
                ));
            }
            _ => {}
        }
    }

    root = root.child(row);
    root.into_any_element()
}

/// Render the config (left) column.
fn render_config_column(
    entity: Entity<AppState>,
    plugin_idx: usize,
    controls: &[ControlSpec],
    params: &[ParamSpec],
    values: &[f64],
    file_paths: &HashMap<usize, String>,
    is_editing: bool,
    selected_param: usize,
    width: f32,
    knob_size: KnobSize,
    theme: &Theme,
) -> impl IntoElement {
    let mut col = div().flex().flex_col().gap_2().w(px(width));
    col = col.child(render_section_title("CONFIG", theme));
    for spec in controls {
        col = col.child(render_control(
            entity.clone(),
            plugin_idx,
            spec,
            params,
            values,
            file_paths,
            is_editing,
            selected_param,
            None,
            knob_size,
            theme,
        ));
    }
    col
}

/// Render the main (center) column with groups and tabs.
fn render_main_column(
    entity: Entity<AppState>,
    plugin_idx: usize,
    layout: &'static PluginLayout,
    params: &[ParamSpec],
    values: &[f64],
    file_paths: &HashMap<usize, String>,
    solved: &SolvedLayout,
    is_editing: bool,
    selected_param: usize,
    active_tab: usize,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    theme: &Theme,
) -> impl IntoElement {
    let mut center = div().flex().flex_col().gap_4().flex_1();

    // Render control groups
    if !layout.main.is_empty() {
        let groups_container = if solved.group_direction == sotf_plugins::layout_solver::Direction::Row {
            div().flex().gap_6().items_start()
        } else {
            div().flex().flex_col().gap_4()
        };

        let mut container = groups_container;
        for group in layout.main {
            container = container.child(render_group(
                entity.clone(),
                plugin_idx,
                group,
                layout,
                params,
                values,
                file_paths,
                is_editing,
                selected_param,
                solved,
                plugin_data,
                theme,
            ));
        }
        center = center.child(container);
    }

    // Collect all tabs: explicit layout tabs + collapsed column tabs
    let all_tabs = collect_all_tabs(layout, solved);

    if !all_tabs.is_empty() {
        let clamped_tab = active_tab.min(all_tabs.len().saturating_sub(1));

        // Tab bar (underline style)
        let mut tab_bar = div()
            .flex()
            .border_b_1()
            .border_color(theme.border);
        for (i, (tab_name, _)) in all_tabs.iter().enumerate() {
            let is_active = i == clamped_tab;
            let tab_entity = entity.clone();
            let tab_plugin_idx = plugin_idx;
            let tab_idx = i;
            tab_bar = tab_bar.child(
                div()
                    .text_xs()
                    .px_4()
                    .pb(px(6.0))
                    .pt(px(4.0))
                    .cursor_pointer()
                    .id(SharedString::from(format!("layout-tab-{plugin_idx}-{i}")))
                    .font_weight(if is_active {
                        FontWeight::BOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(if is_active {
                        theme.accent
                    } else {
                        theme.text_muted
                    })
                    .border_b_2()
                    .border_color(if is_active {
                        theme.accent
                    } else {
                        gpui::rgba(0x00000000)
                    })
                    .hover(|s| {
                        s.text_color(theme.text_primary).border_color(if is_active {
                            theme.accent
                        } else {
                            theme.text_muted
                        })
                    })
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
        if let Some((_, tab_content)) = all_tabs.get(clamped_tab) {
            let mut tab_div = div().flex().flex_wrap().gap_4();
            for spec in *tab_content {
                tab_div = tab_div.child(render_control(
                    entity.clone(),
                    plugin_idx,
                    spec,
                    params,
                    values,
                    file_paths,
                    is_editing,
                    selected_param,
                    None,
                    solved.knob_size,
                    theme,
                ));
            }
            center = center.child(tab_div);
        }
    }

    center
}

/// Render the output (right) column.
fn render_output_column(
    entity: Entity<AppState>,
    plugin_idx: usize,
    controls: &[ControlSpec],
    params: &[ParamSpec],
    values: &[f64],
    file_paths: &HashMap<usize, String>,
    is_editing: bool,
    selected_param: usize,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    width: f32,
    knob_size: KnobSize,
    theme: &Theme,
) -> impl IntoElement {
    let mut col = div().flex().flex_col().gap_2().w(px(width));
    col = col.child(render_section_title("OUTPUT", theme));
    for spec in controls {
        col = col.child(render_control(
            entity.clone(),
            plugin_idx,
            spec,
            params,
            values,
            file_paths,
            is_editing,
            selected_param,
            plugin_data,
            knob_size,
            theme,
        ));
    }
    col
}

/// Render a control group (titled section with controls).
fn render_group(
    entity: Entity<AppState>,
    plugin_idx: usize,
    group: &ControlGroup,
    layout: &'static PluginLayout,
    params: &[ParamSpec],
    values: &[f64],
    file_paths: &HashMap<usize, String>,
    is_editing: bool,
    selected_param: usize,
    solved: &SolvedLayout,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    theme: &Theme,
) -> impl IntoElement {
    let mut col = div().flex().flex_col().gap_2();
    if !group.title.is_empty() {
        col = col.child(render_section_title(group.title, theme));
    }

    // Check if this group has sliders — if so, arrange horizontally
    let has_sliders = group
        .controls
        .iter()
        .any(|c| matches!(c.control_type, ControlType::VerticalSlider));

    if has_sliders {
        let mut slider_row = div().flex().gap_2().items_end();
        for spec in group.controls {
            slider_row = slider_row.child(render_control(
                entity.clone(),
                plugin_idx,
                spec,
                params,
                values,
                file_paths,
                is_editing,
                selected_param,
                plugin_data,
                solved.knob_size,
                theme,
            ));
        }
        col = col.child(slider_row);
    } else {
        // Knobs/toggles: wrap in a flex-wrap container
        let mut knob_row = div().flex().flex_wrap().gap_2();
        for spec in group.controls {
            knob_row = knob_row.child(render_control(
                entity.clone(),
                plugin_idx,
                spec,
                params,
                values,
                file_paths,
                is_editing,
                selected_param,
                plugin_data,
                solved.knob_size,
                theme,
            ));
        }
        col = col.child(knob_row);
    }

    // Render visualization below this group if specified
    if solved.show_visualizations && !group.title.is_empty() {
        for viz in layout.visualizations {
            match viz {
                VizSlot::TransferCurve {
                    position: VizPosition::BelowGroup(target),
                } if *target == group.title => {
                    col = col.child(render_transfer_curve_for_layout(
                        params,
                        values,
                        plugin_data,
                        theme,
                    ));
                }
                _ => {}
            }
        }
    }

    col
}

// ============================================================================
// Control Rendering
// ============================================================================

/// Convert solver knob size to GPUI potentiometer size.
fn pot_size(knob_size: KnobSize) -> PotentiometerSize {
    match knob_size {
        KnobSize::Xs => PotentiometerSize::Xs,
        KnobSize::Sm => PotentiometerSize::Sm,
        KnobSize::Md => PotentiometerSize::Md,
    }
}

/// One step larger than the solver's knob size (for KnobLarge controls).
fn pot_size_large(knob_size: KnobSize) -> PotentiometerSize {
    match knob_size {
        KnobSize::Xs => PotentiometerSize::Sm,
        KnobSize::Sm => PotentiometerSize::Md,
        KnobSize::Md => PotentiometerSize::Lg,
    }
}

/// Render a single control based on its ControlType.
fn render_control(
    entity: Entity<AppState>,
    plugin_idx: usize,
    spec: &ControlSpec,
    params: &[ParamSpec],
    values: &[f64],
    file_paths: &HashMap<usize, String>,
    is_editing: bool,
    selected_param: usize,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    knob_size: KnobSize,
    theme: &Theme,
) -> AnyElement {
    let idx = spec.param_index;

    match spec.control_type {
        ControlType::Knob => {
            if let Some(param) = params.get(idx) {
                let value = values.get(idx).copied().unwrap_or(0.0);
                render_param_as_knob(
                    entity, plugin_idx, idx, param, value, is_editing, selected_param,
                    pot_size(knob_size), theme,
                )
            } else {
                div().into_any_element()
            }
        }
        ControlType::KnobLarge => {
            if let Some(param) = params.get(idx) {
                let value = values.get(idx).copied().unwrap_or(0.0);
                render_param_as_knob(
                    entity, plugin_idx, idx, param, value, is_editing, selected_param,
                    pot_size_large(knob_size), theme,
                )
            } else {
                div().into_any_element()
            }
        }
        ControlType::VerticalSlider => {
            if let Some(param) = params.get(idx) {
                let value = values.get(idx).copied().unwrap_or(0.0);
                render_param_as_slider(
                    entity, plugin_idx, idx, param, value, is_editing, selected_param,
                    knob_size, theme,
                )
            } else {
                div().into_any_element()
            }
        }
        ControlType::Toggle => {
            if let Some(param) = params.get(idx) {
                let value = values.get(idx).copied().unwrap_or(0.0);
                render_param_as_toggle(
                    entity, plugin_idx, idx, param, value, is_editing, selected_param,
                    knob_size, theme,
                )
            } else {
                div().into_any_element()
            }
        }
        ControlType::ButtonSet { labels } => {
            if let Some(param) = params.get(idx) {
                let value = values.get(idx).copied().unwrap_or(0.0);
                render_param_as_button_set(
                    entity, plugin_idx, idx, param, value, labels, is_editing,
                    selected_param, theme,
                )
            } else {
                div().into_any_element()
            }
        }
        ControlType::Selector => {
            if let Some(param) = params.get(idx) {
                let value = values.get(idx).copied().unwrap_or(0.0);
                render_param_as_selector(
                    entity, plugin_idx, idx, param, value, is_editing, selected_param,
                    theme,
                )
            } else {
                div().into_any_element()
            }
        }
        ControlType::BarMeter { min_db, max_db } => {
            render_bar_meter(plugin_data, min_db, max_db, theme)
        }
        ControlType::Label => {
            if let Some(param) = params.get(idx) {
                let value = values.get(idx).copied().unwrap_or(0.0);
                render_param_as_label(param, value, theme)
            } else {
                div().into_any_element()
            }
        }
        ControlType::FilePicker => {
            if let Some(param) = params.get(idx) {
                let path = file_paths.get(&idx).map(|s| s.as_str());
                render_file_picker(plugin_idx, idx, param, path, theme)
            } else {
                div().into_any_element()
            }
        }
    }
}

/// Render a param as a knob (rotary potentiometer).
fn render_param_as_knob(
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    value: f64,
    is_editing: bool,
    selected_param: usize,
    size: PotentiometerSize,
    theme: &Theme,
) -> AnyElement {
    match param.param_type {
        ParamType::Float { min, max, .. } => {
            let display_min = min * param.display_scale;
            let display_max = max * param.display_scale;
            let display_val = value * param.display_scale;
            render_knob_sized(
                entity, plugin_idx, param.name, display_val, display_min, display_max,
                param.unit, idx, selected_param, is_editing, None, size, theme,
            )
            .into_any_element()
        }
        ParamType::Int { min, max, .. } => {
            render_knob_sized(
                entity, plugin_idx, param.name, value, min as f64, max as f64,
                param.unit, idx, selected_param, is_editing, None, size, theme,
            )
            .into_any_element()
        }
        _ => {
            // Bool/Choice as knob — fall back to toggle
            render_param_as_toggle(entity, plugin_idx, idx, param, value, is_editing, selected_param, KnobSize::Sm, theme)
        }
    }
}

/// Render a param as a vertical slider.
fn render_param_as_slider(
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    value: f64,
    is_editing: bool,
    selected_param: usize,
    knob_size: KnobSize,
    theme: &Theme,
) -> AnyElement {
    match param.param_type {
        ParamType::Float { min, max, .. } => {
            let display_min = min * param.display_scale;
            let display_max = max * param.display_scale;
            let display_val = value * param.display_scale;
            render_vertical_slider_with_ticks(
                entity, plugin_idx, param.name, display_val, display_min, display_max,
                param.unit, idx, selected_param, is_editing, None, 180.0, theme,
            )
            .into_any_element()
        }
        ParamType::Int { min, max, .. } => {
            render_vertical_slider_with_ticks(
                entity, plugin_idx, param.name, value, min as f64, max as f64,
                param.unit, idx, selected_param, is_editing, None, 180.0, theme,
            )
            .into_any_element()
        }
        _ => render_param_as_knob(
            entity, plugin_idx, idx, param, value, is_editing, selected_param,
            pot_size(knob_size), theme,
        ),
    }
}

/// Render a param as a toggle (for Bool and Choice types).
fn render_param_as_toggle(
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    value: f64,
    is_editing: bool,
    selected_param: usize,
    knob_size: KnobSize,
    theme: &Theme,
) -> AnyElement {
    match param.param_type {
        ParamType::Bool {
            true_label,
            false_label,
            ..
        } => {
            let is_on = value > 0.5;
            let label = if true_label != "On" || false_label != "Off" {
                let state_label = if is_on { true_label } else { false_label };
                format!("{}: {}", param.name, state_label)
            } else {
                param.name.to_string()
            };
            render_toggle(
                entity, plugin_idx, &label, is_on, idx, selected_param, is_editing,
                theme,
            )
            .into_any_element()
        }
        ParamType::Choice { labels, .. } => {
            let label = labels.get(value as usize).copied().unwrap_or("?");
            render_toggle(
                entity, plugin_idx,
                &format!("{}: {}", param.name, label),
                true, idx, selected_param, is_editing, theme,
            )
            .into_any_element()
        }
        _ => {
            // Float/Int as toggle doesn't make sense, render as knob
            render_param_as_knob(
                entity, plugin_idx, idx, param, value, is_editing, selected_param,
                pot_size(knob_size), theme,
            )
        }
    }
}

/// Render a param as a click-to-cycle selector (for Choice params).
///
/// Displays the current label; clicking advances to the next option.
fn render_param_as_selector(
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    value: f64,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
) -> AnyElement {
    match param.param_type {
        ParamType::Choice { labels, .. } => {
            let current = (value as usize).min(labels.len().saturating_sub(1));
            let label = labels.get(current).copied().unwrap_or("?");
            let is_sel = selected_param == idx && is_editing;
            let num_labels = labels.len();

            let sel_entity = entity.clone();
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .id(SharedString::from(format!("selector-{plugin_idx}-{idx}")))
                .when(is_sel, |d| d.border_1().border_color(theme.accent))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child(param.name),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child(label.to_string()),
                )
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    let next = (current + 1) % num_labels;
                    sel_entity.update(cx, |state, _| {
                        state.app.set_plugin_param(plugin_idx, idx, next as f64);
                    });
                })
                .into_any_element()
        }
        // Non-choice params: fall back to toggle
        _ => render_param_as_toggle(
            entity, plugin_idx, idx, param, value, is_editing, selected_param, KnobSize::Sm, theme,
        ),
    }
}

/// Render a param as a horizontal button set (mutually exclusive buttons).
fn render_param_as_button_set(
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    _param: &ParamSpec,
    value: f64,
    labels: &'static [&'static str],
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
) -> AnyElement {
    let current = value as usize;
    let is_sel = selected_param == idx && is_editing;

    let mut row = div()
        .flex()
        .gap_1()
        .rounded_md()
        .when(is_sel, |d| {
            d.border_1().border_color(theme.accent)
        });

    for (i, label) in labels.iter().enumerate() {
        let is_active = i == current;
        let btn_entity = entity.clone();
        let btn_idx = idx;
        let btn_plugin_idx = plugin_idx;
        let btn_val = i;
        row = row.child(
            div()
                .text_xs()
                .px_2()
                .py_1()
                .rounded_sm()
                .cursor_pointer()
                .id(SharedString::from(format!("btn-set-{plugin_idx}-{idx}-{i}")))
                .when(is_active, |d| {
                    d.bg(theme.accent).text_color(theme.text_on_accent)
                })
                .when(!is_active, |d| {
                    d.bg(theme.background_secondary)
                        .text_color(theme.text_secondary)
                })
                .hover(|d| d.opacity(0.8))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    btn_entity.update(cx, |state, _| {
                        state
                            .app
                            .set_plugin_param(btn_plugin_idx, btn_idx, btn_val as f64);
                    });
                })
                .child(label.to_string()),
        );
    }

    row.into_any_element()
}

/// Render a gain reduction meter.
fn render_bar_meter(
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    _min_db: f64,
    max_db: f64,
    theme: &Theme,
) -> AnyElement {
    // Extract gain reduction from plugin data (different types have different fields)
    let gr_db: f64 = plugin_data
        .and_then(|d| {
            if let Some(cd) = d.downcast_ref::<sotf_plugins::CompressorData>() {
                // Per-channel GR: take max across channels
                cd.gain_reduction_db
                    .iter()
                    .copied()
                    .reduce(f32::max)
                    .map(|v| v as f64)
            } else if let Some(ld) = d.downcast_ref::<sotf_plugins::LimiterData>() {
                Some(ld.gain_reduction_db as f64)
            } else if let Some(gd) = d.downcast_ref::<sotf_plugins::GateData>() {
                // Use attenuation_db: take max across channels
                gd.attenuation_db
                    .iter()
                    .copied()
                    .reduce(f32::max)
                    .map(|v| v as f64)
            } else {
                None
            }
        })
        .unwrap_or(0.0);

    render_gr_meter(gr_db, max_db, theme).into_any_element()
}

/// Render a read-only label for a param value.
fn render_param_as_label(param: &ParamSpec, value: f64, theme: &Theme) -> AnyElement {
    let display_val = value * param.display_scale;
    let formatted = if param.unit.is_empty() {
        format!("{:.1}", display_val)
    } else {
        format!("{:.1} {}", display_val, param.unit)
    };

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
                .text_xs()
                .text_color(theme.text_muted)
                .child(param.name),
        )
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_primary)
                .child(formatted),
        )
        .into_any_element()
}

/// Render a file picker with load button.
fn render_file_picker(
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    file_path: Option<&str>,
    theme: &Theme,
) -> AnyElement {
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
    let engine_key = param.engine_key;

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
                        .child(param.name),
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
                .id(SharedString::from(format!("load-file-btn-{plugin_idx}-{idx}")))
                .cursor_pointer()
                .hover(|s| s.bg(theme.surface_hover))
                .on_click(move |_, _, cx| match engine_key {
                    "sofa_file" => {
                        cx.dispatch_action(&OpenSofaFile { plugin_idx });
                    }
                    "ir_file" => {
                        cx.dispatch_action(&OpenIrFile { plugin_idx });
                    }
                    _ => {
                        log::warn!("No file open action for engine_key: {}", engine_key);
                    }
                })
                .child("Load"),
        )
        .into_any_element()
}

// ============================================================================
// Helpers
// ============================================================================

/// Render a transfer curve visualization using param values from the layout.
///
/// Looks up threshold, ratio, and knee by engine_key. For limiters (no ratio param),
/// uses ratio=∞ and is_limiter=true for proper brickwall rendering.
fn render_transfer_curve_for_layout(
    params: &[ParamSpec],
    values: &[f64],
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    theme: &Theme,
) -> AnyElement {
    // Look up dynamics params by engine_key
    let find_value = |key: &str| -> Option<f64> {
        params
            .iter()
            .enumerate()
            .find(|(_, p)| p.engine_key == key)
            .and_then(|(i, _)| values.get(i).copied())
    };

    let threshold_db = find_value("threshold").unwrap_or(-20.0);
    let knee_db = find_value("knee").unwrap_or(0.0);

    // Detect limiter: no ratio param means brickwall limiter
    let (ratio, is_limiter) = match find_value("ratio") {
        Some(r) => (r, false),
        None => (f64::INFINITY, true),
    };

    // Extract input level from plugin data for operating point indicator
    let input_level_db: Option<f64> = plugin_data.and_then(|d| {
        if let Some(cd) = d.downcast_ref::<sotf_plugins::CompressorData>() {
            cd.gain_reduction_db
                .iter()
                .copied()
                .reduce(f32::max)
                .map(|gr| {
                    // Estimate input level from GR: input ≈ threshold - GR
                    // (rough approximation for visualization)
                    threshold_db - gr as f64
                })
        } else if let Some(ld) = d.downcast_ref::<sotf_plugins::LimiterData>() {
            Some(ld.peak_db as f64)
        } else if let Some(gd) = d.downcast_ref::<sotf_plugins::GateData>() {
            gd.input_levels_db
                .iter()
                .copied()
                .reduce(f32::max)
                .map(|v| v as f64)
        } else {
            None
        }
    });

    render_transfer_curve_with_level(threshold_db, ratio, knee_db, is_limiter, 200.0, input_level_db, theme)
        .into_any_element()
}

/// Extract file path strings from PluginSettings for FilePath params.
fn extract_file_paths(params: &[ParamSpec], settings: &PluginSettings) -> HashMap<usize, String> {
    let mut file_paths = HashMap::new();
    for (i, spec) in params.iter().enumerate() {
        if matches!(spec.param_type, ParamType::FilePath) {
            let path = match settings {
                PluginSettings::BinauralDecoder { sofa_file, .. }
                    if spec.engine_key == "sofa_file" =>
                {
                    sofa_file.clone()
                }
                PluginSettings::Convolution { ir_file, .. }
                    if spec.engine_key == "ir_file" =>
                {
                    ir_file.clone()
                }
                PluginSettings::ABCompare {
                    path_a_config,
                    path_b_config,
                    ..
                } => {
                    if spec.engine_key == "path_a_config" {
                        path_a_config.clone()
                    } else if spec.engine_key == "path_b_config" {
                        path_b_config.clone()
                    } else {
                        String::new()
                    }
                }
                _ => String::new(),
            };
            file_paths.insert(i, path);
        }
    }
    file_paths
}

/// Collect all tabs: explicit LAYOUT tabs + collapsed column content.
fn collect_all_tabs(
    layout: &'static PluginLayout,
    solved: &SolvedLayout,
) -> Vec<(&'static str, &'static [ControlSpec])> {
    let mut tabs: Vec<(&'static str, &'static [ControlSpec])> = Vec::new();

    // Explicit tabs from the layout
    for tab in layout.tabs {
        tabs.push((tab.name, tab.controls));
    }

    // Collapsed columns become tabs
    for collapsed in &solved.collapsed_tabs {
        match collapsed.role {
            ColumnRole::Config if layout.has_config() => {
                tabs.push(("Config", layout.config));
            }
            ColumnRole::Output if layout.has_output() => {
                tabs.push(("Output", layout.output));
            }
            _ => {}
        }
    }

    tabs
}
