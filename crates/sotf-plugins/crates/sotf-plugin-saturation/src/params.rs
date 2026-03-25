//! Saturation plugin parameter definitions -- single source of truth.
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
use sotf_host::param_specs::{find_by_key as pk, ParamSpec};
use sotf_host::plugin_layout::*;
use sotf_host::plugin_params::PluginParamDef;

// ============================================================================
// Mode Constants
// ============================================================================

pub const MODES: &[&str] = &["Soft Clip", "Tube", "Tape", "Exciter"];
pub const OVERSAMPLING_OPTIONS: &[&str] = &["Off", "2x", "4x"];

// ============================================================================
// Parameter Specifications
// ============================================================================

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::choice("Mode", "mode", 0, MODES, "Saturation")
        .setup()
        .doc("Saturation algorithm"),
    ParamSpec::float("Drive", "drive", 2.0, 1.0, 20.0, 0.1, "", "Saturation")
        .doc("Saturation intensity"),
    ParamSpec::float("Tone", "tone", 1.5, 1.0, 3.0, 0.1, "", "Saturation")
        .doc("Harmonic character (tube mode: even/odd balance)"),
    ParamSpec::float(
        "Exciter Freq",
        "exciter_freq",
        3000.0,
        500.0,
        10000.0,
        100.0,
        "Hz",
        "Exciter",
    )
    .setup()
    .doc("Crossover frequency for exciter mode"),
    ParamSpec::choice(
        "Oversampling",
        "oversampling",
        1,
        OVERSAMPLING_OPTIONS,
        "Quality",
    )
    .setup()
    .doc("Oversampling factor for alias suppression"),
    ParamSpec::float("Output", "output_gain", 0.0, -12.0, 12.0, 0.1, "dB", "Output")
        .doc("Output gain compensation"),
    ParamSpec::float("Mix", "mix", 0.5, 0.0, 1.0, 0.01, "%", "Output")
        .scaled(100.0)
        .output()
        .doc("Dry/wet blend"),
];

// ============================================================================
// UI Layout
// ============================================================================

/// Saturation: idx 0=mode, 1=drive, 2=tone, 3=exciter_freq, 4=oversampling, 5=output_gain, 6=mix
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::selector(0), // mode
        ControlSpec::selector(4), // oversampling
    ],
    main: &[
        ControlGroup {
            title: "SATURATION",
            controls: &[
                ControlSpec::slider(1), // drive
                ControlSpec::slider(2), // tone
            ],
        },
        ControlGroup {
            title: "EXCITER",
            controls: &[
                ControlSpec::slider(3), // exciter_freq
            ],
        },
    ],
    output: &[
        ControlSpec::knob(5), // output_gain
        ControlSpec::knob(6), // mix
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

/// Saturation plugin parameters.
///
/// All serde defaults are derived from PARAMS -- adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_mode")]
    pub mode: f64,
    #[serde(default = "d_drive")]
    pub drive: f64,
    #[serde(default = "d_tone")]
    pub tone: f64,
    #[serde(default = "d_exciter_freq")]
    pub exciter_freq: f64,
    #[serde(default = "d_oversampling")]
    pub oversampling: f64,
    #[serde(default = "d_output_gain")]
    pub output_gain: f64,
    #[serde(default = "d_mix")]
    pub mix: f64,
}

fn d_mode() -> f64 {
    pk(PARAMS, "mode").default_f64()
}
fn d_drive() -> f64 {
    pk(PARAMS, "drive").default_f64()
}
fn d_tone() -> f64 {
    pk(PARAMS, "tone").default_f64()
}
fn d_exciter_freq() -> f64 {
    pk(PARAMS, "exciter_freq").default_f64()
}
fn d_oversampling() -> f64 {
    pk(PARAMS, "oversampling").default_f64()
}
fn d_output_gain() -> f64 {
    pk(PARAMS, "output_gain").default_f64()
}
fn d_mix() -> f64 {
    pk(PARAMS, "mix").default_f64()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            mode: d_mode(),
            drive: d_drive(),
            tone: d_tone(),
            exciter_freq: d_exciter_freq(),
            oversampling: d_oversampling(),
            output_gain: d_output_gain(),
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
    const PLUGIN_TYPE_KEY: &'static str = "saturation";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.mode),
            1 => Some(self.drive),
            2 => Some(self.tone),
            3 => Some(self.exciter_freq),
            4 => Some(self.oversampling),
            5 => Some(self.output_gain),
            6 => Some(self.mix),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.mode = value,
            1 => self.drive = value,
            2 => self.tone = value,
            3 => self.exciter_freq = value,
            4 => self.oversampling = value,
            5 => self.output_gain = value,
            6 => self.mix = value,
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
        assert_eq!(original.mode, restored.mode);
        assert_eq!(original.drive, restored.drive);
        assert_eq!(original.tone, restored.tone);
        assert_eq!(original.exciter_freq, restored.exciter_freq);
        assert_eq!(original.oversampling, restored.oversampling);
        assert_eq!(original.output_gain, restored.output_gain);
        assert_eq!(original.mix, restored.mix);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.mode, pk(PARAMS, "mode").default_f64());
        assert_eq!(p.drive, pk(PARAMS, "drive").default_f64());
        assert_eq!(p.tone, pk(PARAMS, "tone").default_f64());
        assert_eq!(p.exciter_freq, pk(PARAMS, "exciter_freq").default_f64());
        assert_eq!(
            p.oversampling,
            pk(PARAMS, "oversampling").default_f64()
        );
        assert_eq!(p.output_gain, pk(PARAMS, "output_gain").default_f64());
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64());
    }
}
