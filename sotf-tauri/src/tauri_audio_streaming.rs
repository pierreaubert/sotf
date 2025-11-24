// ============================================================================
// Streaming Audio Commands
// ============================================================================

use crate::tauri_audio_recording::AudioError;
use autoeq::iir::Biquad;
use sotf_audio::{
    AudioEngineManager, AudioFileInfo, LoudnessCompensation, PluginConfig, StreamingState,
};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

/// Simple filter parameters from frontend (without computed coefficients)
#[derive(Clone, serde::Deserialize)]
pub struct FilterParams {
    pub filter_type: String,
    pub freq: f64,
    pub q: f64,
    pub db_gain: f64,
}

/// Audio file information for the frontend
#[derive(Clone, serde::Serialize)]
pub struct AudioFileInfoPayload {
    path: String,
    format: String,
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    duration_seconds: Option<f64>,
}

fn convert_audio_file_info(info: &AudioFileInfo) -> AudioFileInfoPayload {
    AudioFileInfoPayload {
        path: info.path.to_string_lossy().to_string(),
        format: info.format.to_string(),
        sample_rate: info.spec.sample_rate,
        channels: info.spec.channels,
        bits_per_sample: info.spec.bits_per_sample,
        duration_seconds: info.duration_seconds,
    }
}

#[tauri::command]
pub async fn stream_load_file(
    file_path: String,
    streaming_manager: State<'_, Mutex<AudioEngineManager>>,
    app_handle: AppHandle,
) -> Result<AudioFileInfoPayload, String> {
    println!("[TAURI] Loading file for streaming: {}", file_path);

    let mut manager = streaming_manager.lock().await;
    match manager.load_file(&file_path) {
        Ok(audio_info) => {
            let payload = convert_audio_file_info(&audio_info);

            // Emit file loaded event
            let _ = app_handle.emit("stream:file-loaded", &payload);

            Ok(payload)
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            let _ = app_handle.emit(
                "stream:error",
                AudioError {
                    error: error_msg.clone(),
                },
            );
            Err(error_msg)
        }
    }
}

/// Helper to create EQ plugin config from biquad filters
fn create_eq_plugin_config(filters: &[Biquad]) -> Result<PluginConfig, String> {
    use autoeq::iir::BiquadFilterType;

    let filter_configs: Result<Vec<_>, String> = filters
        .iter()
        .map(|f| {
            // Use long_name() from BiquadFilterType
            let filter_type = match f.filter_type {
                BiquadFilterType::HighpassVariableQ => "highpass".to_string(),
                _ => f.filter_type.long_name().to_lowercase(),
            };

            Ok(serde_json::json!({
                "filter_type": filter_type,
                "freq": f.freq,
                "q": f.q,
                "db_gain": f.db_gain,
            }))
        })
        .collect();

    let filter_configs = filter_configs?;

    let parameters = serde_json::json!({
        "filters": filter_configs,
    });

    Ok(PluginConfig {
        plugin_type: "eq".to_string(),
        parameters,
    })
}

#[tauri::command]
pub async fn stream_start_playback(
    output_device: Option<String>,
    filters: Vec<FilterParams>,
    streaming_manager: State<'_, Mutex<AudioEngineManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!("[TAURI] Starting playback with {} filters", filters.len());

    // Get audio info to determine sample rate and output channels
    let manager = streaming_manager.lock().await;
    let audio_info = manager
        .get_audio_info()
        .ok_or_else(|| "No audio file loaded".to_string())?;
    let sample_rate = audio_info.spec.sample_rate as f64;
    let output_channels = audio_info.spec.channels as usize;
    drop(manager);

    // Convert FilterParams to Biquad with computed coefficients
    let biquads: Result<Vec<Biquad>, String> = filters
        .iter()
        .map(|f| {
            use autoeq::iir::BiquadFilterType;

            let filter_type = match f.filter_type.as_str() {
                "Peak" => BiquadFilterType::Peak,
                "Lowpass" => BiquadFilterType::Lowpass,
                "Highpass" => BiquadFilterType::Highpass,
                "Lowshelf" => BiquadFilterType::Lowshelf,
                "Highshelf" => BiquadFilterType::Highshelf,
                "Notch" => BiquadFilterType::Notch,
                "Bandpass" => BiquadFilterType::Bandpass,
                _ => return Err(format!("Unknown filter type: {}", f.filter_type)),
            };

            Ok(Biquad::new(
                filter_type,
                sample_rate,
                f.freq,
                f.q,
                f.db_gain,
            ))
        })
        .collect();

    let biquads = biquads?;

    // Build plugin chain
    let mut plugins = Vec::new();

    // Add EQ plugin if filters are present
    if !biquads.is_empty() {
        plugins.push(create_eq_plugin_config(&biquads)?);
    }

    // Start playback
    let mut manager = streaming_manager.lock().await;
    match manager.start_playback(output_device.clone(), plugins, output_channels) {
        Ok(_) => {
            // Emit state change event
            let _ = app_handle.emit(
                "stream:state-changed",
                serde_json::json!({
                    "state": "playing",
                    "output_device": output_device,
                }),
            );
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            let _ = app_handle.emit(
                "stream:error",
                AudioError {
                    error: error_msg.clone(),
                },
            );
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub async fn stream_pause_playback(
    streaming_manager: State<'_, Mutex<AudioEngineManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!("[TAURI] Pausing playback");

    let manager = streaming_manager.lock().await;
    match manager.pause() {
        Ok(_) => {
            let _ = app_handle.emit(
                "stream:state-changed",
                serde_json::json!({
                    "state": "paused",
                }),
            );
            Ok(())
        }
        Err(e) => Err(format!("{}", e)),
    }
}

#[tauri::command]
pub async fn stream_resume_playback(
    streaming_manager: State<'_, Mutex<AudioEngineManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!("[TAURI] Resuming playback");

    let manager = streaming_manager.lock().await;
    match manager.resume() {
        Ok(_) => {
            let _ = app_handle.emit(
                "stream:state-changed",
                serde_json::json!({
                    "state": "playing",
                }),
            );
            Ok(())
        }
        Err(e) => Err(format!("{}", e)),
    }
}

#[tauri::command]
pub async fn stream_stop_playback(
    streaming_manager: State<'_, Mutex<AudioEngineManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!("[TAURI] Stopping playback");

    let mut manager = streaming_manager.lock().await;
    match manager.stop() {
        Ok(_) => {
            let _ = app_handle.emit(
                "stream:state-changed",
                serde_json::json!({
                    "state": "idle",
                }),
            );
            Ok(())
        }
        Err(e) => Err(format!("{}", e)),
    }
}

#[tauri::command]
pub async fn stream_seek(
    seconds: f64,
    streaming_manager: State<'_, Mutex<AudioEngineManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!("[TAURI] Seeking to {}s", seconds);

    let manager = streaming_manager.lock().await;
    match manager.seek(seconds) {
        Ok(_) => {
            let _ = app_handle.emit(
                "stream:position-changed",
                serde_json::json!({
                    "position_seconds": seconds,
                }),
            );
            Ok(())
        }
        Err(e) => Err(format!("{}", e)),
    }
}

#[tauri::command]
pub async fn stream_get_state(
    streaming_manager: State<'_, Mutex<AudioEngineManager>>,
) -> Result<String, String> {
    let manager = streaming_manager.lock().await;
    let state = manager.get_state();

    let state_str = match state {
        StreamingState::Idle => "idle",
        StreamingState::Loading => "loading",
        StreamingState::Ready => "ready",
        StreamingState::Playing => "playing",
        StreamingState::Paused => "paused",
        StreamingState::Seeking => "seeking",
        StreamingState::Error => "error",
    };

    Ok(state_str.to_string())
}

#[tauri::command]
pub async fn stream_get_file_info(
    streaming_manager: State<'_, Mutex<AudioEngineManager>>,
) -> Result<Option<AudioFileInfoPayload>, String> {
    let manager = streaming_manager.lock().await;
    match manager.get_audio_info() {
        Some(info) => Ok(Some(convert_audio_file_info(&info))),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn stream_update_filters(
    filters: Vec<FilterParams>,
    _loudness: Option<LoudnessCompensation>,
    streaming_manager: State<'_, Mutex<AudioEngineManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!("[TAURI] Updating filters: {} filters", filters.len());

    // Get audio info to determine sample rate
    let manager = streaming_manager.lock().await;
    let audio_info = manager
        .get_audio_info()
        .ok_or_else(|| "No audio file loaded".to_string())?;
    let sample_rate = audio_info.spec.sample_rate as f64;
    drop(manager);

    // Convert FilterParams to Biquad with computed coefficients
    let biquads: Result<Vec<Biquad>, String> = filters
        .iter()
        .map(|f| {
            use autoeq::iir::BiquadFilterType;

            let filter_type = match f.filter_type.as_str() {
                "Peak" => BiquadFilterType::Peak,
                "Lowpass" => BiquadFilterType::Lowpass,
                "Highpass" => BiquadFilterType::Highpass,
                "Lowshelf" => BiquadFilterType::Lowshelf,
                "Highshelf" => BiquadFilterType::Highshelf,
                "Notch" => BiquadFilterType::Notch,
                "Bandpass" => BiquadFilterType::Bandpass,
                _ => return Err(format!("Unknown filter type: {}", f.filter_type)),
            };

            Ok(Biquad::new(
                filter_type,
                sample_rate,
                f.freq,
                f.q,
                f.db_gain,
            ))
        })
        .collect();

    let biquads = biquads?;

    // Build plugin chain
    let mut plugins = Vec::new();

    // Add EQ plugin if filters are present
    if !biquads.is_empty() {
        plugins.push(create_eq_plugin_config(&biquads)?);
    }

    // Update plugin chain
    let manager = streaming_manager.lock().await;
    match manager.update_plugin_chain(plugins) {
        Ok(_) => {
            let _ = app_handle.emit(
                "stream:filters-updated",
                serde_json::json!({
                    "ok": true,
                }),
            );
            println!("[TAURI] Filters updated successfully");
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to update filters: {}", e);
            let _ = app_handle.emit(
                "stream:error",
                AudioError {
                    error: error_msg.clone(),
                },
            );
            Err(error_msg)
        }
    }

    /*
    let manager = streaming_manager.lock().await;
    match manager.update_filters(filters, loudness).await {
        Ok(_) => {
            let _ = app_handle.emit(
                "stream:filters-updated",
                serde_json::json!({
                    "ok": true,
                }),
            );
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            let _ = app_handle.emit(
                "stream:filters-updated",
                serde_json::json!({
                    "ok": false,
                    "error": error_msg.clone(),
                }),
            );
            Err(error_msg)
        }
    }
    */
}
