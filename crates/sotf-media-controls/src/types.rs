use std::time::Duration;

/// Platform-specific construction config.
///
/// Field-compatible with `souvlaki::PlatformConfig` for migration.
#[derive(Debug, Clone)]
pub struct PlatformConfig<'a> {
    /// D-Bus name suffix on Linux. Ignored on other platforms.
    pub dbus_name: &'a str,
    /// User-visible application name.
    pub display_name: &'a str,
    /// Windows HWND. Ignored on other platforms. Currently unused; the Windows
    /// backend is a no-op.
    pub hwnd: Option<*mut core::ffi::c_void>,
}

/// Now-playing metadata. Lifetime-borrowed to mirror souvlaki's API.
#[derive(Debug, Clone, Default)]
pub struct MediaMetadata<'a> {
    pub title: Option<&'a str>,
    pub artist: Option<&'a str>,
    pub album: Option<&'a str>,
    pub duration: Option<Duration>,
    pub cover_url: Option<&'a str>,
}

/// Current playback state.
#[derive(Debug, Clone, Copy)]
pub enum MediaPlayback {
    Stopped,
    Paused { progress: Option<MediaPosition> },
    Playing { progress: Option<MediaPosition> },
}

/// Wall-clock playback position.
#[derive(Debug, Clone, Copy)]
pub struct MediaPosition(pub Duration);

/// Direction for `Seek` / `SeekBy` events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekDirection {
    Forward,
    Backward,
}

/// Event raised by the OS media controls.
#[derive(Debug, Clone)]
pub enum MediaControlEvent {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Stop,
    /// Absolute position requested by the user (e.g. dragging the scrubber).
    SetPosition(MediaPosition),
    /// Volume in `[0.0, 1.0]`.
    SetVolume(f64),
    /// "Skip 10s" style seek with implementation-defined offset.
    Seek(SeekDirection),
    /// Explicit duration-offset seek.
    SeekBy(SeekDirection, Duration),
    /// MPRIS-only: foreground / "raise window" request.
    Raise,
    /// MPRIS-only: app quit request.
    Quit,
    /// MPRIS-only: open URI request.
    OpenUri(String),
}
