use super::consts::GLOBAL_PLAYER;
use super::misc::ffi_guard;
use super::pending::pending_imports;
use super::pending::pending_qr_payloads;
use super::pending::pending_queue;
use super::remote_command::push_remote_command;
use super::types::RemoteCommand;
use sotf_audio_player::Player;

#[cfg(any(target_os = "ios", target_os = "tvos"))]
use gpui::{AppContext, WindowOptions};
use std::ffi::{CStr, CString};
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(any(target_os = "ios", target_os = "tvos"))]
use super::assets::Assets;
#[cfg(any(target_os = "ios", target_os = "tvos"))]
use super::misc::get_ios_music_directory;
#[cfg(any(target_os = "ios", target_os = "tvos"))]
use sotf_audio_player_gpui::app::state::ui::LayoutState;
#[cfg(any(target_os = "ios", target_os = "tvos"))]
use sotf_audio_player_gpui::app::{App, AppState};
#[cfg(any(target_os = "ios", target_os = "tvos"))]
use sotf_audio_player_gpui::ui;

/// Pop one queued command for the iOS GPUI tick.
///
/// Return codes are intentionally scalar so `app-gpui` can consume remote
/// commands without depending on this crate and creating a cycle:
/// 0 = none, 1 = next track, 2 = previous track, 3 = imported files noticed,
/// 4 = QR payload scanned.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_pop_remote_command() -> i32 {
    ffi_guard(|| match pending_queue().pop() {
        None => 0,
        Some(RemoteCommand::NextTrack) => 1,
        Some(RemoteCommand::PrevTrack) => 2,
        Some(RemoteCommand::ImportFiles(paths)) => {
            log::info!(
                "[iOS] Imported files drain observed {} paths; full library import pending",
                paths.len()
            );
            3
        }
        Some(RemoteCommand::QrPayloadScanned) => 4,
    })
}

/// Return and clear imported file paths as a JSON string for GPUI to consume.
///
/// The returned pointer must be released with `sotf_ios_string_free`.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_take_imported_files_json() -> *mut std::ffi::c_char {
    ffi_guard(|| {
        // Drain the lock-free queue. Each path is converted to a `String` for
        // JSON serialization; the JSON output itself is the only allocation
        // passed back to Swift.
        let paths: Vec<String> = std::iter::from_fn(|| pending_imports().pop())
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        let Ok(json) = serde_json::to_string(&paths) else {
            log::error!("[iOS] failed to serialize imported file paths");
            return std::ptr::null_mut();
        };
        match CString::new(json) {
            Ok(value) => value.into_raw(),
            Err(_) => {
                log::error!("[iOS] imported file JSON contained interior NUL");
                std::ptr::null_mut()
            }
        }
    })
}

/// Return the next scanned QR payload for GPUI to consume.
///
/// The returned pointer must be released with `sotf_ios_string_free`.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_take_scanned_qr_payload() -> *mut std::ffi::c_char {
    ffi_guard(|| {
        let Some(payload) = pending_qr_payloads().pop() else {
            return std::ptr::null_mut();
        };
        match CString::new(payload) {
            Ok(value) => value.into_raw(),
            Err(_) => {
                log::error!("[iOS] scanned QR payload contained interior NUL");
                std::ptr::null_mut()
            }
        }
    })
}

/// Free a string returned by an iOS Rust FFI function.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_string_free(value: *mut std::ffi::c_char) {
    ffi_guard(|| {
        if value.is_null() {
            return;
        }
        // SAFETY: callers may only pass pointers returned by `CString::into_raw`
        // from this crate, and each pointer is freed at most once.
        unsafe {
            let _ = CString::from_raw(value);
        }
    })
}

/// Called from Swift to start the GPUI application.
#[cfg(any(target_os = "ios", target_os = "tvos"))]
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_start() {
    ffi_guard(|| {
        // Set up logging to os_log
        if let Err(e) = oslog::OsLogger::new("org.spinorama.sotf")
            .level_filter(log::LevelFilter::Info)
            .init()
        {
            eprintln!("[iOS] oslog init failed: {e}");
        }

        log::info!("sotf_ios_start: registering app callback");

        // Register asset source so SVG icons, fonts, and brand images load correctly.
        gpui_ios::ios::ffi::set_asset_source(Assets);

        gpui_ios::ios::ffi::set_app_callback(Box::new(|cx: &mut gpui::App| {
            log::info!("GPUI app callback: setting up SotF player");

            // Load custom fonts
            let fonts = vec![
                "fonts/B612-Regular.ttf",
                "fonts/B612-Italic.ttf",
                "fonts/B612-Bold.ttf",
                "fonts/B612-BoldItalic.ttf",
            ];

            let mut font_data = Vec::new();
            for path in fonts {
                if let Some(file) = Assets::get(path) {
                    font_data.push(file.data);
                } else {
                    log::warn!("Failed to load font: {}", path);
                }
            }

            if !font_data.is_empty() {
                if let Err(e) = cx.text_system().add_fonts(font_data) {
                    log::error!("[iOS] add_fonts failed: {e}");
                }
            }

            // Open a fullscreen window with the player
            let open_result = cx.open_window(
                WindowOptions {
                    window_bounds: None,
                    ..Default::default()
                },
                |_, cx| {
                    // Load configuration before creating entities
                    let mut temp_app = App::new();
                    let layout_state = match temp_app.load_config() {
                        Ok(l) => l,
                        Err(e) => {
                            log::warn!("Could not load saved configuration: {}", e);
                            LayoutState::default()
                        }
                    };

                    let player = Player::new();
                    if let Err(e) = player.set_volume(temp_app.playback.volume) {
                        log::warn!("Failed to set initial volume: {}", e);
                    }

                    let layout = cx.new(|_| layout_state);
                    #[allow(clippy::arc_with_non_send_sync)]
                    // Player is !Send + !Sync because of internal *const pointers used
                    // by the audio engine. The Arc<Mutex<_>> wrapper is the agreed
                    // pattern for cross-thread access on this type.
                    let player_arc = Arc::new(parking_lot::Mutex::new(player));

                    // Store global handle for C FFI callbacks (interruptions, remote commands)
                    if GLOBAL_PLAYER.set(Arc::clone(&player_arc)).is_err() {
                        log::error!(
                            "[iOS] GLOBAL_PLAYER already set — re-entry into sotf_ios_start"
                        );
                    }

                    let app_state = cx.new(|_cx| {
                        let mut app = temp_app;
                        app.load_audio_devices();

                        // Auto-add the iOS sandbox Music directory to the library
                        if let Some(music_dir) = get_ios_music_directory() {
                            log::info!("Adding iOS music directory: {}", music_dir.display());
                            app.add_directory_quiet(music_dir);
                        }

                        app.start_remote_server_discovery();

                        AppState {
                            app,
                            layout,
                            player: player_arc,
                        }
                    });

                    cx.new(|cx| ui::PlayerView::new(app_state.clone(), cx))
                },
            );

            if let Err(e) = open_result {
                log::error!("[iOS] Failed to open player window: {e}");
                return;
            }

            cx.activate(true);
        }));

        log::info!("sotf_ios_start: calling run_app");
        gpui_ios::ios::ffi::run_app();
    })
}

/// Called from Swift when the user imports files via the document picker.
///
/// The JSON payload is an array of absolute paths. Each entry that points to
/// an audio file is forwarded to the playback queue via `Player::queue_next`,
/// and the full list is also pushed onto the pending remote-command queue so
/// the GPUI side can register the containing folders for a library scan.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_files_imported(paths_json: *const std::ffi::c_char) {
    ffi_guard(AssertUnwindSafe(|| {
        if paths_json.is_null() {
            return;
        }

        // SAFETY: caller (Swift) guarantees `paths_json` is non-null (checked
        // above), points to a NUL-terminated UTF-8 buffer, and remains valid
        // for the duration of this call.
        let json_str = match unsafe { std::ffi::CStr::from_ptr(paths_json) }.to_str() {
            Ok(s) => s,
            Err(e) => {
                log::error!("[iOS] Invalid UTF-8 in imported paths: {}", e);
                return;
            }
        };

        let paths: Vec<String> = match serde_json::from_str(json_str) {
            Ok(p) => p,
            Err(e) => {
                log::error!("[iOS] Failed to parse imported paths JSON: {}", e);
                return;
            }
        };

        log::info!("[iOS] Files imported: {} files", paths.len());
        let path_bufs: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();

        // Minimum-viable wiring: queue each imported audio file for gapless
        // playback. `queue_next` is a no-op when no engine is running, so this
        // is safe even before the user has started playback.
        if let Some(player) = GLOBAL_PLAYER.get() {
            for path in &path_bufs {
                log::info!("[iOS]   {}", path.display());
                let res = player.lock().queue_next(path.clone());
                if let Err(e) = res {
                    log::warn!("[iOS] queue_next({}) failed: {}", path.display(), e);
                }
            }
        } else {
            log::warn!(
                "[iOS] Imported files but GLOBAL_PLAYER not set yet ({} paths)",
                path_bufs.len()
            );
        }

        // Also enqueue for the GPUI library-import consumer.
        for path in &path_bufs {
            pending_imports().push(path.clone());
        }
        push_remote_command(RemoteCommand::ImportFiles(path_bufs));
    }))
}

/// Called from Swift when the native QR scanner reads a SOTF connection code.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_qr_scanned(payload: *const std::ffi::c_char) {
    ffi_guard(AssertUnwindSafe(|| {
        if payload.is_null() {
            return;
        }

        // SAFETY: caller (Swift) keeps `payload` valid for the duration of
        // this call. We immediately copy it into an owned Rust string.
        let payload = match unsafe { CStr::from_ptr(payload) }.to_str() {
            Ok(value) => value.to_string(),
            Err(err) => {
                log::error!("[iOS] Invalid UTF-8 in scanned QR payload: {err}");
                return;
            }
        };
        if payload.trim().is_empty() {
            return;
        }

        pending_qr_payloads().push(payload);
        push_remote_command(RemoteCommand::QrPayloadScanned);
    }))
}

/// Called when an audio interruption begins or ends (phone call, alarm, Siri).
/// `began` = true means pause, false means resume.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_audio_interrupted(began: bool) {
    ffi_guard(|| {
        let Some(player) = GLOBAL_PLAYER.get() else {
            return;
        };

        if began {
            log::info!("[iOS] Audio interrupted — pausing");
            let _ = player.lock().pause();
        } else {
            log::info!("[iOS] Audio interruption ended — resuming");
            let _ = player.lock().resume();
        }
    })
}

/// Called when the audio route changes (headphone unplug → pause).
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_audio_route_changed() {
    ffi_guard(|| {
        let Some(player) = GLOBAL_PLAYER.get() else {
            return;
        };

        log::info!("[iOS] Audio route changed — pausing");
        let _ = player.lock().pause();
    })
}

/// Play from lock screen or Control Center.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_remote_play() {
    ffi_guard(|| {
        let Some(player) = GLOBAL_PLAYER.get() else {
            return;
        };
        log::info!("[iOS] Remote: play");
        let _ = player.lock().resume();
    })
}

/// Pause from lock screen or Control Center.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_remote_pause() {
    ffi_guard(|| {
        let Some(player) = GLOBAL_PLAYER.get() else {
            return;
        };
        log::info!("[iOS] Remote: pause");
        let _ = player.lock().pause();
    })
}

/// Toggle play/pause from lock screen or Control Center.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_remote_toggle_play_pause() {
    ffi_guard(|| {
        let Some(player) = GLOBAL_PLAYER.get() else {
            return;
        };

        // Release the lock between is_playing() and pause()/resume() so that
        // any re-entrant callback (e.g. route-changed reaching back through
        // the engine state observer) does not deadlock — `parking_lot::Mutex`
        // is not reentrant. The tiny TOCTOU window is harmless for a UI
        // toggle.
        let is_playing = player.lock().is_playing();
        if is_playing {
            log::info!("[iOS] Remote: pause");
            let _ = player.lock().pause();
        } else {
            log::info!("[iOS] Remote: play");
            let _ = player.lock().resume();
        }
    })
}

/// Next track from lock screen or Control Center.
///
/// Implementation note: track navigation lives on the AppState's
/// `QueueController`, not on the engine-level `Player`. We therefore push a
/// `RemoteCommand::NextTrack` onto the pending queue for the GPUI tick to
/// drain on the UI thread.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_remote_next_track() {
    ffi_guard(|| {
        log::info!("[iOS] Remote: next track");
        push_remote_command(RemoteCommand::NextTrack);
    })
}

/// Previous track from lock screen or Control Center. See `next_track`.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_remote_prev_track() {
    ffi_guard(|| {
        log::info!("[iOS] Remote: prev track");
        push_remote_command(RemoteCommand::PrevTrack);
    })
}

/// Seek to position from lock screen scrubber.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_remote_seek(position: f64) {
    ffi_guard(|| {
        let Some(player) = GLOBAL_PLAYER.get() else {
            return;
        };

        log::info!("[iOS] Remote: seek to {:.1}s", position);
        let _ = player.lock().seek(position);
    })
}

#[cfg(test)]
mod tests {
    use super::super::consts::QUEUE_TEST_LOCK;
    use super::*;

    fn drain_all_queues() {
        while pending_queue().pop().is_some() {}
        while pending_imports().pop().is_some() {}
        while pending_qr_payloads().pop().is_some() {}
    }

    #[test]
    fn pop_remote_command_returns_expected_codes() {
        let _guard = QUEUE_TEST_LOCK.lock();
        drain_all_queues();

        assert_eq!(sotf_ios_pop_remote_command(), 0);

        push_remote_command(RemoteCommand::NextTrack);
        push_remote_command(RemoteCommand::PrevTrack);
        push_remote_command(RemoteCommand::QrPayloadScanned);
        push_remote_command(RemoteCommand::ImportFiles(vec![
            PathBuf::from("/tmp/x.mp3"),
        ]));

        assert_eq!(sotf_ios_pop_remote_command(), 1);
        assert_eq!(sotf_ios_pop_remote_command(), 2);
        assert_eq!(sotf_ios_pop_remote_command(), 4);
        assert_eq!(sotf_ios_pop_remote_command(), 3);
        assert_eq!(sotf_ios_pop_remote_command(), 0);
    }

    #[test]
    fn take_imported_files_json_roundtrips() {
        let _guard = QUEUE_TEST_LOCK.lock();
        drain_all_queues();

        // An empty queue serializes as `[]`; the pointer must still be freed.
        let empty = sotf_ios_take_imported_files_json();
        assert!(!empty.is_null());
        assert_eq!(unsafe { CStr::from_ptr(empty) }.to_str().unwrap(), "[]");
        sotf_ios_string_free(empty);

        pending_imports().push(PathBuf::from("/music/a.mp3"));
        pending_imports().push(PathBuf::from("/music/b.flac"));

        let ptr = sotf_ios_take_imported_files_json();
        assert!(!ptr.is_null());
        let json = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
        assert_eq!(json, r#"["/music/a.mp3","/music/b.flac"]"#);
        sotf_ios_string_free(ptr);

        let empty2 = sotf_ios_take_imported_files_json();
        assert!(!empty2.is_null());
        assert_eq!(
            unsafe { CStr::from_ptr(empty2) }.to_str().unwrap(),
            "[]"
        );
        sotf_ios_string_free(empty2);
    }

    #[test]
    fn take_scanned_qr_payload_roundtrips() {
        let _guard = QUEUE_TEST_LOCK.lock();
        drain_all_queues();

        assert!(sotf_ios_take_scanned_qr_payload().is_null());

        pending_qr_payloads().push("qr-payload-1".to_string());

        let ptr = sotf_ios_take_scanned_qr_payload();
        assert!(!ptr.is_null());
        let payload = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
        assert_eq!(payload, "qr-payload-1");
        sotf_ios_string_free(ptr);

        assert!(sotf_ios_take_scanned_qr_payload().is_null());
    }
}
