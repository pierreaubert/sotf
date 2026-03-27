//! Bridge between `ParamSpec` arrays and the `Plugin` trait's parameter methods.
//!
//! These helpers derive `parameters()`, `get_parameter()`, and `set_parameter()`
//! directly from the PARAMS spec, eliminating manual duplication. Adding a new
//! entry to PARAMS automatically makes it available in all three methods.
//!
//! # Usage
//!
//! ```rust,ignore
//! use sotf_host::param_bridge;
//!
//! impl Plugin for MyPlugin {
//!     fn parameters(&self) -> Vec<Parameter> {
//!         param_bridge::build_parameters(PARAMS, |i| /* get f64 value at index i */)
//!     }
//!     fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
//!         param_bridge::get_parameter(PARAMS, id, |i| /* get f64 value at index i */)
//!     }
//!     fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
//!         let idx = param_bridge::set_parameter(PARAMS, &id, &value, |i, v| {
//!             /* set f64 value at index i */
//!         })?;
//!         // Optional: side-effect dispatch based on idx
//!         self.rebuild_cached_parameters();
//!         Ok(())
//!     }
//! }
//! ```

use crate::param_specs::{ParamSpec, ParamType};
use crate::parameters::{Parameter, ParameterId, ParameterValue};

/// Build the full `Vec<Parameter>` from a PARAMS spec and a value getter.
///
/// This replaces hand-coded `rebuild_cached_parameters()` methods.
/// FilePath params are included with an empty string default (the actual
/// path is managed separately by the plugin).
pub fn build_parameters(
    specs: &[ParamSpec],
    get_value: impl Fn(usize) -> Option<f64>,
) -> Vec<Parameter> {
    specs
        .iter()
        .enumerate()
        .map(|(i, spec)| {
            let val = get_value(i).unwrap_or(spec.default_f64());
            spec_to_parameter(spec, val)
        })
        .collect()
}

/// Look up a parameter by string ID and return its current `ParameterValue`.
///
/// This replaces hand-coded `get_parameter()` if-else chains.
pub fn get_parameter(
    specs: &[ParamSpec],
    id: &ParameterId,
    get_value: impl Fn(usize) -> Option<f64>,
) -> Option<ParameterValue> {
    let (idx, spec) = specs
        .iter()
        .enumerate()
        .find(|(_, s)| s.engine_key == id.0)?;
    let val = get_value(idx)?;
    Some(f64_to_param_value(spec, val))
}

/// Validate and set a parameter by string ID. Returns the PARAMS index that
/// was set, so the caller can dispatch side-effects (e.g., recompute coefficients).
///
/// This replaces hand-coded `set_parameter()` if-else chains.
/// Validation (type check + range) is done here — no need to call `validate_parameter`.
pub fn set_parameter(
    specs: &[ParamSpec],
    id: &ParameterId,
    value: &ParameterValue,
    mut set_value: impl FnMut(usize, f64),
) -> Result<usize, String> {
    let (idx, spec) = specs
        .iter()
        .enumerate()
        .find(|(_, s)| s.engine_key == id.0)
        .ok_or_else(|| format!("Unknown parameter: {}", id))?;

    let f64_val = param_value_to_f64(spec, value)?;
    set_value(idx, f64_val);
    Ok(idx)
}

// ============================================================================
// Internal conversions
// ============================================================================

/// Convert a `ParamSpec` + current f64 value into a `Parameter` for caching.
fn spec_to_parameter(spec: &ParamSpec, value: f64) -> Parameter {
    match spec.param_type {
        ParamType::Float { min, max, .. } => Parameter::new_float(
            spec.engine_key,
            spec.name,
            value as f32,
            min as f32,
            max as f32,
        ),
        ParamType::Int { min, max, .. } => {
            Parameter::new_int(spec.engine_key, spec.name, value as i32, min as i32, max as i32)
        }
        ParamType::Bool { .. } => Parameter::new_bool(spec.engine_key, spec.name, value > 0.5),
        ParamType::Choice { labels, .. } => {
            let idx = value as i32;
            let num = labels.len() as i32;
            Parameter::new_int(spec.engine_key, spec.name, idx, 0, num.saturating_sub(1))
        }
        ParamType::FilePath => Parameter::new_string(spec.engine_key, spec.name, String::new()),
    }
}

/// Convert an f64 (from `param_value()`) to the appropriate `ParameterValue`.
fn f64_to_param_value(spec: &ParamSpec, val: f64) -> ParameterValue {
    match spec.param_type {
        ParamType::Float { .. } => ParameterValue::Float(val as f32),
        ParamType::Int { .. } => ParameterValue::Int(val as i32),
        ParamType::Bool { .. } => ParameterValue::Bool(val > 0.5),
        ParamType::Choice { .. } => ParameterValue::Int(val as i32),
        ParamType::FilePath => ParameterValue::String(String::new()),
    }
}

/// Convert a `ParameterValue` to f64 for `set_param_value()`, with type + range validation.
fn param_value_to_f64(spec: &ParamSpec, value: &ParameterValue) -> Result<f64, String> {
    match (&spec.param_type, value) {
        (ParamType::Float { min, max, .. }, ParameterValue::Float(v)) => {
            if v.is_nan() {
                return Err(format!("{}: value is NaN", spec.engine_key));
            }
            if v.is_infinite() {
                return Err(format!("{}: value is infinite", spec.engine_key));
            }
            let clamped = (*v as f64).clamp(*min, *max);
            Ok(clamped)
        }
        (ParamType::Int { min, max, .. }, ParameterValue::Int(v)) => {
            let clamped = (*v as i64).clamp(*min, *max);
            Ok(clamped as f64)
        }
        (ParamType::Bool { .. }, ParameterValue::Bool(v)) => Ok(if *v { 1.0 } else { 0.0 }),
        (ParamType::Choice { labels, .. }, ParameterValue::Int(v)) => {
            let clamped = (*v).max(0) as usize;
            let clamped = clamped.min(labels.len().saturating_sub(1));
            Ok(clamped as f64)
        }
        // Allow Float→Int coercion (ParameterValue::parse sometimes produces Float for ints)
        (ParamType::Int { min, max, .. }, ParameterValue::Float(v)) => {
            let clamped = (*v as i64).clamp(*min, *max);
            Ok(clamped as f64)
        }
        // Allow Int→Float coercion
        (ParamType::Float { min, max, .. }, ParameterValue::Int(v)) => {
            let clamped = (*v as f64).clamp(*min, *max);
            Ok(clamped)
        }
        _ => Err(format!(
            "{}: type mismatch (expected {:?}, got {:?})",
            spec.engine_key,
            spec_type_name(&spec.param_type),
            value_type_name(value),
        )),
    }
}

fn spec_type_name(pt: &ParamType) -> &'static str {
    match pt {
        ParamType::Float { .. } => "Float",
        ParamType::Int { .. } => "Int",
        ParamType::Bool { .. } => "Bool",
        ParamType::Choice { .. } => "Choice/Int",
        ParamType::FilePath => "FilePath",
    }
}

fn value_type_name(v: &ParameterValue) -> &'static str {
    match v {
        ParameterValue::Float(_) => "Float",
        ParameterValue::Int(_) => "Int",
        ParameterValue::Bool(_) => "Bool",
        ParameterValue::String(_) => "String",
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::param_specs::ParamSpec;

    const TEST_PARAMS: &[ParamSpec] = &[
        ParamSpec::float("Gain", "gain_db", 0.0, -60.0, 12.0, 0.1, "dB", "General"),
        ParamSpec::bool_param("Bypass", "bypass", false, "General"),
        ParamSpec::int("FFT Size", "fft_size", 1024, 256, 4096, 256, "", "Advanced"),
    ];

    #[test]
    fn build_parameters_contains_all_specs() {
        let values = [0.0_f64, 0.0, 1024.0];
        let params = build_parameters(TEST_PARAMS, |i| Some(values[i]));
        assert_eq!(params.len(), 3);
        assert_eq!(params[0].id, ParameterId::from("gain_db"));
        assert_eq!(params[1].id, ParameterId::from("bypass"));
        assert_eq!(params[2].id, ParameterId::from("fft_size"));
    }

    #[test]
    fn get_parameter_finds_by_key() {
        let values = [-3.0_f64, 1.0, 2048.0];
        let result = get_parameter(TEST_PARAMS, &ParameterId::from("gain_db"), |i| {
            Some(values[i])
        });
        assert_eq!(result, Some(ParameterValue::Float(-3.0)));

        let result = get_parameter(TEST_PARAMS, &ParameterId::from("bypass"), |i| {
            Some(values[i])
        });
        assert_eq!(result, Some(ParameterValue::Bool(true)));

        let result = get_parameter(TEST_PARAMS, &ParameterId::from("fft_size"), |i| {
            Some(values[i])
        });
        assert_eq!(result, Some(ParameterValue::Int(2048)));
    }

    #[test]
    fn get_parameter_returns_none_for_unknown() {
        let result =
            get_parameter(TEST_PARAMS, &ParameterId::from("nonexistent"), |_| Some(0.0));
        assert!(result.is_none());
    }

    #[test]
    fn set_parameter_returns_index() {
        let mut stored = [0.0_f64; 3];
        let idx = set_parameter(
            TEST_PARAMS,
            &ParameterId::from("gain_db"),
            &ParameterValue::Float(-6.0),
            |i, v| stored[i] = v,
        )
        .unwrap();
        assert_eq!(idx, 0);
        assert_eq!(stored[0], -6.0);
    }

    #[test]
    fn set_parameter_clamps_to_range() {
        let mut stored = [0.0_f64; 3];
        set_parameter(
            TEST_PARAMS,
            &ParameterId::from("gain_db"),
            &ParameterValue::Float(100.0),
            |i, v| stored[i] = v,
        )
        .unwrap();
        assert_eq!(stored[0], 12.0); // clamped to max
    }

    #[test]
    fn set_parameter_rejects_unknown() {
        let result = set_parameter(
            TEST_PARAMS,
            &ParameterId::from("nope"),
            &ParameterValue::Float(0.0),
            |_, _| {},
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown parameter"));
    }

    #[test]
    fn set_parameter_rejects_type_mismatch() {
        let result = set_parameter(
            TEST_PARAMS,
            &ParameterId::from("bypass"),
            &ParameterValue::String("hello".into()),
            |_, _| {},
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("type mismatch"));
    }
}
