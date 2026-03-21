//! Audio devices stub for iOS.
//!
//! Provides the same data types as the desktop `devices` module (AudioDevice,
//! AudioConfig, AudioState, SharedAudioState) but with stub implementations
//! for functions that require cpal.

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

/// iOS stub: returns a single "System Output" device
pub fn get_audio_devices() -> Result<HashMap<String, Vec<AudioDevice>>, String> {
    let mut devices_map = HashMap::new();
    devices_map.insert("input".to_string(), Vec::new());
    devices_map.insert(
        "output".to_string(),
        vec![AudioDevice {
            device_id: Some("ios-system-output".to_string()),
            name: "System Output".to_string(),
            display_info: Some("iOS Audio".to_string()),
            is_input: false,
            is_default: true,
            supported_configs: vec![AudioConfig {
                sample_rate: 48000,
                channels: 2,
                buffer_size: None,
                sample_format: "f32".to_string(),
            }],
            default_config: Some(AudioConfig {
                sample_rate: 48000,
                channels: 2,
                buffer_size: None,
                sample_format: "f32".to_string(),
            }),
            available_sample_rates: vec![44100, 48000],
        }],
    );
    Ok(devices_map)
}

pub fn get_device_supported_sample_rates(_device_identifier: Option<&str>) -> Option<Vec<u32>> {
    Some(vec![44100, 48000])
}

pub fn get_device_current_sample_rate(_device_identifier: Option<&str>) -> Option<u32> {
    Some(48000)
}

pub fn verify_working_sample_rate(
    _device_identifier: Option<&str>,
    requested_rate: u32,
    _requested_channels: usize,
) -> Option<u32> {
    Some(requested_rate)
}

pub fn is_null_device(_name: &str) -> bool {
    false
}

pub fn set_audio_device(
    _device_identifier: String,
    _is_input: bool,
    _config: AudioConfig,
    _audio_state: &SharedAudioState,
) -> Result<String, String> {
    Err("Audio device selection not available on iOS".to_string())
}

pub fn get_audio_config(audio_state: &SharedAudioState) -> Result<AudioState, String> {
    let state = audio_state
        .lock()
        .map_err(|e| format!("Failed to lock audio state: {}", e))?;
    Ok(state.clone())
}

pub fn get_device_properties(
    _device_identifier: String,
    _is_input: bool,
) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "name": "System Output",
        "type": "output",
    }))
}
