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
    /// Stable declaration order used by the realtime sync path. Hash-map
    /// iteration would make same-frame adapter commands nondeterministic.
    sync_entries: Vec<ParamEntry>,
}

#[derive(Clone, Copy)]
enum ParamKind {
    Float,
    Bool,
    Int,
}

#[derive(Clone)]
struct ParamEntry {
    kind: ParamKind,
    index: usize,
    id: ParameterId,
    realtime: bool,
}

impl DynamicParams {
    pub fn from_infos(infos: &[BridgedParamInfo]) -> Arc<Self> {
        let mut float_params = Vec::new();
        let mut bool_params = Vec::new();
        let mut int_params = Vec::new();
        let mut param_map = HashMap::new();
        let mut sync_entries = Vec::new();

        for info in infos {
            if info.steps == 1 && info.min_value == 0.0 && info.max_value == 1.0 {
                // Bool parameter
                let idx = bool_params.len();
                let mut param = BoolParam::new(&info.name, info.default_value > 0.5);
                if !info.realtime {
                    param = param.hide();
                }
                bool_params.push(param);
                let entry = ParamEntry {
                    kind: ParamKind::Bool,
                    index: idx,
                    id: ParameterId::from(info.id.as_str()),
                    realtime: info.realtime,
                };
                sync_entries.push(entry.clone());
                param_map.insert(info.id.clone(), entry);
            } else if info.steps > 0 && info.steps < 100 {
                // Int/Choice parameter
                let idx = int_params.len();
                let mut param = IntParam::new(
                    &info.name,
                    info.default_value as i32,
                    IntRange::Linear {
                        min: info.min_value as i32,
                        max: info.max_value as i32,
                    },
                );
                if !info.realtime {
                    param = param.hide();
                }
                int_params.push(param);
                let entry = ParamEntry {
                    kind: ParamKind::Int,
                    index: idx,
                    id: ParameterId::from(info.id.as_str()),
                    realtime: info.realtime,
                };
                sync_entries.push(entry.clone());
                param_map.insert(info.id.clone(), entry);
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

                let mut param = FloatParam::new(&info.name, info.default_value as f32, range);
                if !info.realtime {
                    param = param.hide();
                }
                float_params.push(param);
                let entry = ParamEntry {
                    kind: ParamKind::Float,
                    index: idx,
                    id: ParameterId::from(info.id.as_str()),
                    realtime: info.realtime,
                };
                sync_entries.push(entry.clone());
                param_map.insert(info.id.clone(), entry);
            }
        }

        Arc::new(Self {
            float_params,
            bool_params,
            int_params,
            param_map,
            sync_entries,
        })
    }

    /// Sync all parameter values to a SOTF plugin.
    pub fn sync_to_plugin(&self, plugin: &mut dyn sotf_host::plugin::Plugin) {
        for entry in self.sync_entries.iter().filter(|entry| entry.realtime) {
            let value = match entry.kind {
                ParamKind::Float => ParameterValue::Float(self.float_params[entry.index].value()),
                ParamKind::Bool => ParameterValue::Bool(self.bool_params[entry.index].value()),
                ParamKind::Int => ParameterValue::Int(self.int_params[entry.index].value()),
            };
            if plugin.get_parameter(&entry.id).as_ref() != Some(&value) {
                let _ = plugin.set_parameter(entry.id.clone(), value);
            }
        }
    }

    /// Allocation-free identity for construction-sized parameters. A change
    /// while active requires the host to deactivate/reactivate the instance;
    /// the render thread must never rebuild or destroy the DSP graph.
    #[cfg(any(feature = "linear-phase-eq", test))]
    pub(crate) fn structural_fingerprint(&self) -> u64 {
        let mut fingerprint = 0xcbf2_9ce4_8422_2325_u64;
        for entry in self.sync_entries.iter().filter(|entry| !entry.realtime) {
            let bits = match entry.kind {
                ParamKind::Float => self.float_params[entry.index].value().to_bits() as u64,
                ParamKind::Bool => u64::from(self.bool_params[entry.index].value()),
                ParamKind::Int => self.int_params[entry.index].value() as u32 as u64,
            };
            fingerprint ^= bits;
            fingerprint = fingerprint.wrapping_mul(0x100_0000_01b3);
        }
        fingerprint
    }

    #[cfg(any(feature = "linear-phase-eq", test))]
    fn value(&self, id: &str) -> Option<ParameterValue> {
        let entry = self.param_map.get(id)?;
        Some(match entry.kind {
            ParamKind::Float => ParameterValue::Float(self.float_params[entry.index].value()),
            ParamKind::Bool => ParameterValue::Bool(self.bool_params[entry.index].value()),
            ParamKind::Int => ParameterValue::Int(self.int_params[entry.index].value()),
        })
    }

    /// Build the construction-sized LinearPhaseEQ configuration represented by
    /// NIH's non-automatable parameters. Hosts apply these values when they
    /// recreate/initialize the plugin, never from the render callback.
    #[cfg(any(feature = "linear-phase-eq", test))]
    pub(crate) fn linear_phase_eq_config_json(&self) -> Result<String, String> {
        let int_value = |id: &str| match self.value(id) {
            Some(ParameterValue::Int(value)) => Ok(value),
            _ => Err(format!("missing LinearPhaseEQ integer parameter '{id}'")),
        };
        let float_value = |id: &str| match self.value(id) {
            Some(ParameterValue::Float(value)) => Ok(value),
            _ => Err(format!("missing LinearPhaseEQ float parameter '{id}'")),
        };
        let bool_value = |id: &str| match self.value(id) {
            Some(ParameterValue::Bool(value)) => Ok(value),
            _ => Err(format!("missing LinearPhaseEQ boolean parameter '{id}'")),
        };

        let num_filters = usize::try_from(int_value("num_filters")?)
            .map_err(|_| "LinearPhaseEQ num_filters must be positive".to_string())?;
        let filter_types = ["Peak", "Lowshelf", "Highshelf", "Lowpass", "Highpass"];
        let mut filters = Vec::with_capacity(num_filters);
        for index in 0..num_filters {
            let type_index = usize::try_from(int_value(&format!("band_{index}_type"))?)
                .map_err(|_| format!("LinearPhaseEQ band {index} type must be non-negative"))?;
            let filter_type = filter_types
                .get(type_index)
                .ok_or_else(|| format!("LinearPhaseEQ band {index} type is out of range"))?;
            filters.push(serde_json::json!({
                "filter_type": filter_type,
                "frequency": float_value(&format!("band_{index}_freq"))?,
                "q": float_value(&format!("band_{index}_q"))?,
                "gain_db": float_value(&format!("band_{index}_gain"))?,
                "active": bool_value(&format!("band_{index}_active"))?,
            }));
        }

        serde_json::to_string(&serde_json::json!({
            "num_filters": num_filters,
            "fir_length_index": int_value("fir_length")?,
            "phase_mode_index": int_value("phase_mode")?,
            "auto_gain": bool_value("auto_gain")?,
            "mix": float_value("mix")?,
            "filters": filters,
        }))
        .map_err(|error| format!("failed to serialize LinearPhaseEQ parameters: {error}"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_host::plugin::Plugin;

    fn linear_phase_infos() -> Vec<BridgedParamInfo> {
        let bridge = plugins_bridge::param_bridge::ParamBridge::new(
            sotf_plugins::param_specs::linear_phase_eq::PARAMS,
        );
        let mut infos = (0..bridge.count())
            .filter_map(|index| bridge.info(index))
            .collect::<Vec<_>>();
        let plugin =
            plugins_bridge::create_plugin("LinearPhaseEQ", 2, 48_000, r#"{"num_filters":10}"#)
                .unwrap();
        for parameter in plugin.parameters() {
            if infos.iter().any(|info| info.id == parameter.id.as_str()) {
                continue;
            }
            if let Some(info) = crate::wrapper::bridged_info_from_parameter(&parameter) {
                infos.push(info);
            }
        }
        infos
    }

    #[test]
    fn structural_linear_state_builds_adapter_with_matching_latency_and_bands() {
        let mut infos = linear_phase_infos();
        for info in &mut infos {
            info.default_value = match info.id.as_str() {
                "num_filters" => 1.0,
                "fir_length" => 3.0,
                "phase_mode" => 1.0,
                "auto_gain" => 1.0,
                "mix" => 0.25,
                "band_0_type" => 2.0,
                "band_0_freq" => 2_000.0,
                "band_0_q" => 2.0,
                "band_0_gain" => 6.0,
                "band_0_active" => 0.0,
                _ => info.default_value,
            };
        }
        let params = DynamicParams::from_infos(&infos);
        let config = params.linear_phase_eq_config_json().unwrap();
        let inner = plugins_bridge::create_plugin("LinearPhaseEQ", 2, 48_000, &config).unwrap();
        let inner_latency = inner.latency_samples();
        let expected_adapter_latency = 2 * inner.realtime_quantum_frames().max(64);
        let mut adapter = sotf_host::AsyncTimelinePlugin::new(inner, 48_000, 64).unwrap();

        assert_eq!(
            adapter.latency_samples(),
            inner_latency + expected_adapter_latency
        );
        assert_eq!(
            adapter.get_parameter(&ParameterId::from("fir_length")),
            Some(ParameterValue::Int(3))
        );
        assert_eq!(
            adapter.get_parameter(&ParameterId::from("phase_mode")),
            Some(ParameterValue::Int(1))
        );
        assert_eq!(
            adapter.get_parameter(&ParameterId::from("band_0_type")),
            Some(ParameterValue::Int(2))
        );
        assert_eq!(
            adapter.get_parameter(&ParameterId::from("band_0_gain")),
            Some(ParameterValue::Float(6.0))
        );
        assert_eq!(
            adapter.get_parameter(&ParameterId::from("band_0_active")),
            Some(ParameterValue::Bool(false))
        );

        // Render synchronization only forwards realtime parameters. The
        // construction-sized values above therefore cannot be silently
        // rejected or diverge on the callback.
        params.sync_to_plugin(&mut adapter);
        assert_eq!(
            adapter.get_parameter(&ParameterId::from("fir_length")),
            Some(ParameterValue::Int(3))
        );
    }

    #[test]
    fn linear_structural_parameters_are_not_realtime_automation_entries() {
        let params = DynamicParams::from_infos(&linear_phase_infos());
        for id in [
            "num_filters",
            "fir_length",
            "phase_mode",
            "auto_gain",
            "band_0_gain",
        ] {
            let entry = params.param_map.get(id).unwrap();
            assert!(!entry.realtime, "{id}");
            let flags = match entry.kind {
                ParamKind::Float => params.float_params[entry.index].flags(),
                ParamKind::Bool => params.bool_params[entry.index].flags(),
                ParamKind::Int => params.int_params[entry.index].flags(),
            };
            assert!(flags.contains(ParamFlags::HIDDEN), "{id}");
        }
        assert!(params.param_map.get("mix").unwrap().realtime);
    }

    #[test]
    fn structural_fingerprint_changes_with_restored_constructor_state() {
        let baseline_infos = linear_phase_infos();
        let baseline = DynamicParams::from_infos(&baseline_infos);
        let mut changed_infos = baseline_infos;
        changed_infos
            .iter_mut()
            .find(|info| info.id == "fir_length")
            .unwrap()
            .default_value = 4.0;
        let changed = DynamicParams::from_infos(&changed_infos);
        assert_ne!(
            baseline.structural_fingerprint(),
            changed.structural_fingerprint()
        );
    }

    #[test]
    fn ten_band_restored_state_has_a_complete_constructor_schema() {
        let mut infos = linear_phase_infos();
        assert!(infos.iter().any(|info| info.id == "band_9_gain"));
        for info in &mut infos {
            info.default_value = match info.id.as_str() {
                "num_filters" => 10.0,
                "band_9_type" => 2.0,
                "band_9_freq" => 12_000.0,
                "band_9_q" => 1.25,
                "band_9_gain" => 3.5,
                "band_9_active" => 1.0,
                _ => info.default_value,
            };
        }
        let params = DynamicParams::from_infos(&infos);
        let config = params.linear_phase_eq_config_json().unwrap();
        let mut plugin = plugins_bridge::create_plugin("LinearPhaseEQ", 2, 48_000, &config)
            .expect("ten-band restored state must reconstruct");
        plugin.initialize(48_000).unwrap();
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("num_filters")),
            Some(ParameterValue::Int(10))
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("band_9_gain")),
            Some(ParameterValue::Float(3.5))
        );
    }
}
