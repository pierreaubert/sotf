//! Camera capture module
//!
//! Provides interfaces for capturing frames from cameras (webcam, phone camera)
//! using OpenCV's VideoCapture API.

use crate::error::{ScannerError, ScannerResult};
use opencv::{
    core::{Mat, Size, Vector},
    imgproc,
    prelude::*,
    videoio::{self, VideoCapture, VideoCaptureTrait},
};
use parking_lot::Mutex;

/// A frame captured from the camera
#[derive(Debug, Clone)]
pub struct Frame {
    /// The image data (OpenCV Mat)
    pub(crate) data: Mat,

    /// Frame width in pixels
    pub width: u32,

    /// Frame height in pixels
    pub height: u32,

    /// Timestamp (milliseconds since start)
    pub timestamp: u64,
}

impl Frame {
    /// Create a new frame from OpenCV Mat
    pub fn new(data: Mat, timestamp: u64) -> ScannerResult<Self> {
        let size = data.size()?;
        Ok(Self {
            data,
            width: size.width as u32,
            height: size.height as u32,
            timestamp,
        })
    }

    /// Get the raw OpenCV Mat data
    pub fn mat(&self) -> &Mat {
        &self.data
    }

    /// Convert frame to RGB format
    pub fn to_rgb(&self) -> ScannerResult<Mat> {
        let mut rgb = Mat::default();
        imgproc::cvt_color(&self.data, &mut rgb, imgproc::COLOR_BGR2RGB, 0)?;
        Ok(rgb)
    }

    /// Resize the frame
    pub fn resize(&self, width: u32, height: u32) -> ScannerResult<Self> {
        let mut resized = Mat::default();
        let size = Size::new(width as i32, height as i32);
        imgproc::resize(
            &self.data,
            &mut resized,
            size,
            0.0,
            0.0,
            imgproc::INTER_LINEAR,
        )?;

        Self::new(resized, self.timestamp)
    }

    /// Convert frame to grayscale
    pub fn to_gray(&self) -> ScannerResult<Mat> {
        let mut gray = Mat::default();
        imgproc::cvt_color(&self.data, &mut gray, imgproc::COLOR_BGR2GRAY, 0)?;
        Ok(gray)
    }
}

/// Camera interface for capturing video frames
pub struct Camera {
    /// OpenCV video capture device (wrapped in Mutex for interior mutability)
    capture: Mutex<VideoCapture>,

    /// Device index
    device_index: u32,

    /// Start time (for timestamps)
    start_time: std::time::Instant,
}

impl Camera {
    /// Open a camera device with specified parameters
    ///
    /// # Arguments
    /// * `device_index` - Camera device index (0 for default camera)
    /// * `width` - Desired frame width
    /// * `height` - Desired frame height
    /// * `fps` - Desired frame rate
    pub fn new(device_index: u32, width: u32, height: u32, fps: u32) -> ScannerResult<Self> {
        let mut capture = VideoCapture::new(device_index as i32, videoio::CAP_ANY)
            .map_err(|e| ScannerError::Camera(format!("Failed to open camera: {}", e)))?;

        if !capture.is_opened()? {
            return Err(ScannerError::Camera(format!(
                "Camera {} could not be opened",
                device_index
            )));
        }

        // Set camera properties
        capture.set(videoio::CAP_PROP_FRAME_WIDTH, width as f64)?;
        capture.set(videoio::CAP_PROP_FRAME_HEIGHT, height as f64)?;
        capture.set(videoio::CAP_PROP_FPS, fps as f64)?;

        // Enable auto-focus if available
        let _ = capture.set(videoio::CAP_PROP_AUTOFOCUS, 1.0);

        Ok(Self {
            capture: Mutex::new(capture),
            device_index,
            start_time: std::time::Instant::now(),
        })
    }

    /// Capture a single frame from the camera
    pub fn capture_frame(&self) -> ScannerResult<Frame> {
        let mut mat = Mat::default();

        // Read frame from camera (requires mutable access via Mutex)
        {
            let mut capture_guard = self.capture.lock();
            capture_guard
                .read(&mut mat)
                .map_err(|e| ScannerError::Camera(format!("Failed to read frame: {}", e)))?;
        }

        if mat.empty() {
            return Err(ScannerError::Camera("Captured frame is empty".to_string()));
        }

        let timestamp = self.start_time.elapsed().as_millis() as u64;
        Frame::new(mat, timestamp)
    }

    /// Get the actual frame width
    pub fn get_width(&self) -> ScannerResult<u32> {
        let capture = self.capture.lock();
        Ok(capture.get(videoio::CAP_PROP_FRAME_WIDTH)? as u32)
    }

    /// Get the actual frame height
    pub fn get_height(&self) -> ScannerResult<u32> {
        let capture = self.capture.lock();
        Ok(capture.get(videoio::CAP_PROP_FRAME_HEIGHT)? as u32)
    }

    /// Get the actual frame rate
    pub fn get_fps(&self) -> ScannerResult<u32> {
        let capture = self.capture.lock();
        Ok(capture.get(videoio::CAP_PROP_FPS)? as u32)
    }

    /// Get the device index
    pub fn device_index(&self) -> u32 {
        self.device_index
    }

    /// Check if the camera is opened
    pub fn is_opened(&self) -> bool {
        let capture = self.capture.lock();
        capture.is_opened().unwrap_or(false)
    }

    /// Release the camera
    pub fn release(&self) -> ScannerResult<()> {
        let mut capture = self.capture.lock();
        capture
            .release()
            .map_err(|e| ScannerError::Camera(format!("Failed to release camera: {}", e)))
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        // Release camera resources on drop
        let _ = self.release();
    }
}

/// List available camera devices
pub fn list_cameras() -> Vec<u32> {
    let mut cameras = Vec::new();

    // Try to open cameras 0-9
    for i in 0..10 {
        if let Ok(capture) = VideoCapture::new(i, videoio::CAP_ANY) {
            if capture.is_opened().unwrap_or(false) {
                cameras.push(i as u32);
            }
        }
    }

    cameras
}

/// Get information about a camera device
pub fn get_camera_info(device_index: u32) -> ScannerResult<CameraInfo> {
    let capture = VideoCapture::new(device_index as i32, videoio::CAP_ANY)?;

    if !capture.is_opened()? {
        return Err(ScannerError::Camera(format!(
            "Camera {} not available",
            device_index
        )));
    }

    let width = capture.get(videoio::CAP_PROP_FRAME_WIDTH)? as u32;
    let height = capture.get(videoio::CAP_PROP_FRAME_HEIGHT)? as u32;
    let fps = capture.get(videoio::CAP_PROP_FPS)? as u32;

    Ok(CameraInfo {
        device_index,
        width,
        height,
        fps,
    })
}

/// Information about a camera device
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CameraInfo {
    pub device_index: u32,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_cameras() {
        let cameras = list_cameras();
        println!("Available cameras: {:?}", cameras);
        // This test just ensures the function doesn't crash
    }

    #[test]
    #[ignore] // Ignore by default as it requires a camera
    fn test_camera_capture() {
        let cameras = list_cameras();
        if cameras.is_empty() {
            println!("No cameras available, skipping test");
            return;
        }

        let camera = Camera::new(cameras[0], 640, 480, 30);
        assert!(camera.is_ok());

        let camera = camera.unwrap();
        assert!(camera.is_opened());

        let frame = camera.capture_frame();
        assert!(frame.is_ok());

        let frame = frame.unwrap();
        assert!(frame.width > 0);
        assert!(frame.height > 0);
    }
}
