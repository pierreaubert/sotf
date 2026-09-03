use super::misc::fallback_output_format;
use cpal::traits::DeviceTrait;
use cpal::{Device, SampleFormat, StreamConfig};

/// Choose the best output sample format and channel count supported by the device.
/// Returns (format, hw_channels) where hw_channels may differ from config.channels
/// if the device doesn't support the requested channel count.
/// Prefers F32 > I32 > I16 (highest fidelity first).
pub(super) fn choose_output_format(device: &Device, config: &StreamConfig) -> (SampleFormat, u16) {
    let supported: Vec<_> = match device.supported_output_configs() {
        Ok(configs) => configs.collect(),
        Err(e) => {
            let fallback = fallback_output_format(
                device
                    .default_output_config()
                    .ok()
                    .map(|cfg| (cfg.sample_format(), cfg.channels())),
                config.channels,
            );
            log::warn!(
                "[Playback Thread] Cannot query supported formats: {}, falling back to {:?}/{}ch",
                e,
                fallback.0,
                fallback.1
            );
            return fallback;
        }
    };
    let candidates: Vec<_> = supported
        .iter()
        .map(|c| {
            (
                c.sample_format(),
                c.channels(),
                c.min_sample_rate(),
                c.max_sample_rate(),
            )
        })
        .collect();

    let log_configs = || {
        supported
            .iter()
            .map(|c| {
                format!(
                    "{:?}/{}ch/{}-{}Hz",
                    c.sample_format(),
                    c.channels(),
                    c.min_sample_rate(),
                    c.max_sample_rate()
                )
            })
            .collect::<Vec<_>>()
    };

    // First try: exact channel count match
    if let Some(fmt) =
        pick_preferred_output_format(&candidates, config.channels, config.sample_rate)
    {
        log::info!(
            "[Playback Thread] Chosen output format: {:?} for {}ch {}Hz (device configs: {:?})",
            fmt,
            config.channels,
            config.sample_rate,
            log_configs()
        );
        return (fmt, config.channels);
    }

    // Second try: device has a config with ch >= requested that supports this sample rate.
    // Use the requested channel count (not the device's) — CoreAudio/ALSA/WASAPI can
    // typically open a stream with fewer channels than the device maximum. This avoids
    // inflating to e.g. 94 channels on a Fireface UFX+ when only 6 are needed.
    let mut available_channels: Vec<u16> = supported
        .iter()
        .filter(|c| {
            c.min_sample_rate() <= config.sample_rate && c.max_sample_rate() >= config.sample_rate
        })
        .map(|c| c.channels())
        .collect();
    available_channels.sort();
    available_channels.dedup();

    if available_channels.iter().any(|&ch| ch >= config.channels) {
        let stream_channels = compatible_stream_channels(config.channels, &available_channels);
        // Select the format from the same native layout that will be opened.
        // Choosing format and channel count independently can synthesize an
        // unsupported pair (for example F32/6ch from F32/2ch + I16/6ch).
        let fmt = pick_preferred_output_format(&candidates, stream_channels, config.sample_rate);
        if let Some(fmt) = fmt {
            log::info!(
                "[Playback Thread] No exact {}ch config; using {}ch stream with {:?} format \
                 (device supports {:?}ch). Device configs: {:?}",
                config.channels,
                stream_channels,
                fmt,
                available_channels,
                log_configs()
            );
            return (fmt, stream_channels);
        }
    }

    // Third try: downmix — pick highest channel count <= requested.
    let alt_ch = available_channels
        .iter()
        .rev()
        .find(|&&ch| ch <= config.channels)
        .copied();

    if let Some(ch) = alt_ch
        && let Some(fmt) = pick_preferred_output_format(&candidates, ch, config.sample_rate)
    {
        log::info!(
            "[Playback Thread] Device doesn't support {}ch; using {}ch {:?} (will downmix). Device configs: {:?}",
            config.channels,
            ch,
            fmt,
            log_configs()
        );
        return (fmt, ch);
    }

    log::info!(
        "[Playback Thread] No compatible config for {}ch {}Hz among device formats, falling back to default format. Device configs: {:?}",
        config.channels,
        config.sample_rate,
        log_configs()
    );
    fallback_output_format(
        device
            .default_output_config()
            .ok()
            .map(|cfg| (cfg.sample_format(), cfg.channels())),
        config.channels,
    )
}

fn compatible_stream_channels(requested: u16, available: &[u16]) -> u16 {
    #[cfg(target_os = "macos")]
    {
        available
            .iter()
            .copied()
            .find(|&channels| channels >= requested)
            .unwrap_or(requested)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = available;
        requested
    }
}

pub(super) fn pick_preferred_output_format(
    candidates: &[(SampleFormat, u16, cpal::SampleRate, cpal::SampleRate)],
    channels: u16,
    sample_rate: cpal::SampleRate,
) -> Option<SampleFormat> {
    [
        SampleFormat::F32,
        SampleFormat::I32,
        SampleFormat::I16,
        SampleFormat::U32,
        SampleFormat::U16,
    ]
    .into_iter()
    .find(|fmt| {
        candidates.iter().any(|candidate| {
            candidate.0 == *fmt
                && candidate.1 == channels
                && candidate.2 <= sample_rate
                && candidate.3 >= sample_rate
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{compatible_stream_channels, pick_preferred_output_format};
    use cpal::{SampleFormat, SampleRate};

    #[test]
    fn compatible_stream_uses_native_coreaudio_channel_count() {
        let actual = compatible_stream_channels(2, &[6]);
        #[cfg(target_os = "macos")]
        assert_eq!(actual, 6);
        #[cfg(not(target_os = "macos"))]
        assert_eq!(actual, 2);
    }

    #[test]
    fn wider_stream_format_matches_the_selected_native_channel_layout() {
        let rate: SampleRate = 44_100;
        let candidates = [
            (SampleFormat::F32, 2, rate, rate),
            (SampleFormat::I16, 6, rate, rate),
        ];
        let channels = compatible_stream_channels(4, &[2, 6]);
        let format = pick_preferred_output_format(&candidates, channels, rate);

        #[cfg(target_os = "macos")]
        assert_eq!((format, channels), (Some(SampleFormat::I16), 6));
        #[cfg(not(target_os = "macos"))]
        assert_eq!((format, channels), (None, 4));
    }
}
