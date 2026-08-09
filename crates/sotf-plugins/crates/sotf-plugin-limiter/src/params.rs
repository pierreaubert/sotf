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
use sotf_host::param_specs::{ParamSpec, find_by_key as pk};
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
    ParamSpec::bool_labeled("ISP Limit", "isp_mode", false, "On", "Off", "Detection")
        .setup()
        .doc("Guarantee output has no inter-sample peaks above ceiling"),
    ParamSpec::bool_labeled("Dual Release", "dual_release", false, "On", "Off", "Timing")
        .setup()
        .doc("Fast+slow release envelopes"),
    ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.05, "%", "Output")
        .scaled(100.0)
        .output()
        .doc("Dry/wet blend"),
    // --- Phase 3B: SOTA additions ---
    ParamSpec::float("Link", "link_amount", 1.0, 0.0, 1.0, 0.01, "%", "Detection")
        .scaled(100.0)
        .doc("Channel linking: 0%=independent, 100%=linked (all channels see max peak)"),
    ParamSpec::bool_labeled(
        "Feed Forward",
        "feed_forward",
        false,
        "On",
        "Off",
        "Detection",
    )
    .setup()
    .doc("Scan lookahead buffer for anticipatory gain reduction"),
];

// ============================================================================
// UI Layout
// ============================================================================

/// Limiter: idx 0=threshold, 1=release, 2=lookahead, 3=soft_knee, 4=true_peak, 5=isp_mode, 6=dual_release, 7=mix, 8=link_amount, 9=feed_forward
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::toggle(3),
        ControlSpec::toggle(4),
        ControlSpec::toggle(5),
        ControlSpec::toggle(6),
        ControlSpec::toggle(9),
    ], // soft_knee, true_peak, isp_mode, dual_release, feed_forward
    main: &[
        ControlGroup::new(
            "DYNAMICS",
            "DYNAMICS",
            &[
                ControlSpec::slider(0), // threshold (ceiling)
                ControlSpec::slider(8), // link_amount
            ],
        ),
        ControlGroup::new(
            "TIMING",
            "TIMING",
            &[
                ControlSpec::slider(1), // release
                ControlSpec::slider(2), // lookahead
            ],
        ),
        ControlGroup::new(
            "OUTPUT",
            "OUTPUT",
            &[ControlSpec::meter(-20.0, 0.0), ControlSpec::knob(7)],
        ),
    ],
    output: &[],
    tabs: &[],
    visualizations: &[VizSlot::TransferCurve {
        position: VizPosition::BelowGroup("DYNAMICS"),
    }],
    column_constraints: &[
        ColumnConstraint::config(100.0, 0.5),
        ColumnConstraint::main(300.0),
    ],
    dynamic_sections: &[],
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
    #[serde(default = "d_isp_mode")]
    pub isp_mode: bool,
    #[serde(default = "d_dual_release")]
    pub dual_release: bool,
    #[serde(default = "d_mix")]
    pub mix: f64,
    #[serde(default = "d_link_amount")]
    pub link_amount: f64,
    #[serde(default = "d_feed_forward")]
    pub feed_forward: bool,
}

fn d_link_amount() -> f64 {
    pk(PARAMS, "link_amount").default_f64()
}
fn d_feed_forward() -> bool {
    pk(PARAMS, "feed_forward").default_bool()
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
fn d_isp_mode() -> bool {
    pk(PARAMS, "isp_mode").default_bool()
}
fn d_dual_release() -> bool {
    pk(PARAMS, "dual_release").default_bool()
}
fn d_mix() -> f64 {
    pk(PARAMS, "mix").default_f64()
}

/// Public default helpers used by `LimiterPluginParams` so its serde defaults
/// come from the same `PARAMS` array used by `PluginParamDef`.
pub fn default_threshold_db() -> f32 {
    d_threshold() as f32
}
pub fn default_release_ms() -> f32 {
    d_release() as f32
}
pub fn default_lookahead_ms() -> f32 {
    d_lookahead() as f32
}
pub fn default_soft() -> bool {
    d_soft()
}
pub fn default_true_peak() -> bool {
    d_true_peak()
}
pub fn default_isp_mode() -> bool {
    d_isp_mode()
}
pub fn default_dual_release() -> bool {
    d_dual_release()
}
pub fn default_mix() -> f32 {
    d_mix() as f32
}
pub fn default_link_amount() -> f32 {
    d_link_amount() as f32
}
pub fn default_feed_forward() -> bool {
    d_feed_forward()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            threshold: d_threshold(),
            release: d_release(),
            lookahead: d_lookahead(),
            soft: d_soft(),
            true_peak: d_true_peak(),
            isp_mode: d_isp_mode(),
            dual_release: d_dual_release(),
            mix: d_mix(),
            link_amount: d_link_amount(),
            feed_forward: d_feed_forward(),
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
            5 => Some(if self.isp_mode { 1.0 } else { 0.0 }),
            6 => Some(if self.dual_release { 1.0 } else { 0.0 }),
            7 => Some(self.mix),
            8 => Some(self.link_amount),
            9 => Some(if self.feed_forward { 1.0 } else { 0.0 }),
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
            5 => self.isp_mode = value > 0.5,
            6 => self.dual_release = value > 0.5,
            7 => self.mix = value,
            8 => self.link_amount = value,
            9 => self.feed_forward = value > 0.5,
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
        assert_eq!(original.isp_mode, restored.isp_mode);
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
        assert_eq!(p.isp_mode, pk(PARAMS, "isp_mode").default_bool());
        assert_eq!(p.dual_release, pk(PARAMS, "dual_release").default_bool());
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64());
    }
}
