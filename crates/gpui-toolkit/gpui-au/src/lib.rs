//! macOS Audio Unit platform backend for GPUI.
//!
//! Embeds GPUI rendering inside AUv3 AudioUnit ViewControllers via Metal/wgpu.
//! Follows the same pattern as gpui-ios but adapted for macOS NSView embedding:
//! - Takes an external NSView (from AUViewController) instead of creating a UIWindow
//! - Renders via CAMetalLayer + wgpu (Metal backend)
//! - Frame rendering driven by Swift CVDisplayLink/timer → `gpui_au_request_frame()`
//! - Mouse/keyboard events forwarded from NSView → FFI → GPUI window

#![cfg(target_os = "macos")]
// FFI functions necessarily dereference raw pointers from C callers
#![allow(clippy::not_unsafe_ptr_arg_deref)]

pub use gpui;

mod dispatcher;
mod display;
pub mod ffi;
mod platform;
mod text_system;
mod window;

pub use platform::AuPlatform;
pub(crate) use dispatcher::AuDispatcher;
pub(crate) use display::AuDisplay;
pub(crate) use text_system::AuTextSystem;
pub(crate) use window::AuWindow;
