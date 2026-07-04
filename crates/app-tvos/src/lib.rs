//! tvOS (Apple TV) app shell for SOTF music player.
//!
//! This crate compiles to a static library (.a) that the Xcode project links.
//! The Swift AppDelegate calls `sotf_tvos_start()` to launch the GPUI app.
//!
//! Architecture:
//!   Swift AppDelegate → sotf_tvos_start() → GPUI app callback → PlayerView
//!   Swift CADisplayLink → gpui_ios_request_current_frame() → GPUI render tick
//!
//! Key differences from iOS:
//!   - No document picker (no local file import)
//!   - No MPNowPlayingInfoCenter (tvOS doesn't support lock-screen metadata)
//!   - Input via Siri Remote (focus engine + button presses) instead of touch
//!   - Larger default font scale for 10-foot viewing distance

#[cfg(any(target_os = "ios", target_os = "tvos"))]
mod imp;
