//! 3D Convex Hull computation
//!
//! This module provides a wrapper around the convexhull3d crate for computing
//! the convex hull of a 3D point cloud. The convex hull is the smallest convex
//! polyhedron that contains all the points.

use crate::error::{ScannerError, ScannerResult};
use crate::pointcloud::PointCloud;
use convexhull3d::{ConvexHull3D as ExternalHull, Vertex};
use nalgebra::Point3;
use serde::{Deserialize, Serialize};

/// A 3D convex hull represented as a polyhedron
///
/// This wraps the convexhull3d crate's implementation with nalgebra types
/// for compatibility with the rest of the head_scanner crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvexHull3D {
    /// Vertices of the hull
    vertices: Vec<Point3<f32>>,

    /// Faces as triplets of vertex indices (counter-clockwise winding)
    faces: Vec<[usize; 3]>,
}

impl ConvexHull3D {
    /// Create a new convex hull
    pub fn new(vertices: Vec<Point3<f32>>, faces: Vec<[usize; 3]>) -> Self {
        Self { vertices, faces }
    }

    /// Get the vertices
    pub fn vertices(&self) -> &[Point3<f32>] {
        &self.vertices
    }

    /// Get the faces
    pub fn faces(&self) -> &[[usize; 3]] {
        &self.faces
    }

    /// Get the number of vertices
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Get the number of faces
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    /// Compute the volume of the convex hull
    pub fn volume(&self) -> f32 {
        let mut volume = 0.0;

        // Use the origin as reference point
        let origin = Point3::origin();

        for face in &self.faces {
            let v0 = self.vertices[face[0]];
            let v1 = self.vertices[face[1]];
            let v2 = self.vertices[face[2]];

            // Compute signed volume of tetrahedron
            let a = v0 - origin;
            let b = v1 - origin;
            let c = v2 - origin;

            volume += a.dot(&b.cross(&c)) / 6.0;
        }

        volume.abs()
    }

    /// Compute the surface area of the convex hull
    pub fn surface_area(&self) -> f32 {
        let mut area = 0.0;

        for face in &self.faces {
            let v0 = self.vertices[face[0]];
            let v1 = self.vertices[face[1]];
            let v2 = self.vertices[face[2]];

            let edge1 = v1 - v0;
            let edge2 = v2 - v0;

            area += edge1.cross(&edge2).magnitude() * 0.5;
        }

        area
    }
}

/// Compute the 3D convex hull of a point cloud using the Quickhull algorithm
///
/// # Type Conversion Notes
///
/// This function converts f32 → f64 → f32 during processing:
/// 1. Input: Point3<f32> from point cloud
/// 2. Conversion to f64 for convexhull3d crate (uses f64 internally for precision)
/// 3. Conversion back to f32 for output
///
/// **Precision**: For typical head scanning use cases (coordinates < 1000 cm),
/// f32 provides ~7 decimal digits of precision, which is sufficient. However,
/// for very large coordinate values, the f64→f32 conversion may lose precision.
///
/// If your application requires higher precision, consider keeping the output
/// as f64 throughout the pipeline.
pub fn compute_convex_hull_3d(point_cloud: &PointCloud) -> ScannerResult<ConvexHull3D> {
    let points: Vec<Point3<f32>> = point_cloud.points().iter().map(|p| p.position).collect();

    if points.len() < 4 {
        return Err(ScannerError::InsufficientData(
            "At least 4 points required for 3D convex hull".to_string(),
        ));
    }

    // Convert nalgebra Point3<f32> to convexhull3d Vertex (f64)
    // Using f64 for convex hull computation provides better numerical stability
    let vertices: Vec<Vertex> = points
        .iter()
        .map(|p| Vertex::new(p.x as f64, p.y as f64, p.z as f64))
        .collect();

    // Compute convex hull using the external crate
    let hull = ExternalHull::build(&vertices)
        .map_err(|e| ScannerError::ConvexHull(format!("Convex hull computation failed: {}", e)))?;

    // Convert back to nalgebra types
    let output_vertices: Vec<Point3<f32>> = hull
        .vertices()
        .iter()
        .map(|v| Point3::new(v.x as f32, v.y as f32, v.z as f32))
        .collect();

    let output_faces: Vec<[usize; 3]> = hull.faces().iter().map(|f| [f.v0, f.v1, f.v2]).collect();

    Ok(ConvexHull3D::new(output_vertices, output_faces))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pointcloud::Point;

    #[test]
    fn test_convex_hull_tetrahedron() {
        let mut cloud = PointCloud::new();
        cloud.add_point(Point::new(0.0, 0.0, 0.0));
        cloud.add_point(Point::new(1.0, 0.0, 0.0));
        cloud.add_point(Point::new(0.0, 1.0, 0.0));
        cloud.add_point(Point::new(0.0, 0.0, 1.0));

        let hull = compute_convex_hull_3d(&cloud);
        assert!(hull.is_ok());

        let hull = hull.unwrap();
        assert_eq!(hull.vertex_count(), 4);
        assert_eq!(hull.face_count(), 4); // Tetrahedron has 4 faces
        assert!(hull.surface_area() > 0.0);
        assert!(hull.volume() > 0.0);
    }

    #[test]
    fn test_convex_hull_cube() {
        let mut cloud = PointCloud::new();
        // Cube vertices
        cloud.add_point(Point::new(0.0, 0.0, 0.0));
        cloud.add_point(Point::new(1.0, 0.0, 0.0));
        cloud.add_point(Point::new(0.0, 1.0, 0.0));
        cloud.add_point(Point::new(1.0, 1.0, 0.0));
        cloud.add_point(Point::new(0.0, 0.0, 1.0));
        cloud.add_point(Point::new(1.0, 0.0, 1.0));
        cloud.add_point(Point::new(0.0, 1.0, 1.0));
        cloud.add_point(Point::new(1.0, 1.0, 1.0));

        let hull = compute_convex_hull_3d(&cloud);
        assert!(hull.is_ok());

        let hull = hull.unwrap();
        assert_eq!(hull.vertex_count(), 8);
        assert!(hull.face_count() >= 6); // At least 6 faces, may be more if triangulated
        assert!((hull.volume() - 1.0).abs() < 0.01); // Volume should be 1.0
    }

    #[test]
    fn test_insufficient_points() {
        let mut cloud = PointCloud::new();
        cloud.add_point(Point::new(0.0, 0.0, 0.0));
        cloud.add_point(Point::new(1.0, 0.0, 0.0));
        cloud.add_point(Point::new(0.0, 1.0, 0.0));

        let hull = compute_convex_hull_3d(&cloud);
        assert!(hull.is_err());
    }

    #[test]
    fn test_hull_properties() {
        let mut cloud = PointCloud::new();
        // Sphere-like point cloud
        for i in 0..10 {
            let angle = std::f32::consts::PI * 2.0 * i as f32 / 10.0;
            cloud.add_point(Point::new(angle.cos(), angle.sin(), 0.0));
            cloud.add_point(Point::new(angle.cos(), angle.sin(), 1.0));
        }
        cloud.add_point(Point::new(0.0, 0.0, 0.5));

        let hull = compute_convex_hull_3d(&cloud).unwrap();

        // Hull should have positive volume and area
        assert!(hull.volume() > 0.0);
        assert!(hull.surface_area() > 0.0);

        // Number of vertices should be <= original points
        assert!(hull.vertex_count() <= cloud.len());
    }
}
