use super::types::TuiParamDescriptor;
use super::types::TuiParamType;

pub(super) fn spec_to_descriptor(
    spec: &sotf_plugins::param_specs::ParamSpec,
) -> TuiParamDescriptor {
    use sotf_plugins::param_specs::ParamType;
    TuiParamDescriptor {
        name: spec.name.to_string(),
        param_type: match spec.param_type {
            ParamType::Float { min, max, step, .. } => TuiParamType::Float { min, max, step },
            ParamType::Int { min, max, step, .. } => TuiParamType::Int {
                min: min as i32,
                max: max as i32,
                step: step as i32,
            },
            ParamType::Bool { .. } => TuiParamType::Bool,
            ParamType::Choice { labels, .. } => TuiParamType::Choice {
                count: labels.len(),
            },
            ParamType::FilePath => TuiParamType::Choice { count: 0 },
        },
        unit: spec.unit.to_string(),
        group: spec.group.to_string(),
        doc: spec.doc.to_string(),
    }
}

pub(super) fn specs_to_descriptors(
    specs: &[sotf_plugins::param_specs::ParamSpec],
) -> Vec<TuiParamDescriptor> {
    specs.iter().map(spec_to_descriptor).collect()
}
