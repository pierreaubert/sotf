// ============================================================================
// Real-time Spectrum Analysis Commands
// ============================================================================

use sotf_audio::AudioEngineManager;
use tauri::State;
use tokio::sync::Mutex;

#[tauri::command]
pub async fn stream_enable_spectrum_monitoring(
    streaming_manager: State<'_, Mutex<AudioEngineManager>>,
) -> Result<(), String> {
    println!("[TAURI] Enabling real-time spectrum monitoring");

    let mut manager = streaming_manager.lock().await;
    manager.enable_spectrum_monitoring()
}

#[tauri::command]
pub async fn stream_disable_spectrum_monitoring(
    streaming_manager: State<'_, Mutex<AudioEngineManager>>,
) -> Result<(), String> {
    println!("[TAURI] Disabling real-time spectrum monitoring");

    let mut manager = streaming_manager.lock().await;
    manager.disable_spectrum_monitoring();
    Ok(())
}

#[tauri::command]
pub async fn stream_get_spectrum(
    streaming_manager: State<'_, Mutex<AudioEngineManager>>,
) -> Result<Option<sotf_audio::SpectrumInfo>, String> {
    let manager = streaming_manager.lock().await;
    Ok(manager.get_spectrum())
}
