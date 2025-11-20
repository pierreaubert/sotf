//! 3D reconstruction from 2D features
//!
//! This module handles converting detected 2D features from camera frames
//! into 3D point clouds using various reconstruction techniques.

use crate::camera::Frame;
use crate::error::{ScannerError, ScannerResult};
use crate::pointcloud::Point;
use crate::vision::Feature;
use nalgebra::{Matrix3, Point3, Vector3};

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
            // Default depth based on typical head size and distance
            estimate_depth_from_feature_type(&feature.feature_type)
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
        // Grid points: add variation based on position to create depth
        s if s.starts_with("grid_") => {
            // Parse grid coordinates from "grid_i_j"
            let parts: Vec<&str> = s.split('_').collect();
            if parts.len() == 3 {
                if let (Ok(i), Ok(j)) = (parts[1].parse::<i32>(), parts[2].parse::<i32>()) {
                    // Create depth variation based on grid position
                    // Center of face (grid 3-4, 3-4) is closer (nose area)
                    // Edges are farther
                    let center_i = 3.5;
                    let center_j = 3.5;
                    let dist_from_center = ((i as f32 - center_i).powi(2) + (j as f32 - center_j).powi(2)).sqrt();
                    
                    // Base depth 50cm, vary ±3cm based on position
                    // Center (nose) is closer, edges (ears/sides) are farther
                    let depth_variation = (dist_from_center - 2.5) * 0.6; // -1.5cm to +2cm range
                    return 50.0 + depth_variation;
                }
            }
            50.0
        }
        _ => 50.0,
    }
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
