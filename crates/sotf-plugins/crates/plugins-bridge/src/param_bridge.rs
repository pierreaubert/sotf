//! Generic parameter normalization bridge using ParamSpec.
//!
//! Converts between normalized (0.0-1.0) values used by AU/VST3 hosts
//! and raw plugin parameter values.

use sotf_host::param_specs::{ParamSpec, ParamType};
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::Plugin;

/// Bridge between AU/VST3 normalized parameters and SOTF plugin parameters.
///
/// Built from a slice of `ParamSpec` (the single source of truth for parameter metadata).
pub struct ParamBridge {
    specs: Vec<ParamSpec>,
}

/// Information about a single parameter, suitable for FFI export.
#[derive(Debug, Clone)]
pub struct BridgedParamInfo {
    /// Unique parameter ID (matches ParamSpec.engine_key)
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Unit string ("dB", "Hz", "%", "")
    pub unit: String,
    /// Minimum value (raw, not normalized)
    pub min_value: f64,
    /// Maximum value (raw, not normalized)
    pub max_value: f64,
    /// Default value (raw, not normalized)
    pub default_value: f64,
    /// Number of discrete steps (0 = continuous)
    pub steps: u32,
    /// Whether this parameter should use logarithmic scaling in UI
    pub logarithmic: bool,
    /// Group name for UI organization
    pub group: String,
}

impl ParamBridge {
    /// Create a ParamBridge from a ParamSpec array.
    pub fn new(specs: &[ParamSpec]) -> Self {
        Self {
            specs: specs.to_vec(),
        }
    }

    /// Number of parameters.
    pub fn count(&self) -> usize {
        self.specs.len()
    }

    /// Get parameter info by index.
    pub fn info(&self, index: usize) -> Option<BridgedParamInfo> {
        self.specs.get(index).map(|spec| {
            let (min, max, default, steps, logarithmic) = match spec.param_type {
                ParamType::Float {
                    default,
                    min,
                    max,
                    step,
                } => {
                    let steps = if step > 0.0 {
                        ((max - min) / step).round() as u32
                    } else {
                        0
                    };
                    // Frequency parameters are typically logarithmic
                    let is_log = spec.unit.eq_ignore_ascii_case("Hz");
                    (min, max, default, steps, is_log)
                }
                ParamType::Int {
                    default,
                    min,
                    max,
                    step,
                } => {
                    let steps = if step > 0 {
                        ((max - min) / step) as u32
                    } else {
                        (max - min) as u32
                    };
                    (min as f64, max as f64, default as f64, steps, false)
                }
                ParamType::Bool { default, .. } => {
                    (0.0, 1.0, if default { 1.0 } else { 0.0 }, 1, false)
                }
                ParamType::Choice {
                    default_index,
                    labels,
                } => {
                    let max = if labels.is_empty() {
                        0.0
                    } else {
                        (labels.len() - 1) as f64
                    };
                    (0.0, max, default_index as f64, labels.len() as u32, false)
                }
                ParamType::FilePath => (0.0, 1.0, 0.0, 0, false),
            };

            BridgedParamInfo {
                id: spec.engine_key.to_string(),
                name: spec.name.to_string(),
                unit: spec.unit.to_string(),
                min_value: min,
                max_value: max,
                default_value: default,
                steps,
                logarithmic,
                group: spec.group.to_string(),
            }
        })
    }

    /// Get ParamSpec by index.
    pub fn spec(&self, index: usize) -> Option<&ParamSpec> {
        self.specs.get(index)
    }

    /// Find parameter index by engine_key.
    pub fn find_index(&self, engine_key: &str) -> Option<usize> {
        self.specs.iter().position(|s| s.engine_key == engine_key)
    }

    /// Normalize a raw value to 0.0-1.0 range.
    pub fn normalize(&self, index: usize, raw_value: f64) -> Option<f64> {
        let spec = self.specs.get(index)?;
        Some(normalize_value(spec, raw_value))
    }

    /// Denormalize a 0.0-1.0 value to the raw parameter range.
    pub fn denormalize(&self, index: usize, normalized: f64) -> Option<f64> {
        let spec = self.specs.get(index)?;
        Some(denormalize_value(spec, normalized))
    }

    /// Set a normalized parameter value (0.0-1.0) on a plugin.
    pub fn set_normalized(
        &self,
        plugin: &mut dyn Plugin,
        index: usize,
        normalized: f64,
    ) -> Result<(), String> {
        let spec = self
            .specs
            .get(index)
            .ok_or_else(|| format!("Parameter index {index} out of range"))?;

        let raw = denormalize_value(spec, normalized);
        let value = raw_to_parameter_value(spec, raw);
        let id = ParameterId(spec.engine_key.to_string());
        plugin.set_parameter(id, value)
    }

    /// Get a normalized parameter value (0.0-1.0) from a plugin.
    pub fn get_normalized(&self, plugin: &dyn Plugin, index: usize) -> Option<f64> {
        let spec = self.specs.get(index)?;
        let id = ParameterId(spec.engine_key.to_string());
        let value = plugin.get_parameter(&id)?;
        let raw = parameter_value_to_raw(&value);
        Some(normalize_value(spec, raw))
    }

    /// Set a raw parameter value on a plugin.
    pub fn set_raw(
        &self,
        plugin: &mut dyn Plugin,
        engine_key: &str,
        raw_value: f64,
    ) -> Result<(), String> {
        let spec = self
            .specs
            .iter()
            .find(|s| s.engine_key == engine_key)
            .ok_or_else(|| format!("Unknown parameter: {engine_key}"))?;

        let clamped = spec.clamp_f64(raw_value);
        let value = raw_to_parameter_value(spec, clamped);
        let id = ParameterId(engine_key.to_string());
        plugin.set_parameter(id, value)
    }

    /// Get a raw parameter value from a plugin.
    pub fn get_raw(&self, plugin: &dyn Plugin, engine_key: &str) -> Option<f64> {
        let id = ParameterId(engine_key.to_string());
        let value = plugin.get_parameter(&id)?;
        Some(parameter_value_to_raw(&value))
    }
}

/// Normalize a raw value to 0.0-1.0 based on the parameter spec.
fn normalize_value(spec: &ParamSpec, raw: f64) -> f64 {
    match spec.param_type {
        ParamType::Float { min, max, .. } => {
            if (max - min).abs() < f64::EPSILON {
                return 0.0;
            }
            if spec.unit.eq_ignore_ascii_case("Hz") && min > 0.0 {
                // Logarithmic scaling for frequency parameters
                let log_min = min.ln();
                let log_max = max.ln();
                let log_val = raw.clamp(min, max).ln();
                ((log_val - log_min) / (log_max - log_min)).clamp(0.0, 1.0)
            } else {
                ((raw - min) / (max - min)).clamp(0.0, 1.0)
            }
        }
        ParamType::Int { min, max, .. } => {
            if max == min {
                return 0.0;
            }
            ((raw - min as f64) / (max - min) as f64).clamp(0.0, 1.0)
        }
        ParamType::Bool { .. } => {
            if raw > 0.5 {
                1.0
            } else {
                0.0
            }
        }
        ParamType::Choice { labels, .. } => {
            if labels.len() <= 1 {
                return 0.0;
            }
            (raw / (labels.len() - 1) as f64).clamp(0.0, 1.0)
        }
        ParamType::FilePath => 0.0,
    }
}

/// Denormalize a 0.0-1.0 value to the raw parameter range.
fn denormalize_value(spec: &ParamSpec, normalized: f64) -> f64 {
    let n = normalized.clamp(0.0, 1.0);
    match spec.param_type {
        ParamType::Float { min, max, step, .. } => {
            let raw = if spec.unit.eq_ignore_ascii_case("Hz") && min > 0.0 {
                // Logarithmic scaling for frequency parameters
                let log_min = min.ln();
                let log_max = max.ln();
                (log_min + n * (log_max - log_min)).exp()
            } else {
                min + n * (max - min)
            };
            if step > 0.0 {
                (raw / step).round() * step
            } else {
                raw
            }
        }
        ParamType::Int { min, max, step, .. } => {
            let raw = min as f64 + n * (max - min) as f64;
            if step > 1 {
                ((raw / step as f64).round() * step as f64).clamp(min as f64, max as f64)
            } else {
                raw.round().clamp(min as f64, max as f64)
            }
        }
        ParamType::Bool { .. } => {
            if n >= 0.5 {
                1.0
            } else {
                0.0
            }
        }
        ParamType::Choice { labels, .. } => {
            if labels.is_empty() {
                return 0.0;
            }
            let index = (n * (labels.len() - 1) as f64).round() as usize;
            index.min(labels.len() - 1) as f64
        }
        ParamType::FilePath => 0.0,
    }
}

/// Convert a raw f64 value to a ParameterValue based on the spec type.
fn raw_to_parameter_value(spec: &ParamSpec, raw: f64) -> ParameterValue {
    match spec.param_type {
        ParamType::Float { .. } => ParameterValue::Float(raw as f32),
        ParamType::Int { .. } => ParameterValue::Int(raw.round() as i32),
        ParamType::Bool { .. } => ParameterValue::Bool(raw > 0.5),
        ParamType::Choice { .. } => ParameterValue::Int(raw.round() as i32),
        ParamType::FilePath => ParameterValue::String(String::new()),
    }
}

/// Extract a raw f64 from a ParameterValue.
fn parameter_value_to_raw(value: &ParameterValue) -> f64 {
    match value {
        ParameterValue::Float(f) => *f as f64,
        ParameterValue::Int(i) => *i as f64,
        ParameterValue::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        ParameterValue::String(_) => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_host::param_specs::ParamSpec;

    fn make_float_spec(min: f64, max: f64) -> ParamSpec {
        ParamSpec::float(
            "Test",
            "test",
            (min + max) / 2.0,
            min,
            max,
            0.0,
            "dB",
            "General",
        )
    }

    fn make_freq_spec() -> ParamSpec {
        ParamSpec::float("Frequency", "freq", 1000.0, 20.0, 20000.0, 0.0, "Hz", "EQ")
    }

    #[test]
    fn test_linear_normalize_roundtrip() {
        let spec = make_float_spec(-12.0, 12.0);
        let bridge = ParamBridge::new(&[spec]);

        // Min → 0.0
        assert!((bridge.normalize(0, -12.0).unwrap() - 0.0).abs() < 1e-10);
        // Max → 1.0
        assert!((bridge.normalize(0, 12.0).unwrap() - 1.0).abs() < 1e-10);
        // Mid → 0.5
        assert!((bridge.normalize(0, 0.0).unwrap() - 0.5).abs() < 1e-10);

        // Round-trip
        for raw in [-12.0, -6.0, 0.0, 3.0, 12.0] {
            let n = bridge.normalize(0, raw).unwrap();
            let back = bridge.denormalize(0, n).unwrap();
            assert!(
                (back - raw).abs() < 1e-6,
                "Round-trip failed for {raw}: got {back}"
            );
        }
    }

    #[test]
    fn test_log_normalize_roundtrip() {
        let spec = make_freq_spec();
        let bridge = ParamBridge::new(&[spec]);

        // Min → 0.0
        assert!((bridge.normalize(0, 20.0).unwrap() - 0.0).abs() < 1e-10);
        // Max → 1.0
        assert!((bridge.normalize(0, 20000.0).unwrap() - 1.0).abs() < 1e-10);
        // 1000 Hz should be roughly in the middle of log scale
        let n_1k = bridge.normalize(0, 1000.0).unwrap();
        assert!(n_1k > 0.4 && n_1k < 0.6, "1kHz normalized = {n_1k}");

        // Round-trip
        for raw in [20.0, 100.0, 1000.0, 10000.0, 20000.0] {
            let n = bridge.normalize(0, raw).unwrap();
            let back = bridge.denormalize(0, n).unwrap();
            assert!(
                (back - raw).abs() / raw < 1e-6,
                "Round-trip failed for {raw}: got {back}"
            );
        }
    }

    #[test]
    fn test_bool_normalize() {
        let spec = ParamSpec::bool_param("Bypass", "bypass", false, "General");
        let bridge = ParamBridge::new(&[spec]);

        assert!((bridge.normalize(0, 0.0).unwrap() - 0.0).abs() < 1e-10);
        assert!((bridge.normalize(0, 1.0).unwrap() - 1.0).abs() < 1e-10);
        assert!((bridge.denormalize(0, 0.0).unwrap() - 0.0).abs() < 1e-10);
        assert!((bridge.denormalize(0, 1.0).unwrap() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_choice_normalize() {
        let labels: &[&str] = &["Low", "Mid", "High"];
        let spec = ParamSpec::choice("Mode", "mode", 0, labels, "General");
        let bridge = ParamBridge::new(&[spec]);

        assert!((bridge.normalize(0, 0.0).unwrap() - 0.0).abs() < 1e-10);
        assert!((bridge.normalize(0, 1.0).unwrap() - 0.5).abs() < 1e-10);
        assert!((bridge.normalize(0, 2.0).unwrap() - 1.0).abs() < 1e-10);

        assert!((bridge.denormalize(0, 0.0).unwrap() - 0.0).abs() < 1e-10);
        assert!((bridge.denormalize(0, 0.5).unwrap() - 1.0).abs() < 1e-10);
        assert!((bridge.denormalize(0, 1.0).unwrap() - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_choice_normalize_degenerate_labels() {
        let empty = ParamSpec::choice("Mode", "mode", 0, &[], "General");
        let single = ParamSpec::choice("Mode", "mode", 0, &["Only"], "General");
        let bridge = ParamBridge::new(&[empty, single]);

        for index in [0, 1] {
            let normalized = bridge.normalize(index, 1.0).unwrap();
            assert!(
                normalized.is_finite(),
                "degenerate choice normalization must stay finite"
            );
            assert_eq!(normalized, 0.0);
        }
    }

    #[test]
    fn test_info() {
        let spec = make_float_spec(-24.0, 24.0);
        let bridge = ParamBridge::new(&[spec]);
        let info = bridge.info(0).unwrap();

        assert_eq!(info.id, "test");
        assert_eq!(info.name, "Test");
        assert_eq!(info.unit, "dB");
        assert!((info.min_value - (-24.0)).abs() < 1e-10);
        assert!((info.max_value - 24.0).abs() < 1e-10);
        assert!(!info.logarithmic);
    }

    #[test]
    fn test_hz_case_insensitive_logarithmic_scaling() {
        let spec_lower = ParamSpec::float("Freq", "freq", 1000.0, 20.0, 20000.0, 0.0, "hz", "EQ");
        let spec_upper = ParamSpec::float("Freq", "freq", 1000.0, 20.0, 20000.0, 0.0, "HZ", "EQ");
        let bridge = ParamBridge::new(&[spec_lower, spec_upper]);

        // info() should report logarithmic for both "hz" and "HZ"
        assert!(
            bridge.info(0).unwrap().logarithmic,
            "lowercase 'hz' should be logarithmic"
        );
        assert!(
            bridge.info(1).unwrap().logarithmic,
            "uppercase 'HZ' should be logarithmic"
        );

        // normalize should use log scale for case-insensitive Hz
        let n_lower = bridge.normalize(0, 1000.0).unwrap();
        let n_upper = bridge.normalize(1, 1000.0).unwrap();
        assert!(
            (n_lower - n_upper).abs() < 1e-10,
            "normalize should match for hz vs HZ"
        );
        assert!(
            n_lower > 0.4 && n_lower < 0.6,
            "1kHz normalized = {n_lower}"
        );

        // denormalize round-trip
        let back_lower = bridge.denormalize(0, n_lower).unwrap();
        let back_upper = bridge.denormalize(1, n_upper).unwrap();
        assert!(
            (back_lower - 1000.0).abs() / 1000.0 < 1e-6,
            "denormalize hz failed: {back_lower}"
        );
        assert!(
            (back_upper - 1000.0).abs() / 1000.0 < 1e-6,
            "denormalize HZ failed: {back_upper}"
        );
    }
}
