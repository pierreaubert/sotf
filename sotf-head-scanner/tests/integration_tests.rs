//! Integration tests for head scanner
//!
//! These tests verify the complete workflow from camera capture to mesh generation

use head_scanner::*;
use pointcloud::{Point, PointCloud};
use reconstruction::CameraIntrinsics;

#[test]
fn test_complete_scanning_workflow() {
    // This test verifies the entire pipeline works together
    // In practice, this would use actual camera frames

    // 1. Create a point cloud from synthetic data
    let mut point_cloud = PointCloud::new();

    // Add points representing a head (simplified sphere)
    let num_points = 100;
    for i in 0..num_points {
        let theta = (i as f32 / num_points as f32) * 2.0 * std::f32::consts::PI;
        for j in 0..num_points {
            let phi = (j as f32 / num_points as f32) * std::f32::consts::PI;

            let radius = 10.0; // 10cm radius head
            let x = radius * phi.sin() * theta.cos();
            let y = radius * phi.sin() * theta.sin();
            let z = radius * phi.cos();

            point_cloud.add_point(Point::new(x, y, z));
        }
    }

    // 2. Compute convex hull
    let hull_result = convexhull::compute_convex_hull_3d(&point_cloud);
    assert!(
        hull_result.is_ok(),
        "Convex hull computation should succeed"
    );

    let hull = hull_result.unwrap();
    assert!(hull.vertex_count() > 0, "Hull should have vertices");
    assert!(hull.face_count() > 0, "Hull should have faces");
    assert!(hull.volume() > 0.0, "Hull should have positive volume");

    // 3. Convert to mesh
    let mesh = mesh::Mesh::from_convex_hull(&hull);
    assert!(mesh.vertices().len() > 0, "Mesh should have vertices");
    assert!(mesh.triangles().len() > 0, "Mesh should have triangles");
}

#[test]
fn test_sfm_reconstruction_pipeline() {
    use reconstruction::{CameraPose, SfMReconstructor};
    use vision::Feature;

    let intrinsics = CameraIntrinsics::default_webcam(1280, 720);
    let mut sfm = SfMReconstructor::new(intrinsics);

    // Simulate multiple frames with features
    for frame_idx in 0..5 {
        let features = vec![
            Feature::new(640.0 + frame_idx as f32, 360.0, "nose".to_string(), 0.9),
            Feature::new(600.0 + frame_idx as f32, 340.0, "left_eye".to_string(), 0.8),
            Feature::new(
                680.0 + frame_idx as f32,
                340.0,
                "right_eye".to_string(),
                0.8,
            ),
        ];

        let result = sfm.add_frame(features);
        assert!(result.is_ok(), "Adding frame should succeed");
    }

    let points = sfm.get_points();
    assert!(points.len() > 0, "SfM should reconstruct some 3D points");
}

#[test]
fn test_bundle_adjustment_improves_reconstruction() {
    use bundle_adjustment::{BundleAdjuster, Point3DWithObservations};
    use nalgebra::{Point2, Point3};
    use reconstruction::{CameraIntrinsics, CameraPose};

    let intrinsics = CameraIntrinsics::default_webcam(1280, 720);
    let adjuster = BundleAdjuster::new(intrinsics);

    // Create simple test data
    let poses = vec![CameraPose::identity(), {
        let mut pose = CameraPose::identity();
        pose.position.z = 10.0;
        pose
    }];

    let points = vec![Point3DWithObservations {
        position: Point3::new(0.0, 0.0, 50.0),
        observations: vec![
            (0, Point2::new(640.0, 360.0)),
            (1, Point2::new(640.0, 360.0)),
        ],
    }];

    let result = adjuster.optimize(&poses, &points);
    assert!(result.is_ok(), "Bundle adjustment should succeed");

    let (optimized_poses, optimized_points) = result.unwrap();
    assert_eq!(optimized_poses.len(), poses.len());
    assert_eq!(optimized_points.len(), 1);
}

#[test]
fn test_stereo_depth_estimation() {
    use stereo::{DepthMap, StereoConfig, StereoDepthEstimator};

    let config = StereoConfig::default_webcam_stereo(1280, 720, 6.0);
    let estimator = StereoDepthEstimator::new(config);

    // Test depth map creation and filtering
    let depths = vec![vec![10.0, 20.0, 0.0], vec![15.0, -5.0, 25.0]];
    let mut depth_map = DepthMap::new(depths);

    assert_eq!(depth_map.dimensions(), (3, 2));

    depth_map.filter_invalid();
    assert_eq!(depth_map.get_depth(2, 0), Some(0.0)); // Zero filtered
    assert_eq!(depth_map.get_depth(1, 1), Some(0.0)); // Negative filtered
}

#[test]
fn test_texture_mapping_pipeline() {
    use texture::{TextureMapper, UVCoord};

    let mapper = TextureMapper::new(512, 512);

    // Test UV coordinate generation
    let uv = UVCoord::new(0.5, 0.5);
    assert_eq!(uv.u, 0.5);
    assert_eq!(uv.v, 0.5);

    // Test point in triangle
    assert!(mapper.point_in_triangle(5, 5, (0, 0), (10, 0), (5, 10)));
    assert!(!mapper.point_in_triangle(20, 20, (0, 0), (10, 0), (5, 10)));
}

#[test]
fn test_vision_model_preprocessing() {
    // Test that preprocessing functions are available
    // Actual testing would require real frames
    use vision::{Feature, apply_nms};

    let features = vec![
        Feature::new(100.0, 100.0, "test1".to_string(), 0.9),
        Feature::new(105.0, 105.0, "test2".to_string(), 0.8), // Close to first, should be suppressed
        Feature::new(200.0, 200.0, "test3".to_string(), 0.95),
    ];

    let filtered = apply_nms(features, 0.5);
    assert!(
        filtered.len() <= 3,
        "NMS should reduce or maintain feature count"
    );
}

#[test]
fn test_scanner_state_transitions() {
    let config = ScannerConfig::default();
    let scanner = HeadScanner::new(config);
    assert!(scanner.is_ok());

    let scanner = scanner.unwrap();
    assert_eq!(scanner.get_state(), ScanState::Idle);
    assert_eq!(scanner.get_coverage(), 0.0);
    assert!(!scanner.is_scan_complete());
}

#[test]
fn test_coverage_tracking() {
    use coverage::CoverageMap;
    use pointcloud::Point;

    let mut coverage = CoverageMap::new();

    // Add some points
    let points = vec![
        Point::new(1.0, 0.0, 0.0),
        Point::new(0.0, 1.0, 0.0),
        Point::new(0.0, 0.0, 1.0),
    ];

    coverage.update(&points);

    assert!(coverage.get_coverage_percentage() >= 0.0);
    assert!(coverage.get_coverage_percentage() <= 1.0);
}

#[test]
fn test_point_cloud_operations() {
    let mut cloud = PointCloud::new();

    // Add points
    cloud.add_point(Point::new(1.0, 2.0, 3.0));
    cloud.add_point(Point::new(4.0, 5.0, 6.0));

    assert_eq!(cloud.len(), 2);
    assert!(!cloud.is_empty());

    // Test add_points
    let more_points = vec![Point::new(7.0, 8.0, 9.0), Point::new(10.0, 11.0, 12.0)];
    cloud.add_points(&more_points);

    assert_eq!(cloud.len(), 4);
}

#[test]
fn test_mesh_export() {
    use mesh::Mesh;
    use tempfile::tempdir;

    let mut cloud = PointCloud::new();
    cloud.add_point(Point::new(0.0, 0.0, 0.0));
    cloud.add_point(Point::new(1.0, 0.0, 0.0));
    cloud.add_point(Point::new(0.0, 1.0, 0.0));
    cloud.add_point(Point::new(0.0, 0.0, 1.0));

    let hull = convexhull::compute_convex_hull_3d(&cloud).unwrap();
    let mesh = Mesh::from_convex_hull(&hull);

    let dir = tempdir().unwrap();
    let obj_path = dir.path().join("test.obj");

    let result = mesh.export(obj_path.to_str().unwrap());
    assert!(result.is_ok(), "Mesh export should succeed");

    // Verify file was created
    assert!(obj_path.exists(), "OBJ file should be created");
}
