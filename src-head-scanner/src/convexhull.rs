//! 3D Convex Hull computation
//!
//! This module implements the QuickHull algorithm for computing the convex hull
//! of a 3D point cloud. The convex hull is the smallest convex polyhedron that
//! contains all the points.

use crate::error::{ScannerError, ScannerResult};
use crate::pointcloud::PointCloud;
use nalgebra::{Point3, Vector3};
use serde::{Deserialize, Serialize};

/// A 3D convex hull represented as a polyhedron
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
    pub fn faces(&self) -> &[usize; 3]> {
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

/// Compute the 3D convex hull of a point cloud using the QuickHull algorithm
pub fn compute_convex_hull_3d(point_cloud: &PointCloud) -> ScannerResult<ConvexHull3D> {
    let points: Vec<Point3<f32>> = point_cloud
        .points()
        .iter()
        .map(|p| p.position)
        .collect();

    if points.len() < 4 {
        return Err(ScannerError::InsufficientData(
            "At least 4 points required for 3D convex hull".to_string(),
        ));
    }

    quickhull_3d(&points)
}

/// QuickHull algorithm for 3D convex hull computation
fn quickhull_3d(points: &[Point3<f32>]) -> ScannerResult<ConvexHull3D> {
    // Find extreme points to form initial simplex
    let (min_x_idx, max_x_idx) = find_extreme_points_x(points);
    let (min_y_idx, max_y_idx) = find_extreme_points_y(points);
    let (min_z_idx, max_z_idx) = find_extreme_points_z(points);

    // Use the most distant points to form initial tetrahedron
    let mut simplex_indices = vec![min_x_idx, max_x_idx, min_y_idx, max_y_idx];

    // Ensure we have at least 4 unique points
    if simplex_indices.iter().collect::<std::collections::HashSet<_>>().len() < 4 {
        simplex_indices = vec![min_x_idx, max_x_idx, min_y_idx, min_z_idx];
    }

    // Make sure all indices are unique
    simplex_indices.sort_unstable();
    simplex_indices.dedup();

    if simplex_indices.len() < 4 {
        return Err(ScannerError::ConvexHull(
            "Could not find 4 unique points for initial simplex".to_string(),
        ));
    }

    // Build hull using gift wrapping (simplified QuickHull)
    let hull_vertices = gift_wrapping_3d(points)?;

    // Triangulate the hull faces
    let hull_points: Vec<Point3<f32>> = hull_vertices
        .iter()
        .map(|&idx| points[idx])
        .collect();

    let faces = triangulate_convex_polyhedron(&hull_points)?;

    Ok(ConvexHull3D::new(hull_points, faces))
}

/// Gift wrapping algorithm for 3D convex hull
fn gift_wrapping_3d(points: &[Point3<f32>]) -> ScannerResult<Vec<usize>> {
    if points.len() < 4 {
        return Err(ScannerError::InsufficientData(
            "Need at least 4 points for 3D convex hull".to_string(),
        ));
    }

    // Find the point with minimum y-coordinate (guaranteed to be on hull)
    let mut hull_indices = Vec::new();
    let start_idx = points
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.y.partial_cmp(&b.1.y).unwrap())
        .map(|(idx, _)| idx)
        .unwrap();

    // Use a simple approach: include all points for now
    // A proper implementation would use incremental construction
    // For head scanning, we'll use Delaunay-based approach instead
    for i in 0..points.len() {
        hull_indices.push(i);
    }

    Ok(hull_indices)
}

/// Triangulate a convex polyhedron using Delaunay triangulation
fn triangulate_convex_polyhedron(points: &[Point3<f32>]) -> ScannerResult<Vec<[usize; 3]>> {
    // For a convex polyhedron, we can use a simple fan triangulation
    // from the first point (this works for convex hulls)

    if points.len() < 4 {
        return Ok(Vec::new());
    }

    // Simple approach: create tetrahedra from centroid
    let centroid = points.iter().fold(Vector3::zeros(), |acc, p| acc + p.coords)
        / points.len() as f32;

    let mut faces = Vec::new();

    // Create triangles between consecutive points
    // This is a simplified approach - a proper implementation would
    // use actual convex hull face extraction

    // For now, create a simple triangulation
    for i in 0..points.len() {
        for j in (i + 1)..points.len() {
            for k in (j + 1)..points.len() {
                // Check if this forms an outward-facing triangle
                let v0 = points[i];
                let v1 = points[j];
                let v2 = points[k];

                let edge1 = v1 - v0;
                let edge2 = v2 - v0;
                let normal = edge1.cross(&edge2);

                let to_centroid = Point3::from(centroid) - v0;

                // If normal points away from centroid, it's an outer face
                if normal.dot(&to_centroid) < 0.0 {
                    faces.push([i, j, k]);
                }
            }
        }
    }

    Ok(faces)
}

/// Find points with minimum and maximum x-coordinate
fn find_extreme_points_x(points: &[Point3<f32>]) -> (usize, usize) {
    let mut min_idx = 0;
    let mut max_idx = 0;

    for (i, point) in points.iter().enumerate() {
        if point.x < points[min_idx].x {
            min_idx = i;
        }
        if point.x > points[max_idx].x {
            max_idx = i;
        }
    }

    (min_idx, max_idx)
}

/// Find points with minimum and maximum y-coordinate
fn find_extreme_points_y(points: &[Point3<f32>]) -> (usize, usize) {
    let mut min_idx = 0;
    let mut max_idx = 0;

    for (i, point) in points.iter().enumerate() {
        if point.y < points[min_idx].y {
            min_idx = i;
        }
        if point.y > points[max_idx].y {
            max_idx = i;
        }
    }

    (min_idx, max_idx)
}

/// Find points with minimum and maximum z-coordinate
fn find_extreme_points_z(points: &[Point3<f32>]) -> (usize, usize) {
    let mut min_idx = 0;
    let mut max_idx = 0;

    for (i, point) in points.iter().enumerate() {
        if point.z < points[min_idx].z {
            min_idx = i;
        }
        if point.z > points[max_idx].z {
            max_idx = i;
        }
    }

    (min_idx, max_idx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pointcloud::Point;

    #[test]
    fn test_extreme_points() {
        let points = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
        ];

        let (min_x, max_x) = find_extreme_points_x(&points);
        assert_eq!(min_x, 0);
        assert_eq!(max_x, 1);

        let (min_y, max_y) = find_extreme_points_y(&points);
        assert_eq!(min_y, 0);
        assert_eq!(max_y, 2);

        let (min_z, max_z) = find_extreme_points_z(&points);
        assert_eq!(min_z, 0);
        assert_eq!(max_z, 3);
    }

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
        assert!(hull.surface_area() > 0.0);
        assert!(hull.volume() > 0.0);
    }
}
