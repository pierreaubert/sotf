//! Automatic camera calibration using checkerboard patterns
//!
//! This module implements Zhang's calibration method for automatic
//! camera intrinsic parameter estimation.

use crate::camera::Frame;
use crate::error::{ScannerError, ScannerResult};
use crate::reconstruction::CameraIntrinsics;
use nalgebra::{DMatrix, DVector, Matrix3, Point2, Point3, Vector3};
use opencv::{
    calib3d, core,
    core::{Point2f, Point3f, Size, Vector},
    imgproc,
    prelude::*,
};

/// Checkerboard pattern for calibration
#[derive(Debug, Clone)]
pub struct CheckerboardPattern {
    /// Number of inner corners in width
    pub width: i32,

    /// Number of inner corners in height
    pub height: i32,

    /// Size of each square in real-world units (e.g., mm or cm)
    pub square_size: f32,
}

impl Default for CheckerboardPattern {
    fn default() -> Self {
        Self {
            width: 9, // 9x6 is common for calibration
            height: 6,
            square_size: 25.0, // 25mm squares
        }
    }
}

impl CheckerboardPattern {
    /// Create a new checkerboard pattern
    pub fn new(width: i32, height: i32, square_size: f32) -> Self {
        Self {
            width,
            height,
            square_size,
        }
    }

    /// Get the pattern size
    pub fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    /// Generate 3D object points for the checkerboard
    pub fn object_points(&self) -> Vec<Point3f> {
        let mut points = Vec::new();
        for j in 0..self.height {
            for i in 0..self.width {
                points.push(Point3f::new(
                    i as f32 * self.square_size,
                    j as f32 * self.square_size,
                    0.0,
                ));
            }
        }
        points
    }
}

/// Calibration result containing camera parameters
#[derive(Debug, Clone)]
pub struct CalibrationResult {
    /// Camera intrinsic parameters
    pub intrinsics: CameraIntrinsics,

    /// Reprojection error (RMS)
    pub rms_error: f64,

    /// Number of frames used
    pub num_frames: usize,

    /// Image size used for calibration
    pub image_size: (u32, u32),
}

/// Camera calibrator
pub struct CameraCalibrator {
    pattern: CheckerboardPattern,
    object_points: Vec<Vector<Point3f>>,
    image_points: Vec<Vector<Point2f>>,
    image_size: Option<Size>,
}

impl CameraCalibrator {
    /// Create a new camera calibrator
    pub fn new(pattern: CheckerboardPattern) -> Self {
        Self {
            pattern,
            object_points: Vec::new(),
            image_points: Vec::new(),
            image_size: None,
        }
    }

    /// Create with default checkerboard pattern
    pub fn default() -> Self {
        Self::new(CheckerboardPattern::default())
    }

    /// Detect checkerboard corners in a frame
    pub fn detect_corners(&self, frame: &Frame) -> ScannerResult<Option<Vec<Point2f>>> {
        let gray = frame.to_gray()?;
        let mut corners = Vector::<Point2f>::new();

        // Find checkerboard corners
        let found = calib3d::find_chessboard_corners(
            &gray,
            self.pattern.size(),
            &mut corners,
            calib3d::CALIB_CB_ADAPTIVE_THRESH
                | calib3d::CALIB_CB_NORMALIZE_IMAGE
                | calib3d::CALIB_CB_FAST_CHECK,
        )?;

        if !found {
            return Ok(None);
        }

        // Refine corner positions to sub-pixel accuracy
        let term_criteria =
            core::TermCriteria::new(core::TermCriteria_EPS + core::TermCriteria_COUNT, 30, 0.001)?;

        imgproc::corner_sub_pix(
            &gray,
            &mut corners,
            Size::new(11, 11),
            Size::new(-1, -1),
            term_criteria,
        )?;

        Ok(Some(corners.to_vec()))
    }

    /// Add a calibration frame
    pub fn add_frame(&mut self, frame: &Frame) -> ScannerResult<bool> {
        // Store image size from first frame
        if self.image_size.is_none() {
            self.image_size = Some(Size::new(frame.width as i32, frame.height as i32));
        }

        // Detect corners
        if let Some(corners) = self.detect_corners(frame)? {
            // Add object points (same for all frames)
            let obj_points = self.pattern.object_points();
            self.object_points.push(Vector::from_iter(obj_points));

            // Add image points
            self.image_points.push(Vector::from_iter(corners));

            log::info!(
                "Calibration frame added ({} total)",
                self.object_points.len()
            );
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get number of calibration frames collected
    pub fn frame_count(&self) -> usize {
        self.object_points.len()
    }

    /// Check if enough frames have been collected
    pub fn has_enough_frames(&self) -> bool {
        self.frame_count() >= 10 // Minimum 10 frames recommended
    }

    /// Calibrate the camera
    pub fn calibrate(&self) -> ScannerResult<CalibrationResult> {
        if !self.has_enough_frames() {
            return Err(ScannerError::InvalidConfig(format!(
                "Not enough calibration frames: {} (minimum 10 required)",
                self.frame_count()
            )));
        }

        let image_size = self
            .image_size
            .ok_or_else(|| ScannerError::InvalidConfig("No image size available".to_string()))?;

        // Prepare output matrices
        let mut camera_matrix = Mat::default();
        let mut dist_coeffs = Mat::default();
        let mut rvecs = Vector::<Mat>::new();
        let mut tvecs = Vector::<Mat>::new();

        // Convert to proper OpenCV types
        let obj_points_vec: Vector<Vector<Point3f>> =
            Vector::from_iter(self.object_points.iter().cloned());
        let img_points_vec: Vector<Vector<Point2f>> =
            Vector::from_iter(self.image_points.iter().cloned());

        // Run calibration
        let rms_error = calib3d::calibrate_camera(
            &obj_points_vec,
            &img_points_vec,
            image_size,
            &mut camera_matrix,
            &mut dist_coeffs,
            &mut rvecs,
            &mut tvecs,
            0, // flags
            core::TermCriteria::new(core::TermCriteria_EPS + core::TermCriteria_COUNT, 30, 1e-6)?,
        )?;

        // Extract camera parameters
        let fx = *camera_matrix.at_2d::<f64>(0, 0)?;
        let fy = *camera_matrix.at_2d::<f64>(1, 1)?;
        let cx = *camera_matrix.at_2d::<f64>(0, 2)?;
        let cy = *camera_matrix.at_2d::<f64>(1, 2)?;

        // Extract distortion coefficients
        let k1 = *dist_coeffs.at::<f64>(0)?;
        let k2 = *dist_coeffs.at::<f64>(1)?;
        let p1 = *dist_coeffs.at::<f64>(2)?;
        let p2 = *dist_coeffs.at::<f64>(3)?;
        let k3 = *dist_coeffs.at::<f64>(4)?;

        let intrinsics = CameraIntrinsics {
            fx: fx as f32,
            fy: fy as f32,
            cx: cx as f32,
            cy: cy as f32,
            distortion: Some([k1 as f32, k2 as f32, p1 as f32, p2 as f32, k3 as f32]),
        };

        log::info!("Camera calibration complete:");
        log::info!("  fx={:.2}, fy={:.2}", fx, fy);
        log::info!("  cx={:.2}, cy={:.2}", cx, cy);
        log::info!("  RMS error: {:.4}", rms_error);

        Ok(CalibrationResult {
            intrinsics,
            rms_error,
            num_frames: self.frame_count(),
            image_size: (image_size.width as u32, image_size.height as u32),
        })
    }

    /// Draw detected corners on a frame for visualization
    pub fn draw_corners(&self, frame: &Frame, corners: &[Point2f]) -> ScannerResult<Mat> {
        let mut display = frame.mat().try_clone()?;

        let corners_vec: Vector<Point2f> = Vector::from_iter(corners.iter().cloned());
        calib3d::draw_chessboard_corners(&mut display, self.pattern.size(), &corners_vec, true)?;

        Ok(display)
    }

    /// Reset calibration data
    pub fn reset(&mut self) {
        self.object_points.clear();
        self.image_points.clear();
        self.image_size = None;
    }
}

/// Interactive calibration session
pub struct CalibrationSession {
    calibrator: CameraCalibrator,
    min_frames: usize,
    max_frames: usize,
}

impl CalibrationSession {
    /// Create a new calibration session
    pub fn new(pattern: CheckerboardPattern) -> Self {
        Self {
            calibrator: CameraCalibrator::new(pattern),
            min_frames: 10,
            max_frames: 30,
        }
    }

    /// Create with default settings
    pub fn default() -> Self {
        Self::new(CheckerboardPattern::default())
    }

    /// Set minimum number of frames
    pub fn with_min_frames(mut self, min: usize) -> Self {
        self.min_frames = min;
        self
    }

    /// Set maximum number of frames
    pub fn with_max_frames(mut self, max: usize) -> Self {
        self.max_frames = max;
        self
    }

    /// Process a frame and add if checkerboard is detected
    pub fn process_frame(&mut self, frame: &Frame) -> ScannerResult<bool> {
        if self.calibrator.frame_count() >= self.max_frames {
            return Ok(false);
        }

        self.calibrator.add_frame(frame)
    }

    /// Check if calibration is ready
    pub fn is_ready(&self) -> bool {
        self.calibrator.frame_count() >= self.min_frames
    }

    /// Check if maximum frames reached
    pub fn is_complete(&self) -> bool {
        self.calibrator.frame_count() >= self.max_frames
    }

    /// Get progress (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        (self.calibrator.frame_count() as f32 / self.min_frames as f32).min(1.0)
    }

    /// Get frame count
    pub fn frame_count(&self) -> usize {
        self.calibrator.frame_count()
    }

    /// Calibrate and return result
    pub fn calibrate(&self) -> ScannerResult<CalibrationResult> {
        self.calibrator.calibrate()
    }

    /// Detect corners in current frame
    pub fn detect_corners(&self, frame: &Frame) -> ScannerResult<Option<Vec<Point2f>>> {
        self.calibrator.detect_corners(frame)
    }

    /// Draw corners on frame
    pub fn draw_corners(&self, frame: &Frame, corners: &[Point2f]) -> ScannerResult<Mat> {
        self.calibrator.draw_corners(frame, corners)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkerboard_pattern() {
        let pattern = CheckerboardPattern::new(9, 6, 25.0);
        assert_eq!(pattern.width, 9);
        assert_eq!(pattern.height, 6);

        let points = pattern.object_points();
        assert_eq!(points.len(), 54); // 9 * 6
    }

    #[test]
    fn test_calibrator_creation() {
        let calibrator = CameraCalibrator::default();
        assert_eq!(calibrator.frame_count(), 0);
        assert!(!calibrator.has_enough_frames());
    }
}
