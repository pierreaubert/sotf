use super::PluginFuzzer;
use rand::RngExt;
use rand::rngs::StdRng;
use sotf_plugins::{Plugin, SpectrumAnalyzerPlugin, SpectrumConfig};

pub(super) struct SpectrumAnalyzerFuzzer;

impl PluginFuzzer for SpectrumAnalyzerFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let num_bins = rng.random_range(10..100);
        let min_freq = rng.random_range(10.0..100.0);
        let max_freq = rng.random_range(10000.0..22000.0);
        let smoothing = rng.random_range(0.0..1.0);

        let config = SpectrumConfig {
            num_bins,
            min_freq,
            max_freq,
            smoothing,
        };

        let plugin = SpectrumAnalyzerPlugin::with_config(channels, config)
            .expect("Failed to create SpectrumAnalyzerPlugin");

        let desc = format!(
            "bins={} min_freq={:.0}Hz max_freq={:.0}Hz smoothing={:.2}",
            num_bins, min_freq, max_freq, smoothing
        );

        (Box::new(plugin), desc)
    }
}
