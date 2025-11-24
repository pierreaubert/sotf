//! Stereo camera support for improved depth estimation
//!
//! This module implements stereo vision algorithms to compute depth from
//! two camera views, providing much better depth accuracy than monocular
//! depth estimation.

use crate::camera::Frame;
use crate::error::{ScannerError, ScannerResult};
use crate::reconstruction::CameraIntrinsics;
use nalgebra::{Matrix3, Point2, Point3, Vector3};
use opencv::{
    calib3d,
    core::{Mat, Point2f, Scalar, Size, Vector},
    imgproc,
    prelude::*,
};
use serde::{Deserialize, Serialize};

/// Stereo camera configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StereoConfig {
    /// Left camera intrinsics
    pub left_intrinsics: CameraIntrinsics,

    /// Right camera intrinsics
    pub right_intrinsics: CameraIntrinsics,

    /// Baseline distance between cameras (cm)
    pub baseline: f32,

    /// Rotation from left to right camera
    pub rotation: Matrix3<f32>,

    /// Translation from left to right camera
    pub translation: Vector3<f32>,

    /// Minimum disparity for stereo matching
    pub min_disparity: i32,

    /// Number of disparities to search
    pub num_disparities: i32,

    /// Block size for stereo matching
    pub block_size: i32,
}

impl StereoConfig {
    /// Create default stereo configuration for typical webcam setup
    pub fn default_webcam_stereo(image_width: u32, image_height: u32, baseline_cm: f32) -> Self {
        let intrinsics = CameraIntrinsics::default_webcam(image_width, image_height);

        Self {
            left_intrinsics: intrinsics.clone(),
            right_intrinsics: intrinsics,
            baseline: baseline_cm,
            rotation: Matrix3::identity(),
            translation: Vector3::new(baseline_cm, 0.0, 0.0), // Cameras separated horizontally
            min_disparity: 0,
            num_disparities: 64, // Must be divisible by 16
            block_size: 15,      // Odd number
        }
    }

    /// Calibrate stereo system from chessboard images
    ///
    /// This would typically be called during setup with calibration patterns
    pub fn calibrate_from_images(
        _left_images: &[Frame],
        _right_images: &[Frame],
        _chessboard_size: (usize, usize),
    ) -> ScannerResult<Self> {
        // Placeholder for calibration
        // In practice, use OpenCV's stereoCalibrate
        Err(ScannerError::NotImplemented(
            "Stereo calibration from images not yet implemented".to_string(),
        ))
    }
}

/// Stereo depth estimator
pub struct StereoDepthEstimator {
    config: StereoConfig,
}

impl StereoDepthEstimator {
    /// Create a new stereo depth estimator
    pub fn new(config: StereoConfig) -> Self {
        Self { config }
    }

    /// Compute depth map from stereo pair
    ///
    /// Returns a depth map where each pixel contains the estimated depth in cm
    pub fn compute_depth_map(
        &self,
        left_frame: &Frame,
        right_frame: &Frame,
    ) -> ScannerResult<DepthMap> {
        // Validate input frames
        if left_frame.width != right_frame.width || left_frame.height != right_frame.height {
            return Err(ScannerError::InvalidInput(format!(
                "Stereo frames must have same dimensions (left: {}x{}, right: {}x{})",
                left_frame.width, left_frame.height, right_frame.width, right_frame.height
            )));
        }

        if left_frame.width == 0 || left_frame.height == 0 {
            return Err(ScannerError::InvalidInput(
                "Frame dimensions must be non-zero".to_string(),
            ));
        }

        // Validate stereo parameters
        if self.config.block_size % 2 == 0 || self.config.block_size < 5 {
            return Err(ScannerError::InvalidInput(format!(
                "Block size must be odd and >= 5, got {}",
                self.config.block_size
            )));
        }

        if self.config.num_disparities % 16 != 0 || self.config.num_disparities <= 0 {
            return Err(ScannerError::InvalidInput(format!(
                "Number of disparities must be positive and divisible by 16, got {}",
                self.config.num_disparities
            )));
        }

        // Convert frames to grayscale for stereo matching
        let left_gray = left_frame.to_gray()?;
        let right_gray = right_frame.to_gray()?;

        // Create stereo block matcher
        let mut stereo =
            calib3d::StereoBM::create(self.config.num_disparities, self.config.block_size)?;

        // Set parameters
        stereo.set_min_disparity(self.config.min_disparity)?;
        stereo.set_num_disparities(self.config.num_disparities)?;
        stereo.set_block_size(self.config.block_size)?;

        // Compute disparity map
        let mut disparity = Mat::default();
        stereo.compute(&left_gray, &right_gray, &mut disparity)?;

        // Convert disparity to depth
        // Depth = (focal_length * baseline) / disparity
        let focal_length = self.config.left_intrinsics.fx;
        let baseline = self.config.baseline;

        let depth_map = self.disparity_to_depth(&disparity, focal_length, baseline)?;

        Ok(depth_map)
    }

    /// Triangulate 3D points from corresponding features in stereo pair
    pub fn triangulate_points(
        &self,
        left_features: &[Point2<f32>],
        right_features: &[Point2<f32>],
    ) -> ScannerResult<Vec<Point3<f32>>> {
        if left_features.len() != right_features.len() {
            return Err(ScannerError::InvalidInput(
                "Left and right feature counts must match".to_string(),
            ));
        }

        let mut points_3d = Vec::new();

        for (left_pt, right_pt) in left_features.iter().zip(right_features.iter()) {
            // Compute disparity
            let disparity = left_pt.x - right_pt.x;

            if disparity > 0.0 {
                // Triangulate using disparity formula: Z = (f * baseline) / disparity
                let depth = (self.config.left_intrinsics.fx * self.config.baseline) / disparity;

                // Unproject to 3D
                // NOTE: unproject() returns a NORMALIZED direction vector
                // We must scale by depth to get the actual 3D position
                let ray = self.config.left_intrinsics.unproject(left_pt.x, left_pt.y);

                // ray is already normalized, so we can scale directly by depth
                // This gives us the 3D point in camera coordinates
                let point_3d = Point3::from(ray * depth);

                points_3d.push(point_3d);
            }
        }

        Ok(points_3d)
    }

    /// Match features between left and right images using stereo constraints
    pub fn match_stereo_features(
        &self,
        left_features: &[Point2<f32>],
        right_features: &[Point2<f32>],
    ) -> Vec<(usize, usize)> {
        let mut matches = Vec::new();
        let epipolar_threshold = 2.0; // pixels

        for (i, left_pt) in left_features.iter().enumerate() {
            let mut best_match = None;
            let mut best_distance = f32::MAX;

            for (j, right_pt) in right_features.iter().enumerate() {
                // Check epipolar constraint (y coordinates should be similar for rectified images)
                if (left_pt.y - right_pt.y).abs() > epipolar_threshold {
                    continue;
                }

                // Right point should be to the left of left point (positive disparity)
                if right_pt.x >= left_pt.x {
                    continue;
                }

                // Compute matching cost (simple L2 distance)
                let dist = (left_pt - right_pt).norm();

                if dist < best_distance {
                    best_distance = dist;
                    best_match = Some(j);
                }
            }

            if let Some(right_idx) = best_match {
                matches.push((i, right_idx));
            }
        }

        matches
    }

    /// Convert disparity map to depth map
    fn disparity_to_depth(
        &self,
        disparity: &Mat,
        focal_length: f32,
        baseline: f32,
    ) -> ScannerResult<DepthMap> {
        let height = disparity.rows() as usize;
        let width = disparity.cols() as usize;

        let mut depths = vec![vec![0.0f32; width]; height];

        for y in 0..height {
            for x in 0..width {
                // Get disparity value (OpenCV stores as 16-bit fixed point)
                // Safely convert i16 to avoid overflow
                let disp_raw = disparity.at_2d::<i16>(y as i32, x as i32)?;

                // Check for invalid disparity marker (OpenCV uses negative values)
                if *disp_raw < 0 {
                    depths[y][x] = 0.0;
                    continue;
                }

                // Convert from fixed-point (divide by 16)
                let disp_val = *disp_raw as f32 / 16.0;

                // Convert to depth using stereo formula
                // Add small epsilon to avoid division by zero
                let depth = if disp_val > 0.1 {
                    (focal_length * baseline) / disp_val
                } else {
                    0.0 // Invalid depth (disparity too small)
                };

                depths[y][x] = depth;
            }
        }

        Ok(DepthMap::new(depths))
    }
}

/// A depth map storing depth values for each pixel
#[derive(Debug, Clone)]
pub struct DepthMap {
    /// Depth values (height x width)
    depths: Vec<Vec<f32>>,
}

impl DepthMap {
    /// Create a new depth map
    pub fn new(depths: Vec<Vec<f32>>) -> Self {
        Self { depths }
    }

    /// Get depth at pixel (x, y)
    pub fn get_depth(&self, x: usize, y: usize) -> Option<f32> {
        self.depths.get(y)?.get(x).copied()
    }

    /// Get the dimensions (width, height)
    pub fn dimensions(&self) -> (usize, usize) {
        if self.depths.is_empty() {
            (0, 0)
        } else {
            (self.depths[0].len(), self.depths.len())
        }
    }

    /// Get all depth values
    pub fn depths(&self) -> &[Vec<f32>] {
        &self.depths
    }

    /// Filter invalid depths (zero or negative)
    pub fn filter_invalid(&mut self) {
        for row in &mut self.depths {
            for depth in row {
                if *depth <= 0.0 || depth.is_nan() || depth.is_infinite() {
                    *depth = 0.0;
                }
            }
        }
    }

    /// Apply median filter to reduce noise
    pub fn median_filter(&mut self, kernel_size: usize) {
        let (width, height) = self.dimensions();
        let mut filtered = self.depths.clone();

        let radius = kernel_size / 2;

        for y in radius..(height - radius) {
            for x in radius..(width - radius) {
                // Collect neighboring values
                let mut values = Vec::new();
                for dy in 0..kernel_size {
                    for dx in 0..kernel_size {
                        let ny = y + dy - radius;
                        let nx = x + dx - radius;
                        if let Some(&depth) = self.depths.get(ny).and_then(|row| row.get(nx)) {
                            if depth > 0.0 {
                                values.push(depth);
                            }
                        }
                    }
                }

                if !values.is_empty() {
                    // Compute median
                    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    filtered[y][x] = values[values.len() / 2];
                }
            }
        }

        self.depths = filtered;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stereo_config_creation() {
        let config = StereoConfig::default_webcam_stereo(1280, 720, 6.0);
        assert_eq!(config.baseline, 6.0);
        assert!(config.num_disparities > 0);
        assert!(config.block_size > 0);
    }

    #[test]
    fn test_depth_map_creation() {
        let depths = vec![vec![10.0, 20.0, 30.0], vec![15.0, 25.0, 35.0]];
        let depth_map = DepthMap::new(depths);

        assert_eq!(depth_map.dimensions(), (3, 2));
        assert_eq!(depth_map.get_depth(1, 0), Some(20.0));
    }

    #[test]
    fn test_depth_map_filtering() {
        let depths = vec![vec![10.0, 0.0, 30.0], vec![15.0, -5.0, 35.0]];
        let mut depth_map = DepthMap::new(depths);

        depth_map.filter_invalid();

        assert_eq!(depth_map.get_depth(0, 0), Some(10.0));
        assert_eq!(depth_map.get_depth(1, 0), Some(0.0)); // Invalid filtered
        assert_eq!(depth_map.get_depth(1, 1), Some(0.0)); // Negative filtered
    }

    #[test]
    fn test_triangulate_points() {
        let config = StereoConfig::default_webcam_stereo(1280, 720, 6.0);
        let estimator = StereoDepthEstimator::new(config);

        let left_features = vec![Point2::new(640.0, 360.0)];
        let right_features = vec![Point2::new(630.0, 360.0)]; // 10 pixel disparity

        let points = estimator.triangulate_points(&left_features, &right_features);
        assert!(points.is_ok());

        let points = points.unwrap();
        assert_eq!(points.len(), 1);
        assert!(points[0].z > 0.0); // Should have positive depth
    }

    #[test]
    fn test_stereo_matching() {
        let config = StereoConfig::default_webcam_stereo(1280, 720, 6.0);
        let estimator = StereoDepthEstimator::new(config);

        let left_features = vec![Point2::new(100.0, 100.0), Point2::new(200.0, 200.0)];

        let right_features = vec![
            Point2::new(95.0, 100.0),  // Matches first left feature
            Point2::new(195.0, 200.0), // Matches second left feature
        ];

        let matches = estimator.match_stereo_features(&left_features, &right_features);
        assert_eq!(matches.len(), 2);
    }
}
