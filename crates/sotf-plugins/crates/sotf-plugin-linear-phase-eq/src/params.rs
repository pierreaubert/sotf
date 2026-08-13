//! FIR EQ plugin parameter definitions -- single source of truth.
//!
//! This file owns:
//! - Parameter specs (PARAMS array)
//! - Per-band template specs (BAND_TEMPLATE)
//! - UI layout (LAYOUT)
//! - Serializable state (Params struct with serde defaults)
//! - Index<->field mapping (PluginParamDef impl)
//!
//! Adding a parameter: add to PARAMS, add field to Params, add match arms.
//! Nothing else needs to change.

use serde::{Deserialize, Serialize};
use sotf_host::param_specs::{ParamSpec, find_by_key as pk};
use sotf_host::plugin_layout::*;
use sotf_host::plugin_params::PluginParamDef;

// ============================================================================
// Constants
// ============================================================================

pub const FIR_LENGTH_OPTIONS: &[&str] = &["1024", "2048", "4096", "8192"];
pub const PHASE_MODE_OPTIONS: &[&str] = &["Linear", "Minimum"];
pub const MAX_FILTERS: usize = 10;

// ============================================================================
// Parameter Specifications
// ============================================================================

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::int(
        "Num Filters",
        "num_filters",
        5,
        1,
        MAX_FILTERS as i64,
        1,
        "",
        "EQ",
    )
        .structural()
        .doc("Number of EQ bands"),
    ParamSpec::choice("FIR Length", "fir_length", 1, FIR_LENGTH_OPTIONS, "Quality")
        .structural()
        .setup()
        .doc("FIR length in taps (higher = better bass resolution, more latency)"),
    ParamSpec::choice("Phase Mode", "phase_mode", 0, PHASE_MODE_OPTIONS, "Phase")
        .structural()
        .setup()
        .doc("FIR phase design mode"),
    ParamSpec::bool_param("Auto Gain", "auto_gain", false, "Output")
        .structural()
        .doc("Compensate output level"),
    ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.01, "%", "Output")
        .scaled(100.0)
        .output()
        .doc("Dry/wet mix"),
];

// ============================================================================
// Per-Band Template
// ============================================================================

/// Template for each filter band (repeated per filter).
pub const BAND_TEMPLATE: &[ParamSpec] = &[
    ParamSpec::choice(
        "Type",
        "type",
        0,
        &["Peak", "Lowshelf", "Highshelf", "Lowpass", "Highpass"],
        "Band",
    )
    .structural()
    .doc("Filter type"),
    ParamSpec::float(
        "Frequency",
        "freq",
        1000.0,
        20.0,
        20000.0,
        10.0,
        "Hz",
        "Band",
    )
    .structural()
    .doc("Center frequency"),
    ParamSpec::float("Q", "q", 1.0, 0.1, 10.0, 0.05, "", "Band")
        .structural()
        .doc("Bandwidth"),
    ParamSpec::float("Gain", "gain", 0.0, -24.0, 24.0, 0.5, "dB", "Band")
        .structural()
        .doc("Boost/cut"),
    ParamSpec::bool_param("Active", "active", true, "Band")
        .structural()
        .doc("Enable this band"),
];

// ============================================================================
// UI Layout
// ============================================================================

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::knob(0),     // num_filters
        ControlSpec::selector(1), // fir_length
        ControlSpec::selector(2), // phase_mode
    ],
    main: &[],
    output: &[
        ControlSpec::toggle(3), // auto_gain
        ControlSpec::knob(4),   // mix
    ],
    tabs: &[],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::config(100.0, 0.5),
        ColumnConstraint::main(300.0),
        ColumnConstraint::output(120.0, 0.6),
    ],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// FIR EQ plugin parameters.
///
/// All serde defaults are derived from PARAMS -- adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_num_filters")]
    pub num_filters: f64,
    #[serde(default = "d_fir_length")]
    pub fir_length: f64,
    #[serde(default = "d_phase_mode")]
    pub phase_mode: f64,
    #[serde(default = "d_auto_gain")]
    pub auto_gain: f64,
    #[serde(default = "d_mix")]
    pub mix: f64,
}

fn d_num_filters() -> f64 {
    pk(PARAMS, "num_filters").default_f64()
}
fn d_fir_length() -> f64 {
    pk(PARAMS, "fir_length").default_f64()
}
fn d_phase_mode() -> f64 {
    pk(PARAMS, "phase_mode").default_f64()
}
fn d_auto_gain() -> f64 {
    pk(PARAMS, "auto_gain").default_f64()
}
fn d_mix() -> f64 {
    pk(PARAMS, "mix").default_f64()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            num_filters: d_num_filters(),
            fir_length: d_fir_length(),
            phase_mode: d_phase_mode(),
            auto_gain: d_auto_gain(),
            mix: d_mix(),
        }
    }
}

// ============================================================================
// PluginParamDef implementation
// ============================================================================

impl PluginParamDef for Params {
    const PARAMS: &'static [ParamSpec] = PARAMS;
    const LAYOUT: Option<&'static PluginLayout> = Some(&LAYOUT);
    const VERSION: u32 = 1;
    const PLUGIN_TYPE_KEY: &'static str = "linear_phase_eq";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.num_filters),
            1 => Some(self.fir_length),
            2 => Some(self.phase_mode),
            3 => Some(self.auto_gain),
            4 => Some(self.mix),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.num_filters = value,
            1 => self.fir_length = value,
            2 => self.phase_mode = value,
            3 => self.auto_gain = value,
            4 => self.mix = value,
            _ => {}
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_index_coverage() {
        let p = Params::default();
        for i in 0..PARAMS.len() {
            assert!(
                p.param_value(i).is_some(),
                "param_value({}) returned None",
                i
            );
        }
        assert!(
            p.param_value(PARAMS.len()).is_none(),
            "param_value beyond PARAMS.len() should return None"
        );
    }

    #[test]
    fn roundtrip_serde() {
        let original = Params::default();
        let json = serde_json::to_value(&original).unwrap();
        let restored: Params = serde_json::from_value(json).unwrap();
        assert_eq!(original.num_filters, restored.num_filters);
        assert_eq!(original.fir_length, restored.fir_length);
        assert_eq!(original.phase_mode, restored.phase_mode);
        assert_eq!(original.auto_gain, restored.auto_gain);
        assert_eq!(original.mix, restored.mix);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.num_filters, pk(PARAMS, "num_filters").default_f64());
        assert_eq!(p.fir_length, pk(PARAMS, "fir_length").default_f64());
        assert_eq!(p.phase_mode, pk(PARAMS, "phase_mode").default_f64());
        assert_eq!(p.auto_gain, pk(PARAMS, "auto_gain").default_f64());
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64());
    }

    #[test]
    fn band_template_non_empty() {
        assert!(!BAND_TEMPLATE.is_empty());
    }

    #[test]
    fn band_template_has_unique_engine_keys() {
        for (i, a) in BAND_TEMPLATE.iter().enumerate() {
            for b in &BAND_TEMPLATE[i + 1..] {
                assert_ne!(
                    a.engine_key, b.engine_key,
                    "duplicate engine_key '{}' in BAND_TEMPLATE",
                    a.engine_key
                );
            }
        }
    }
}
