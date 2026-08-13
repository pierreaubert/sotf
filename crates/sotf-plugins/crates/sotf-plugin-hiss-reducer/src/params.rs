use serde::{Deserialize, Serialize};
use sotf_host::param_specs::{ParamSpec, find_by_key as pk};
use sotf_host::plugin_layout::*;
use sotf_host::plugin_params::PluginParamDef;

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::bool_param("Enabled", "enabled", true, "General")
        .doc("Enable high-frequency hiss reduction"),
    ParamSpec::float(
        "Threshold",
        "threshold_db",
        -30.0,
        -60.0,
        -10.0,
        0.5,
        "dB",
        "General",
    )
    .doc("Absolute dBFS threshold for persistent low-level high-band reduction"),
    ParamSpec::float(
        "Frequency",
        "frequency_hz",
        4000.0,
        1000.0,
        16000.0,
        100.0,
        "Hz",
        "General",
    )
    .doc("One-pole high-band edge (limited to 0.45 x sample rate)"),
    ParamSpec::float("Strength", "strength", 0.5, 0.0, 1.0, 0.01, "", "General")
        .scaled(100.0)
        .doc("Hiss attenuation strength"),
];

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[],
    main: &[ControlGroup::new(
        "HISS",
        "HISS",
        &[
            ControlSpec::toggle(0),
            ControlSpec::knob(1),
            ControlSpec::knob(2),
            ControlSpec::slider(3),
        ],
    )],
    output: &[],
    tabs: &[],
    visualizations: &[],
    column_constraints: &[ColumnConstraint::main(320.0)],
    dynamic_sections: &[],
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    #[serde(default = "d_enabled")]
    pub enabled: bool,
    #[serde(default = "d_threshold_db")]
    pub threshold_db: f64,
    #[serde(default = "d_frequency_hz")]
    pub frequency_hz: f64,
    #[serde(default = "d_strength")]
    pub strength: f64,
}

fn d_enabled() -> bool {
    pk(PARAMS, "enabled").default_bool()
}
fn d_threshold_db() -> f64 {
    pk(PARAMS, "threshold_db").default_f64()
}
fn d_frequency_hz() -> f64 {
    pk(PARAMS, "frequency_hz").default_f64()
}
fn d_strength() -> f64 {
    pk(PARAMS, "strength").default_f64()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            enabled: d_enabled(),
            threshold_db: d_threshold_db(),
            frequency_hz: d_frequency_hz(),
            strength: d_strength(),
        }
    }
}

impl PluginParamDef for Params {
    const PARAMS: &'static [ParamSpec] = PARAMS;
    const LAYOUT: Option<&'static PluginLayout> = Some(&LAYOUT);
    const VERSION: u32 = 1;
    const PLUGIN_TYPE_KEY: &'static str = "hiss_reducer";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(if self.enabled { 1.0 } else { 0.0 }),
            1 => Some(self.threshold_db),
            2 => Some(self.frequency_hz),
            3 => Some(self.strength),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.enabled = value > 0.5,
            1 => self.threshold_db = value,
            2 => self.frequency_hz = value,
            3 => self.strength = value,
            _ => {}
        }
    }
}
