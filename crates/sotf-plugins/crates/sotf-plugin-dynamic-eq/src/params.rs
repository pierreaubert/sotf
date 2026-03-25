//! Dynamic EQ plugin parameter definitions -- single source of truth.
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
// Constants
// ============================================================================

pub const MAX_BANDS: usize = 8;

// ============================================================================
// Parameter Specifications
// ============================================================================

pub const PARAMS: &[ParamSpec] = &[
    // Global: num_bands
    ParamSpec::int("Num Bands", "num_bands", 4, 1, 8, 1, "Bands", "Setup")
        .setup()
        .doc("Number of dynamic EQ bands"),
    // Global: threshold
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
    .doc("Global detection threshold"),
    // Global: ratio
    ParamSpec::float("Ratio", "ratio", 2.0, 1.0, 20.0, 0.1, ":1", "Dynamics")
        .doc("Global dynamics ratio"),
    // Global: attack
    ParamSpec::float(
        "Attack",
        "attack",
        5.0,
        0.1,
        100.0,
        0.1,
        "ms",
        "Timing",
    )
    .doc("Global attack time"),
    // Global: release
    ParamSpec::float(
        "Release",
        "release",
        50.0,
        10.0,
        1000.0,
        1.0,
        "ms",
        "Timing",
    )
    .doc("Global release time"),
    // Global: knee
    ParamSpec::float("Knee", "knee", 6.0, 0.0, 20.0, 0.5, "dB", "Dynamics")
        .doc("Global soft knee"),
    // Global: link_channels
    ParamSpec::bool_labeled(
        "Link Channels",
        "link_channels",
        true,
        "Linked",
        "Unlinked",
        "Channels",
    )
    .setup()
    .doc("Stereo-link detection"),
    // Global: mix
    ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.01, "%", "Output")
        .scaled(100.0)
        .output()
        .doc("Dry/wet mix"),
];

/// Number of global parameters.
pub const NUM_GLOBAL_PARAMS: usize = 8;

/// Per-band parameter template (7 params per band).
pub const BAND_PARAMS: &[ParamSpec] = &[
    ParamSpec::float(
        "Frequency",
        "frequency",
        1000.0,
        20.0,
        20000.0,
        1.0,
        "Hz",
        "EQ",
    )
    .doc("Band center frequency"),
    ParamSpec::float("Q", "q", 1.0, 0.1, 10.0, 0.01, "", "EQ")
        .doc("Band Q factor (bandwidth)"),
    ParamSpec::float("Gain", "gain", 0.0, -24.0, 24.0, 0.1, "dB", "EQ")
        .doc("Target EQ gain when triggered"),
    ParamSpec::float(
        "Threshold",
        "band_threshold",
        -20.0,
        -60.0,
        0.0,
        0.5,
        "dB",
        "Dynamics",
    )
    .doc("Band-specific threshold (overrides global)"),
    ParamSpec::float(
        "Ratio",
        "band_ratio",
        2.0,
        1.0,
        20.0,
        0.1,
        ":1",
        "Dynamics",
    )
    .doc("Band-specific ratio (overrides global)"),
    ParamSpec::bool_param("Active", "active", true, "Band").doc("Enable/disable this band"),
    ParamSpec::bool_param("Solo", "solo", false, "Band").doc("Solo this band"),
];

/// Number of parameters per band.
pub const NUM_BAND_PARAMS: usize = 7;

// ============================================================================
// UI Layout
// ============================================================================

/// Dynamic EQ layout:
/// idx 0=num_bands, 1=threshold, 2=ratio, 3=attack, 4=release, 5=knee,
/// 6=link_channels, 7=mix
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::slider(0), // num_bands
        ControlSpec::toggle(6), // link_channels
    ],
    main: &[
        ControlGroup {
            title: "DYNAMICS",
            controls: &[
                ControlSpec::slider(1), // threshold
                ControlSpec::slider(2), // ratio
                ControlSpec::slider(3), // attack
                ControlSpec::slider(4), // release
                ControlSpec::slider(5), // knee
            ],
        },
    ],
    output: &[
        ControlSpec::meter(-30.0, 0.0), // GR meter
        ControlSpec::knob(7),           // mix
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
// Serializable Band Parameters
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandParams {
    #[serde(default = "d_frequency")]
    pub frequency: f64,
    #[serde(default = "d_q")]
    pub q: f64,
    #[serde(default = "d_gain")]
    pub gain: f64,
    #[serde(default = "d_band_threshold")]
    pub band_threshold: f64,
    #[serde(default = "d_band_ratio")]
    pub band_ratio: f64,
    #[serde(default = "d_active")]
    pub active: bool,
    #[serde(default)]
    pub solo: bool,
}

fn d_frequency() -> f64 {
    pk(BAND_PARAMS, "frequency").default_f64()
}
fn d_q() -> f64 {
    pk(BAND_PARAMS, "q").default_f64()
}
fn d_gain() -> f64 {
    pk(BAND_PARAMS, "gain").default_f64()
}
fn d_band_threshold() -> f64 {
    pk(BAND_PARAMS, "band_threshold").default_f64()
}
fn d_band_ratio() -> f64 {
    pk(BAND_PARAMS, "band_ratio").default_f64()
}
fn d_active() -> bool {
    pk(BAND_PARAMS, "active").default_bool()
}

impl Default for BandParams {
    fn default() -> Self {
        Self {
            frequency: d_frequency(),
            q: d_q(),
            gain: d_gain(),
            band_threshold: d_band_threshold(),
            band_ratio: d_band_ratio(),
            active: d_active(),
            solo: false,
        }
    }
}

// ============================================================================
// Serializable Parameter State
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_num_bands")]
    pub num_bands: i64,
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
    #[serde(default = "d_link_channels")]
    pub link_channels: bool,
    #[serde(default = "d_mix")]
    pub mix: f64,
    #[serde(default = "d_bands")]
    pub bands: Vec<BandParams>,
}

fn d_num_bands() -> i64 {
    pk(PARAMS, "num_bands").default_f64() as i64
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
fn d_link_channels() -> bool {
    pk(PARAMS, "link_channels").default_bool()
}
fn d_mix() -> f64 {
    pk(PARAMS, "mix").default_f64()
}
fn d_bands() -> Vec<BandParams> {
    let n = d_num_bands() as usize;
    (0..n).map(|_| BandParams::default()).collect()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            num_bands: d_num_bands(),
            threshold: d_threshold(),
            ratio: d_ratio(),
            attack: d_attack(),
            release: d_release(),
            knee: d_knee(),
            link_channels: d_link_channels(),
            mix: d_mix(),
            bands: d_bands(),
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
    const PLUGIN_TYPE_KEY: &'static str = "dynamic_eq";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.num_bands as f64),
            1 => Some(self.threshold),
            2 => Some(self.ratio),
            3 => Some(self.attack),
            4 => Some(self.release),
            5 => Some(self.knee),
            6 => Some(if self.link_channels { 1.0 } else { 0.0 }),
            7 => Some(self.mix),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.num_bands = value as i64,
            1 => self.threshold = value,
            2 => self.ratio = value,
            3 => self.attack = value,
            4 => self.release = value,
            5 => self.knee = value,
            6 => self.link_channels = value >= 0.5,
            7 => self.mix = value,
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
        assert_eq!(original.num_bands, restored.num_bands);
        assert_eq!(original.threshold, restored.threshold);
        assert_eq!(original.ratio, restored.ratio);
        assert_eq!(original.attack, restored.attack);
        assert_eq!(original.release, restored.release);
        assert_eq!(original.knee, restored.knee);
        assert_eq!(original.link_channels, restored.link_channels);
        assert_eq!(original.mix, restored.mix);
        assert_eq!(original.bands.len(), restored.bands.len());
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.num_bands, pk(PARAMS, "num_bands").default_f64() as i64);
        assert_eq!(p.threshold, pk(PARAMS, "threshold").default_f64());
        assert_eq!(p.ratio, pk(PARAMS, "ratio").default_f64());
        assert_eq!(p.attack, pk(PARAMS, "attack").default_f64());
        assert_eq!(p.release, pk(PARAMS, "release").default_f64());
        assert_eq!(p.knee, pk(PARAMS, "knee").default_f64());
        assert_eq!(p.link_channels, pk(PARAMS, "link_channels").default_bool());
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64());
    }
}
