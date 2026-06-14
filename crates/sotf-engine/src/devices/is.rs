use super::device::device_matches_str;
use super::filter::filter_advertised_sample_rates;
use super::find::find_real_output_device;
use super::misc::ASIO_DEVICE_PREFIX;
use super::misc::build_sample_rate_candidates;
use super::misc::probe_channel_order;
use cpal::Sample;
use cpal::traits::{DeviceTrait, HostTrait};
use std::sync::Arc;

/// Check if a device identifier requests an ASIO device.
/// Case-insensitive: "ASIO:", "asio:", "Asio:" all match.
pub fn is_asio_device(identifier: &str) -> bool {
    identifier.len() >= ASIO_DEVICE_PREFIX.len()
        && identifier[..ASIO_DEVICE_PREFIX.len()].eq_ignore_ascii_case(ASIO_DEVICE_PREFIX)
}

/// Verify which sample rate actually produces working audio callbacks on a device.
///
/// On some Linux/ALSA systems, `default_output_config()` reports a rate (e.g., 44100Hz)
/// that doesn't actually produce callbacks. This function creates a brief test stream
/// at each candidate rate and checks that the audio callback fires.
///
/// Returns the first working sample rate, or None if none work.
pub fn verify_working_sample_rate(
    device_identifier: Option<&str>,
    requested_rate: u32,
    requested_channels: usize,
) -> Option<u32> {
    use cpal::StreamConfig;
    use cpal::traits::StreamTrait;
    use std::sync::atomic::{AtomicU64, Ordering};

    // On PipeWire, skip the verify probe entirely. PipeWire handles all sample rates
    // transparently via its built-in resampler, and the test stream can interfere with
    // the real playback stream on PipeWire's ALSA compatibility layer.
    #[cfg(target_os = "linux")]
    if is_pipewire() {
        log::info!(
            "[AUDIO] PipeWire detected, skipping sample rate verification (using {}Hz)",
            requested_rate
        );
        return Some(requested_rate);
    }

    let host = cpal::default_host();
    let device = if let Some(id) = device_identifier {
        let devices = host.output_devices().ok()?;
        devices.into_iter().find(|d| device_matches_str(d, id))?
    } else {
        find_real_output_device(&host)?
    };

    let device_default = device.default_output_config().map(|c| c.sample_rate()).ok();
    let advertised_ranges = device
        .supported_output_configs()
        .ok()
        .map(|configs| configs.collect::<Vec<_>>());
    let candidates = build_sample_rate_candidates(requested_rate, device_default);
    let filtered_candidates =
        filter_advertised_sample_rates(&candidates, advertised_ranges.as_deref());
    let candidates = if filtered_candidates.is_empty() {
        candidates
    } else {
        filtered_candidates
    };

    // Get device's default channel count for test streams
    let default_channels = device
        .default_output_config()
        .map(|c| c.channels())
        .unwrap_or(2);

    for &rate in &candidates {
        for test_channels in probe_channel_order(requested_channels, default_channels) {
            let config = StreamConfig {
                channels: test_channels,
                sample_rate: rate,
                buffer_size: cpal::BufferSize::Default,
            };

            let callback_count = Arc::new(AtomicU64::new(0));
            let total_samples = Arc::new(AtomicU64::new(0));

            // Try multiple sample formats — hw: devices often don't support f32.
            let stream = {
                let mut result = None;
                // Try f32 first (most common on PulseAudio/PipeWire), then i32, then i16
                {
                    let cc = callback_count.clone();
                    let ts = total_samples.clone();
                    if let Ok(s) = device.build_output_stream(
                        &config,
                        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                            cc.fetch_add(1, Ordering::Relaxed);
                            ts.fetch_add(data.len() as u64, Ordering::Relaxed);
                            data.fill(0.0);
                        },
                        |_err| {},
                        None,
                    ) {
                        result = Some(s);
                    }
                }
                if result.is_none() {
                    let cc = callback_count.clone();
                    let ts = total_samples.clone();
                    if let Ok(s) = device.build_output_stream(
                        &config,
                        move |data: &mut [i32], _: &cpal::OutputCallbackInfo| {
                            cc.fetch_add(1, Ordering::Relaxed);
                            ts.fetch_add(data.len() as u64, Ordering::Relaxed);
                            data.fill(0);
                        },
                        |_err| {},
                        None,
                    ) {
                        result = Some(s);
                    }
                }
                if result.is_none() {
                    let cc = callback_count.clone();
                    let ts = total_samples.clone();
                    if let Ok(s) = device.build_output_stream(
                        &config,
                        move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                            cc.fetch_add(1, Ordering::Relaxed);
                            ts.fetch_add(data.len() as u64, Ordering::Relaxed);
                            data.fill(0);
                        },
                        |_err| {},
                        None,
                    ) {
                        result = Some(s);
                    }
                }
                if result.is_none() {
                    let cc = callback_count.clone();
                    let ts = total_samples.clone();
                    if let Ok(s) = device.build_output_stream(
                        &config,
                        move |data: &mut [u32], _: &cpal::OutputCallbackInfo| {
                            cc.fetch_add(1, Ordering::Relaxed);
                            ts.fetch_add(data.len() as u64, Ordering::Relaxed);
                            data.fill(0);
                        },
                        |_err| {},
                        None,
                    ) {
                        result = Some(s);
                    }
                }
                if result.is_none() {
                    let cc = callback_count.clone();
                    let ts = total_samples.clone();
                    if let Ok(s) = device.build_output_stream(
                        &config,
                        move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                            cc.fetch_add(1, Ordering::Relaxed);
                            ts.fetch_add(data.len() as u64, Ordering::Relaxed);
                            data.fill(u16::from_sample(0.0f32));
                        },
                        |_err| {},
                        None,
                    ) {
                        result = Some(s);
                    }
                }
                match result {
                    Some(s) => s,
                    None => continue,
                }
            };

            if stream.play().is_err() {
                continue;
            }

            std::thread::sleep(std::time::Duration::from_millis(150));
            let count_phase1 = callback_count.load(Ordering::Relaxed);

            if count_phase1 == 0 {
                drop(stream);
                #[cfg(target_os = "linux")]
                std::thread::sleep(std::time::Duration::from_millis(50));
                #[cfg(not(target_os = "linux"))]
                std::thread::sleep(std::time::Duration::from_millis(30));
                log::debug!(
                    "[AUDIO] Device rate verification: {}Hz/{}ch - no callbacks in 150ms",
                    rate,
                    test_channels
                );
                continue;
            }

            std::thread::sleep(std::time::Duration::from_millis(150));
            let count_phase2 = callback_count.load(Ordering::Relaxed);
            let samples = total_samples.load(Ordering::Relaxed);

            drop(stream);
            #[cfg(target_os = "linux")]
            std::thread::sleep(std::time::Duration::from_millis(50));
            #[cfg(not(target_os = "linux"))]
            std::thread::sleep(std::time::Duration::from_millis(30));

            let expected_samples = rate as u64 * test_channels as u64 * 300 / 1000;
            let new_callbacks = count_phase2 - count_phase1;
            let enough_data = samples > expected_samples / 10;

            if enough_data && (new_callbacks > 0 || count_phase1 >= 2) {
                if rate != requested_rate {
                    log::warn!(
                        "[AUDIO] Device rate verification: requested {}Hz doesn't work, using {}Hz with {}ch ({} callbacks, {} samples in 300ms)",
                        requested_rate,
                        rate,
                        test_channels,
                        count_phase2,
                        samples
                    );
                } else {
                    log::info!(
                        "[AUDIO] Device rate verification: {}Hz works with {}ch ({} callbacks, {} samples in 300ms)",
                        rate,
                        test_channels,
                        count_phase2,
                        samples
                    );
                }
                return Some(rate);
            }

            log::debug!(
                "[AUDIO] Device rate verification: {}Hz/{}ch - stalled (phase1={} phase2={} callbacks, {} samples, expected >{})",
                rate,
                test_channels,
                count_phase1,
                count_phase2,
                samples,
                expected_samples / 10
            );
        }
    }

    log::warn!(
        "[AUDIO] Device rate verification: no working rate found (tried {:?})",
        candidates
    );
    None
}

/// Check if a device name looks like a virtual null/discard sink that won't produce real audio.
pub fn is_null_device(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("discard all samples")
        || lower.contains("null")
        || (lower.contains("generate zero") && lower.contains("capture"))
}

/// Detect if PipeWire is the active audio server on Linux.
#[cfg(target_os = "linux")]
fn is_pipewire() -> bool {
    // PIPEWIRE_RUNTIME_DIR is set by PipeWire when it's the active audio server
    if std::env::var("PIPEWIRE_RUNTIME_DIR").is_ok() {
        return true;
    }
    // Fallback: check XDG_RUNTIME_DIR for pipewire socket
    if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
        let socket = std::path::Path::new(&xdg).join("pipewire-0");
        if socket.exists() {
            return true;
        }
    }
    false
}
