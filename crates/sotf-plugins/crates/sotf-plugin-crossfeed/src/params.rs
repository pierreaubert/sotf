//! Crossfeed plugin parameter definitions — single source of truth.
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
// Choice Label Constants
// ============================================================================

pub const MODE_LABELS: &[&str] = &["Disable", "Bauer", "Meier", "Multiband"];
pub const PRESET_LABELS: &[&str] = &["Default", "Cmoy", "Meier", "Mb", "Off"];

// ============================================================================
// Parameter Specifications
// ============================================================================

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::choice("Mode", "crossfeed_mode", 0, MODE_LABELS, "General")
        .structural()
        .setup()
        .doc("Crossfeed algorithm selection"),
    ParamSpec::choice("Preset", "crossfeed_preset", 0, PRESET_LABELS, "General")
        .structural()
        .setup()
        .doc("Load preset parameter values"),
    ParamSpec::bool_param("Enabled", "enabled", true, "General")
        .setup()
        .doc("Enable crossfeed processing"),
    ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.05, "%", "General")
        .output()
        .doc("Dry/wet blend"),
    // Bauer
    ParamSpec::float(
        "Bauer Cutoff",
        "bauer_fcut_hz",
        700.0,
        400.0,
        1000.0,
        10.0,
        "Hz",
        "Bauer",
    )
    .doc("Bauer shelving filter frequency"),
    ParamSpec::float(
        "Bauer Feed",
        "bauer_feed_db",
        4.5,
        0.0,
        15.0,
        0.5,
        "dB",
        "Bauer",
    )
    .doc("Bauer cross-feed level"),
    // Meier
    ParamSpec::float(
        "Meier Level",
        "meier_level",
        30.0,
        0.0,
        100.0,
        1.0,
        "%",
        "Meier",
    )
    .doc("Meier crossfeed strength"),
    // Multiband
    ParamSpec::float(
        "MB Low Freq",
        "mb_low_freq_hz",
        150.0,
        50.0,
        500.0,
        5.0,
        "Hz",
        "Multiband",
    )
    .doc("Low/mid band split frequency"),
    ParamSpec::float(
        "MB Mid/High Freq",
        "mb_mid_high_freq_hz",
        5700.0,
        2000.0,
        15000.0,
        50.0,
        "Hz",
        "Multiband",
    )
    .doc("Mid/high band split frequency"),
    ParamSpec::float(
        "MB Low Feed",
        "mb_low_feed_db",
        0.0,
        -20.0,
        0.0,
        0.5,
        "dB",
        "Multiband",
    )
    .doc("Low band cross-feed level"),
    ParamSpec::float(
        "MB Mid Feed",
        "mb_mid_feed_db",
        6.0,
        0.0,
        15.0,
        0.5,
        "dB",
        "Multiband",
    )
    .doc("Mid band cross-feed level"),
    ParamSpec::float(
        "MB High Feed",
        "mb_high_feed_db",
        3.0,
        0.0,
        15.0,
        0.5,
        "dB",
        "Multiband",
    )
    .doc("High band cross-feed level"),
    // ITD (Interaural Time Difference)
    ParamSpec::float(
        "ITD Delay",
        "itd_delay_ms",
        0.0,
        0.0,
        1.0,
        0.01,
        "ms",
        "General",
    )
    .doc("Interaural time difference"),
    // Auto Gain
    ParamSpec::bool_param("Auto Gain", "autogain_enabled", false, "Auto Gain")
        .output()
        .doc("Auto-normalize output level"),
    ParamSpec::float(
        "Target LUFS",
        "autogain_target_lufs",
        -18.0,
        -40.0,
        -12.0,
        0.5,
        "LUFS",
        "Auto Gain",
    )
    .output()
    .doc("Target loudness level"),
    ParamSpec::float(
        "Max Gain",
        "autogain_max_gain_db",
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
        "Smoothing",
        "autogain_smoothing_ms",
        100.0,
        10.0,
        5000.0,
        10.0,
        "ms",
        "Auto Gain",
    )
    .output()
    .doc("Auto gain transition time"),
];

// ============================================================================
// UI Layout
// ============================================================================

/// Crossfeed: idx 0=mode, 1=preset, 2=enabled, 3=mix,
/// 4=bauer_fcut, 5=bauer_feed, 6=meier_level,
/// 7=mb_low_freq, 8=mb_mid_high_freq, 9=mb_low_feed, 10=mb_mid_feed, 11=mb_high_feed,
/// 12=itd_delay_ms,
/// 13=autogain_enabled, 14=target_lufs, 15=max_gain, 16=smoothing
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::selector(1), // crossfeed_preset
        ControlSpec::toggle(2),   // enabled
    ],
    main: &[
        ControlGroup {
            title: "",
            controls: &[
                ControlSpec::button_set(0, MODE_LABELS), // mode
            ],
        },
        ControlGroup {
            title: "BAUER",
            controls: &[
                ControlSpec::knob(4), // bauer_fcut_hz
                ControlSpec::knob(5), // bauer_feed_db
            ],
        },
        ControlGroup {
            title: "MEIER",
            controls: &[ControlSpec::knob(6)], // meier_level
        },
        ControlGroup {
            title: "MULTIBAND",
            controls: &[
                ControlSpec::knob(7),  // mb_low_freq_hz
                ControlSpec::knob(8),  // mb_mid_high_freq_hz
                ControlSpec::knob(9),  // mb_low_feed_db
                ControlSpec::knob(10), // mb_mid_feed_db
                ControlSpec::knob(11), // mb_high_feed_db
            ],
        },
        ControlGroup {
            title: "ITD",
            controls: &[
                ControlSpec::knob(12), // itd_delay_ms
            ],
        },
    ],
    output: &[
        ControlSpec::knob(14),   // target_lufs
        ControlSpec::toggle(13), // autogain_enabled
        ControlSpec::knob(15),   // max_gain
        ControlSpec::knob(3),    // mix
        ControlSpec::knob(16),   // smoothing
    ],
    tabs: &[],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::main(350.0),
        ColumnConstraint::output(120.0, 0.6),
    ],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// Crossfeed plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
///
/// Mode and preset are stored as usize indices into MODE_LABELS / PRESET_LABELS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_crossfeed_mode")]
    pub crossfeed_mode: usize,
    #[serde(default = "d_crossfeed_preset")]
    pub crossfeed_preset: usize,
    #[serde(default = "d_enabled")]
    pub enabled: bool,
    #[serde(default = "d_mix")]
    pub mix: f64,
    #[serde(default = "d_bauer_fcut_hz")]
    pub bauer_fcut_hz: f64,
    #[serde(default = "d_bauer_feed_db")]
    pub bauer_feed_db: f64,
    #[serde(default = "d_meier_level")]
    pub meier_level: f64,
    #[serde(default = "d_mb_low_freq_hz")]
    pub mb_low_freq_hz: f64,
    #[serde(default = "d_mb_mid_high_freq_hz")]
    pub mb_mid_high_freq_hz: f64,
    #[serde(default = "d_mb_low_feed_db")]
    pub mb_low_feed_db: f64,
    #[serde(default = "d_mb_mid_feed_db")]
    pub mb_mid_feed_db: f64,
    #[serde(default = "d_mb_high_feed_db")]
    pub mb_high_feed_db: f64,
    #[serde(default = "d_itd_delay_ms")]
    pub itd_delay_ms: f64,
    #[serde(default = "d_autogain_enabled")]
    pub autogain_enabled: bool,
    #[serde(default = "d_autogain_target_lufs")]
    pub autogain_target_lufs: f64,
    #[serde(default = "d_autogain_max_gain_db")]
    pub autogain_max_gain_db: f64,
    #[serde(default = "d_autogain_smoothing_ms")]
    pub autogain_smoothing_ms: f64,
}

fn d_crossfeed_mode() -> usize {
    pk(PARAMS, "crossfeed_mode").default_usize()
}
fn d_crossfeed_preset() -> usize {
    pk(PARAMS, "crossfeed_preset").default_usize()
}
fn d_enabled() -> bool {
    pk(PARAMS, "enabled").default_bool()
}
fn d_mix() -> f64 {
    pk(PARAMS, "mix").default_f64()
}
fn d_bauer_fcut_hz() -> f64 {
    pk(PARAMS, "bauer_fcut_hz").default_f64()
}
fn d_bauer_feed_db() -> f64 {
    pk(PARAMS, "bauer_feed_db").default_f64()
}
fn d_meier_level() -> f64 {
    pk(PARAMS, "meier_level").default_f64()
}
fn d_mb_low_freq_hz() -> f64 {
    pk(PARAMS, "mb_low_freq_hz").default_f64()
}
fn d_mb_mid_high_freq_hz() -> f64 {
    pk(PARAMS, "mb_mid_high_freq_hz").default_f64()
}
fn d_mb_low_feed_db() -> f64 {
    pk(PARAMS, "mb_low_feed_db").default_f64()
}
fn d_mb_mid_feed_db() -> f64 {
    pk(PARAMS, "mb_mid_feed_db").default_f64()
}
fn d_mb_high_feed_db() -> f64 {
    pk(PARAMS, "mb_high_feed_db").default_f64()
}
fn d_itd_delay_ms() -> f64 {
    pk(PARAMS, "itd_delay_ms").default_f64()
}
fn d_autogain_enabled() -> bool {
    pk(PARAMS, "autogain_enabled").default_bool()
}
fn d_autogain_target_lufs() -> f64 {
    pk(PARAMS, "autogain_target_lufs").default_f64()
}
fn d_autogain_max_gain_db() -> f64 {
    pk(PARAMS, "autogain_max_gain_db").default_f64()
}
fn d_autogain_smoothing_ms() -> f64 {
    pk(PARAMS, "autogain_smoothing_ms").default_f64()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            crossfeed_mode: d_crossfeed_mode(),
            crossfeed_preset: d_crossfeed_preset(),
            enabled: d_enabled(),
            mix: d_mix(),
            bauer_fcut_hz: d_bauer_fcut_hz(),
            bauer_feed_db: d_bauer_feed_db(),
            meier_level: d_meier_level(),
            mb_low_freq_hz: d_mb_low_freq_hz(),
            mb_mid_high_freq_hz: d_mb_mid_high_freq_hz(),
            mb_low_feed_db: d_mb_low_feed_db(),
            mb_mid_feed_db: d_mb_mid_feed_db(),
            mb_high_feed_db: d_mb_high_feed_db(),
            itd_delay_ms: d_itd_delay_ms(),
            autogain_enabled: d_autogain_enabled(),
            autogain_target_lufs: d_autogain_target_lufs(),
            autogain_max_gain_db: d_autogain_max_gain_db(),
            autogain_smoothing_ms: d_autogain_smoothing_ms(),
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
    const PLUGIN_TYPE_KEY: &'static str = "crossfeed";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.crossfeed_mode as f64),
            1 => Some(self.crossfeed_preset as f64),
            2 => Some(if self.enabled { 1.0 } else { 0.0 }),
            3 => Some(self.mix),
            4 => Some(self.bauer_fcut_hz),
            5 => Some(self.bauer_feed_db),
            6 => Some(self.meier_level),
            7 => Some(self.mb_low_freq_hz),
            8 => Some(self.mb_mid_high_freq_hz),
            9 => Some(self.mb_low_feed_db),
            10 => Some(self.mb_mid_feed_db),
            11 => Some(self.mb_high_feed_db),
            12 => Some(self.itd_delay_ms),
            13 => Some(if self.autogain_enabled { 1.0 } else { 0.0 }),
            14 => Some(self.autogain_target_lufs),
            15 => Some(self.autogain_max_gain_db),
            16 => Some(self.autogain_smoothing_ms),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.crossfeed_mode = value as usize,
            1 => self.crossfeed_preset = value as usize,
            2 => self.enabled = value > 0.5,
            3 => self.mix = value,
            4 => self.bauer_fcut_hz = value,
            5 => self.bauer_feed_db = value,
            6 => self.meier_level = value,
            7 => self.mb_low_freq_hz = value,
            8 => self.mb_mid_high_freq_hz = value,
            9 => self.mb_low_feed_db = value,
            10 => self.mb_mid_feed_db = value,
            11 => self.mb_high_feed_db = value,
            12 => self.itd_delay_ms = value,
            13 => self.autogain_enabled = value > 0.5,
            14 => self.autogain_target_lufs = value,
            15 => self.autogain_max_gain_db = value,
            16 => self.autogain_smoothing_ms = value,
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
        assert_eq!(original.crossfeed_mode, restored.crossfeed_mode);
        assert_eq!(original.crossfeed_preset, restored.crossfeed_preset);
        assert_eq!(original.enabled, restored.enabled);
        assert_eq!(original.mix, restored.mix);
        assert_eq!(original.bauer_fcut_hz, restored.bauer_fcut_hz);
        assert_eq!(original.bauer_feed_db, restored.bauer_feed_db);
        assert_eq!(original.meier_level, restored.meier_level);
        assert_eq!(original.mb_low_freq_hz, restored.mb_low_freq_hz);
        assert_eq!(original.mb_mid_high_freq_hz, restored.mb_mid_high_freq_hz);
        assert_eq!(original.mb_low_feed_db, restored.mb_low_feed_db);
        assert_eq!(original.mb_mid_feed_db, restored.mb_mid_feed_db);
        assert_eq!(original.mb_high_feed_db, restored.mb_high_feed_db);
        assert_eq!(original.itd_delay_ms, restored.itd_delay_ms);
        assert_eq!(original.autogain_enabled, restored.autogain_enabled);
        assert_eq!(original.autogain_target_lufs, restored.autogain_target_lufs);
        assert_eq!(original.autogain_max_gain_db, restored.autogain_max_gain_db);
        assert_eq!(
            original.autogain_smoothing_ms,
            restored.autogain_smoothing_ms
        );
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(
            p.crossfeed_mode,
            pk(PARAMS, "crossfeed_mode").default_usize()
        );
        assert_eq!(
            p.crossfeed_preset,
            pk(PARAMS, "crossfeed_preset").default_usize()
        );
        assert_eq!(p.enabled, pk(PARAMS, "enabled").default_bool());
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64());
        assert_eq!(p.bauer_fcut_hz, pk(PARAMS, "bauer_fcut_hz").default_f64());
        assert_eq!(p.bauer_feed_db, pk(PARAMS, "bauer_feed_db").default_f64());
        assert_eq!(p.meier_level, pk(PARAMS, "meier_level").default_f64());
        assert_eq!(p.mb_low_freq_hz, pk(PARAMS, "mb_low_freq_hz").default_f64());
        assert_eq!(
            p.mb_mid_high_freq_hz,
            pk(PARAMS, "mb_mid_high_freq_hz").default_f64()
        );
        assert_eq!(p.mb_low_feed_db, pk(PARAMS, "mb_low_feed_db").default_f64());
        assert_eq!(p.mb_mid_feed_db, pk(PARAMS, "mb_mid_feed_db").default_f64());
        assert_eq!(
            p.mb_high_feed_db,
            pk(PARAMS, "mb_high_feed_db").default_f64()
        );
        assert_eq!(p.itd_delay_ms, pk(PARAMS, "itd_delay_ms").default_f64());
        assert_eq!(
            p.autogain_enabled,
            pk(PARAMS, "autogain_enabled").default_bool()
        );
        assert_eq!(
            p.autogain_target_lufs,
            pk(PARAMS, "autogain_target_lufs").default_f64()
        );
        assert_eq!(
            p.autogain_max_gain_db,
            pk(PARAMS, "autogain_max_gain_db").default_f64()
        );
        assert_eq!(
            p.autogain_smoothing_ms,
            pk(PARAMS, "autogain_smoothing_ms").default_f64()
        );
    }
}
