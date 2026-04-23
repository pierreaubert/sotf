//! OS media controls integration (MPRIS on Linux, MediaPlayer on macOS/Windows).
//!
//! Provides play/pause/next/previous from the desktop environment's media controls.

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};
use std::sync::mpsc;
use std::time::Duration;

pub struct GpuiMediaControls {
    controls: MediaControls,
    rx: mpsc::Receiver<MediaControlEvent>,
}

impl GpuiMediaControls {
    pub fn new() -> anyhow::Result<Self> {
        // On Windows, souvlaki panics (instead of returning Err) if no HWND is
        // provided. Catch the panic so the app starts without media controls
        // rather than crashing.
        let result = std::panic::catch_unwind(|| {
            let config = PlatformConfig {
                dbus_name: "sotf_player",
                display_name: "SotF Player",
                hwnd: get_hwnd(),
            };
            MediaControls::new(config)
        });

        let mut controls = match result {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => return Err(anyhow::anyhow!("media controls init failed: {e}")),
            Err(_) => {
                return Err(anyhow::anyhow!(
                    "media controls panicked during init (no HWND on Windows?)"
                ));
            }
        };

        let (tx, rx) = mpsc::channel();
        controls.attach(move |event: MediaControlEvent| {
            let _ = tx.send(event);
        })?;

        Ok(Self { controls, rx })
    }

    /// Poll for a single pending media control event (non-blocking).
    pub fn poll_event(&self) -> Option<MediaControlEvent> {
        self.rx.try_recv().ok()
    }

    /// Update Now Playing metadata.
    pub fn set_metadata(
        &mut self,
        title: Option<&str>,
        artist: Option<&str>,
        album: Option<&str>,
        duration: Option<Duration>,
        cover_url: Option<&str>,
    ) {
        let metadata = MediaMetadata {
            title,
            artist,
            album,
            duration,
            cover_url,
        };
        if let Err(e) = self.controls.set_metadata(metadata) {
            log::warn!("Failed to set media metadata: {}", e);
        }
    }

    /// Update playback state in the OS media controls.
    pub fn set_playback(&mut self, playback: MediaPlayback) {
        if let Err(e) = self.controls.set_playback(playback) {
            log::warn!("Failed to set media playback state: {}", e);
        }
    }
}

/// Update media controls metadata and playback state from current app state.
pub fn update_media_controls(
    mc: &mut GpuiMediaControls,
    app: &crate::app::state::App,
    position_secs: f64,
) {
    // Current track metadata
    let queue_item = app
        .playback
        .current_queue_index
        .and_then(|idx| app.queue_state.get(idx));

    let track = queue_item.and_then(|item| item.current_track());
    let album_title = queue_item.map(|item| item.album.title.as_str());

    let title_owned: String;
    let artist_owned: String;
    let (title, artist) = match track {
        Some(t) => {
            title_owned = t.title.clone().unwrap_or_default();
            artist_owned = t.artist.clone().unwrap_or_default();
            (
                if title_owned.is_empty() {
                    None
                } else {
                    Some(title_owned.as_str())
                },
                if artist_owned.is_empty() {
                    None
                } else {
                    Some(artist_owned.as_str())
                },
            )
        }
        None => (None, None),
    };

    let duration = track.and_then(|t| t.duration_secs).map(Duration::from_secs);

    let cover_url_owned = queue_item
        .and_then(|item| item.album.album_art_path.as_ref())
        .filter(|path| path.exists())
        .map(|path| format!("file://{}", path.display()));
    let cover_url = cover_url_owned.as_deref();

    mc.set_metadata(title, artist, album_title, duration, cover_url);

    // Playback state
    let progress = Some(MediaPosition(Duration::from_secs_f64(position_secs)));
    let playback = if app.playback.is_playing {
        MediaPlayback::Playing { progress }
    } else if app.playback.current_queue_index.is_some() {
        MediaPlayback::Paused { progress }
    } else {
        MediaPlayback::Stopped
    };
    mc.set_playback(playback);
}

/// On Windows, souvlaki requires a valid HWND.
#[cfg(target_os = "windows")]
fn get_hwnd() -> Option<*mut core::ffi::c_void> {
    // souvlaki on Windows panics if hwnd is None (requires a message window).
    // We don't have easy access to the GPUI HWND here, so we return None
    // and let the caller catch the panic via catch_unwind.
    None
}

#[cfg(not(target_os = "windows"))]
fn get_hwnd() -> Option<*mut core::ffi::c_void> {
    None
}
