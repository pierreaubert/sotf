//! Convolution plugin parameter definitions — single source of truth.
//!
//! This file owns:
//! - Parameter specs (PARAMS array)
//! - UI layout (LAYOUT)
//! - Serializable state (Params struct with serde defaults)
//! - Index↔field mapping (PluginParamDef impl)
//!
//! Adding a parameter: add to PARAMS, add field to Params, add match arms.
//! Nothing else needs to change.

use serde::{Deserialize, Serialize};
use sotf_host::param_specs::{ParamSpec, find_by_key as pk};
use sotf_host::plugin_layout::*;
use sotf_host::plugin_params::PluginParamDef;

// ============================================================================
// Parameter Specifications
// ============================================================================

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::file_path("IR File", "ir_file", "General")
        .setup()
        .doc("Impulse response WAV file path"),
    ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.05, "%", "General")
        .output()
        .doc("Dry/wet blend"),
    ParamSpec::float("Gain", "gain_db", 0.0, -20.0, 20.0, 0.5, "dB", "General")
        .output()
        .doc("Output level trim"),
    ParamSpec::bool_param("Use NUPC", "use_nupc", true, "General")
        .structural()
        .doc("Non-uniform partitioned convolution"),
    // Phase 4F: SOTA additions
    ParamSpec::bool_param("Zero-Latency Head", "zero_latency_head", false, "Quality")
        .structural()
        .doc("Time-domain processing of first IR taps for zero additional latency"),
    ParamSpec::int("Head Taps", "head_taps", 128, 32, 512, 32, "", "Quality")
        .structural()
        .doc("Number of IR taps processed in time domain (32-512)"),
];

// ============================================================================
// UI Layout
// ============================================================================

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[ControlSpec::file_picker(0)], // ir_file
    main: &[],                              // IR waveform placeholder (future viz)
    output: &[
        ControlSpec::knob(1), // mix
        ControlSpec::knob(2), // gain_db
    ],
    tabs: &[TabSpec {
        name: "Advanced",
        controls: &[
            ControlSpec::toggle(3), // use_nupc
            ControlSpec::toggle(4), // zero_latency_head
            ControlSpec::knob(5),   // head_taps
        ],
    }],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::config(180.0, 0.5),
        ColumnConstraint::main(200.0),
        ColumnConstraint::output(120.0, 0.6),
    ],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// Convolution plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    // ir_file is handled separately (FilePath — skip in param_value/set_param_value)
    #[serde(default = "d_mix")]
    pub mix: f64,
    #[serde(default = "d_gain_db")]
    pub gain_db: f64,
    #[serde(default = "d_use_nupc")]
    pub use_nupc: bool,
    #[serde(default)]
    pub zero_latency_head: bool,
    #[serde(default = "d_head_taps")]
    pub head_taps: usize,
}

fn d_head_taps() -> usize {
    pk(PARAMS, "head_taps").default_f64() as usize
}
fn d_mix() -> f64 {
    pk(PARAMS, "mix").default_f64()
}
fn d_gain_db() -> f64 {
    pk(PARAMS, "gain_db").default_f64()
}
fn d_use_nupc() -> bool {
    pk(PARAMS, "use_nupc").default_bool()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            mix: d_mix(),
            gain_db: d_gain_db(),
            use_nupc: d_use_nupc(),
            zero_latency_head: false,
            head_taps: d_head_taps(),
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
    const PLUGIN_TYPE_KEY: &'static str = "convolution";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => None, // ir_file (FilePath — handled separately)
            1 => Some(self.mix),
            2 => Some(self.gain_db),
            3 => Some(if self.use_nupc { 1.0 } else { 0.0 }),
            4 => Some(if self.zero_latency_head { 1.0 } else { 0.0 }),
            5 => Some(self.head_taps as f64),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => {} // ir_file (FilePath — handled separately)
            1 => self.mix = value,
            2 => self.gain_db = value,
            3 => self.use_nupc = value > 0.5,
            4 => self.zero_latency_head = value > 0.5,
            5 => self.head_taps = value.clamp(PARAMS[5].min_f64(), PARAMS[5].max_f64()) as usize,
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
        // index 0 is FilePath (ir_file) — returns None by design
        assert!(p.param_value(0).is_none(), "ir_file should return None");
        for i in 1..PARAMS.len() {
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
        assert_eq!(original.mix, restored.mix);
        assert_eq!(original.gain_db, restored.gain_db);
        assert_eq!(original.use_nupc, restored.use_nupc);
        assert_eq!(original.head_taps, restored.head_taps);
    }

    #[test]
    fn head_taps_setter_clamps_to_spec_range() {
        let mut p = Params::default();
        p.set_param_value(5, -1.0);
        assert_eq!(p.head_taps, pk(PARAMS, "head_taps").min_f64() as usize);

        p.set_param_value(5, 9999.0);
        assert_eq!(p.head_taps, pk(PARAMS, "head_taps").max_f64() as usize);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64());
        assert_eq!(p.gain_db, pk(PARAMS, "gain_db").default_f64());
        assert_eq!(p.use_nupc, pk(PARAMS, "use_nupc").default_bool());
    }
}
