//! Cross-platform OS media controls for SOTF apps.
//!
//! Replaces the [`souvlaki`] crate with a tighter, dependency-light API:
//!
//! - **macOS**: `MPRemoteCommandCenter` + `MPNowPlayingInfoCenter` via the
//!   modern `objc2-media-player` bindings (no `cocoa-rs` chain).
//! - **Linux / FreeBSD**: MPRIS via `mpris-server` (zbus-based, modern).
//! - **Windows / iOS / tvOS / other**: graceful no-op `MediaControls::new`
//!   returns `Err(Error::Unsupported)` so callers fall back without panics.

// FFI wrapper crate: every "unsafe" is a thin pass-through to the Apple
// MediaPlayer / mpris-server APIs whose preconditions are documented at the
// framework level, not per call site. The pedantic lints below produce noise
// that obscures the actual structure rather than catching bugs here.
#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::unused_self,
    clippy::unnecessary_wraps,
    clippy::items_after_statements,
    clippy::semicolon_outside_block,
    clippy::undocumented_unsafe_blocks,
    clippy::unnecessary_safety_comment,
    clippy::clone_on_ref_ptr,
    reason = "pedantic noise on a thin FFI wrapper crate; see crate docs"
)]

use std::time::Duration;

mod backend;
mod types;

pub use types::{
    MediaControlEvent, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig, SeekDirection,
    WindowHandle,
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
    ///
    /// On macOS, construction wires `MPRemoteCommandCenter` targets and must
    /// happen on the main thread. Later [`Self::set_metadata`] and
    /// [`Self::set_playback`] calls may come from any thread.
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
    /// Build a `MediaPosition` from a `f64` second count.
    ///
    /// `NaN`, infinities, and non-positive values clamp to zero —
    /// `Duration::from_secs_f64` would otherwise panic on NaN / infinity.
    /// Use this constructor when the value comes from external/untrusted
    /// sources (UI scrubbers, MPRIS, lock-screen `positionTime`, ...).
    #[must_use]
    pub fn from_secs_f64(secs: f64) -> Self {
        if !secs.is_finite() || secs <= 0.0 {
            Self(Duration::ZERO)
        } else {
            Self(Duration::from_secs_f64(secs))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_position_from_secs_handles_nan() {
        // `Duration::from_secs_f64(NaN)` panics — the constructor's job
        // is to guard against that.
        let p = MediaPosition::from_secs_f64(f64::NAN);
        assert_eq!(p.0, Duration::ZERO);
    }

    #[test]
    fn media_position_from_secs_handles_infinities() {
        assert_eq!(
            MediaPosition::from_secs_f64(f64::INFINITY).0,
            Duration::ZERO
        );
        assert_eq!(
            MediaPosition::from_secs_f64(f64::NEG_INFINITY).0,
            Duration::ZERO
        );
    }

    #[test]
    fn media_position_from_secs_handles_negative() {
        assert_eq!(MediaPosition::from_secs_f64(-1.0).0, Duration::ZERO);
    }

    #[test]
    fn media_position_from_secs_preserves_positive() {
        let p = MediaPosition::from_secs_f64(1.5);
        assert_eq!(p.0, Duration::from_millis(1500));
    }
}
