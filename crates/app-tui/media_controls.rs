use sotf_media_controls::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use std::sync::mpsc;
use std::time::Duration;

pub struct TuiMediaControls {
    controls: MediaControls,
    rx: mpsc::Receiver<MediaControlEvent>,
}

impl TuiMediaControls {
    pub fn new() -> Result<Self, sotf_media_controls::Error> {
        #[cfg(target_os = "macos")]
        init_macos_app();

        let config = PlatformConfig {
            dbus_name: "sotf_player",
            display_name: "SOTF Player",
            hwnd: get_hwnd(),
        };

        let mut controls = MediaControls::new(config)?;

        let (tx, rx) = mpsc::channel();
        controls.attach(move |event: MediaControlEvent| {
            // Best effort — if receiver dropped, we just ignore
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

/// On macOS, initialize NSApplication with Accessory activation policy
/// so MPRemoteCommandCenter events are delivered to our process.
#[cfg(target_os = "macos")]
fn init_macos_app() {
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

    // SAFETY: TUI main() runs on the main thread.
    let mtm = unsafe { objc2::MainThreadMarker::new_unchecked() };
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app.finishLaunching();
}

/// Pump the macOS run loop so that pending OS events — including media key
/// callbacks registered by sotf_media_controls via MPRemoteCommandCenter — are dispatched.
///
/// MPRemoteCommandCenter delivers events through CoreFoundation run loop
/// sources, NOT through the NSApplication event queue. We must call
/// CFRunLoopRunInMode to process those sources. We also drain NSApp events
/// for completeness (other Cocoa callbacks may depend on them).
///
/// Processes all queued events without blocking. No-op on non-macOS platforms.
#[cfg(target_os = "macos")]
pub fn pump_macos_event_loop() {
    use core_foundation_sys::runloop::{CFRunLoopRunInMode, kCFRunLoopDefaultMode};
    use objc2_app_kit::{NSApplication, NSEventMask};
    use objc2_foundation::{NSDate, NSDefaultRunLoopMode};

    // 1. Process CoreFoundation run loop sources (MPRemoteCommandCenter events).
    //    returnAfterSourceHandled=1 processes one source per call; loop to drain all.
    //    Timeout 0.0 means return immediately if no sources are ready.
    // SAFETY: FFI call with valid static string and numeric args.
    loop {
        let result = unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.0, 1) };
        // kCFRunLoopRunHandledSource = 4 — a source was processed, check for more
        if result != 4 {
            break;
        }
    }

    // 2. Drain NSApplication event queue (other Cocoa callbacks).
    // SAFETY: Called on the main thread from the TUI event loop.
    let mtm = unsafe { objc2::MainThreadMarker::new_unchecked() };
    let app = NSApplication::sharedApplication(mtm);

    let distant_past = NSDate::distantPast();
    // SAFETY: NSDefaultRunLoopMode is a valid static string constant.
    let mode = unsafe { NSDefaultRunLoopMode };

    loop {
        let event = app.nextEventMatchingMask_untilDate_inMode_dequeue(
            NSEventMask::Any,
            Some(&distant_past),
            mode,
            true,
        );
        match event {
            Some(event) => app.sendEvent(&event),
            None => break,
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn pump_macos_event_loop() {
    // No-op on non-macOS platforms.
}

/// On Windows, sotf_media_controls requires a valid HWND. Use the console window handle.
#[cfg(target_os = "windows")]
fn get_hwnd() -> Option<*mut core::ffi::c_void> {
    unsafe extern "system" {
        fn GetConsoleWindow() -> *mut core::ffi::c_void;
    }
    let h = unsafe { GetConsoleWindow() };
    if h.is_null() { None } else { Some(h) }
}

#[cfg(not(target_os = "windows"))]
fn get_hwnd() -> Option<*mut core::ffi::c_void> {
    None
}
