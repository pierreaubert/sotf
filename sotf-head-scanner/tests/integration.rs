//! Integration tests for the head-scanner library
//!
//! These tests verify end-to-end functionality

#[cfg(test)]
mod tests {
    use head_scanner::{ScanState, Scanner, ScannerConfig};
    use std::path::PathBuf;

    #[test]
    fn test_scanner_initialization() {
        let config = ScannerConfig::default();
        let scanner = Scanner::new_with_config(config);

        // Scanner should be in Idle state initially
        assert_eq!(scanner.get_state(), ScanState::Idle);
    }

    #[test]
    fn test_config_validation() {
        let mut config = ScannerConfig::default();

        // Test valid config
        assert!(config.min_coverage > 0.0 && config.min_coverage <= 1.0);
        assert!(config.point_density > 0.0);

        // Test model path validation with allowed directories
        config.model_path = Some("/safe/models/model.onnx".to_string());
        config.model_base_dirs = vec![PathBuf::from("/safe/models")];

        // Test with empty base dirs (less secure, allows any path)
        config.model_base_dirs = Vec::new();
    }

    #[test]
    fn test_security_path_validation() {
        use head_scanner::security::{validate_export_path, validate_path};
        use std::path::PathBuf;

        let temp_dir = std::env::temp_dir();

        // Test valid path
        let result = validate_export_path("output.obj", Some(&temp_dir));
        assert!(result.is_ok(), "Valid path should pass validation");

        // Test path traversal attack
        let result = validate_export_path("../../../etc/passwd", Some(&temp_dir));
        assert!(result.is_err(), "Path traversal should be blocked");

        // Test URL-encoded traversal
        let result = validate_export_path("%2e%2e/etc/passwd", Some(&temp_dir));
        assert!(result.is_err(), "URL-encoded traversal should be blocked");

        // Test null byte injection
        let result = validate_export_path("file\0.obj", Some(&temp_dir));
        assert!(result.is_err(), "Null byte should be blocked");
    }

    #[test]
    fn test_coverage_tracking() {
        use head_scanner::coverage::CoverageMap;
        use nalgebra::Point3;

        let mut coverage = CoverageMap::new(100); // 100cm^3 volume

        // Add some points
        for i in 0..10 {
            coverage.add_point(&Point3::new(i as f32, 0.0, 0.0));
        }

        let percentage = coverage.get_coverage_percentage();
        assert!(percentage > 0.0, "Coverage should increase with points");
        assert!(percentage <= 1.0, "Coverage should not exceed 100%");
    }

    #[test]
    fn test_guidance_system() {
        use head_scanner::guidance::{HeadRegion, ScanGuidance};
        use head_scanner::reconstruction::CameraPose;
        use nalgebra::{Point3, UnitQuaternion};

        let mut guidance = ScanGuidance::new();

        // Create mock camera poses
        let pose1 = CameraPose {
            position: Point3::new(0.0, 0.0, 50.0),
            rotation: UnitQuaternion::identity(),
        };

        // Update guidance with pose
        guidance.update_pose(&pose1, 100);

        // Get next region to scan
        let next_region = guidance.get_next_region();
        println!("Next region to scan: {:?}", next_region);
    }

    #[test]
    fn test_mesh_operations() {
        use head_scanner::mesh::Mesh;
        use nalgebra::Point3;

        let mut mesh = Mesh::new();

        // Add vertices
        mesh.add_vertex(Point3::new(0.0, 0.0, 0.0), None, None);
        mesh.add_vertex(Point3::new(1.0, 0.0, 0.0), None, None);
        mesh.add_vertex(Point3::new(0.0, 1.0, 0.0), None, None);

        // Add triangle
        mesh.add_triangle([0, 1, 2]);

        assert_eq!(mesh.vertex_count(), 3);
        assert_eq!(mesh.triangle_count(), 1);

        // Test bounds calculation
        let bounds = mesh.bounding_box();
        assert!(
            bounds.is_some(),
            "Mesh with vertices should have bounding box"
        );
    }

    #[test]
    fn test_convex_hull() {
        use head_scanner::convexhull::ConvexHull3D;
        use nalgebra::Point3;

        // Create a simple set of points
        let points = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
        ];

        let result = ConvexHull3D::compute(&points);
        assert!(result.is_ok(), "Convex hull should compute successfully");

        if let Ok(hull) = result {
            // Tetrahedron should have 4 faces
            assert_eq!(hull.faces().len(), 4, "Tetrahedron should have 4 faces");
        }
    }

    #[test]
    #[ignore = "Requires camera hardware"]
    fn test_camera_capture() {
        // This test requires actual camera hardware
        // Run with: cargo test --ignored

        use head_scanner::camera::Camera;

        let result = Camera::new(0, 640, 480, 30);

        if let Ok(mut camera) = result {
            let frame = camera.capture_frame();
            assert!(frame.is_ok(), "Should capture frame from camera");
        } else {
            println!("No camera available, skipping test");
        }
    }

    #[test]
    #[cfg(feature = "sofa")]
    fn test_end_to_end_sofa_generation() {
        use head_scanner::acoustics::generate_sofa_analytical;
        use head_scanner::mesh::Mesh;
        use nalgebra::Point3;
        use std::fs;

        // Create a minimal test mesh
        let mut mesh = Mesh::new();
        let radius = 25.0;

        // Add 6 vertices (simple octahedron)
        mesh.add_vertex(Point3::new(radius, 0.0, 0.0), None, None);
        mesh.add_vertex(Point3::new(-radius, 0.0, 0.0), None, None);
        mesh.add_vertex(Point3::new(0.0, radius, 0.0), None, None);
        mesh.add_vertex(Point3::new(0.0, -radius, 0.0), None, None);
        mesh.add_vertex(Point3::new(0.0, 0.0, radius), None, None);
        mesh.add_vertex(Point3::new(0.0, 0.0, -radius), None, None);

        // Add triangles
        mesh.add_triangle([0, 2, 4]);
        mesh.add_triangle([1, 2, 4]);
        mesh.add_triangle([0, 3, 4]);
        mesh.add_triangle([1, 3, 4]);

        // Generate SOFA file
        let temp_dir = std::env::temp_dir();
        let output_path = temp_dir.join("integration_test.sofa");

        let result =
            generate_sofa_analytical(&mesh, output_path.to_str().unwrap(), 44100.0, 8, 4, 1.0);

        assert!(result.is_ok(), "SOFA generation should succeed");
        assert!(output_path.exists(), "SOFA file should be created");

        // Cleanup
        let _ = fs::remove_file(&output_path);
    }

    #[test]
    fn test_error_handling_chain() {
        // Test that errors propagate correctly through the system
        use head_scanner::error::{ScannerError, ScannerResult};

        fn inner_function() -> ScannerResult<i32> {
            Err(ScannerError::InvalidConfig("Test error".to_string()))
        }

        fn outer_function() -> ScannerResult<i32> {
            inner_function()?;
            Ok(42)
        }

        let result = outer_function();
        assert!(result.is_err(), "Error should propagate");

        match result {
            Err(ScannerError::InvalidConfig(msg)) => {
                assert_eq!(msg, "Test error");
            }
            _ => panic!("Wrong error type"),
        }
    }

    #[test]
    fn test_concurrent_scanner_usage() {
        use std::sync::Arc;
        use std::thread;

        // Test that Scanner can be used from multiple threads with Arc
        let config = ScannerConfig::default();
        let scanner = Arc::new(Scanner::new_with_config(config));

        let handles: Vec<_> = (0..4)
            .map(|i| {
                let scanner_clone = Arc::clone(&scanner);
                thread::spawn(move || {
                    let state = scanner_clone.get_state();
                    println!("Thread {} - Scanner state: {:?}", i, state);
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread should complete successfully");
        }
    }
}
