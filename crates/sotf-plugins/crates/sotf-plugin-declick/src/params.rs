use serde::{Deserialize, Serialize};
use sotf_host::param_specs::{ParamSpec, find_by_key as pk};
use sotf_host::plugin_layout::*;
use sotf_host::plugin_params::PluginParamDef;

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::bool_param("Enabled", "enabled", true, "General")
        .doc("Enable time-domain click repair"),
    ParamSpec::float(
        "Sensitivity",
        "sensitivity",
        10.0,
        1.0,
        100.0,
        1.0,
        "",
        "General",
    )
    .doc("Click detection sensitivity; lower values detect more clicks"),
    ParamSpec::bool_param("Link Channels", "link_channels", true, "General")
        .doc("Link click decisions in adjacent channel pairs to preserve spatial coherence"),
];

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[],
    main: &[ControlGroup::new(
        "REPAIR",
        "REPAIR",
        &[
            ControlSpec::toggle(0),
            ControlSpec::knob_large(1),
            ControlSpec::toggle(2),
        ],
    )],
    output: &[],
    tabs: &[],
    visualizations: &[],
    column_constraints: &[ColumnConstraint::main(220.0)],
    dynamic_sections: &[],
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_enabled")]
    pub enabled: bool,
    #[serde(default = "d_sensitivity")]
    pub sensitivity: f64,
    #[serde(default = "d_link_channels")]
    pub link_channels: bool,
}

fn d_enabled() -> bool {
    pk(PARAMS, "enabled").default_bool()
}
fn d_sensitivity() -> f64 {
    pk(PARAMS, "sensitivity").default_f64()
}
fn d_link_channels() -> bool {
    pk(PARAMS, "link_channels").default_bool()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            enabled: d_enabled(),
            sensitivity: d_sensitivity(),
            link_channels: d_link_channels(),
        }
    }
}

impl PluginParamDef for Params {
    const PARAMS: &'static [ParamSpec] = PARAMS;
    const LAYOUT: Option<&'static PluginLayout> = Some(&LAYOUT);
    const VERSION: u32 = 2;
    const PLUGIN_TYPE_KEY: &'static str = "declick";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(if self.enabled { 1.0 } else { 0.0 }),
            1 => Some(self.sensitivity),
            2 => Some(if self.link_channels { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.enabled = value > 0.5,
            1 => self.sensitivity = value,
            2 => self.link_channels = value > 0.5,
            _ => {}
        }
    }
}
