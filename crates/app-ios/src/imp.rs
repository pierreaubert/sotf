use gpui::*;
use rust_embed::RustEmbed;
use sotf_audio_player::{Player, SotfRemoteAuthToken, SotfRemoteServer};
use sotf_audio_player_gpui::app::state::ui::LayoutState;
use sotf_audio_player_gpui::app::{App, AppState};
use sotf_audio_player_gpui::ui;
use std::borrow::Cow;
use std::collections::VecDeque;
use std::ffi::CString;
use std::panic::{AssertUnwindSafe, UnwindSafe};
use std::path::PathBuf;
use std::sync::Arc;

use std::sync::OnceLock;

// Swift functions we can call from Rust
unsafe extern "C" {
    fn sotf_ios_show_document_picker();
    fn sotf_ios_get_music_directory() -> *const std::ffi::c_char;
    fn sotf_ios_update_now_playing(
        title: *const std::ffi::c_char,
        artist: *const std::ffi::c_char,
        album: *const std::ffi::c_char,
        duration: f64,
        position: f64,
        is_playing: bool,
    );
    fn sotf_ios_update_now_playing_position(position: f64, is_playing: bool);
    #[allow(dead_code)]
    fn sotf_ios_keychain_save(key: *const std::ffi::c_char, token: *const std::ffi::c_char)
    -> bool;
    #[allow(dead_code)]
    fn sotf_ios_keychain_load(key: *const std::ffi::c_char) -> *const std::ffi::c_char;
    #[allow(dead_code)]
    fn sotf_ios_keychain_delete(key: *const std::ffi::c_char) -> bool;
}

/// Global handle to the player so C FFI callbacks can control playback.
/// Set once during app initialization, never changes.
static GLOBAL_PLAYER: OnceLock<Arc<parking_lot::Mutex<Player>>> = OnceLock::new();

/// Remote control commands that require AppState/Queue access. The GPUI event
/// loop drains this queue on each tick through `sotf_ios_pop_remote_command`.
#[derive(Debug, Clone)]
pub enum RemoteCommand {
    NextTrack,
    PrevTrack,
    /// File paths imported from the iOS document picker. The consumer should
    /// either add them to the library or push them onto the playback queue.
    ImportFiles(Vec<PathBuf>),
}

static PENDING_REMOTE_COMMANDS: OnceLock<parking_lot::Mutex<VecDeque<RemoteCommand>>> =
    OnceLock::new();
static PENDING_IMPORTED_FILES: OnceLock<parking_lot::Mutex<Vec<PathBuf>>> = OnceLock::new();

fn pending_queue() -> &'static parking_lot::Mutex<VecDeque<RemoteCommand>> {
    PENDING_REMOTE_COMMANDS.get_or_init(|| parking_lot::Mutex::new(VecDeque::new()))
}

fn pending_imports() -> &'static parking_lot::Mutex<Vec<PathBuf>> {
    PENDING_IMPORTED_FILES.get_or_init(|| parking_lot::Mutex::new(Vec::new()))
}

/// Push a remote command for the GPUI loop to drain.
fn push_remote_command(cmd: RemoteCommand) {
    pending_queue().lock().push_back(cmd);
}

/// Drain pending remote commands. Intended for the GPUI tick consumer (TODO).
#[allow(dead_code)]
pub fn drain_pending_remote_commands() -> Vec<RemoteCommand> {
    pending_queue().lock().drain(..).collect()
}

/// Pop one queued command for the iOS GPUI tick.
///
/// Return codes are intentionally scalar so `app-gpui` can consume remote
/// commands without depending on this crate and creating a cycle:
/// 0 = none, 1 = next track, 2 = previous track, 3 = imported files noticed.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_pop_remote_command() -> i32 {
    ffi_guard(|| match pending_queue().lock().pop_front() {
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
    })
}

/// Return and clear imported file paths as a JSON string for GPUI to consume.
///
/// The returned pointer must be released with `sotf_ios_string_free`.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_take_imported_files_json() -> *mut std::ffi::c_char {
    ffi_guard(|| {
        let paths: Vec<String> = pending_imports()
            .lock()
            .drain(..)
            .map(|path| path.display().to_string())
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

/// FFI panic guard. Wrap every `extern "C"` body in this so a Rust panic does
/// not unwind across the C ABI (which is UB under the workspace's
/// `panic = "unwind"` strategy).
fn ffi_guard<F, R>(f: F) -> R
where
    F: FnOnce() -> R + UnwindSafe,
    R: Default,
{
    match std::panic::catch_unwind(f) {
        Ok(r) => r,
        Err(_) => {
            log::error!("[iOS] FFI call panicked; returning default");
            R::default()
        }
    }
}

/// Embedded assets including Lucide SVG icons and brand images
#[derive(RustEmbed)]
#[folder = "../app-gpui/assets"]
#[include = "icons/*.svg"]
#[include = "fonts/*.ttf"]
#[include = "brands/*.jpg"]
#[include = "brands/*.jpeg"]
#[include = "brands/*.png"]
#[include = "brands/*.webp"]
#[include = "sotf.jpg"]
struct Assets;

/// Called from Swift to start the GPUI application.
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

// ============================================================================
// iOS File Import FFI
// ============================================================================

/// Get the iOS sandbox music directory path from Swift.
fn get_ios_music_directory() -> Option<PathBuf> {
    // SAFETY: `sotf_ios_get_music_directory` is implemented on the Swift side
    // and is documented to return either NULL or a pointer to a NUL-terminated
    // UTF-8 C string whose storage outlives this call (it is held by Swift in
    // an autoreleased NSString backing buffer). We check NULL below, then
    // immediately copy the bytes into an owned `PathBuf` so the Swift-owned
    // pointer is not retained past this function.
    let c_str = unsafe { sotf_ios_get_music_directory() };
    if c_str.is_null() {
        return None;
    }
    // SAFETY: see the comment above — `c_str` is non-null and points to a
    // NUL-terminated UTF-8 buffer valid for the duration of this call.
    let path_str = unsafe { std::ffi::CStr::from_ptr(c_str) }.to_str().ok()?;
    Some(PathBuf::from(path_str))
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
        pending_imports().lock().extend(path_bufs.iter().cloned());
        push_remote_command(RemoteCommand::ImportFiles(path_bufs));
    }))
}

// ============================================================================
// Audio Lifecycle FFI (Swift AudioManager → Rust)
// ============================================================================

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

// ============================================================================
// Remote Command FFI (Swift MPRemoteCommandCenter → Rust)
// ============================================================================

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

// ============================================================================
// Now Playing Update (Rust → Swift)
// ============================================================================

/// Build a `CString` from a `&str`, logging and substituting on interior NUL.
fn cstring_or_unknown(value: &str, field: &str) -> CString {
    match CString::new(value) {
        Ok(c) => c,
        Err(_) => {
            log::warn!("[iOS] interior NUL in {field}; substituting <unknown>");
            // "<unknown>" contains no NUL — unwrap is infallible.
            CString::new("<unknown>").expect("static literal has no NUL")
        }
    }
}

#[allow(dead_code)]
fn cstring_for_keychain(value: &str, field: &str) -> Option<CString> {
    match CString::new(value) {
        Ok(value) => Some(value),
        Err(_) => {
            log::warn!("[iOS] interior NUL in remote {field}; refusing Keychain operation");
            None
        }
    }
}

/// Store a remote server bearer token in the iOS Keychain.
///
/// The account name is derived from `SotfRemoteServer::token_secret_key`; the
/// token is never written to `remote_servers.json`.
#[allow(dead_code)]
pub fn save_remote_auth_token(server: &SotfRemoteServer, token: &SotfRemoteAuthToken) -> bool {
    let Some(key) = cstring_for_keychain(&server.token_secret_key(), "token key") else {
        return false;
    };
    let Some(token) = cstring_for_keychain(token.as_str(), "token") else {
        return false;
    };
    unsafe { sotf_ios_keychain_save(key.as_ptr(), token.as_ptr()) }
}

/// Load a remote server bearer token from the iOS Keychain.
#[allow(dead_code)]
pub fn load_remote_auth_token(server: &SotfRemoteServer) -> Option<SotfRemoteAuthToken> {
    let key = cstring_for_keychain(&server.token_secret_key(), "token key")?;
    let token = unsafe { sotf_ios_keychain_load(key.as_ptr()) };
    if token.is_null() {
        return None;
    }
    let token = unsafe { std::ffi::CStr::from_ptr(token) }.to_str().ok()?;
    SotfRemoteAuthToken::new(token).ok()
}

/// Delete a remote server bearer token from the iOS Keychain.
#[allow(dead_code)]
pub fn delete_remote_auth_token(server: &SotfRemoteServer) -> bool {
    let Some(key) = cstring_for_keychain(&server.token_secret_key(), "token key") else {
        return false;
    };
    unsafe { sotf_ios_keychain_delete(key.as_ptr()) }
}

/// Update the lock screen Now Playing info with full track metadata.
pub fn update_now_playing_info(
    title: &str,
    artist: &str,
    album: &str,
    duration: f64,
    position: f64,
    is_playing: bool,
) {
    let c_title = cstring_or_unknown(title, "title");
    let c_artist = cstring_or_unknown(artist, "artist");
    let c_album = cstring_or_unknown(album, "album");

    // SAFETY: all three pointers come from `CString`s owned in this stack
    // frame and remain valid for the entire duration of the call. The Swift
    // implementation copies the strings synchronously, so we may drop them
    // when this scope ends.
    unsafe {
        sotf_ios_update_now_playing(
            c_title.as_ptr(),
            c_artist.as_ptr(),
            c_album.as_ptr(),
            duration,
            position,
            is_playing,
        );
    }
}

/// Update just the playback position (called periodically, no metadata change).
pub fn update_now_playing_position(position: f64, is_playing: bool) {
    // SAFETY: `sotf_ios_update_now_playing_position` takes only scalar
    // arguments — no pointer lifetime to uphold.
    unsafe {
        sotf_ios_update_now_playing_position(position, is_playing);
    }
}

// ============================================================================
// Asset Source
// ============================================================================

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        Self::get(path)
            .map(|file| Some(file.data))
            .ok_or_else(|| anyhow::anyhow!("Could not find asset at path \"{}\"", path))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        if path.is_empty() {
            return Ok(Vec::new());
        }
        Ok(Self::iter()
            .filter_map(|p| {
                if p.starts_with(path) {
                    Some(SharedString::from(p.to_string()))
                } else {
                    None
                }
            })
            .collect())
    }
}
