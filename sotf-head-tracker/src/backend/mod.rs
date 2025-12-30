// ============================================================================
// Vision Backend Module
// ============================================================================
//
// Platform-specific face detection backends.

#[cfg(target_os = "macos")]
mod macos_vision;

#[cfg(target_os = "macos")]
pub use macos_vision::{MacOSVisionBackend, FaceDetection};
