use super::PluginFuzzer;
use rand::RngExt;
use rand::rngs::StdRng;
use sotf_plugins::{
    LoudnessCompensationPlugin, LoudnessCompensationPluginParams, ParametricInPlacePluginAdapter,
    Plugin,
};

pub(super) struct FletcherMunsonFuzzer;

impl PluginFuzzer for FletcherMunsonFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let playback_volume_db = rng.random_range(-60.0..0.0);
        let reference_level_db = rng.random_range(60.0..100.0);

        let params = LoudnessCompensationPluginParams {
            mode: 2, // Auto
            playback_volume_db,
            reference_level_db,
            ..Default::default()
        };
        let plugin = LoudnessCompensationPlugin::from_params(channels, params)
            .expect("Failed to create FletcherMunsonPlugin (LoudnessCompensation Auto)");

        let desc = format!(
            "playback_vol={:.1}dB ref_level={:.1}dB",
            playback_volume_db, reference_level_db
        );

        (Box::new(ParametricInPlacePluginAdapter::new(plugin)), desc)
    }
}
