//! Expander plugin parameter definitions — single source of truth.
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
        -40.0,
        -80.0,
        0.0,
        1.0,
        "dB",
        "Dynamics",
    )
    .doc("Level below which expansion starts"),
    ParamSpec::float("Ratio", "ratio", 2.0, 1.0, 20.0, 0.1, ":1", "Dynamics")
        .doc("Expansion amount (input:output)"),
    ParamSpec::float("Attack", "attack", 1.0, 0.1, 50.0, 0.1, "ms", "Timing")
        .doc("Time to reach full expansion"),
    ParamSpec::float(
        "Release", "release", 100.0, 10.0, 2000.0, 5.0, "ms", "Timing",
    )
    .doc("Time to return to unity gain"),
    ParamSpec::float("Range", "range", 40.0, 0.0, 80.0, 1.0, "dB", "Dynamics")
        .doc("Max attenuation below threshold"),
    ParamSpec::float("Knee", "knee", 6.0, 0.0, 20.0, 0.5, "dB", "Dynamics")
        .doc("Softness of threshold transition"),
    ParamSpec::float(
        "Hysteresis",
        "hysteresis",
        4.0,
        0.0,
        12.0,
        0.1,
        "dB",
        "Dynamics",
    )
    .doc("Open/close threshold difference"),
    ParamSpec::float("Hold", "hold", 10.0, 0.0, 500.0, 1.0, "ms", "Timing")
        .doc("Minimum open time after trigger"),
    ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.01, "%", "Output")
        .scaled(100.0)
        .output()
        .doc("Dry/wet blend"),
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
        500.0,
        5.0,
        "Hz",
        "Sidechain",
    )
    .setup()
    .doc("High-pass on detector input"),
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
    ParamSpec::choice(
        "Detection Mode",
        "detection_mode",
        0,
        DETECTION_MODES,
        "Sidechain",
    )
    .setup()
    .doc("Peak or RMS level detection"),
    ParamSpec::bool_param(
        "Measured Auto Makeup",
        "measured_auto_makeup",
        false,
        "Output",
    )
    .output()
    .doc("Makeup based on measured reduction"),
];

// ============================================================================
// UI Layout
// ============================================================================

/// Expander: idx 0=threshold, 1=ratio, 2=attack, 3=release, 4=range, 5=knee,
/// 6=hysteresis, 7=hold, 8=mix, 9=auto_makeup, 10=link, 11=sidechain_hpf,
/// 12=lookahead_ms, 13=detection_mode, 14=measured_auto_makeup
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::toggle(10), // link_channels
        ControlSpec::knob(11),   // sidechain_hpf_hz
    ],
    main: &[
        ControlGroup {
            title: "DYNAMICS",
            controls: &[
                ControlSpec::slider(0), // threshold
                ControlSpec::slider(1), // ratio
                ControlSpec::slider(4), // range
                ControlSpec::slider(5), // knee
            ],
        },
        ControlGroup {
            title: "TIMING",
            controls: &[
                ControlSpec::slider(2), // attack
                ControlSpec::slider(3), // release
                ControlSpec::slider(7), // hold
            ],
        },
    ],
    output: &[
        ControlSpec::meter(-30.0, 0.0), // GR meter
        ControlSpec::toggle(9),         // auto_makeup
        ControlSpec::knob(8),           // mix
    ],
    tabs: &[TabSpec {
        name: "Advanced",
        controls: &[ControlSpec::knob(6)], // hysteresis
    }],
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

/// Expander plugin parameters.
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
    #[serde(default = "d_range")]
    pub range: f64,
    #[serde(default = "d_knee")]
    pub knee: f64,
    #[serde(default = "d_hysteresis")]
    pub hysteresis: f64,
    #[serde(default = "d_hold")]
    pub hold: f64,
    #[serde(default = "d_mix")]
    pub mix: f64,
    #[serde(default = "d_auto_makeup")]
    pub auto_makeup: bool,
    #[serde(default = "d_link_channels")]
    pub link_channels: bool,
    #[serde(default = "d_sidechain_hpf_hz")]
    pub sidechain_hpf_hz: f64,
    #[serde(default = "d_lookahead_ms")]
    pub lookahead_ms: f64,
    #[serde(default = "d_detection_mode")]
    pub detection_mode: String,
    #[serde(default = "d_measured_auto_makeup")]
    pub measured_auto_makeup: bool,
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
fn d_range() -> f64 {
    pk(PARAMS, "range").default_f64()
}
fn d_knee() -> f64 {
    pk(PARAMS, "knee").default_f64()
}
fn d_hysteresis() -> f64 {
    pk(PARAMS, "hysteresis").default_f64()
}
fn d_hold() -> f64 {
    pk(PARAMS, "hold").default_f64()
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
fn d_lookahead_ms() -> f64 {
    pk(PARAMS, "lookahead_ms").default_f64()
}
fn d_detection_mode() -> String {
    DETECTION_MODES[0].to_string()
}
fn d_measured_auto_makeup() -> bool {
    pk(PARAMS, "measured_auto_makeup").default_bool()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            threshold: d_threshold(),
            ratio: d_ratio(),
            attack: d_attack(),
            release: d_release(),
            range: d_range(),
            knee: d_knee(),
            hysteresis: d_hysteresis(),
            hold: d_hold(),
            mix: d_mix(),
            auto_makeup: d_auto_makeup(),
            link_channels: d_link_channels(),
            sidechain_hpf_hz: d_sidechain_hpf_hz(),
            lookahead_ms: d_lookahead_ms(),
            detection_mode: d_detection_mode(),
            measured_auto_makeup: d_measured_auto_makeup(),
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
    const PLUGIN_TYPE_KEY: &'static str = "expander";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.threshold),
            1 => Some(self.ratio),
            2 => Some(self.attack),
            3 => Some(self.release),
            4 => Some(self.range),
            5 => Some(self.knee),
            6 => Some(self.hysteresis),
            7 => Some(self.hold),
            8 => Some(self.mix),
            9 => Some(if self.auto_makeup { 1.0 } else { 0.0 }),
            10 => Some(if self.link_channels { 1.0 } else { 0.0 }),
            11 => Some(self.sidechain_hpf_hz),
            12 => Some(self.lookahead_ms),
            13 => Some(
                DETECTION_MODES
                    .iter()
                    .position(|&m| m.eq_ignore_ascii_case(&self.detection_mode))
                    .unwrap_or(0) as f64,
            ),
            14 => Some(if self.measured_auto_makeup { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.threshold = value,
            1 => self.ratio = value,
            2 => self.attack = value,
            3 => self.release = value,
            4 => self.range = value,
            5 => self.knee = value,
            6 => self.hysteresis = value,
            7 => self.hold = value,
            8 => self.mix = value,
            9 => self.auto_makeup = value > 0.5,
            10 => self.link_channels = value > 0.5,
            11 => self.sidechain_hpf_hz = value,
            12 => self.lookahead_ms = value,
            13 => {
                let idx = value as usize;
                if let Some(&label) = DETECTION_MODES.get(idx) {
                    self.detection_mode = label.to_string();
                }
            }
            14 => self.measured_auto_makeup = value > 0.5,
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
        assert_eq!(original.range, restored.range);
        assert_eq!(original.knee, restored.knee);
        assert_eq!(original.hysteresis, restored.hysteresis);
        assert_eq!(original.hold, restored.hold);
        assert_eq!(original.mix, restored.mix);
        assert_eq!(original.auto_makeup, restored.auto_makeup);
        assert_eq!(original.link_channels, restored.link_channels);
        assert_eq!(original.sidechain_hpf_hz, restored.sidechain_hpf_hz);
        assert_eq!(original.lookahead_ms, restored.lookahead_ms);
        assert_eq!(original.detection_mode, restored.detection_mode);
        assert_eq!(
            original.measured_auto_makeup,
            restored.measured_auto_makeup
        );
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.threshold, pk(PARAMS, "threshold").default_f64());
        assert_eq!(p.ratio, pk(PARAMS, "ratio").default_f64());
        assert_eq!(p.attack, pk(PARAMS, "attack").default_f64());
        assert_eq!(p.release, pk(PARAMS, "release").default_f64());
        assert_eq!(p.range, pk(PARAMS, "range").default_f64());
        assert_eq!(p.knee, pk(PARAMS, "knee").default_f64());
        assert_eq!(p.hysteresis, pk(PARAMS, "hysteresis").default_f64());
        assert_eq!(p.hold, pk(PARAMS, "hold").default_f64());
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
        assert_eq!(p.lookahead_ms, pk(PARAMS, "lookahead_ms").default_f64());
        assert_eq!(p.detection_mode, DETECTION_MODES[0]);
        assert_eq!(
            p.measured_auto_makeup,
            pk(PARAMS, "measured_auto_makeup").default_bool()
        );
    }
}
