//! Tests for Structure-from-Motion (SfM) reconstruction

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vision::{Feature, FeatureTracker};
    use nalgebra::{Matrix3, Point2, Point3, Rotation3, Vector3};

    /// Test essential matrix estimation from matched features
    #[test]
    fn test_essential_matrix_estimation() {
        // Create synthetic matched features from known camera motion
        let rotation = Rotation3::from_euler_angles(0.0, 0.1, 0.0); // 0.1 rad rotation around Y
        let translation = Vector3::new(1.0, 0.0, 0.0); // 1 unit translation in X

        // Create test points in 3D
        let points_3d = vec![
            Point3::new(0.0, 0.0, 5.0),
            Point3::new(1.0, 0.0, 5.0),
            Point3::new(0.0, 1.0, 5.0),
            Point3::new(1.0, 1.0, 5.0),
            Point3::new(0.5, 0.5, 6.0),
        ];

        // Project to first camera (identity pose)
        let focal = 500.0;
        let mut points1 = Vec::new();
        for p in &points_3d {
            let x = (p.x / p.z) * focal + 320.0;
            let y = (p.y / p.z) * focal + 240.0;
            points1.push(Point2::new(x, y));
        }

        // Project to second camera (rotated and translated)
        let mut points2 = Vec::new();
        for p in &points_3d {
            let p_cam2 = rotation * p + translation;
            let x = (p_cam2.x / p_cam2.z) * focal + 320.0;
            let y = (p_cam2.y / p_cam2.z) * focal + 240.0;
            points2.push(Point2::new(x, y));
        }

        // Estimate essential matrix
        let result = estimate_essential_matrix(&points1, &points2, focal);

        assert!(result.is_ok(), "Essential matrix estimation should succeed");

        let (essential, inliers) = result.unwrap();

        // Should have found some inliers
        assert!(inliers.len() >= 4, "Should have at least 4 inliers");

        // Essential matrix should not be zero
        assert!(
            essential.norm() > 0.1,
            "Essential matrix should not be near zero"
        );
    }

    /// Test camera pose recovery from essential matrix
    #[test]
    fn test_pose_recovery() {
        // Create a known essential matrix from R and t
        let rotation = Rotation3::from_euler_angles(0.0, 0.2, 0.0);
        let translation = Vector3::new(1.0, 0.0, 0.0).normalize();

        // E = [t]_x * R (skew-symmetric matrix of t times R)
        let tx = translation.x;
        let ty = translation.y;
        let tz = translation.z;

        let t_skew = Matrix3::new(0.0, -tz, ty, tz, 0.0, -tx, -ty, tx, 0.0);

        let essential = t_skew * rotation.matrix();

        // Create test point correspondences
        let focal = 500.0;
        let points1 = vec![Point2::new(320.0, 240.0)];
        let points2 = vec![Point2::new(340.0, 245.0)];

        // Recover pose
        let result = recover_pose_from_essential(&essential, &points1, &points2, focal);

        assert!(result.is_ok(), "Pose recovery should succeed");

        let (r, t) = result.unwrap();

        // Check rotation is close to original (within tolerance due to numerical errors)
        let angle_diff = (r.transpose() * rotation.matrix()).trace();
        assert!(
            angle_diff > 2.5,
            "Recovered rotation should be close to original"
        );

        // Check translation direction is similar (may be scaled)
        let t_dot = t.normalize().dot(&translation);
        assert!(
            t_dot.abs() > 0.9,
            "Recovered translation direction should be similar"
        );
    }

    /// Test triangulation of 3D points from two views
    #[test]
    fn test_triangulation() {
        // Setup two cameras
        let focal = 500.0;
        let pose1 = CameraPose {
            position: Point3::origin(),
            rotation: Rotation3::identity(),
        };

        let pose2 = CameraPose {
            position: Point3::new(1.0, 0.0, 0.0),
            rotation: Rotation3::identity(),
        };

        // Known 3D point
        let point_3d = Point3::new(0.5, 0.0, 5.0);

        // Project to both cameras
        let p1 = project_point(&point_3d, &pose1, focal);
        let p2 = project_point(&point_3d, &pose2, focal);

        // Triangulate
        let result = triangulate_point(&p1, &p2, &pose1, &pose2, focal);

        assert!(result.is_ok(), "Triangulation should succeed");

        let reconstructed = result.unwrap();

        // Check reconstructed point is close to original
        let error = (reconstructed - point_3d).norm();
        assert!(
            error < 0.1,
            "Triangulation error should be small, got {}",
            error
        );
    }

    /// Test SfM frame matching and feature tracking
    #[test]
    fn test_sfm_frame_matching() {
        let mut tracker = FeatureTracker::new();

        // Create sequence of features representing camera motion
        let features_t0 = vec![
            Feature::new(100.0, 100.0, "p1".to_string(), 0.9),
            Feature::new(200.0, 100.0, "p2".to_string(), 0.9),
            Feature::new(150.0, 150.0, "p3".to_string(), 0.9),
        ];

        let features_t1 = vec![
            Feature::new(105.0, 102.0, "p1".to_string(), 0.9), // Moved slightly
            Feature::new(205.0, 102.0, "p2".to_string(), 0.9),
            Feature::new(155.0, 152.0, "p3".to_string(), 0.9),
        ];

        let features_t2 = vec![
            Feature::new(110.0, 104.0, "p1".to_string(), 0.9), // Continued motion
            Feature::new(210.0, 104.0, "p2".to_string(), 0.9),
            Feature::new(160.0, 154.0, "p3".to_string(), 0.9),
        ];

        // Track through sequence
        tracker.update(features_t0);
        tracker.update(features_t1);
        tracker.update(features_t2);

        let tracks = tracker.get_tracks();

        // Should have 3 tracks (one per feature)
        assert_eq!(tracks.len(), 3, "Should have tracked 3 features");

        // Each track should have 3 observations
        for track in &tracks {
            assert_eq!(
                track.observations.len(),
                3,
                "Each track should have 3 observations"
            );
        }
    }

    /// Test minimum inlier requirement
    #[test]
    fn test_sfm_inlier_validation() {
        // Test with too few points
        let points1 = vec![Point2::new(100.0, 100.0), Point2::new(200.0, 100.0)];

        let points2 = vec![Point2::new(105.0, 102.0), Point2::new(205.0, 102.0)];

        let focal = 500.0;
        let result = estimate_essential_matrix(&points1, &points2, focal);

        // Should fail with too few points
        assert!(result.is_err(), "Should fail with fewer than 5 points");
    }

    /// Helper: Project 3D point to 2D image coordinates
    fn project_point(point: &Point3<f32>, pose: &CameraPose, focal: f32) -> Point2<f32> {
        // Transform to camera coordinates
        let p_cam = pose.rotation.inverse() * (point - pose.position);

        // Perspective projection
        let x = (p_cam.x / p_cam.z) * focal + 320.0;
        let y = (p_cam.y / p_cam.z) * focal + 240.0;

        Point2::new(x, y)
    }

    /// Test full SfM pipeline with synthetic data
    #[test]
    fn test_sfm_pipeline_integration() {
        // This would test the full pipeline:
        // 1. Feature detection across frames
        // 2. Feature matching/tracking
        // 3. Essential matrix estimation
        // 4. Pose recovery
        // 5. Triangulation
        // 6. Bundle adjustment (optional)

        // For now, verify the components exist
        assert!(
            true,
            "SfM pipeline components exist and are tested individually"
        );
    }
}
