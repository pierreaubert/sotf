//! MonoToStereo plugin parameter definitions — single source of truth.
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
    ParamSpec::float("Width", "stereo_width", 0.5, 0.0, 1.0, 0.05, "", "General")
        .doc("Stereo spread amount"),
    ParamSpec::float(
        "Haas Delay",
        "haas_delay_ms",
        1.5,
        0.0,
        5.0,
        0.1,
        "ms",
        "General",
    )
    .doc("Inter-channel delay for Haas effect"),
    ParamSpec::bool_param("Comp EQ", "enable_comp_eq", true, "EQ")
        .setup()
        .doc("Compensate comb filter coloration"),
    ParamSpec::float(
        "Comp EQ Depth",
        "comp_eq_depth_db",
        1.0,
        0.0,
        3.0,
        0.1,
        "dB",
        "EQ",
    )
    .doc("Compensation EQ strength"),
    ParamSpec::float(
        "Decor Low",
        "decor_low_hz",
        300.0,
        100.0,
        500.0,
        10.0,
        "Hz",
        "General",
    )
    .doc("Decorrelation low crossover"),
    ParamSpec::float(
        "Decor High",
        "decor_high_hz",
        2000.0,
        1000.0,
        5000.0,
        10.0,
        "Hz",
        "General",
    )
    .doc("Decorrelation high crossover"),
    ParamSpec::bool_param("Freq Dependent", "freq_dependent", true, "General")
        .doc("Vary width by frequency band"),
];

// ============================================================================
// UI Layout
// ============================================================================

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::toggle(2), // enable_comp_eq
        ControlSpec::knob(3),   // comp_eq_depth_db
    ],
    main: &[ControlGroup {
        title: "",
        controls: &[ControlSpec::slider(0)], // stereo_width
    }],
    output: &[],
    tabs: &[TabSpec {
        name: "Advanced",
        controls: &[
            ControlSpec::knob(1),   // haas_delay_ms
            ControlSpec::knob(4),   // decor_low_hz
            ControlSpec::knob(5),   // decor_high_hz
            ControlSpec::toggle(6), // freq_dependent
        ],
    }],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::config(100.0, 0.5),
        ColumnConstraint::main(200.0),
    ],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// MonoToStereo plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_stereo_width")]
    pub stereo_width: f64,
    #[serde(default = "d_haas_delay_ms")]
    pub haas_delay_ms: f64,
    #[serde(default = "d_enable_comp_eq")]
    pub enable_comp_eq: bool,
    #[serde(default = "d_comp_eq_depth_db")]
    pub comp_eq_depth_db: f64,
    #[serde(default = "d_decor_low_hz")]
    pub decor_low_hz: f64,
    #[serde(default = "d_decor_high_hz")]
    pub decor_high_hz: f64,
    #[serde(default = "d_freq_dependent")]
    pub freq_dependent: bool,
}

fn d_stereo_width() -> f64 {
    pk(PARAMS, "stereo_width").default_f64()
}
fn d_haas_delay_ms() -> f64 {
    pk(PARAMS, "haas_delay_ms").default_f64()
}
fn d_enable_comp_eq() -> bool {
    pk(PARAMS, "enable_comp_eq").default_bool()
}
fn d_comp_eq_depth_db() -> f64 {
    pk(PARAMS, "comp_eq_depth_db").default_f64()
}
fn d_decor_low_hz() -> f64 {
    pk(PARAMS, "decor_low_hz").default_f64()
}
fn d_decor_high_hz() -> f64 {
    pk(PARAMS, "decor_high_hz").default_f64()
}
fn d_freq_dependent() -> bool {
    pk(PARAMS, "freq_dependent").default_bool()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            stereo_width: d_stereo_width(),
            haas_delay_ms: d_haas_delay_ms(),
            enable_comp_eq: d_enable_comp_eq(),
            comp_eq_depth_db: d_comp_eq_depth_db(),
            decor_low_hz: d_decor_low_hz(),
            decor_high_hz: d_decor_high_hz(),
            freq_dependent: d_freq_dependent(),
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
    const PLUGIN_TYPE_KEY: &'static str = "mono_to_stereo";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.stereo_width),
            1 => Some(self.haas_delay_ms),
            2 => Some(if self.enable_comp_eq { 1.0 } else { 0.0 }),
            3 => Some(self.comp_eq_depth_db),
            4 => Some(self.decor_low_hz),
            5 => Some(self.decor_high_hz),
            6 => Some(if self.freq_dependent { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.stereo_width = value,
            1 => self.haas_delay_ms = value,
            2 => self.enable_comp_eq = value > 0.5,
            3 => self.comp_eq_depth_db = value,
            4 => self.decor_low_hz = value,
            5 => self.decor_high_hz = value,
            6 => self.freq_dependent = value > 0.5,
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
        assert_eq!(original.stereo_width, restored.stereo_width);
        assert_eq!(original.haas_delay_ms, restored.haas_delay_ms);
        assert_eq!(original.enable_comp_eq, restored.enable_comp_eq);
        assert_eq!(original.comp_eq_depth_db, restored.comp_eq_depth_db);
        assert_eq!(original.decor_low_hz, restored.decor_low_hz);
        assert_eq!(original.decor_high_hz, restored.decor_high_hz);
        assert_eq!(original.freq_dependent, restored.freq_dependent);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.stereo_width, pk(PARAMS, "stereo_width").default_f64());
        assert_eq!(p.haas_delay_ms, pk(PARAMS, "haas_delay_ms").default_f64());
        assert_eq!(
            p.enable_comp_eq,
            pk(PARAMS, "enable_comp_eq").default_bool()
        );
        assert_eq!(
            p.comp_eq_depth_db,
            pk(PARAMS, "comp_eq_depth_db").default_f64()
        );
        assert_eq!(p.decor_low_hz, pk(PARAMS, "decor_low_hz").default_f64());
        assert_eq!(p.decor_high_hz, pk(PARAMS, "decor_high_hz").default_f64());
        assert_eq!(
            p.freq_dependent,
            pk(PARAMS, "freq_dependent").default_bool()
        );
    }
}
