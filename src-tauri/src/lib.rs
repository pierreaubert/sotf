use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

// Import audio streaming types
// use sotf_audio::AudioManager;  // LEGACY: AudioManager has been removed (part of CamillaDSP phase-out)
use sotf_audio::AudioStreamingManager;
use sotf_audio::SharedAudioState;

// Declare modules
mod tauri_audio_capture; // NEW: Audio capture with test signals
mod tauri_audio_devices;
mod tauri_audio_loudness;
mod tauri_audio_recording; // NOTE: Module kept for AudioError type, but commands disabled
mod tauri_audio_replaygain;
mod tauri_audio_spectrum;
mod tauri_audio_streaming;
mod tauri_compute_eq;
mod tauri_generate_eq;
mod tauri_optim;
mod tauri_plots;
mod tauri_roomeq; // NEW: Multi-channel room EQ optimization
mod tauri_speakers;

pub use tauri_speakers::{get_speaker_measurements, get_speaker_versions, get_speakers};

pub use tauri_optim::{CancellationState, cancel_optimization, run_optimization};

pub use tauri_roomeq::{cancel_roomeq_optimization, run_roomeq_optimization};

pub use tauri_plots::{
    generate_peq_plots, generate_plot_filters, generate_plot_spin, generate_plot_spin_details,
    generate_plot_spin_tonal,
};

pub use tauri_audio_devices::{
    get_audio_config, get_audio_devices, get_device_properties, set_audio_device,
};

pub use sotf_audio::StreamingState;
pub use tauri_audio_streaming::{
    stream_get_file_info, stream_get_state, stream_load_file, stream_pause_playback,
    stream_resume_playback, stream_seek, stream_start_playback, stream_stop_playback,
    stream_update_filters,
};

pub use tauri_audio_loudness::{
    stream_disable_loudness_monitoring, stream_enable_loudness_monitoring, stream_get_loudness,
};

pub use tauri_audio_spectrum::{
    stream_disable_spectrum_monitoring, stream_enable_spectrum_monitoring, stream_get_spectrum,
};

pub use tauri_audio_replaygain::analyze_replaygain;

pub use tauri_compute_eq::compute_eq_response;

pub use tauri_generate_eq::{
    generate_apo_format, generate_aupreset_format, generate_rme_format, generate_rme_room_format,
};

pub use tauri_audio_capture::{load_recordings_zip, record_channel, save_recordings_zip};

#[tauri::command]
fn exit_app(window: tauri::Window) {
    window.close().unwrap();
}

#[tauri::command]
fn resolve_demo_track_path(relative_path: String) -> Result<String, String> {
    // relative_path is like "/demo-audio/classical.flac"
    // Remove leading slash if present
    let path = relative_path.trim_start_matches('/');

    #[cfg(debug_assertions)]
    {
        // In dev mode, construct absolute path from current working directory
        // When running with cargo/tauri dev, CWD is already in src-tauri/
        let cwd = std::env::current_dir()
            .map_err(|e| format!("Failed to get current directory: {}", e))?;

        println!("[TAURI] Current working directory: {:?}", cwd);

        // Check if we're already in src-tauri/ directory
        let demo_path = if cwd.ends_with("src-tauri") {
            // Already in src-tauri/, so just use public/
            cwd.join("public").join(path)
        } else {
            // At project root, need to go into src-tauri/public/
            cwd.join("src-tauri").join("public").join(path)
        };

        println!("[TAURI] Looking for demo track at: {:?}", demo_path);

        // Check if file exists
        if !demo_path.exists() {
            return Err(format!("Demo track not found at: {:?}", demo_path));
        }

        Ok(demo_path.to_string_lossy().to_string())
    }

    #[cfg(not(debug_assertions))]
    {
        // In production, demo files should be bundled as resources
        // For now, try the same approach as dev mode
        let cwd = std::env::current_dir()
            .map_err(|e| format!("Failed to get current directory: {}", e))?;

        let demo_path = if cwd.ends_with("src-tauri") {
            cwd.join("public").join(path)
        } else {
            cwd.join("src-tauri").join("public").join(path)
        };

        if !demo_path.exists() {
            return Err(format!("Demo track not found at: {:?}", demo_path));
        }

        Ok(demo_path.to_string_lossy().to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // LEGACY: CamillaDSP code commented out (AudioManager removed as part of phase-out)
    // // Find CamillaDSP binary
    // let camilla_binary = sotf_audio::camilla::find_camilladsp_binary().unwrap_or_else(|e| {
    //     eprintln!("[TAURI] Warning: CamillaDSP binary not found: {}", e);
    //     eprintln!("[TAURI] Audio playback features will not be available.");
    //     std::path::PathBuf::from("/usr/local/bin/camilladsp")
    // });
    //
    // // Create AudioManager (wrapped in Mutex for Tauri state)
    // let audio_manager = Mutex::new(AudioManager::new(camilla_binary.clone()));

    // Create AudioStreamingManager for all audio format playback (WAV, FLAC, MP3, etc.)
    // NOTE: No longer requires CamillaDSP - uses native AudioEngine instead
    let streaming_manager = Mutex::new(AudioStreamingManager::new());

    let mut builder = tauri::Builder::default();

    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(tauri_plugin_devtools::init());
    }

    builder
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(CancellationState::new())
        .manage(SharedAudioState::default())
        // .manage(audio_manager)  // LEGACY: AudioManager removed
        .manage(streaming_manager)
        .setup(|app| {
            // Spawn background task to monitor streaming events and forward them to frontend
            let app_handle = app.handle().clone();

            tauri::async_runtime::spawn(async move {
                loop {
                    // Poll for events from the decoder thread
                    let streaming_mgr = app_handle.state::<Mutex<AudioStreamingManager>>();
                    if let Ok(manager) = streaming_mgr.try_lock() {
                        for event in manager.drain_events() {
                            let _ = match event {
                                sotf_audio::StreamingEvent::EndOfStream => {
                                    println!("[EVENT MONITOR] End of stream detected");
                                    app_handle.emit(
                                        "stream:state-changed",
                                        serde_json::json!({ "state": "ended" }),
                                    )
                                }
                                sotf_audio::StreamingEvent::StateChanged(state) => {
                                    let state_str = match state {
                                        StreamingState::Idle => "idle",
                                        StreamingState::Loading => "loading",
                                        StreamingState::Ready => "ready",
                                        StreamingState::Playing => "playing",
                                        StreamingState::Paused => "paused",
                                        StreamingState::Seeking => "seeking",
                                        StreamingState::Error => "error",
                                    };
                                    println!("[TAURI] State changed to: {}", state_str);
                                    app_handle.emit(
                                        "stream:state-changed",
                                        serde_json::json!({ "state": state_str }),
                                    )
                                }
                                sotf_audio::StreamingEvent::Error(msg) => {
                                    println!("[TAURI] Error: {}", msg);
                                    app_handle.emit(
                                        "stream:state-changed",
                                        serde_json::json!({ "state": "error", "message": msg }),
                                    )
                                }
                            };
                        }
                    }

                    // Sleep to avoid busy-waiting
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            run_optimization,
            cancel_optimization,
            run_roomeq_optimization,
            cancel_roomeq_optimization,
            get_speakers,
            get_speaker_versions,
            get_speaker_measurements,
            generate_plot_filters,
            generate_plot_spin,
            generate_plot_spin_details,
            generate_plot_spin_tonal,
            generate_peq_plots,
            exit_app,
            resolve_demo_track_path,
            get_audio_devices,
            set_audio_device,
            get_audio_config,
            get_device_properties,
            // LEGACY: Audio recording commands disabled (AudioManager removed)
            // audio_start_recording,
            // audio_stop_recording,
            // audio_get_signal_peak,
            // audio_get_recording_spl,
            // Streaming playback commands (supports all audio formats)
            stream_load_file,
            stream_start_playback,
            stream_pause_playback,
            stream_resume_playback,
            stream_stop_playback,
            stream_seek,
            stream_get_state,
            stream_get_file_info,
            stream_update_filters,
            stream_enable_loudness_monitoring,
            stream_disable_loudness_monitoring,
            stream_get_loudness,
            stream_enable_spectrum_monitoring,
            stream_disable_spectrum_monitoring,
            stream_get_spectrum,
            // Export format commands
            generate_apo_format,
            generate_aupreset_format,
            generate_rme_format,
            generate_rme_room_format,
            // ReplayGain analysis
            analyze_replaygain,
            // EQ response computation
            compute_eq_response,
            // Audio capture commands
            record_channel,
            save_recordings_zip,
            load_recordings_zip
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
