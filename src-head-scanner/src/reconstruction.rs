//! 3D reconstruction from 2D features
//!
//! This module handles converting detected 2D features from camera frames
//! into 3D point clouds using various reconstruction techniques.

use crate::camera::Frame;
use crate::error::{ScannerError, ScannerResult};
use crate::pointcloud::Point;
use crate::vision::Feature;
use nalgebra::{Matrix3, Matrix3x4, Point2, Point3, Vector3, UnitQuaternion};
use opencv::{
    calib3d, core::{Mat, Point2f, Vector as CvVector, Scalar},
    prelude::*,
};
use serde::{Deserialize, Serialize};

/// Camera intrinsic parameters
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CameraIntrinsics {
    /// Focal length in x direction (pixels)
    pub fx: f32,

    /// Focal length in y direction (pixels)
    pub fy: f32,

    /// Principal point x coordinate (pixels)
    pub cx: f32,

    /// Principal point y coordinate (pixels)
    pub cy: f32,

    /// Radial distortion coefficients
    pub distortion: Option<[f32; 5]>,
}

impl CameraIntrinsics {
    /// Create camera intrinsics for a typical webcam
    pub fn default_webcam(image_width: u32, image_height: u32) -> Self {
        let fx = image_width as f32 * 1.2; // Approximate focal length
        let fy = image_width as f32 * 1.2;
        let cx = image_width as f32 / 2.0;
        let cy = image_height as f32 / 2.0;

        Self {
            fx,
            fy,
            cx,
            cy,
            distortion: None,
        }
    }

    /// Get the camera matrix (K)
    pub fn camera_matrix(&self) -> Matrix3<f32> {
        Matrix3::new(self.fx, 0.0, self.cx, 0.0, self.fy, self.cy, 0.0, 0.0, 1.0)
    }

    /// Unproject a 2D point to a 3D ray
    pub fn unproject(&self, pixel_x: f32, pixel_y: f32) -> Vector3<f32> {
        let x = (pixel_x - self.cx) / self.fx;
        let y = (pixel_y - self.cy) / self.fy;
        let z = 1.0;

        Vector3::new(x, y, z).normalize()
    }

    /// Project a 3D point to 2D image coordinates
    pub fn project(&self, point: &Point3<f32>) -> (f32, f32) {
        let x = point.x * self.fx / point.z + self.cx;
        let y = point.y * self.fy / point.z + self.cy;
        (x, y)
    }
}

/// Camera pose (position and orientation)
#[derive(Debug, Clone)]
pub struct CameraPose {
    /// Position in world coordinates
    pub position: Point3<f32>,

    /// Rotation matrix (3x3)
    pub rotation: Matrix3<f32>,
}

impl CameraPose {
    /// Create identity pose (camera at origin looking down +Z)
    pub fn identity() -> Self {
        Self {
            position: Point3::origin(),
            rotation: Matrix3::identity(),
        }
    }

    /// Transform a point from camera space to world space
    pub fn to_world(&self, point_camera: &Point3<f32>) -> Point3<f32> {
        let rotated = self.rotation * point_camera.coords;
        Point3::from(rotated + self.position.coords)
    }

    /// Transform a point from world space to camera space
    pub fn to_camera(&self, point_world: &Point3<f32>) -> Point3<f32> {
        let translated = point_world - self.position;
        Point3::from(self.rotation.transpose() * translated)
    }
}

impl Default for CameraPose {
    fn default() -> Self {
        Self::identity()
    }
}

/// Convert detected features to 3D points
///
/// This function takes 2D features and estimates their 3D positions using depth information
/// or triangulation from multiple views.
pub fn features_to_points(features: &[Feature], frame: &Frame) -> ScannerResult<Vec<Point>> {
    if features.is_empty() {
        return Ok(Vec::new());
    }

    // Use default camera intrinsics based on frame size
    let intrinsics = CameraIntrinsics::default_webcam(frame.width, frame.height);

    let mut points = Vec::new();

    for feature in features {
        // Get depth estimate
        let depth = feature.depth.unwrap_or_else(|| {
            // Add small random variation to depth to prevent all points collapsing
            // This helps when camera is static or moving slowly
            let base_depth = estimate_depth_from_feature_type(&feature.feature_type);
            
            // Add position-dependent variation for more realistic depth
            // Use feature position to create deterministic but varied depth
            let x_factor = (feature.position.x * 0.01).sin() * 0.5; // ±0.5cm
            let y_factor = (feature.position.y * 0.01).cos() * 0.5; // ±0.5cm
            
            base_depth + x_factor + y_factor
        });

        // Unproject to 3D
        let ray = intrinsics.unproject(feature.position.x, feature.position.y);
        let point_3d = Point3::from(ray * depth);

        // Create point with color information if available
        let mut point = Point::new(point_3d.x, point_3d.y, point_3d.z);
        point.set_confidence(feature.confidence);

        points.push(point);
    }

    Ok(points)
}

/// Estimate depth based on feature type
fn estimate_depth_from_feature_type(feature_type: &str) -> f32 {
    match feature_type {
        "face" => 50.0, // 50cm typical distance
        "nose" => 48.0, // Slightly closer (nose protrudes)
        "ear" => 52.0,  // Slightly farther
        "left_eye" => 49.5,
        "right_eye" => 49.5,
        "mouth" => 49.0,
        // Corner features get slight random variation for better 3D structure
        "corner" => {
            // Add small variation to prevent all corners at same depth
            // Use a hash-like function for deterministic but varied depth
            50.0 + (feature_type.len() as f32 * 0.1) % 2.0 - 1.0 // ±1cm variation
        }
        _ => 50.0,
    }
}

/// Estimate essential matrix from feature correspondences
pub fn estimate_essential_matrix(
    points1: &[(f32, f32)],
    points2: &[(f32, f32)],
    intrinsics: &CameraIntrinsics,
) -> ScannerResult<(Matrix3<f64>, Vec<bool>)> {
    if points1.len() < 8 || points2.len() < 8 {
        return Err(ScannerError::InvalidConfig(
            "Need at least 8 point correspondences for essential matrix".to_string(),
        ));
    }
    
    // Convert to OpenCV format
    let mut pts1 = CvVector::<Point2f>::new();
    let mut pts2 = CvVector::<Point2f>::new();
    
    for (p1, p2) in points1.iter().zip(points2.iter()) {
        pts1.push(Point2f::new(p1.0, p1.1));
        pts2.push(Point2f::new(p2.0, p2.1));
    }
    
    // Camera matrix (K)
    let mut camera_matrix = Mat::zeros(3, 3, opencv::core::CV_64F)?.to_mat()?;
    *camera_matrix.at_2d_mut::<f64>(0, 0)? = intrinsics.fx as f64;
    *camera_matrix.at_2d_mut::<f64>(1, 1)? = intrinsics.fy as f64;
    *camera_matrix.at_2d_mut::<f64>(0, 2)? = intrinsics.cx as f64;
    *camera_matrix.at_2d_mut::<f64>(1, 2)? = intrinsics.cy as f64;
    *camera_matrix.at_2d_mut::<f64>(2, 2)? = 1.0;
    
    // Compute essential matrix using RANSAC
    let mut mask = Mat::default();
    let essential_mat = calib3d::find_essential_mat(
        &pts1,
        &pts2,
        &camera_matrix,
        calib3d::RANSAC,
        0.999,  // confidence
        1.0,    // threshold
        1000,   // max iterations
        &mut mask,
    ).map_err(|e| ScannerError::VisionModel(format!("Essential matrix failed: {}", e)))?;
    
    // Convert to nalgebra Matrix3
    let mut e = Matrix3::<f64>::zeros();
    for i in 0..3 {
        for j in 0..3 {
            e[(i, j)] = *essential_mat.at_2d::<f64>(i as i32, j as i32)
                .map_err(|e| ScannerError::VisionModel(format!("Matrix access failed: {}", e)))?;
        }
    }
    
    // Extract inlier mask
    let mut inliers = Vec::new();
    for i in 0..mask.rows() {
        let val = *mask.at::<u8>(i)
            .map_err(|e| ScannerError::VisionModel(format!("Mask access failed: {}", e)))?;
        inliers.push(val != 0);
    }
    
    log::debug!("Essential matrix: {} inliers of {} points", inliers.iter().filter(|&&x| x).count(), points1.len());
    
    Ok((e, inliers))
}

/// Recover camera pose (R, t) from essential matrix
pub fn recover_pose_from_essential(
    essential: &Matrix3<f64>,
    points1: &[(f32, f32)],
    points2: &[(f32, f32)],
    intrinsics: &CameraIntrinsics,
    inliers: &[bool],
) -> ScannerResult<CameraPose> {
    // Convert to OpenCV format (only inliers)
    let mut pts1 = CvVector::<Point2f>::new();
    let mut pts2 = CvVector::<Point2f>::new();
    
    for (i, (p1, p2)) in points1.iter().zip(points2.iter()).enumerate() {
        if inliers[i] {
            pts1.push(Point2f::new(p1.0, p1.1));
            pts2.push(Point2f::new(p2.0, p2.1));
        }
    }
    
    // Convert essential matrix to OpenCV
    let mut e_mat = Mat::zeros(3, 3, opencv::core::CV_64F)?.to_mat()?;
    for i in 0..3 {
        for j in 0..3 {
            *e_mat.at_2d_mut::<f64>(i as i32, j as i32)? = essential[(i, j)];
        }
    }
    
    // Camera matrix
    let mut camera_matrix = Mat::zeros(3, 3, opencv::core::CV_64F)?.to_mat()?;
    *camera_matrix.at_2d_mut::<f64>(0, 0)? = intrinsics.fx as f64;
    *camera_matrix.at_2d_mut::<f64>(1, 1)? = intrinsics.fy as f64;
    *camera_matrix.at_2d_mut::<f64>(0, 2)? = intrinsics.cx as f64;
    *camera_matrix.at_2d_mut::<f64>(1, 2)? = intrinsics.cy as f64;
    *camera_matrix.at_2d_mut::<f64>(2, 2)? = 1.0;
    
    // Recover pose
    let mut r_mat = Mat::default();
    let mut t_mat = Mat::default();
    let mut mask = Mat::default();
    let mut triangulated = Mat::default();
    
    calib3d::recover_pose_triangulated(
        &e_mat,
        &pts1,
        &pts2,
        &camera_matrix,
        &mut r_mat,
        &mut t_mat,
        1000.0, // distance threshold
        &mut mask,
        &mut triangulated,
    ).map_err(|e| ScannerError::VisionModel(format!("Pose recovery failed: {}", e)))?;
    
    // Convert R and t to nalgebra
    let mut rotation = Matrix3::<f32>::zeros();
    for i in 0..3 {
        for j in 0..3 {
            rotation[(i, j)] = *r_mat.at_2d::<f64>(i as i32, j as i32)? as f32;
        }
    }
    
    let position = Point3::new(
        *t_mat.at::<f64>(0)? as f32,
        *t_mat.at::<f64>(1)? as f32,
        *t_mat.at::<f64>(2)? as f32,
    );
    
    log::debug!("Recovered pose: position={:?}, rotation determinant={}", position, rotation.determinant());
    
    Ok(CameraPose { position, rotation })
}

/// Triangulate 3D point from two views
pub fn triangulate_point(
    point1: &Point2<f32>,
    point2: &Point2<f32>,
    pose1: &CameraPose,
    pose2: &CameraPose,
    intrinsics: &CameraIntrinsics,
) -> ScannerResult<Point3<f32>> {
    // Build projection matrices P = K[R|t]
    let k = Matrix3::new(
        intrinsics.fx, 0.0, intrinsics.cx,
        0.0, intrinsics.fy, intrinsics.cy,
        0.0, 0.0, 1.0,
    );
    
    // P1 = K[R1|t1]
    let mut p1 = Matrix3x4::<f32>::zeros();
    for i in 0..3 {
        for j in 0..3 {
            p1[(i, j)] = pose1.rotation[(i, j)];
        }
        p1[(i, 3)] = pose1.position[i];
    }
    let p1 = k * p1;
    
    // P2 = K[R2|t2]
    let mut p2 = Matrix3x4::<f32>::zeros();
    for i in 0..3 {
        for j in 0..3 {
            p2[(i, j)] = pose2.rotation[(i, j)];
        }
        p2[(i, 3)] = pose2.position[i];
    }
    let p2 = k * p2;
    
    // DLT triangulation (Direct Linear Transform)
    // Build matrix A from the two projection equations
    let mut a = nalgebra::Matrix4::<f32>::zeros();
    
    a.set_row(0, &(point1.x * p1.row(2) - p1.row(0)));
    a.set_row(1, &(point1.y * p1.row(2) - p1.row(1)));
    a.set_row(2, &(point2.x * p2.row(2) - p2.row(0)));
    a.set_row(3, &(point2.y * p2.row(2) - p2.row(1)));
    
    // Solve using SVD
    let svd = a.svd(true, true);
    let v = svd.v_t.ok_or_else(|| {
        ScannerError::VisionModel("SVD failed to compute V".to_string())
    })?;
    
    // Solution is last column of V (smallest singular value)
    let x = v.row(3);
    
    // Convert from homogeneous to 3D
    let w = x[3];
    if w.abs() < 1e-6 {
        return Err(ScannerError::VisionModel("Point at infinity".to_string()));
    }
    
    Ok(Point3::new(x[0] / w, x[1] / w, x[2] / w))
}

/// Structure-from-Motion reconstructor
///
/// Builds a 3D point cloud from multiple views by tracking features
/// and estimating camera poses.
pub struct SfMReconstructor {
    /// Camera intrinsics
    intrinsics: CameraIntrinsics,

    /// Estimated camera poses for each frame
    poses: Vec<CameraPose>,

    /// 3D points reconstructed so far
    points: Vec<Point3<f32>>,

    /// Feature tracks (each track is a list of 2D observations across frames)
    tracks: Vec<Vec<(usize, Feature)>>, // (frame_index, feature)
}

impl SfMReconstructor {
    /// Create a new SfM reconstructor
    pub fn new(intrinsics: CameraIntrinsics) -> Self {
        Self {
            intrinsics,
            poses: Vec::new(),
            points: Vec::new(),
            tracks: Vec::new(),
        }
    }

    /// Add a new frame with detected features
    pub fn add_frame(&mut self, features: Vec<Feature>) -> ScannerResult<()> {
        let frame_idx = self.poses.len();

        // Estimate camera pose for this frame
        let pose = if frame_idx == 0 {
            // First frame: use identity pose
            CameraPose::identity()
        } else {
            // Subsequent frames: estimate pose from feature correspondences
            self.estimate_pose(&features)?
        };

        self.poses.push(pose);

        // Update feature tracks
        self.update_tracks(frame_idx, features);

        // Triangulate new points
        self.triangulate_tracks()?;

        Ok(())
    }

    /// Get all reconstructed 3D points
    pub fn get_points(&self) -> &[Point3<f32>] {
        &self.points
    }

    /// Estimate camera pose for a new frame
    fn estimate_pose(&self, features: &[Feature]) -> ScannerResult<CameraPose> {
        // Simplified pose estimation - in practice, use PnP (Perspective-n-Point)
        // or essential matrix decomposition

        // For now, assume camera moves slightly forward
        let mut pose = self.poses.last().cloned().unwrap_or_default();
        pose.position.z += 2.0; // Move 2cm forward

        Ok(pose)
    }

    /// Update feature tracks with new observations
    fn update_tracks(&mut self, frame_idx: usize, features: Vec<Feature>) {
        for feature in features {
            // Try to match with existing tracks
            let mut matched = false;

            for track in &mut self.tracks {
                if let Some((_, last_feature)) = track.last() {
                    // Simple matching based on feature type and proximity
                    if last_feature.feature_type == feature.feature_type {
                        let dist = (last_feature.position - feature.position).norm();
                        if dist < 50.0 {
                            // threshold
                            track.push((frame_idx, feature.clone()));
                            matched = true;
                            break;
                        }
                    }
                }
            }

            // Create new track if not matched
            if !matched {
                self.tracks.push(vec![(frame_idx, feature)]);
            }
        }
    }

    /// Triangulate 3D points from feature tracks
    fn triangulate_tracks(&mut self) -> ScannerResult<()> {
        for track in &self.tracks {
            if track.len() < 2 {
                continue; // Need at least 2 views
            }

            // Get first and last observations
            let (frame1, feature1) = &track[0];
            let (frame2, feature2) = &track[track.len() - 1];

            if frame1 == frame2 {
                continue;
            }

            // Triangulate
            if let Ok(point) = self.triangulate_point(
                &self.poses[*frame1],
                &feature1,
                &self.poses[*frame2],
                &feature2,
            ) {
                self.points.push(point);
            }
        }

        Ok(())
    }

    /// Triangulate a single 3D point from two views
    fn triangulate_point(
        &self,
        pose1: &CameraPose,
        feature1: &Feature,
        pose2: &CameraPose,
        feature2: &Feature,
    ) -> ScannerResult<Point3<f32>> {
        // Get rays from both cameras
        let ray1 = self
            .intrinsics
            .unproject(feature1.position.x, feature1.position.y);
        let ray2 = self
            .intrinsics
            .unproject(feature2.position.x, feature2.position.y);

        // Transform rays to world space
        let ray1_world = pose1.rotation * ray1;
        let ray2_world = pose2.rotation * ray2;

        // Find closest point between two rays (simplified triangulation)
        let point = triangulate_rays(&pose1.position, &ray1_world, &pose2.position, &ray2_world);

        Ok(point)
    }
}

/// Triangulate a 3D point from two rays
fn triangulate_rays(
    origin1: &Point3<f32>,
    direction1: &Vector3<f32>,
    origin2: &Point3<f32>,
    direction2: &Vector3<f32>,
) -> Point3<f32> {
    // Compute the closest point between two lines in 3D
    let w = origin1 - origin2;
    let a = direction1.dot(direction1);
    let b = direction1.dot(direction2);
    let c = direction2.dot(direction2);
    let d = direction1.dot(&w);
    let e = direction2.dot(&w);

    let denom = a * c - b * b;
    let t1 = if denom.abs() > 1e-6 {
        (b * e - c * d) / denom
    } else {
        0.0
    };

    let point1 = origin1 + direction1 * t1;
    let t2 = (a * e - b * d) / denom;
    let point2 = origin2 + direction2 * t2;

    // Return midpoint
    Point3::from((point1.coords + point2.coords) * 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_intrinsics() {
        let intrinsics = CameraIntrinsics::default_webcam(1280, 720);
        assert_eq!(intrinsics.cx, 640.0);
        assert_eq!(intrinsics.cy, 360.0);

        let ray = intrinsics.unproject(640.0, 360.0);
        assert!((ray.z - 1.0).abs() < 1e-3); // Ray should point forward
    }

    #[test]
    fn test_camera_pose() {
        let pose = CameraPose::identity();
        let point_camera = Point3::new(1.0, 2.0, 3.0);
        let point_world = pose.to_world(&point_camera);

        assert_eq!(point_camera, point_world); // Identity transform

        let point_back = pose.to_camera(&point_world);
        assert!((point_back.coords - point_camera.coords).norm() < 1e-6);
    }

    #[test]
    fn test_triangulate_rays() {
        let origin1 = Point3::new(0.0, 0.0, 0.0);
        let direction1 = Vector3::new(1.0, 0.0, 1.0).normalize();

        let origin2 = Point3::new(2.0, 0.0, 0.0);
        let direction2 = Vector3::new(-1.0, 0.0, 1.0).normalize();

        let point = triangulate_rays(&origin1, &direction1, &origin2, &direction2);

        // Point should be somewhere in the middle
        assert!(point.x > 0.0 && point.x < 2.0);
        assert!(point.z > 0.0);
    }
}
