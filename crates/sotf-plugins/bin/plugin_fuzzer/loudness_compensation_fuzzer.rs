use super::PluginFuzzer;
use rand::Rng;
use rand::rngs::StdRng;
use sotf_plugins::{ParametricInPlacePluginAdapter, 
    LoudnessCompensationPlugin, LoudnessCompensationPluginParams, Plugin};

pub(super) struct LoudnessCompensationFuzzer;

impl PluginFuzzer for LoudnessCompensationFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let low_freq = rng.random_range(20.0..500.0);
        let low_gain = rng.random_range(0.0..20.0);
        let high_freq = rng.random_range(5000.0..20000.0);
        let high_gain = rng.random_range(0.0..20.0);

        let params = LoudnessCompensationPluginParams {
            low_freq,
            low_gain,
            high_freq,
            high_gain,
            mid_enabled: true,
            mid_freq: 3000.0,
            mid_gain: 3.0,
            mid_q: 0.707,
            channel_params: vec![],
            auto_gain_enabled: false,
            auto_gain_max_db: 12.0,
            auto_gain_smoothing_ms: 100.0,
            auto_gain_position: "post".to_string(),
            mode: 0,
            playback_level_db: 70.0,
            reference_level_db: 83.0,
            playback_volume_db: 0.0,
        };
        let plugin = LoudnessCompensationPlugin::from_params(channels, params)
            .expect("Failed to create LoudnessCompensationPlugin");

        let desc = format!(
            "low_freq={:.0}Hz low_gain={:.1}dB high_freq={:.0}Hz high_gain={:.1}dB",
            low_freq, low_gain, high_freq, high_gain
        );

        (Box::new(ParametricInPlacePluginAdapter::new(plugin)), desc)
    }
}
