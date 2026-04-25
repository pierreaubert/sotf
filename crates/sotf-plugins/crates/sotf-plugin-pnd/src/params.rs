//! PND plugin parameter definitions — single source of truth.
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
        "Correction",
        "correction_strength",
        1.0,
        0.0,
        2.0,
        0.05,
        "",
        "General",
    )
    .scaled(100.0)
    .doc("Pitch correction strength"),
    ParamSpec::float(
        "Analysis Window",
        "analysis_window_ms",
        100.0,
        20.0,
        500.0,
        5.0,
        "ms",
        "General",
    )
    .structural()
    .setup()
    .doc("FFT analysis window size"),
    ParamSpec::float(
        "Drift Smoothing",
        "drift_smoothing",
        0.1,
        0.001,
        1.0,
        0.001,
        "",
        "General",
    )
    .scaled(1000.0)
    .doc("Pitch drift low-pass smoothing"),
    ParamSpec::bool_param("Multi-Channel", "multi_channel_analysis", true, "Analysis")
        .structural()
        .setup()
        .doc("Analyze all channels together"),
    ParamSpec::float(
        "Confidence Threshold",
        "confidence_threshold",
        0.5,
        0.0,
        1.0,
        0.01,
        "",
        "Correction",
    )
    .doc("Min detection confidence to apply"),
    ParamSpec::bool_param("Phase Vocoder", "phase_vocoder", false, "Correction")
        .structural()
        .setup()
        .doc("Use phase vocoder for correction"),
];

// ============================================================================
// UI Layout
// ============================================================================

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[],
    main: &[
        ControlGroup {
            title: "CORRECTION",
            controls: &[
                ControlSpec::knob(0),   // correction_strength
                ControlSpec::knob(4),   // confidence_threshold
                ControlSpec::toggle(5), // phase_vocoder
            ],
        },
        ControlGroup {
            title: "ANALYSIS",
            controls: &[
                ControlSpec::knob(1),   // analysis_window_ms
                ControlSpec::knob(2),   // drift_smoothing
                ControlSpec::toggle(3), // multi_channel_analysis
            ],
        },
    ],
    output: &[],
    tabs: &[],
    visualizations: &[],
    column_constraints: &[ColumnConstraint::main(250.0)],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// PND plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_correction_strength")]
    pub correction_strength: f64,
    #[serde(default = "d_analysis_window_ms")]
    pub analysis_window_ms: f64,
    #[serde(default = "d_drift_smoothing")]
    pub drift_smoothing: f64,
    #[serde(default = "d_multi_channel_analysis")]
    pub multi_channel_analysis: bool,
    #[serde(default = "d_confidence_threshold")]
    pub confidence_threshold: f64,
    #[serde(default = "d_phase_vocoder")]
    pub phase_vocoder: bool,
}

fn d_correction_strength() -> f64 {
    pk(PARAMS, "correction_strength").default_f64()
}
fn d_analysis_window_ms() -> f64 {
    pk(PARAMS, "analysis_window_ms").default_f64()
}
fn d_drift_smoothing() -> f64 {
    pk(PARAMS, "drift_smoothing").default_f64()
}
fn d_multi_channel_analysis() -> bool {
    pk(PARAMS, "multi_channel_analysis").default_bool()
}
fn d_confidence_threshold() -> f64 {
    pk(PARAMS, "confidence_threshold").default_f64()
}
fn d_phase_vocoder() -> bool {
    pk(PARAMS, "phase_vocoder").default_bool()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            correction_strength: d_correction_strength(),
            analysis_window_ms: d_analysis_window_ms(),
            drift_smoothing: d_drift_smoothing(),
            multi_channel_analysis: d_multi_channel_analysis(),
            confidence_threshold: d_confidence_threshold(),
            phase_vocoder: d_phase_vocoder(),
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
    const PLUGIN_TYPE_KEY: &'static str = "pnd";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.correction_strength),
            1 => Some(self.analysis_window_ms),
            2 => Some(self.drift_smoothing),
            3 => Some(if self.multi_channel_analysis {
                1.0
            } else {
                0.0
            }),
            4 => Some(self.confidence_threshold),
            5 => Some(if self.phase_vocoder { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.correction_strength = value,
            1 => self.analysis_window_ms = value,
            2 => self.drift_smoothing = value,
            3 => self.multi_channel_analysis = value > 0.5,
            4 => self.confidence_threshold = value,
            5 => self.phase_vocoder = value > 0.5,
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
        assert_eq!(original.correction_strength, restored.correction_strength);
        assert_eq!(original.analysis_window_ms, restored.analysis_window_ms);
        assert_eq!(original.drift_smoothing, restored.drift_smoothing);
        assert_eq!(
            original.multi_channel_analysis,
            restored.multi_channel_analysis
        );
        assert_eq!(original.confidence_threshold, restored.confidence_threshold);
        assert_eq!(original.phase_vocoder, restored.phase_vocoder);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(
            p.correction_strength,
            pk(PARAMS, "correction_strength").default_f64()
        );
        assert_eq!(
            p.analysis_window_ms,
            pk(PARAMS, "analysis_window_ms").default_f64()
        );
        assert_eq!(
            p.drift_smoothing,
            pk(PARAMS, "drift_smoothing").default_f64()
        );
        assert_eq!(
            p.multi_channel_analysis,
            pk(PARAMS, "multi_channel_analysis").default_bool()
        );
        assert_eq!(
            p.confidence_threshold,
            pk(PARAMS, "confidence_threshold").default_f64()
        );
        assert_eq!(p.phase_vocoder, pk(PARAMS, "phase_vocoder").default_bool());
    }
}
