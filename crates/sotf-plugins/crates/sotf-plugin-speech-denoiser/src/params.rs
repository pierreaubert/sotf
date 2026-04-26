use serde::{Deserialize, Serialize};
use sotf_host::param_specs::{find_by_key as pk, ParamSpec};
use sotf_host::plugin_layout::*;
use sotf_host::plugin_params::PluginParamDef;

pub const PARAMS: &[ParamSpec] = &[ParamSpec::bool_param("Enabled", "enabled", true, "General")
    .doc("Enable RNNoise speech denoising")];

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[],
    main: &[ControlGroup {
        title: "SPEECH",
        controls: &[ControlSpec::toggle(0)],
    }],
    output: &[],
    tabs: &[],
    visualizations: &[],
    column_constraints: &[ColumnConstraint::main(180.0)],
    dynamic_sections: &[],
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_enabled")]
    pub enabled: bool,
}

fn d_enabled() -> bool {
    pk(PARAMS, "enabled").default_bool()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            enabled: d_enabled(),
        }
    }
}

impl PluginParamDef for Params {
    const PARAMS: &'static [ParamSpec] = PARAMS;
    const LAYOUT: Option<&'static PluginLayout> = Some(&LAYOUT);
    const VERSION: u32 = 1;
    const PLUGIN_TYPE_KEY: &'static str = "speech_denoiser";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(if self.enabled { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        if index == 0 {
            self.enabled = value > 0.5;
        }
    }
}
