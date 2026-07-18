use super::PluginFuzzer;
use rand::RngExt;
use rand::rngs::StdRng;
use sotf_plugins::{LimiterPlugin, LimiterPluginParams, ParametricInPlacePluginAdapter, Plugin};

pub(super) struct LimiterFuzzer;

impl PluginFuzzer for LimiterFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let threshold_db = rng.random_range(-20.0..0.0);
        let release_ms = rng.random_range(10.0..1000.0);
        let lookahead_ms = rng.random_range(0.0..20.0);
        let soft = rng.random_bool(0.5);
        let mix = rng.random_range(0.0..1.0);

        let params = LimiterPluginParams {
            threshold_db,
            release_ms,
            lookahead_ms,
            soft,
            mix,
            true_peak: false,
            isp_mode: false,
            dual_release: false,
            feed_forward: false,
            link_amount: 1.0,
        };
        let plugin = LimiterPlugin::from_params(channels, params);

        let desc = format!(
            "threshold={:.1}dB release={:.0}ms lookahead={:.1}ms soft={} mix={:.2}",
            threshold_db, release_ms, lookahead_ms, soft, mix
        );

        (Box::new(ParametricInPlacePluginAdapter::new(plugin)), desc)
    }
}
