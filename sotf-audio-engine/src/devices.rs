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

/// Extract device info from cpal device using description() and id()
fn get_device_info<D: DeviceTrait>(device: &D) -> Option<(String, Option<String>, Option<String>)> {
    // Get display name from description (preferred) or fallback to name()
    let (name, display_info) = if let Ok(desc) = device.description() {
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
        (name, display_info)
    } else if let Ok(name) = device.name() {
        (name, None)
    } else {
        return None;
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
                        let rate_range = if available_sample_rates.is_empty() {
                            "unknown".to_string()
                        } else if available_sample_rates.len() == 1 {
                            format!("{} Hz", available_sample_rates[0])
                        } else {
                            format!(
                                "{}-{} Hz",
                                available_sample_rates.first().unwrap(),
                                available_sample_rates.last().unwrap()
                            )
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
                    let (default_config, available_sample_rates) =
                        if let Ok(configs_iter) = device.supported_output_configs() {
                            let configs: Vec<_> = configs_iter.collect();

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
                        let rate_range = if available_sample_rates.is_empty() {
                            "unknown".to_string()
                        } else if available_sample_rates.len() == 1 {
                            format!("{} Hz", available_sample_rates[0])
                        } else {
                            format!(
                                "{}-{} Hz",
                                available_sample_rates.first().unwrap(),
                                available_sample_rates.last().unwrap()
                            )
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

/// Check if a device matches the given identifier (ID preferred, name fallback)
fn device_matches<D: DeviceTrait>(device: &D, identifier: &str) -> bool {
    // First try to match by device ID (preferred for persistence)
    if let Ok(id) = device.id() {
        if id.to_string() == identifier {
            return true;
        }
    }
    // Fallback to name matching for legacy saved states
    if let Ok(desc) = device.description() {
        if desc.name() == identifier {
            return true;
        }
    }
    // Last resort: deprecated name() method
    if let Ok(name) = device.name() {
        if name == identifier {
            return true;
        }
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
        .or_else(|_| device.name())
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
