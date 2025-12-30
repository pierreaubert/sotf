// ============================================================================
// sotf-head-tracker: Camera-based head tracking for spatial audio
// ============================================================================
//
// This crate provides real-time head position tracking via webcam for use
// with the XTC (Crosstalk Cancellation) and Binaural audio plugins.
//
// Architecture:
// - Camera thread captures frames at 30-60 FPS
// - Vision thread processes frames for face detection
// - Lock-free queue transfers HeadPosition to audio thread
// - Smoothing filter reduces jitter
//
// Platform support:
// - macOS: Apple Vision framework (primary, lowest latency)
// - Cross-platform: ONNX Runtime + MediaPipe (optional)

mod types;
pub use types::{CalibrationData, FaceRect, HeadPosition, HeadTrackerConfig, HeadTrackerError};

mod smoother;
pub use smoother::HeadPositionSmoother;

pub mod camera;

pub mod integration;
pub use integration::{HeadTrackingBridge, HeadTrackingTarget, XtcHeadParams};

#[cfg(target_os = "macos")]
#[cfg(feature = "macos-vision")]
pub mod backend;

#[cfg(target_os = "macos")]
#[cfg(feature = "macos-vision")]
mod tracker;
#[cfg(target_os = "macos")]
#[cfg(feature = "macos-vision")]
pub use tracker::HeadTracker;

// Re-export for convenience
pub use crossbeam::queue::ArrayQueue;
