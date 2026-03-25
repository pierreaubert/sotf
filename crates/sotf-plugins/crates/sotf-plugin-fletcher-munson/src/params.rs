//! Fletcher-Munson plugin parameter definitions — single source of truth.
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
// Choice Label Constants
// ============================================================================

pub const LOUDNESS_TYPE_LABELS: &[&str] = &["Momentary", "ShortTerm"];

// ============================================================================
// Parameter Specifications
// ============================================================================

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::float(
        "Playback Volume",
        "playback_volume_db",
        0.0,
        -80.0,
        0.0,
        0.5,
        "dB",
        "Global",
    )
    .setup()
    .doc("Current playback level (from engine)"),
    ParamSpec::float(
        "Reference",
        "reference_level_db",
        -14.0,
        -40.0,
        0.0,
        0.5,
        "dB",
        "Global",
    )
    .setup()
    .doc("Flat-response reference level"),
    ParamSpec::bool_param("Enabled", "enabled", true, "Global")
        .setup()
        .doc("Enable loudness compensation"),
    ParamSpec::float(
        "Smoothing",
        "smoothing_ms",
        30.0,
        1.0,
        200.0,
        1.0,
        "ms",
        "Global",
    )
    .setup()
    .doc("Gain transition time"),
    ParamSpec::bool_param("Auto Gain", "auto_gain_enabled", false, "Auto Gain")
        .output()
        .doc("Auto-normalize output level"),
    ParamSpec::float(
        "Max Correction",
        "auto_gain_max_db",
        12.0,
        0.0,
        24.0,
        1.0,
        "dB",
        "Auto Gain",
    )
    .output()
    .doc("Maximum auto gain correction"),
    ParamSpec::float(
        "AG Smoothing",
        "auto_gain_smoothing_ms",
        100.0,
        10.0,
        500.0,
        5.0,
        "ms",
        "Auto Gain",
    )
    .output()
    .doc("Auto gain transition time"),
    ParamSpec::choice(
        "AG Loudness Type",
        "auto_gain_loudness_type",
        0,
        LOUDNESS_TYPE_LABELS,
        "Auto Gain",
    )
    .output()
    .doc("Loudness measurement window"),
    // Band 1
    ParamSpec::float(
        "Band 1 Freq",
        "band1_freq",
        60.0,
        20.0,
        20000.0,
        5.0,
        "Hz",
        "Band 1",
    )
    .doc("Sub-bass center frequency"),
    ParamSpec::float("Band 1 Q", "band1_q", 0.5, 0.1, 10.0, 0.05, "", "Band 1")
        .doc("Sub-bass bandwidth"),
    ParamSpec::float(
        "Band 1 Max",
        "band1_max_gain",
        15.0,
        0.0,
        24.0,
        0.5,
        "dB",
        "Band 1",
    )
    .doc("Sub-bass max boost"),
    ParamSpec::float(
        "Band 1 Slope",
        "band1_slope",
        0.6,
        0.0,
        1.0,
        0.01,
        "",
        "Band 1",
    )
    .doc("Sub-bass gain per dB volume delta"),
    // Band 2
    ParamSpec::float(
        "Band 2 Freq",
        "band2_freq",
        250.0,
        20.0,
        20000.0,
        10.0,
        "Hz",
        "Band 2",
    )
    .doc("Mid-bass center frequency"),
    ParamSpec::float("Band 2 Q", "band2_q", 0.707, 0.1, 10.0, 0.05, "", "Band 2")
        .doc("Mid-bass bandwidth"),
    ParamSpec::float(
        "Band 2 Max",
        "band2_max_gain",
        8.0,
        0.0,
        24.0,
        0.5,
        "dB",
        "Band 2",
    )
    .doc("Mid-bass max boost"),
    ParamSpec::float(
        "Band 2 Slope",
        "band2_slope",
        0.4,
        0.0,
        1.0,
        0.01,
        "",
        "Band 2",
    )
    .doc("Mid-bass gain per dB volume delta"),
    // Band 3
    ParamSpec::float(
        "Band 3 Freq",
        "band3_freq",
        3500.0,
        20.0,
        20000.0,
        50.0,
        "Hz",
        "Band 3",
    )
    .doc("Presence center frequency"),
    ParamSpec::float("Band 3 Q", "band3_q", 1.0, 0.1, 10.0, 0.05, "", "Band 3")
        .doc("Presence bandwidth"),
    ParamSpec::float(
        "Band 3 Max",
        "band3_max_gain",
        4.0,
        0.0,
        24.0,
        0.5,
        "dB",
        "Band 3",
    )
    .doc("Presence max boost"),
    ParamSpec::float(
        "Band 3 Slope",
        "band3_slope",
        0.2,
        0.0,
        1.0,
        0.01,
        "",
        "Band 3",
    )
    .doc("Presence gain per dB volume delta"),
    // Band 4
    ParamSpec::float(
        "Band 4 Freq",
        "band4_freq",
        12000.0,
        20.0,
        20000.0,
        100.0,
        "Hz",
        "Band 4",
    )
    .doc("Air/brilliance center frequency"),
    ParamSpec::float("Band 4 Q", "band4_q", 0.707, 0.1, 10.0, 0.05, "", "Band 4")
        .doc("Air/brilliance bandwidth"),
    ParamSpec::float(
        "Band 4 Max",
        "band4_max_gain",
        6.0,
        0.0,
        24.0,
        0.5,
        "dB",
        "Band 4",
    )
    .doc("Air/brilliance max boost"),
    ParamSpec::float(
        "Band 4 Slope",
        "band4_slope",
        0.3,
        0.0,
        1.0,
        0.01,
        "",
        "Band 4",
    )
    .doc("Air gain per dB volume delta"),
    ParamSpec::bool_param("ISO 226:2003", "iso_226", false, "Global")
        .setup()
        .doc("Use ISO 226:2003 equal-loudness"),
];

// ============================================================================
// UI Layout
// ============================================================================

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::label(0),  // playback_volume_db (engine-set, read-only)
        ControlSpec::knob(1),   // reference_level_db
        ControlSpec::toggle(2), // enabled
        ControlSpec::knob(3),   // smoothing_ms
    ],
    main: &[
        ControlGroup {
            title: "BAND 1 \u{2014} SUB-BASS",
            controls: &[
                ControlSpec::knob(8),  // band1_freq
                ControlSpec::knob(9),  // band1_q
                ControlSpec::knob(10), // band1_max_gain
                ControlSpec::knob(11), // band1_slope
            ],
        },
        ControlGroup {
            title: "BAND 2 \u{2014} MID-BASS",
            controls: &[
                ControlSpec::knob(12), // band2_freq
                ControlSpec::knob(13), // band2_q
                ControlSpec::knob(14), // band2_max_gain
                ControlSpec::knob(15), // band2_slope
            ],
        },
        ControlGroup {
            title: "BAND 3 \u{2014} PRESENCE",
            controls: &[
                ControlSpec::knob(16), // band3_freq
                ControlSpec::knob(17), // band3_q
                ControlSpec::knob(18), // band3_max_gain
                ControlSpec::knob(19), // band3_slope
            ],
        },
        ControlGroup {
            title: "BAND 4 \u{2014} AIR",
            controls: &[
                ControlSpec::knob(20), // band4_freq
                ControlSpec::knob(21), // band4_q
                ControlSpec::knob(22), // band4_max_gain
                ControlSpec::knob(23), // band4_slope
            ],
        },
    ],
    output: &[
        ControlSpec::toggle(4),   // auto_gain_enabled
        ControlSpec::knob(5),     // auto_gain_max_db
        ControlSpec::knob(6),     // auto_gain_smoothing_ms
        ControlSpec::selector(7), // auto_gain_loudness_type
    ],
    tabs: &[],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::config(120.0, 0.5),
        ColumnConstraint::main(400.0),
        ColumnConstraint::output(120.0, 0.6),
    ],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// Fletcher-Munson plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_playback_volume_db")]
    pub playback_volume_db: f64,
    #[serde(default = "d_reference_level_db")]
    pub reference_level_db: f64,
    #[serde(default = "d_enabled")]
    pub enabled: bool,
    #[serde(default = "d_smoothing_ms")]
    pub smoothing_ms: f64,
    #[serde(default = "d_auto_gain_enabled")]
    pub auto_gain_enabled: bool,
    #[serde(default = "d_auto_gain_max_db")]
    pub auto_gain_max_db: f64,
    #[serde(default = "d_auto_gain_smoothing_ms")]
    pub auto_gain_smoothing_ms: f64,
    #[serde(default = "d_auto_gain_loudness_type")]
    pub auto_gain_loudness_type: usize,
    // Band 1
    #[serde(default = "d_band1_freq")]
    pub band1_freq: f64,
    #[serde(default = "d_band1_q")]
    pub band1_q: f64,
    #[serde(default = "d_band1_max_gain")]
    pub band1_max_gain: f64,
    #[serde(default = "d_band1_slope")]
    pub band1_slope: f64,
    // Band 2
    #[serde(default = "d_band2_freq")]
    pub band2_freq: f64,
    #[serde(default = "d_band2_q")]
    pub band2_q: f64,
    #[serde(default = "d_band2_max_gain")]
    pub band2_max_gain: f64,
    #[serde(default = "d_band2_slope")]
    pub band2_slope: f64,
    // Band 3
    #[serde(default = "d_band3_freq")]
    pub band3_freq: f64,
    #[serde(default = "d_band3_q")]
    pub band3_q: f64,
    #[serde(default = "d_band3_max_gain")]
    pub band3_max_gain: f64,
    #[serde(default = "d_band3_slope")]
    pub band3_slope: f64,
    // Band 4
    #[serde(default = "d_band4_freq")]
    pub band4_freq: f64,
    #[serde(default = "d_band4_q")]
    pub band4_q: f64,
    #[serde(default = "d_band4_max_gain")]
    pub band4_max_gain: f64,
    #[serde(default = "d_band4_slope")]
    pub band4_slope: f64,
    #[serde(default = "d_iso_226")]
    pub iso_226: bool,
}

fn d_playback_volume_db() -> f64 {
    pk(PARAMS, "playback_volume_db").default_f64()
}
fn d_reference_level_db() -> f64 {
    pk(PARAMS, "reference_level_db").default_f64()
}
fn d_enabled() -> bool {
    pk(PARAMS, "enabled").default_bool()
}
fn d_smoothing_ms() -> f64 {
    pk(PARAMS, "smoothing_ms").default_f64()
}
fn d_auto_gain_enabled() -> bool {
    pk(PARAMS, "auto_gain_enabled").default_bool()
}
fn d_auto_gain_max_db() -> f64 {
    pk(PARAMS, "auto_gain_max_db").default_f64()
}
fn d_auto_gain_smoothing_ms() -> f64 {
    pk(PARAMS, "auto_gain_smoothing_ms").default_f64()
}
fn d_auto_gain_loudness_type() -> usize {
    pk(PARAMS, "auto_gain_loudness_type").default_usize()
}
fn d_band1_freq() -> f64 {
    pk(PARAMS, "band1_freq").default_f64()
}
fn d_band1_q() -> f64 {
    pk(PARAMS, "band1_q").default_f64()
}
fn d_band1_max_gain() -> f64 {
    pk(PARAMS, "band1_max_gain").default_f64()
}
fn d_band1_slope() -> f64 {
    pk(PARAMS, "band1_slope").default_f64()
}
fn d_band2_freq() -> f64 {
    pk(PARAMS, "band2_freq").default_f64()
}
fn d_band2_q() -> f64 {
    pk(PARAMS, "band2_q").default_f64()
}
fn d_band2_max_gain() -> f64 {
    pk(PARAMS, "band2_max_gain").default_f64()
}
fn d_band2_slope() -> f64 {
    pk(PARAMS, "band2_slope").default_f64()
}
fn d_band3_freq() -> f64 {
    pk(PARAMS, "band3_freq").default_f64()
}
fn d_band3_q() -> f64 {
    pk(PARAMS, "band3_q").default_f64()
}
fn d_band3_max_gain() -> f64 {
    pk(PARAMS, "band3_max_gain").default_f64()
}
fn d_band3_slope() -> f64 {
    pk(PARAMS, "band3_slope").default_f64()
}
fn d_band4_freq() -> f64 {
    pk(PARAMS, "band4_freq").default_f64()
}
fn d_band4_q() -> f64 {
    pk(PARAMS, "band4_q").default_f64()
}
fn d_band4_max_gain() -> f64 {
    pk(PARAMS, "band4_max_gain").default_f64()
}
fn d_band4_slope() -> f64 {
    pk(PARAMS, "band4_slope").default_f64()
}
fn d_iso_226() -> bool {
    pk(PARAMS, "iso_226").default_bool()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            playback_volume_db: d_playback_volume_db(),
            reference_level_db: d_reference_level_db(),
            enabled: d_enabled(),
            smoothing_ms: d_smoothing_ms(),
            auto_gain_enabled: d_auto_gain_enabled(),
            auto_gain_max_db: d_auto_gain_max_db(),
            auto_gain_smoothing_ms: d_auto_gain_smoothing_ms(),
            auto_gain_loudness_type: d_auto_gain_loudness_type(),
            band1_freq: d_band1_freq(),
            band1_q: d_band1_q(),
            band1_max_gain: d_band1_max_gain(),
            band1_slope: d_band1_slope(),
            band2_freq: d_band2_freq(),
            band2_q: d_band2_q(),
            band2_max_gain: d_band2_max_gain(),
            band2_slope: d_band2_slope(),
            band3_freq: d_band3_freq(),
            band3_q: d_band3_q(),
            band3_max_gain: d_band3_max_gain(),
            band3_slope: d_band3_slope(),
            band4_freq: d_band4_freq(),
            band4_q: d_band4_q(),
            band4_max_gain: d_band4_max_gain(),
            band4_slope: d_band4_slope(),
            iso_226: d_iso_226(),
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
    const PLUGIN_TYPE_KEY: &'static str = "fletcher_munson";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.playback_volume_db),
            1 => Some(self.reference_level_db),
            2 => Some(if self.enabled { 1.0 } else { 0.0 }),
            3 => Some(self.smoothing_ms),
            4 => Some(if self.auto_gain_enabled { 1.0 } else { 0.0 }),
            5 => Some(self.auto_gain_max_db),
            6 => Some(self.auto_gain_smoothing_ms),
            7 => Some(self.auto_gain_loudness_type as f64),
            8 => Some(self.band1_freq),
            9 => Some(self.band1_q),
            10 => Some(self.band1_max_gain),
            11 => Some(self.band1_slope),
            12 => Some(self.band2_freq),
            13 => Some(self.band2_q),
            14 => Some(self.band2_max_gain),
            15 => Some(self.band2_slope),
            16 => Some(self.band3_freq),
            17 => Some(self.band3_q),
            18 => Some(self.band3_max_gain),
            19 => Some(self.band3_slope),
            20 => Some(self.band4_freq),
            21 => Some(self.band4_q),
            22 => Some(self.band4_max_gain),
            23 => Some(self.band4_slope),
            24 => Some(if self.iso_226 { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.playback_volume_db = value,
            1 => self.reference_level_db = value,
            2 => self.enabled = value > 0.5,
            3 => self.smoothing_ms = value,
            4 => self.auto_gain_enabled = value > 0.5,
            5 => self.auto_gain_max_db = value,
            6 => self.auto_gain_smoothing_ms = value,
            7 => self.auto_gain_loudness_type = value as usize,
            8 => self.band1_freq = value,
            9 => self.band1_q = value,
            10 => self.band1_max_gain = value,
            11 => self.band1_slope = value,
            12 => self.band2_freq = value,
            13 => self.band2_q = value,
            14 => self.band2_max_gain = value,
            15 => self.band2_slope = value,
            16 => self.band3_freq = value,
            17 => self.band3_q = value,
            18 => self.band3_max_gain = value,
            19 => self.band3_slope = value,
            20 => self.band4_freq = value,
            21 => self.band4_q = value,
            22 => self.band4_max_gain = value,
            23 => self.band4_slope = value,
            24 => self.iso_226 = value > 0.5,
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
        assert_eq!(original.playback_volume_db, restored.playback_volume_db);
        assert_eq!(original.reference_level_db, restored.reference_level_db);
        assert_eq!(original.enabled, restored.enabled);
        assert_eq!(original.smoothing_ms, restored.smoothing_ms);
        assert_eq!(original.auto_gain_enabled, restored.auto_gain_enabled);
        assert_eq!(original.auto_gain_max_db, restored.auto_gain_max_db);
        assert_eq!(
            original.auto_gain_smoothing_ms,
            restored.auto_gain_smoothing_ms
        );
        assert_eq!(
            original.auto_gain_loudness_type,
            restored.auto_gain_loudness_type
        );
        assert_eq!(original.band1_freq, restored.band1_freq);
        assert_eq!(original.band1_q, restored.band1_q);
        assert_eq!(original.band1_max_gain, restored.band1_max_gain);
        assert_eq!(original.band1_slope, restored.band1_slope);
        assert_eq!(original.band2_freq, restored.band2_freq);
        assert_eq!(original.band2_q, restored.band2_q);
        assert_eq!(original.band2_max_gain, restored.band2_max_gain);
        assert_eq!(original.band2_slope, restored.band2_slope);
        assert_eq!(original.band3_freq, restored.band3_freq);
        assert_eq!(original.band3_q, restored.band3_q);
        assert_eq!(original.band3_max_gain, restored.band3_max_gain);
        assert_eq!(original.band3_slope, restored.band3_slope);
        assert_eq!(original.band4_freq, restored.band4_freq);
        assert_eq!(original.band4_q, restored.band4_q);
        assert_eq!(original.band4_max_gain, restored.band4_max_gain);
        assert_eq!(original.band4_slope, restored.band4_slope);
        assert_eq!(original.iso_226, restored.iso_226);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(
            p.playback_volume_db,
            pk(PARAMS, "playback_volume_db").default_f64()
        );
        assert_eq!(
            p.reference_level_db,
            pk(PARAMS, "reference_level_db").default_f64()
        );
        assert_eq!(p.enabled, pk(PARAMS, "enabled").default_bool());
        assert_eq!(p.smoothing_ms, pk(PARAMS, "smoothing_ms").default_f64());
        assert_eq!(
            p.auto_gain_enabled,
            pk(PARAMS, "auto_gain_enabled").default_bool()
        );
        assert_eq!(
            p.auto_gain_max_db,
            pk(PARAMS, "auto_gain_max_db").default_f64()
        );
        assert_eq!(
            p.auto_gain_smoothing_ms,
            pk(PARAMS, "auto_gain_smoothing_ms").default_f64()
        );
        assert_eq!(
            p.auto_gain_loudness_type,
            pk(PARAMS, "auto_gain_loudness_type").default_usize()
        );
        assert_eq!(p.band1_freq, pk(PARAMS, "band1_freq").default_f64());
        assert_eq!(p.band1_q, pk(PARAMS, "band1_q").default_f64());
        assert_eq!(
            p.band1_max_gain,
            pk(PARAMS, "band1_max_gain").default_f64()
        );
        assert_eq!(p.band1_slope, pk(PARAMS, "band1_slope").default_f64());
        assert_eq!(p.band2_freq, pk(PARAMS, "band2_freq").default_f64());
        assert_eq!(p.band2_q, pk(PARAMS, "band2_q").default_f64());
        assert_eq!(
            p.band2_max_gain,
            pk(PARAMS, "band2_max_gain").default_f64()
        );
        assert_eq!(p.band2_slope, pk(PARAMS, "band2_slope").default_f64());
        assert_eq!(p.band3_freq, pk(PARAMS, "band3_freq").default_f64());
        assert_eq!(p.band3_q, pk(PARAMS, "band3_q").default_f64());
        assert_eq!(
            p.band3_max_gain,
            pk(PARAMS, "band3_max_gain").default_f64()
        );
        assert_eq!(p.band3_slope, pk(PARAMS, "band3_slope").default_f64());
        assert_eq!(p.band4_freq, pk(PARAMS, "band4_freq").default_f64());
        assert_eq!(p.band4_q, pk(PARAMS, "band4_q").default_f64());
        assert_eq!(
            p.band4_max_gain,
            pk(PARAMS, "band4_max_gain").default_f64()
        );
        assert_eq!(p.band4_slope, pk(PARAMS, "band4_slope").default_f64());
        assert_eq!(p.iso_226, pk(PARAMS, "iso_226").default_bool());
    }
}
