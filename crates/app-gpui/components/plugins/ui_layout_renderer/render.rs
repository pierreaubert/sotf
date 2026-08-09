use super::super::actions::{OpenAbConfigFile, OpenIrFile, OpenSofaFile};
use super::super::common::{
    render_knob_sized_enabled, render_section_title, render_toggle_enabled,
    render_transfer_curve_with_level, render_vertical_slider_with_ticks_enabled,
};
use super::super::level_meters::render_gr_meter;
use super::misc::AUTO_COLUMN_MIN_SIDE_WIDTH;
use super::misc::control_column_width;
use super::misc::extract_file_paths;
use super::misc::visible_control_count;
use super::mode_selector_info::detect_mode_selector;
use super::mode_selector_info::mode_visible_groups;
use super::mode_selector_info::solve_main_groups;
use super::pot::pot_size;
use super::pot::pot_size_large;
use super::types::LayoutTabContent;
use super::types::collect_all_tabs;
use crate::app::AppState;
use crate::app::i18n::PluginCommonTranslations;
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName};
use crate::components::plugins::editing::PluginEditingManager;
use crate::components::plugins::theme::PluginTheme;
use crate::components::themed_tooltip;
use crate::plugin_file_picker::{FilePickerOpenTarget, file_picker_open_target};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_audio_kit::audio::potentiometer::PotentiometerSize;
use gpui_ui_kit::{
    AdaptiveOverflow, Button, ButtonSize, ButtonVariant, IconButton, IconButtonSize,
    IconButtonVariant,
};
use sotf_audio_player::PluginSettings;
use sotf_plugins::layout_solver::{Direction, KnobSize, SolvedLayout, solve_layout_scaled};
use sotf_plugins::param_specs::{ParamSpec, ParamType};
use sotf_plugins::plugin_layout::*;
use std::collections::HashMap;

/// Render a plugin using its declarative layout.
///
/// This replaces individual `render_*_plugin()` functions. Call this for any
/// plugin that has a `PluginLayout` definition (i.e., `settings.layout().is_some()`).
#[allow(clippy::too_many_arguments)]
pub fn render_from_layout(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    is_editing: bool,
    selected_param: usize,
    active_tab: usize,
    overflow_open: bool,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    available_width: f32,
    layout_scale: f32,
    text: PluginCommonTranslations,
    theme: &Theme,
    plugin_theme: &PluginTheme,
    spider_snapshot: Option<crate::components::plugins::spatial_spider::SpatialSpiderSnapshot>,
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
    let solved = solve_layout_scaled(layout.column_constraints, available_width, layout_scale);

    // Overlay the chassis theme onto the global app theme so every helper
    // that takes `&Theme` (section title, knob, toggle, panel, ...) picks up
    // the chassis colors with no signature changes downstream. Semantic
    // colors (error / warning / meter palette) keep their global values.
    let chassis_theme = plugin_theme.apply_to(theme);

    render_solved_layout(
        d,
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
        overflow_open,
        plugin_data,
        available_width,
        layout_scale,
        text,
        &chassis_theme,
        spider_snapshot.as_ref(),
    )
}

/// Render only the primary control groups from a plugin's declarative layout.
///
/// Custom plugin views use this when they need bespoke content for one portion
/// of a plugin but still need the standard parameter controls.
#[allow(clippy::too_many_arguments)]
pub fn render_main_controls_from_layout(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    is_editing: bool,
    selected_param: usize,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    available_width: f32,
    layout_scale: f32,
    theme: &Theme,
) -> AnyElement {
    let Some(layout) = settings.layout() else {
        return div().into_any_element();
    };
    let params = settings.param_specs();
    let values: Vec<f64> = (0..params.len())
        .map(|i| settings.param_value(i).unwrap_or(0.0))
        .collect();
    let file_paths = extract_file_paths(params, settings);
    let solved = solve_layout_scaled(layout.column_constraints, available_width, layout_scale);
    let main_width = solved
        .column_width(ColumnRole::Main)
        .unwrap_or(available_width);

    render_main_column(
        d,
        entity,
        plugin_idx,
        layout,
        params,
        &values,
        &file_paths,
        &solved,
        main_width,
        is_editing,
        selected_param,
        0,
        false,
        plugin_data,
        layout_scale,
        None,
        None,
        theme,
        false,
    )
    .into_any_element()
}

/// Render only setup/output controls for use in the rack configuration
/// popover. The main plugin surface intentionally stays focused on the
/// primary controls; setup and generated output controls live behind the gear.
#[allow(clippy::too_many_arguments)]
pub fn render_config_controls_from_layout(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    is_editing: bool,
    selected_param: usize,
    available_width: f32,
    layout_scale: f32,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    theme: &Theme,
    plugin_theme: &PluginTheme,
) -> Option<AnyElement> {
    let layout = settings.layout()?;
    if layout.config.is_empty() && layout.output.is_empty() {
        return None;
    }

    let params = settings.param_specs();
    let values: Vec<f64> = (0..params.len())
        .map(|i| settings.param_value(i).unwrap_or(0.0))
        .collect();
    let file_paths = extract_file_paths(params, settings);
    let solved = solve_layout_scaled(layout.column_constraints, available_width, layout_scale);
    let chassis_theme = plugin_theme.apply_to(theme);

    let mut content = div().flex().flex_col().gap(d.section);
    if !layout.config.is_empty() {
        content = content.child(render_config_column(
            d,
            entity.clone(),
            plugin_idx,
            "CONFIG",
            layout.config,
            params,
            &values,
            &file_paths,
            is_editing,
            selected_param,
            plugin_data,
            available_width.max(AUTO_COLUMN_MIN_SIDE_WIDTH),
            solved.knob_size,
            solved.slider_height,
            &chassis_theme,
        ));
    }
    if !layout.output.is_empty() {
        content = content.child(render_config_column(
            d,
            entity,
            plugin_idx,
            "OUTPUT",
            layout.output,
            params,
            &values,
            &file_paths,
            is_editing,
            selected_param,
            plugin_data,
            available_width.max(AUTO_COLUMN_MIN_SIDE_WIDTH),
            solved.knob_size,
            solved.slider_height,
            &chassis_theme,
        ));
    }

    Some(content.into_any_element())
}

/// Preferred width for the generated setup popover, derived from the controls
/// it will actually contain. File paths and long choice rows receive more room;
/// compact numeric/toggle layouts stay narrow.
pub fn config_controls_preferred_width(settings: &PluginSettings, layout_scale: f32) -> f32 {
    let Some(layout) = settings.layout() else {
        return 220.0 * layout_scale;
    };
    let params = settings.param_specs();
    let controls = layout.config.iter().chain(layout.output.iter());
    let mut preferred = layout
        .column_constraints
        .iter()
        .filter(|constraint| matches!(constraint.role, ColumnRole::Config | ColumnRole::Output))
        .map(|constraint| constraint.preferred_width)
        .fold(0.0_f32, f32::max);

    for control in controls.filter(|control| !control.hidden) {
        let Some(param) = params.get(control.param_index) else {
            continue;
        };
        let control_width = match &param.param_type {
            ParamType::FilePath => 360.0,
            ParamType::Choice { labels, .. } => {
                let label_width: usize = labels.iter().map(|label| label.chars().count() + 3).sum();
                40.0 + label_width as f32 * 7.0
            }
            _ => 120.0 + param.name.chars().count() as f32 * 7.0,
        };
        preferred = preferred.max(control_width);
    }

    preferred.clamp(220.0, 420.0) * layout_scale
}

/// Render explicit bottom tabs from a plugin's declarative layout.
///
/// Custom plugin views use this for layout-declared supplemental controls
/// while keeping their bespoke main surface.
#[allow(clippy::too_many_arguments)]
pub fn render_tabs_from_layout(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    is_editing: bool,
    selected_param: usize,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    available_width: f32,
    layout_scale: f32,
    theme: &Theme,
) -> AnyElement {
    let Some(layout) = settings.layout() else {
        return div().into_any_element();
    };
    if layout.tabs.is_empty() {
        return div().into_any_element();
    }

    let params = settings.param_specs();
    let values: Vec<f64> = (0..params.len())
        .map(|i| settings.param_value(i).unwrap_or(0.0))
        .collect();
    let file_paths = extract_file_paths(params, settings);
    let solved = solve_layout_scaled(layout.column_constraints, available_width, layout_scale);

    let mut container = div().flex().flex_col().gap(d.gap);
    for tab in collect_all_tabs(layout, &solved, &[]) {
        container = container.child(
            div()
                .flex()
                .flex_col()
                .gap(d.gap)
                .child(render_section_title(d, tab.name, theme))
                .child(render_layout_tab_content(
                    d,
                    entity.clone(),
                    plugin_idx,
                    tab.content,
                    layout,
                    params,
                    &values,
                    &file_paths,
                    is_editing,
                    selected_param,
                    &solved,
                    plugin_data,
                    theme,
                    None,
                    None,
                )),
        );
    }

    container.into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_solved_layout(
    d: &Ds,
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
    overflow_open: bool,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    available_width: f32,
    layout_scale: f32,
    text: PluginCommonTranslations,
    theme: &Theme,
    spider_snapshot: Option<&crate::components::plugins::spatial_spider::SpatialSpiderSnapshot>,
) -> AnyElement {
    let mut root = div()
        .flex()
        .flex_col()
        .gap(d.section)
        .size_full()
        .bg(theme.background);

    let main_width = solved
        .column_width(ColumnRole::Main)
        .unwrap_or(available_width);

    let row = div()
        .flex()
        .items_start()
        .justify_center()
        .w_full()
        .child(render_main_column(
            d,
            entity.clone(),
            plugin_idx,
            layout,
            params,
            values,
            file_paths,
            solved,
            main_width,
            is_editing,
            selected_param,
            active_tab,
            overflow_open,
            plugin_data,
            layout_scale,
            spider_snapshot,
            Some(text),
            theme,
            true,
        ));

    root = root.child(row);

    // Custom visualizations rendered at the root level (FullCenter position).
    // BelowGroup positions are handled inside render_main_column where the
    // target group is in scope.
    if let Some(snapshot) = spider_snapshot.as_ref() {
        for viz in layout.visualizations {
            if let VizSlot::Custom { name, position } = viz
                && *name == sotf_plugins::plugin_layout::viz_names::SPATIAL_SPIDER
                && matches!(position, VizPosition::FullCenter)
            {
                root = root.child(render_spatial_spider_viz(
                    d,
                    entity.clone(),
                    plugin_idx,
                    snapshot,
                    text,
                    theme,
                ));
            }
        }
    }

    root.into_any_element()
}

/// Render the spatial spider (SPL / correlation) panel for any layout-driven
/// plugin that opts in via `VizSlot::Custom { name: "spatial_spider", ... }`.
/// Thin wrapper around the shared
/// [`spatial_spider::render_spatial_spider_panel`] — bundles in the speaker
/// config inference from the loudness data's channel count.
fn render_spatial_spider_viz(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    snapshot: &crate::components::plugins::spatial_spider::SpatialSpiderSnapshot,
    text: PluginCommonTranslations,
    theme: &Theme,
) -> AnyElement {
    crate::components::plugins::spatial_spider::render_spatial_spider_panel(
        d, entity, plugin_idx, snapshot, None, text, theme,
    )
}

/// Render the config (left) column.
fn render_config_column(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    title: &str,
    controls: &[ControlSpec],
    params: &[ParamSpec],
    values: &[f64],
    file_paths: &HashMap<usize, String>,
    is_editing: bool,
    selected_param: usize,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    width: f32,
    knob_size: KnobSize,
    slider_height: f32,
    theme: &Theme,
) -> impl IntoElement {
    let mut col = div().flex().flex_col().gap(d.gap).w(px(width)).flex_none();
    col = col.child(render_section_title(d, title, theme));
    for spec in controls {
        if spec.hidden {
            continue;
        }
        col = col.child(render_control(
            d,
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
            slider_height,
            theme,
        ));
    }
    col
}

/// Render the main (center) column with groups and tabs.
#[allow(clippy::too_many_arguments)]
fn render_main_column(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    layout: &'static PluginLayout,
    params: &[ParamSpec],
    values: &[f64],
    file_paths: &HashMap<usize, String>,
    solved: &SolvedLayout,
    main_width: f32,
    is_editing: bool,
    selected_param: usize,
    active_tab: usize,
    overflow_open: bool,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    layout_scale: f32,
    spider_snapshot: Option<&crate::components::plugins::spatial_spider::SpatialSpiderSnapshot>,
    text: Option<PluginCommonTranslations>,
    theme: &Theme,
    include_tabs: bool,
) -> impl IntoElement {
    let mut center = div()
        .flex()
        .flex_col()
        .gap(d.section)
        .w(px(main_width))
        .flex_none();

    // Detect a "mode selector" group: an untitled main group containing a single
    // ButtonSet bound to a Choice param whose labels alias other group titles.
    // When present, the button row is lifted above the parameter cards and the
    // aliased groups are filtered to only show the one matching the active mode.
    let mode = detect_mode_selector(layout, params);

    // Render control groups
    if !layout.main.is_empty() {
        // Top toolbar (centered) for the mode selector, when detected.
        if let Some(info) = mode.as_ref() {
            let value = values.get(info.param_idx).copied().unwrap_or(0.0);
            if let Some(param) = params.get(info.param_idx) {
                let toolbar = render_param_as_button_set(
                    d,
                    entity.clone(),
                    plugin_idx,
                    info.param_idx,
                    param,
                    value,
                    info.labels,
                    false,
                    is_editing,
                    selected_param,
                    true,
                    theme,
                );
                center = center.child(
                    div()
                        .w_full()
                        .flex()
                        .justify_center()
                        .mb(d.section)
                        .child(toolbar),
                );
            }
        }

        let (visible_groups, overflow_groups) = if include_tabs {
            solve_main_groups(layout, values, mode.as_ref(), main_width, layout_scale)
        } else {
            (
                mode_visible_groups(layout, values, mode.as_ref()),
                Vec::new(),
            )
        };

        let mut container = div()
            .flex()
            .gap(d.section)
            .items_start()
            .justify_center()
            .when(!include_tabs, |div| div.flex_wrap());
        for group in &visible_groups {
            container = container.child(render_group(
                d,
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
                spider_snapshot,
                text,
            ));
        }
        center = center.child(container);

        if !overflow_groups.is_empty() {
            let overflow_count: usize = overflow_groups
                .iter()
                .map(|group| visible_control_count(group))
                .sum();
            let mut overflow_content = div()
                .flex()
                .flex_col()
                .items_stretch()
                .gap(d.section)
                .p(d.card);
            for group in &overflow_groups {
                overflow_content = overflow_content.child(render_group(
                    d,
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
                    spider_snapshot,
                    text,
                ));
            }

            let overflow_entity = entity.clone();
            let more_label = format!(
                "{} ({overflow_count})",
                text.map_or("More", |translations| translations.more)
            );
            let trigger = Button::new(
                SharedString::from(format!("plugin-more-trigger-{plugin_idx}")),
                more_label,
            )
            .variant(ButtonVariant::Secondary)
            .size(ButtonSize::Sm);
            center = center.child(
                div().w_full().flex().justify_end().child(
                    AdaptiveOverflow::new(SharedString::from(format!("plugin-more-{plugin_idx}")))
                        .open(overflow_open)
                        .trigger(trigger)
                        .content(overflow_content)
                        .on_open_change(move |open, _window, cx| {
                            overflow_entity.update(cx, |state, _| {
                                state
                                    .app
                                    .plugin_ui
                                    .plugin_auto_overflow_open
                                    .insert(plugin_idx, open);
                            });
                        }),
                ),
            );
        }

        let all_tabs = if include_tabs {
            collect_all_tabs(layout, solved, &[])
        } else {
            Vec::new()
        };

        if !all_tabs.is_empty() {
            let clamped_tab = active_tab.min(all_tabs.len().saturating_sub(1));
            // Tab bar (underline style)
            let mut tab_bar = div()
                .flex()
                .justify_center()
                .border_b_1()
                .border_color(theme.border);
            for (i, tab) in all_tabs.iter().enumerate() {
                let is_active = i == clamped_tab;
                let tab_entity = entity.clone();
                let tab_plugin_idx = plugin_idx;
                let tab_idx = i;
                tab_bar = tab_bar.child(
                    div()
                        // Match the size used by Potentiometer titles (e.g.
                        // "LFE Gain") so the tab labels read as peer headings,
                        // not chart labels.
                        .text_size(d.text_sm)
                        .px(d.card)
                        // intentional: asymmetric 6px bottom / 4px top for tab underline spacing
                        .pb(rems(0.375))
                        .pt(d.grid)
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
                            Theme::with_opacity(theme.border, 0.0)
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
                                state
                                    .app
                                    .plugin_ui
                                    .plugin_auto_tab
                                    .insert(tab_plugin_idx, tab_idx);
                            });
                        })
                        .child(tab.name.to_string()),
                );
            }
            center = center.child(tab_bar);

            // Active tab content
            if let Some(tab) = all_tabs.get(clamped_tab) {
                let tab_div = render_layout_tab_content(
                    d,
                    entity.clone(),
                    plugin_idx,
                    tab.content,
                    layout,
                    params,
                    values,
                    file_paths,
                    is_editing,
                    selected_param,
                    solved,
                    plugin_data,
                    theme,
                    spider_snapshot,
                    text,
                );
                center = center.child(tab_div);
            }
        }
    }

    center
}

/// Render a control group (titled section with controls).
#[allow(clippy::too_many_arguments)]
fn render_group(
    d: &Ds,
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
    spider_snapshot: Option<&crate::components::plugins::spatial_spider::SpatialSpiderSnapshot>,
    text: Option<PluginCommonTranslations>,
) -> impl IntoElement {
    // Individual controls carry their own visual frames. The generated group
    // wrapper should size to content instead of drawing a large empty chassis.
    let has_sliders = group
        .controls
        .iter()
        .any(|c| matches!(c.control_type, ControlType::VerticalSlider));
    let stack_controls = solved.group_direction == Direction::Column;
    let compact_width = solved
        .column_width(ColumnRole::Main)
        .unwrap_or_else(|| control_column_width(solved.knob_size) * 2.0);

    let mut col = div().flex().flex_col().gap(d.gap).flex_none();
    if !group.title.is_empty() {
        col = col.child(render_section_title(d, group.title, theme));
    }

    if has_sliders {
        let mut slider_row = div()
            .flex()
            .gap(d.gap)
            .items_end()
            .when(stack_controls, |row| {
                row.max_w(px(compact_width)).flex_wrap()
            });
        for spec in group.controls {
            if spec.hidden {
                continue;
            }
            slider_row = slider_row.child(render_control(
                d,
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
                solved.slider_height,
                theme,
            ));
        }
        col = col.child(slider_row);
    } else {
        let visible_count = visible_control_count(group);
        let base_width = control_column_width(solved.knob_size);
        let use_two_columns = visible_count >= 4;
        let two_column_width = base_width * 2.0 + 12.0;
        let mut knob_row = div()
            .flex()
            .gap(d.gap)
            .when(!use_two_columns, |d| d.flex_col())
            .when(use_two_columns, |d| d.flex_wrap().w(px(two_column_width)));
        for spec in group.controls {
            if spec.hidden {
                continue;
            }
            knob_row = knob_row.child(render_control(
                d,
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
                solved.slider_height,
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
                        d,
                        params,
                        values,
                        plugin_data,
                        theme,
                    ));
                }
                VizSlot::Custom {
                    name,
                    position: VizPosition::BelowGroup(target),
                } if *target == group.title
                    && *name == sotf_plugins::plugin_layout::viz_names::SPATIAL_SPIDER =>
                {
                    if let (Some(snapshot), Some(text)) = (spider_snapshot, text) {
                        col = col.child(render_spatial_spider_viz(
                            d,
                            entity.clone(),
                            plugin_idx,
                            snapshot,
                            text,
                            theme,
                        ));
                    }
                }
                _ => {}
            }
        }
    }

    col
}

/// Render a single control based on its ControlType.
fn render_control(
    d: &Ds,
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
    slider_height: f32,
    theme: &Theme,
) -> AnyElement {
    let idx = spec.param_index;
    let interactive = spec.is_enabled(values);

    let control = match spec.control_type {
        ControlType::Knob => {
            if let Some(param) = params.get(idx) {
                let value = values.get(idx).copied().unwrap_or(0.0);
                render_param_as_knob(
                    entity,
                    plugin_idx,
                    idx,
                    param,
                    value,
                    is_editing,
                    selected_param,
                    pot_size(knob_size),
                    interactive,
                    theme,
                )
            } else {
                div().into_any_element()
            }
        }
        ControlType::KnobLarge => {
            if let Some(param) = params.get(idx) {
                let value = values.get(idx).copied().unwrap_or(0.0);
                render_param_as_knob(
                    entity,
                    plugin_idx,
                    idx,
                    param,
                    value,
                    is_editing,
                    selected_param,
                    pot_size_large(knob_size),
                    interactive,
                    theme,
                )
            } else {
                div().into_any_element()
            }
        }
        ControlType::VerticalSlider => {
            if let Some(param) = params.get(idx) {
                let value = values.get(idx).copied().unwrap_or(0.0);
                render_param_as_slider(
                    entity,
                    plugin_idx,
                    idx,
                    param,
                    value,
                    is_editing,
                    selected_param,
                    knob_size,
                    slider_height,
                    interactive,
                    theme,
                )
            } else {
                div().into_any_element()
            }
        }
        ControlType::Toggle => {
            if let Some(param) = params.get(idx) {
                let value = values.get(idx).copied().unwrap_or(0.0);
                render_param_as_toggle(
                    d,
                    entity,
                    plugin_idx,
                    idx,
                    param,
                    value,
                    is_editing,
                    selected_param,
                    knob_size,
                    interactive,
                    theme,
                )
            } else {
                div().into_any_element()
            }
        }
        ControlType::ButtonSet { labels } => {
            if let Some(param) = params.get(idx) {
                let value = values.get(idx).copied().unwrap_or(0.0);
                render_param_as_button_set(
                    d,
                    entity,
                    plugin_idx,
                    idx,
                    param,
                    value,
                    labels,
                    true,
                    is_editing,
                    selected_param,
                    interactive,
                    theme,
                )
            } else {
                div().into_any_element()
            }
        }
        ControlType::Selector => {
            if let Some(param) = params.get(idx) {
                let value = values.get(idx).copied().unwrap_or(0.0);
                render_param_as_selector(
                    d,
                    entity,
                    plugin_idx,
                    idx,
                    param,
                    value,
                    is_editing,
                    selected_param,
                    interactive,
                    theme,
                )
            } else {
                div().into_any_element()
            }
        }
        ControlType::BarMeter { min_db, max_db } => {
            render_bar_meter(d, plugin_data, min_db, max_db, theme)
        }
        ControlType::Label => {
            if let Some(param) = params.get(idx) {
                let value = values.get(idx).copied().unwrap_or(0.0);
                render_param_as_label(d, param, value, theme)
            } else {
                div().into_any_element()
            }
        }
        ControlType::FilePicker => {
            if let Some(param) = params.get(idx) {
                let path = file_paths.get(&idx).map(|s| s.as_str());
                render_file_picker(d, plugin_idx, idx, param, path, interactive, theme)
            } else {
                div().into_any_element()
            }
        }
    };
    let engine_key = params
        .get(idx)
        .map(|param| param.engine_key)
        .unwrap_or("meter");
    div()
        .id(SharedString::from(format!(
            "plugin-control-{plugin_idx}-{engine_key}"
        )))
        .child(control)
        .into_any_element()
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
    interactive: bool,
    theme: &Theme,
) -> AnyElement {
    match param.param_type {
        ParamType::Float { min, max, .. } => {
            let display_min = min * param.display_scale;
            let display_max = max * param.display_scale;
            let display_val = value * param.display_scale;
            render_knob_sized_enabled(
                entity,
                plugin_idx,
                param.name,
                display_val,
                display_min,
                display_max,
                param.unit,
                idx,
                selected_param,
                is_editing,
                None,
                size,
                interactive,
                theme,
            )
            .into_any_element()
        }
        ParamType::Int { min, max, .. } => render_knob_sized_enabled(
            entity,
            plugin_idx,
            param.name,
            value,
            min as f64,
            max as f64,
            param.unit,
            idx,
            selected_param,
            is_editing,
            None,
            size,
            interactive,
            theme,
        )
        .into_any_element(),
        _ => {
            // Bool/Choice as knob — fall back to the legacy inline toggle.
            render_param_as_inline_toggle(
                entity,
                plugin_idx,
                idx,
                param,
                value,
                is_editing,
                selected_param,
                interactive,
                theme,
            )
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
    slider_height: f32,
    interactive: bool,
    theme: &Theme,
) -> AnyElement {
    match param.param_type {
        ParamType::Float { min, max, .. } => {
            let display_min = min * param.display_scale;
            let display_max = max * param.display_scale;
            let display_val = value * param.display_scale;
            render_vertical_slider_with_ticks_enabled(
                entity,
                plugin_idx,
                param.name,
                display_val,
                display_min,
                display_max,
                param.unit,
                idx,
                selected_param,
                is_editing,
                None,
                slider_height,
                interactive,
                theme,
            )
            .into_any_element()
        }
        ParamType::Int { min, max, .. } => render_vertical_slider_with_ticks_enabled(
            entity,
            plugin_idx,
            param.name,
            value,
            min as f64,
            max as f64,
            param.unit,
            idx,
            selected_param,
            is_editing,
            None,
            slider_height,
            interactive,
            theme,
        )
        .into_any_element(),
        _ => render_param_as_knob(
            entity,
            plugin_idx,
            idx,
            param,
            value,
            is_editing,
            selected_param,
            pot_size(knob_size),
            interactive,
            theme,
        ),
    }
}

/// Render a param as a toggle (for Bool and Choice types).
fn render_param_as_toggle(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    value: f64,
    is_editing: bool,
    selected_param: usize,
    knob_size: KnobSize,
    interactive: bool,
    theme: &Theme,
) -> AnyElement {
    match param.param_type {
        ParamType::Bool {
            true_label,
            false_label,
            ..
        } => {
            let is_on = value > 0.5;
            let labels = [false_label, true_label];
            render_param_as_button_set(
                d,
                entity,
                plugin_idx,
                idx,
                param,
                if is_on { 1.0 } else { 0.0 },
                &labels,
                true,
                is_editing,
                selected_param,
                interactive,
                theme,
            )
            .into_any_element()
        }
        ParamType::Choice { labels, .. } => render_param_as_button_set(
            d,
            entity,
            plugin_idx,
            idx,
            param,
            value,
            labels,
            true,
            is_editing,
            selected_param,
            interactive,
            theme,
        )
        .into_any_element(),
        _ => {
            // Float/Int as toggle doesn't make sense, render as knob
            render_param_as_knob(
                entity,
                plugin_idx,
                idx,
                param,
                value,
                is_editing,
                selected_param,
                pot_size(knob_size),
                interactive,
                theme,
            )
        }
    }
}

/// Legacy inline fallback for unusual Bool/Choice controls rendered as knobs.
fn render_param_as_inline_toggle(
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    value: f64,
    is_editing: bool,
    selected_param: usize,
    interactive: bool,
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
            render_toggle_enabled(
                entity,
                plugin_idx,
                &label,
                is_on,
                interactive,
                idx,
                selected_param,
                is_editing,
                theme,
            )
            .into_any_element()
        }
        ParamType::Choice { labels, .. } => {
            let label = labels.get(value as usize).copied().unwrap_or("?");
            render_toggle_enabled(
                entity,
                plugin_idx,
                &format!("{}: {}", param.name, label),
                true,
                interactive,
                idx,
                selected_param,
                is_editing,
                theme,
            )
            .into_any_element()
        }
        _ => div().into_any_element(),
    }
}

/// Render a choice parameter as an explicit set of options.
///
/// Showing every choice avoids the opaque click-to-cycle interaction and lets
/// users compare modes and select the desired value directly.
fn render_param_as_selector(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    value: f64,
    is_editing: bool,
    selected_param: usize,
    interactive: bool,
    theme: &Theme,
) -> AnyElement {
    match param.param_type {
        ParamType::Choice { labels, .. } => render_param_as_button_set(
            d,
            entity,
            plugin_idx,
            idx,
            param,
            value,
            labels,
            true,
            is_editing,
            selected_param,
            interactive,
            theme,
        ),
        // Non-choice params: fall back to toggle
        _ => render_param_as_toggle(
            d,
            entity,
            plugin_idx,
            idx,
            param,
            value,
            is_editing,
            selected_param,
            KnobSize::Sm,
            interactive,
            theme,
        ),
    }
}

/// Render a param as a horizontal button set (mutually exclusive buttons).
fn render_param_as_button_set(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    value: f64,
    labels: &[&str],
    show_label: bool,
    is_editing: bool,
    selected_param: usize,
    interactive: bool,
    theme: &Theme,
) -> AnyElement {
    let current = value as usize;
    let is_sel = selected_param == idx && is_editing;

    let mut choices = div()
        .flex()
        .flex_wrap()
        .gap(d.grid)
        .justify_end()
        .items_center();

    for (i, label) in labels.iter().enumerate() {
        let is_active = i == current;
        let btn_entity = entity.clone();
        let btn_idx = idx;
        let btn_plugin_idx = plugin_idx;
        let btn_val = i;
        let choice = div()
            .text_size(d.text_sm)
            .min_w(rems(2.0))
            .min_h(rems(2.0))
            .flex()
            .items_center()
            .justify_center()
            .px(d.pad_y)
            .py(d.pad_y_half)
            .rounded(d.r_sm)
            .when(interactive, |el| el.cursor_pointer())
            .id(SharedString::from(format!(
                "btn-set-{plugin_idx}-{idx}-{i}"
            )))
            .when(is_active, |d| {
                d.bg(theme.accent).text_color(theme.text_on_accent)
            })
            .when(!is_active, |d| {
                d.bg(theme.background_secondary)
                    .text_color(theme.text_secondary)
            })
            .when(interactive, |el| {
                el.hover(|d| d.opacity(0.8))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        btn_entity.update(cx, |state, _| {
                            state
                                .app
                                .set_plugin_param(btn_plugin_idx, btn_idx, btn_val as f64);
                        });
                    })
            })
            .when(!interactive, |el| el.opacity(0.45))
            .child(label.to_string());
        choices = choices.child(choice);
    }

    if show_label {
        div()
            .flex()
            .flex_col()
            .items_stretch()
            .gap(d.grid)
            .min_w(rems(8.125))
            .max_w(rems(15.0))
            .flex_1()
            .rounded(d.r_md)
            .when(is_sel, |el| el.border_1().border_color(theme.accent))
            .child(
                div()
                    .text_size(d.text_sm)
                    .text_color(theme.text_muted)
                    .text_left()
                    .child(param.name.to_string()),
            )
            .child(choices)
            .into_any_element()
    } else {
        choices
            .rounded(d.r_md)
            .when(is_sel, |el| el.border_1().border_color(theme.accent))
            .into_any_element()
    }
}

/// Render a gain reduction meter.
fn render_bar_meter(
    d: &Ds,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    min_db: f64,
    _max_db: f64,
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
            } else if let Some(ed) = d.downcast_ref::<sotf_plugins::MultibandExpanderData>() {
                ed.attenuation_db
                    .iter()
                    .copied()
                    .reduce(f32::max)
                    .map(|v| v as f64)
            } else if let Some(dd) = d.downcast_ref::<sotf_plugins::DeEsserData>() {
                dd.gain_reduction_db
                    .iter()
                    .copied()
                    .reduce(f32::max)
                    .map(|v| v as f64)
            } else {
                None
            }
        })
        .unwrap_or(0.0);

    render_gr_meter(d, gr_db, min_db, theme).into_any_element()
}

/// Render a read-only label for a param value.
fn render_param_as_label(d: &Ds, param: &ParamSpec, value: f64, theme: &Theme) -> AnyElement {
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
        .px(d.pad_x)
        .py(d.pad_y)
        .rounded(d.r_lg)
        .bg(theme.background_secondary)
        .child(
            div()
                .text_size(d.text_sm)
                .text_color(theme.text_muted)
                .child(param.name),
        )
        .child(
            div()
                .text_size(d.text_sm)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_primary)
                .child(formatted),
        )
        .into_any_element()
}

/// Render a file picker with load button.
fn render_file_picker(
    d: &Ds,
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    file_path: Option<&str>,
    interactive: bool,
    theme: &Theme,
) -> AnyElement {
    let has_file = file_path.is_some_and(|p| !p.is_empty());
    let display_name = file_path
        .filter(|p| !p.is_empty())
        .and_then(|p| p.rsplit(['/', '\\']).next())
        .unwrap_or("None");
    let text_color = if has_file {
        theme.text_primary
    } else {
        theme.text_muted
    };
    let engine_key = param.engine_key;
    let param_name = param.name;

    div()
        .flex()
        .items_center()
        .justify_between()
        .px(d.pad_x)
        .py(d.pad_y)
        .rounded(d.r_lg)
        .bg(theme.background_secondary)
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(d.text_sm)
                        .text_color(theme.text_muted)
                        .child(param.name),
                )
                .child(
                    div()
                        .text_size(d.text_sm)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(text_color)
                        .overflow_hidden()
                        .text_ellipsis()
                        .max_w(rems(7.5))
                        .child(display_name.to_string()),
                ),
        )
        .child(
            div()
                .id(SharedString::from(format!(
                    "load-file-tooltip-{plugin_idx}-{idx}"
                )))
                .child(
                    IconButton::with_child(
                        SharedString::from(format!("load-file-btn-{plugin_idx}-{idx}")),
                        Icon::new(IconName::Folder)
                            .small()
                            .color(theme.text_secondary),
                    )
                    .variant(IconButtonVariant::Outline)
                    .size(IconButtonSize::Sm)
                    .theme(theme.to_icon_button_theme())
                    .aria_label(param_name)
                    .when(interactive, |button| {
                        button.on_click_event(move |_event, _window, cx| {
                            match file_picker_open_target(engine_key) {
                                Some(FilePickerOpenTarget::Sofa) => {
                                    cx.dispatch_action(&OpenSofaFile {
                                        plugin_idx,
                                        param_idx: idx,
                                    });
                                }
                                Some(FilePickerOpenTarget::Ir) => {
                                    cx.dispatch_action(&OpenIrFile {
                                        plugin_idx,
                                        param_idx: idx,
                                    });
                                }
                                Some(FilePickerOpenTarget::AbConfig(path_id)) => {
                                    cx.dispatch_action(&OpenAbConfigFile {
                                        plugin_idx,
                                        path_id: path_id.to_string(),
                                    });
                                }
                                None => {
                                    log::warn!(
                                        "No file open action for engine_key: {}",
                                        engine_key
                                    );
                                }
                            }
                        })
                    })
                    .when(!interactive, |button| button.disabled(true)),
                )
                .tooltip({
                    let theme = theme.clone();
                    move |_window, cx| themed_tooltip(param_name, &theme, cx)
                }),
        )
        .into_any_element()
}

/// Render a transfer curve visualization using param values from the layout.
///
/// Looks up threshold, ratio, and knee by engine_key. For limiters (no ratio param),
/// uses ratio=∞ and is_limiter=true for proper brickwall rendering.
fn render_transfer_curve_for_layout(
    d: &Ds,
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
            cd.band_levels_db
                .iter()
                .copied()
                .reduce(f32::max)
                .map(|level| level as f64)
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

    render_transfer_curve_with_level(
        d,
        threshold_db,
        ratio,
        knee_db,
        is_limiter,
        200.0,
        input_level_db,
        theme,
    )
    .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_layout_tab_content(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    content: LayoutTabContent,
    layout: &'static PluginLayout,
    params: &[ParamSpec],
    values: &[f64],
    file_paths: &HashMap<usize, String>,
    is_editing: bool,
    selected_param: usize,
    solved: &SolvedLayout,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    theme: &Theme,
    spider_snapshot: Option<&crate::components::plugins::spatial_spider::SpatialSpiderSnapshot>,
    text: Option<PluginCommonTranslations>,
) -> AnyElement {
    match content {
        LayoutTabContent::Controls(controls) => {
            let mut tab_div = div().flex().flex_wrap().justify_center().gap(d.section);
            for spec in controls {
                if spec.hidden {
                    continue;
                }
                tab_div = tab_div.child(render_control(
                    d,
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
                    solved.slider_height,
                    theme,
                ));
            }
            tab_div.into_any_element()
        }
        LayoutTabContent::Group(group) => div()
            .flex()
            .justify_center()
            .child(render_group(
                d,
                entity,
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
                spider_snapshot,
                text,
            ))
            .into_any_element(),
    }
}
