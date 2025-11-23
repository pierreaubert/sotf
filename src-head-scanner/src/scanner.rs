//! Simplified Scanner interface for FFI
//!
//! This module provides a simpler, synchronous wrapper around HeadScanner
//! that is more suitable for FFI bindings.

use crate::error::{ScannerError, ScannerResult};
use crate::guidance::ScanGuidance;
use crate::mesh::Mesh;
use crate::pointcloud::{Point, PointCloud};
use crate::{HeadScanner, ScannerConfig};
use nalgebra::{Point3, UnitQuaternion};

/// Simplified scanner interface for FFI
///
/// This wraps HeadScanner but provides a synchronous interface suitable
/// for C/Swift FFI where async is not available.
pub struct Scanner {
    inner: HeadScanner,
    point_cloud: PointCloud,
}

impl Scanner {
    /// Create a new scanner with default configuration
    pub fn new() -> Self {
        let config = ScannerConfig::default();
        let inner = HeadScanner::new(config).expect("Failed to create scanner");

        Self {
            inner,
            point_cloud: PointCloud::new(),
        }
    }

    /// Process a frame from RGB and depth data
    ///
    /// This is a synchronous wrapper for FFI that processes RGB-D frames
    /// and updates the internal point cloud.
    ///
    /// # Parameters
    /// - `rgb`: RGB image data (width * height * 3 bytes)
    /// - `depth`: Depth map (width * height floats, in cm)
    /// - `width`: Image width in pixels
    /// - `height`: Image height in pixels
    /// - `position`: Camera position in 3D space (cm)
    /// - `rotation`: Camera orientation as unit quaternion
    pub fn process_frame(
        &mut self,
        rgb: &[u8],
        depth: &[f32],
        width: u32,
        height: u32,
        position: Point3<f32>,
        rotation: UnitQuaternion<f32>,
    ) -> ScannerResult<()> {
        let pixel_count = (width * height) as usize;

        // Validate buffer sizes
        if rgb.len() != pixel_count * 3 {
            return Err(ScannerError::InvalidConfig(format!(
                "RGB buffer size mismatch: expected {}, got {}",
                pixel_count * 3,
                rgb.len()
            )));
        }

        if depth.len() != pixel_count {
            return Err(ScannerError::InvalidConfig(format!(
                "Depth buffer size mismatch: expected {}, got {}",
                pixel_count,
                depth.len()
            )));
        }

        // Convert depth map to 3D points
        // Use camera intrinsics for projection (simple pinhole model)
        let fx = width as f32 * 0.8; // Typical focal length
        let fy = width as f32 * 0.8;
        let cx = width as f32 / 2.0;
        let cy = height as f32 / 2.0;

        let mut new_points = Vec::new();

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                let d = depth[idx];

                // Skip invalid depth values
                if !d.is_finite() || d <= 0.0 || d > 200.0 {
                    continue;
                }

                // Back-project pixel to 3D point in camera coordinates
                let x_cam = (x as f32 - cx) * d / fx;
                let y_cam = (y as f32 - cy) * d / fy;
                let z_cam = d;

                // Transform to world coordinates using camera pose
                let point_cam = Point3::new(x_cam, y_cam, z_cam);
                let point_world = rotation * point_cam + position.coords;

                // Get RGB color
                let rgb_idx = idx * 3;
                let color = if rgb_idx + 2 < rgb.len() {
                    Some([rgb[rgb_idx], rgb[rgb_idx + 1], rgb[rgb_idx + 2]])
                } else {
                    None
                };

                new_points.push(Point {
                    position: point_world,
                    color,
                    normal: None,
                    confidence: 1.0,
                });
            }
        }

        // Add points to the point cloud (with deduplication)
        self.point_cloud.add_points(&new_points);

        // Periodically downsample to control memory
        if self.point_cloud.len() % 1000 == 0 && self.point_cloud.len() > 0 {
            self.point_cloud.voxel_downsample(1.0); // 1cm voxel size
        }

        Ok(())
    }

    /// Get the reconstructed mesh from the current point cloud
    ///
    /// This generates a mesh from the accumulated point cloud using
    /// Poisson surface reconstruction or similar algorithm.
    pub fn get_mesh(&self) -> ScannerResult<Mesh> {
        if self.point_cloud.is_empty() {
            return Err(ScannerError::InvalidConfig(
                "Cannot generate mesh from empty point cloud".to_string(),
            ));
        }

        // For now, create a simple mesh from the point cloud
        // In a full implementation, this would use Poisson reconstruction or
        // other surface reconstruction algorithms

        // Collect all points as vertices
        let mut vertices: Vec<crate::mesh::Vertex> = Vec::new();

        // If we have enough points, estimate normals first
        if self.point_cloud.len() >= 10 {
            // Create a temporary mutable copy for normal estimation
            let mut temp_cloud = self.point_cloud.clone();
            temp_cloud.estimate_normals(10);

            // Collect vertices with estimated normals
            for point in temp_cloud.points() {
                vertices.push(
                    crate::mesh::Vertex::from_point(point.position)
                        .with_normal(point.normal)
                        .with_color(point.color),
                );
            }
        } else {
            // Collect vertices without normal estimation
            for point in self.point_cloud.points() {
                vertices.push(
                    crate::mesh::Vertex::from_point(point.position)
                        .with_normal(point.normal)
                        .with_color(point.color),
                );
            }
        }

        // Create mesh from collected vertices (no triangles yet)
        let mesh = Mesh::from_parts(vertices, Vec::new());

        // TODO: Implement actual surface reconstruction
        // For now, we just return the vertices without connectivity
        // A full implementation would:
        // 1. Estimate normals if not present
        // 2. Run Poisson surface reconstruction or Delaunay triangulation
        // 3. Generate proper triangle connectivity

        Ok(mesh)
    }

    /// Get the scan guidance system
    pub fn get_guidance(&self) -> ScanGuidance {
        self.inner.get_guidance()
    }

    /// Get the current point cloud size
    pub fn point_count(&self) -> usize {
        self.point_cloud.len()
    }
}

impl Default for Scanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scanner_creation() {
        let scanner = Scanner::new();
        assert_eq!(scanner.point_count(), 0);
    }

    #[test]
    fn test_process_frame_validation() {
        let mut scanner = Scanner::new();

        let width = 640u32;
        let height = 480u32;
        let pixel_count = (width * height) as usize;

        // Valid buffers
        let rgb = vec![0u8; pixel_count * 3];
        let depth = vec![1.0f32; pixel_count];
        let position = Point3::new(0.0, 0.0, 100.0);
        let rotation = UnitQuaternion::identity();

        let result = scanner.process_frame(&rgb, &depth, width, height, position, rotation);
        assert!(result.is_ok());

        // Invalid RGB buffer size
        let bad_rgb = vec![0u8; 100];
        let result = scanner.process_frame(&bad_rgb, &depth, width, height, position, rotation);
        assert!(result.is_err());

        // Invalid depth buffer size
        let bad_depth = vec![1.0f32; 100];
        let result = scanner.process_frame(&rgb, &bad_depth, width, height, position, rotation);
        assert!(result.is_err());
    }

    #[test]
    fn test_point_accumulation() {
        let mut scanner = Scanner::new();

        let width = 64u32;
        let height = 48u32;
        let pixel_count = (width * height) as usize;

        let rgb = vec![128u8; pixel_count * 3];
        let depth = vec![100.0f32; pixel_count]; // All points at 100cm depth
        let position = Point3::new(0.0, 0.0, 0.0);
        let rotation = UnitQuaternion::identity();

        // Process first frame
        scanner
            .process_frame(&rgb, &depth, width, height, position, rotation)
            .unwrap();

        assert!(scanner.point_count() > 0, "Should have accumulated points");

        let first_count = scanner.point_count();

        // Process second frame from different position
        let position2 = Point3::new(10.0, 0.0, 0.0);
        scanner
            .process_frame(&rgb, &depth, width, height, position2, rotation)
            .unwrap();

        // Should have more points now (some may be deduplicated)
        assert!(
            scanner.point_count() >= first_count,
            "Should accumulate points from multiple frames"
        );
    }

    #[test]
    fn test_get_mesh_empty() {
        let scanner = Scanner::new();
        let result = scanner.get_mesh();
        assert!(
            result.is_err(),
            "Should fail to generate mesh from empty cloud"
        );
    }

    #[test]
    fn test_get_mesh_with_points() {
        let mut scanner = Scanner::new();

        // Add some points
        let width = 32u32;
        let height = 24u32;
        let pixel_count = (width * height) as usize;

        let rgb = vec![255u8; pixel_count * 3];
        let depth = vec![50.0f32; pixel_count];
        let position = Point3::new(0.0, 0.0, 0.0);
        let rotation = UnitQuaternion::identity();

        scanner
            .process_frame(&rgb, &depth, width, height, position, rotation)
            .unwrap();

        // Generate mesh
        let result = scanner.get_mesh();
        assert!(result.is_ok(), "Should generate mesh from point cloud");

        let mesh = result.unwrap();
        assert!(mesh.vertex_count() > 0, "Mesh should have vertices");
    }
}
