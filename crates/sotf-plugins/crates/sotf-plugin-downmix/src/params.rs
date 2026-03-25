//! Downmix plugin parameter definitions — single source of truth.
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
use sotf_host::param_specs::{find_by_key as pk, ParamSpec};
use sotf_host::plugin_layout::*;
use sotf_host::plugin_params::PluginParamDef;

// ============================================================================
// Parameter Specifications
// ============================================================================

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::float(
        "Center Gain",
        "center_gain_db",
        -3.0,
        -12.0,
        0.0,
        0.5,
        "dB",
        "Gains",
    )
    .doc("Center channel fold-down level"),
    ParamSpec::float(
        "Surround Gain",
        "surround_gain_db",
        -3.0,
        -12.0,
        0.0,
        0.5,
        "dB",
        "Gains",
    )
    .doc("Surround channels fold-down level"),
    ParamSpec::float(
        "Height Gain",
        "height_gain_db",
        -6.0,
        -60.0,
        0.0,
        0.5,
        "dB",
        "Gains",
    )
    .doc("Height channels fold-down level"),
    ParamSpec::float(
        "LFE Gain",
        "lfe_gain_db",
        -10.0,
        -60.0,
        0.0,
        0.5,
        "dB",
        "Gains",
    )
    .doc("LFE channel fold-down level"),
    ParamSpec::bool_param("Phase Coherence", "phase_coherence", true, "Phase")
        .doc("Phase-align channels before mix"),
    ParamSpec::float(
        "Phase Blend Low",
        "phase_blend_low_hz",
        500.0,
        100.0,
        1000.0,
        10.0,
        "Hz",
        "Phase",
    )
    .doc("Phase correction low crossover"),
    ParamSpec::float(
        "Phase Blend High",
        "phase_blend_high_hz",
        2000.0,
        1000.0,
        5000.0,
        10.0,
        "Hz",
        "Phase",
    )
    .doc("Phase correction high crossover"),
    ParamSpec::bool_param("ITU-R BS.775 Mode", "itu_mode", false, "Mode")
        .doc("Use ITU standard downmix coeffs"),
];

// ============================================================================
// UI Layout
// ============================================================================

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[ControlSpec::toggle(4)], // phase_coherence
    main: &[ControlGroup {
        title: "CHANNEL GAINS",
        controls: &[
            ControlSpec::knob(0), // center_gain_db
            ControlSpec::knob(1), // surround_gain_db
            ControlSpec::knob(2), // height_gain_db
            ControlSpec::knob(3), // lfe_gain_db
        ],
    }],
    output: &[],
    tabs: &[TabSpec {
        name: "Phase",
        controls: &[
            ControlSpec::knob(5), // phase_blend_low_hz
            ControlSpec::knob(6), // phase_blend_high_hz
        ],
    }],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::config(100.0, 0.5),
        ColumnConstraint::main(300.0),
    ],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// Downmix plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_center_gain_db")]
    pub center_gain_db: f64,
    #[serde(default = "d_surround_gain_db")]
    pub surround_gain_db: f64,
    #[serde(default = "d_height_gain_db")]
    pub height_gain_db: f64,
    #[serde(default = "d_lfe_gain_db")]
    pub lfe_gain_db: f64,
    #[serde(default = "d_phase_coherence")]
    pub phase_coherence: bool,
    #[serde(default = "d_phase_blend_low_hz")]
    pub phase_blend_low_hz: f64,
    #[serde(default = "d_phase_blend_high_hz")]
    pub phase_blend_high_hz: f64,
    #[serde(default = "d_itu_mode")]
    pub itu_mode: bool,
}

fn d_center_gain_db() -> f64 {
    pk(PARAMS, "center_gain_db").default_f64()
}
fn d_surround_gain_db() -> f64 {
    pk(PARAMS, "surround_gain_db").default_f64()
}
fn d_height_gain_db() -> f64 {
    pk(PARAMS, "height_gain_db").default_f64()
}
fn d_lfe_gain_db() -> f64 {
    pk(PARAMS, "lfe_gain_db").default_f64()
}
fn d_phase_coherence() -> bool {
    pk(PARAMS, "phase_coherence").default_bool()
}
fn d_phase_blend_low_hz() -> f64 {
    pk(PARAMS, "phase_blend_low_hz").default_f64()
}
fn d_phase_blend_high_hz() -> f64 {
    pk(PARAMS, "phase_blend_high_hz").default_f64()
}
fn d_itu_mode() -> bool {
    pk(PARAMS, "itu_mode").default_bool()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            center_gain_db: d_center_gain_db(),
            surround_gain_db: d_surround_gain_db(),
            height_gain_db: d_height_gain_db(),
            lfe_gain_db: d_lfe_gain_db(),
            phase_coherence: d_phase_coherence(),
            phase_blend_low_hz: d_phase_blend_low_hz(),
            phase_blend_high_hz: d_phase_blend_high_hz(),
            itu_mode: d_itu_mode(),
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
    const PLUGIN_TYPE_KEY: &'static str = "downmix";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.center_gain_db),
            1 => Some(self.surround_gain_db),
            2 => Some(self.height_gain_db),
            3 => Some(self.lfe_gain_db),
            4 => Some(if self.phase_coherence { 1.0 } else { 0.0 }),
            5 => Some(self.phase_blend_low_hz),
            6 => Some(self.phase_blend_high_hz),
            7 => Some(if self.itu_mode { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.center_gain_db = value,
            1 => self.surround_gain_db = value,
            2 => self.height_gain_db = value,
            3 => self.lfe_gain_db = value,
            4 => self.phase_coherence = value > 0.5,
            5 => self.phase_blend_low_hz = value,
            6 => self.phase_blend_high_hz = value,
            7 => self.itu_mode = value > 0.5,
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
        assert_eq!(original.center_gain_db, restored.center_gain_db);
        assert_eq!(original.surround_gain_db, restored.surround_gain_db);
        assert_eq!(original.height_gain_db, restored.height_gain_db);
        assert_eq!(original.lfe_gain_db, restored.lfe_gain_db);
        assert_eq!(original.phase_coherence, restored.phase_coherence);
        assert_eq!(original.phase_blend_low_hz, restored.phase_blend_low_hz);
        assert_eq!(original.phase_blend_high_hz, restored.phase_blend_high_hz);
        assert_eq!(original.itu_mode, restored.itu_mode);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(
            p.center_gain_db,
            pk(PARAMS, "center_gain_db").default_f64()
        );
        assert_eq!(
            p.surround_gain_db,
            pk(PARAMS, "surround_gain_db").default_f64()
        );
        assert_eq!(
            p.height_gain_db,
            pk(PARAMS, "height_gain_db").default_f64()
        );
        assert_eq!(p.lfe_gain_db, pk(PARAMS, "lfe_gain_db").default_f64());
        assert_eq!(
            p.phase_coherence,
            pk(PARAMS, "phase_coherence").default_bool()
        );
        assert_eq!(
            p.phase_blend_low_hz,
            pk(PARAMS, "phase_blend_low_hz").default_f64()
        );
        assert_eq!(
            p.phase_blend_high_hz,
            pk(PARAMS, "phase_blend_high_hz").default_f64()
        );
        assert_eq!(p.itu_mode, pk(PARAMS, "itu_mode").default_bool());
    }
}
