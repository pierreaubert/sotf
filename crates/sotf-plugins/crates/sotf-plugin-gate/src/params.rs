//! Gate plugin parameter definitions — single source of truth.
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
// Detection Mode / HPF Order Constants
// ============================================================================

pub const DETECTION_MODES: &[&str] = &["Peak", "RMS"];
pub const HPF_ORDERS: &[&str] = &["2nd", "4th"];

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
    .doc("Level below which gate closes"),
    ParamSpec::float("Ratio", "ratio", 10.0, 1.0, 100.0, 0.1, ":1", "Dynamics")
        .doc("Attenuation depth when closed"),
    ParamSpec::float("Attack", "attack", 1.0, 0.1, 50.0, 0.1, "ms", "Timing")
        .doc("Time for gate to open"),
    ParamSpec::float("Hold", "hold", 10.0, 0.0, 1000.0, 1.0, "ms", "Timing")
        .doc("Minimum open time after trigger"),
    ParamSpec::float(
        "Release", "release", 100.0, 10.0, 2000.0, 5.0, "ms", "Timing",
    )
    .doc("Time for gate to close"),
    ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.01, "%", "Output")
        .scaled(100.0)
        .output()
        .doc("Dry/wet blend"),
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
        0.0,
        0.0,
        200.0,
        5.0,
        "Hz",
        "Sidechain",
    )
    .setup()
    .doc("High-pass on detector input"),
    ParamSpec::choice(
        "HPF Order",
        "sidechain_hpf_order",
        0,
        HPF_ORDERS,
        "Sidechain",
    )
    .setup()
    .doc("Sidechain HPF filter order"),
    ParamSpec::choice(
        "Detection",
        "detection_mode",
        0,
        DETECTION_MODES,
        "Sidechain",
    )
    .setup()
    .doc("Level detection mode"),
    ParamSpec::bool_labeled(
        "Ext Sidechain",
        "sidechain_external",
        false,
        "On",
        "Off",
        "Sidechain",
    )
    .setup()
    .doc("Use external sidechain input"),
    ParamSpec::float("Range", "range_db", 80.0, 0.0, 120.0, 1.0, "dB", "Dynamics")
        .doc("Max attenuation when gate closed"),
    ParamSpec::float(
        "Hysteresis",
        "hysteresis_db",
        4.0,
        0.0,
        12.0,
        0.1,
        "dB",
        "Dynamics",
    )
    .doc("Open/close threshold difference"),
    ParamSpec::float("Knee", "knee_db", 0.0, 0.0, 20.0, 0.5, "dB", "Dynamics")
        .doc("Softness of threshold transition"),
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
];

// ============================================================================
// UI Layout
// ============================================================================

/// Gate: idx 0=threshold, 1=ratio, 2=attack, 3=hold, 4=release, 5=mix, 6=link, 7=sidechain_hpf,
/// 8=sidechain_hpf_order, 9=detection_mode, 10=sidechain_external,
/// 11=range_db, 12=hysteresis_db, 13=knee_db, 14=lookahead_ms
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::toggle(6),   // link_channels
        ControlSpec::knob(7),     // sidechain_hpf_hz
        ControlSpec::selector(8), // sidechain_hpf_order
        ControlSpec::selector(9), // detection_mode
    ],
    main: &[
        ControlGroup {
            title: "DYNAMICS",
            controls: &[
                ControlSpec::slider(0), // threshold
                ControlSpec::slider(1), // ratio
            ],
        },
        ControlGroup {
            title: "TIMING",
            controls: &[
                ControlSpec::slider(2), // attack
                ControlSpec::slider(3), // hold
                ControlSpec::slider(4), // release
            ],
        },
    ],
    output: &[
        ControlSpec::meter(-30.0, 0.0), // GR meter
        ControlSpec::knob(5),           // mix
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
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// Gate plugin parameters.
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
    #[serde(default = "d_hold")]
    pub hold: f64,
    #[serde(default = "d_release")]
    pub release: f64,
    #[serde(default = "d_mix")]
    pub mix: f64,
    #[serde(default = "d_link_channels")]
    pub link_channels: bool,
    #[serde(default = "d_sidechain_hpf_hz")]
    pub sidechain_hpf_hz: f64,
    #[serde(default = "d_sidechain_hpf_order")]
    pub sidechain_hpf_order: String,
    #[serde(default = "d_detection_mode")]
    pub detection_mode: String,
    #[serde(default = "d_sidechain_external")]
    pub sidechain_external: bool,
    #[serde(default = "d_range_db")]
    pub range_db: f64,
    #[serde(default = "d_hysteresis_db")]
    pub hysteresis_db: f64,
    #[serde(default = "d_knee_db")]
    pub knee_db: f64,
    #[serde(default = "d_lookahead_ms")]
    pub lookahead_ms: f64,
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
fn d_hold() -> f64 {
    pk(PARAMS, "hold").default_f64()
}
fn d_release() -> f64 {
    pk(PARAMS, "release").default_f64()
}
fn d_mix() -> f64 {
    pk(PARAMS, "mix").default_f64()
}
fn d_link_channels() -> bool {
    pk(PARAMS, "link_channels").default_bool()
}
fn d_sidechain_hpf_hz() -> f64 {
    pk(PARAMS, "sidechain_hpf_hz").default_f64()
}
fn d_sidechain_hpf_order() -> String {
    HPF_ORDERS[0].to_string()
}
fn d_detection_mode() -> String {
    DETECTION_MODES[0].to_string()
}
fn d_sidechain_external() -> bool {
    pk(PARAMS, "sidechain_external").default_bool()
}
fn d_range_db() -> f64 {
    pk(PARAMS, "range_db").default_f64()
}
fn d_hysteresis_db() -> f64 {
    pk(PARAMS, "hysteresis_db").default_f64()
}
fn d_knee_db() -> f64 {
    pk(PARAMS, "knee_db").default_f64()
}
fn d_lookahead_ms() -> f64 {
    pk(PARAMS, "lookahead_ms").default_f64()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            threshold: d_threshold(),
            ratio: d_ratio(),
            attack: d_attack(),
            hold: d_hold(),
            release: d_release(),
            mix: d_mix(),
            link_channels: d_link_channels(),
            sidechain_hpf_hz: d_sidechain_hpf_hz(),
            sidechain_hpf_order: d_sidechain_hpf_order(),
            detection_mode: d_detection_mode(),
            sidechain_external: d_sidechain_external(),
            range_db: d_range_db(),
            hysteresis_db: d_hysteresis_db(),
            knee_db: d_knee_db(),
            lookahead_ms: d_lookahead_ms(),
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
    const PLUGIN_TYPE_KEY: &'static str = "gate";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.threshold),
            1 => Some(self.ratio),
            2 => Some(self.attack),
            3 => Some(self.hold),
            4 => Some(self.release),
            5 => Some(self.mix),
            6 => Some(if self.link_channels { 1.0 } else { 0.0 }),
            7 => Some(self.sidechain_hpf_hz),
            8 => Some(
                HPF_ORDERS
                    .iter()
                    .position(|&m| m.eq_ignore_ascii_case(&self.sidechain_hpf_order))
                    .unwrap_or(0) as f64,
            ),
            9 => Some(
                DETECTION_MODES
                    .iter()
                    .position(|&m| m.eq_ignore_ascii_case(&self.detection_mode))
                    .unwrap_or(0) as f64,
            ),
            10 => Some(if self.sidechain_external { 1.0 } else { 0.0 }),
            11 => Some(self.range_db),
            12 => Some(self.hysteresis_db),
            13 => Some(self.knee_db),
            14 => Some(self.lookahead_ms),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.threshold = value,
            1 => self.ratio = value,
            2 => self.attack = value,
            3 => self.hold = value,
            4 => self.release = value,
            5 => self.mix = value,
            6 => self.link_channels = value > 0.5,
            7 => self.sidechain_hpf_hz = value,
            8 => {
                let idx = value as usize;
                if let Some(&label) = HPF_ORDERS.get(idx) {
                    self.sidechain_hpf_order = label.to_string();
                }
            }
            9 => {
                let idx = value as usize;
                if let Some(&label) = DETECTION_MODES.get(idx) {
                    self.detection_mode = label.to_string();
                }
            }
            10 => self.sidechain_external = value > 0.5,
            11 => self.range_db = value,
            12 => self.hysteresis_db = value,
            13 => self.knee_db = value,
            14 => self.lookahead_ms = value,
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
        assert_eq!(original.hold, restored.hold);
        assert_eq!(original.release, restored.release);
        assert_eq!(original.mix, restored.mix);
        assert_eq!(original.link_channels, restored.link_channels);
        assert_eq!(original.sidechain_hpf_hz, restored.sidechain_hpf_hz);
        assert_eq!(original.sidechain_hpf_order, restored.sidechain_hpf_order);
        assert_eq!(original.detection_mode, restored.detection_mode);
        assert_eq!(original.sidechain_external, restored.sidechain_external);
        assert_eq!(original.range_db, restored.range_db);
        assert_eq!(original.hysteresis_db, restored.hysteresis_db);
        assert_eq!(original.knee_db, restored.knee_db);
        assert_eq!(original.lookahead_ms, restored.lookahead_ms);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.threshold, pk(PARAMS, "threshold").default_f64());
        assert_eq!(p.ratio, pk(PARAMS, "ratio").default_f64());
        assert_eq!(p.attack, pk(PARAMS, "attack").default_f64());
        assert_eq!(p.hold, pk(PARAMS, "hold").default_f64());
        assert_eq!(p.release, pk(PARAMS, "release").default_f64());
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64());
        assert_eq!(p.link_channels, pk(PARAMS, "link_channels").default_bool());
        assert_eq!(
            p.sidechain_hpf_hz,
            pk(PARAMS, "sidechain_hpf_hz").default_f64()
        );
        assert_eq!(p.sidechain_hpf_order, HPF_ORDERS[0]);
        assert_eq!(p.detection_mode, DETECTION_MODES[0]);
        assert_eq!(
            p.sidechain_external,
            pk(PARAMS, "sidechain_external").default_bool()
        );
        assert_eq!(p.range_db, pk(PARAMS, "range_db").default_f64());
        assert_eq!(p.hysteresis_db, pk(PARAMS, "hysteresis_db").default_f64());
        assert_eq!(p.knee_db, pk(PARAMS, "knee_db").default_f64());
        assert_eq!(p.lookahead_ms, pk(PARAMS, "lookahead_ms").default_f64());
    }
}
