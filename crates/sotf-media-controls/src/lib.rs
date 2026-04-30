//! Cross-platform OS media controls for SOTF apps.
//!
//! Replaces the [`souvlaki`] crate with a tighter, dependency-light API:
//!
//! - **macOS**: `MPRemoteCommandCenter` + `MPNowPlayingInfoCenter` via the
//!   modern `objc2-media-player` bindings (no `cocoa-rs` chain).
//! - **Linux / FreeBSD**: MPRIS via `mpris-server` (zbus-based, modern).
//! - **Windows / iOS / tvOS / other**: graceful no-op `MediaControls::new`
//!   returns `Err(Error::Unsupported)` so callers fall back without panics.
//!
//! The public surface mirrors the slice of `souvlaki` we actually used, so
//! migration is mostly mechanical (`use souvlaki::X` → `use sotf_media_controls::X`).
//!
//! [`souvlaki`]: https://crates.io/crates/souvlaki

use std::time::Duration;

mod backend;
mod types;

pub use types::{
    MediaControlEvent, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig, SeekDirection,
};

/// Errors produced by media-control operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("media controls are not supported on this platform")]
    Unsupported,
    #[error("failed to initialize media controls: {0}")]
    Init(String),
    #[error("failed to update media controls: {0}")]
    Update(String),
    #[error("failed to attach event handler: {0}")]
    Attach(String),
}

/// Cross-platform OS media-controls handle.
pub struct MediaControls {
    inner: backend::Backend,
}

impl std::fmt::Debug for MediaControls {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaControls").finish_non_exhaustive()
    }
}

impl MediaControls {
    /// Create a new media-controls handle for the current platform.
    ///
    /// On unsupported platforms (Windows, iOS, tvOS, ...) this returns
    /// [`Error::Unsupported`]; callers should treat that as "no OS media
    /// controls available" rather than a hard failure.
    pub fn new(config: PlatformConfig<'_>) -> Result<Self, Error> {
        Ok(Self {
            inner: backend::Backend::new(config)?,
        })
    }

    /// Register an event handler invoked from the OS-controls callback
    /// thread (macOS main run loop / D-Bus reader thread).
    ///
    /// `handler` must be `Send + 'static`. Use a channel inside the closure if
    /// you need the events on a different thread.
    pub fn attach(
        &mut self,
        handler: impl FnMut(MediaControlEvent) + Send + 'static,
    ) -> Result<(), Error> {
        self.inner.attach(Box::new(handler))
    }

    /// Update the Now-Playing metadata.
    pub fn set_metadata(&mut self, metadata: MediaMetadata<'_>) -> Result<(), Error> {
        self.inner.set_metadata(metadata)
    }

    /// Update the playback state (playing/paused/stopped + optional progress).
    pub fn set_playback(&mut self, playback: MediaPlayback) -> Result<(), Error> {
        self.inner.set_playback(playback)
    }
}

/// Convenience helper used by callers that want to construct a `MediaPosition`
/// from a `f64` of seconds without depending on `Duration` directly.
impl MediaPosition {
    pub fn from_secs_f64(secs: f64) -> Self {
        Self(Duration::from_secs_f64(secs.max(0.0)))
    }
}
