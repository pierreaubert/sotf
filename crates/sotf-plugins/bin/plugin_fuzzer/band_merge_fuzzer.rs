use super::PluginFuzzer;
use rand::RngExt;
use rand::rngs::StdRng;
use sotf_plugins::{BandMergePlugin, BandMergePluginParams, Plugin};

pub(super) struct BandMergeFuzzer;

impl PluginFuzzer for BandMergeFuzzer {
    fn create_plugin(&self, _channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let bands = rng.random_range(2..=4);
        let output_channels = 2; // Fixed stereo output for fuzzer

        let params = BandMergePluginParams {
            bands,
            band_gains_db: Vec::new(),
            band_mutes: Vec::new(),
        };

        let plugin = BandMergePlugin::from_params(output_channels, &params)
            .expect("Failed to create BandMergePlugin");

        let desc = format!("bands={} out_ch={}", bands, output_channels);

        (Box::new(plugin), desc)
    }
}
