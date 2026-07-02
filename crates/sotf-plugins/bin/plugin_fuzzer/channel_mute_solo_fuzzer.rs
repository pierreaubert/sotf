use super::PluginFuzzer;
use rand::Rng;
use rand::rngs::StdRng;
use sotf_plugins::{
    ChannelMuteSoloParams, ChannelMuteSoloPlugin, ChannelState, ParametricInPlacePluginAdapter,
    Plugin,
};

pub(super) struct ChannelMuteSoloFuzzer;

fn avoid_intentionally_silent_config(
    channels: usize,
    enabled: bool,
    channel_states: &mut [ChannelState],
    rng: &mut StdRng,
) {
    let channel_count = channels.min(channel_states.len());
    if !enabled || channel_count == 0 {
        return;
    }

    let states = &mut channel_states[..channel_count];
    let has_soloed_channel = states.iter().any(|state| state.soloed);
    let all_channels_muted = states.iter().all(|state| state.muted);
    if !has_soloed_channel && all_channels_muted {
        let audible_channel = rng.random_range(0..channel_count);
        states[audible_channel].muted = false;
    }
}

impl PluginFuzzer for ChannelMuteSoloFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let enabled = rng.random_bool(0.8); // 80% enabled

        let mut channel_states = Vec::with_capacity(channels);

        for _ in 0..channels {
            let muted = rng.random_bool(0.2);
            let soloed = rng.random_bool(0.1);
            let dimmed = rng.random_bool(0.1);

            channel_states.push(ChannelState {
                muted,
                soloed,
                dimmed,
            });
        }

        avoid_intentionally_silent_config(channels, enabled, &mut channel_states, rng);

        let mut desc_parts = Vec::new();
        for (ch, state) in channel_states.iter().enumerate() {
            if state.muted || state.soloed || state.dimmed {
                desc_parts.push(format!(
                    "ch{}:{}{}{}",
                    ch,
                    if state.muted { "M" } else { "" },
                    if state.soloed { "S" } else { "" },
                    if state.dimmed { "D" } else { "" }
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

        (Box::new(ParametricInPlacePluginAdapter::new(plugin)), desc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn mono_enabled_mute_is_not_generated_as_intentional_silence() {
        let mut rng = StdRng::seed_from_u64(0);
        let mut states = vec![ChannelState {
            muted: true,
            soloed: false,
            dimmed: false,
        }];

        avoid_intentionally_silent_config(1, true, &mut states, &mut rng);

        assert!(!states[0].muted);
    }

    #[test]
    fn solo_channel_may_remain_muted_because_solo_has_priority() {
        let mut rng = StdRng::seed_from_u64(0);
        let mut states = vec![ChannelState {
            muted: true,
            soloed: true,
            dimmed: false,
        }];

        avoid_intentionally_silent_config(1, true, &mut states, &mut rng);

        assert!(states[0].muted);
        assert!(states[0].soloed);
    }

    #[test]
    fn all_muted_multichannel_config_keeps_one_channel_audible() {
        let mut rng = StdRng::seed_from_u64(0);
        let mut states = vec![
            ChannelState {
                muted: true,
                soloed: false,
                dimmed: false,
            },
            ChannelState {
                muted: true,
                soloed: false,
                dimmed: false,
            },
        ];

        avoid_intentionally_silent_config(2, true, &mut states, &mut rng);

        assert!(states.iter().any(|state| !state.muted));
    }
}
