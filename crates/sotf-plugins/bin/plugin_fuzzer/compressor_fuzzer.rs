use super::PluginFuzzer;
use rand::Rng;
use rand::rngs::StdRng;
use sotf_plugins::{ParametricInPlacePluginAdapter, 
    BandCompressorParams, MultibandCompressorPlugin,
    MultibandCompressorPluginParams, Plugin};

pub(super) struct CompressorFuzzer;

impl PluginFuzzer for CompressorFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let threshold_db = rng.random_range(-60.0..0.0);
        let ratio = rng.random_range(1.0..20.0);
        let attack_ms = rng.random_range(0.1..100.0);
        let release_ms = rng.random_range(10.0..1000.0);
        let knee_db = rng.random_range(0.0..20.0);
        let makeup_gain_db = rng.random_range(-24.0..24.0);
        let mix = rng.random_range(0.0..1.0);
        let auto_makeup = rng.random_bool(0.5);
        let link_channels = rng.random_bool(0.5);
        let sidechain_hpf_hz = rng.random_range(0.0..200.0);

        let params = MultibandCompressorPluginParams {
            num_bands: 1,
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            mix,
            link_channels,
            bands: vec![BandCompressorParams {
                makeup_gain_db,
                auto_makeup,
                measured_auto_makeup: false,
                ..Default::default()
            }],
            ..Default::default()
        };
        let plugin = MultibandCompressorPlugin::from_params(channels, params);

        let desc = format!(
            "threshold={:.1}dB ratio={:.2}:1 attack={:.1}ms release={:.0}ms knee={:.1}dB makeup={:.1}dB mix={:.2} auto_makeup={} link={} sc_hpf={:.0}Hz",
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            makeup_gain_db,
            mix,
            auto_makeup,
            link_channels,
            sidechain_hpf_hz
        );

        (Box::new(ParametricInPlacePluginAdapter::new(plugin)), desc)
    }
}
