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
        "Drift Time",
        "drift_smoothing",
        0.1,
        0.001,
        1.0,
        0.001,
        "ms",
        "General",
    )
    .scaled(1000.0)
    .doc("Sample-clock pitch-drift smoothing time constant"),
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
    ParamSpec::float(
        "Reference Pitch",
        "reference_frequency_hz",
        0.0,
        0.0,
        20_000.0,
        1.0,
        "Hz",
        "Correction",
    )
    .doc("Known pilot/note frequency for absolute correction; 0 uses change-only tracking"),
    ParamSpec::bool_param(
        "Formant Preservation",
        "formant_preservation",
        false,
        "Correction",
    )
    .structural()
    .setup()
    .doc("Preserve the broad spectral envelope during pitch correction"),
    ParamSpec::float(
        "Formant Strength",
        "formant_strength",
        1.0,
        0.0,
        1.0,
        0.05,
        "",
        "Correction",
    )
    .structural()
    .setup()
    .doc("Blend toward the transported spectral envelope"),
];

// ============================================================================
// UI Layout
// ============================================================================

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[],
    main: &[
        ControlGroup::new(
            "CORRECTION",
            "CORRECTION",
            &[
                ControlSpec::knob(0),   // correction_strength
                ControlSpec::knob(4),   // confidence_threshold
                ControlSpec::knob(5),   // reference_frequency_hz
                ControlSpec::toggle(6), // formant_preservation
                ControlSpec::knob(7),   // formant_strength
            ],
        ),
        ControlGroup::new(
            "ANALYSIS",
            "ANALYSIS",
            &[
                ControlSpec::knob(1),   // analysis_window_ms
                ControlSpec::knob(2),   // drift_smoothing
                ControlSpec::toggle(3), // multi_channel_analysis
            ],
        ),
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
    #[serde(default = "d_reference_frequency_hz")]
    pub reference_frequency_hz: f64,
    #[serde(default = "d_formant_preservation")]
    pub formant_preservation: bool,
    #[serde(default = "d_formant_strength")]
    pub formant_strength: f64,
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
fn d_reference_frequency_hz() -> f64 {
    pk(PARAMS, "reference_frequency_hz").default_f64()
}
fn d_formant_preservation() -> bool {
    pk(PARAMS, "formant_preservation").default_bool()
}
fn d_formant_strength() -> f64 {
    pk(PARAMS, "formant_strength").default_f64()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            correction_strength: d_correction_strength(),
            analysis_window_ms: d_analysis_window_ms(),
            drift_smoothing: d_drift_smoothing(),
            multi_channel_analysis: d_multi_channel_analysis(),
            confidence_threshold: d_confidence_threshold(),
            reference_frequency_hz: d_reference_frequency_hz(),
            formant_preservation: d_formant_preservation(),
            formant_strength: d_formant_strength(),
        }
    }
}

// ============================================================================
// PluginParamDef implementation
// ============================================================================

impl PluginParamDef for Params {
    const PARAMS: &'static [ParamSpec] = PARAMS;
    const LAYOUT: Option<&'static PluginLayout> = Some(&LAYOUT);
    const VERSION: u32 = 3;
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
            5 => Some(self.reference_frequency_hz),
            6 => Some(if self.formant_preservation { 1.0 } else { 0.0 }),
            7 => Some(self.formant_strength),
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
            5 => self.reference_frequency_hz = value,
            6 => self.formant_preservation = value > 0.5,
            7 => self.formant_strength = value,
            _ => {}
        }
    }

    fn migrate(mut value: serde_json::Value, from_version: u32) -> serde_json::Value {
        if from_version < 2
            && let Some(object) = value.as_object_mut()
        {
            // Both legacy values selected behavior that is now replaced by
            // the sole fixed-frame, duration-preserving correction engine.
            object.remove("phase_vocoder");
        }
        if from_version < 3
            && let Some(object) = value.as_object_mut()
        {
            // Formant preservation is opt-in so every v1/v2 preset retains
            // the exact uniform correction behavior it had before v3.
            object
                .entry("formant_preservation")
                .or_insert(serde_json::Value::Bool(false));
            object
                .entry("formant_strength")
                .or_insert(serde_json::json!(1.0));
        }
        value
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
        assert_eq!(
            original.reference_frequency_hz,
            restored.reference_frequency_hz
        );
        assert_eq!(original.formant_preservation, restored.formant_preservation);
        assert_eq!(original.formant_strength, restored.formant_strength);
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
        assert_eq!(
            p.reference_frequency_hz,
            pk(PARAMS, "reference_frequency_hz").default_f64()
        );
        assert_eq!(
            p.formant_preservation,
            pk(PARAMS, "formant_preservation").default_bool()
        );
        assert_eq!(
            p.formant_strength,
            pk(PARAMS, "formant_strength").default_f64()
        );
    }

    #[test]
    fn legacy_phase_vocoder_values_migrate_explicitly_to_v3_schema() {
        for legacy in [false, true] {
            let migrated = Params::migrate(
                serde_json::json!({
                    "correction_strength": 0.75,
                    "phase_vocoder": legacy,
                }),
                1,
            );
            assert_eq!(migrated["correction_strength"], 0.75);
            assert!(migrated.get("phase_vocoder").is_none());
            let params: Params = serde_json::from_value(migrated).unwrap();
            assert_eq!(params.correction_strength, 0.75);
            assert!(!params.formant_preservation);
            assert_eq!(params.formant_strength, 1.0);
        }
    }
}
