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
];

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[],
    main: &[ControlGroup::new(
        "REPAIR",
        "REPAIR",
        &[ControlSpec::toggle(0), ControlSpec::knob_large(1)],
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
}

fn d_enabled() -> bool {
    pk(PARAMS, "enabled").default_bool()
}
fn d_sensitivity() -> f64 {
    pk(PARAMS, "sensitivity").default_f64()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            enabled: d_enabled(),
            sensitivity: d_sensitivity(),
        }
    }
}

impl PluginParamDef for Params {
    const PARAMS: &'static [ParamSpec] = PARAMS;
    const LAYOUT: Option<&'static PluginLayout> = Some(&LAYOUT);
    const VERSION: u32 = 1;
    const PLUGIN_TYPE_KEY: &'static str = "declick";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(if self.enabled { 1.0 } else { 0.0 }),
            1 => Some(self.sensitivity),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.enabled = value > 0.5,
            1 => self.sensitivity = value,
            _ => {}
        }
    }
}
