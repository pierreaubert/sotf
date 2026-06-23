//! Delay plugin parameter definitions — single source of truth.
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
// Parameter Specifications
// ============================================================================

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::float(
        "Delay", "delay_ms", 100.0, 0.0, 5000.0, 1.0, "ms", "General",
    )
    .doc("Delay time"),
    ParamSpec::float(
        "Feedback", "feedback", 0.3, -0.95, 0.95, 0.01, "", "General",
    )
    .doc("Amount fed back into delay line"),
    ParamSpec::float("Mix", "mix", 0.5, 0.0, 1.0, 0.01, "%", "General")
        .scaled(100.0)
        .output()
        .doc("Dry/wet blend"),
    ParamSpec::float(
        "LFO Rate",
        "lfo_rate_hz",
        0.0,
        0.0,
        20.0,
        0.1,
        "Hz",
        "Modulation",
    )
    .doc("Modulation oscillator speed"),
    ParamSpec::float(
        "LFO Depth",
        "lfo_depth_ms",
        0.0,
        0.0,
        10.0,
        0.1,
        "ms",
        "Modulation",
    )
    .doc("Modulation amount on delay time"),
    ParamSpec::float(
        "Allpass Coeff",
        "allpass_coeff",
        0.5,
        0.0,
        0.99,
        0.01,
        "",
        "General",
    )
    .doc("Allpass filter coefficient"),
    ParamSpec::bool_param("Allpass Feedback", "allpass_feedback", false, "General")
        .doc("Use allpass filter in feedback path"),
];

// ============================================================================
// UI Layout
// ============================================================================

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[],
    main: &[ControlGroup {
        title: "",
        controls: &[
            ControlSpec::slider(0), // delay_ms
            ControlSpec::slider(1), // feedback
            ControlSpec::knob(5),   // allpass_coeff
            ControlSpec::toggle(6), // allpass_feedback
        ],
    }],
    output: &[ControlSpec::knob(2)], // mix
    tabs: &[TabSpec {
        name: "Modulation",
        controls: &[
            ControlSpec::knob(3), // lfo_rate_hz
            ControlSpec::knob(4), // lfo_depth_ms
        ],
    }],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::main(200.0),
        ColumnConstraint::output(120.0, 0.6),
    ],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// Delay plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_delay_ms")]
    pub delay_ms: f64,
    #[serde(default = "d_feedback")]
    pub feedback: f64,
    #[serde(default = "d_mix")]
    pub mix: f64,
    #[serde(default = "d_lfo_rate_hz")]
    pub lfo_rate_hz: f64,
    #[serde(default = "d_lfo_depth_ms")]
    pub lfo_depth_ms: f64,
    #[serde(default = "d_allpass_feedback")]
    pub allpass_feedback: bool,
    #[serde(default = "d_allpass_coeff")]
    pub allpass_coeff: f64,
}

fn d_delay_ms() -> f64 {
    pk(PARAMS, "delay_ms").default_f64()
}
fn d_feedback() -> f64 {
    pk(PARAMS, "feedback").default_f64()
}
fn d_mix() -> f64 {
    pk(PARAMS, "mix").default_f64()
}
fn d_lfo_rate_hz() -> f64 {
    pk(PARAMS, "lfo_rate_hz").default_f64()
}
fn d_lfo_depth_ms() -> f64 {
    pk(PARAMS, "lfo_depth_ms").default_f64()
}
fn d_allpass_feedback() -> bool {
    pk(PARAMS, "allpass_feedback").default_bool()
}
fn d_allpass_coeff() -> f64 {
    pk(PARAMS, "allpass_coeff").default_f64()
}

pub fn default_delay_ms() -> f32 {
    d_delay_ms() as f32
}
pub fn default_feedback() -> f32 {
    d_feedback() as f32
}
pub fn default_mix() -> f32 {
    d_mix() as f32
}
pub fn default_lfo_rate_hz() -> f32 {
    d_lfo_rate_hz() as f32
}
pub fn default_lfo_depth_ms() -> f32 {
    d_lfo_depth_ms() as f32
}
pub fn default_allpass_feedback() -> bool {
    d_allpass_feedback()
}
pub fn default_allpass_coeff() -> f32 {
    d_allpass_coeff() as f32
}

impl Default for Params {
    fn default() -> Self {
        Self {
            delay_ms: d_delay_ms(),
            feedback: d_feedback(),
            mix: d_mix(),
            lfo_rate_hz: d_lfo_rate_hz(),
            lfo_depth_ms: d_lfo_depth_ms(),
            allpass_feedback: d_allpass_feedback(),
            allpass_coeff: d_allpass_coeff(),
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
    const PLUGIN_TYPE_KEY: &'static str = "delay";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.delay_ms),
            1 => Some(self.feedback),
            2 => Some(self.mix),
            3 => Some(self.lfo_rate_hz),
            4 => Some(self.lfo_depth_ms),
            5 => Some(self.allpass_coeff),
            6 => Some(if self.allpass_feedback { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.delay_ms = value,
            1 => self.feedback = value,
            2 => self.mix = value,
            3 => self.lfo_rate_hz = value,
            4 => self.lfo_depth_ms = value,
            5 => self.allpass_coeff = value,
            6 => self.allpass_feedback = value > 0.5,
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
        assert_eq!(original.delay_ms, restored.delay_ms);
        assert_eq!(original.feedback, restored.feedback);
        assert_eq!(original.mix, restored.mix);
        assert_eq!(original.lfo_rate_hz, restored.lfo_rate_hz);
        assert_eq!(original.lfo_depth_ms, restored.lfo_depth_ms);
        assert_eq!(original.allpass_feedback, restored.allpass_feedback);
        assert_eq!(original.allpass_coeff, restored.allpass_coeff);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.delay_ms, pk(PARAMS, "delay_ms").default_f64());
        assert_eq!(p.feedback, pk(PARAMS, "feedback").default_f64());
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64());
        assert_eq!(p.lfo_rate_hz, pk(PARAMS, "lfo_rate_hz").default_f64());
        assert_eq!(p.lfo_depth_ms, pk(PARAMS, "lfo_depth_ms").default_f64());
        assert_eq!(
            p.allpass_feedback,
            pk(PARAMS, "allpass_feedback").default_bool()
        );
        assert_eq!(p.allpass_coeff, pk(PARAMS, "allpass_coeff").default_f64());
    }
}
