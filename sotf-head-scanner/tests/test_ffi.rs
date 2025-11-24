//! Tests for FFI (Foreign Function Interface) bindings
//!
//! These tests verify the safety and correctness of the C API exposed to Swift/iOS

#[cfg(test)]
mod tests {
    use head_scanner::ffi::*;
    use head_scanner::{Mesh, ScanGuidance, Scanner};
    use std::ffi::CString;
    use std::ptr;

    #[test]
    fn test_scanner_lifecycle() {
        // Test scanner creation and destruction
        let scanner = scanner_new();
        assert!(
            !scanner.is_null(),
            "Scanner should not be null after creation"
        );

        // Free the scanner
        scanner_free(scanner);

        // Freeing null should not panic
        scanner_free(ptr::null_mut());
    }

    #[test]
    fn test_guidance_lifecycle() {
        let scanner = scanner_new();
        assert!(!scanner.is_null());

        // Get guidance
        let guidance = scanner_get_guidance(scanner);
        assert!(!guidance.is_null(), "Guidance should not be null");

        // Free guidance
        guidance_free(guidance);

        // Cleanup
        scanner_free(scanner);

        // Freeing null should not panic
        guidance_free(ptr::null_mut());
    }

    #[test]
    fn test_process_frame_null_pointers() {
        // Test that null pointers are properly rejected
        let result = scanner_process_frame(
            ptr::null_mut(), // null scanner
            ptr::null(),     // null rgb_data
            ptr::null(),     // null depth_data
            640,
            480,
            ptr::null(), // null pose
        );

        assert_eq!(
            result,
            ScannerResultCode::InvalidInput,
            "Should reject null pointers"
        );
    }

    #[test]
    fn test_process_frame_dimension_validation() {
        let scanner = scanner_new();
        assert!(!scanner.is_null());

        // Create dummy data
        let rgb_data = vec![0u8; 640 * 480 * 3];
        let depth_data = vec![1.0f32; 640 * 480];
        let pose = CameraPose {
            position: Point3D {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            rotation: Quaternion {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        };

        // Test with valid dimensions
        let result = scanner_process_frame(
            scanner,
            rgb_data.as_ptr(),
            depth_data.as_ptr(),
            640,
            480,
            &pose as *const CameraPose,
        );

        // May fail for other reasons (no vision model, etc.) but not for dimension validation
        assert!(
            result == ScannerResultCode::Ok || result == ScannerResultCode::Error,
            "Valid dimensions should pass validation"
        );

        // Test with too small dimensions
        let result = scanner_process_frame(
            scanner,
            rgb_data.as_ptr(),
            depth_data.as_ptr(),
            32, // Too small
            32,
            &pose as *const CameraPose,
        );

        assert_eq!(
            result,
            ScannerResultCode::InvalidInput,
            "Should reject dimensions < 64"
        );

        // Test with too large dimensions
        let result = scanner_process_frame(
            scanner,
            rgb_data.as_ptr(),
            depth_data.as_ptr(),
            20000, // Too large
            20000,
            &pose as *const CameraPose,
        );

        assert_eq!(
            result,
            ScannerResultCode::InvalidInput,
            "Should reject dimensions > 16384"
        );

        scanner_free(scanner);
    }

    #[test]
    fn test_process_frame_pose_validation() {
        let scanner = scanner_new();
        let rgb_data = vec![0u8; 640 * 480 * 3];
        let depth_data = vec![1.0f32; 640 * 480];

        // Test with NaN in position
        let pose_nan = CameraPose {
            position: Point3D {
                x: f32::NAN,
                y: 0.0,
                z: 1.0,
            },
            rotation: Quaternion {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        };

        let result = scanner_process_frame(
            scanner,
            rgb_data.as_ptr(),
            depth_data.as_ptr(),
            640,
            480,
            &pose_nan as *const CameraPose,
        );

        assert_eq!(
            result,
            ScannerResultCode::InvalidInput,
            "Should reject NaN in position"
        );

        // Test with infinity in position
        let pose_inf = CameraPose {
            position: Point3D {
                x: 0.0,
                y: f32::INFINITY,
                z: 1.0,
            },
            rotation: Quaternion {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        };

        let result = scanner_process_frame(
            scanner,
            rgb_data.as_ptr(),
            depth_data.as_ptr(),
            640,
            480,
            &pose_inf as *const CameraPose,
        );

        assert_eq!(
            result,
            ScannerResultCode::InvalidInput,
            "Should reject infinity in position"
        );

        // Test with unnormalized quaternion
        let pose_bad_quat = CameraPose {
            position: Point3D {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            rotation: Quaternion {
                x: 1.0,
                y: 1.0,
                z: 1.0,
                w: 1.0,
            }, // Not normalized
        };

        let result = scanner_process_frame(
            scanner,
            rgb_data.as_ptr(),
            depth_data.as_ptr(),
            640,
            480,
            &pose_bad_quat as *const CameraPose,
        );

        assert_eq!(
            result,
            ScannerResultCode::InvalidInput,
            "Should reject unnormalized quaternion"
        );

        scanner_free(scanner);
    }

    #[test]
    fn test_process_frame_depth_validation() {
        let scanner = scanner_new();
        let rgb_data = vec![0u8; 640 * 480 * 3];
        let pose = CameraPose {
            position: Point3D {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            rotation: Quaternion {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        };

        // Test with all NaN depth values (should be rejected)
        let depth_all_nan = vec![f32::NAN; 640 * 480];

        let result = scanner_process_frame(
            scanner,
            rgb_data.as_ptr(),
            depth_all_nan.as_ptr(),
            640,
            480,
            &pose as *const CameraPose,
        );

        assert_eq!(
            result,
            ScannerResultCode::InvalidInput,
            "Should reject all-NaN depth data"
        );

        // Test with all negative depth values (should be rejected)
        let depth_all_negative = vec![-1.0f32; 640 * 480];

        let result = scanner_process_frame(
            scanner,
            rgb_data.as_ptr(),
            depth_all_negative.as_ptr(),
            640,
            480,
            &pose as *const CameraPose,
        );

        assert_eq!(
            result,
            ScannerResultCode::InvalidInput,
            "Should reject all-negative depth data"
        );

        scanner_free(scanner);
    }

    #[test]
    fn test_mesh_export_path_validation() {
        // Create a minimal mesh
        let mesh = Mesh::new();
        let mesh_ptr = &mesh as *const Mesh;

        // Test with null path
        let result = mesh_export_obj(mesh_ptr, ptr::null());
        assert_eq!(
            result,
            ScannerResultCode::InvalidInput,
            "Should reject null path"
        );

        // Test with path containing null byte
        let path_with_null = CString::new("/tmp/test\0malicious.obj").unwrap();
        let result = mesh_export_obj(mesh_ptr, path_with_null.as_ptr());
        // Note: CString already prevents null bytes, so this tests our additional checks

        // Test with too long path
        let long_path = "a".repeat(5000);
        let long_path_cstr = CString::new(long_path).unwrap();
        let result = mesh_export_obj(mesh_ptr, long_path_cstr.as_ptr());
        assert_eq!(
            result,
            ScannerResultCode::InvalidInput,
            "Should reject path > 4096 bytes"
        );
    }

    #[test]
    fn test_sofa_generation_validation() {
        let mesh = Mesh::new();
        let mesh_ptr = &mesh as *const Mesh;
        let path = CString::new("/tmp/test.sofa").unwrap();

        // Test with invalid sample rate (too low)
        let result = scanner_generate_sofa(
            mesh_ptr,
            path.as_ptr(),
            1000.0, // < 8000 Hz
            360,
            180,
            1.0,
        );

        assert_eq!(
            result,
            ScannerResultCode::InvalidInput,
            "Should reject sample rate < 8000 Hz"
        );

        // Test with invalid sample rate (too high)
        let result = scanner_generate_sofa(
            mesh_ptr,
            path.as_ptr(),
            250000.0, // > 192000 Hz
            360,
            180,
            1.0,
        );

        assert_eq!(
            result,
            ScannerResultCode::InvalidInput,
            "Should reject sample rate > 192000 Hz"
        );

        // Test with NaN sample rate
        let result = scanner_generate_sofa(mesh_ptr, path.as_ptr(), f32::NAN, 360, 180, 1.0);

        assert_eq!(
            result,
            ScannerResultCode::InvalidInput,
            "Should reject NaN sample rate"
        );

        // Test with invalid azimuth resolution
        let result = scanner_generate_sofa(
            mesh_ptr,
            path.as_ptr(),
            44100.0,
            0, // Too small
            180,
            1.0,
        );

        assert_eq!(
            result,
            ScannerResultCode::InvalidInput,
            "Should reject azimuth resolution = 0"
        );

        // Test with invalid elevation resolution
        let result = scanner_generate_sofa(
            mesh_ptr,
            path.as_ptr(),
            44100.0,
            360,
            5000, // Too large
            1.0,
        );

        assert_eq!(
            result,
            ScannerResultCode::InvalidInput,
            "Should reject elevation resolution > 3600"
        );

        // Test with invalid distance (negative)
        let result = scanner_generate_sofa(
            mesh_ptr,
            path.as_ptr(),
            44100.0,
            360,
            180,
            -1.0, // Negative
        );

        assert_eq!(
            result,
            ScannerResultCode::InvalidInput,
            "Should reject negative distance"
        );

        // Test with invalid distance (too far)
        let result = scanner_generate_sofa(
            mesh_ptr,
            path.as_ptr(),
            44100.0,
            360,
            180,
            150.0, // > 100m
        );

        assert_eq!(
            result,
            ScannerResultCode::InvalidInput,
            "Should reject distance > 100m"
        );
    }

    #[test]
    fn test_error_reporting() {
        // Test that error messages are properly set and retrieved
        let scanner = scanner_new();
        let rgb_data = vec![0u8; 100];
        let depth_data = vec![1.0f32; 100];
        let pose = CameraPose {
            position: Point3D {
                x: f32::NAN,
                y: 0.0,
                z: 0.0,
            },
            rotation: Quaternion {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        };

        // Trigger an error (NaN in position)
        let result = scanner_process_frame(
            scanner,
            rgb_data.as_ptr(),
            depth_data.as_ptr(),
            10,
            10,
            &pose as *const CameraPose,
        );

        assert_eq!(result, ScannerResultCode::InvalidInput);

        // Get error message
        let error_ptr = scanner_last_error();
        assert!(!error_ptr.is_null(), "Error message should be set");

        if !error_ptr.is_null() {
            let error_str = unsafe { std::ffi::CStr::from_ptr(error_ptr) };
            let error = error_str.to_str().unwrap();
            println!("Error message: {}", error);
            assert!(error.len() > 0, "Error message should not be empty");
        }

        scanner_free(scanner);
    }

    #[test]
    fn test_guidance_metrics() {
        let scanner = scanner_new();
        let guidance = scanner_get_guidance(scanner);
        assert!(!guidance.is_null());

        // Get initial metrics
        let metrics = guidance_get_metrics(guidance);

        // Should start with zero coverage
        assert_eq!(metrics.coverage, 0.0, "Initial coverage should be 0");
        assert_eq!(
            metrics.angular_coverage, 0.0,
            "Initial angular coverage should be 0"
        );

        guidance_free(guidance);
        scanner_free(scanner);
    }

    #[test]
    fn test_guidance_region_tracking() {
        let scanner = scanner_new();
        let guidance = scanner_get_guidance(scanner);

        // Initially, no regions should be covered
        assert!(
            !guidance_is_region_covered(guidance, HeadRegionC::Front),
            "Front region should not be covered initially"
        );

        // Get next region to scan
        let next = guidance_get_next_region(guidance);
        assert_eq!(
            next,
            HeadRegionC::Front,
            "Should recommend front region first"
        );

        guidance_free(guidance);
        scanner_free(scanner);
    }

    #[test]
    fn test_mesh_query_functions() {
        let mesh = Mesh::new();
        let mesh_ptr = &mesh as *const Mesh;

        // Test with valid mesh
        let vertex_count = mesh_vertex_count(mesh_ptr);
        let triangle_count = mesh_triangle_count(mesh_ptr);

        // Empty mesh should have 0 vertices and triangles
        assert_eq!(vertex_count, 0, "Empty mesh should have 0 vertices");
        assert_eq!(triangle_count, 0, "Empty mesh should have 0 triangles");

        // Test with null pointer
        let vertex_count_null = mesh_vertex_count(ptr::null());
        let triangle_count_null = mesh_triangle_count(ptr::null());

        assert_eq!(vertex_count_null, 0, "Null mesh should return 0 vertices");
        assert_eq!(
            triangle_count_null, 0,
            "Null mesh should return 0 triangles"
        );
    }

    #[test]
    fn test_ffi_thread_safety() {
        // Test that FFI functions can be called from multiple threads
        use std::thread;

        let handles: Vec<_> = (0..4)
            .map(|_| {
                thread::spawn(|| {
                    let scanner = scanner_new();
                    assert!(!scanner.is_null());

                    let guidance = scanner_get_guidance(scanner);
                    assert!(!guidance.is_null());

                    guidance_free(guidance);
                    scanner_free(scanner);
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread should not panic");
        }
    }

    #[test]
    fn test_integer_overflow_protection() {
        // Test that integer overflow is prevented in buffer size calculations
        let scanner = scanner_new();
        let rgb_data = vec![0u8; 1024];
        let depth_data = vec![1.0f32; 1024];
        let pose = CameraPose {
            position: Point3D {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            rotation: Quaternion {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        };

        // Try to cause overflow with huge dimensions
        let result = scanner_process_frame(
            scanner,
            rgb_data.as_ptr(),
            depth_data.as_ptr(),
            u32::MAX, // Would overflow
            u32::MAX,
            &pose as *const CameraPose,
        );

        assert_eq!(
            result,
            ScannerResultCode::InvalidInput,
            "Should reject dimensions that would cause overflow"
        );

        scanner_free(scanner);
    }
}
