use super::PluginFuzzer;
use rand::Rng;
use rand::rngs::StdRng;
use sotf_plugins::{GatePlugin, GatePluginParams, InPlacePluginAdapter, Plugin};

pub(super) struct GateFuzzer;

impl PluginFuzzer for GateFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let threshold_db = rng.random_range(-80.0..0.0);
        let ratio = rng.random_range(1.0..100.0);
        let attack_ms = rng.random_range(0.1..50.0);
        let hold_ms = rng.random_range(0.0..1000.0);
        let release_ms = rng.random_range(10.0..2000.0);
        let mix = rng.random_range(0.0..1.0);
        let link_channels = rng.random_bool(0.5);
        let sidechain_hpf_hz = rng.random_range(0.0..200.0);

        let params = GatePluginParams {
            threshold_db,
            ratio,
            attack_ms,
            hold_ms,
            release_ms,
            mix,
            link_channels,
            sidechain_hpf_hz,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "Peak".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        };
        let plugin = GatePlugin::from_params(channels, params);

        let desc = format!(
            "threshold={:.1}dB ratio={:.1}:1 attack={:.1}ms hold={:.0}ms release={:.0}ms mix={:.2} link={} sc_hpf={:.0}Hz",
            threshold_db,
            ratio,
            attack_ms,
            hold_ms,
            release_ms,
            mix,
            link_channels,
            sidechain_hpf_hz
        );

        (Box::new(InPlacePluginAdapter::new(plugin)), desc)
    }
}
