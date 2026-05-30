//! Audio device facade for iOS.
//!
//! Provides the same data types as the desktop `devices` module (AudioDevice,
//! AudioConfig, AudioState, SharedAudioState) without depending on cpal.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Represents information about an audio device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub device_id: Option<String>,
    pub name: String,
    pub display_info: Option<String>,
    pub is_input: bool,
    pub is_default: bool,
    pub supported_configs: Vec<AudioConfig>,
    pub default_config: Option<AudioConfig>,
    pub available_sample_rates: Vec<u32>,
}

/// Represents audio configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_size: Option<u32>,
    pub sample_format: String,
}

/// State for storing the currently selected audio configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AudioState {
    pub selected_input_device: Option<String>,
    pub selected_output_device: Option<String>,
    pub input_config: Option<AudioConfig>,
    pub output_config: Option<AudioConfig>,
}

pub type SharedAudioState = Arc<Mutex<AudioState>>;

pub const IOS_SYSTEM_OUTPUT_ID: &str = "ios-system-output";
pub const IOS_SYSTEM_OUTPUT_NAME: &str = "System Output";
const IOS_SAMPLE_RATES: &[u32] = &[44_100, 48_000];

pub const ASIO_DEVICE_PREFIX: &str = "ASIO:";

pub fn is_asio_device(identifier: &str) -> bool {
    identifier.len() >= ASIO_DEVICE_PREFIX.len()
        && identifier[..ASIO_DEVICE_PREFIX.len()].eq_ignore_ascii_case(ASIO_DEVICE_PREFIX)
}

pub fn strip_asio_prefix(identifier: &str) -> &str {
    if is_asio_device(identifier) {
        &identifier[ASIO_DEVICE_PREFIX.len()..]
    } else {
        identifier
    }
}

pub fn list_asio_devices() -> Vec<String> {
    Vec::new()
}

fn ios_default_output_config() -> AudioConfig {
    AudioConfig {
        sample_rate: 48_000,
        channels: 2,
        buffer_size: None,
        sample_format: "f32".to_string(),
    }
}

fn matches_ios_output(identifier: &str) -> bool {
    identifier == IOS_SYSTEM_OUTPUT_ID || identifier.eq_ignore_ascii_case(IOS_SYSTEM_OUTPUT_NAME)
}

fn ios_config_supported(config: &AudioConfig) -> bool {
    IOS_SAMPLE_RATES.contains(&config.sample_rate)
        && (1..=2).contains(&config.channels)
        && config.sample_format == "f32"
}

/// iOS exposes the app's system output route as a single selectable device.
pub fn get_audio_devices() -> Result<HashMap<String, Vec<AudioDevice>>, String> {
    let mut devices_map = HashMap::new();
    devices_map.insert("input".to_string(), Vec::new());
    devices_map.insert(
        "output".to_string(),
        vec![AudioDevice {
            device_id: Some(IOS_SYSTEM_OUTPUT_ID.to_string()),
            name: IOS_SYSTEM_OUTPUT_NAME.to_string(),
            display_info: Some("iOS Audio".to_string()),
            is_input: false,
            is_default: true,
            supported_configs: vec![ios_default_output_config()],
            default_config: Some(ios_default_output_config()),
            available_sample_rates: IOS_SAMPLE_RATES.to_vec(),
        }],
    );
    Ok(devices_map)
}

pub fn get_device_supported_sample_rates(device_identifier: Option<&str>) -> Option<Vec<u32>> {
    if device_identifier.is_none_or(matches_ios_output) {
        Some(IOS_SAMPLE_RATES.to_vec())
    } else {
        None
    }
}

pub fn get_device_current_sample_rate(device_identifier: Option<&str>) -> Option<u32> {
    if device_identifier.is_none_or(matches_ios_output) {
        Some(48_000)
    } else {
        None
    }
}

pub fn verify_working_sample_rate(
    device_identifier: Option<&str>,
    requested_rate: u32,
    requested_channels: usize,
) -> Option<u32> {
    if device_identifier.is_some_and(|identifier| !matches_ios_output(identifier)) {
        return None;
    }
    if requested_channels == 0 || requested_channels > 2 {
        return None;
    }
    if IOS_SAMPLE_RATES.contains(&requested_rate) {
        Some(requested_rate)
    } else {
        Some(48_000)
    }
}

pub fn is_null_device(_name: &str) -> bool {
    false
}

pub fn set_audio_device(
    device_identifier: String,
    is_input: bool,
    config: AudioConfig,
    audio_state: &SharedAudioState,
) -> Result<String, String> {
    if is_input {
        return Err("iOS input device selection is managed by AVAudioSession".to_string());
    }
    if !matches_ios_output(&device_identifier) {
        return Err(format!("Device '{}' not found", device_identifier));
    }
    if !ios_config_supported(&config) {
        return Err(format!(
            "Configuration not supported by iOS output '{}': sample_rate={}, channels={}, format={}",
            device_identifier, config.sample_rate, config.channels, config.sample_format
        ));
    }

    let mut state = audio_state
        .lock()
        .map_err(|e| format!("Failed to lock audio state: {}", e))?;
    state.selected_output_device = Some(IOS_SYSTEM_OUTPUT_ID.to_string());
    state.output_config = Some(config.clone());

    Ok(format!(
        "Successfully configured output device '{}' with sample_rate={}, channels={}, format={}",
        device_identifier, config.sample_rate, config.channels, config.sample_format
    ))
}

pub fn get_audio_config(audio_state: &SharedAudioState) -> Result<AudioState, String> {
    let state = audio_state
        .lock()
        .map_err(|e| format!("Failed to lock audio state: {}", e))?;
    Ok(state.clone())
}

pub fn get_device_properties(
    device_identifier: String,
    is_input: bool,
) -> Result<serde_json::Value, String> {
    if is_input {
        return Err("iOS input device properties are managed by AVAudioSession".to_string());
    }
    if !matches_ios_output(&device_identifier) {
        return Err(format!("Device '{}' not found", device_identifier));
    }

    Ok(serde_json::json!({
        "id": IOS_SYSTEM_OUTPUT_ID,
        "name": IOS_SYSTEM_OUTPUT_NAME,
        "type": "output",
        "sample_rates": IOS_SAMPLE_RATES,
        "channels": 2,
        "sample_format": "f32",
    }))
}
