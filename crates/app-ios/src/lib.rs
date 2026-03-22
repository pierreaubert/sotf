//! iOS app shell for SOTF music player.
//!
//! This crate compiles to a static library (.a) that the Xcode project links.
//! The Swift AppDelegate calls `sotf_ios_start()` to launch the GPUI app.
//!
//! Architecture:
//!   Swift AppDelegate → sotf_ios_start() → GPUI app callback → PlayerView
//!   Swift CADisplayLink → gpui_ios_request_frame() → GPUI render tick

use gpui::*;
use rust_embed::RustEmbed;
use sotf_audio_player::Player;
use sotf_audio_player_gpui::app::state::ui::LayoutState;
use sotf_audio_player_gpui::app::{App, AppState};
use sotf_audio_player_gpui::ui;
use std::borrow::Cow;
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
}

/// Global handle to the player so C FFI callbacks can control playback.
/// Set once during app initialization, never changes.
static GLOBAL_PLAYER: OnceLock<Arc<parking_lot::Mutex<Player>>> = OnceLock::new();

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
    // Set up logging to os_log
    oslog::OsLogger::new("org.spinorama.sotf")
        .level_filter(log::LevelFilter::Info)
        .init()
        .ok();

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
            cx.text_system().add_fonts(font_data).unwrap();
        }

        // Open a fullscreen window with the player
        cx.open_window(
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
                let player_arc = Arc::new(parking_lot::Mutex::new(player));

                // Store global handle for C FFI callbacks (interruptions, remote commands)
                GLOBAL_PLAYER.set(Arc::clone(&player_arc)).ok();

                let app_state = cx.new(|_cx| {
                    let mut app = temp_app;
                    app.load_audio_devices();

                    // Auto-add the iOS sandbox Music directory to the library
                    if let Some(music_dir) = get_ios_music_directory() {
                        log::info!("Adding iOS music directory: {}", music_dir.display());
                        app.add_directory_quiet(music_dir);
                    }

                    AppState {
                        app,
                        layout,
                        player: player_arc,
                    }
                });

                cx.new(|cx| ui::PlayerView::new(app_state.clone(), cx))
            },
        )
        .expect("Failed to open player window");

        cx.activate(true);
    }));

    log::info!("sotf_ios_start: calling run_app");
    gpui_ios::ios::ffi::run_app();
}

// ============================================================================
// iOS File Import FFI
// ============================================================================

/// Get the iOS sandbox music directory path from Swift.
fn get_ios_music_directory() -> Option<PathBuf> {
    let c_str = unsafe { sotf_ios_get_music_directory() };
    if c_str.is_null() {
        return None;
    }
    let path_str = unsafe { std::ffi::CStr::from_ptr(c_str) }
        .to_str()
        .ok()?;
    Some(PathBuf::from(path_str))
}

/// Called from Swift when the user imports files via the document picker.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_files_imported(paths_json: *const std::ffi::c_char) {
    if paths_json.is_null() {
        return;
    }

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
    for path in &paths {
        log::info!("[iOS]   {}", path);
    }
}

// ============================================================================
// Audio Lifecycle FFI (Swift AudioManager → Rust)
// ============================================================================

/// Called when an audio interruption begins or ends (phone call, alarm, Siri).
/// `began` = true means pause, false means resume.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_audio_interrupted(began: bool) {
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
}

/// Called when the audio route changes (headphone unplug → pause).
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_audio_route_changed() {
    let Some(player) = GLOBAL_PLAYER.get() else {
        return;
    };

    log::info!("[iOS] Audio route changed — pausing");
    let _ = player.lock().pause();
}

// ============================================================================
// Remote Command FFI (Swift MPRemoteCommandCenter → Rust)
// ============================================================================

/// Play from lock screen or Control Center.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_remote_play() {
    let Some(player) = GLOBAL_PLAYER.get() else {
        return;
    };
    log::info!("[iOS] Remote: play");
    let _ = player.lock().resume();
}

/// Pause from lock screen or Control Center.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_remote_pause() {
    let Some(player) = GLOBAL_PLAYER.get() else {
        return;
    };
    log::info!("[iOS] Remote: pause");
    let _ = player.lock().pause();
}

/// Toggle play/pause from lock screen or Control Center.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_remote_toggle_play_pause() {
    let Some(player) = GLOBAL_PLAYER.get() else {
        return;
    };

    let p = player.lock();
    if p.is_playing() {
        log::info!("[iOS] Remote: pause");
        let _ = p.pause();
    } else {
        log::info!("[iOS] Remote: play");
        let _ = p.resume();
    }
}

/// Next track from lock screen or Control Center.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_remote_next_track() {
    log::info!("[iOS] Remote: next track");
    // Note: Next/prev track requires access to the queue which lives in AppState.
    // The global player handle only controls the engine (pause/resume/seek).
    // For now, log the event. Full implementation requires a global command channel
    // to the GPUI event loop, which is a future enhancement.
}

/// Previous track from lock screen or Control Center.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_remote_prev_track() {
    log::info!("[iOS] Remote: prev track");
    // Same note as next_track — requires queue access.
}

/// Seek to position from lock screen scrubber.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_ios_remote_seek(position: f64) {
    let Some(player) = GLOBAL_PLAYER.get() else {
        return;
    };

    log::info!("[iOS] Remote: seek to {:.1}s", position);
    let _ = player.lock().seek(position);
}

// ============================================================================
// Now Playing Update (Rust → Swift)
// ============================================================================

/// Update the lock screen Now Playing info with full track metadata.
pub fn update_now_playing_info(
    title: &str,
    artist: &str,
    album: &str,
    duration: f64,
    position: f64,
    is_playing: bool,
) {
    let c_title = std::ffi::CString::new(title).unwrap_or_default();
    let c_artist = std::ffi::CString::new(artist).unwrap_or_default();
    let c_album = std::ffi::CString::new(album).unwrap_or_default();

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
