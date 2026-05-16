use core::marker::PhantomData;
use std::time::Duration;

/// Opaque, lifetime-bound window handle.
///
/// SAFETY: On Windows this is a raw `HWND` (an `*mut c_void`). Raw pointers
/// are `!Send + !Sync` by default. We wrap them in this newtype with an
/// explicit `Send` impl so the value can be moved into the backend
/// constructor — but only `unsafe` callers can construct one, because:
///
/// 1. The pointer must remain valid for the lifetime of the `MediaControls`
///    handle (tracked at compile time via the `'a` phantom lifetime).
/// 2. The pointer must be safe to use from whichever thread the backend
///    operates the SMTC singleton from.
///
/// On macOS / Linux the contained pointer is never dereferenced.
#[derive(Debug, Clone, Copy)]
pub struct WindowHandle<'a> {
    raw: *mut core::ffi::c_void,
    _lifetime: PhantomData<&'a ()>,
}

impl WindowHandle<'_> {
    /// Construct a `WindowHandle` from a raw HWND.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `raw` remains valid for the lifetime
    /// `'a`, and that it is safe to access from the thread on which the
    /// platform backend (SMTC on Windows) operates.
    #[must_use]
    pub unsafe fn from_raw(raw: *mut core::ffi::c_void) -> Self {
        Self {
            raw,
            _lifetime: PhantomData,
        }
    }

    /// Returns the underlying raw pointer.
    #[must_use]
    pub fn as_raw(self) -> *mut core::ffi::c_void {
        self.raw
    }
}

// SAFETY: The pointer is `Send`-by-convention — callers guarantee
// thread-safety as documented on `from_raw`.
unsafe impl Send for WindowHandle<'_> {}
// SAFETY: same justification as `Send`.
unsafe impl Sync for WindowHandle<'_> {}

/// Platform-specific construction config.
///
/// Field-compatible with `souvlaki::PlatformConfig` for migration.
#[derive(Debug, Clone, Default)]
pub struct PlatformConfig<'a> {
    /// D-Bus name suffix on Linux. Ignored on other platforms.
    pub dbus_name: &'a str,
    /// User-visible application name.
    pub display_name: &'a str,
    /// Windows HWND. Ignored on macOS / Linux; currently unused because the
    /// Windows backend is a no-op.
    ///
    /// SAFETY contract for the caller (when SMTC ships): the pointer must
    /// remain valid for the lifetime of the resulting `MediaControls`, and
    /// it must be safe to access from the thread on which the backend
    /// operates the SMTC singleton. Prefer
    /// [`PlatformConfig::with_window_handle`] (which captures the lifetime
    /// via [`WindowHandle`]'s phantom) over a raw-pointer literal.
    pub hwnd: Option<*mut core::ffi::c_void>,
}

impl<'a> PlatformConfig<'a> {
    /// Builder-style constructor that ties the HWND to the `'a` lifetime
    /// of the returned [`PlatformConfig`] via [`WindowHandle`].
    ///
    /// Captures the SAFETY preconditions at the construction site and
    /// prevents `PlatformConfig` from outliving the window the HWND came
    /// from.
    #[must_use]
    pub fn with_window_handle(mut self, handle: WindowHandle<'a>) -> Self {
        self.hwnd = Some(handle.as_raw());
        self
    }
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
