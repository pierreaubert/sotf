use super::PluginFuzzer;
use rand::Rng;
use rand::rngs::StdRng;
use sotf_plugins::{
    ChannelMuteSoloParams, ChannelMuteSoloPlugin, ChannelState, InPlacePluginAdapter, Plugin,
};

pub(super) struct ChannelMuteSoloFuzzer;

impl PluginFuzzer for ChannelMuteSoloFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let enabled = rng.random_bool(0.8); // 80% enabled

        let mut channel_states = Vec::with_capacity(channels);
        let mut desc_parts = Vec::new();

        for ch in 0..channels {
            let muted = rng.random_bool(0.2);
            let soloed = rng.random_bool(0.1);
            let dimmed = rng.random_bool(0.1);

            channel_states.push(ChannelState {
                muted,
                soloed,
                dimmed,
            });

            if muted || soloed || dimmed {
                desc_parts.push(format!(
                    "ch{}:{}{}{}",
                    ch,
                    if muted { "M" } else { "" },
                    if soloed { "S" } else { "" },
                    if dimmed { "D" } else { "" }
                ));
            }
        }

        let params = ChannelMuteSoloParams {
            enabled,
            channel_states,
            dim_gain_db: -20.0,
            fade_ms: 5.0,
        };
        let plugin = ChannelMuteSoloPlugin::from_params(channels, params);

        let desc = format!(
            "enabled={} {}",
            enabled,
            if desc_parts.is_empty() {
                "no_changes".to_string()
            } else {
                desc_parts.join(" ")
            }
        );

        (Box::new(InPlacePluginAdapter::new(plugin)), desc)
    }
}
