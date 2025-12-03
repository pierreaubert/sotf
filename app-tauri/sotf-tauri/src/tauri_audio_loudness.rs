// ============================================================================
// Real-time Loudness Monitoring Commands
// ============================================================================

use sotf_audio::{AudioEngineManager, LoudnessInfo};
use tauri::State;
use tokio::sync::Mutex;

#[tauri::command]
pub async fn stream_enable_loudness_monitoring(
    streaming_manager: State<'_, Mutex<AudioEngineManager>>,
) -> Result<(), String> {
    println!("[TAURI] Enabling real-time loudness monitoring");

    let mut manager = streaming_manager.lock().await;
    manager.enable_loudness_monitoring()
}

#[tauri::command]
pub async fn stream_disable_loudness_monitoring(
    streaming_manager: State<'_, Mutex<AudioEngineManager>>,
) -> Result<(), String> {
    println!("[TAURI] Disabling real-time loudness monitoring");

    let mut manager = streaming_manager.lock().await;
    manager.disable_loudness_monitoring();
    Ok(())
}

#[tauri::command]
pub async fn stream_get_loudness(
    streaming_manager: State<'_, Mutex<AudioEngineManager>>,
) -> Result<Option<LoudnessInfo>, String> {
    let manager = streaming_manager.lock().await;
    Ok(manager.get_loudness())
}
