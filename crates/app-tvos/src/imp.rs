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
    fn sotf_tvos_get_music_directory() -> *const std::ffi::c_char;
}

/// Global handle to the player so C FFI callbacks can control playback.
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
pub extern "C" fn sotf_tvos_start() {
    // Set up logging to os_log
    oslog::OsLogger::new("org.spinorama.sotf.tv")
        .level_filter(log::LevelFilter::Info)
        .init()
        .ok();

    log::info!("sotf_tvos_start: registering app callback");

    // Register asset source so SVG icons, fonts, and brand images load correctly.
    gpui_ios::ios::ffi::set_asset_source(Assets);

    gpui_ios::ios::ffi::set_app_callback(Box::new(|cx: &mut gpui::App| {
        log::info!("GPUI app callback: setting up SotF TV player");

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

                // Bump font scale for 10-foot viewing distance on TV
                if temp_app.ui_state.font_scale < 1.5 {
                    temp_app.ui_state.font_scale = 1.5;
                }

                let player = Player::new();
                if let Err(e) = player.set_volume(temp_app.playback.volume) {
                    log::warn!("Failed to set initial volume: {}", e);
                }

                let layout = cx.new(|_| layout_state);
                #[allow(clippy::arc_with_non_send_sync)]
                let player_arc = Arc::new(parking_lot::Mutex::new(player));

                // Store global handle for C FFI callbacks
                GLOBAL_PLAYER.set(Arc::clone(&player_arc)).ok();

                let app_state = cx.new(|_cx| {
                    let mut app = temp_app;
                    app.load_audio_devices();

                    // Auto-add the tvOS music directory to the library
                    if let Some(music_dir) = get_tvos_music_directory() {
                        log::info!("Adding tvOS music directory: {}", music_dir.display());
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

    log::info!("sotf_tvos_start: calling run_app");
    gpui_ios::ios::ffi::run_app();
}

// ============================================================================
// tvOS Helpers
// ============================================================================

/// Get the tvOS sandbox music directory path from Swift.
fn get_tvos_music_directory() -> Option<PathBuf> {
    let c_str = unsafe { sotf_tvos_get_music_directory() };
    if c_str.is_null() {
        return None;
    }
    let path_str = unsafe { std::ffi::CStr::from_ptr(c_str) }.to_str().ok()?;
    Some(PathBuf::from(path_str))
}

// ============================================================================
// Audio Lifecycle FFI (Swift AudioManager → Rust)
// ============================================================================

/// Called when an audio interruption begins or ends.
#[unsafe(no_mangle)]
pub extern "C" fn sotf_tvos_audio_interrupted(began: bool) {
    let Some(player) = GLOBAL_PLAYER.get() else {
        return;
    };

    if began {
        log::info!("[tvOS] Audio interrupted — pausing");
        let _ = player.lock().pause();
    } else {
        log::info!("[tvOS] Audio interruption ended — resuming");
        let _ = player.lock().resume();
    }
}

// ============================================================================
// Remote Command FFI (Swift → Rust)
// ============================================================================

#[unsafe(no_mangle)]
pub extern "C" fn sotf_tvos_remote_play() {
    let Some(player) = GLOBAL_PLAYER.get() else {
        return;
    };
    log::info!("[tvOS] Remote: play");
    let _ = player.lock().resume();
}

#[unsafe(no_mangle)]
pub extern "C" fn sotf_tvos_remote_pause() {
    let Some(player) = GLOBAL_PLAYER.get() else {
        return;
    };
    log::info!("[tvOS] Remote: pause");
    let _ = player.lock().pause();
}

#[unsafe(no_mangle)]
pub extern "C" fn sotf_tvos_remote_toggle_play_pause() {
    let Some(player) = GLOBAL_PLAYER.get() else {
        return;
    };

    // Release the lock between is_playing() and pause()/resume() so that
    // any re-entrant callback (e.g. audio interruption reaching back through
    // the engine state observer) does not deadlock — `parking_lot::Mutex` is
    // not reentrant. The tiny TOCTOU window is harmless for a UI toggle.
    let is_playing = player.lock().is_playing();
    if is_playing {
        log::info!("[tvOS] Remote: pause");
        let _ = player.lock().pause();
    } else {
        log::info!("[tvOS] Remote: play");
        let _ = player.lock().resume();
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
