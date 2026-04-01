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
    /// Cached C-compatible info structs (leaked CStrings for FFI safety).
    /// Stored as `ParameterInfo` directly so we can return stable pointers via `get_info()`.
    cached_infos: Vec<ParameterInfo>,
}

impl ParameterMap {
    /// Create parameter map from a plugin using its ParamSpec definitions.
    pub fn from_plugin(plugin: &dyn Plugin, plugin_type: &str) -> Self {
        let specs = get_param_specs(plugin_type);
        let bridge = ParamBridge::new(specs);

        // Pre-build cached C-compatible info structs from static ParamSpec
        let mut cached_infos = Vec::with_capacity(bridge.count());
        for i in 0..bridge.count() {
            if let Some(info) = bridge.info(i) {
                let id = CString::new(info.id).unwrap().into_raw() as *const c_char;
                let name = CString::new(info.name).unwrap().into_raw() as *const c_char;
                let unit = CString::new(info.unit).unwrap().into_raw() as *const c_char;

                cached_infos.push(ParameterInfo {
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

        // Expand per-band templates for plugins with dynamic bands (EQ, multiband, etc.)
        // Band parameters use "band_N_field" naming convention matching the Rust plugin's
        // set_parameter/get_parameter interface.
        if let Some((template, max_bands)) = get_band_template(plugin_type) {
            expand_band_params(&mut cached_infos, template, max_bands);
        }

        // Fallback: if no static specs produced params, use Plugin::parameters()
        if cached_infos.is_empty() {
            for param in plugin.parameters() {
                let (min, max, default) =
                    match (&param.min_value, &param.max_value, &param.default_value) {
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

                cached_infos.push(ParameterInfo {
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
    ///
    /// Returns a reference to the cached info, which is valid for the lifetime of this ParameterMap.
    /// This is critical for FFI safety — callers can convert the reference to a raw pointer that
    /// remains valid as long as the PluginHandle (and thus this ParameterMap) is alive.
    pub fn get_info(&self, index: usize) -> Option<&ParameterInfo> {
        self.cached_infos.get(index)
    }

    /// Get the param_id string for a given index.
    pub fn param_id_at(&self, index: usize) -> Option<&str> {
        self.cached_infos.get(index).map(|info| {
            // SAFETY: id was created from CString::into_raw and is valid for the lifetime of self
            unsafe { std::ffi::CStr::from_ptr(info.id).to_str().unwrap_or("") }
        })
    }

    /// Get denormalized parameter value by index.
    ///
    /// Returns the raw value in parameter units (Hz, dB, etc.).
    /// Uses the `ParamBridge` for correct scaling (log for Hz, linear for others).
    pub fn get_denormalized_by_index(&self, plugin: &dyn Plugin, index: usize) -> Option<f64> {
        let info = self.cached_infos.get(index)?;
        let param_id = unsafe { std::ffi::CStr::from_ptr(info.id).to_str().unwrap_or("") };
        let normalized = self.get_normalized(plugin, param_id)?;
        // Use bridge for correct denormalization (handles log scaling for Hz params)
        self.bridge.denormalize(index, normalized).or(Some(
            info.min_value + normalized * (info.max_value - info.min_value),
        ))
    }

    /// Set denormalized parameter value by index.
    ///
    /// Takes the raw value in parameter units (Hz, dB, etc.) and normalizes internally.
    /// Uses the `ParamBridge` for correct scaling (log for Hz, linear for others).
    pub fn set_denormalized_by_index(
        &self,
        plugin: &mut dyn Plugin,
        index: usize,
        value: f64,
    ) -> Result<(), String> {
        let info = self
            .cached_infos
            .get(index)
            .ok_or_else(|| format!("Parameter index {index} out of range"))?;
        let param_id = unsafe { std::ffi::CStr::from_ptr(info.id).to_str().unwrap_or("") };
        // Use bridge for correct normalization (handles log scaling for Hz params)
        let normalized = self.bridge.normalize(index, value).unwrap_or_else(|| {
            // Fallback: linear normalization
            let range = info.max_value - info.min_value;
            if range.abs() < f64::EPSILON {
                0.0
            } else {
                ((value - info.min_value) / range).clamp(0.0, 1.0)
            }
        });
        self.set_normalized(plugin, param_id, normalized)
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
            return self.bridge.set_normalized(plugin, index, normalized_value);
        }

        // Fallback: direct set using raw parameter system
        // Denormalize using cached info (log scaling for Hz params, linear for others)
        if let Some(pos) = self.cached_infos.iter().position(|info| {
            let id = unsafe { std::ffi::CStr::from_ptr(info.id).to_str().unwrap_or("") };
            id == param_id
        }) {
            let info = &self.cached_infos[pos];
            let raw = if info.logarithmic && info.min_value > 0.0 {
                let log_min = info.min_value.ln();
                let log_max = info.max_value.ln();
                (log_min + normalized_value * (log_max - log_min)).exp()
            } else {
                info.min_value + (normalized_value * (info.max_value - info.min_value))
            };
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
        let pos = self.cached_infos.iter().position(|info| {
            let id = unsafe { std::ffi::CStr::from_ptr(info.id).to_str().unwrap_or("") };
            id == param_id
        })?;

        let info = &self.cached_infos[pos];
        let id = sotf_host::parameters::ParameterId(param_id.to_string());
        let value = plugin.get_parameter(&id)?;
        let raw = match value {
            sotf_host::parameters::ParameterValue::Float(f) => f as f64,
            sotf_host::parameters::ParameterValue::Int(i) => i as f64,
            sotf_host::parameters::ParameterValue::Bool(b) => {
                if b {
                    1.0
                } else {
                    0.0
                }
            }
            _ => return None,
        };
        if info.logarithmic && info.min_value > 0.0 {
            let log_min = info.min_value.ln();
            let log_max = info.max_value.ln();
            let log_val = raw.clamp(info.min_value, info.max_value).ln();
            Some(((log_val - log_min) / (log_max - log_min)).clamp(0.0, 1.0))
        } else {
            let range = info.max_value - info.min_value;
            if range.abs() < f64::EPSILON {
                return Some(0.0);
            }
            Some(((raw - info.min_value) / range).clamp(0.0, 1.0))
        }
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

/// Get the band template and max band count for plugins with per-band parameters.
/// Returns None for plugins without dynamic bands.
fn get_band_template(
    plugin_type: &str,
) -> Option<(&'static [sotf_host::param_specs::ParamSpec], usize)> {
    use sotf_plugins::param_specs::*;

    match plugin_type {
        "EQ" | "eq" => Some((eq::BAND_TEMPLATE, 20)),
        "MultibandCompressor" | "multiband_compressor" => {
            Some((multiband_compressor::BAND_TEMPLATE, 5))
        }
        "MultibandExpander" | "multiband_expander" => Some((multiband_expander::BAND_TEMPLATE, 5)),
        "DynamicEQ" | "dynamic_eq" => Some((dynamic_eq::BAND_PARAMS, 8)),
        "LinearPhaseEQ" | "linear_phase_eq" => Some((linear_phase_eq::BAND_TEMPLATE, 20)),
        _ => None,
    }
}

/// Expand a per-band ParamSpec template into concrete ParameterInfo entries.
///
/// For each band 0..max_bands, creates parameters with IDs like "band_0_frequency",
/// "band_1_q", etc. — matching the naming convention used by the Rust plugins'
/// set_parameter/get_parameter implementations.
fn expand_band_params(
    cached_infos: &mut Vec<ParameterInfo>,
    template: &[sotf_host::param_specs::ParamSpec],
    max_bands: usize,
) {
    use sotf_host::param_specs::ParamType;

    for band_idx in 0..max_bands {
        for spec in template {
            let band_id = format!("band_{}_{}", band_idx, spec.engine_key);
            let band_name = format!("Band {} {}", band_idx + 1, spec.name);

            let (min, max, default, steps, logarithmic) = match spec.param_type {
                ParamType::Float {
                    default,
                    min,
                    max,
                    step,
                } => {
                    let steps = if step > 0.0 {
                        ((max - min) / step) as u32
                    } else {
                        0
                    };
                    // Hz params with positive min use logarithmic scaling
                    let is_log = spec.unit == "Hz" && min > 0.0;
                    (min, max, default, steps, is_log)
                }
                ParamType::Int {
                    default,
                    min,
                    max,
                    step,
                } => (
                    min as f64,
                    max as f64,
                    default as f64,
                    ((max - min) / step) as u32,
                    false,
                ),
                ParamType::Bool { default, .. } => {
                    (0.0, 1.0, if default { 1.0 } else { 0.0 }, 1, false)
                }
                ParamType::Choice {
                    default_index,
                    labels,
                } => (
                    0.0,
                    (labels.len().saturating_sub(1)) as f64,
                    default_index as f64,
                    labels.len().saturating_sub(1) as u32,
                    false,
                ),
                ParamType::FilePath => continue, // skip file paths for AU
            };

            let id = CString::new(band_id).unwrap().into_raw() as *const c_char;
            let name = CString::new(band_name).unwrap().into_raw() as *const c_char;
            let unit = CString::new(spec.unit).unwrap().into_raw() as *const c_char;

            cached_infos.push(ParameterInfo {
                id,
                name,
                unit,
                min_value: min,
                max_value: max,
                default_value: default,
                steps,
                logarithmic,
            });
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
        "FletcherMunson" | "fletcher_munson" => loudness_compensation::PARAMS,
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
        "Saturation" | "saturation" => saturation::PARAMS,
        "StereoImager" | "stereo_imager" => stereo_imager::PARAMS,
        "TransientShaper" | "transient_shaper" => transient_shaper::PARAMS,
        "DeEsser" | "de_esser" => de_esser::PARAMS,
        "DynamicEQ" | "dynamic_eq" => dynamic_eq::PARAMS,
        "LinearPhaseEQ" | "linear_phase_eq" => linear_phase_eq::PARAMS,
        "Dither" | "dither" => dither::PARAMS,
        // Plugins without param_specs entries fall back to Plugin::parameters() in from_plugin()
        "Delay" | "delay" | "Matrix" | "matrix" | "Crossover" | "crossover" | "Resampler"
        | "resampler" => &[],
        other => {
            panic!("get_param_specs: unknown plugin type \"{other}\" — add it to the match arm")
        }
    }
}

/// Get the global (non-band) ParamSpec array for a plugin type.
/// Used by `AuHostState` to determine the global param offset for band-based plugins.
pub fn global_param_specs(plugin_type: &str) -> &'static [sotf_host::param_specs::ParamSpec] {
    get_param_specs(plugin_type)
}

/// Get band template info for a plugin type: `(params_per_band, max_bands)`.
/// Returns `None` for plugins without dynamic bands.
pub fn band_template_info(plugin_type: &str) -> Option<(usize, usize)> {
    get_band_template(plugin_type).map(|(template, max_bands)| (template.len(), max_bands))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_map_eq() {
        let plugin = plugins_bridge::create_plugin("EQ", 2, 48000, "{}").unwrap();
        let param_map = ParameterMap::from_plugin(&*plugin, "EQ");
        // 3 global (max_filters, tdf2, topology) + 20 bands × 4 params (frequency, q, gain_db, filter_type)
        assert_eq!(param_map.count(), 3 + 20 * 4);
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
