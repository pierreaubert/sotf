use super::PluginFuzzer;
use rand::Rng;
use rand::rngs::StdRng;
use sotf_plugins::param_specs::gain::default_smoothing_ms;
use sotf_plugins::{GainPlugin, GainPluginParams, ParametricPluginAdapter, Plugin};

pub(super) struct GainFuzzer;

impl PluginFuzzer for GainFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let gain_db = rng.random_range(-60.0..0.0);
        let params = GainPluginParams {
            gain_db,
            smoothing_ms: default_smoothing_ms(),
            channel_gains: vec![],
        };
        let plugin = Box::new(ParametricPluginAdapter::new(
            GainPlugin::from_params(channels, params).expect("Failed to create GainPlugin"),
        ));
        let desc = format!("gain_db={:.2}", gain_db);
        (plugin, desc)
    }
}
