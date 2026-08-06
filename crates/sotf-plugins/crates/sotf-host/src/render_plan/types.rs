use super::control_plan::controls_to_plans;
use super::format::format_viz_position;
use crate::param_specs::ParamSpec;
use crate::plugin_layout::{ControlGroup, DynamicSection, TabSpec, VizSlot};
use serde::Serialize;

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
    /// Stable IDs of main groups rendered on the primary surface.
    pub visible_group_ids: Vec<String>,
    /// Stable IDs of main groups rendered in responsive overflow.
    pub overflow_group_ids: Vec<String>,

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
    pub enabled: bool,
}

/// A named group of controls.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GroupPlan {
    pub id: String,
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

pub(super) fn group_to_plan(
    group: &ControlGroup,
    params: &[ParamSpec],
    values: &[f64],
) -> GroupPlan {
    GroupPlan {
        id: group.id.to_string(),
        title: group.title.to_string(),
        controls: controls_to_plans(group.controls, params, values),
    }
}

pub(super) fn tab_to_plan(tab: &TabSpec, params: &[ParamSpec], values: &[f64]) -> TabPlan {
    TabPlan {
        name: tab.name.to_string(),
        controls: controls_to_plans(tab.controls, params, values),
    }
}

pub(super) fn viz_to_plan(viz: &VizSlot) -> VizPlan {
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

pub(super) fn dynamic_section_to_plan(
    ds: &DynamicSection,
    params: &[ParamSpec],
    values: &[f64],
) -> DynamicSectionPlan {
    DynamicSectionPlan {
        instance_name: ds.instance_name.to_string(),
        count_range: ds.count_range,
        has_global_defaults: ds.has_global_defaults,
        template_controls: controls_to_plans(ds.template_controls, params, values),
    }
}
