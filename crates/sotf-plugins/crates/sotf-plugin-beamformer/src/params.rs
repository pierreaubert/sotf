//! Beamformer plugin parameter definitions — single source of truth.
//!
//! This file owns:
//! - Parameter specs (PARAMS array)
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
// Parameter Specifications
// ============================================================================

pub const BEAMFORMER_TYPES: &[&str] = &["MVDR", "Superdirective", "GSC"];

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::int("Microphones", "num_mics", 2, 2, 8, 1, "", "Array")
        .structural()
        .doc("Number of array microphones"),
    ParamSpec::float(
        "Mic Spacing",
        "mic_spacing_cm",
        5.0,
        1.0,
        50.0,
        0.5,
        "cm",
        "Array",
    )
    .structural()
    .doc("Distance between microphones"),
    ParamSpec::float(
        "Steer Angle",
        "steer_angle_deg",
        0.0,
        -180.0,
        180.0,
        1.0,
        "\u{b0}",
        "General",
    )
    .doc("Beam steering direction"),
    ParamSpec::choice(
        "Algorithm",
        "beamformer_type",
        0,
        BEAMFORMER_TYPES,
        "General",
    )
    .structural()
    .doc("Beamforming algorithm"),
];

// ============================================================================
// UI Layout
// ============================================================================

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::slider(0), // num_mics
        ControlSpec::slider(1), // mic_spacing_cm
    ],
    main: &[ControlGroup {
        title: "",
        controls: &[
            ControlSpec::slider(2),   // steer_angle_deg
            ControlSpec::selector(3), // beamformer_type
        ],
    }],
    output: &[],
    tabs: &[],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::config(150.0, 0.4),
        ColumnConstraint::main(200.0),
    ],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// Beamformer plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_num_mics")]
    pub num_mics: usize,
    #[serde(default = "d_mic_spacing_cm")]
    pub mic_spacing_cm: f64,
    #[serde(default = "d_steer_angle_deg")]
    pub steer_angle_deg: f64,
    #[serde(default = "d_beamformer_type")]
    pub beamformer_type: String,
}

fn d_num_mics() -> usize {
    pk(PARAMS, "num_mics").default_usize()
}
fn d_mic_spacing_cm() -> f64 {
    pk(PARAMS, "mic_spacing_cm").default_f64()
}
fn d_steer_angle_deg() -> f64 {
    pk(PARAMS, "steer_angle_deg").default_f64()
}
fn d_beamformer_type() -> String {
    BEAMFORMER_TYPES[0].to_string()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            num_mics: d_num_mics(),
            mic_spacing_cm: d_mic_spacing_cm(),
            steer_angle_deg: d_steer_angle_deg(),
            beamformer_type: d_beamformer_type(),
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
    const PLUGIN_TYPE_KEY: &'static str = "beamformer";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.num_mics as f64),
            1 => Some(self.mic_spacing_cm),
            2 => Some(self.steer_angle_deg),
            3 => {
                let idx = BEAMFORMER_TYPES
                    .iter()
                    .position(|&t| t == self.beamformer_type)
                    .unwrap_or(0);
                Some(idx as f64)
            }
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.num_mics = value as usize,
            1 => self.mic_spacing_cm = value,
            2 => self.steer_angle_deg = value,
            3 => {
                let idx = (value as usize).min(BEAMFORMER_TYPES.len() - 1);
                self.beamformer_type = BEAMFORMER_TYPES[idx].to_string();
            }
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
        assert_eq!(original.num_mics, restored.num_mics);
        assert_eq!(original.mic_spacing_cm, restored.mic_spacing_cm);
        assert_eq!(original.steer_angle_deg, restored.steer_angle_deg);
        assert_eq!(original.beamformer_type, restored.beamformer_type);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.num_mics, pk(PARAMS, "num_mics").default_usize());
        assert_eq!(p.mic_spacing_cm, pk(PARAMS, "mic_spacing_cm").default_f64());
        assert_eq!(
            p.steer_angle_deg,
            pk(PARAMS, "steer_angle_deg").default_f64()
        );
        assert_eq!(p.beamformer_type, BEAMFORMER_TYPES[0]);
    }
}
