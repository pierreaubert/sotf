use super::misc::fallback_output_format;
use cpal::traits::DeviceTrait;
use cpal::{Device, SampleFormat, StreamConfig};

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
                "[CpalSink] Cannot query supported formats: {}, falling back to {:?}/{}ch",
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

    // First try: exact channel count match
    if let Some(fmt) =
        pick_preferred_output_format(&candidates, config.channels, config.sample_rate)
    {
        return (fmt, config.channels);
    }

    // Second try: if the device has any compatible config at or above the
    // requested width, keep the requested channel count. A 94ch interface can
    // prove that 10ch is within capability, but must not force SOTF to open 94ch.
    let mut available_channels: Vec<u16> = supported
        .iter()
        .filter(|c| {
            c.min_sample_rate() <= config.sample_rate && c.max_sample_rate() >= config.sample_rate
        })
        .map(|c| c.channels())
        .collect();
    available_channels.sort();
    available_channels.dedup();

    if available_channels.iter().any(|&ch| ch >= config.channels)
        && let Some(fmt) = pick_format_any_channels(&candidates, config.sample_rate)
    {
        log::info!(
            "[CpalSink] No exact {}ch config; using requested count with {:?} format (device supports {:?}ch)",
            config.channels,
            fmt,
            available_channels
        );
        return (fmt, config.channels);
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
        log::warn!(
            "[CpalSink] Device doesn't support {}ch; using {}ch {:?}",
            config.channels,
            ch,
            fmt,
        );
        return (fmt, ch);
    }

    fallback_output_format(
        device
            .default_output_config()
            .ok()
            .map(|cfg| (cfg.sample_format(), cfg.channels())),
        config.channels,
    )
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

/// Pick preferred format from any channel count config that supports the sample rate.
pub(super) fn pick_format_any_channels(
    candidates: &[(SampleFormat, u16, cpal::SampleRate, cpal::SampleRate)],
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
        candidates
            .iter()
            .any(|c| c.0 == *fmt && c.2 <= sample_rate && c.3 >= sample_rate)
    })
}

/// Pick the smallest hardware channel count above the requested logical width.
///
/// Some CoreAudio/aggregate/pro interfaces advertise only a large native stream
/// width. If CPAL rejects the logical stream width, the sink can retry at this
/// hardware width while still consuming and reporting the logical channel count.
pub(super) fn pick_wider_hardware_format(
    candidates: &[(SampleFormat, u16, cpal::SampleRate, cpal::SampleRate)],
    requested_channels: u16,
    sample_rate: cpal::SampleRate,
) -> Option<(SampleFormat, u16)> {
    let mut wider_channels: Vec<u16> = candidates
        .iter()
        .filter(|candidate| {
            candidate.1 > requested_channels
                && candidate.2 <= sample_rate
                && candidate.3 >= sample_rate
        })
        .map(|candidate| candidate.1)
        .collect();
    wider_channels.sort_unstable();
    wider_channels.dedup();

    wider_channels.into_iter().find_map(|channels| {
        pick_preferred_output_format(candidates, channels, sample_rate)
            .map(|format| (format, channels))
    })
}

pub(super) fn choose_wider_hardware_retry(
    device: &Device,
    config: &StreamConfig,
) -> Option<(SampleFormat, u16)> {
    let candidates: Vec<_> = device
        .supported_output_configs()
        .ok()?
        .map(|c| {
            (
                c.sample_format(),
                c.channels(),
                c.min_sample_rate(),
                c.max_sample_rate(),
            )
        })
        .collect();

    pick_wider_hardware_format(&candidates, config.channels, config.sample_rate)
}
