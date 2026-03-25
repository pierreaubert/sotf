//! Render Plan — testable snapshot of layout decisions
//!
//! Combines `PluginParamDef::PARAMS`, `PluginParamDef::LAYOUT`, and `solve_layout()`
//! into a serializable `PluginRenderPlan` that captures every structural decision
//! the layout renderer will make — without any GPUI dependency.
//!
//! Used for snapshot testing: 33 plugins × 10 device profiles = ~330 JSON snapshots.
//! Any layout regression at any screen size produces a diff.

use crate::design_system::DesignSystem;
use crate::layout_solver::{self, Direction, KnobSize, Orientation};
use crate::param_specs::ParamSpec;
use crate::plugin_layout::{
    ControlGroup, ControlSpec, ControlType, DynamicSection, PluginLayout, TabSpec, VizPosition,
    VizSlot,
};
use crate::plugin_params::PluginParamDef;
use serde::Serialize;

// ============================================================================
// Render Plan Types
// ============================================================================

/// Serializable snapshot of what the layout renderer will draw.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PluginRenderPlan {
    pub plugin_type: String,
    pub width: f32,

    // Design system identity
    pub design_language: String,
    pub corner_radius_md: f32,
    pub min_touch_target: f32,
    pub toggle_variant: String,
    pub label_position: String,

    // Solved layout decisions
    pub orientation: String,
    pub knob_size: String,
    pub group_direction: String,
    pub slider_height: f32,
    pub show_visualizations: bool,
    pub columns_visible: Vec<String>,
    pub columns_collapsed: Vec<String>,

    // Controls by column
    pub config_controls: Vec<ControlPlan>,
    pub main_groups: Vec<GroupPlan>,
    pub output_controls: Vec<ControlPlan>,
    pub tabs: Vec<TabPlan>,
    pub viz_slots: Vec<VizPlan>,
    pub dynamic_sections: Vec<DynamicSectionPlan>,
}

/// A single control in the render plan.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ControlPlan {
    pub param_index: usize,
    pub param_name: String,
    pub control_type: String,
    pub unit: String,
    pub range: Option<(f64, f64)>,
    pub read_only: bool,
}

/// A named group of controls.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GroupPlan {
    pub title: String,
    pub controls: Vec<ControlPlan>,
}

/// A tab with its controls.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TabPlan {
    pub name: String,
    pub controls: Vec<ControlPlan>,
}

/// A visualization slot.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VizPlan {
    pub viz_type: String,
    pub position: String,
}

/// A dynamic (repeated) section.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DynamicSectionPlan {
    pub instance_name: String,
    pub count_range: (usize, usize),
    pub has_global_defaults: bool,
    pub template_controls: Vec<ControlPlan>,
}

// ============================================================================
// Builder
// ============================================================================

/// Build a render plan for a plugin at a given width, using the neutral design system.
pub fn build_render_plan<P: PluginParamDef>(available_width: f32) -> PluginRenderPlan {
    build_render_plan_with_ds::<P>(available_width, &DesignSystem::neutral())
}

/// Build a render plan for a plugin at a given width and design system.
pub fn build_render_plan_with_ds<P: PluginParamDef>(
    available_width: f32,
    ds: &DesignSystem,
) -> PluginRenderPlan {
    let layout = P::LAYOUT.unwrap_or_else(|| {
        panic!(
            "plugin '{}' must have a LAYOUT to build a render plan",
            P::PLUGIN_TYPE_KEY
        )
    });
    let params = P::PARAMS;
    build_render_plan_from_layout(P::PLUGIN_TYPE_KEY, params, layout, available_width, ds)
}

/// Build a render plan from raw layout + params (for plugins not using PluginParamDef).
pub fn build_render_plan_from_layout(
    plugin_type: &str,
    params: &[ParamSpec],
    layout: &PluginLayout,
    available_width: f32,
    ds: &DesignSystem,
) -> PluginRenderPlan {
    let solved =
        layout_solver::solve_layout_with_ds(layout.column_constraints, available_width, ds);

    PluginRenderPlan {
        plugin_type: plugin_type.to_string(),
        width: available_width,
        design_language: ds.language.as_str().to_string(),
        corner_radius_md: ds.corners.md,
        min_touch_target: ds.interaction.min_touch_target,
        toggle_variant: format_toggle_variant(&ds.toggle_variant),
        label_position: format_label_position(&ds.label_position),
        orientation: format_orientation(solved.orientation),
        knob_size: format_knob_size(solved.knob_size),
        group_direction: format_direction(solved.group_direction),
        slider_height: solved.slider_height,
        show_visualizations: solved.show_visualizations,
        columns_visible: solved
            .columns
            .iter()
            .map(|c| format!("{:?}", c.role).to_lowercase())
            .collect(),
        columns_collapsed: solved
            .collapsed_tabs
            .iter()
            .map(|t| format!("{:?}", t.role).to_lowercase())
            .collect(),
        config_controls: controls_to_plans(layout.config, params),
        main_groups: layout
            .main
            .iter()
            .map(|g| group_to_plan(g, params))
            .collect(),
        output_controls: controls_to_plans(layout.output, params),
        tabs: layout
            .tabs
            .iter()
            .map(|t| tab_to_plan(t, params))
            .collect(),
        viz_slots: layout.visualizations.iter().map(viz_to_plan).collect(),
        dynamic_sections: layout
            .dynamic_sections
            .iter()
            .map(|ds| dynamic_section_to_plan(ds, params))
            .collect(),
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn controls_to_plans(specs: &[ControlSpec], params: &[ParamSpec]) -> Vec<ControlPlan> {
    specs
        .iter()
        .map(|spec| control_to_plan(spec, params))
        .collect()
}

fn control_to_plan(spec: &ControlSpec, params: &[ParamSpec]) -> ControlPlan {
    let (param_name, unit, range) = if spec.param_index < params.len() {
        let p = &params[spec.param_index];
        (p.name.to_string(), p.unit.to_string(), param_range(p))
    } else {
        // Meter or placeholder (param_index == usize::MAX)
        ("(meter)".to_string(), String::new(), None)
    };

    ControlPlan {
        param_index: spec.param_index,
        param_name,
        control_type: format_control_type(&spec.control_type),
        unit,
        range,
        read_only: spec.read_only,
    }
}

fn group_to_plan(group: &ControlGroup, params: &[ParamSpec]) -> GroupPlan {
    GroupPlan {
        title: group.title.to_string(),
        controls: controls_to_plans(group.controls, params),
    }
}

fn tab_to_plan(tab: &TabSpec, params: &[ParamSpec]) -> TabPlan {
    TabPlan {
        name: tab.name.to_string(),
        controls: controls_to_plans(tab.controls, params),
    }
}

fn viz_to_plan(viz: &VizSlot) -> VizPlan {
    let (viz_type, position) = match viz {
        VizSlot::TransferCurve { position } => ("transfer_curve", format_viz_position(position)),
        VizSlot::FrequencyResponse { position } => {
            ("frequency_response", format_viz_position(position))
        }
        VizSlot::Custom { name, position } => (name as &str, format_viz_position(position)),
    };
    VizPlan {
        viz_type: viz_type.to_string(),
        position,
    }
}

fn dynamic_section_to_plan(ds: &DynamicSection, params: &[ParamSpec]) -> DynamicSectionPlan {
    DynamicSectionPlan {
        instance_name: ds.instance_name.to_string(),
        count_range: ds.count_range,
        has_global_defaults: ds.has_global_defaults,
        template_controls: controls_to_plans(ds.template_controls, params),
    }
}

fn param_range(p: &ParamSpec) -> Option<(f64, f64)> {
    use crate::param_specs::ParamType;
    match p.param_type {
        ParamType::Float { min, max, .. } => Some((min, max)),
        ParamType::Int { min, max, .. } => Some((min as f64, max as f64)),
        ParamType::Bool { .. } => Some((0.0, 1.0)),
        ParamType::Choice { labels, .. } => Some((0.0, labels.len().saturating_sub(1) as f64)),
        ParamType::FilePath => None,
    }
}

fn format_orientation(o: Orientation) -> String {
    match o {
        Orientation::Horizontal => "horizontal".to_string(),
        Orientation::Vertical => "vertical".to_string(),
    }
}

fn format_knob_size(k: KnobSize) -> String {
    match k {
        KnobSize::Xs => "xs".to_string(),
        KnobSize::Sm => "sm".to_string(),
        KnobSize::Md => "md".to_string(),
    }
}

fn format_direction(d: Direction) -> String {
    match d {
        Direction::Row => "row".to_string(),
        Direction::Column => "column".to_string(),
    }
}

fn format_control_type(ct: &ControlType) -> String {
    match ct {
        ControlType::Knob => "knob".to_string(),
        ControlType::KnobLarge => "knob_large".to_string(),
        ControlType::VerticalSlider => "vertical_slider".to_string(),
        ControlType::Toggle => "toggle".to_string(),
        ControlType::ButtonSet { .. } => "button_set".to_string(),
        ControlType::Selector => "selector".to_string(),
        ControlType::BarMeter { min_db, max_db } => {
            format!("bar_meter({min_db:.0}..{max_db:.0})")
        }
        ControlType::Label => "label".to_string(),
        ControlType::FilePicker => "file_picker".to_string(),
    }
}

fn format_toggle_variant(tv: &crate::design_system::ToggleVariant) -> String {
    use crate::design_system::ToggleVariant;
    match tv {
        ToggleVariant::Capsule => "capsule".to_string(),
        ToggleVariant::ThumbOnTrack => "thumb_on_track".to_string(),
        ToggleVariant::Segmented => "segmented".to_string(),
        ToggleVariant::Pill => "pill".to_string(),
    }
}

fn format_label_position(lp: &crate::design_system::LabelPosition) -> String {
    use crate::design_system::LabelPosition;
    match lp {
        LabelPosition::Below => "below".to_string(),
        LabelPosition::Right => "right".to_string(),
    }
}

fn format_viz_position(pos: &VizPosition) -> String {
    match pos {
        VizPosition::BelowGroup(title) => format!("below:{title}"),
        VizPosition::FullCenter => "full_center".to_string(),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_solver::SLIDER_HEIGHT_COMPACT;
    use crate::param_specs::{ParamCategory, ParamType, UpdateMode};
    use crate::plugin_layout::ColumnConstraint;

    // Static test data to satisfy 'static lifetime requirements of PluginLayout.

    static TEST_PARAMS: [ParamSpec; 2] = [
        ParamSpec {
            name: "Gain",
            engine_key: "gain_db",
            param_type: ParamType::Float {
                default: 0.0,
                min: -60.0,
                max: 12.0,
                step: 0.1,
            },
            unit: "dB",
            group: "Main",
            update_mode: UpdateMode::Realtime,
            display_scale: 1.0,
            category: ParamCategory::Primary,
            doc: "Output gain",
        },
        ParamSpec {
            name: "Mix",
            engine_key: "mix",
            param_type: ParamType::Float {
                default: 1.0,
                min: 0.0,
                max: 1.0,
                step: 0.01,
            },
            unit: "%",
            group: "Output",
            update_mode: UpdateMode::Realtime,
            display_scale: 100.0,
            category: ParamCategory::Output,
            doc: "Dry/wet mix",
        },
    ];

    static TEST_MAIN_CONTROLS: [ControlSpec; 1] = [ControlSpec::knob(0)];
    static TEST_MAIN_GROUPS: [ControlGroup; 1] = [ControlGroup {
        title: "MAIN",
        controls: &TEST_MAIN_CONTROLS,
    }];
    static TEST_OUTPUT_CONTROLS: [ControlSpec; 1] = [ControlSpec::knob(1)];
    static TEST_COLUMN_CONSTRAINTS: [ColumnConstraint; 2] = [
        ColumnConstraint::main(200.0),
        ColumnConstraint::output(100.0, 0.6),
    ];

    static TEST_LAYOUT: PluginLayout = PluginLayout {
        config: &[],
        main: &TEST_MAIN_GROUPS,
        output: &TEST_OUTPUT_CONTROLS,
        tabs: &[],
        visualizations: &[],
        column_constraints: &TEST_COLUMN_CONSTRAINTS,
        dynamic_sections: &[],
    };

    #[test]
    fn test_build_plan_wide() {
        let plan =
            build_render_plan_from_layout("test_plugin", &TEST_PARAMS, &TEST_LAYOUT, 1200.0, &DesignSystem::neutral());

        assert_eq!(plan.plugin_type, "test_plugin");
        assert_eq!(plan.width, 1200.0);
        assert_eq!(plan.orientation, "horizontal");
        assert!(plan.columns_visible.contains(&"main".to_string()));
        assert!(plan.columns_visible.contains(&"output".to_string()));
        assert!(plan.columns_collapsed.is_empty());
        assert_eq!(plan.main_groups.len(), 1);
        assert_eq!(plan.main_groups[0].title, "MAIN");
        assert_eq!(plan.main_groups[0].controls[0].param_name, "Gain");
        assert_eq!(plan.main_groups[0].controls[0].control_type, "knob");
        assert_eq!(plan.output_controls.len(), 1);
        assert_eq!(plan.output_controls[0].param_name, "Mix");
    }

    #[test]
    fn test_build_plan_vertical_mode() {
        let plan =
            build_render_plan_from_layout("test_plugin", &TEST_PARAMS, &TEST_LAYOUT, 350.0, &DesignSystem::neutral());

        assert_eq!(plan.orientation, "vertical");
        assert_eq!(plan.knob_size, "xs");
        assert_eq!(plan.slider_height, SLIDER_HEIGHT_COMPACT);
        assert!(!plan.show_visualizations);
        assert!(plan.columns_collapsed.contains(&"output".to_string()));
    }

    static VIZ_LAYOUT_CONSTRAINTS: [ColumnConstraint; 1] = [ColumnConstraint::main(200.0)];
    static VIZ_LAYOUT_VIZS: [VizSlot; 1] = [VizSlot::TransferCurve {
        position: VizPosition::FullCenter,
    }];
    static VIZ_LAYOUT: PluginLayout = PluginLayout {
        config: &[],
        main: &[],
        output: &[],
        tabs: &[],
        visualizations: &VIZ_LAYOUT_VIZS,
        column_constraints: &VIZ_LAYOUT_CONSTRAINTS,
        dynamic_sections: &[],
    };

    #[test]
    fn test_build_plan_with_viz() {
        let plan = build_render_plan_from_layout("comp", &TEST_PARAMS, &VIZ_LAYOUT, 1000.0, &DesignSystem::neutral());

        assert_eq!(plan.viz_slots.len(), 1);
        assert_eq!(plan.viz_slots[0].viz_type, "transfer_curve");
        assert_eq!(plan.viz_slots[0].position, "full_center");
    }

    static DS_TEMPLATE_CONTROLS: [ControlSpec; 2] = [ControlSpec::knob(0), ControlSpec::knob(1)];
    static DS_SECTIONS: [DynamicSection; 1] = [DynamicSection {
        instance_name: "Band",
        template_params: &[0, 1],
        template_controls: &DS_TEMPLATE_CONTROLS,
        count_range: (2, 5),
        count_param_index: None,
        has_global_defaults: true,
    }];
    static DS_LAYOUT: PluginLayout = PluginLayout {
        config: &[],
        main: &[],
        output: &[],
        tabs: &[],
        visualizations: &[],
        column_constraints: &VIZ_LAYOUT_CONSTRAINTS,
        dynamic_sections: &DS_SECTIONS,
    };

    #[test]
    fn test_build_plan_with_dynamic_section() {
        let plan = build_render_plan_from_layout("mb_comp", &TEST_PARAMS, &DS_LAYOUT, 1000.0, &DesignSystem::neutral());

        assert_eq!(plan.dynamic_sections.len(), 1);
        assert_eq!(plan.dynamic_sections[0].instance_name, "Band");
        assert_eq!(plan.dynamic_sections[0].count_range, (2, 5));
        assert!(plan.dynamic_sections[0].has_global_defaults);
        assert_eq!(plan.dynamic_sections[0].template_controls.len(), 2);
    }

    static METER_CONTROLS: [ControlSpec; 1] = [ControlSpec::meter(-60.0, 0.0)];
    static METER_GROUPS: [ControlGroup; 1] = [ControlGroup {
        title: "METERS",
        controls: &METER_CONTROLS,
    }];
    static METER_LAYOUT: PluginLayout = PluginLayout {
        config: &[],
        main: &METER_GROUPS,
        output: &[],
        tabs: &[],
        visualizations: &[],
        column_constraints: &VIZ_LAYOUT_CONSTRAINTS,
        dynamic_sections: &[],
    };

    #[test]
    fn test_meter_control_plan() {
        let plan = build_render_plan_from_layout("test", &TEST_PARAMS, &METER_LAYOUT, 1000.0, &DesignSystem::neutral());

        let meter = &plan.main_groups[0].controls[0];
        assert_eq!(meter.param_name, "(meter)");
        assert_eq!(meter.control_type, "bar_meter(-60..0)");
        assert!(meter.read_only);
    }
}
