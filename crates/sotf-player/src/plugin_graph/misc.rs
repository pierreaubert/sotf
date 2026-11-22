/// Adapt a per-channel crossover to the graph's actual input width.
///
/// Retain existing channel values and leave dormant Bands configurations alone.
pub(super) fn adapt_crossover_channels(
    settings: &mut sotf_audio::plugins::PluginSettings,
    channels: usize,
) {
    use sotf_plugins::plugin_crossover::CrossoverTopology;

    let sotf_audio::plugins::PluginSettings::Crossover {
        topology,
        frequency,
        output,
        extra_frequencies,
        channel_frequencies_hz,
        channel_modes,
        ..
    } = settings
    else {
        return;
    };
    let per_channel = match topology {
        Some(CrossoverTopology::PerChannel) => true,
        Some(CrossoverTopology::Bands) => false,
        None => {
            channel_frequencies_hz
                .as_ref()
                .is_some_and(|f| !f.is_empty())
                && extra_frequencies.is_empty()
        }
    };
    if !per_channel || channels == 0 {
        return;
    }

    let previous_width = channel_frequencies_hz.as_ref().map_or(0, Vec::len);
    let fresh = previous_width == 0;
    if fresh && channel_modes.as_ref().is_some_and(|m| !m.is_empty()) {
        return;
    }
    let fallback = match output.to_ascii_lowercase().as_str() {
        "lowpass" => "lowpass",
        "highpass" => "highpass",
        // Both has no per-channel equivalent. A newly selected topology
        // starts transparently until the user chooses each channel's mode.
        "both" if fresh => "passthrough",
        _ => {
            // Preserve invalid or incomplete presets for factory validation.
            // A complete mode list makes the scalar output dormant.
            if channel_modes
                .as_ref()
                .is_none_or(|m| m.len() != previous_width)
            {
                return;
            }
            "passthrough"
        }
    };
    if !fresh
        && *topology == Some(CrossoverTopology::PerChannel)
        && channel_modes
            .as_ref()
            .is_some_and(|m| m.len() != previous_width)
    {
        return;
    }
    channel_frequencies_hz
        .get_or_insert_with(Vec::new)
        .resize(channels, *frequency);
    channel_modes
        .get_or_insert_with(Vec::new)
        .resize(channels, fallback.to_owned());
}

/// Whether this dynamics processor appends an external key bus after program inputs.
pub(super) fn uses_external_key(settings: &sotf_audio::plugins::PluginSettings) -> bool {
    matches!(
        settings,
        sotf_audio::plugins::PluginSettings::Gate {
            sidechain_external: true,
            ..
        } | sotf_audio::plugins::PluginSettings::DeEsser {
            sidechain_external: true,
            ..
        }
    )
}
