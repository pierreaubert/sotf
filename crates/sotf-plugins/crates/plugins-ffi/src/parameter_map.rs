// ============================================================================
// Parameter Mapping System - Delegates to plugins-bridge ParamBridge
// ============================================================================
//
// Maps plugin parameters to a generic C-compatible parameter system for AU hosts.
// Uses ParamBridge from plugins-bridge for normalization and metadata.

use plugins_bridge::ParamBridge;
use sotf_host::plugin::Plugin;
use std::ffi::CString;
use std::os::raw::c_char;

/// Parameter information exposed to AU host
#[repr(C)]
#[derive(Debug, Clone)]
pub struct ParameterInfo {
    /// Unique parameter ID (e.g., "threshold_db")
    pub id: *const c_char,
    /// Human-readable name (e.g., "Threshold")
    pub name: *const c_char,
    /// Unit string (e.g., "Hz", "dB", "")
    pub unit: *const c_char,
    /// Minimum value
    pub min_value: f64,
    /// Maximum value
    pub max_value: f64,
    /// Default value
    pub default_value: f64,
    /// Number of steps (0 = continuous)
    pub steps: u32,
    /// Whether this parameter uses logarithmic scaling
    pub logarithmic: bool,
}

/// Parameter mapping for a plugin, backed by plugins-bridge ParamBridge.
pub struct ParameterMap {
    bridge: ParamBridge,
    /// Cached C-compatible info structs (leaked CStrings for FFI safety)
    cached_infos: Vec<CachedParamInfo>,
}

struct CachedParamInfo {
    id: *const c_char,
    name: *const c_char,
    unit: *const c_char,
    min_value: f64,
    max_value: f64,
    default_value: f64,
    steps: u32,
    logarithmic: bool,
}

impl ParameterMap {
    /// Create parameter map from a plugin using its ParamSpec definitions.
    pub fn from_plugin(plugin: &dyn Plugin, plugin_type: &str) -> Self {
        let specs = get_param_specs(plugin_type);
        let bridge = ParamBridge::new(specs);

        // Pre-build cached C-compatible info structs
        let mut cached_infos = Vec::with_capacity(bridge.count());
        for i in 0..bridge.count() {
            if let Some(info) = bridge.info(i) {
                // Leak CStrings intentionally for FFI safety — they live as long as the ParameterMap
                let id = CString::new(info.id).unwrap().into_raw() as *const c_char;
                let name = CString::new(info.name).unwrap().into_raw() as *const c_char;
                let unit = CString::new(info.unit).unwrap().into_raw() as *const c_char;

                cached_infos.push(CachedParamInfo {
                    id,
                    name,
                    unit,
                    min_value: info.min_value,
                    max_value: info.max_value,
                    default_value: info.default_value,
                    steps: info.steps,
                    logarithmic: info.logarithmic,
                });
            }
        }

        // Also add parameters from Plugin::parameters() that aren't in ParamSpec
        // (for plugins without PARAMS arrays, fallback to the plugin's own parameter list)
        if cached_infos.is_empty() {
            for param in plugin.parameters() {
                let (min, max, default) = match (&param.min_value, &param.max_value, &param.default_value) {
                    (
                        Some(sotf_host::parameters::ParameterValue::Float(min)),
                        Some(sotf_host::parameters::ParameterValue::Float(max)),
                        sotf_host::parameters::ParameterValue::Float(def),
                    ) => (*min as f64, *max as f64, *def as f64),
                    (
                        Some(sotf_host::parameters::ParameterValue::Int(min)),
                        Some(sotf_host::parameters::ParameterValue::Int(max)),
                        sotf_host::parameters::ParameterValue::Int(def),
                    ) => (*min as f64, *max as f64, *def as f64),
                    _ => (0.0, 1.0, 0.0),
                };

                let id = CString::new(param.id.0.clone()).unwrap().into_raw() as *const c_char;
                let name = CString::new(param.name.clone()).unwrap().into_raw() as *const c_char;
                let unit = CString::new(param.unit.clone()).unwrap().into_raw() as *const c_char;

                cached_infos.push(CachedParamInfo {
                    id,
                    name,
                    unit,
                    min_value: min,
                    max_value: max,
                    default_value: default,
                    steps: 0,
                    logarithmic: param.logarithmic,
                });
            }
        }

        Self {
            bridge,
            cached_infos,
        }
    }

    /// Get the number of parameters.
    pub fn count(&self) -> usize {
        self.cached_infos.len()
    }

    /// Get parameter info by index.
    pub fn get_info(&self, index: usize) -> Option<ParameterInfo> {
        self.cached_infos.get(index).map(|cached| ParameterInfo {
            id: cached.id,
            name: cached.name,
            unit: cached.unit,
            min_value: cached.min_value,
            max_value: cached.max_value,
            default_value: cached.default_value,
            steps: cached.steps,
            logarithmic: cached.logarithmic,
        })
    }

    /// Set parameter value (normalized 0.0-1.0).
    pub fn set_normalized(
        &self,
        plugin: &mut dyn Plugin,
        param_id: &str,
        normalized_value: f64,
    ) -> Result<(), String> {
        // Try ParamBridge first
        if let Some(index) = self.bridge.find_index(param_id) {
            return self
                .bridge
                .set_normalized(plugin, index, normalized_value);
        }

        // Fallback: direct set using raw parameter system
        // Denormalize using cached info
        if let Some(pos) = self.cached_infos.iter().position(|c| {
            let id = unsafe { std::ffi::CStr::from_ptr(c.id).to_str().unwrap_or("") };
            id == param_id
        }) {
            let cached = &self.cached_infos[pos];
            let raw = cached.min_value
                + (normalized_value * (cached.max_value - cached.min_value));
            let id = sotf_host::parameters::ParameterId(param_id.to_string());
            let value = sotf_host::parameters::ParameterValue::Float(raw as f32);
            plugin.set_parameter(id, value)
        } else {
            Err(format!("Unknown parameter: {param_id}"))
        }
    }

    /// Get parameter value (normalized 0.0-1.0).
    pub fn get_normalized(&self, plugin: &dyn Plugin, param_id: &str) -> Option<f64> {
        // Try ParamBridge first
        if let Some(index) = self.bridge.find_index(param_id) {
            return self.bridge.get_normalized(plugin, index);
        }

        // Fallback: direct get using raw parameter system
        let pos = self.cached_infos.iter().position(|c| {
            let id = unsafe { std::ffi::CStr::from_ptr(c.id).to_str().unwrap_or("") };
            id == param_id
        })?;

        let cached = &self.cached_infos[pos];
        let id = sotf_host::parameters::ParameterId(param_id.to_string());
        let value = plugin.get_parameter(&id)?;
        let raw = match value {
            sotf_host::parameters::ParameterValue::Float(f) => f as f64,
            sotf_host::parameters::ParameterValue::Int(i) => i as f64,
            sotf_host::parameters::ParameterValue::Bool(b) => if b { 1.0 } else { 0.0 },
            _ => return None,
        };
        let range = cached.max_value - cached.min_value;
        if range.abs() < f64::EPSILON {
            return Some(0.0);
        }
        Some(((raw - cached.min_value) / range).clamp(0.0, 1.0))
    }
}

impl Drop for ParameterMap {
    fn drop(&mut self) {
        // Reclaim the leaked CStrings
        for info in &self.cached_infos {
            unsafe {
                if !info.id.is_null() {
                    drop(CString::from_raw(info.id as *mut c_char));
                }
                if !info.name.is_null() {
                    drop(CString::from_raw(info.name as *mut c_char));
                }
                if !info.unit.is_null() {
                    drop(CString::from_raw(info.unit as *mut c_char));
                }
            }
        }
    }
}

/// Get the ParamSpec array for a given plugin type.
fn get_param_specs(plugin_type: &str) -> &'static [sotf_host::param_specs::ParamSpec] {
    use sotf_plugins::param_specs::*;

    match plugin_type {
        // EQ has GLOBAL_PARAMS + per-band BAND_TEMPLATE (dynamic bands)
        // Expose global params; band params come from Plugin::parameters() fallback
        "EQ" | "eq" => eq::GLOBAL_PARAMS,
        "Compressor" | "compressor" => compressor::PARAMS,
        "Limiter" | "limiter" => limiter::PARAMS,
        "Gate" | "gate" => gate::PARAMS,
        "Gain" | "gain" => gain::PARAMS,
        "Expander" | "expander" => expander::PARAMS,
        "Crossfeed" | "crossfeed" => crossfeed::PARAMS,
        "FletcherMunson" | "fletcher_munson" => fletcher_munson::PARAMS,
        "LoudnessCompensation" | "loudness_compensation" => loudness_compensation::PARAMS,
        // Multiband plugins have GLOBAL_PARAMS + per-band params (dynamic)
        "MultibandCompressor" | "multiband_compressor" => multiband_compressor::GLOBAL_PARAMS,
        "MultibandExpander" | "multiband_expander" => multiband_expander::GLOBAL_PARAMS,
        "Upmixer" | "upmixer" => upmixer::PARAMS,
        "XTC" | "xtc" => xtc::PARAMS,
        "Binaural" | "binaural" => binaural::PARAMS,
        "ChannelMuteSolo" | "channel_mute_solo" => channel_mute_solo::PARAMS,
        "Convolution" | "convolution" => convolution::PARAMS,
        "ABCompare" | "ab_compare" => ab_compare::PARAMS,
        "MonoToStereo" | "mono_to_stereo" => mono_to_stereo::PARAMS,
        "PND" | "pnd" => pnd::PARAMS,
        "Denoiser" | "denoiser" => denoiser::PARAMS,
        "Downmix" | "downmix" => downmix::PARAMS,
        other => panic!("get_param_specs: unknown plugin type \"{other}\" — add it to the match arm"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_map_eq() {
        let plugin = plugins_bridge::create_plugin("EQ", 2, 48000, "{}").unwrap();
        let param_map = ParameterMap::from_plugin(&*plugin, "EQ");
        assert!(param_map.count() > 0);
    }

    #[test]
    fn test_parameter_map_compressor() {
        let plugin = plugins_bridge::create_plugin("Compressor", 2, 48000, "{}").unwrap();
        let param_map = ParameterMap::from_plugin(&*plugin, "Compressor");
        assert!(param_map.count() > 0);

        // Check we can get info
        let info = param_map.get_info(0).unwrap();
        assert!(!info.id.is_null());
        assert!(!info.name.is_null());
    }
}
