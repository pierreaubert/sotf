//! Spectral Compressor plugin parameter definitions — single source of truth.
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
// Constants
// ============================================================================

pub const FFT_SIZES: &[&str] = &["1024", "2048", "4096"];
pub const TARGET_MODES: &[&str] = &["All", "Tonal", "Transient"];

// ============================================================================
// Parameter Specifications
// ============================================================================

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::choice("FFT Size", "fft_size", 1, FFT_SIZES, "Analysis")
        .structural()
        .setup()
        .doc("FFT window size (higher = better frequency resolution, more latency)"),
    ParamSpec::float(
        "Threshold",
        "threshold",
        -20.0,
        -60.0,
        0.0,
        0.5,
        "dB",
        "Dynamics",
    )
    .doc("Compression threshold per bin"),
    ParamSpec::float("Ratio", "ratio", 2.0, 1.0, 20.0, 0.1, ":1", "Dynamics")
        .doc("Compression ratio"),
    ParamSpec::float("Attack", "attack", 5.0, 0.1, 100.0, 0.1, "ms", "Dynamics")
        .doc("Per-bin attack time"),
    ParamSpec::float(
        "Release", "release", 50.0, 10.0, 1000.0, 1.0, "ms", "Dynamics",
    )
    .doc("Per-bin release time"),
    ParamSpec::float("Knee", "knee", 6.0, 0.0, 20.0, 0.5, "dB", "Dynamics").doc("Soft knee width"),
    ParamSpec::float(
        "Spectral Smooth",
        "spectral_smoothing",
        0.3,
        0.0,
        1.0,
        0.01,
        "",
        "Quality",
    )
    .scaled(100.0)
    .setup()
    .doc("Frequency-axis smoothing (reduces musical artifacts)"),
    ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.01, "%", "Output")
        .scaled(100.0)
        .output()
        .doc("Dry/wet blend"),
    // --- Phase 4A: SOTA additions ---
    ParamSpec::choice("Target", "target_mode", 0, TARGET_MODES, "Analysis")
        .doc("Compress all bins, tonal only, or transient only"),
    ParamSpec::bool_labeled("Delta Listen", "delta_listen", false, "On", "Off", "Output")
        .output()
        .doc("Solo the compression delta (hear what's being removed)"),
    ParamSpec::bool_param(
        "Adaptive Threshold",
        "adaptive_threshold",
        false,
        "Analysis",
    )
    .doc("Auto-set threshold relative to long-term spectral average per bin"),
    ParamSpec::float(
        "Adaptive Offset",
        "adaptive_offset_db",
        0.0,
        -20.0,
        20.0,
        0.5,
        "dB",
        "Analysis",
    )
    .doc("Offset from adaptive threshold (positive = less compression)"),
];

// ============================================================================
// UI Layout
// ============================================================================

/// Spectral Compressor: idx 0=fft_size, 1=threshold, 2=ratio, 3=attack,
/// 4=release, 5=knee, 6=spectral_smoothing, 7=mix, 8=target_mode,
/// 9=delta_listen, 10=adaptive_threshold, 11=adaptive_offset_db
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::selector(0), // fft_size
        ControlSpec::knob(6),     // spectral_smoothing
        ControlSpec::selector(8), // target_mode
        ControlSpec::toggle(9),   // delta_listen
        ControlSpec::toggle(10),  // adaptive_threshold
        ControlSpec::knob(11),    // adaptive_offset_db
    ],
    main: &[
        ControlGroup {
            title: "DYNAMICS",
            controls: &[
                ControlSpec::slider(1), // threshold
                ControlSpec::slider(2), // ratio
                ControlSpec::slider(5), // knee
            ],
        },
        ControlGroup {
            title: "TIMING",
            controls: &[
                ControlSpec::slider(3), // attack
                ControlSpec::slider(4), // release
            ],
        },
    ],
    output: &[
        ControlSpec::knob(7), // mix
    ],
    tabs: &[],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::config(100.0, 0.5),
        ColumnConstraint::main(300.0),
        ColumnConstraint::output(80.0, 0.6),
    ],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// Spectral Compressor plugin parameters.
///
/// All serde defaults are derived from PARAMS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_fft_size")]
    pub fft_size: usize,
    #[serde(default = "d_threshold")]
    pub threshold: f64,
    #[serde(default = "d_ratio")]
    pub ratio: f64,
    #[serde(default = "d_attack")]
    pub attack: f64,
    #[serde(default = "d_release")]
    pub release: f64,
    #[serde(default = "d_knee")]
    pub knee: f64,
    #[serde(default = "d_spectral_smoothing")]
    pub spectral_smoothing: f64,
    #[serde(default = "d_mix")]
    pub mix: f64,
    #[serde(default = "d_target_mode")]
    pub target_mode: f64,
    #[serde(default)]
    pub delta_listen: f64,
    #[serde(default)]
    pub adaptive_threshold: f64,
    #[serde(default)]
    pub adaptive_offset_db: f64,
}

fn d_target_mode() -> f64 {
    pk(PARAMS, "target_mode").default_f64()
}
fn d_fft_size() -> usize {
    pk(PARAMS, "fft_size").default_f64() as usize
}
fn d_threshold() -> f64 {
    pk(PARAMS, "threshold").default_f64()
}
fn d_ratio() -> f64 {
    pk(PARAMS, "ratio").default_f64()
}
fn d_attack() -> f64 {
    pk(PARAMS, "attack").default_f64()
}
fn d_release() -> f64 {
    pk(PARAMS, "release").default_f64()
}
fn d_knee() -> f64 {
    pk(PARAMS, "knee").default_f64()
}
fn d_spectral_smoothing() -> f64 {
    pk(PARAMS, "spectral_smoothing").default_f64()
}
fn d_mix() -> f64 {
    pk(PARAMS, "mix").default_f64()
}

// Public default helpers for the runtime parameter struct.
pub fn default_fft_size_index() -> usize {
    d_fft_size()
}
pub fn default_threshold_db() -> f32 {
    d_threshold() as f32
}
pub fn default_ratio() -> f32 {
    d_ratio() as f32
}
pub fn default_attack_ms() -> f32 {
    d_attack() as f32
}
pub fn default_release_ms() -> f32 {
    d_release() as f32
}
pub fn default_knee_db() -> f32 {
    d_knee() as f32
}
pub fn default_spectral_smoothing() -> f32 {
    d_spectral_smoothing() as f32
}
pub fn default_mix() -> f32 {
    d_mix() as f32
}

impl Default for Params {
    fn default() -> Self {
        Self {
            fft_size: d_fft_size(),
            threshold: d_threshold(),
            ratio: d_ratio(),
            attack: d_attack(),
            release: d_release(),
            knee: d_knee(),
            spectral_smoothing: d_spectral_smoothing(),
            mix: d_mix(),
            target_mode: d_target_mode(),
            delta_listen: 0.0,
            adaptive_threshold: 0.0,
            adaptive_offset_db: 0.0,
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
    const PLUGIN_TYPE_KEY: &'static str = "spectral_compressor";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.fft_size as f64),
            1 => Some(self.threshold),
            2 => Some(self.ratio),
            3 => Some(self.attack),
            4 => Some(self.release),
            5 => Some(self.knee),
            6 => Some(self.spectral_smoothing),
            7 => Some(self.mix),
            8 => Some(self.target_mode),
            9 => Some(self.delta_listen),
            10 => Some(self.adaptive_threshold),
            11 => Some(self.adaptive_offset_db),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.fft_size = value as usize,
            1 => self.threshold = value,
            2 => self.ratio = value,
            3 => self.attack = value,
            4 => self.release = value,
            5 => self.knee = value,
            6 => self.spectral_smoothing = value,
            7 => self.mix = value,
            8 => self.target_mode = value,
            9 => self.delta_listen = value,
            10 => self.adaptive_threshold = value,
            11 => self.adaptive_offset_db = value,
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
        assert_eq!(original.fft_size, restored.fft_size);
        assert_eq!(original.threshold, restored.threshold);
        assert_eq!(original.ratio, restored.ratio);
        assert_eq!(original.attack, restored.attack);
        assert_eq!(original.release, restored.release);
        assert_eq!(original.knee, restored.knee);
        assert_eq!(original.spectral_smoothing, restored.spectral_smoothing);
        assert_eq!(original.mix, restored.mix);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.fft_size, pk(PARAMS, "fft_size").default_f64() as usize);
        assert_eq!(p.threshold, pk(PARAMS, "threshold").default_f64());
        assert_eq!(p.ratio, pk(PARAMS, "ratio").default_f64());
        assert_eq!(p.attack, pk(PARAMS, "attack").default_f64());
        assert_eq!(p.release, pk(PARAMS, "release").default_f64());
        assert_eq!(p.knee, pk(PARAMS, "knee").default_f64());
        assert_eq!(
            p.spectral_smoothing,
            pk(PARAMS, "spectral_smoothing").default_f64()
        );
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64());
    }

    #[test]
    fn test_set_param_value_roundtrip() {
        let mut p = Params::default();
        p.set_param_value(0, 2.0);
        assert_eq!(p.param_value(0), Some(2.0));
        p.set_param_value(1, -30.0);
        assert_eq!(p.param_value(1), Some(-30.0));
        p.set_param_value(8, 1.0); // target_mode -> Tonal
        assert_eq!(p.param_value(8), Some(1.0));
    }
}
