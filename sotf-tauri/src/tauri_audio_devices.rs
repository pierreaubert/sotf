// ============================================================================
// Audio Device Management Commands (Tauri wrappers for backend functions)
// ============================================================================

use sotf_audio::SharedAudioState;
use sotf_audio::devices::{AudioConfig, AudioDevice};
use tauri::State;

#[tauri::command]
pub async fn get_audio_devices()
-> Result<std::collections::HashMap<String, Vec<AudioDevice>>, String> {
    sotf_audio::devices::get_audio_devices()
}

#[tauri::command]
pub async fn set_audio_device(
    device_name: String,
    is_input: bool,
    config: AudioConfig,
    audio_state: State<'_, SharedAudioState>,
) -> Result<String, String> {
    sotf_audio::devices::set_audio_device(device_name, is_input, config, &*audio_state)
}

#[tauri::command]
pub async fn get_audio_config(
    audio_state: State<'_, SharedAudioState>,
) -> Result<sotf_audio::devices::AudioState, String> {
    sotf_audio::devices::get_audio_config(&*audio_state)
}

#[tauri::command]
pub async fn get_device_properties(
    device_name: String,
    is_input: bool,
) -> Result<serde_json::Value, String> {
    sotf_audio::devices::get_device_properties(device_name, is_input)
}
