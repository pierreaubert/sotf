use sotf_audio::replaygain::{ReplayGainInfo, analyze_file};

#[tauri::command]
pub async fn analyze_replaygain(file_path: String) -> Result<ReplayGainInfo, String> {
    println!("[TAURI] Analyzing file: {}", file_path);

    match analyze_file(&file_path) {
        Ok(info) => {
            println!(
                "[TAURI] Analysis complete - Gain: {:.2} dB, Peak: {:.6}",
                info.gain, info.peak
            );
            Ok(info)
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            println!("[TAURI] Analysis failed: {}", error_msg);
            Err(error_msg)
        }
    }
}
