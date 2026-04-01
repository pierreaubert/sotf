//! Dynamic parameter bridge between ParamSpec and nih-plug's Params trait.

use nih_plug::prelude::*;
use plugins_bridge::param_bridge::BridgedParamInfo;
use sotf_host::parameters::{ParameterId, ParameterValue};
use std::collections::HashMap;
use std::sync::Arc;

/// Dynamic nih-plug Params implementation built from ParamSpec metadata.
pub struct DynamicParams {
    float_params: Vec<FloatParam>,
    bool_params: Vec<BoolParam>,
    int_params: Vec<IntParam>,
    /// Map from parameter ID to (kind, index)
    param_map: HashMap<String, ParamEntry>,
}

#[derive(Clone, Copy)]
enum ParamKind {
    Float,
    Bool,
    Int,
}

#[derive(Clone, Copy)]
struct ParamEntry {
    kind: ParamKind,
    index: usize,
}

impl DynamicParams {
    pub fn from_infos(infos: &[BridgedParamInfo]) -> Arc<Self> {
        let mut float_params = Vec::new();
        let mut bool_params = Vec::new();
        let mut int_params = Vec::new();
        let mut param_map = HashMap::new();

        for info in infos {
            if info.steps == 1 && info.min_value == 0.0 && info.max_value == 1.0 {
                // Bool parameter
                let idx = bool_params.len();
                let param = BoolParam::new(&info.name, info.default_value > 0.5);
                bool_params.push(param);
                param_map.insert(
                    info.id.clone(),
                    ParamEntry {
                        kind: ParamKind::Bool,
                        index: idx,
                    },
                );
            } else if info.steps > 0 && info.steps < 100 {
                // Int/Choice parameter
                let idx = int_params.len();
                let param = IntParam::new(
                    &info.name,
                    info.default_value as i32,
                    IntRange::Linear {
                        min: info.min_value as i32,
                        max: info.max_value as i32,
                    },
                );
                int_params.push(param);
                param_map.insert(
                    info.id.clone(),
                    ParamEntry {
                        kind: ParamKind::Int,
                        index: idx,
                    },
                );
            } else {
                // Float parameter
                let idx = float_params.len();
                let range = if info.logarithmic {
                    FloatRange::Skewed {
                        min: info.min_value as f32,
                        max: info.max_value as f32,
                        factor: FloatRange::skew_factor(-2.0),
                    }
                } else {
                    FloatRange::Linear {
                        min: info.min_value as f32,
                        max: info.max_value as f32,
                    }
                };

                let param = FloatParam::new(&info.name, info.default_value as f32, range);
                float_params.push(param);
                param_map.insert(
                    info.id.clone(),
                    ParamEntry {
                        kind: ParamKind::Float,
                        index: idx,
                    },
                );
            }
        }

        Arc::new(Self {
            float_params,
            bool_params,
            int_params,
            param_map,
        })
    }

    /// Sync all parameter values to a SOTF plugin.
    pub fn sync_to_plugin(&self, plugin: &mut dyn sotf_host::plugin::Plugin) {
        for (id, entry) in &self.param_map {
            let value = match entry.kind {
                ParamKind::Float => ParameterValue::Float(self.float_params[entry.index].value()),
                ParamKind::Bool => ParameterValue::Bool(self.bool_params[entry.index].value()),
                ParamKind::Int => ParameterValue::Int(self.int_params[entry.index].value()),
            };
            let _ = plugin.set_parameter(ParameterId(id.clone()), value);
        }
    }
}

// SAFETY: nih-plug requires Params to be Send + Sync. Our params are simple value types.
unsafe impl Send for DynamicParams {}
unsafe impl Sync for DynamicParams {}

// SAFETY: All parameter pointers are valid for the lifetime of DynamicParams.
// The param_map returns stable pointers to owned fields.
unsafe impl Params for DynamicParams {
    fn param_map(&self) -> Vec<(String, ParamPtr, String)> {
        let mut map = Vec::new();

        for (id, entry) in &self.param_map {
            let ptr = match entry.kind {
                ParamKind::Float => self.float_params[entry.index].as_ptr(),
                ParamKind::Bool => self.bool_params[entry.index].as_ptr(),
                ParamKind::Int => self.int_params[entry.index].as_ptr(),
            };
            map.push((id.clone(), ptr, String::new()));
        }

        map
    }
}
