// ============================================================================
// Camera Capture Module
// ============================================================================
//
// Platform-specific camera capture:
// - macOS: AVFoundation (direct objc2 bindings)
// - Linux/Windows: nokhwa

use crate::HeadTrackerError;
use log::{debug, info};

// Platform-specific implementations
#[cfg(target_os = "macos")]
mod avfoundation;
#[cfg(target_os = "macos")]
use avfoundation::AVFoundationCapture;

/// Camera frame data
#[derive(Clone)]
pub struct CameraFrame {
    /// RGB pixel data (width * height * 3 bytes)
    pub data: Vec<u8>,
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Timestamp in milliseconds
    pub timestamp_ms: u64,
}

impl std::fmt::Debug for CameraFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CameraFrame")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("timestamp_ms", &self.timestamp_ms)
            .field("data_len", &self.data.len())
            .finish()
    }
}

// ============================================================================
// macOS implementation using AVFoundation
// ============================================================================

#[cfg(target_os = "macos")]
pub struct CameraCapture {
    inner: AVFoundationCapture,
    camera_index: usize,
    target_fps: u32,
}

#[cfg(target_os = "macos")]
impl std::fmt::Debug for CameraCapture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CameraCapture")
            .field("camera_index", &self.camera_index)
            .field("target_fps", &self.target_fps)
            .field("is_open", &self.inner.is_running())
            .finish()
    }
}

#[cfg(target_os = "macos")]
impl CameraCapture {
    /// Create a new camera capture instance
    pub fn new(camera_index: usize, target_fps: u32) -> Self {
        Self {
            inner: AVFoundationCapture::new(camera_index),
            camera_index,
            target_fps,
        }
    }

    /// Set target resolution (ignored on macOS - uses camera default)
    pub fn with_resolution(self, _width: u32, _height: u32) -> Self {
        // AVFoundation uses camera's default resolution
        self
    }

    /// Open the camera
    pub fn open(&mut self) -> Result<(), HeadTrackerError> {
        info!(
            "Opening camera {} @ {}fps (macOS AVFoundation)",
            self.camera_index, self.target_fps
        );
        self.inner.start()
    }

    /// Close the camera
    pub fn close(&mut self) {
        debug!("Closing camera");
        self.inner.stop();
    }

    /// Check if camera is open
    pub fn is_open(&self) -> bool {
        self.inner.is_running()
    }

    /// Capture a single frame (blocking with timeout)
    pub fn capture_frame(&mut self) -> Result<CameraFrame, HeadTrackerError> {
        // Wait up to 100ms for a frame
        self.inner.capture_frame(100)
    }

    /// Get current camera resolution
    pub fn resolution(&self) -> Option<(u32, u32)> {
        // Get from latest frame if available
        self.inner.get_frame().map(|f| (f.width, f.height))
    }
}

#[cfg(target_os = "macos")]
impl Drop for CameraCapture {
    fn drop(&mut self) {
        self.close();
    }
}

// ============================================================================
// Linux/Windows implementation using nokhwa
// ============================================================================

#[cfg(not(target_os = "macos"))]
use nokhwa::pixel_format::RgbFormat;
#[cfg(not(target_os = "macos"))]
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
#[cfg(not(target_os = "macos"))]
use nokhwa::Camera;
#[cfg(not(target_os = "macos"))]
use log::{error, warn};

#[cfg(not(target_os = "macos"))]
pub struct CameraCapture {
    camera: Option<Camera>,
    camera_index: usize,
    target_fps: u32,
    target_width: u32,
    target_height: u32,
    is_open: Arc<AtomicBool>,
    start_time_ms: u64,
}

#[cfg(not(target_os = "macos"))]
impl std::fmt::Debug for CameraCapture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CameraCapture")
            .field("camera_index", &self.camera_index)
            .field("target_fps", &self.target_fps)
            .field("is_open", &self.is_open.load(Ordering::Relaxed))
            .finish()
    }
}

#[cfg(not(target_os = "macos"))]
impl CameraCapture {
    /// Create a new camera capture instance
    pub fn new(camera_index: usize, target_fps: u32) -> Self {
        Self {
            camera: None,
            camera_index,
            target_fps,
            target_width: 640,
            target_height: 480,
            is_open: Arc::new(AtomicBool::new(false)),
            start_time_ms: 0,
        }
    }

    /// Set target resolution
    pub fn with_resolution(mut self, width: u32, height: u32) -> Self {
        self.target_width = width;
        self.target_height = height;
        self
    }

    /// Open the camera
    pub fn open(&mut self) -> Result<(), HeadTrackerError> {
        if self.is_open.load(Ordering::Relaxed) {
            return Err(HeadTrackerError::AlreadyRunning);
        }

        info!(
            "Opening camera {} at {}x{} @ {}fps (nokhwa)",
            self.camera_index, self.target_width, self.target_height, self.target_fps
        );

        let index = CameraIndex::Index(self.camera_index as u32);
        let requested =
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);

        let camera = Camera::new(index, requested).map_err(|e| {
            error!("Failed to create camera: {}", e);
            HeadTrackerError::Camera(e.to_string())
        })?;

        info!("Camera opened: {:?}", camera.camera_format());

        self.start_time_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        self.camera = Some(camera);
        self.is_open.store(true, Ordering::Relaxed);

        Ok(())
    }

    /// Close the camera
    pub fn close(&mut self) {
        if let Some(mut camera) = self.camera.take() {
            debug!("Closing camera");
            let _ = camera.stop_stream();
        }
        self.is_open.store(false, Ordering::Relaxed);
    }

    /// Check if camera is open
    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::Relaxed)
    }

    /// Capture a single frame
    pub fn capture_frame(&mut self) -> Result<CameraFrame, HeadTrackerError> {
        let camera = self.camera.as_mut().ok_or(HeadTrackerError::NotStarted)?;

        let frame = camera.frame().map_err(|e| {
            warn!("Frame capture error: {}", e);
            HeadTrackerError::Camera(e.to_string())
        })?;

        let decoded = frame.decode_image::<RgbFormat>().map_err(|e| {
            warn!("Frame decode error: {}", e);
            HeadTrackerError::Camera(e.to_string())
        })?;

        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
            - self.start_time_ms;

        Ok(CameraFrame {
            data: decoded.into_raw(),
            width: frame.resolution().width(),
            height: frame.resolution().height(),
            timestamp_ms,
        })
    }

    /// Get current camera resolution
    pub fn resolution(&self) -> Option<(u32, u32)> {
        self.camera.as_ref().map(|c| {
            let fmt = c.camera_format();
            (fmt.resolution().width(), fmt.resolution().height())
        })
    }
}

#[cfg(not(target_os = "macos"))]
impl Drop for CameraCapture {
    fn drop(&mut self) {
        self.close();
    }
}

// ============================================================================
// Common functions
// ============================================================================

/// List available cameras
#[cfg(not(target_os = "macos"))]
pub fn list_cameras() -> Vec<String> {
    use log::warn;
    match nokhwa::query(nokhwa::utils::ApiBackend::Auto) {
        Ok(cameras) => cameras
            .iter()
            .map(|c| format!("{}: {}", c.index(), c.human_name()))
            .collect(),
        Err(e) => {
            warn!("Failed to query cameras: {}", e);
            Vec::new()
        }
    }
}

#[cfg(target_os = "macos")]
pub fn list_cameras() -> Vec<String> {
    // AVFoundation doesn't easily enumerate - just return default
    vec!["0: Default Camera".to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_capture_creation() {
        let capture = CameraCapture::new(0, 30);
        assert!(!capture.is_open());
    }

    #[test]
    fn test_camera_frame_debug() {
        let frame = CameraFrame {
            data: vec![0u8; 640 * 480 * 3],
            width: 640,
            height: 480,
            timestamp_ms: 1000,
        };
        let debug_str = format!("{:?}", frame);
        assert!(debug_str.contains("640"));
        assert!(debug_str.contains("480"));
    }
}
