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

use serde::{Deserialize, Deserializer, Serialize};
use sotf_host::param_specs::{ParamSpec, find_by_key as pk};
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
    ParamSpec::float(
        "Output",
        "output_gain",
        0.0,
        -12.0,
        12.0,
        0.1,
        "dB",
        "Output",
    )
    .doc("Output gain compensation"),
    ParamSpec::float("Mix", "mix", 0.5, 0.0, 1.0, 0.01, "%", "Output")
        .scaled(100.0)
        .output()
        .doc("Dry/wet blend"),
    // --- Phase 3A: SOTA additions ---
    ParamSpec::float(
        "Dynamic",
        "dynamic_amount",
        0.0,
        0.0,
        1.0,
        0.01,
        "%",
        "Dynamic",
    )
    .scaled(100.0)
    .doc("Envelope-followed drive modulation depth"),
    ParamSpec::float(
        "Dyn Attack",
        "dynamic_attack_ms",
        5.0,
        0.1,
        100.0,
        0.5,
        "ms",
        "Dynamic",
    )
    .doc("Dynamic saturation envelope attack time"),
    ParamSpec::float(
        "Dyn Release",
        "dynamic_release_ms",
        50.0,
        1.0,
        500.0,
        1.0,
        "ms",
        "Dynamic",
    )
    .doc("Dynamic saturation envelope release time"),
    ParamSpec::bool_labeled("DC Block", "dc_blocker", true, "On", "Off", "Quality")
        .doc("Remove DC offset from asymmetric saturation"),
    ParamSpec::bool_labeled("ADAA", "use_adaa", true, "On", "Off", "Quality")
        .doc("Antiderivative anti-aliasing when oversampling is off"),
];

// ============================================================================
// UI Layout
// ============================================================================

/// Saturation: idx 0=mode, 1=drive, 2=tone, 3=exciter_freq, 4=oversampling, 5=output_gain, 6=mix,
///             7=dynamic_amount, 8=dynamic_attack_ms, 9=dynamic_release_ms, 10=dc_blocker, 11=use_adaa
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::selector(0), // mode
        ControlSpec::selector(4), // oversampling
        ControlSpec::toggle(10),  // dc_blocker
        ControlSpec::toggle(11),  // use_adaa
    ],
    main: &[
        ControlGroup::new(
            "SATURATION",
            "SATURATION",
            &[
                ControlSpec::slider(1), // drive
                ControlSpec::slider(2), // tone
            ],
        ),
        ControlGroup::new(
            "EXCITER",
            "EXCITER",
            &[
                ControlSpec::slider(3), // exciter_freq
            ],
        ),
        ControlGroup::new(
            "DYNAMIC",
            "DYNAMIC",
            &[
                ControlSpec::slider(7), // dynamic_amount
                ControlSpec::slider(8), // dynamic_attack_ms
                ControlSpec::slider(9), // dynamic_release_ms
            ],
        ),
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
    #[serde(default = "d_dynamic_amount")]
    pub dynamic_amount: f64,
    #[serde(default = "d_dynamic_attack_ms")]
    pub dynamic_attack_ms: f64,
    #[serde(default = "d_dynamic_release_ms")]
    pub dynamic_release_ms: f64,
    #[serde(default = "d_dc_blocker", deserialize_with = "deserialize_bool_legacy")]
    pub dc_blocker: bool,
    #[serde(default = "d_use_adaa", deserialize_with = "deserialize_bool_legacy")]
    pub use_adaa: bool,
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
fn d_dynamic_amount() -> f64 {
    pk(PARAMS, "dynamic_amount").default_f64()
}
fn d_dynamic_attack_ms() -> f64 {
    pk(PARAMS, "dynamic_attack_ms").default_f64()
}
fn d_dynamic_release_ms() -> f64 {
    pk(PARAMS, "dynamic_release_ms").default_f64()
}
fn d_dc_blocker() -> bool {
    pk(PARAMS, "dc_blocker").default_bool()
}
fn d_use_adaa() -> bool {
    pk(PARAMS, "use_adaa").default_bool()
}

#[derive(Deserialize)]
#[serde(untagged)]
enum BoolOrNumber {
    Bool(bool),
    Number(f64),
}

fn deserialize_bool_legacy<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    match BoolOrNumber::deserialize(deserializer)? {
        BoolOrNumber::Bool(value) => Ok(value),
        BoolOrNumber::Number(value) => Ok(value > 0.5),
    }
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
            dynamic_amount: d_dynamic_amount(),
            dynamic_attack_ms: d_dynamic_attack_ms(),
            dynamic_release_ms: d_dynamic_release_ms(),
            dc_blocker: d_dc_blocker(),
            use_adaa: d_use_adaa(),
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
            7 => Some(self.dynamic_amount),
            8 => Some(self.dynamic_attack_ms),
            9 => Some(self.dynamic_release_ms),
            10 => Some(if self.dc_blocker { 1.0 } else { 0.0 }),
            11 => Some(if self.use_adaa { 1.0 } else { 0.0 }),
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
            7 => self.dynamic_amount = value,
            8 => self.dynamic_attack_ms = value,
            9 => self.dynamic_release_ms = value,
            10 => self.dc_blocker = value > 0.5,
            11 => self.use_adaa = value > 0.5,
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
        assert_eq!(p.oversampling, pk(PARAMS, "oversampling").default_f64());
        assert_eq!(p.output_gain, pk(PARAMS, "output_gain").default_f64());
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64());
    }

    #[test]
    fn bool_params_serialize_as_booleans() {
        let json = serde_json::to_value(Params::default()).unwrap();

        assert_eq!(json["dc_blocker"], serde_json::Value::Bool(true));
        assert_eq!(json["use_adaa"], serde_json::Value::Bool(true));
    }

    #[test]
    fn bool_params_accept_legacy_numeric_presets() {
        let p: Params = serde_json::from_value(serde_json::json!({
            "dc_blocker": 0.0,
            "use_adaa": 1.0,
        }))
        .unwrap();

        assert!(!p.dc_blocker);
        assert!(p.use_adaa);
    }
}
