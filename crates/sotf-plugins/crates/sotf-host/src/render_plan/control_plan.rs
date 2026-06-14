use super::format::format_control_type;
use super::misc::param_range;
use super::types::ControlPlan;
use crate::param_specs::ParamSpec;
use crate::plugin_layout::ControlSpec;

pub(super) fn controls_to_plans(specs: &[ControlSpec], params: &[ParamSpec]) -> Vec<ControlPlan> {
    specs
        .iter()
        .map(|spec| control_to_plan(spec, params))
        .collect()
}

pub(super) fn control_to_plan(spec: &ControlSpec, params: &[ParamSpec]) -> ControlPlan {
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
