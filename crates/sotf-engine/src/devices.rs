use cpal::Device;
use cpal::Sample;
use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Represents information about an audio device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    /// Stable device identifier (persists across reboots)
    pub device_id: Option<String>,
    /// Human-readable display name (from cpal description)
    pub name: String,
    /// Extended display info (manufacturer, interface type, etc.)
    pub display_info: Option<String>,
    pub is_input: bool,
    pub is_default: bool,
    pub supported_configs: Vec<AudioConfig>,
    pub default_config: Option<AudioConfig>,
    pub available_sample_rates: Vec<u32>, // List of available sample rates for user selection
}

/// Represents audio configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_size: Option<u32>,
    pub sample_format: String, // "f32", "i16", "u16"
}

/// State for storing the currently selected audio configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AudioState {
    /// Selected input device ID (stable across reboots)
    /// Falls back to name matching for legacy saved states
    pub selected_input_device: Option<String>,
    /// Selected output device ID (stable across reboots)
    /// Falls back to name matching for legacy saved states
    pub selected_output_device: Option<String>,
    pub input_config: Option<AudioConfig>,
    pub output_config: Option<AudioConfig>,
}

pub type SharedAudioState = Arc<Mutex<AudioState>>;

/// Helper function to convert cpal sample format to string
fn format_to_string(format: cpal::SampleFormat) -> String {
    match format {
        cpal::SampleFormat::F32 => "f32".to_string(),
        cpal::SampleFormat::I16 => "i16".to_string(),
        cpal::SampleFormat::U16 => "u16".to_string(),
        _ => "unknown".to_string(),
    }
}

// =============================================================================
// ASIO Host Selection (Windows only, requires `asio` feature)
// =============================================================================

/// ASIO device prefix. Device identifiers starting with "ASIO:" will use the
/// ASIO host instead of the default (WASAPI) host on Windows.
///
/// Example: "ASIO:Focusrite USB ASIO" selects the Focusrite ASIO driver.
pub const ASIO_DEVICE_PREFIX: &str = "ASIO:";

/// Check if a device identifier requests an ASIO device.
/// Case-insensitive: "ASIO:", "asio:", "Asio:" all match.
pub fn is_asio_device(identifier: &str) -> bool {
    identifier.len() >= ASIO_DEVICE_PREFIX.len()
        && identifier[..ASIO_DEVICE_PREFIX.len()].eq_ignore_ascii_case(ASIO_DEVICE_PREFIX)
}

/// Strip the "ASIO:" prefix from a device identifier, returning the actual device name.
/// Case-insensitive: "ASIO:", "asio:", "Asio:" prefixes are all stripped.
pub fn strip_asio_prefix(identifier: &str) -> &str {
    if is_asio_device(identifier) {
        &identifier[ASIO_DEVICE_PREFIX.len()..]
    } else {
        identifier
    }
}

/// Get the appropriate cpal Host for a device identifier.
///
/// On Windows with the `asio` feature enabled, if the identifier starts with "ASIO:",
/// returns the ASIO host. Otherwise returns the default host (WASAPI on Windows,
/// CoreAudio on macOS, ALSA on Linux).
///
/// # ASIO Usage
///
/// To use an ASIO device, prefix the device name with "ASIO:":
/// ```text
/// "ASIO:Focusrite USB ASIO"     -> uses ASIO host, device "Focusrite USB ASIO"
/// "Focusrite USB ASIO"          -> uses default host (WASAPI)
/// "Built-in Output"             -> uses default host
/// ```
///
/// ASIO provides lower latency (~2-5ms vs WASAPI's ~10-30ms) but requires:
/// - ASIO drivers installed for the audio hardware
/// - The `asio` feature enabled at build time (requires Steinberg ASIO SDK)
/// - Exclusive device access (other apps can't use the device simultaneously)
pub fn get_host_for_device(device_identifier: Option<&str>) -> cpal::Host {
    #[cfg(all(target_os = "windows", feature = "asio"))]
    {
        if let Some(id) = device_identifier {
            if is_asio_device(id) {
                let asio_name = strip_asio_prefix(id);
                log::info!(
                    "[AUDIO] ASIO device requested: '{}', initializing ASIO host",
                    asio_name
                );

                match cpal::host_from_id(cpal::HostId::Asio) {
                    Ok(host) => {
                        log::info!("[AUDIO] ASIO host initialized successfully");
                        return host;
                    }
                    Err(e) => {
                        log::error!(
                            "[AUDIO] Failed to initialize ASIO host: {}. Falling back to default host.",
                            e
                        );
                    }
                }
            }
        }
    }

    #[cfg(not(all(target_os = "windows", feature = "asio")))]
    if let Some(id) = device_identifier
        && is_asio_device(id)
    {
        #[cfg(not(target_os = "windows"))]
        log::warn!(
            "[AUDIO] ASIO device '{}' requested but ASIO is only available on Windows",
            strip_asio_prefix(id)
        );
        #[cfg(all(target_os = "windows", not(feature = "asio")))]
        log::warn!(
            "[AUDIO] ASIO device '{}' requested but the 'asio' feature is not enabled. \
             Rebuild with --features asio to enable ASIO support.",
            strip_asio_prefix(id)
        );
    }

    cpal::default_host()
}

/// List ASIO devices available on the system.
///
/// Returns device names prefixed with "ASIO:" for use with `get_host_for_device`.
/// Returns an empty Vec if ASIO is not available.
#[cfg(all(target_os = "windows", feature = "asio"))]
pub fn list_asio_devices() -> Vec<String> {
    match cpal::host_from_id(cpal::HostId::Asio) {
        Ok(host) => {
            let mut devices = Vec::new();
            if let Ok(output_devices) = host.output_devices() {
                for device in output_devices {
                    if let Ok(desc) = device.description() {
                        devices.push(format!("{}{}", ASIO_DEVICE_PREFIX, desc.name()));
                    }
                }
            }
            devices
        }
        Err(e) => {
            log::debug!("[AUDIO] ASIO not available: {}", e);
            Vec::new()
        }
    }
}

/// List ASIO devices (stub when ASIO is not available).
#[cfg(not(all(target_os = "windows", feature = "asio")))]
pub fn list_asio_devices() -> Vec<String> {
    Vec::new()
}

/// Extract device info from cpal device using description() and id()
fn get_device_info<D: DeviceTrait>(device: &D) -> Option<(String, Option<String>, Option<String>)> {
    // Get display name from description
    let desc = device.description().ok()?;
    let name = desc.name().to_string();
    // Build extended display info from manufacturer and interface type
    let mut info_parts = Vec::new();
    if let Some(manufacturer) = desc.manufacturer() {
        info_parts.push(manufacturer.to_string());
    }
    let interface_str = format!("{:?}", desc.interface_type());
    if interface_str != "Unknown" {
        info_parts.push(interface_str);
    }
    let display_info = if info_parts.is_empty() {
        None
    } else {
        Some(info_parts.join(" - "))
    };

    // Get stable device ID for persistence
    let device_id = device.id().ok().map(|id| id.to_string());

    Some((name, device_id, display_info))
}

/// Get information about all available audio devices
pub fn get_audio_devices() -> Result<HashMap<String, Vec<AudioDevice>>, String> {
    let host = cpal::default_host();
    let mut devices_map = HashMap::new();

    // Get input devices
    let mut input_devices = Vec::new();
    match host.input_devices() {
        Ok(devices) => {
            let default_input = host.default_input_device();
            let default_input_id = default_input.as_ref().and_then(|d| d.id().ok());

            // WORKAROUND: On macOS, collecting devices into a Vec first can prevent
            // crashes caused by iterator issues with CoreAudio
            let device_vec: Vec<_> = devices.collect();
            for device in device_vec {
                if let Some((name, device_id, display_info)) = get_device_info(&device) {
                    // Compare by device ID if available, otherwise by name
                    let is_default = match (&device_id, &default_input_id) {
                        (Some(id), Some(default_id)) => id == &default_id.to_string(),
                        _ => false,
                    };

                    // Get supported configurations
                    let mut supported_configs = Vec::new();
                    if let Ok(configs) = device.supported_input_configs() {
                        for config in configs {
                            let config_range = config;
                            // Add min and max sample rate configs
                            for sample_rate in [
                                config_range.min_sample_rate(),
                                config_range.max_sample_rate(),
                            ] {
                                // Only include valid channel configurations (1 or 2 for input devices)
                                let max_channels = config_range.channels();
                                let channel_configs: Vec<u16> = if max_channels == 1 {
                                    vec![1]
                                } else if max_channels >= 2 {
                                    vec![1, 2] // Most inputs are mono or stereo
                                } else {
                                    vec![max_channels] // Fallback to device max
                                };

                                for &channels in &channel_configs {
                                    supported_configs.push(AudioConfig {
                                        sample_rate,
                                        channels,
                                        buffer_size: None,
                                        sample_format: format_to_string(config.sample_format()),
                                    });
                                }
                            }
                        }
                    }

                    // Get configuration with most channels (instead of default)
                    // Use current/default sample rate, not max
                    let (default_config, available_sample_rates) =
                        if let Ok(configs_iter) = device.supported_input_configs() {
                            let configs: Vec<_> = configs_iter.collect();

                            // Find config with most channels
                            let max_channel_config =
                                configs.iter().max_by_key(|config| config.channels());

                            // Get current sample rate from device default
                            let current_sample_rate = device
                                .default_input_config()
                                .map(|cfg| cfg.sample_rate())
                                .unwrap_or(48000); // Fallback to 48kHz

                            let default_cfg = max_channel_config.map(|config| {
                                // Use current sample rate, clamped to supported range
                                let sample_rate = current_sample_rate
                                    .max(config.min_sample_rate())
                                    .min(config.max_sample_rate());

                                AudioConfig {
                                    sample_rate,
                                    channels: config.channels(),
                                    buffer_size: None,
                                    sample_format: format_to_string(config.sample_format()),
                                }
                            });

                            // Collect all available sample rates across all configs
                            let mut sample_rates = std::collections::HashSet::new();
                            for config in &configs {
                                sample_rates.insert(config.min_sample_rate());
                                sample_rates.insert(config.max_sample_rate());
                                // Add common rates if in range
                                for &rate in &[44100, 48000, 88200, 96000, 176400, 192000] {
                                    if rate >= config.min_sample_rate()
                                        && rate <= config.max_sample_rate()
                                    {
                                        sample_rates.insert(rate);
                                    }
                                }
                            }
                            let mut rates: Vec<u32> = sample_rates.into_iter().collect();
                            rates.sort_unstable();

                            (default_cfg, rates)
                        } else {
                            (None, Vec::new())
                        };

                    // Report what we detected
                    if let Some(ref cfg) = default_config {
                        let rate_range = match (
                            available_sample_rates.first(),
                            available_sample_rates.last(),
                        ) {
                            (None, _) => "unknown".to_string(),
                            (Some(rate), Some(_)) if available_sample_rates.len() == 1 => {
                                format!("{} Hz", rate)
                            }
                            (Some(first), Some(last)) => format!("{}-{} Hz", first, last),
                            _ => "unknown".to_string(),
                        };
                        format!(
                            "{} ch, {} (current: {} Hz)",
                            cfg.channels, rate_range, cfg.sample_rate
                        )
                    } else {
                        "unknown".to_string()
                    };

                    input_devices.push(AudioDevice {
                        device_id,
                        name,
                        display_info,
                        is_input: true,
                        is_default,
                        supported_configs,
                        default_config,
                        available_sample_rates,
                    });
                }
            }
        }
        Err(e) => {
            log::debug!("[AUDIO ERROR] Failed to enumerate input devices: {}", e);
            // Continue with empty input devices list rather than failing completely
        }
    }

    // Get output devices
    let mut output_devices = Vec::new();
    match host.output_devices() {
        Ok(devices) => {
            let default_output = host.default_output_device();
            let default_output_id = default_output.as_ref().and_then(|d| d.id().ok());

            // WORKAROUND: On macOS, collecting devices into a Vec first can prevent
            // crashes caused by iterator issues with CoreAudio
            let device_vec: Vec<_> = devices.collect();
            for device in device_vec {
                if let Some((name, device_id, display_info)) = get_device_info(&device) {
                    // Compare by device ID if available
                    let is_default = match (&device_id, &default_output_id) {
                        (Some(id), Some(default_id)) => id == &default_id.to_string(),
                        _ => false,
                    };

                    // Get supported configurations
                    let mut supported_configs = Vec::new();
                    if let Ok(configs) = device.supported_output_configs() {
                        for config in configs {
                            let config_range = config;
                            // Add common sample rates
                            for sample_rate in [
                                44100,
                                48000,
                                88200,
                                96000,
                                176400,
                                192000,
                                config_range.min_sample_rate(),
                                config_range.max_sample_rate(),
                            ] {
                                if sample_rate < config_range.min_sample_rate()
                                    || sample_rate > config_range.max_sample_rate()
                                {
                                    continue;
                                }
                                // Common channel configurations
                                for &channels in &[1, 2, config_range.channels()] {
                                    if channels > config_range.channels() {
                                        continue;
                                    }

                                    // Avoid duplicates
                                    let config = AudioConfig {
                                        sample_rate,
                                        channels,
                                        buffer_size: None,
                                        sample_format: format_to_string(config.sample_format()),
                                    };

                                    if !supported_configs.iter().any(|c: &AudioConfig| {
                                        c.sample_rate == config.sample_rate
                                            && c.channels == config.channels
                                            && c.sample_format == config.sample_format
                                    }) {
                                        supported_configs.push(config);
                                    }
                                }
                            }
                        }
                    }

                    // Get configuration with most channels (instead of default)
                    // Use current/default sample rate, not max
                    let (default_config, available_sample_rates) = if let Ok(configs_iter) =
                        device.supported_output_configs()
                    {
                        let configs: Vec<_> = configs_iter.collect();

                        for (idx, cfg) in configs.iter().enumerate() {
                            log::info!(
                                "[AUDIO] Output device '{}' config range [{}]: channels={}, sample_rate={}..{}, format={:?}",
                                name,
                                idx,
                                cfg.channels(),
                                cfg.min_sample_rate(),
                                cfg.max_sample_rate(),
                                cfg.sample_format(),
                            );
                        }

                        // Find config with most channels
                        let max_channel_config =
                            configs.iter().max_by_key(|config| config.channels());

                        // Get current sample rate from device default
                        let current_sample_rate = device
                            .default_output_config()
                            .map(|cfg| cfg.sample_rate())
                            .unwrap_or(48000); // Fallback to 48kHz

                        let default_cfg = max_channel_config.map(|config| {
                            // Use current sample rate, clamped to supported range
                            let sample_rate = current_sample_rate
                                .max(config.min_sample_rate())
                                .min(config.max_sample_rate());

                            AudioConfig {
                                sample_rate,
                                channels: config.channels(),
                                buffer_size: None,
                                sample_format: format_to_string(config.sample_format()),
                            }
                        });

                        // Collect all available sample rates across all configs
                        let mut sample_rates = std::collections::HashSet::new();
                        for config in &configs {
                            sample_rates.insert(config.min_sample_rate());
                            sample_rates.insert(config.max_sample_rate());
                            // Add common rates if in range
                            for &rate in &[44100, 48000, 88200, 96000, 176400, 192000] {
                                if rate >= config.min_sample_rate()
                                    && rate <= config.max_sample_rate()
                                {
                                    sample_rates.insert(rate);
                                }
                            }
                        }
                        let mut rates: Vec<u32> = sample_rates.into_iter().collect();
                        rates.sort_unstable();

                        (default_cfg, rates)
                    } else {
                        (None, Vec::new())
                    };

                    // Report what we detected - don't make assumptions
                    if let Some(ref cfg) = default_config {
                        let rate_range = match (
                            available_sample_rates.first(),
                            available_sample_rates.last(),
                        ) {
                            (None, _) => "unknown".to_string(),
                            (Some(rate), Some(_)) if available_sample_rates.len() == 1 => {
                                format!("{} Hz", rate)
                            }
                            (Some(first), Some(last)) => format!("{}-{} Hz", first, last),
                            _ => "unknown".to_string(),
                        };
                        format!(
                            "{} ch, {} (current: {} Hz)",
                            cfg.channels, rate_range, cfg.sample_rate
                        )
                    } else {
                        "unknown".to_string()
                    };

                    output_devices.push(AudioDevice {
                        device_id,
                        name,
                        display_info,
                        is_input: false,
                        is_default,
                        supported_configs,
                        default_config,
                        available_sample_rates,
                    });
                }
            }
        }
        Err(e) => {
            log::debug!("[AUDIO ERROR] Failed to enumerate output devices: {}", e);
            // Continue with empty output devices list rather than failing completely
        }
    }

    // On Linux/ALSA, cpal exposes many virtual device nodes per physical card
    // (hw:0, plughw:0, default, sysdefault, front:*, surround*:*, etc.).
    // Group them by hardware name so the user sees one entry per physical device.
    #[cfg(target_os = "linux")]
    let input_devices = deduplicate_linux_devices(input_devices);
    #[cfg(target_os = "linux")]
    let output_devices = deduplicate_linux_devices(output_devices);

    // On Windows, WASAPI reports names like "Speakers (RME Fireface UCX)".
    // When multiple devices share the same prefix, strip it so only the
    // distinguishing part remains.
    #[cfg(target_os = "windows")]
    let input_devices = strip_duplicate_prefixes_windows(input_devices);
    #[cfg(target_os = "windows")]
    let output_devices = strip_duplicate_prefixes_windows(output_devices);

    devices_map.insert("input".to_string(), input_devices);
    devices_map.insert("output".to_string(), output_devices);

    // Check if no devices were found at all
    // Note: Using map_or instead of is_none_or for Rust 1.90.0 stability
    // (is_none_or requires Rust 1.82.0+)
    #[allow(clippy::unnecessary_map_or)]
    if devices_map.get("input").map_or(true, |v| v.is_empty())
        && devices_map.get("output").map_or(true, |v| v.is_empty())
    {
        log::debug!("[AUDIO WARNING] No audio devices found on the system");
    }

    Ok(devices_map)
}

/// Get supported sample rates for a specific output device
///
/// # Arguments
/// * `device_identifier` - Device ID or name. If None, uses default output device.
///
/// # Returns
/// Sorted Vec of supported sample rates, or None if device not found
pub fn get_device_supported_sample_rates(device_identifier: Option<&str>) -> Option<Vec<u32>> {
    let host = cpal::default_host();

    // Find the device
    let device = if let Some(identifier) = device_identifier {
        // Look for specific device by ID or name
        host.output_devices()
            .ok()?
            .find(|d| device_matches_str(d, identifier))
    } else {
        // Use default device
        host.default_output_device()
    }?;

    // Collect supported sample rates from all configurations
    let mut sample_rates = std::collections::HashSet::new();
    if let Ok(configs) = device.supported_output_configs() {
        for config in configs {
            sample_rates.insert(config.min_sample_rate());
            sample_rates.insert(config.max_sample_rate());
            // Add common rates if in range
            for &rate in &[44100u32, 48000, 88200, 96000, 176400, 192000] {
                if rate >= config.min_sample_rate() && rate <= config.max_sample_rate() {
                    sample_rates.insert(rate);
                }
            }
        }
    }

    if sample_rates.is_empty() {
        return None;
    }

    let mut rates: Vec<u32> = sample_rates.into_iter().collect();
    rates.sort_unstable();
    Some(rates)
}

/// Get the current (actual running) sample rate of an output device
///
/// Unlike `get_device_supported_sample_rates()` which returns what the device *claims* to support,
/// this returns the rate the device is actually running at via `default_output_config()`.
/// On macOS, the "supported" range may include rates the device won't switch to automatically,
/// so using the current rate and resampling is the correct approach.
///
/// # Arguments
/// * `device_identifier` - Device ID or name. If None, uses default output device.
///
/// # Returns
/// The device's current sample rate, or None if device not found
pub fn get_device_current_sample_rate(device_identifier: Option<&str>) -> Option<u32> {
    let host = cpal::default_host();

    // Find the device
    let device = if let Some(identifier) = device_identifier {
        let devices = match host.output_devices() {
            Ok(d) => d,
            Err(e) => {
                crate::rate_limited_log!(
                    warn,
                    5,
                    "[AUDIO] Failed to enumerate output devices for sample rate query: {}",
                    e
                );
                return None;
            }
        };
        match devices
            .into_iter()
            .find(|d| device_matches_str(d, identifier))
        {
            Some(d) => d,
            None => {
                crate::rate_limited_log!(
                    warn,
                    5,
                    "[AUDIO] Device '{}' not found for sample rate query",
                    identifier
                );
                return None;
            }
        }
    } else {
        match find_real_output_device(&host) {
            Some(d) => d,
            None => {
                crate::rate_limited_log!(
                    warn,
                    5,
                    "[AUDIO] No default output device available for sample rate query"
                );
                return None;
            }
        }
    };

    match device.default_output_config() {
        Ok(config) => {
            let rate = config.sample_rate();
            log::debug!("[AUDIO] Device sample rate query successful: {}Hz", rate);
            Some(rate)
        }
        Err(e) => {
            crate::rate_limited_log!(
                warn,
                5,
                "[AUDIO] Failed to get default output config for sample rate: {}",
                e
            );
            None
        }
    }
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

fn probe_channel_order(requested_channels: usize, default_channels: u16) -> Vec<u16> {
    let requested = u16::try_from(requested_channels).ok().filter(|&ch| ch > 0);
    let mut order = Vec::new();

    if let Some(ch) = requested {
        order.push(ch);
    }
    if order.first().copied() != Some(default_channels) {
        order.push(default_channels);
    }

    order
}

fn build_sample_rate_candidates(requested_rate: u32, device_default: Option<u32>) -> Vec<u32> {
    let mut candidates = vec![requested_rate];
    for rate in [48_000, 44_100, 96_000, 192_000] {
        if !candidates.contains(&rate) {
            candidates.push(rate);
        }
    }
    if let Some(rate) = device_default
        && !candidates.contains(&rate)
    {
        candidates.push(rate);
    }
    candidates
}

fn filter_advertised_sample_rates(
    candidates: &[u32],
    advertised_ranges: Option<&[cpal::SupportedStreamConfigRange]>,
) -> Vec<u32> {
    let Some(advertised_ranges) = advertised_ranges else {
        return candidates.to_vec();
    };

    let advertised_bounds: Vec<(u32, u32)> = advertised_ranges
        .iter()
        .map(|range| (range.min_sample_rate(), range.max_sample_rate()))
        .collect();
    filter_sample_rates_by_bounds(candidates, &advertised_bounds)
}

fn filter_sample_rates_by_bounds(candidates: &[u32], advertised_bounds: &[(u32, u32)]) -> Vec<u32> {
    candidates
        .iter()
        .copied()
        .filter(|rate| {
            advertised_bounds
                .iter()
                .any(|(min, max)| min <= rate && rate <= max)
        })
        .collect()
}

/// Check if a device name looks like a virtual null/discard sink that won't produce real audio.
pub fn is_null_device(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("discard all samples")
        || lower.contains("null")
        || (lower.contains("generate zero") && lower.contains("capture"))
}

/// Given a cpal host, find the first non-null output device.
/// Returns the default device if it's not a null device, otherwise scans for a real one.
fn find_real_output_device(host: &cpal::Host) -> Option<Device> {
    let default = host.default_output_device()?;
    let name = default
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_default();

    if !is_null_device(&name) {
        return Some(default);
    }

    log::warn!(
        "[AUDIO] Default output device is '{}' (null sink), searching for a real device",
        name
    );

    // Find the first real hardware device
    let devices = host.output_devices().ok()?;
    for dev in devices {
        let dev_name = dev
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_default();
        if !is_null_device(&dev_name) && !dev_name.is_empty() {
            log::info!("[AUDIO] Using fallback output device: '{}'", dev_name);
            return Some(dev);
        }
    }

    log::warn!("[AUDIO] No real output device found, using null sink as last resort");
    Some(default)
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

/// Helper to match device by string identifier
fn device_matches_str<D: DeviceTrait>(device: &D, identifier: &str) -> bool {
    // First try to match by device ID (preferred for persistence)
    if let Ok(id) = device.id()
        && id.to_string() == identifier
    {
        return true;
    }
    // Try description name
    if let Ok(desc) = device.description() {
        let name = desc.name();
        // Exact match
        if name == identifier {
            return true;
        }
        // Case-insensitive match
        if name.to_lowercase() == identifier.to_lowercase() {
            return true;
        }
        // Partial match (starts with or contains)
        let lower_name = name.to_lowercase();
        let lower_id = identifier.to_lowercase();
        if lower_name.starts_with(&lower_id) || lower_name.contains(&lower_id) {
            return true;
        }
    }
    false
}

/// Check if a device matches the given identifier (ID preferred, name fallback)
fn device_matches<D: DeviceTrait>(device: &D, identifier: &str) -> bool {
    // First try to match by device ID (preferred for persistence)
    if let Ok(id) = device.id()
        && id.to_string() == identifier
    {
        return true;
    }
    // Fallback to name matching for legacy saved states
    if let Ok(desc) = device.description()
        && desc.name() == identifier
    {
        return true;
    }
    false
}

/// Set the configuration for an audio device
pub fn set_audio_device(
    device_identifier: String,
    is_input: bool,
    config: AudioConfig,
    audio_state: &SharedAudioState,
) -> Result<String, String> {
    let host = cpal::default_host();

    // Find the device by ID or name
    let device = if is_input {
        host.input_devices()
            .map_err(|e| format!("Failed to enumerate input devices: {}", e))?
            .find(|d| device_matches(d, &device_identifier))
    } else {
        host.output_devices()
            .map_err(|e| format!("Failed to enumerate output devices: {}", e))?
            .find(|d| device_matches(d, &device_identifier))
    };

    let device = device.ok_or_else(|| format!("Device '{}' not found", device_identifier))?;

    // Validate the configuration against device capabilities
    let config_valid = if is_input {
        match device.supported_input_configs() {
            Ok(configs) => {
                let mut valid = false;
                for supported_config in configs {
                    let sample_rate = config.sample_rate;
                    if supported_config.min_sample_rate() <= sample_rate
                        && supported_config.max_sample_rate() >= sample_rate
                        && supported_config.channels() >= config.channels
                        && format_to_string(supported_config.sample_format())
                            == config.sample_format
                    {
                        valid = true;
                        break;
                    }
                }
                valid
            }
            Err(e) => {
                return Err(format!("Failed to get input configs: {}", e));
            }
        }
    } else {
        match device.supported_output_configs() {
            Ok(configs) => {
                let mut valid = false;
                for supported_config in configs {
                    let sample_rate = config.sample_rate;
                    if supported_config.min_sample_rate() <= sample_rate
                        && supported_config.max_sample_rate() >= sample_rate
                        && supported_config.channels() >= config.channels
                        && format_to_string(supported_config.sample_format())
                            == config.sample_format
                    {
                        valid = true;
                        break;
                    }
                }
                valid
            }
            Err(e) => {
                return Err(format!("Failed to get output configs: {}", e));
            }
        }
    };

    if !config_valid {
        log::info!(
            "[AUDIO ERROR] Invalid configuration for device '{}': sample_rate={}, channels={}, format={}",
            device_identifier,
            config.sample_rate,
            config.channels,
            config.sample_format
        );
        return Err(format!(
            "Configuration not supported by device '{}': sample_rate={}, channels={}, format={}",
            device_identifier, config.sample_rate, config.channels, config.sample_format
        ));
    }

    // Get the device ID for persistence (preferred over name)
    let device_id_for_state = device
        .id()
        .ok()
        .map(|id| id.to_string())
        .unwrap_or_else(|| device_identifier.clone());

    // Store the configuration in the application state
    let mut state = audio_state
        .lock()
        .map_err(|e| format!("Failed to lock audio state: {}", e))?;

    if is_input {
        state.selected_input_device = Some(device_id_for_state.clone());
        state.input_config = Some(config.clone());
    } else {
        state.selected_output_device = Some(device_id_for_state.clone());
        state.output_config = Some(config.clone());
    }

    let success_msg = format!(
        "Successfully configured {} device '{}' with sample_rate={}, channels={}, format={}",
        if is_input { "input" } else { "output" },
        device_identifier,
        config.sample_rate,
        config.channels,
        config.sample_format
    );
    Ok(success_msg)
}

/// Get the current audio configuration
pub fn get_audio_config(audio_state: &SharedAudioState) -> Result<AudioState, String> {
    let state = audio_state.lock().map_err(|e| {
        log::debug!("[AUDIO ERROR] Failed to lock audio state: {}", e);
        format!("Failed to lock audio state: {}", e)
    })?;
    Ok(state.clone())
}

/// Get detailed properties of a specific audio device
pub fn get_device_properties(
    device_identifier: String,
    is_input: bool,
) -> Result<serde_json::Value, String> {
    let host = cpal::default_host();

    // Find the device by ID or name
    let device = if is_input {
        host.input_devices()
            .map_err(|e| format!("Failed to enumerate input devices: {}", e))?
            .find(|d| device_matches(d, &device_identifier))
    } else {
        host.output_devices()
            .map_err(|e| format!("Failed to enumerate output devices: {}", e))?
            .find(|d| device_matches(d, &device_identifier))
    };

    let device = device.ok_or_else(|| format!("Device '{}' not found", device_identifier))?;

    // Get display name from description
    let display_name = device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| device_identifier.clone());

    // Get all supported configurations
    let mut properties = serde_json::json!({
        "name": display_name,
        "type": if is_input { "input" } else { "output" },
    });

    let mut config_ranges = Vec::new();
    if is_input {
        if let Ok(configs) = device.supported_input_configs() {
            for config in configs {
                config_ranges.push(serde_json::json!({
                    "min_sample_rate": config.min_sample_rate(),
                    "max_sample_rate": config.max_sample_rate(),
                    "channels": config.channels(),
                    "sample_format": format_to_string(config.sample_format()),
                    "buffer_size_range": match config.buffer_size() {
                        cpal::SupportedBufferSize::Range { min, max } => {
                            serde_json::json!({ "min": min, "max": max })
                        },
                        cpal::SupportedBufferSize::Unknown => serde_json::json!("unknown"),
                    },
                }));
            }
        }
    } else if let Ok(configs) = device.supported_output_configs() {
        for config in configs {
            config_ranges.push(serde_json::json!({
                "min_sample_rate": config.min_sample_rate(),
                "max_sample_rate": config.max_sample_rate(),
                "channels": config.channels(),
                "sample_format": format_to_string(config.sample_format()),
                "buffer_size_range": match config.buffer_size() {
                    cpal::SupportedBufferSize::Range { min, max } => {
                        serde_json::json!({ "min": min, "max": max })
                    },
                    cpal::SupportedBufferSize::Unknown => serde_json::json!("unknown"),
                },
            }));
        }
    }
    properties["supported_config_ranges"] = serde_json::json!(config_ranges);

    // Get default configuration
    if is_input {
        if let Ok(default_config) = device.default_input_config() {
            properties["default_config"] = serde_json::json!({
                "sample_rate": default_config.sample_rate(),
                "channels": default_config.channels(),
                "sample_format": format_to_string(default_config.sample_format()),
                "buffer_size": match default_config.buffer_size() {
                    cpal::SupportedBufferSize::Range { min, max } => {
                        serde_json::json!({ "min": min, "max": max })
                    },
                    cpal::SupportedBufferSize::Unknown => serde_json::json!("unknown"),
                },
            });
        }
    } else if let Ok(default_config) = device.default_output_config() {
        properties["default_config"] = serde_json::json!({
            "sample_rate": default_config.sample_rate(),
            "channels": default_config.channels(),
            "sample_format": format_to_string(default_config.sample_format()),
            "buffer_size": match default_config.buffer_size() {
                cpal::SupportedBufferSize::Range { min, max } => {
                    serde_json::json!({ "min": min, "max": max })
                },
                cpal::SupportedBufferSize::Unknown => serde_json::json!("unknown"),
            },
        });
    }

    Ok(properties)
}

/// Helper to match a device from a list of (id, name) tuples based on identifier
///
/// Priority:
/// 1. Exact ID match
/// 2. Exact Name match (case-insensitive)
/// 3. Starts With match (case-insensitive)
/// 4. Contains match (case-insensitive)
fn match_device_priority(devices: &[(String, String)], identifier: &str) -> Option<usize> {
    let target = identifier.to_lowercase();

    // 1. Exact ID match
    if let Some(idx) = devices.iter().position(|(id, _)| id == identifier) {
        log::debug!("[find_device] Found device by ID match: {}", devices[idx].0);
        return Some(idx);
    }

    // 2. Exact Name match (case-insensitive)
    if let Some(idx) = devices
        .iter()
        .position(|(_, name)| name.to_lowercase() == target)
    {
        log::debug!(
            "[find_device] Found device by Exact Name match: {}",
            devices[idx].1
        );
        return Some(idx);
    }

    // 3. Starts With match (case-insensitive)
    if let Some(idx) = devices
        .iter()
        .position(|(_, name)| name.to_lowercase().starts_with(&target))
    {
        log::debug!(
            "[find_device] Found device by Starts With match: {}",
            devices[idx].1
        );
        return Some(idx);
    }

    // 4. Contains match (case-insensitive)
    if let Some(idx) = devices
        .iter()
        .position(|(_, name)| name.to_lowercase().contains(&target))
    {
        log::debug!(
            "[find_device] Found device by Contains match: {}",
            devices[idx].1
        );
        return Some(idx);
    }

    None
}

fn summarize_available_device_names(device_info: &[(String, String)], limit: usize) -> String {
    let mut names: Vec<String> = device_info
        .iter()
        .filter_map(|(_, name)| {
            let trimmed = name.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
        .collect();
    names.sort();
    names.dedup();

    let shown: Vec<&str> = names.iter().take(limit).map(String::as_str).collect();
    if names.len() > limit {
        format!(
            "{} ... and {} more",
            shown.join(", "),
            names.len().saturating_sub(limit)
        )
    } else {
        shown.join(", ")
    }
}

/// Find an audio device by name or ID with prioritization
pub fn find_device(
    host: &cpal::Host,
    identifier: &str,
    is_input: bool,
) -> Result<cpal::Device, String> {
    let devices: Vec<cpal::Device> = if is_input {
        host.input_devices()
            .map_err(|e| format!("Failed to enumerate input devices: {}", e))?
            .collect()
    } else {
        host.output_devices()
            .map_err(|e| format!("Failed to enumerate output devices: {}", e))?
            .collect()
    };

    // Extract info for matching
    let device_info: Vec<(String, String)> = devices
        .iter()
        .map(|d| {
            let id = d.id().ok().map(|i| i.to_string()).unwrap_or_default();
            let name = d
                .description()
                .ok()
                .map(|desc| desc.name().to_string())
                .unwrap_or_default();
            (id, name)
        })
        .collect();

    if let Some(idx) = match_device_priority(&device_info, identifier) {
        Ok(devices[idx].clone())
    } else {
        // Device not found - provide helpful error message with available devices
        let device_type = if is_input { "input" } else { "output" };
        let available_summary = summarize_available_device_names(&device_info, 12);
        Err(format!(
            "Audio device '{}' not found. Available {} devices ({} total): {}",
            identifier,
            device_type,
            device_info.len(),
            available_summary
        ))
    }
}

/// On Windows, WASAPI reports device names like "Speakers (RME Fireface UCX)" or
/// "Microphone (Realtek Audio)". When multiple devices share the same prefix
/// (e.g. two "Speakers (...)" entries), strip the prefix so the user sees just
/// the distinguishing device name.
#[cfg(target_os = "windows")]
fn strip_duplicate_prefixes_windows(devices: Vec<AudioDevice>) -> Vec<AudioDevice> {
    if devices.is_empty() {
        return devices;
    }

    fn extract_prefix(name: &str) -> Option<&str> {
        let paren_pos = name.find('(')?;
        let prefix = name[..paren_pos].trim();
        if prefix.is_empty() {
            return None;
        }
        Some(prefix)
    }

    fn extract_paren_content(name: &str) -> Option<&str> {
        let start = name.find('(')? + 1;
        let end = name.rfind(')')?;
        if start >= end {
            return None;
        }
        Some(name[start..end].trim())
    }

    // Count how many devices share each prefix (case-insensitive)
    let mut prefix_counts: HashMap<String, usize> = HashMap::new();
    for device in &devices {
        if let Some(prefix) = extract_prefix(&device.name) {
            *prefix_counts.entry(prefix.to_uppercase()).or_insert(0) += 1;
        }
    }

    devices
        .into_iter()
        .map(|mut device| {
            if let Some(prefix) = extract_prefix(&device.name) {
                let count = prefix_counts
                    .get(&prefix.to_uppercase())
                    .copied()
                    .unwrap_or(0);
                if count > 1 {
                    if let Some(content) = extract_paren_content(&device.name) {
                        log::debug!(
                            "[AUDIO] Stripping duplicate prefix '{}' from device '{}' -> '{}'",
                            prefix,
                            device.name,
                            content,
                        );
                        device.name = content.to_string();
                    }
                }
            }
            device
        })
        .collect()
}

/// Deduplicate Linux ALSA device nodes that map to the same physical hardware.
///
/// On ALSA, a single sound card (e.g. "HDA Intel PCH") appears as many device nodes:
///   - "front:CARD=PCH,DEV=0"
///   - "surround51:CARD=PCH,DEV=0"
///   - "surround71:CARD=PCH,DEV=0"
///   - "sysdefault:CARD=PCH"
///   - "default:CARD=PCH"
///   - "hw:CARD=PCH,DEV=0"
///   - "plughw:CARD=PCH,DEV=0"
///   - etc.
///
/// We group by the CARD name and keep the best representative (highest channel count,
/// widest sample rate range), merging supported configs and sample rates.
#[cfg(target_os = "linux")]
fn deduplicate_linux_devices(devices: Vec<AudioDevice>) -> Vec<AudioDevice> {
    use std::collections::BTreeMap;

    if devices.is_empty() {
        return devices;
    }

    // Extract the CARD name from ALSA device IDs/names like "front:CARD=PCH,DEV=0"
    fn extract_card_key(device: &AudioDevice) -> String {
        // Try the device_id first, then the name
        let source = device.device_id.as_deref().unwrap_or(&device.name);

        // Look for "CARD=<name>" pattern
        if let Some(card_start) = source.find("CARD=") {
            let after_card = &source[card_start + 5..];
            let card_name = after_card
                .split(|c: char| c == ',' || c == ' ' || c == ':')
                .next()
                .unwrap_or(after_card);
            if !card_name.is_empty() {
                return card_name.to_string();
            }
        }

        // For "hw:0,0" style, extract the card number
        if let Some(rest) = source.strip_prefix("hw:") {
            let card_num = rest
                .split(|c: char| c == ',' || c == ' ')
                .next()
                .unwrap_or(rest);
            if !card_num.is_empty() {
                return format!("hw:{}", card_num);
            }
        }

        // For "default", "pipewire", or other non-ALSA names, keep as-is
        device.name.clone()
    }

    // Group devices by card key, preserving insertion order
    let mut groups: BTreeMap<String, Vec<AudioDevice>> = BTreeMap::new();
    let mut key_order: Vec<String> = Vec::new();

    for device in devices {
        let key = extract_card_key(&device);
        if !groups.contains_key(&key) {
            key_order.push(key.clone());
        }
        groups.entry(key).or_default().push(device);
    }

    let mut result = Vec::new();

    for key in &key_order {
        let group = groups.remove(key).unwrap();

        if group.len() == 1 {
            result.push(group.into_iter().next().unwrap());
            continue;
        }

        // Pick the best representative: prefer default, then highest channel count
        let best_idx = group
            .iter()
            .enumerate()
            .max_by_key(|(_, d)| {
                let ch = d
                    .default_config
                    .as_ref()
                    .map(|c| c.channels as u32)
                    .unwrap_or(0);
                let is_default = if d.is_default { 1000u32 } else { 0 };
                is_default + ch
            })
            .map(|(i, _)| i)
            .unwrap_or(0);

        let mut merged = group[best_idx].clone();

        // Merge supported configs and sample rates from all variants
        let mut all_sample_rates = std::collections::HashSet::new();
        for rate in &merged.available_sample_rates {
            all_sample_rates.insert(*rate);
        }

        for (i, variant) in group.iter().enumerate() {
            if i == best_idx {
                continue;
            }

            // Merge sample rates
            for rate in &variant.available_sample_rates {
                all_sample_rates.insert(*rate);
            }

            // Merge supported configs (deduplicate)
            for cfg in &variant.supported_configs {
                let already_exists = merged.supported_configs.iter().any(|c| {
                    c.sample_rate == cfg.sample_rate
                        && c.channels == cfg.channels
                        && c.sample_format == cfg.sample_format
                });
                if !already_exists {
                    merged.supported_configs.push(cfg.clone());
                }
            }

            // Take the highest channel default config
            if let Some(ref variant_cfg) = variant.default_config {
                if let Some(ref merged_cfg) = merged.default_config {
                    if variant_cfg.channels > merged_cfg.channels {
                        merged.default_config = Some(variant_cfg.clone());
                    }
                } else {
                    merged.default_config = variant.default_config.clone();
                }
            }

            // Inherit is_default from any variant
            if variant.is_default {
                merged.is_default = true;
            }
        }

        let mut rates: Vec<u32> = all_sample_rates.into_iter().collect();
        rates.sort_unstable();
        merged.available_sample_rates = rates;

        let variant_count = group.len();
        log::info!(
            "[AUDIO] Grouped {} ALSA device nodes under '{}'",
            variant_count,
            merged.name,
        );

        result.push(merged);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_device_priority() {
        let devices = vec![
            ("id1".to_string(), "My Microphone".to_string()),
            ("id2".to_string(), "Built-in Microphone".to_string()),
            ("id3".to_string(), "Microphone (USB)".to_string()),
            ("id4".to_string(), "Speakers".to_string()),
        ];

        // 1. Exact ID
        assert_eq!(match_device_priority(&devices, "id2"), Some(1));

        // 2. Exact Name
        assert_eq!(match_device_priority(&devices, "Speakers"), Some(3));
        assert_eq!(match_device_priority(&devices, "speakers"), Some(3)); // Case insensitive

        // 3. Starts With
        // "Microphone" should match "Microphone (USB)" (idx 2) NOT "My Microphone" (idx 0) or "Built-in" (idx 1)
        // Wait, "Microphone" as exact match doesn't exist.
        // "Microphone (USB)" starts with "Microphone".
        // "My Microphone" contains "Microphone".
        // "Built-in Microphone" contains "Microphone".
        // The logic prioritizes Starts With.
        assert_eq!(match_device_priority(&devices, "Microphone"), Some(2));

        // 4. Contains
        assert_eq!(match_device_priority(&devices, "Built-in"), Some(1));

        // Edge case: "Micro"
        // "Microphone (USB)" starts with it -> idx 2
        assert_eq!(match_device_priority(&devices, "Micro"), Some(2));

        // Edge case: "USB"
        // "Microphone (USB)" contains it -> idx 2
        assert_eq!(match_device_priority(&devices, "USB"), Some(2));

        // Non-matching
        assert_eq!(match_device_priority(&devices, "Not Found"), None);
    }

    #[test]
    fn summarize_available_device_names_limits_long_lists() {
        let devices: Vec<(String, String)> = (0..20)
            .map(|i| (format!("id{i}"), format!("Device {i:02}")))
            .collect();

        let summary = summarize_available_device_names(&devices, 5);

        assert!(summary.contains("Device 00"));
        assert!(summary.contains("and 15 more"));
        assert!(!summary.contains("Device 19"));
    }

    #[test]
    fn test_probe_channel_order_prefers_requested_then_default() {
        assert_eq!(probe_channel_order(6, 2), vec![6, 2]);
    }

    #[test]
    fn test_probe_channel_order_deduplicates_matching_default() {
        assert_eq!(probe_channel_order(2, 2), vec![2]);
    }

    #[test]
    fn test_build_sample_rate_candidates_deduplicates_and_keeps_requested_first() {
        assert_eq!(
            build_sample_rate_candidates(44_100, Some(48_000)),
            vec![44_100, 48_000, 96_000, 192_000]
        );
        assert_eq!(
            build_sample_rate_candidates(88_200, Some(176_400)),
            vec![88_200, 48_000, 44_100, 96_000, 192_000, 176_400]
        );
    }

    #[test]
    fn test_filter_sample_rates_by_bounds_skips_unadvertised_rates() {
        let candidates = vec![44_100, 48_000, 88_200, 96_000, 192_000];
        let advertised = vec![(48_000, 96_000)];

        assert_eq!(
            filter_sample_rates_by_bounds(&candidates, &advertised),
            vec![48_000, 88_200, 96_000]
        );
    }

    fn make_device(name: &str) -> AudioDevice {
        AudioDevice {
            device_id: None,
            name: name.to_string(),
            display_info: None,
            is_input: false,
            is_default: false,
            supported_configs: vec![],
            default_config: None,
            available_sample_rates: vec![],
        }
    }

    #[test]
    fn test_strip_duplicate_prefixes_windows() {
        // Import the function for testing (it's cfg(windows) only, so we test the logic directly)
        fn extract_prefix(name: &str) -> Option<&str> {
            let paren_pos = name.find('(')?;
            let prefix = name[..paren_pos].trim();
            if prefix.is_empty() {
                return None;
            }
            Some(prefix)
        }

        fn extract_paren_content(name: &str) -> Option<&str> {
            let start = name.find('(')? + 1;
            let end = name.rfind(')')?;
            if start >= end {
                return None;
            }
            Some(name[start..end].trim())
        }

        // Test prefix extraction
        assert_eq!(extract_prefix("Speakers (RME Fireface)"), Some("Speakers"));
        assert_eq!(extract_prefix("SPEAKERS (RME Fireface)"), Some("SPEAKERS"));
        assert_eq!(extract_prefix("RME Fireface"), None);
        assert_eq!(extract_prefix("(RME Fireface)"), None);

        // Test paren content extraction
        assert_eq!(
            extract_paren_content("Speakers (RME Fireface UCX)"),
            Some("RME Fireface UCX")
        );
        assert_eq!(
            extract_paren_content("SPEAKERS (Realtek High Definition Audio)"),
            Some("Realtek High Definition Audio")
        );
        assert_eq!(extract_paren_content("No Parens"), None);
        assert_eq!(extract_paren_content("()"), None);

        // Test the full stripping logic inline
        let devices = vec![
            make_device("Speakers (RME Fireface UCX)"),
            make_device("Speakers (Realtek High Definition Audio)"),
            make_device("Microphone (RME Fireface UCX)"),
        ];

        // Count prefixes
        let mut prefix_counts: HashMap<String, usize> = HashMap::new();
        for device in &devices {
            if let Some(prefix) = extract_prefix(&device.name) {
                *prefix_counts.entry(prefix.to_uppercase()).or_insert(0) += 1;
            }
        }

        // "SPEAKERS" appears 2x, "MICROPHONE" appears 1x
        assert_eq!(prefix_counts.get("SPEAKERS"), Some(&2));
        assert_eq!(prefix_counts.get("MICROPHONE"), Some(&1));

        // Apply stripping
        let result: Vec<String> = devices
            .into_iter()
            .map(|mut device| {
                if let Some(prefix) = extract_prefix(&device.name) {
                    let count = prefix_counts
                        .get(&prefix.to_uppercase())
                        .copied()
                        .unwrap_or(0);
                    if count > 1
                        && let Some(content) = extract_paren_content(&device.name)
                    {
                        device.name = content.to_string();
                    }
                }
                device.name
            })
            .collect();

        assert_eq!(result[0], "RME Fireface UCX");
        assert_eq!(result[1], "Realtek High Definition Audio");
        assert_eq!(result[2], "Microphone (RME Fireface UCX)"); // Kept — only 1 "Microphone"
    }

    #[test]
    fn test_strip_duplicate_prefixes_case_insensitive() {
        fn extract_prefix(name: &str) -> Option<&str> {
            let paren_pos = name.find('(')?;
            let prefix = name[..paren_pos].trim();
            if prefix.is_empty() {
                return None;
            }
            Some(prefix)
        }

        // "Speakers" and "SPEAKERS" should be treated as the same prefix
        let devices = vec![
            make_device("Speakers (Device A)"),
            make_device("SPEAKERS (Device B)"),
        ];

        let mut prefix_counts: HashMap<String, usize> = HashMap::new();
        for device in &devices {
            if let Some(prefix) = extract_prefix(&device.name) {
                *prefix_counts.entry(prefix.to_uppercase()).or_insert(0) += 1;
            }
        }

        assert_eq!(prefix_counts.get("SPEAKERS"), Some(&2));
    }

    #[test]
    fn test_strip_duplicate_prefixes_single_device_unchanged() {
        fn extract_prefix(name: &str) -> Option<&str> {
            let paren_pos = name.find('(')?;
            let prefix = name[..paren_pos].trim();
            if prefix.is_empty() {
                return None;
            }
            Some(prefix)
        }

        fn extract_paren_content(name: &str) -> Option<&str> {
            let start = name.find('(')? + 1;
            let end = name.rfind(')')?;
            if start >= end {
                return None;
            }
            Some(name[start..end].trim())
        }

        // Single device with prefix — name should stay unchanged
        let devices = vec![make_device("Speakers (RME Fireface UCX)")];

        let mut prefix_counts: HashMap<String, usize> = HashMap::new();
        for device in &devices {
            if let Some(prefix) = extract_prefix(&device.name) {
                *prefix_counts.entry(prefix.to_uppercase()).or_insert(0) += 1;
            }
        }

        let result: Vec<String> = devices
            .into_iter()
            .map(|mut device| {
                if let Some(prefix) = extract_prefix(&device.name) {
                    let count = prefix_counts
                        .get(&prefix.to_uppercase())
                        .copied()
                        .unwrap_or(0);
                    if count > 1
                        && let Some(content) = extract_paren_content(&device.name)
                    {
                        device.name = content.to_string();
                    }
                }
                device.name
            })
            .collect();

        assert_eq!(result[0], "Speakers (RME Fireface UCX)");
    }

    #[test]
    fn test_is_asio_device() {
        assert!(is_asio_device("ASIO:Focusrite USB ASIO"));
        assert!(is_asio_device("ASIO:"));
        assert!(!is_asio_device("Focusrite USB ASIO"));
        assert!(!is_asio_device("Built-in Output"));
        assert!(!is_asio_device(""));
        assert!(!is_asio_device("ASI")); // too short
    }

    #[test]
    fn test_is_asio_device_case_insensitive() {
        assert!(is_asio_device("asio:Focusrite"));
        assert!(is_asio_device("Asio:Focusrite"));
        assert!(is_asio_device("aSiO:Focusrite"));
    }

    #[test]
    fn test_strip_asio_prefix() {
        assert_eq!(
            strip_asio_prefix("ASIO:Focusrite USB ASIO"),
            "Focusrite USB ASIO"
        );
        assert_eq!(strip_asio_prefix("ASIO:"), "");
        assert_eq!(strip_asio_prefix("Focusrite"), "Focusrite");
        assert_eq!(strip_asio_prefix(""), "");
    }

    #[test]
    fn test_strip_asio_prefix_case_insensitive() {
        assert_eq!(strip_asio_prefix("asio:MyDevice"), "MyDevice");
        assert_eq!(strip_asio_prefix("Asio:MyDevice"), "MyDevice");
    }

    #[test]
    fn test_get_host_for_device_default_without_asio_prefix() {
        // Without ASIO prefix, should return default host (never panics)
        let _host = get_host_for_device(None);
        let _host = get_host_for_device(Some("Built-in Output"));
        let _host = get_host_for_device(Some("Focusrite USB ASIO"));
    }

    #[test]
    fn test_list_asio_devices_returns_vec() {
        // On non-Windows or without ASIO feature, returns empty vec
        let devices = list_asio_devices();
        #[cfg(not(all(target_os = "windows", feature = "asio")))]
        assert!(devices.is_empty());
        let _ = devices;
    }
}
