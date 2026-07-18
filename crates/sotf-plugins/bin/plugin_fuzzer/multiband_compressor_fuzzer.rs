use super::PluginFuzzer;
use rand::RngExt;
use rand::rngs::StdRng;
use sotf_plugins::{
    MultibandCompressorPlugin, MultibandCompressorPluginParams, ParametricInPlacePluginAdapter,
    Plugin,
};

pub(super) struct MultibandCompressorFuzzer;

impl PluginFuzzer for MultibandCompressorFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let num_bands = rng.random_range(2..=5);
        let crossover_preset = rng.random_range(0..=3);
        let threshold_db = rng.random_range(-60.0..0.0);
        let ratio = rng.random_range(1.0..20.0);
        let attack_ms = rng.random_range(0.1..100.0);
        let release_ms = rng.random_range(10.0..1000.0);
        let knee_db = rng.random_range(0.0..20.0);
        let mix = rng.random_range(0.0..1.0);
        let link_channels = rng.random_bool(0.5);

        // Generate random crossover frequencies (sorted ascending)
        let mut freqs = vec![
            rng.random_range(20.0..500.0),
            rng.random_range(500.0..5000.0),
            rng.random_range(5000.0..15000.0),
            rng.random_range(10000.0..18000.0),
        ];
        freqs.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let params = MultibandCompressorPluginParams {
            num_bands,
            crossover_preset,
            crossover_frequencies: freqs.clone(),
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            link_channels,
            mix,
            per_band_lookahead_ms: 0.0,
            ms_mode: false,
            bands: vec![], // Use defaults for per-band params
            sidechain_tilt_db: 0.0,
            link_amount: 1.0,
            ..Default::default()
        };
        let plugin = MultibandCompressorPlugin::from_params(channels, params);

        let desc = format!(
            "bands={} preset={} threshold={:.1}dB ratio={:.2}:1 attack={:.1}ms release={:.0}ms knee={:.1}dB mix={:.2} link={}",
            num_bands,
            crossover_preset,
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            mix,
            link_channels
        );

        (Box::new(ParametricInPlacePluginAdapter::new(plugin)), desc)
    }
}
