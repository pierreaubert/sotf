use super::super::actions::{OpenAbConfigFile, OpenHrtfDatabase, OpenIrFile, OpenSofaFile};
use super::super::common::{render_section_title, render_transfer_curve_with_level};
use super::super::level_meters::render_gr_meter;
use super::misc::extract_file_paths;
use super::mode_selector_info::detect_mode_selector;
use super::mode_selector_info::mode_visible_groups;
use super::mode_selector_info::solve_main_groups;
use super::types::LayoutTabContent;
use super::types::collect_all_tabs;
use crate::app::AppState;
#[cfg(feature = "dev-api")]
use crate::app::dev_api::{DevElementState, DevTrackExt};
use crate::app::i18n::PluginCommonTranslations;
use crate::components::design::Ds;
use crate::components::graphs::common::rgba_to_u32;
use crate::components::graphs::response_graphs::{ChartConfig, Series, render_line_chart};
use crate::components::icons::{Icon, IconName};
use crate::components::plugins::editing::PluginEditingManager;
use crate::components::plugins::theme::PluginTheme;
use crate::components::themed_tooltip;
use crate::plugin_file_picker::{FilePickerOpenTarget, file_picker_open_target};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_px::ScaleType;
use gpui_ui_kit::{
    Accordion, AccordionItem, AccordionMode, Button, ButtonSize, ButtonVariant, IconButton,
    IconButtonSize, IconButtonVariant, NumberInput, NumberInputSize, Select, SelectOption,
    SelectSize, Slider, SliderSize,
};
use sotf_audio_player::PluginSettings;
use sotf_plugins::layout_solver::{KnobSize, SolvedLayout, solve_layout_scaled};
use sotf_plugins::param_specs::{ParamCategory, ParamSpec, ParamType};
use sotf_plugins::plugin_layout::*;
use std::collections::HashMap;

#[derive(Clone, Copy, Default)]
struct ChoiceMenuRenderState {
    instance_id: Option<usize>,
    open_menu: Option<(usize, usize)>,
    highlighted_index: Option<usize>,
    compact_all_selectors: bool,
    selector_width: Option<f32>,
    relocate_globals: bool,
    hide_label: bool,
}

/// Momentary profile actions are not stored settings. Reflect the engine's
/// live capture state in their two-choice control so Off sends cancellation.
fn apply_live_profile_action_values(
    settings: &PluginSettings,
    values: &mut [f64],
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
) {
    match settings {
        PluginSettings::HissReducer { .. } => {
            if let Some(snapshot) = plugin_data.and_then(|data| {
                data.downcast_ref::<sotf_plugins::plugin_hiss_reducer::snapshot::ProfileSnapshot>()
            }) {
                if let Some(value) = values.get_mut(5) {
                    *value = f64::from(snapshot.capture_state().0 as u8);
                }
                if let Some(value) = values.get_mut(6)
                    && let Ok(status) = snapshot.try_status()
                {
                    *value = f64::from(status.use_flag as u8);
                }
            }
        }
        PluginSettings::Denoiser { .. } => {
            if let Some(data) =
                plugin_data.and_then(|data| data.downcast_ref::<sotf_plugins::DenoiserData>())
            {
                if let Some(value) = values.get_mut(20) {
                    *value = f64::from(data.is_learning_noise as u8);
                }
                if let Some(value) = values.get_mut(21) {
                    *value = f64::from(data.requested_use_profile as u8);
                }
            }
        }
        _ => {}
    }
}

fn solve_editor_layout(layout: &PluginLayout, available_width: f32, scale: f32) -> SolvedLayout {
    // Setup is disclosed separately; reserving invisible side columns would
    // make the primary controls stack even in a wide measured editor.
    let mut constraints: Vec<_> = layout
        .column_constraints
        .iter()
        .filter(|column| column.role == ColumnRole::Main)
        .copied()
        .collect();
    if constraints.is_empty() {
        constraints.push(ColumnConstraint::main(200.0));
    }
    solve_layout_scaled(&constraints, available_width, scale)
}

fn localized_parameter_specs(
    params: &[ParamSpec],
    text: PluginCommonTranslations,
) -> Vec<ParamSpec> {
    params
        .iter()
        .map(|param| {
            let mut localized = *param;
            localized.name = text.label(param.name);
            if let ParamType::Bool {
                true_label,
                false_label,
                ..
            } = &mut localized.param_type
            {
                *true_label = text.label(true_label);
                *false_label = text.label(false_label);
            }
            localized
        })
        .collect()
}

/// Render a plugin using its declarative layout.
///
/// This replaces individual `render_*_plugin()` functions. Call this for any
/// plugin that has a `PluginLayout` definition (i.e., `settings.layout().is_some()`).
#[allow(clippy::too_many_arguments)]
pub fn render_from_layout(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    plugin_instance_id: Option<usize>,
    choice_menu: Option<(usize, usize)>,
    choice_highlight: Option<usize>,
    settings: &PluginSettings,
    is_editing: bool,
    selected_param: usize,
    active_tab: usize,
    sections: Option<(usize, Vec<String>)>,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    available_width: f32,
    available_height: f32,
    layout_scale: f32,
    text: PluginCommonTranslations,
    theme: &Theme,
    plugin_theme: &PluginTheme,
    spider_snapshot: Option<crate::components::plugins::spatial_spider::SpatialSpiderSnapshot>,
) -> AnyElement {
    let choice_state = ChoiceMenuRenderState {
        instance_id: plugin_instance_id,
        open_menu: choice_menu,
        highlighted_index: choice_highlight,
        compact_all_selectors: true,
        selector_width: None,
        relocate_globals: generated_header_globals(settings),
        hide_label: false,
    };
    let Some(layout) = settings.layout() else {
        return Empty.into_any_element();
    };
    let localized_params = localized_parameter_specs(settings.param_specs(), text);
    let params = localized_params.as_slice();
    let mut values: Vec<f64> = (0..params.len())
        .map(|i| settings.param_value(i).unwrap_or(0.0))
        .collect();
    apply_live_profile_action_values(settings, &mut values, plugin_data);

    // Extract file paths for FilePath params
    let file_paths = extract_file_paths(params, settings);

    // Run the constraint solver
    let solved = solve_editor_layout(layout, available_width, layout_scale);

    // Overlay the chassis theme onto the global app theme so every helper
    // that takes `&Theme` (section title, knob, toggle, panel, ...) picks up
    // the chassis colors with no signature changes downstream. Semantic
    // colors (error / warning / meter palette) keep their global values.
    let chassis_theme = plugin_theme.apply_to(theme);

    if matches!(settings, PluginSettings::Crossover { .. }) && settings.param_value(4) == Some(1.0)
    {
        use sotf_plugins::param_specs::crossover::CHANNEL_PARAM_START;
        let mut channels = div().flex().flex_col().min_w_0().gap(d.grid);
        for index in sotf_audio_player::controllers::plugin::desktop_param_indices(settings)
            .into_iter()
            .filter(|index| {
                *index >= CHANNEL_PARAM_START && (*index - CHANNEL_PARAM_START).is_multiple_of(2)
            })
        {
            let Some(frequency_spec) =
                sotf_audio_player::controllers::plugin::desktop_param_spec(settings, index)
            else {
                continue;
            };
            let Some(mode_spec) =
                sotf_audio_player::controllers::plugin::desktop_param_spec(settings, index + 1)
            else {
                continue;
            };
            let frequency =
                sotf_audio_player::controllers::plugin::desktop_param_value(settings, index)
                    .unwrap_or(frequency_spec.default_f64());
            let mode =
                sotf_audio_player::controllers::plugin::desktop_param_value(settings, index + 1)
                    .unwrap_or(mode_spec.default_f64());
            let row = div()
                .flex()
                .flex_wrap()
                .min_w_0()
                .items_center()
                .gap(d.grid)
                .child(
                    div()
                        .flex_none()
                        .text_size(d.text_sm)
                        .text_color(chassis_theme.text_secondary)
                        .child(format!("Ch {}", (index - CHANNEL_PARAM_START) / 2 + 1)),
                )
                .child(
                    div()
                        .flex_1()
                        // intentional: Keep cutoff slider and exact field usable before the mode set wraps.
                        .min_w(px(280.0))
                        .child(render_horizontal_param(
                            d,
                            entity.clone(),
                            plugin_idx,
                            index,
                            frequency_spec,
                            frequency,
                            is_editing,
                            selected_param,
                            true,
                            &chassis_theme,
                        )),
                )
                .child(render_compact_choice_param(
                    d,
                    entity.clone(),
                    plugin_idx,
                    index + 1,
                    mode_spec,
                    mode,
                    &chassis_theme,
                ));
            #[cfg(feature = "dev-api")]
            let row = row.dev_track(format!(
                "plugin.crossover.channel.{plugin_idx}.{}",
                (index - CHANNEL_PARAM_START) / 2
            ));
            channels = channels.child(row);
        }
        if settings.param_value(0) == Some(1.0) {
            channels = channels.child(render_horizontal_param(
                d,
                entity.clone(),
                plugin_idx,
                3,
                &params[3],
                values[3],
                is_editing,
                selected_param,
                true,
                &chassis_theme,
            ));
        }
        return channels.into_any_element();
    }

    // Small utilities use value/slider rows instead of a bank of framed knobs.
    if params.len() <= 3
        && layout.main.len() == 1
        && layout.visualizations.is_empty()
        && layout.config.is_empty()
        && layout.output.is_empty()
        && layout.tabs.is_empty()
        && params
            .iter()
            .all(|param| matches!(param.param_type, ParamType::Float { .. }))
    {
        let mut rows = div().flex().flex_col().min_w_0().gap(d.gap);
        for spec in layout.main[0].controls.iter().filter(|spec| !spec.hidden) {
            let idx = spec.param_index;
            let Some(param) = params.get(idx) else {
                continue;
            };
            let interactive = spec.is_enabled(&values);
            let value = values[idx];
            let (min, max, step) = match param.param_type {
                ParamType::Float { min, max, step, .. } => (min, max, step),
                ParamType::Int { min, max, step, .. } => (min as f64, max as f64, step as f64),
                _ => continue,
            };
            let scale = param.display_scale;
            let entity_for_change = entity.clone();
            let entity_for_reset = entity.clone();
            let unity_control = if matches!(settings, PluginSettings::Gain { .. }) && idx == 0 {
                let entity = entity.clone();
                let button = Button::new(("gain-unity", plugin_idx), "0 dB")
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Secondary)
                    .theme(chassis_theme.to_button_theme())
                    .disabled(!interactive)
                    .selected(value == 0.0)
                    .aria_label(format!("{}: 0 dB", param.name))
                    .on_click_event(move |_, _, cx| {
                        entity.update(cx, |state, cx| {
                            state.app.reset_plugin_param(plugin_idx, idx);
                            cx.notify();
                        });
                    });
                #[cfg(feature = "dev-api")]
                let button = button.dev_track_with_state(
                    format!("plugin.gain.{plugin_idx}.unity"),
                    DevElementState::default()
                        .enabled(interactive)
                        .selected(value == 0.0),
                );
                Some(button)
            } else {
                None
            };
            let slider = Slider::new(("utility-slider", plugin_idx * 1000 + idx))
                .value((value * scale) as f32)
                .range((min * scale) as f32, (max * scale) as f32)
                .step((step * scale).abs() as f32)
                .logarithmic(min > 0.0 && matches!(param.unit, "Hz" | "ms"))
                .width((available_width - 160.0 * layout_scale).clamp(1.0, 480.0 * layout_scale))
                .size(SliderSize::Sm)
                .theme(chassis_theme.to_slider_theme())
                .disabled(!interactive)
                .aria_label(param.name)
                .on_drag_start({
                    let entity = entity.clone();
                    move |_, _, _, cx| {
                        entity.update(cx, |state, _| {
                            state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                            state.app.plugin_state.plugin_param_selection = idx;
                        });
                    }
                })
                .on_change(move |value, _, cx| {
                    entity_for_change.update(cx, |state, _| {
                        state
                            .app
                            .set_plugin_param(plugin_idx, idx, f64::from(value));
                    })
                })
                .on_reset(move |_, cx| {
                    entity_for_reset.update(cx, |state, _| {
                        state.app.reset_plugin_param(plugin_idx, idx);
                    })
                });
            #[cfg(feature = "dev-api")]
            let slider = slider.dev_track(format!("plugin.slider.{plugin_idx}.{idx}"));
            let row = div()
                .flex()
                .flex_wrap()
                .min_w_0()
                .items_center()
                .gap(d.gap)
                .children(render_param_as_number_input(
                    entity.clone(),
                    plugin_idx,
                    idx,
                    param,
                    value,
                    true,
                    idx,
                    interactive,
                    &chassis_theme,
                ))
                .child(slider)
                .children(unity_control);
            #[cfg(feature = "dev-api")]
            let row = row.dev_track(format!("plugin.layout.utility.row.{plugin_idx}.{idx}"));
            rows = rows.child(div().flex().justify_center().child(row));
        }
        #[cfg(feature = "dev-api")]
        let rows = rows.dev_track(format!("plugin.layout.utility.{plugin_idx}"));
        return rows.into_any_element();
    }

    let controls = render_solved_layout(
        d,
        entity,
        plugin_idx,
        choice_state,
        layout,
        params,
        &values,
        &file_paths,
        &solved,
        is_editing,
        selected_param,
        active_tab,
        sections,
        plugin_data,
        available_width,
        available_height,
        layout_scale,
        text,
        &chassis_theme,
        spider_snapshot.as_ref(),
    );
    if let PluginSettings::AmbisonicsDecoder { order, .. } = settings {
        let channels = order.saturating_add(1).saturating_pow(2);
        let status = gpui_ui_kit::Text::caption(format!("Input: ACN / SN3D · {channels} channels"))
            .color(chassis_theme.text_secondary);
        #[cfg(feature = "dev-api")]
        let status = status.dev_track(format!(
            "plugin.layout.ambisonics.input_status.{plugin_idx}"
        ));
        div()
            .flex()
            .flex_col()
            .min_w_0()
            .gap(d.gap)
            .child(status)
            .child(controls)
            .into_any_element()
    } else if let PluginSettings::Pnd {
        reference_frequency_hz,
        ..
    } = settings
    {
        let mode = if *reference_frequency_hz == 0.0 {
            "Change-only tracking: no fixed reference pitch."
        } else {
            "Fixed-reference pitch correction."
        };
        let status =
            gpui_ui_kit::Text::caption(text.label(mode)).color(chassis_theme.text_secondary);
        #[cfg(feature = "dev-api")]
        let status = status.dev_track_with_state(
            format!("plugin.layout.pnd.reference_status.{plugin_idx}"),
            DevElementState::default().text(text.label(mode)),
        );
        div()
            .flex()
            .flex_col()
            .min_w_0()
            .gap(d.gap)
            .child(status)
            .child(controls)
            .into_any_element()
    } else {
        controls
    }
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
    plugin_instance_id: Option<usize>,
    choice_menu: Option<(usize, usize)>,
    choice_highlight: Option<usize>,
    settings: &PluginSettings,
    is_editing: bool,
    selected_param: usize,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    available_width: f32,
    available_height: f32,
    layout_scale: f32,
    text: PluginCommonTranslations,
    theme: &Theme,
) -> AnyElement {
    let choice_state = ChoiceMenuRenderState {
        instance_id: plugin_instance_id,
        open_menu: choice_menu,
        highlighted_index: choice_highlight,
        compact_all_selectors: true,
        selector_width: None,
        relocate_globals: generated_header_globals(settings),
        hide_label: false,
    };
    let Some(layout) = settings.layout() else {
        return div().into_any_element();
    };
    let localized_params = localized_parameter_specs(settings.param_specs(), text);
    let params = localized_params.as_slice();
    let mut values: Vec<f64> = (0..params.len())
        .map(|i| settings.param_value(i).unwrap_or(0.0))
        .collect();
    apply_live_profile_action_values(settings, &mut values, plugin_data);
    let file_paths = extract_file_paths(params, settings);
    let solved = solve_editor_layout(layout, available_width, layout_scale);
    let main_width = available_width;

    render_main_column(
        d,
        entity,
        plugin_idx,
        choice_state,
        layout,
        params,
        &values,
        &file_paths,
        &solved,
        main_width,
        available_height,
        is_editing,
        selected_param,
        0,
        None,
        plugin_data,
        layout_scale,
        None,
        Some(text),
        theme,
        false,
    )
    .into_any_element()
}

/// Gear menus contain appearance controls only; DSP controls render in the editor.
#[allow(clippy::too_many_arguments)]
pub fn render_config_controls_from_layout(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    plugin_instance_id: Option<usize>,
    choice_menu: Option<(usize, usize)>,
    choice_highlight: Option<usize>,
    settings: &PluginSettings,
    is_editing: bool,
    selected_param: usize,
    available_width: f32,
    layout_scale: f32,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    text: PluginCommonTranslations,
    theme: &Theme,
    plugin_theme: &PluginTheme,
) -> Option<AnyElement> {
    let _ = (
        d,
        entity,
        plugin_idx,
        plugin_instance_id,
        choice_menu,
        choice_highlight,
        settings,
        is_editing,
        selected_param,
        available_width,
        layout_scale,
        plugin_data,
        text,
        theme,
        plugin_theme,
    );
    None
}

/// Configuration controls already owned by specialized editor surfaces.
fn custom_config_owned(settings: &PluginSettings) -> bool {
    matches!(
        super::super::custom_view_registry::plugin_type_key(settings),
        "eq" | "compressor"
            | "upmixer"
            | "dynamic_eq"
            | "linear_phase_eq"
            | "fir_designer"
            | "multiband_compressor"
            | "multiband_expander"
            | "xtc"
            | "channel_mute_solo"
            | "matrix"
            | "spectrum_analyzer"
    )
}

fn generated_header_globals(settings: &PluginSettings) -> bool {
    !matches!(
        super::super::custom_view_registry::plugin_type_key(settings),
        "eq" | "compressor"
            | "upmixer"
            | "dynamic_eq"
            | "linear_phase_eq"
            | "fir_designer"
            | "multiband_compressor"
            | "multiband_expander"
            | "matrix"
            | "channel_mute_solo"
            | "loudness_monitor"
            | "spectrum_analyzer"
            | "xtc"
    )
}

fn main_header_control(spec: &ControlSpec, param: &ParamSpec) -> bool {
    header_config_control(spec, param)
        && matches!(param.category, ParamCategory::Setup | ParamCategory::Output)
}

fn header_config_control(spec: &ControlSpec, param: &ParamSpec) -> bool {
    !spec.hidden
        && !param.engine_key.starts_with("sidechain")
        && !param.engine_key.contains("detection")
        && !matches!(param.engine_key, "learn_noise" | "clear_profile")
        && matches!(
            param.param_type,
            ParamType::Bool { .. } | ParamType::Choice { .. }
        )
}

/// Controls owned by the generated shell, shared with alternate editors.
fn editor_setup_controls(settings: &PluginSettings, header: bool) -> Vec<&'static ControlSpec> {
    let Some(layout) = settings.layout() else {
        return Vec::new();
    };
    let params = settings.param_specs();
    let extra_globals = layout
        .main
        .iter()
        .flat_map(|group| group.controls.iter())
        .chain(layout.output.iter())
        .filter(|spec| {
            header
                && generated_header_globals(settings)
                && params
                    .get(spec.param_index)
                    .is_some_and(|param| main_header_control(spec, param))
        });
    let mut seen = std::collections::HashSet::new();
    let controls: Vec<_> = layout
        .config
        .iter()
        .chain(extra_globals)
        .filter(|spec| {
            !spec.hidden
                && !custom_config_owned(settings)
                && params.get(spec.param_index).is_some_and(|param| {
                    header_config_control(spec, param) == header
                    // Empty-main layouts promote remaining configuration into the primary body.
                    && (header || !layout.main.is_empty())
                })
        })
        .filter(|spec| seen.insert(spec.param_index))
        .collect();
    controls
}

/// Desktop addresses already rendered by the shell header or Advanced section.
///
/// Dedicated EQ header operations have no numeric band address. Specialized headers
/// retain their established sparse desktop addresses; generated shells share their
/// actual control selection rather than recreating category heuristics in Table view.
pub(crate) fn shell_owned_param_indices(settings: &PluginSettings) -> Vec<usize> {
    let custom: &[usize] = match settings {
        PluginSettings::Compressor { .. } => &[7, 8, 14],
        PluginSettings::Upmixer { .. } => &[0],
        PluginSettings::DynamicEq { .. } => &[0, 6],
        PluginSettings::LinearPhaseEq { .. } => &[100, 101, 102, 103, 105],
        PluginSettings::MultibandCompressor { .. } => &[0, 1, 12, 14],
        PluginSettings::MultibandExpander { .. } => &[0, 1, 15],
        PluginSettings::SpectrumAnalyzer { .. } => &[4, 5],
        PluginSettings::ChannelMuteSolo { .. } => &[0],
        PluginSettings::XTC { .. } => &[13, 24],
        _ => &[],
    };
    let mut indices = custom.to_vec();
    for header in [true, false] {
        indices.extend(
            editor_setup_controls(settings, header)
                .iter()
                .map(|control| control.param_index),
        );
    }
    indices.sort_unstable();
    indices.dedup();
    indices
}

/// Render each generated configuration control in its single editor location.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_editor_setup(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    instance_id: Option<usize>,
    choice_menu: Option<(usize, usize)>,
    choice_highlight: Option<usize>,
    settings: &PluginSettings,
    text: PluginCommonTranslations,
    theme: &Theme,
    header: bool,
) -> Option<AnyElement> {
    settings.layout()?;
    let params = localized_parameter_specs(settings.param_specs(), text);
    let values: Vec<_> = (0..params.len())
        .map(|i| settings.param_value(i).unwrap_or(0.0))
        .collect();
    let files = extract_file_paths(&params, settings);
    let choice_state = ChoiceMenuRenderState {
        instance_id,
        open_menu: choice_menu,
        highlighted_index: choice_highlight,
        compact_all_selectors: true,
        selector_width: None,
        relocate_globals: generated_header_globals(settings),
        hide_label: false,
    };
    let controls = editor_setup_controls(settings, header);
    if controls.is_empty() {
        return None;
    }
    let mut row = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(d.grid)
        .min_w_0()
        .max_w_full()
        .when(header, |row| row.flex_1());
    for spec in controls {
        let control = render_control(
            text,
            d,
            entity.clone(),
            plugin_idx,
            choice_state,
            spec,
            &params,
            &values,
            &files,
            false,
            0,
            None,
            KnobSize::Xs,
            48.0,
            theme,
        );
        row = row.child(div().flex_none().child(control));
    }
    #[cfg(feature = "dev-api")]
    let row = row.dev_track(format!(
        "plugin.setup.{}.{}",
        plugin_idx,
        if header { "header" } else { "advanced" }
    ));
    Some(row.into_any_element())
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
    plugin_instance_id: Option<usize>,
    choice_menu: Option<(usize, usize)>,
    choice_highlight: Option<usize>,
    settings: &PluginSettings,
    is_editing: bool,
    selected_param: usize,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    available_width: f32,
    layout_scale: f32,
    text: PluginCommonTranslations,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    let choice_state = ChoiceMenuRenderState {
        instance_id: plugin_instance_id,
        open_menu: choice_menu,
        highlighted_index: choice_highlight,
        compact_all_selectors: true,
        selector_width: None,
        relocate_globals: generated_header_globals(settings),
        hide_label: false,
    };
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

    let tabs = collect_all_tabs(layout, &solved, &[]);
    let active_tab = entity
        .read(cx)
        .app
        .plugin_ui
        .plugin_auto_tab
        .get(&plugin_idx)
        .copied()
        .unwrap_or(0)
        .min(tabs.len().saturating_sub(1));
    let mut tab_bar = div()
        .flex()
        .flex_wrap()
        .justify_center()
        .border_b_1()
        .border_color(theme.border);
    for (i, tab) in tabs.iter().enumerate() {
        let tab_entity = entity.clone();
        let label = text.label(tab.name);
        let button = Button::new(
            SharedString::from(format!("layout-tab-{plugin_idx}-{i}")),
            label,
        )
        .size(ButtonSize::Sm)
        .variant(if i == active_tab {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Ghost
        })
        .selected(i == active_tab)
        .theme(theme.to_button_theme())
        .aria_label(label)
        .on_click(move |_, cx| {
            tab_entity.update(cx, |state, cx| {
                state.app.plugin_ui.plugin_auto_tab.insert(plugin_idx, i);
                cx.notify();
            });
        });
        #[cfg(feature = "dev-api")]
        let button = button.dev_track_with_state(
            format!("plugin.layout.tab.{plugin_idx}.{i}"),
            DevElementState::default().selected(i == active_tab),
        );
        tab_bar = tab_bar.child(button);
    }
    #[cfg(feature = "dev-api")]
    let tab_bar = tab_bar.dev_track(format!("plugin.layout.tabs.{plugin_idx}"));
    let mut container = div().flex().flex_col().gap(d.gap).child(tab_bar);
    if let Some(tab) = tabs.get(active_tab) {
        let content = render_layout_tab_content(
            d,
            entity,
            plugin_idx,
            choice_state,
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
            Some(text),
        );
        #[cfg(feature = "dev-api")]
        let content = content.dev_track(format!("plugin.layout.tab-content.{plugin_idx}"));
        container = container.child(content);
    }

    container.into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_solved_layout(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    choice_state: ChoiceMenuRenderState,
    layout: &'static PluginLayout,
    params: &[ParamSpec],
    values: &[f64],
    file_paths: &HashMap<usize, String>,
    solved: &SolvedLayout,
    is_editing: bool,
    selected_param: usize,
    active_tab: usize,
    sections: Option<(usize, Vec<String>)>,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    available_width: f32,
    available_height: f32,
    layout_scale: f32,
    text: PluginCommonTranslations,
    theme: &Theme,
    spider_snapshot: Option<&crate::components::plugins::spatial_spider::SpatialSpiderSnapshot>,
) -> AnyElement {
    let mut root = div()
        .flex()
        .flex_col()
        .gap(d.section)
        .w_full()
        .bg(theme.background);
    let mut status_height = 0.0;

    if let Some(snapshot) = plugin_data.and_then(|data| {
        data.downcast_ref::<sotf_plugins::plugin_hiss_reducer::snapshot::ProfileSnapshot>()
    }) {
        let (capturing, progress) = snapshot.capture_state();
        let status = if capturing {
            format!(
                "{} · {} {:.0}%",
                text.denoiser_profile,
                text.denoiser_learning,
                progress * 100.0
            )
        } else {
            match snapshot.try_status() {
                Ok(status) if status.present => {
                    format!(
                        "{} · {}",
                        text.denoiser_profile,
                        if status.use_flag {
                            text.denoiser_active
                        } else {
                            text.denoiser_captured
                        }
                    )
                }
                Ok(_) => format!("{} · {}", text.denoiser_profile, text.denoiser_none),
                Err(_) => format!("{} · …", text.denoiser_profile),
            }
        };
        let status_row = div()
            .text_size(d.text_sm)
            .text_color(theme.text_secondary)
            .child(status.clone());
        #[cfg(feature = "dev-api")]
        let status_row = status_row.dev_track_with_state(
            format!("plugin.hiss.profile_status.{plugin_idx}"),
            DevElementState::default().text(status),
        );
        root = root.child(status_row);
        status_height = 26.0 * layout_scale;
    }

    let main_width = available_width;

    // A block wrapper lets the width-constrained column report its wrapped height.
    // A single-child flex row retained its taller intrinsic estimate when controls wrapped.
    let row = div().w_full().child(render_main_column(
        d,
        entity.clone(),
        plugin_idx,
        choice_state,
        layout,
        params,
        values,
        file_paths,
        solved,
        main_width,
        (available_height - status_height).max(1.0),
        is_editing,
        selected_param,
        active_tab,
        sections,
        plugin_data,
        layout_scale,
        spider_snapshot,
        Some(text),
        theme,
        true,
    ));

    #[cfg(feature = "dev-api")]
    let row = {
        use crate::app::dev_api::DevTrackExt;
        row.dev_track(format!("plugin.layout.row.{plugin_idx}"))
    };
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
/// Render the main (center) column with groups and tabs.
#[allow(clippy::too_many_arguments)]
fn render_main_column(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    choice_state: ChoiceMenuRenderState,
    layout: &'static PluginLayout,
    params: &[ParamSpec],
    values: &[f64],
    file_paths: &HashMap<usize, String>,
    solved: &SolvedLayout,
    main_width: f32,
    available_height: f32,
    is_editing: bool,
    selected_param: usize,
    active_tab: usize,
    sections: Option<(usize, Vec<String>)>,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    layout_scale: f32,
    spider_snapshot: Option<&crate::components::plugins::spatial_spider::SpatialSpiderSnapshot>,
    text: Option<PluginCommonTranslations>,
    theme: &Theme,
    include_tabs: bool,
) -> impl IntoElement {
    let text_or_default = text.unwrap_or_else(|| {
        PluginCommonTranslations::for_language(crate::app::i18n::Language::English)
    });
    let mut center = div()
        .flex()
        .flex_col()
        .gap(if available_height < 420.0 {
            d.grid
        } else {
            d.section
        })
        .w(px(main_width))
        .flex_none();

    // Detect a "mode selector" group: an untitled main group containing a single
    // ButtonSet bound to a Choice param whose labels alias other group titles.
    // When present, the button row is lifted above the parameter cards and the
    // aliased groups are filtered to only show the one matching the active mode.
    let mode = detect_mode_selector(layout, params);

    // A few plugins intentionally keep their primary controls in `config`,
    // `output`, or tabs (Convolution is the important example). The normal
    // rack surface only used to render `main`, leaving those plugins as a
    // blank chassis until the gear popover was opened. Keep the declarative
    // layout useful on the primary surface by promoting the remaining
    // controls when no main groups exist.
    if layout.main.is_empty() {
        let mut fallback = div()
            .flex()
            .flex_wrap()
            .justify_center()
            .items_start()
            .gap(d.section);

        let fallback_controls = layout
            .config
            .iter()
            .chain(layout.output.iter())
            .filter(|spec| !spec.hidden)
            .count();
        let fallback_tabs = collect_all_tabs(layout, solved, &[]);

        for spec in layout.config.iter().chain(layout.output.iter()) {
            if (choice_state.relocate_globals
                && params
                    .get(spec.param_index)
                    .is_some_and(|param| main_header_control(spec, param)))
                || spec.hidden
                || (layout
                    .config
                    .iter()
                    .any(|config| std::ptr::eq(config, spec))
                    && params
                        .get(spec.param_index)
                        .is_some_and(|param| header_config_control(spec, param)))
            {
                continue;
            }
            fallback = fallback.child(render_control(
                text_or_default,
                d,
                entity.clone(),
                plugin_idx,
                choice_state,
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

        for tab in &fallback_tabs {
            fallback = fallback.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.gap)
                    .child(render_section_title(
                        d,
                        text.map_or(tab.name, |translations| translations.label(tab.name)),
                        theme,
                    ))
                    .child(render_layout_tab_content(
                        d,
                        entity.clone(),
                        plugin_idx,
                        choice_state,
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
                    )),
            );
        }

        if fallback_controls == 0 && fallback_tabs.is_empty() {
            fallback = fallback.child(
                div()
                    .w_full()
                    .flex()
                    .justify_center()
                    .py(d.card)
                    .text_size(d.text_sm)
                    .text_color(theme.text_muted)
                    .child(text.map_or("No configurable controls", |t| t.no_controls)),
            );
        }

        center = center.child(fallback);
    } else {
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
                    false,
                    is_editing,
                    selected_param,
                    true,
                    theme,
                    text.unwrap_or_else(|| {
                        PluginCommonTranslations::for_language(crate::app::i18n::Language::English)
                    }),
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

        let (mut visible_groups, overflow_groups) = if include_tabs {
            solve_main_groups(layout, values, mode.as_ref(), main_width, layout_scale)
        } else {
            (
                mode_visible_groups(layout, values, mode.as_ref()),
                Vec::new(),
            )
        };
        if available_height < 420.0 {
            // Short racks lead with the schema's most important visible
            // controls; equal priorities retain declaration order.
            visible_groups.sort_by(|a, b| {
                b.layout
                    .collapse_priority
                    .total_cmp(&a.layout.collapse_priority)
            });
        }

        // Analog EQ exposes four independent filter bands. At rack widths the
        // whole four-band stack pushes every selected-band control below the
        // viewport, so give the user an explicit band strip and one editable
        // band inspector. Wide editors retain the complete declaration.
        let analog_bands = ["LOW", "LOW MID", "HIGH MID", "HIGH"];
        let analog_primary_indices: Vec<usize> = params
            .iter()
            .enumerate()
            .filter(|(_, param)| matches!(param.engine_key, "analog_color" | "analog_trim"))
            .map(|(index, _)| index)
            .collect();
        let analog_compact = (main_width < 700.0 || available_height < 420.0)
            && analog_bands
                .iter()
                .all(|id| layout.main.iter().any(|group| group.id == *id));
        if analog_compact {
            let (instance_id, expanded) = sections.clone().unwrap_or((plugin_idx, Vec::new()));
            let selected_band = expanded
                .iter()
                .find_map(|id| id.strip_prefix("analog-band:"))
                .filter(|id| analog_bands.contains(id))
                .unwrap_or("LOW");
            use gpui_ui_kit::{ButtonSet, ButtonSetOption, ButtonSetSize};
            let band_entity = entity.clone();
            let strip = ButtonSet::new(("analog-band-strip", plugin_idx))
                .options(
                    analog_bands
                        .iter()
                        .map(|band| ButtonSetOption::new(*band, *band))
                        .collect(),
                )
                .selected(selected_band)
                .size(ButtonSetSize::Xs)
                .theme(theme.to_button_set_theme())
                .on_change(move |band, _, cx| {
                    if !analog_bands.contains(&band.as_ref()) {
                        return;
                    }
                    band_entity.update(cx, |state, cx| {
                        let expanded = state
                            .app
                            .plugin_state
                            .plugin_ui_state
                            .plugin_sections
                            .entry(instance_id)
                            .or_default();
                        expanded.retain(|id| !id.starts_with("analog-band:"));
                        expanded.push(format!("analog-band:{band}"));
                        cx.notify();
                    });
                });
            #[cfg(feature = "dev-api")]
            let selected_band_index = analog_bands.iter().position(|band| *band == selected_band);
            #[cfg(feature = "dev-api")]
            let strip = strip.option_wrapper(move |index, button| {
                let band = analog_bands[index];
                button
                    .dev_track_with_state(
                        format!("analog.band.{plugin_idx}.{}", band.replace(' ', "-")),
                        DevElementState::default().selected(Some(index) == selected_band_index),
                    )
                    .into_any_element()
            });
            center = center.child(strip);
            let mut primary = div()
                .flex()
                .flex_wrap()
                .items_start()
                .justify_center()
                .gap(d.half_grid);
            if let Some(band) = layout.main.iter().find(|group| group.id == selected_band) {
                for spec in band.controls.iter().filter(|spec| !spec.hidden) {
                    primary = primary.child(render_control(
                        text_or_default,
                        d,
                        entity.clone(),
                        plugin_idx,
                        choice_state,
                        spec,
                        params,
                        values,
                        file_paths,
                        is_editing,
                        selected_param,
                        plugin_data,
                        KnobSize::Xs,
                        64.0,
                        theme,
                    ));
                }
            }
            if let Some(analog) = layout.main.iter().find(|group| group.id == "ANALOG") {
                for spec in analog.controls.iter().filter(|spec| {
                    !spec.hidden && analog_primary_indices.contains(&spec.param_index)
                }) {
                    primary = primary.child(render_control(
                        text_or_default,
                        d,
                        entity.clone(),
                        plugin_idx,
                        choice_state,
                        spec,
                        params,
                        values,
                        file_paths,
                        is_editing,
                        selected_param,
                        plugin_data,
                        KnobSize::Xs,
                        64.0,
                        theme,
                    ));
                }
            }
            center = center.child(primary);
            visible_groups.retain(|group| !analog_bands.contains(&group.id));
        }

        let gate_compact = available_height < 420.0
            && layout.main.iter().any(|group| group.id == "TIMING")
            && params
                .get(15)
                .is_some_and(|param| param.engine_key == "mode")
            && layout.main.iter().any(|group| group.id == "DYNAMICS");
        if gate_compact {
            let mut toolbar = div().flex().items_center().justify_between().gap(d.grid);
            if !choice_state.relocate_globals
                && let Some(param) = params.get(15)
                && let ParamType::Choice { labels, .. } = param.param_type
            {
                toolbar = toolbar.child(render_param_as_button_set(
                    d,
                    entity.clone(),
                    plugin_idx,
                    15,
                    param,
                    values.get(15).copied().unwrap_or(0.0),
                    labels,
                    false,
                    true,
                    is_editing,
                    selected_param,
                    true,
                    theme,
                    text.unwrap_or_else(|| {
                        PluginCommonTranslations::for_language(crate::app::i18n::Language::English)
                    }),
                ));
            }
            let reduction = current_gain_reduction_db(plugin_data);
            let gr = div()
                .flex()
                .items_center()
                .gap(d.grid)
                .child(gpui_ui_kit::Text::caption("GR"))
                .child(gpui_ui_kit::Text::label(format!("{reduction:.1} dB")));
            #[cfg(feature = "dev-api")]
            let gr = {
                use crate::app::dev_api::DevTrackExt;
                gr.dev_track(format!("plugin.control.{plugin_idx}.{}", usize::MAX))
            };
            center = center.child(toolbar.child(gr));
            let mut primary = div()
                .flex()
                .flex_wrap()
                .items_start()
                .justify_center()
                .gap(d.half_grid);
            for idx in [0, 11, 5, 2, 4] {
                if let Some(spec) = layout
                    .main
                    .iter()
                    .flat_map(|group| group.controls.iter())
                    .find(|spec| spec.param_index == idx && !spec.hidden)
                {
                    primary = primary.child(render_control(
                        text_or_default,
                        d,
                        entity.clone(),
                        plugin_idx,
                        choice_state,
                        spec,
                        params,
                        values,
                        file_paths,
                        is_editing,
                        selected_param,
                        plugin_data,
                        KnobSize::Xs,
                        64.0,
                        theme,
                    ));
                }
            }
            center = center.child(primary);
            visible_groups.retain(|group| group.id != "DYNAMICS");
        }

        // Crossfeed's common listening controls must remain above algorithm details.
        let crossfeed_primary = params
            .iter()
            .any(|param| param.engine_key == "bauer_fcut_hz");
        let denoiser_primary = params
            .iter()
            .any(|param| param.engine_key == "mcra_alpha_s");
        let primary_indices: &[usize] = if crossfeed_primary {
            &[3, 12, 17] // Mix, ITD, Head Yaw: frozen Crossfeed desktop addresses.
        } else if denoiser_primary {
            &[0, 1, 3, 4] // Reduction, Floor, Attack, Release: canonical Denoiser addresses.
        } else {
            &[]
        };
        if !primary_indices.is_empty() {
            let mut primary = div()
                .flex()
                .flex_wrap()
                .justify_center()
                .gap(d.grid)
                .w_full();
            for &index in primary_indices {
                if let Some(spec) = layout
                    .main
                    .iter()
                    .flat_map(|group| group.controls.iter())
                    .find(|spec| spec.param_index == index && !spec.hidden)
                {
                    primary = primary.child(render_control(
                        text_or_default,
                        d,
                        entity.clone(),
                        plugin_idx,
                        choice_state,
                        spec,
                        params,
                        values,
                        file_paths,
                        is_editing,
                        selected_param,
                        plugin_data,
                        KnobSize::Xs,
                        solved.slider_height,
                        theme,
                    ));
                }
            }
            center = center.child(primary);
        }

        let output_meter_compact = available_height < 420.0
            && layout.output.iter().any(|spec| {
                !spec.hidden && matches!(spec.control_type, ControlType::BarMeter { .. })
            });
        if output_meter_compact && !gate_compact {
            let reduction = current_gain_reduction_db(plugin_data);
            let magnitude = if reduction.is_finite() {
                reduction.abs()
            } else {
                0.0
            };
            let max_reduction = layout
                .output
                .iter()
                .find_map(|spec| match spec.control_type {
                    ControlType::BarMeter { min_db, .. } => Some(min_db),
                    _ => None,
                })
                .unwrap_or(-30.0);
            let ticks = gpui_audio_kit::TickConfig::gain_reduction(max_reduction);
            let ratio = ticks.value_to_position(magnitude).clamp(0.0, 1.0);
            let meter_theme = super::super::theme::MeterTheme::from_theme(theme, layout_scale);
            let meter_theme =
                meter_theme.to_horizontal_meter_theme(d.text_xs, px(meter_theme.horizontal_gap));
            let color = if magnitude < 3.0 {
                theme.feedback.meter_normal
            } else if magnitude < 10.0 {
                theme.feedback.meter_warning
            } else {
                theme.feedback.meter_clip
            };
            let gr = div()
                .w_full()
                .child(gpui_audio_kit::render_horizontal_meter_bar_with(
                    "GR",
                    ratio,
                    color,
                    format!("-{magnitude:.1} dB"),
                    meter_theme,
                ));
            #[cfg(feature = "dev-api")]
            let gr = {
                use crate::app::dev_api::DevTrackExt;
                gr.dev_track(format!("plugin.control.{plugin_idx}.{}", usize::MAX))
            };
            center = center.child(gr);
        }

        let mut container = div()
            .flex()
            .gap(if available_height < 420.0 {
                d.grid
            } else {
                d.section
            })
            .items_start()
            .justify_center()
            .flex_wrap();
        // A single primary group needs no second title below the plugin header.
        // Count only groups with body controls after header-owned globals move out.
        visible_groups.retain(|group| {
            group.controls.iter().any(|spec| {
                !spec.hidden
                    && !primary_indices.contains(&spec.param_index)
                    && !(choice_state.relocate_globals
                        && params
                            .get(spec.param_index)
                            .is_some_and(|param| main_header_control(spec, param)))
            })
        });
        let show_group_headings = visible_groups.len() > 1;
        for group in &visible_groups {
            container = container.child(render_group(
                d,
                entity.clone(),
                plugin_idx,
                choice_state,
                group,
                layout,
                params,
                values,
                file_paths,
                is_editing,
                selected_param,
                solved,
                main_width,
                available_height,
                if !primary_indices.is_empty() {
                    primary_indices
                } else if analog_compact && group.id == "ANALOG" {
                    &analog_primary_indices
                } else if gate_compact && group.id == "TIMING" {
                    &[2, 4]
                } else {
                    &[]
                },
                plugin_data,
                theme,
                spider_snapshot,
                text,
                show_group_headings && (!analog_compact || !analog_bands.contains(&group.id)),
            ));
        }
        #[cfg(feature = "dev-api")]
        let container = {
            use crate::app::dev_api::DevTrackExt;
            container.dev_track(format!("plugin.layout.groups.{plugin_idx}"))
        };
        center = center.child(container);

        // Output values and meters stay on the primary surface. Compact
        // global choices have their sole owner in the editor header.
        if layout.output.iter().any(|spec| {
            !(choice_state.relocate_globals
                && params
                    .get(spec.param_index)
                    .is_some_and(|param| main_header_control(spec, param)))
                && !spec.hidden
                && !(output_meter_compact
                    && matches!(spec.control_type, ControlType::BarMeter { .. }))
        }) {
            let mut meter_row = div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_center()
                .gap(d.gap)
                .child(render_section_title(
                    d,
                    text.map_or("OUTPUT", |translations| translations.label("OUTPUT")),
                    theme,
                ));
            for spec in layout.output.iter().filter(|spec| {
                !(choice_state.relocate_globals
                    && params
                        .get(spec.param_index)
                        .is_some_and(|param| main_header_control(spec, param)))
                    && !spec.hidden
                    && !(output_meter_compact
                        && matches!(spec.control_type, ControlType::BarMeter { .. }))
            }) {
                meter_row = meter_row.child(render_control(
                    text_or_default,
                    d,
                    entity.clone(),
                    plugin_idx,
                    choice_state,
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
            center = center.child(meter_row);
        }

        if solved.show_visualizations && available_height < 420.0 {
            for viz in layout.visualizations {
                if matches!(
                    viz,
                    VizSlot::TransferCurve {
                        position: VizPosition::BelowGroup(_)
                    }
                ) {
                    center = center.child(render_transfer_curve_for_layout(
                        d,
                        params,
                        values,
                        plugin_data,
                        main_width.min(300.0),
                        theme,
                        text,
                    ));
                }
            }
        }

        if !overflow_groups.is_empty() {
            let items = overflow_groups
                .iter()
                .map(|group| {
                    let title =
                        text.map_or(group.title, |translations| translations.label(group.title));
                    AccordionItem::new(group.id, title).content(render_group(
                        d,
                        entity.clone(),
                        plugin_idx,
                        choice_state,
                        group,
                        layout,
                        params,
                        values,
                        file_paths,
                        is_editing,
                        selected_param,
                        solved,
                        main_width,
                        available_height,
                        if gate_compact && group.id == "TIMING" {
                            &[2, 4]
                        } else {
                            &[]
                        },
                        plugin_data,
                        theme,
                        spider_snapshot,
                        text,
                        false,
                    ))
                })
                .collect();
            let (instance_id, expanded) = sections.unwrap_or((plugin_idx, Vec::new()));
            let section_entity = entity.clone();
            let accordion = Accordion::new()
                .items(items)
                .mode(AccordionMode::Multiple)
                .expanded(expanded.into_iter().map(SharedString::from).collect())
                .bordered(false)
                .rounded(false)
                .theme(theme.to_accordion_theme())
                .aria_label(text.map_or("Advanced", |translations| translations.configuration));
            #[cfg(feature = "dev-api")]
            let accordion = accordion.header_wrapper(move |id, header| {
                header
                    .dev_track(format!("plugin.layout.header.{plugin_idx}.{id}"))
                    .into_any_element()
            });
            center = center.child(
                div()
                    .w(px(main_width))
                    .flex_none()
                    .child(accordion.on_change(move |id, open, _window, cx| {
                        section_entity.update(cx, |state, cx| {
                            let expanded = state
                                .app
                                .plugin_state
                                .plugin_ui_state
                                .plugin_sections
                                .entry(instance_id)
                                .or_default();
                            expanded.retain(|value| value != id.as_ref());
                            if open {
                                expanded.push(id.to_string());
                            }
                            cx.notify();
                        });
                    })),
            );
        }

        let all_tabs = if include_tabs {
            collect_all_tabs(layout, solved, &[])
        } else {
            Vec::new()
        };

        if !all_tabs.is_empty() {
            let clamped_tab = active_tab.min(all_tabs.len().saturating_sub(1));
            let mut tab_bar = div()
                .flex()
                .flex_wrap()
                .justify_center()
                .border_b_1()
                .border_color(theme.border);
            for (i, tab) in all_tabs.iter().enumerate() {
                let is_active = i == clamped_tab;
                let tab_entity = entity.clone();
                let label = text.map_or(tab.name, |translations| translations.label(tab.name));
                let tab_button = Button::new(
                    SharedString::from(format!("layout-tab-{plugin_idx}-{i}")),
                    label.to_string(),
                )
                .size(ButtonSize::Sm)
                .variant(if is_active {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Ghost
                })
                .selected(is_active)
                .theme(theme.to_button_theme())
                .aria_label(label.to_string())
                .on_click(move |_, cx| {
                    tab_entity.update(cx, |state, cx| {
                        state.app.plugin_ui.plugin_auto_tab.insert(plugin_idx, i);
                        cx.notify();
                    });
                });
                #[cfg(feature = "dev-api")]
                let tab_button = tab_button.dev_track_with_state(
                    format!("plugin.layout.tab.{plugin_idx}.{i}"),
                    DevElementState::default().selected(is_active),
                );
                tab_bar = tab_bar.child(tab_button);
            }
            #[cfg(feature = "dev-api")]
            let tab_bar = {
                use crate::app::dev_api::DevTrackExt;
                tab_bar.dev_track(format!("plugin.layout.tabs.{plugin_idx}"))
            };
            center = center.child(tab_bar);

            // Active tab content
            if let Some(tab) = all_tabs.get(clamped_tab) {
                let tab_div = render_layout_tab_content(
                    d,
                    entity.clone(),
                    plugin_idx,
                    choice_state,
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
                #[cfg(feature = "dev-api")]
                let tab_div = {
                    use crate::app::dev_api::DevTrackExt;
                    tab_div.dev_track(format!("plugin.layout.tab-content.{plugin_idx}"))
                };
                center = center.child(tab_div);
            }
        }
    }

    #[cfg(feature = "dev-api")]
    let center = {
        use crate::app::dev_api::DevTrackExt;
        center.dev_track(format!("plugin.layout.main.{plugin_idx}"))
    };
    center
}

/// Render a control group (titled section with controls).
#[allow(clippy::too_many_arguments)]
fn render_group(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    choice_state: ChoiceMenuRenderState,
    group: &ControlGroup,
    layout: &'static PluginLayout,
    params: &[ParamSpec],
    values: &[f64],
    file_paths: &HashMap<usize, String>,
    is_editing: bool,
    selected_param: usize,
    solved: &SolvedLayout,
    available_width: f32,
    available_height: f32,
    skip_indices: &[usize],
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    theme: &Theme,
    spider_snapshot: Option<&crate::components::plugins::spatial_spider::SpatialSpiderSnapshot>,
    text: Option<PluginCommonTranslations>,
    show_heading: bool,
) -> impl IntoElement {
    let text_or_default = text.unwrap_or_else(|| {
        PluginCommonTranslations::for_language(crate::app::i18n::Language::English)
    });
    // Numeric controls share one dense grammar at every viewport width.
    // Each group wraps as a unit; rows keep the label, slider and exact value together.
    let mut col = div()
        .flex()
        .flex_col()
        .gap(if available_height < 420.0 {
            d.half_grid
        } else {
            d.grid
        })
        .w(rems(23.0))
        .min_w_0()
        .max_w_full();
    if show_heading && !group.title.is_empty() {
        col = col.child(render_section_title(
            d,
            text.map_or(group.title, |translations| translations.label(group.title)),
            theme,
        ));
    }
    // Profile actions and their flags belong together, with natural wrapping
    // instead of full-width action buttons on separate rows.
    let profile_actions = group.controls.iter().any(|spec| {
        params
            .get(spec.param_index)
            .is_some_and(|param| param.engine_key == "learn_noise")
    }) && group.controls.iter().all(|spec| {
        params
            .get(spec.param_index)
            .is_some_and(|param| matches!(param.param_type, ParamType::Bool { .. }))
    });
    let mut profile_row = div().flex().flex_wrap().items_center().gap(d.grid);
    // DeEsser's eight primary rows otherwise hide Mix below a short rack.
    // Use the rendered center width, not the solver's preferred group width.
    // Two compact 280px lanes fit at 560px; narrower editors keep scrolling.
    let deesser_columns = available_height < 420.0
        && available_width >= 560.0
        && group.id == "dynamics"
        && params
            .get(11)
            .is_some_and(|param| param.engine_key == "split_topology");
    if deesser_columns {
        let mut lanes = div().flex().w_full().gap(d.grid).items_start();
        for controls in group.controls.chunks(4) {
            let mut lane = div().flex().flex_col().flex_1().min_w_0().gap(d.half_grid);
            for spec in controls {
                let idx = spec.param_index;
                if spec.hidden || skip_indices.contains(&idx) {
                    continue;
                }
                if let Some(param) = params.get(idx) {
                    lane = lane.child(render_narrow_horizontal_param(
                        d,
                        entity.clone(),
                        plugin_idx,
                        idx,
                        param,
                        values.get(idx).copied().unwrap_or(0.0),
                        is_editing,
                        selected_param,
                        spec.is_enabled(values),
                        theme,
                    ));
                }
            }
            lanes = lanes.child(lane);
        }
        col = col.w_full().child(lanes);
    }
    for spec in group.controls {
        if deesser_columns {
            break;
        }
        if spec.hidden
            || skip_indices.contains(&spec.param_index)
            || (choice_state.relocate_globals
                && params
                    .get(spec.param_index)
                    .is_some_and(|param| main_header_control(spec, param)))
        {
            continue;
        }
        let control = render_control(
            text_or_default,
            d,
            entity.clone(),
            plugin_idx,
            choice_state,
            spec,
            params,
            values,
            file_paths,
            is_editing,
            selected_param,
            plugin_data,
            KnobSize::Xs,
            solved.slider_height,
            theme,
        );
        if profile_actions {
            profile_row = profile_row.child(control);
        } else {
            col = col.child(control);
        }
    }
    if profile_actions {
        col = col.child(profile_row);
    }

    let mut side_curve = None;
    // A single primary group can place its plot beside controls on wide screens.
    let wide_curve = !show_heading && solved.column_width(ColumnRole::Main).unwrap_or(0.0) >= 900.0;
    // Render visualization below this group if specified
    if solved.show_visualizations && available_height >= 420.0 && !group.title.is_empty() {
        for viz in layout.visualizations {
            match viz {
                VizSlot::TransferCurve {
                    position: VizPosition::BelowGroup(target),
                } if *target == group.title => {
                    let curve_width = solved
                        .column_width(ColumnRole::Main)
                        .unwrap_or(200.0)
                        .clamp(200.0, 300.0);
                    let curve = render_transfer_curve_for_layout(
                        d,
                        params,
                        values,
                        plugin_data,
                        if wide_curve {
                            curve_width * 1.5
                        } else {
                            curve_width
                        },
                        theme,
                        text,
                    );
                    if wide_curve {
                        side_curve = Some(curve);
                    } else {
                        col = col.child(curve);
                    }
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

    let col = if let Some(curve) = side_curve {
        div()
            .flex()
            .items_start()
            .gap(d.section)
            .max_w_full()
            .child(col)
            .child(curve)
    } else {
        col
    };
    #[cfg(feature = "dev-api")]
    let col = {
        use crate::app::dev_api::DevTrackExt;
        col.dev_track(format!("plugin.layout.group.{plugin_idx}.{}", group.id))
    };
    col
}

/// Render a single control based on its ControlType.
fn render_control(
    text: PluginCommonTranslations,
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    choice_state: ChoiceMenuRenderState,
    spec: &ControlSpec,
    params: &[ParamSpec],
    values: &[f64],
    file_paths: &HashMap<usize, String>,
    is_editing: bool,
    selected_param: usize,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    knob_size: KnobSize,
    _slider_height: f32,
    theme: &Theme,
) -> AnyElement {
    let idx = spec.param_index;
    let interactive = spec.is_enabled(values);

    let control = match spec.control_type {
        ControlType::Knob | ControlType::KnobLarge | ControlType::VerticalSlider => {
            if let Some(param) = params.get(idx) {
                render_horizontal_param(
                    d,
                    entity,
                    plugin_idx,
                    idx,
                    param,
                    values.get(idx).copied().unwrap_or(0.0),
                    is_editing,
                    selected_param,
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
                    text,
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
                // Five or more options crowd the title row; retain the full list in a dropdown.
                if labels.len() > 4 && matches!(param.param_type, ParamType::Choice { .. }) {
                    return render_param_as_selector(
                        text,
                        d,
                        entity,
                        plugin_idx,
                        choice_state,
                        idx,
                        param,
                        value,
                        knob_size,
                        is_editing,
                        selected_param,
                        interactive,
                        theme,
                    );
                }
                render_param_as_button_set(
                    d,
                    entity,
                    plugin_idx,
                    idx,
                    param,
                    value,
                    labels,
                    true,
                    knob_size == KnobSize::Xs,
                    is_editing,
                    selected_param,
                    interactive,
                    theme,
                    text,
                )
            } else {
                div().into_any_element()
            }
        }
        ControlType::Selector => {
            if let Some(param) = params.get(idx) {
                let value = values.get(idx).copied().unwrap_or(0.0);
                render_param_as_selector(
                    text,
                    d,
                    entity,
                    plugin_idx,
                    choice_state,
                    idx,
                    param,
                    value,
                    knob_size,
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
                let value = if param.engine_key == "playback_volume_db" {
                    plugin_data
                        .and_then(|data| {
                            data.downcast_ref::<
                            sotf_plugins::plugin_loudness_compensation::LoudnessCompensationData,
                        >()
                        })
                        .map(|data| f64::from(data.playback_volume_db))
                } else {
                    values.get(idx).copied()
                };
                let label = render_param_as_label(d, param, value, theme);
                #[cfg(feature = "dev-api")]
                let label = div()
                    .child(label)
                    .dev_track_with_state(
                        format!("plugin.readout.{plugin_idx}.{idx}"),
                        value.map_or_else(
                            || DevElementState::default().enabled(false),
                            |value| {
                                DevElementState::default()
                                    .enabled(false)
                                    .text(format!("{value:.1} {}", param.unit))
                            },
                        ),
                    )
                    .into_any_element();
                label
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
    let control = div()
        .id(SharedString::from(format!(
            "plugin-control-{plugin_idx}-{engine_key}"
        )))
        .child(control);
    #[cfg(feature = "dev-api")]
    let control = control.dev_track(format!("plugin.control.{plugin_idx}.{idx}"));
    control.into_any_element()
}

/// Render a compact horizontal parameter row with exact display-space entry.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_horizontal_param(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    value: f64,
    _is_editing: bool,
    _selected_param: usize,
    interactive: bool,
    theme: &Theme,
) -> AnyElement {
    render_horizontal_param_impl(
        d,
        entity,
        plugin_idx,
        idx,
        param,
        value,
        _is_editing,
        _selected_param,
        interactive,
        theme,
        true,
        false,
    )
}

/// Render a numeric row that fits beside another row in short racks.
#[expect(
    clippy::too_many_arguments,
    reason = "Matches the shared parameter renderer contract"
)]
pub(crate) fn render_narrow_horizontal_param(
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
    render_horizontal_param_impl(
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
        true,
        true,
    )
}

/// Render numeric table values without repeating their name-column label.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_horizontal_param_value(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    value: f64,
    _is_editing: bool,
    _selected_param: usize,
    interactive: bool,
    theme: &Theme,
) -> AnyElement {
    render_horizontal_param_impl(
        d,
        entity,
        plugin_idx,
        idx,
        param,
        value,
        _is_editing,
        _selected_param,
        interactive,
        theme,
        false,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_horizontal_param_impl(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    value: f64,
    _is_editing: bool,
    _selected_param: usize,
    interactive: bool,
    theme: &Theme,
    show_label: bool,
    narrow: bool,
) -> AnyElement {
    let (min, max, step) = match param.param_type {
        ParamType::Float { min, max, step, .. } => (min, max, step),
        ParamType::Int { min, max, step, .. } => (min as f64, max as f64, step as f64),
        _ => return div().into_any_element(),
    };
    let display_scale = if param.display_scale.is_finite() && param.display_scale != 0.0 {
        param.display_scale
    } else {
        1.0
    };
    let display_value = value * display_scale;
    let logarithmic = min > 0.0 && matches!(param.unit, "Hz" | "ms");
    let slider = Slider::new(("plugin-horizontal", plugin_idx * 1000 + idx))
        .value(display_value as f32)
        .range((min * display_scale) as f32, (max * display_scale) as f32)
        .logarithmic(logarithmic)
        .step((step * display_scale).abs() as f32)
        // A 96px slider plus a 4.5rem label and exact value fits a 280px lane.
        // Keep the normal geometry for full-width and table controls.
        .width(if narrow { 96.0 } else { 142.0 })
        .size(SliderSize::Sm)
        .theme(theme.to_slider_theme())
        .disabled(!interactive)
        .aria_label(format!(
            "{} {:.2} {}",
            param.name, display_value, param.unit
        ))
        .on_drag_start({
            let entity = entity.clone();
            move |_, _, _, cx| {
                entity.update(cx, |state, _| {
                    state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                    state.app.plugin_state.plugin_param_selection = idx;
                });
            }
        })
        .on_change({
            let entity = entity.clone();
            move |new_value, _, cx| {
                entity.update(cx, |state, _| {
                    state
                        .app
                        .set_plugin_param(plugin_idx, idx, new_value as f64);
                });
            }
        })
        .on_reset({
            let entity = entity.clone();
            move |_, cx| {
                entity.update(cx, |state, _| {
                    state.app.reset_plugin_param(plugin_idx, idx);
                });
            }
        });
    #[cfg(feature = "dev-api")]
    let slider = slider.dev_track(format!("plugin.slider.{plugin_idx}.{idx}"));
    let exact = NumberInput::new(SharedString::from(format!(
        "plugin-horizontal-number-{plugin_idx}-{idx}"
    )))
    .value(display_value)
    .min((min * display_scale).min(max * display_scale))
    .max((min * display_scale).max(max * display_scale))
    .step((step * display_scale.abs()).max(0.0001))
    .decimals(param.precision())
    .unit(param.unit)
    .size(NumberInputSize::Xs)
    .theme(theme.to_number_input_theme())
    .disabled(!interactive)
    .aria_label(format!("{} value", param.name))
    .on_change(move |new_value, _, cx| {
        entity.update(cx, |state, _| {
            state.app.set_plugin_param(plugin_idx, idx, new_value);
        });
    });
    #[cfg(feature = "dev-api")]
    let exact = exact.dev_track_with_state(
        format!("plugin.param.{plugin_idx}.{idx}.exact"),
        DevElementState::default().enabled(interactive),
    );
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(d.grid)
        .w_full()
        .min_w_0()
        .when(show_label, |row| {
            row.child(
                div()
                    .w(rems(if narrow { 4.5 } else { 7.0 }))
                    .flex_none()
                    .child(gpui_ui_kit::Text::label(param.name)),
            )
        })
        .child(slider)
        .child(exact)
        .into_any_element()
}

/// Replace a selected generic knob/slider with a bounded text editor.
///
/// The audio controls remain the fastest way to browse values, but a selected
/// control must also support exact entry for frequencies, gains, and timing
/// values. `set_plugin_param` accepts display-space values and applies the
/// parameter's display scale, so the editor follows the same contract as the
/// existing potentiometer and slider callbacks.
fn render_param_as_number_input(
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    value: f64,
    is_editing: bool,
    selected_param: usize,
    interactive: bool,
    _theme: &Theme,
) -> Option<AnyElement> {
    if !is_editing || selected_param != idx {
        return None;
    }

    let (min, max, step) = match param.param_type {
        ParamType::Float { min, max, step, .. } => (min, max, step),
        ParamType::Int { min, max, step, .. } => (min as f64, max as f64, step as f64),
        _ => return None,
    };

    let display_scale = if param.display_scale.is_finite() && param.display_scale != 0.0 {
        param.display_scale
    } else {
        1.0
    };
    let display_min = min * display_scale;
    let display_max = max * display_scale;
    let display_value = value * display_scale;
    let display_step = (step * display_scale.abs()).max(0.0001);
    let decimals = param.precision();
    let entity_for_change = entity;

    let input = NumberInput::new(SharedString::from(format!(
        "plugin-number-input-{plugin_idx}-{idx}"
    )))
    .value(display_value)
    .min(display_min.min(display_max))
    .max(display_min.max(display_max))
    .step(display_step)
    .decimals(decimals)
    .unit(param.unit)
    .size(NumberInputSize::Sm)
    .disabled(!interactive)
    .aria_label(format!("{} value", param.name))
    .on_change(move |new_value, _window, cx| {
        entity_for_change.update(cx, |state, _| {
            state.app.set_plugin_param(plugin_idx, idx, new_value);
        });
    });
    #[cfg(feature = "dev-api")]
    let input = input.dev_track(format!("plugin.param.{plugin_idx}.{idx}.exact"));
    Some(
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(gpui_ui_kit::Text::label(param.name))
            .child(input)
            .into_any_element(),
    )
}

/// Render a param as a toggle (for Bool and Choice types).
fn render_param_as_toggle(
    text: PluginCommonTranslations,
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
                knob_size == KnobSize::Xs,
                is_editing,
                selected_param,
                interactive,
                theme,
                text,
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
            knob_size == KnobSize::Xs,
            is_editing,
            selected_param,
            interactive,
            theme,
            text,
        )
        .into_any_element(),
        ParamType::Float { .. } | ParamType::Int { .. } => render_horizontal_param(
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
        ),
        ParamType::FilePath => div().into_any_element(),
    }
}

/// Render a schema-backed dropdown in a custom plugin editor.
pub(crate) fn render_compact_selector_param(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    value: f64,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    render_compact_selector_impl(
        d, entity, plugin_idx, idx, param, value, true, theme, cx, false, None,
    )
}

/// Render a table dropdown without repeating its name-column label.
pub(crate) fn render_compact_selector_value(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    value: f64,
    interactive: bool,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    render_compact_selector_impl(
        d,
        entity,
        plugin_idx,
        idx,
        param,
        value,
        interactive,
        theme,
        cx,
        true,
        None,
    )
}

/// Render a value-only selector constrained to its physical controller cell.
#[expect(clippy::too_many_arguments, reason = "Selector context and cell width")]
pub(crate) fn render_compact_selector_value_width(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    value: f64,
    interactive: bool,
    theme: &Theme,
    cx: &App,
    width: f32,
) -> AnyElement {
    render_compact_selector_impl(
        d,
        entity,
        plugin_idx,
        idx,
        param,
        value,
        interactive,
        theme,
        cx,
        true,
        Some(width),
    )
}

fn render_compact_selector_impl(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    value: f64,
    interactive: bool,
    theme: &Theme,
    cx: &App,
    hide_label: bool,
    selector_width: Option<f32>,
) -> AnyElement {
    let text = PluginCommonTranslations::for_language(entity.read(cx).app.ui_state.language);
    let app = &entity.read(cx).app;
    let instance_id = app
        .plugin_state
        .graph_state
        .editing_graph_node_uuid
        .and_then(|id| app.plugin_state.routing_controller().graph.nodes.get(&id))
        .map(|node| node.plugin.id)
        .or_else(|| {
            app.plugin_state
                .graph
                .get_plugin(plugin_idx)
                .map(|plugin| plugin.id)
        });
    let state = ChoiceMenuRenderState {
        instance_id,
        open_menu: app.plugin_ui.layout_choice_menu,
        highlighted_index: app.plugin_ui.layout_choice_highlight,
        compact_all_selectors: true,
        relocate_globals: false,
        hide_label,
        selector_width,
    };
    render_param_as_selector(
        text,
        d,
        entity,
        plugin_idx,
        state,
        idx,
        param,
        value,
        KnobSize::Xs,
        false,
        idx,
        interactive,
        theme,
    )
}

/// Render a choice parameter using its connected dropdown or button set.
fn render_param_as_selector(
    text: PluginCommonTranslations,
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    choice_state: ChoiceMenuRenderState,
    idx: usize,
    param: &ParamSpec,
    value: f64,
    knob_size: KnobSize,
    is_editing: bool,
    selected_param: usize,
    interactive: bool,
    theme: &Theme,
) -> AnyElement {
    match param.param_type {
        ParamType::Choice { labels, .. }
            if (labels.len() > 6 || choice_state.compact_all_selectors)
                && choice_state.instance_id.is_some() =>
        {
            let selected_index = (value as usize).min(labels.len() - 1);
            let Some(instance_id) = choice_state.instance_id else {
                return render_param_as_button_set(
                    d,
                    entity,
                    plugin_idx,
                    idx,
                    param,
                    value,
                    labels,
                    true,
                    knob_size == KnobSize::Xs,
                    is_editing,
                    selected_param,
                    interactive,
                    theme,
                    text,
                )
                .into_any_element();
            };
            let menu_key = (instance_id, idx);
            let open = choice_state.open_menu == Some(menu_key);
            let toggle_entity = entity.downgrade();
            let highlight_entity = entity.downgrade();
            let change_entity = entity.downgrade();
            let select = Select::new(SharedString::from(format!(
                "plugin-layout-choice-{}-{idx}",
                menu_key.0
            )))
            .aria_label(param.name)
            .size(SelectSize::Xs)
            .when_some(choice_state.selector_width, |select, width| {
                select.width(width)
            })
            .selected(selected_index.to_string())
            .options(
                labels
                    .iter()
                    .enumerate()
                    .map(|(index, label)| SelectOption::new(index.to_string(), *label))
                    .collect(),
            )
            .is_open(open)
            .highlighted_index(open.then_some(choice_state.highlighted_index).flatten())
            .disabled(!interactive)
            .theme(theme.to_select_theme())
            .on_toggle(move |new_open, _, cx| {
                let _ = toggle_entity.update(cx, |state, cx| {
                    state.app.plugin_ui.layout_choice_menu = new_open.then_some(menu_key);
                    state.app.plugin_ui.layout_choice_highlight =
                        new_open.then_some(selected_index);
                    cx.notify();
                });
            })
            .on_highlight(move |index, _, cx| {
                let _ = highlight_entity.update(cx, |state, cx| {
                    state.app.plugin_ui.layout_choice_highlight = index;
                    cx.notify();
                });
            })
            .on_change(move |selected, _, cx| {
                if let Ok(index) = selected.parse::<usize>() {
                    let _ = change_entity.update(cx, |state, cx| {
                        state.app.set_plugin_param(plugin_idx, idx, index as f64);
                        state.app.plugin_ui.layout_choice_menu = None;
                        state.app.plugin_ui.layout_choice_highlight = None;
                        cx.notify();
                    });
                }
            });
            #[cfg(feature = "dev-api")]
            let select = select.dev_track(format!("plugin.select.{plugin_idx}.{idx}"));
            div()
                .key_context("PluginLayoutChoiceSelect")
                .flex()
                .flex_none()
                .when_some(choice_state.selector_width, |row, width| row.w(px(width)))
                .flex_wrap()
                .items_center()
                .gap(d.grid)
                .max_w_full()
                .when(!choice_state.hide_label, |row| {
                    row.child(gpui_ui_kit::Text::label(param.name))
                })
                .child(
                    div()
                        .w(rems(if choice_state.compact_all_selectors {
                            9.0
                        } else if knob_size == KnobSize::Xs {
                            13.0
                        } else {
                            16.0
                        }))
                        .when_some(choice_state.selector_width, |row, width| row.w(px(width)))
                        .max_w_full()
                        .child(select),
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
            knob_size == KnobSize::Xs,
            is_editing,
            selected_param,
            interactive,
            theme,
            text,
        ),
        // Non-choice params: fall back to toggle
        _ => render_param_as_toggle(
            text,
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
        ),
    }
}

/// Render a param as a horizontal button set (mutually exclusive buttons).
/// Render a compact exclusive parameter choice with an inline label.
pub(crate) fn render_compact_choice_param(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    value: f64,
    theme: &Theme,
) -> AnyElement {
    use gpui_ui_kit::{ButtonSet, ButtonSetOption, ButtonSetSize};
    let labels = match param.param_type {
        ParamType::Bool {
            true_label,
            false_label,
            ..
        } => vec![false_label, true_label],
        ParamType::Choice { labels, .. } => labels.to_vec(),
        _ => return div().into_any_element(),
    };
    let choices = ButtonSet::new(("plugin-compact-choice", plugin_idx * 1000 + idx))
        .options(
            labels
                .iter()
                .enumerate()
                .map(|(i, label)| ButtonSetOption::new(i.to_string(), *label))
                .collect(),
        )
        .selected((value as usize).to_string())
        .size(ButtonSetSize::Xs)
        .theme(theme.to_button_set_theme())
        .on_change(move |value, _, cx| {
            if let Ok(value) = value.parse::<usize>() {
                entity.update(cx, |state, cx| {
                    state.app.set_plugin_param(plugin_idx, idx, value as f64);
                    cx.notify();
                });
            }
        });
    #[cfg(feature = "dev-api")]
    let choices = choices
        .option_wrapper(move |option, button| {
            button
                .dev_track(format!("plugin.choice.{plugin_idx}.{idx}.{option}"))
                .into_any_element()
        })
        .dev_track(format!("plugin.choicegroup.{plugin_idx}.{idx}"));
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(d.grid)
        .text_size(d.text_sm)
        .text_color(theme.text_secondary)
        .child(param.name.to_string())
        .child(choices)
        .into_any_element()
}

fn render_param_as_button_set(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    idx: usize,
    param: &ParamSpec,
    value: f64,
    labels: &[&str],
    show_label: bool,
    compact: bool,
    is_editing: bool,
    selected_param: usize,
    interactive: bool,
    theme: &Theme,
    text: PluginCommonTranslations,
) -> AnyElement {
    if matches!(param.engine_key, "learn_noise" | "clear_profile") {
        let capture_active = value > 0.5;
        let is_learn = param.engine_key == "learn_noise";
        let command_value = if is_learn && capture_active { 0.0 } else { 1.0 };
        let label = if is_learn {
            if capture_active {
                text.label("Cancel Learn")
            } else {
                text.label("Learn Noise")
            }
        } else {
            text.label("Clear Profile")
        };
        let action_entity = entity.clone();
        let button = Button::new(
            SharedString::from(format!("profile-action-{plugin_idx}-{idx}")),
            label,
        )
        .size(ButtonSize::Sm)
        .variant(ButtonVariant::Secondary)
        .theme(theme.to_button_theme())
        .disabled(!interactive)
        .aria_label(label)
        .on_click_event(move |_, _, cx| {
            action_entity.update(cx, |state, cx| {
                state.app.set_plugin_param(plugin_idx, idx, command_value);
                cx.notify();
            });
        });
        #[cfg(feature = "dev-api")]
        let button = button.dev_track_with_state(
            format!("plugin.action.{plugin_idx}.{idx}"),
            DevElementState::default().enabled(interactive).text(label),
        );
        return div()
            .flex()
            .flex_none()
            .items_start()
            .gap(d.grid)
            .child(button)
            .into_any_element();
    }
    use gpui_ui_kit::{ButtonSet, ButtonSetOption, ButtonSetSize};
    let choices = ButtonSet::new(("plugin-layout-buttons", plugin_idx * 1000 + idx))
        .options(
            labels
                .iter()
                .enumerate()
                .map(|(i, label)| ButtonSetOption::new(i.to_string(), *label))
                .collect(),
        )
        .selected((value as usize).to_string())
        .size(ButtonSetSize::Xs)
        .disabled(!interactive)
        .theme(theme.to_button_set_theme())
        .on_change(move |value, _, cx| {
            if let Ok(value) = value.parse::<usize>() {
                entity.update(cx, |state, cx| {
                    state.app.set_plugin_param(plugin_idx, idx, value as f64);
                    cx.notify();
                });
            }
        });
    #[cfg(feature = "dev-api")]
    let choices = choices
        .option_wrapper(move |option, button| {
            button
                .dev_track_with_state(
                    format!("plugin.choice.{plugin_idx}.{idx}.{option}"),
                    DevElementState::default()
                        .enabled(interactive)
                        .selected(option == value as usize),
                )
                .into_any_element()
        })
        .dev_track_with_state(
            format!("plugin.choicegroup.{plugin_idx}.{idx}"),
            DevElementState::default().enabled(interactive),
        );
    let _ = (compact, is_editing, selected_param);
    div()
        .key_context("PluginChoice")
        .flex()
        .flex_wrap()
        .items_center()
        .gap(d.grid)
        .when(show_label, |row| {
            row.child(gpui_ui_kit::Text::label(param.name))
        })
        .child(choices)
        .into_any_element()
}

/// Render a gain reduction meter.
fn render_bar_meter(
    d: &Ds,
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    min_db: f64,
    _max_db: f64,
    theme: &Theme,
) -> AnyElement {
    render_gr_meter(d, current_gain_reduction_db(plugin_data), min_db, theme).into_any_element()
}

fn current_gain_reduction_db(
    plugin_data: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
) -> f64 {
    plugin_data
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
        .unwrap_or(0.0)
}

/// Render a read-only label for a param value.
fn render_param_as_label(
    d: &Ds,
    param: &ParamSpec,
    value: Option<f64>,
    theme: &Theme,
) -> AnyElement {
    let formatted = value.map_or_else(
        || "Awaiting engine".to_string(),
        |value| {
            let display_val = value * param.display_scale;
            if param.unit.is_empty() {
                format!("{:.1}", display_val)
            } else {
                format!("{:.1} {}", display_val, param.unit)
            }
        },
    );

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
pub(crate) fn render_file_picker(
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
    let full_path = file_path.filter(|path| !path.is_empty()).map(str::to_owned);
    let engine_key = param.engine_key;
    let param_name = param.name;
    let file_button = IconButton::with_child(
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
        button.on_click_event(
            move |_event, window, cx| match file_picker_open_target(engine_key) {
                Some(FilePickerOpenTarget::Sofa) => window.dispatch_action(
                    Box::new(OpenSofaFile {
                        plugin_idx,
                        param_idx: idx,
                    }),
                    cx,
                ),
                Some(FilePickerOpenTarget::HrtfDatabase) => window.dispatch_action(
                    Box::new(OpenHrtfDatabase {
                        plugin_idx,
                        param_idx: idx,
                    }),
                    cx,
                ),
                Some(FilePickerOpenTarget::Ir) => window.dispatch_action(
                    Box::new(OpenIrFile {
                        plugin_idx,
                        param_idx: idx,
                    }),
                    cx,
                ),
                Some(FilePickerOpenTarget::AbConfig(path_id)) => {
                    window.dispatch_action(
                        Box::new(OpenAbConfigFile {
                            plugin_idx,
                            path_id: path_id.to_string(),
                        }),
                        cx,
                    );
                }
                None => log::warn!("No file open action for engine_key: {}", engine_key),
            },
        )
    })
    .when(!interactive, |button| button.disabled(true));
    #[cfg(feature = "dev-api")]
    let file_button =
        file_button.dev_track(format!("plugin.file_picker.{plugin_idx}.{engine_key}"));

    let filename = div()
        .text_size(d.text_sm)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(text_color)
        .overflow_hidden()
        .text_ellipsis()
        .min_w_0()
        .max_w(rems(7.5))
        .whitespace_nowrap()
        .child(display_name.to_string());
    #[cfg(feature = "dev-api")]
    let filename = filename.dev_track_with_state(
        format!("plugin.file_status.{plugin_idx}.{engine_key}"),
        DevElementState::default()
            .text(display_name)
            .selected(has_file),
    );

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(d.gap)
        .min_w_0()
        .max_w_full()
        .px(d.pad_x)
        .py(d.pad_y)
        .rounded(d.r_lg)
        .bg(theme.background_secondary)
        .child(
            div()
                .flex()
                .items_center()
                .gap(d.gap)
                .min_w_0()
                .child(
                    div()
                        .flex_none()
                        .text_size(d.text_sm)
                        .text_color(theme.text_muted)
                        .child(param.name),
                )
                .child(filename),
        )
        .child(
            div()
                .id(SharedString::from(format!(
                    "load-file-tooltip-{plugin_idx}-{idx}"
                )))
                .child(file_button)
                .tooltip({
                    let theme = theme.clone();
                    move |_window, cx| {
                        themed_tooltip(
                            full_path.clone().unwrap_or_else(|| param_name.to_string()),
                            &theme,
                            cx,
                        )
                    }
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
    width: f32,
    theme: &Theme,
    text: Option<PluginCommonTranslations>,
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
    let knee_db = find_value("knee")
        .or_else(|| find_value("knee_db"))
        .unwrap_or_else(|| {
            if find_value("soft").unwrap_or(0.0) > 0.5 {
                1.0
            } else {
                0.0
            }
        });

    // Detect limiter: no ratio param means brickwall limiter
    let (ratio, is_limiter) = match find_value("ratio") {
        Some(r) => (r, false),
        None => (f64::INFINITY, true),
    };

    // Gate and Expander share neither a compressor's direction nor its slope.
    // Their plots show the processor's static wet target, before envelope history.
    if let Some(range_db) = find_value("range_db").or_else(|| find_value("range"))
        && (find_value("max_boost_db").is_some() || find_value("hysteresis").is_some())
    {
        let mode = find_value("mode");
        let input: Vec<_> = (0..=160)
            .map(|sample| -80.0 + f64::from(sample) * 0.5)
            .collect();
        let output: Vec<_> = input
            .iter()
            .map(|&level| {
                let gain = if let Some(mode) = mode {
                    let mode = match mode as usize {
                        1 => sotf_plugins::GateMode::Upward,
                        2 => sotf_plugins::GateMode::Duck,
                        _ => sotf_plugins::GateMode::Downward,
                    };
                    sotf_plugins::plugin_gate::target_gain_db(
                        level as f32,
                        threshold_db as f32,
                        ratio as f32,
                        knee_db as f32,
                        range_db as f32,
                        find_value("max_boost_db").unwrap_or(12.0) as f32,
                        mode,
                    )
                    .unwrap_or(0.0)
                } else {
                    -sotf_plugins::plugin_multiband_expander::target_attenuation_db(
                        level as f32,
                        threshold_db as f32,
                        ratio as f32,
                        knee_db as f32,
                        range_db as f32,
                    )
                    .unwrap_or(0.0)
                };
                level + f64::from(gain)
            })
            .collect();
        let y_min = output.iter().copied().fold(-80.0, f64::min);
        let y_max = output.iter().copied().fold(0.0, f64::max);
        let chart = render_line_chart(
            vec![
                Series::new(
                    "Unity",
                    rgba_to_u32(theme.text_muted),
                    input.clone(),
                    input.clone(),
                )
                .with_opacity(0.35),
                Series::new("Target response", rgba_to_u32(theme.accent), input, output),
            ],
            ChartConfig {
                title: None,
                x_label: None,
                y_label: None,
                x_range: (-80.0, 0.0),
                y_range: (y_min, y_max),
                x_scale: ScaleType::Linear,
                width,
                height: (width * 0.7).clamp(140.0, 240.0),
            },
            theme,
            None,
        );
        let plot =
            div()
                .flex()
                .flex_col()
                .gap(d.grid)
                .child(chart)
                .child(gpui_ui_kit::Text::caption(
                    text.map_or("Target response", |text| text.label("Target response")),
                ));
        #[cfg(feature = "dev-api")]
        let plot = plot.dev_track("plugin.transfer.target");
        return plot.into_any_element();
    }

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
        width,
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
    choice_state: ChoiceMenuRenderState,
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
    let text = text.unwrap_or_else(|| {
        PluginCommonTranslations::for_language(crate::app::i18n::Language::English)
    });
    match content {
        LayoutTabContent::Controls(controls) => {
            let mut tab_div = div().flex().flex_wrap().justify_center().gap(d.section);
            for spec in controls {
                if spec.hidden {
                    continue;
                }
                tab_div = tab_div.child(render_control(
                    text,
                    d,
                    entity.clone(),
                    plugin_idx,
                    choice_state,
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
                choice_state,
                group,
                layout,
                params,
                values,
                file_paths,
                is_editing,
                selected_param,
                solved,
                solved.column_width(ColumnRole::Main).unwrap_or(0.0),
                f32::INFINITY,
                &[],
                plugin_data,
                theme,
                spider_snapshot,
                Some(text),
                true,
            ))
            .into_any_element(),
    }
}
