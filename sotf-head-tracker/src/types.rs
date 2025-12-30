// ============================================================================
// Core Types for Head Tracking
// ============================================================================

use thiserror::Error;

/// Head position in the acoustic space
///
/// Coordinates are relative to the nominal listening position:
/// - Origin (0, 0, 0) = calibrated center position
/// - X axis: positive = right
/// - Y axis: positive = up
/// - Z axis: positive = forward (toward speakers)
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeadPosition {
    /// Lateral offset from center (meters, + = right)
    pub x: f32,

    /// Vertical offset (meters, + = up)
    pub y: f32,

    /// Depth offset from nominal position (meters, + = forward)
    pub z: f32,

    /// Yaw rotation (degrees, + = looking right)
    pub yaw: f32,

    /// Pitch rotation (degrees, + = looking up)
    pub pitch: f32,

    /// Roll rotation (degrees, + = tilting head right)
    pub roll: f32,

    /// Timestamp in milliseconds (monotonic clock)
    pub timestamp_ms: u64,

    /// Detection confidence (0.0 - 1.0)
    pub confidence: f32,
}

impl HeadPosition {
    /// Create a new head position at origin
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a position with just x/z offset (most common for XTC)
    pub fn from_xz(x: f32, z: f32) -> Self {
        Self {
            x,
            z,
            confidence: 1.0,
            ..Default::default()
        }
    }

    /// Check if position has changed significantly from another
    pub fn significantly_different(&self, other: &Self, pos_threshold_m: f32, angle_threshold_deg: f32) -> bool {
        (self.x - other.x).abs() > pos_threshold_m
            || (self.y - other.y).abs() > pos_threshold_m
            || (self.z - other.z).abs() > pos_threshold_m
            || (self.yaw - other.yaw).abs() > angle_threshold_deg
            || (self.pitch - other.pitch).abs() > angle_threshold_deg
            || (self.roll - other.roll).abs() > angle_threshold_deg
    }

    /// Linear interpolation between two positions
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            x: self.x + t * (other.x - self.x),
            y: self.y + t * (other.y - self.y),
            z: self.z + t * (other.z - self.z),
            yaw: self.yaw + t * (other.yaw - self.yaw),
            pitch: self.pitch + t * (other.pitch - self.pitch),
            roll: self.roll + t * (other.roll - self.roll),
            timestamp_ms: other.timestamp_ms,
            confidence: self.confidence + t * (other.confidence - self.confidence),
        }
    }
}

/// Configuration for head tracking
#[derive(Clone, Debug)]
pub struct HeadTrackerConfig {
    /// Enable head tracking
    pub enabled: bool,

    /// Target frame rate for vision processing (default: 30)
    pub target_fps: u32,

    /// Smoothing time constant in seconds (default: 0.1)
    /// Higher = smoother but more latency
    pub smoothing_time_s: f32,

    /// Minimum confidence to accept detection (default: 0.5)
    pub min_confidence: f32,

    /// Camera device index (default: 0 = first camera)
    pub camera_index: usize,

    /// Calibration: distance from camera to nominal head position (meters)
    pub camera_distance_m: f32,

    /// Calibration: camera horizontal field of view (degrees)
    pub camera_fov_deg: f32,

    /// Position change threshold to trigger update (meters)
    pub position_threshold_m: f32,

    /// Angle change threshold to trigger update (degrees)
    pub angle_threshold_deg: f32,

    /// Number of frames to hold position when face is lost
    pub lost_face_hold_frames: u32,
}

impl Default for HeadTrackerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            target_fps: 30,
            smoothing_time_s: 0.1,
            min_confidence: 0.5,
            camera_index: 0,
            camera_distance_m: 0.6, // Typical laptop webcam distance
            camera_fov_deg: 60.0,   // Typical webcam FOV
            position_threshold_m: 0.01,  // 1cm
            angle_threshold_deg: 1.0,    // 1 degree
            lost_face_hold_frames: 10,   // ~333ms at 30fps
        }
    }
}

/// Errors that can occur during head tracking
#[derive(Error, Debug)]
pub enum HeadTrackerError {
    #[error("Camera error: {0}")]
    Camera(String),

    #[error("Vision processing error: {0}")]
    Vision(String),

    #[error("No camera found")]
    NoCameraFound,

    #[error("Camera already in use")]
    CameraInUse,

    #[error("Tracking not started")]
    NotStarted,

    #[error("Tracking already running")]
    AlreadyRunning,

    #[error("Calibration required")]
    CalibrationRequired,

    #[error("Platform not supported: {0}")]
    PlatformNotSupported(String),
}

/// Calibration data for mapping camera coordinates to acoustic space
#[derive(Clone, Debug, Default)]
pub struct CalibrationData {
    /// Reference face bounding box (normalized 0-1)
    pub reference_face_rect: FaceRect,

    /// Reference face size (used for distance estimation)
    pub reference_face_size: f32,

    /// Pixels per meter at reference distance
    pub pixels_per_meter: f32,

    /// Camera resolution
    pub camera_width: u32,
    pub camera_height: u32,

    /// Is calibration valid?
    pub is_valid: bool,
}

/// Normalized face bounding box (0-1 coordinates)
#[derive(Clone, Copy, Debug, Default)]
pub struct FaceRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl FaceRect {
    /// Center X coordinate
    pub fn center_x(&self) -> f32 {
        self.x + self.width / 2.0
    }

    /// Center Y coordinate
    pub fn center_y(&self) -> f32 {
        self.y + self.height / 2.0
    }

    /// Area (for comparing face sizes)
    pub fn area(&self) -> f32 {
        self.width * self.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_head_position_default() {
        let pos = HeadPosition::default();
        assert_eq!(pos.x, 0.0);
        assert_eq!(pos.y, 0.0);
        assert_eq!(pos.z, 0.0);
        assert_eq!(pos.confidence, 0.0);
    }

    #[test]
    fn test_head_position_from_xz() {
        let pos = HeadPosition::from_xz(0.1, -0.05);
        assert_eq!(pos.x, 0.1);
        assert_eq!(pos.z, -0.05);
        assert_eq!(pos.y, 0.0);
        assert_eq!(pos.confidence, 1.0);
    }

    #[test]
    fn test_significantly_different() {
        let pos1 = HeadPosition::from_xz(0.0, 0.0);
        let pos2 = HeadPosition::from_xz(0.02, 0.0);
        let pos3 = HeadPosition::from_xz(0.005, 0.0);

        assert!(pos1.significantly_different(&pos2, 0.01, 1.0));
        assert!(!pos1.significantly_different(&pos3, 0.01, 1.0));
    }

    #[test]
    fn test_lerp() {
        let pos1 = HeadPosition::from_xz(0.0, 0.0);
        let pos2 = HeadPosition::from_xz(1.0, 2.0);

        let mid = pos1.lerp(&pos2, 0.5);
        assert!((mid.x - 0.5).abs() < 0.001);
        assert!((mid.z - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_face_rect() {
        let rect = FaceRect {
            x: 0.25,
            y: 0.25,
            width: 0.5,
            height: 0.5,
        };
        assert!((rect.center_x() - 0.5).abs() < 0.001);
        assert!((rect.center_y() - 0.5).abs() < 0.001);
        assert!((rect.area() - 0.25).abs() < 0.001);
    }
}
