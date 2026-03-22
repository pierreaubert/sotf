//! Limiter plugin parameter definitions — single source of truth.
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
        "Threshold",
        "threshold",
        -0.1,
        -20.0,
        0.0,
        0.1,
        "dB",
        "Dynamics",
    )
    .doc("Ceiling level (max output)"),
    ParamSpec::float(
        "Release", "release", 50.0, 10.0, 1000.0, 5.0, "ms", "Timing",
    )
    .doc("Time to return to unity gain"),
    ParamSpec::float(
        "Lookahead",
        "lookahead",
        5.0,
        0.0,
        20.0,
        0.5,
        "ms",
        "Timing",
    )
    .doc("Pre-delay for peak catching"),
    ParamSpec::bool_labeled("Soft Knee", "soft", false, "Soft", "Hard", "Dynamics")
        .setup()
        .doc("Gradual vs hard limiting onset"),
    ParamSpec::bool_labeled("True Peak", "true_peak", false, "On", "Off", "Detection")
        .setup()
        .doc("Detect inter-sample peaks"),
    ParamSpec::bool_labeled("Dual Release", "dual_release", false, "On", "Off", "Timing")
        .setup()
        .doc("Fast+slow release envelopes"),
    ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.05, "%", "Output")
        .scaled(100.0)
        .output()
        .doc("Dry/wet blend"),
];

// ============================================================================
// UI Layout
// ============================================================================

/// Limiter: idx 0=threshold, 1=release, 2=lookahead, 3=soft_knee, 4=true_peak, 5=dual_release, 6=mix
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[ControlSpec::toggle(3), ControlSpec::toggle(4), ControlSpec::toggle(5)], // soft_knee, true_peak, dual_release
    main: &[
        ControlGroup {
            title: "DYNAMICS",
            controls: &[ControlSpec::slider(0)], // threshold (ceiling)
        },
        ControlGroup {
            title: "TIMING",
            controls: &[
                ControlSpec::slider(1), // release
                ControlSpec::slider(2), // lookahead
            ],
        },
    ],
    output: &[
        ControlSpec::meter(-20.0, 0.0), // GR meter (limiter range)
        ControlSpec::knob(6),           // mix
    ],
    tabs: &[],
    visualizations: &[VizSlot::TransferCurve {
        position: VizPosition::BelowGroup("DYNAMICS"),
    }],
    column_constraints: &[
        ColumnConstraint::config(100.0, 0.5),
        ColumnConstraint::main(300.0),
        ColumnConstraint::output(120.0, 0.6),
    ],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// Limiter plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_threshold")]
    pub threshold: f64,
    #[serde(default = "d_release")]
    pub release: f64,
    #[serde(default = "d_lookahead")]
    pub lookahead: f64,
    #[serde(default = "d_soft")]
    pub soft: bool,
    #[serde(default = "d_true_peak")]
    pub true_peak: bool,
    #[serde(default = "d_dual_release")]
    pub dual_release: bool,
    #[serde(default = "d_mix")]
    pub mix: f64,
}

fn d_threshold() -> f64 {
    pk(PARAMS, "threshold").default_f64()
}
fn d_release() -> f64 {
    pk(PARAMS, "release").default_f64()
}
fn d_lookahead() -> f64 {
    pk(PARAMS, "lookahead").default_f64()
}
fn d_soft() -> bool {
    pk(PARAMS, "soft").default_bool()
}
fn d_true_peak() -> bool {
    pk(PARAMS, "true_peak").default_bool()
}
fn d_dual_release() -> bool {
    pk(PARAMS, "dual_release").default_bool()
}
fn d_mix() -> f64 {
    pk(PARAMS, "mix").default_f64()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            threshold: d_threshold(),
            release: d_release(),
            lookahead: d_lookahead(),
            soft: d_soft(),
            true_peak: d_true_peak(),
            dual_release: d_dual_release(),
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
    const PLUGIN_TYPE_KEY: &'static str = "limiter";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.threshold),
            1 => Some(self.release),
            2 => Some(self.lookahead),
            3 => Some(if self.soft { 1.0 } else { 0.0 }),
            4 => Some(if self.true_peak { 1.0 } else { 0.0 }),
            5 => Some(if self.dual_release { 1.0 } else { 0.0 }),
            6 => Some(self.mix),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.threshold = value,
            1 => self.release = value,
            2 => self.lookahead = value,
            3 => self.soft = value > 0.5,
            4 => self.true_peak = value > 0.5,
            5 => self.dual_release = value > 0.5,
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
        assert_eq!(original.threshold, restored.threshold);
        assert_eq!(original.release, restored.release);
        assert_eq!(original.lookahead, restored.lookahead);
        assert_eq!(original.soft, restored.soft);
        assert_eq!(original.true_peak, restored.true_peak);
        assert_eq!(original.dual_release, restored.dual_release);
        assert_eq!(original.mix, restored.mix);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.threshold, pk(PARAMS, "threshold").default_f64());
        assert_eq!(p.release, pk(PARAMS, "release").default_f64());
        assert_eq!(p.lookahead, pk(PARAMS, "lookahead").default_f64());
        assert_eq!(p.soft, pk(PARAMS, "soft").default_bool());
        assert_eq!(p.true_peak, pk(PARAMS, "true_peak").default_bool());
        assert_eq!(p.dual_release, pk(PARAMS, "dual_release").default_bool());
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64());
    }
}
