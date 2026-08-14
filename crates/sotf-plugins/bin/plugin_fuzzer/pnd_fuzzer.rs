use super::PluginFuzzer;
use rand::RngExt;
use rand::rngs::StdRng;
use sotf_plugins::{Plugin, PndPlugin, PndPluginParams};

pub(super) struct PndFuzzer;

impl PluginFuzzer for PndFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let params = PndPluginParams {
            correction_strength: rng.random_range(0.0..1.0),
            analysis_window_ms: rng.random_range(20.0..100.0),
            drift_smoothing: rng.random_range(0.001..0.1),
            multi_channel_analysis: true,
            confidence_threshold: 0.5,
            reference_frequency_hz: 0.0,
            phase_vocoder: false,
        };

        let plugin = PndPlugin::from_params(channels, params.clone())
            .expect("fuzzer generated valid PND parameters");

        let desc = format!(
            "strength={:.2} window={:.1}ms",
            params.correction_strength, params.analysis_window_ms
        );

        (Box::new(plugin), desc)
    }
}
