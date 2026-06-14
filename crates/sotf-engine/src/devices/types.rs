use super::device::device_matches;
use super::misc::format_to_string;
use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};
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
pub(super) fn deduplicate_linux_devices(
    devices: Vec<AudioDevice>,
) -> Result<Vec<AudioDevice>, String> {
    use std::collections::BTreeMap;

    if devices.is_empty() {
        return Ok(devices);
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

    merge_linux_device_groups(&key_order, groups)
}

#[cfg(target_os = "linux")]
pub(crate) fn merge_linux_device_groups(
    key_order: &[String],
    mut groups: std::collections::BTreeMap<String, Vec<AudioDevice>>,
) -> Result<Vec<AudioDevice>, String> {
    let mut result = Vec::new();

    for key in key_order {
        let group = groups
            .remove(key)
            .ok_or("internal error: group key missing")?;

        if group.len() <= 1 {
            if let Some(device) = group.into_iter().next() {
                result.push(device);
            } else {
                return Err("internal error: empty group".to_string());
            }
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

    Ok(result)
}
