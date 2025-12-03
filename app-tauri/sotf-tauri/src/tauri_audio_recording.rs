// ============================================================================
// Audio Recording Commands (using AudioManager)
// ============================================================================
// LEGACY: AudioManager has been removed as part of CamillaDSP phase-out
// This module is kept for the AudioError type used by tauri_audio_streaming
// but all recording commands are disabled

// use sotf_audio::AudioManager;
// use std::path::PathBuf;
// use tauri::{AppHandle, Emitter, State};
// use tokio::sync::Mutex;

// LEGACY: Unused struct (kept for reference)
// /// Audio state change event payload (for recording)
// #[derive(Clone, serde::Serialize)]
// struct AudioStateChanged {
//     state: String,
//     file: Option<String>,
//     output_device: Option<String>,
//     input_device: Option<String>,
// }

/// Audio error event payload
#[derive(Clone, serde::Serialize)]
pub struct AudioError {
    pub error: String,
}

// LEGACY: AudioManager commands disabled
// #[tauri::command]
// pub async fn audio_start_recording(
//     output_path: String,
//     input_device: Option<String>,
//     sample_rate: u32,
//     channels: u16,
//     audio_manager: State<'_, Mutex<AudioManager>>,
//     app_handle: AppHandle,
// ) -> Result<(), String> {
//     println!(
//         "[TAURI] Starting recording: {} ({}Hz, {}ch)",
//         output_path, sample_rate, channels
//     );
//
//     let manager = audio_manager.lock().await;
//     let result = manager
//         .start_recording(
//             input_device.clone(),
//             PathBuf::from(&output_path),
//             sample_rate,
//             channels,
//             None,
//         )
//         .await;
//
//     match result {
//         Ok(_) => {
//             // Emit state change event
//             let _ = app_handle.emit(
//                 "audio:state-changed",
//                 AudioStateChanged {
//                     state: "recording".to_string(),
//                     file: Some(output_path),
//                     output_device: None,
//                     input_device,
//                 },
//             );
//             Ok(())
//         }
//         Err(e) => {
//             // Emit error event
//             let _ = app_handle.emit(
//                 "audio:error",
//                 AudioError {
//                     error: e.to_string(),
//                 },
//             );
//             Err(format!("{}", e))
//         }
//     }
// }

// #[tauri::command]
// pub async fn audio_stop_recording(
//     audio_manager: State<'_, Mutex<AudioManager>>,
//     app_handle: AppHandle,
// ) -> Result<(), String> {
//     println!("[TAURI] Stopping recording");
//
//     let manager = audio_manager.lock().await;
//     let result = manager.stop_recording().await;
//
//     match result {
//         Ok(_) => {
//             // Emit state change event
//             let _ = app_handle.emit(
//                 "audio:state-changed",
//                 AudioStateChanged {
//                     state: "idle".to_string(),
//                     file: None,
//                     output_device: None,
//                     input_device: None,
//                 },
//             );
//             Ok(())
//         }
//         Err(e) => Err(format!("{}", e)),
//     }
// }

// #[tauri::command]
// pub async fn audio_get_signal_peak(
//     audio_manager: State<'_, Mutex<AudioManager>>,
// ) -> Result<f32, String> {
//     let manager = audio_manager.lock().await;
//     manager
//         .get_signal_peak()
//         .await
//         .map_err(|e| format!("{}", e))
// }
//
// #[tauri::command]
// pub async fn audio_get_recording_spl(
//     audio_manager: State<'_, Mutex<AudioManager>>,
// ) -> Result<f32, String> {
//     let manager = audio_manager.lock().await;
//
//     // Check if we're recording
//     if !manager.is_recording().map_err(|e| format!("{}", e))? {
//         return Err("Not currently recording".to_string());
//     }
//
//     // Get signal peak and convert to dB SPL
//     let peak = manager
//         .get_signal_peak()
//         .await
//         .map_err(|e| format!("{}", e))?;
//
//     // Convert peak (0.0 to 1.0+) to dB SPL
//     // 0 dBFS = 94 dB SPL (standard calibration for digital audio)
//     // dB = 20 * log10(value)
//     let db_fs = if peak > 0.0 {
//         20.0 * peak.log10()
//     } else {
//         -96.0 // Silence floor
//     };
//
//     // Convert dBFS to dB SPL (assuming 0 dBFS = 94 dB SPL)
//     let db_spl = 94.0 + db_fs;
//
//     Ok(db_spl)
// }
