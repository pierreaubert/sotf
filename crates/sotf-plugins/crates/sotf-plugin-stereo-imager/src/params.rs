//! Stereo Imager plugin parameter definitions -- single source of truth.
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
// Parameter Specifications
// ============================================================================

pub const PARAMS: &[ParamSpec] = &[
    // 0: Global width
    ParamSpec::float("Width", "width", 1.0, 0.0, 2.0, 0.01, "", "Width")
        .scaled(100.0)
        .doc("Global stereo width (0%=mono, 100%=original, 200%=wide)"),
    // 1: Low-mid crossover
    ParamSpec::float(
        "Low-Mid",
        "low_mid_freq",
        250.0,
        80.0,
        1000.0,
        1.0,
        "Hz",
        "Crossover",
    )
    .doc("Low/mid crossover frequency"),
    // 2: Mid-high crossover
    ParamSpec::float(
        "Mid-High",
        "mid_high_freq",
        4000.0,
        1000.0,
        16000.0,
        10.0,
        "Hz",
        "Crossover",
    )
    .doc("Mid/high crossover frequency"),
    // 3: Low band width
    ParamSpec::float(
        "Low Width",
        "low_width",
        1.0,
        0.0,
        2.0,
        0.01,
        "",
        "Band Width",
    )
    .scaled(100.0)
    .doc("Low band stereo width"),
    // 4: Mid band width
    ParamSpec::float(
        "Mid Width",
        "mid_width",
        1.0,
        0.0,
        2.0,
        0.01,
        "",
        "Band Width",
    )
    .scaled(100.0)
    .doc("Mid band stereo width"),
    // 5: High band width
    ParamSpec::float(
        "High Width",
        "high_width",
        1.0,
        0.0,
        2.0,
        0.01,
        "",
        "Band Width",
    )
    .scaled(100.0)
    .doc("High band stereo width"),
    // 6: Mono bass toggle
    ParamSpec::bool_labeled("Mono Bass", "mono_bass", false, "On", "Off", "Options")
        .doc("Collapse stereo below low-mid crossover"),
    // 7: Dry/wet mix
    ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.01, "", "Output")
        .scaled(100.0)
        .output()
        .doc("Dry/wet mix"),
];

// ============================================================================
// UI Layout
// ============================================================================

/// Stereo Imager: idx 0=width, 1=low_mid_freq, 2=mid_high_freq,
/// 3=low_width, 4=mid_width, 5=high_width, 6=mono_bass, 7=mix
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[],
    main: &[
        ControlGroup {
            title: "WIDTH",
            controls: &[ControlSpec::knob_large(0)], // width
        },
        ControlGroup {
            title: "CROSSOVER",
            controls: &[
                ControlSpec::knob(1), // low_mid_freq
                ControlSpec::knob(2), // mid_high_freq
            ],
        },
        ControlGroup {
            title: "BAND WIDTH",
            controls: &[
                ControlSpec::knob(3), // low_width
                ControlSpec::knob(4), // mid_width
                ControlSpec::knob(5), // high_width
            ],
        },
        ControlGroup {
            title: "OPTIONS",
            controls: &[
                ControlSpec::toggle(6), // mono_bass
            ],
        },
    ],
    output: &[
        ControlSpec::knob(7), // mix
    ],
    tabs: &[],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::main(300.0),
        ColumnConstraint::output(120.0, 0.6),
    ],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// Stereo Imager plugin parameters.
///
/// All serde defaults are derived from PARAMS -- adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_width")]
    pub width: f64,
    #[serde(default = "d_low_mid_freq")]
    pub low_mid_freq: f64,
    #[serde(default = "d_mid_high_freq")]
    pub mid_high_freq: f64,
    #[serde(default = "d_low_width")]
    pub low_width: f64,
    #[serde(default = "d_mid_width")]
    pub mid_width: f64,
    #[serde(default = "d_high_width")]
    pub high_width: f64,
    #[serde(default = "d_mono_bass")]
    pub mono_bass: bool,
    #[serde(default = "d_mix")]
    pub mix: f64,
}

fn d_width() -> f64 {
    pk(PARAMS, "width").default_f64()
}
fn d_low_mid_freq() -> f64 {
    pk(PARAMS, "low_mid_freq").default_f64()
}
fn d_mid_high_freq() -> f64 {
    pk(PARAMS, "mid_high_freq").default_f64()
}
fn d_low_width() -> f64 {
    pk(PARAMS, "low_width").default_f64()
}
fn d_mid_width() -> f64 {
    pk(PARAMS, "mid_width").default_f64()
}
fn d_high_width() -> f64 {
    pk(PARAMS, "high_width").default_f64()
}
fn d_mono_bass() -> bool {
    pk(PARAMS, "mono_bass").default_bool()
}
fn d_mix() -> f64 {
    pk(PARAMS, "mix").default_f64()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            width: d_width(),
            low_mid_freq: d_low_mid_freq(),
            mid_high_freq: d_mid_high_freq(),
            low_width: d_low_width(),
            mid_width: d_mid_width(),
            high_width: d_high_width(),
            mono_bass: d_mono_bass(),
            mix: d_mix(),
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
    const PLUGIN_TYPE_KEY: &'static str = "stereo_imager";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.width),
            1 => Some(self.low_mid_freq),
            2 => Some(self.mid_high_freq),
            3 => Some(self.low_width),
            4 => Some(self.mid_width),
            5 => Some(self.high_width),
            6 => Some(if self.mono_bass { 1.0 } else { 0.0 }),
            7 => Some(self.mix),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.width = value,
            1 => self.low_mid_freq = value,
            2 => self.mid_high_freq = value,
            3 => self.low_width = value,
            4 => self.mid_width = value,
            5 => self.high_width = value,
            6 => self.mono_bass = value > 0.5,
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
        assert_eq!(original.width, restored.width);
        assert_eq!(original.low_mid_freq, restored.low_mid_freq);
        assert_eq!(original.mid_high_freq, restored.mid_high_freq);
        assert_eq!(original.low_width, restored.low_width);
        assert_eq!(original.mid_width, restored.mid_width);
        assert_eq!(original.high_width, restored.high_width);
        assert_eq!(original.mono_bass, restored.mono_bass);
        assert_eq!(original.mix, restored.mix);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.width, pk(PARAMS, "width").default_f64());
        assert_eq!(p.low_mid_freq, pk(PARAMS, "low_mid_freq").default_f64());
        assert_eq!(p.mid_high_freq, pk(PARAMS, "mid_high_freq").default_f64());
        assert_eq!(p.low_width, pk(PARAMS, "low_width").default_f64());
        assert_eq!(p.mid_width, pk(PARAMS, "mid_width").default_f64());
        assert_eq!(p.high_width, pk(PARAMS, "high_width").default_f64());
        assert_eq!(p.mono_bass, pk(PARAMS, "mono_bass").default_bool());
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64());
    }
}
