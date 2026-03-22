//! Compressor plugin parameter definitions — single source of truth.
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
// Detection Mode Constants
// ============================================================================

pub const DETECTION_MODES: &[&str] = &["Peak", "RMS"];

// ============================================================================
// Parameter Specifications
// ============================================================================

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::float(
        "Threshold",
        "threshold",
        -20.0,
        -60.0,
        0.0,
        1.0,
        "dB",
        "Dynamics",
    )
    .doc("Level above which compression starts"),
    ParamSpec::float("Ratio", "ratio", 4.0, 1.0, 20.0, 0.1, ":1", "Dynamics")
        .doc("Compression amount (input:output)"),
    ParamSpec::float("Attack", "attack", 5.0, 0.1, 100.0, 0.5, "ms", "Timing")
        .doc("Time to reach full compression"),
    ParamSpec::float(
        "Release", "release", 50.0, 10.0, 1000.0, 5.0, "ms", "Timing",
    )
    .doc("Time to return to unity gain"),
    ParamSpec::float("Knee", "knee", 6.0, 0.0, 20.0, 0.5, "dB", "Dynamics")
        .doc("Softness of threshold transition"),
    ParamSpec::float(
        "Makeup Gain",
        "makeup_gain",
        0.0,
        -24.0,
        24.0,
        0.5,
        "dB",
        "Output",
    )
    .output()
    .doc("Post-compression gain boost"),
    ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.01, "%", "Output")
        .scaled(100.0)
        .output()
        .doc("Dry/wet blend (parallel comp)"),
    ParamSpec::bool_param("Auto Makeup", "auto_makeup", false, "Output")
        .output()
        .doc("Auto-compensate for gain reduction"),
    ParamSpec::bool_labeled(
        "Link Channels",
        "link_channels",
        true,
        "Linked",
        "Unlinked",
        "Channels",
    )
    .setup()
    .doc("Stereo-link detector for L/R"),
    ParamSpec::float(
        "Sidechain HPF",
        "sidechain_hpf_hz",
        80.0,
        0.0,
        200.0,
        5.0,
        "Hz",
        "Sidechain",
    )
    .setup()
    .doc("High-pass on detector input"),
    ParamSpec::choice(
        "Detection Mode",
        "detection_mode",
        0,
        DETECTION_MODES,
        "Sidechain",
    )
    .setup()
    .doc("Peak or RMS level detection"),
    ParamSpec::float(
        "Lookahead",
        "lookahead_ms",
        0.0,
        0.0,
        20.0,
        0.5,
        "ms",
        "Timing",
    )
    .doc("Pre-delay for transient catching"),
    ParamSpec::bool_param(
        "Program Dependent Release",
        "program_dependent_release",
        false,
        "Timing",
    )
    .doc("Adapts release to signal content"),
    ParamSpec::bool_param(
        "Measured Auto Makeup",
        "measured_auto_makeup",
        false,
        "Output",
    )
    .output()
    .doc("Makeup based on measured reduction"),
    ParamSpec::bool_param(
        "External Sidechain",
        "sidechain_external",
        false,
        "Sidechain",
    )
    .setup()
    .doc("Use external signal for detection"),
];

// ============================================================================
// UI Layout
// ============================================================================

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::toggle(8),    // link_channels
        ControlSpec::knob(9),      // sidechain_hpf_hz
        ControlSpec::selector(10), // detection_mode
    ],
    main: &[
        ControlGroup {
            title: "DYNAMICS",
            controls: &[
                ControlSpec::slider(0), // threshold
                ControlSpec::slider(1), // ratio
                ControlSpec::slider(4), // knee
            ],
        },
        ControlGroup {
            title: "TIMING",
            controls: &[
                ControlSpec::slider(2),  // attack
                ControlSpec::slider(3),  // release
                ControlSpec::knob(11),   // lookahead_ms
                ControlSpec::toggle(12), // program_dependent_release
            ],
        },
    ],
    output: &[
        ControlSpec::meter(-30.0, 0.0), // GR meter
        ControlSpec::toggle(7),         // auto_makeup
        ControlSpec::toggle(13),        // measured_auto_makeup
        ControlSpec::knob(5),           // makeup_gain
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

/// Compressor plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
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
    #[serde(default = "d_makeup_gain")]
    pub makeup_gain: f64,
    #[serde(default = "d_mix")]
    pub mix: f64,
    #[serde(default = "d_auto_makeup")]
    pub auto_makeup: bool,
    #[serde(default = "d_link_channels")]
    pub link_channels: bool,
    #[serde(default = "d_sidechain_hpf_hz")]
    pub sidechain_hpf_hz: f64,
    #[serde(default = "d_detection_mode")]
    pub detection_mode: String,
    #[serde(default = "d_lookahead_ms")]
    pub lookahead_ms: f64,
    #[serde(default = "d_program_dependent_release")]
    pub program_dependent_release: bool,
    #[serde(default = "d_measured_auto_makeup")]
    pub measured_auto_makeup: bool,
    #[serde(default = "d_sidechain_external")]
    pub sidechain_external: bool,
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
fn d_makeup_gain() -> f64 {
    pk(PARAMS, "makeup_gain").default_f64()
}
fn d_mix() -> f64 {
    pk(PARAMS, "mix").default_f64()
}
fn d_auto_makeup() -> bool {
    pk(PARAMS, "auto_makeup").default_bool()
}
fn d_link_channels() -> bool {
    pk(PARAMS, "link_channels").default_bool()
}
fn d_sidechain_hpf_hz() -> f64 {
    pk(PARAMS, "sidechain_hpf_hz").default_f64()
}
fn d_detection_mode() -> String {
    DETECTION_MODES[0].to_string()
}
fn d_lookahead_ms() -> f64 {
    pk(PARAMS, "lookahead_ms").default_f64()
}
fn d_program_dependent_release() -> bool {
    pk(PARAMS, "program_dependent_release").default_bool()
}
fn d_measured_auto_makeup() -> bool {
    pk(PARAMS, "measured_auto_makeup").default_bool()
}
fn d_sidechain_external() -> bool {
    pk(PARAMS, "sidechain_external").default_bool()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            threshold: d_threshold(),
            ratio: d_ratio(),
            attack: d_attack(),
            release: d_release(),
            knee: d_knee(),
            makeup_gain: d_makeup_gain(),
            mix: d_mix(),
            auto_makeup: d_auto_makeup(),
            link_channels: d_link_channels(),
            sidechain_hpf_hz: d_sidechain_hpf_hz(),
            detection_mode: d_detection_mode(),
            lookahead_ms: d_lookahead_ms(),
            program_dependent_release: d_program_dependent_release(),
            measured_auto_makeup: d_measured_auto_makeup(),
            sidechain_external: d_sidechain_external(),
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
    const PLUGIN_TYPE_KEY: &'static str = "compressor";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.threshold),
            1 => Some(self.ratio),
            2 => Some(self.attack),
            3 => Some(self.release),
            4 => Some(self.knee),
            5 => Some(self.makeup_gain),
            6 => Some(self.mix),
            7 => Some(if self.auto_makeup { 1.0 } else { 0.0 }),
            8 => Some(if self.link_channels { 1.0 } else { 0.0 }),
            9 => Some(self.sidechain_hpf_hz),
            10 => Some(
                DETECTION_MODES
                    .iter()
                    .position(|&m| m.eq_ignore_ascii_case(&self.detection_mode))
                    .unwrap_or(0) as f64,
            ),
            11 => Some(self.lookahead_ms),
            12 => Some(if self.program_dependent_release {
                1.0
            } else {
                0.0
            }),
            13 => Some(if self.measured_auto_makeup { 1.0 } else { 0.0 }),
            14 => Some(if self.sidechain_external { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.threshold = value,
            1 => self.ratio = value,
            2 => self.attack = value,
            3 => self.release = value,
            4 => self.knee = value,
            5 => self.makeup_gain = value,
            6 => self.mix = value,
            7 => self.auto_makeup = value > 0.5,
            8 => self.link_channels = value > 0.5,
            9 => self.sidechain_hpf_hz = value,
            10 => {
                let idx = value as usize;
                if let Some(&label) = DETECTION_MODES.get(idx) {
                    self.detection_mode = label.to_string();
                }
            }
            11 => self.lookahead_ms = value,
            12 => self.program_dependent_release = value > 0.5,
            13 => self.measured_auto_makeup = value > 0.5,
            14 => self.sidechain_external = value > 0.5,
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
        assert_eq!(original.ratio, restored.ratio);
        assert_eq!(original.attack, restored.attack);
        assert_eq!(original.release, restored.release);
        assert_eq!(original.knee, restored.knee);
        assert_eq!(original.makeup_gain, restored.makeup_gain);
        assert_eq!(original.mix, restored.mix);
        assert_eq!(original.auto_makeup, restored.auto_makeup);
        assert_eq!(original.link_channels, restored.link_channels);
        assert_eq!(original.sidechain_hpf_hz, restored.sidechain_hpf_hz);
        assert_eq!(original.detection_mode, restored.detection_mode);
        assert_eq!(original.lookahead_ms, restored.lookahead_ms);
        assert_eq!(
            original.program_dependent_release,
            restored.program_dependent_release
        );
        assert_eq!(
            original.measured_auto_makeup,
            restored.measured_auto_makeup
        );
        assert_eq!(original.sidechain_external, restored.sidechain_external);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.threshold, pk(PARAMS, "threshold").default_f64());
        assert_eq!(p.ratio, pk(PARAMS, "ratio").default_f64());
        assert_eq!(p.attack, pk(PARAMS, "attack").default_f64());
        assert_eq!(p.release, pk(PARAMS, "release").default_f64());
        assert_eq!(p.knee, pk(PARAMS, "knee").default_f64());
        assert_eq!(p.makeup_gain, pk(PARAMS, "makeup_gain").default_f64());
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64());
        assert_eq!(p.auto_makeup, pk(PARAMS, "auto_makeup").default_bool());
        assert_eq!(
            p.link_channels,
            pk(PARAMS, "link_channels").default_bool()
        );
        assert_eq!(
            p.sidechain_hpf_hz,
            pk(PARAMS, "sidechain_hpf_hz").default_f64()
        );
        assert_eq!(p.detection_mode, DETECTION_MODES[0]);
        assert_eq!(p.lookahead_ms, pk(PARAMS, "lookahead_ms").default_f64());
        assert_eq!(
            p.program_dependent_release,
            pk(PARAMS, "program_dependent_release").default_bool()
        );
        assert_eq!(
            p.measured_auto_makeup,
            pk(PARAMS, "measured_auto_makeup").default_bool()
        );
        assert_eq!(
            p.sidechain_external,
            pk(PARAMS, "sidechain_external").default_bool()
        );
    }
}
