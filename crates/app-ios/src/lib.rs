//! iOS app shell for SOTF music player.
//!
//! This crate compiles to a static library (.a) that the Xcode project links.
//! The Swift AppDelegate calls `sotf_ios_start()` to launch the GPUI app.
//!
//! Architecture:
//!   Swift AppDelegate → sotf_ios_start() → GPUI app callback → PlayerView
//!   Swift CADisplayLink → gpui_ios_request_frame() → GPUI render tick

#[cfg(any(test, target_os = "ios", target_os = "tvos"))]
mod imp;
#[cfg(test)]
mod tests;
