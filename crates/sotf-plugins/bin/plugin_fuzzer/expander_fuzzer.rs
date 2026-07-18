use super::PluginFuzzer;
use rand::RngExt;
use rand::rngs::StdRng;
use sotf_plugins::{
    BandExpanderParams, MultibandExpanderPlugin, MultibandExpanderPluginParams,
    ParametricInPlacePluginAdapter, Plugin,
};

pub(super) struct ExpanderFuzzer;

impl PluginFuzzer for ExpanderFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let threshold_db = rng.random_range(-80.0..0.0);
        let ratio = rng.random_range(1.0..20.0);
        let attack_ms = rng.random_range(0.1..50.0);
        let release_ms = rng.random_range(10.0..2000.0);
        let range_db = rng.random_range(0.0..80.0);
        let knee_db = rng.random_range(0.0..20.0);
        let hysteresis_db = rng.random_range(0.0..12.0);
        let hold_ms = rng.random_range(0.0..500.0);
        let mix = rng.random_range(0.0..1.0);
        let link_channels = rng.random_bool(0.5);
        let sidechain_hpf_hz = rng.random_range(0.0..500.0);

        let params = MultibandExpanderPluginParams {
            num_bands: 1,
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            range_db,
            knee_db,
            hysteresis_db,
            hold_ms,
            mix,
            link_channels,
            bands: vec![BandExpanderParams {
                auto_makeup: rng.random_bool(0.5),
                measured_auto_makeup: false,
                ..Default::default()
            }],
            ..Default::default()
        };
        let plugin = MultibandExpanderPlugin::from_params(channels, params);

        let desc = format!(
            "threshold={:.1}dB ratio={:.2}:1 attack={:.1}ms release={:.0}ms range={:.1}dB knee={:.1}dB hyst={:.1}dB hold={:.0}ms mix={:.2} link={} sc_hpf={:.0}Hz",
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            range_db,
            knee_db,
            hysteresis_db,
            hold_ms,
            mix,
            link_channels,
            sidechain_hpf_hz
        );

        (Box::new(ParametricInPlacePluginAdapter::new(plugin)), desc)
    }
}
