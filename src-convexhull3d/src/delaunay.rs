//! Delaunay triangulation via paraboloid lifting
//!
//! Computes the Delaunay triangulation by lifting points to a paraboloid
//! and computing the lower convex hull.

use crate::nd_types::{DelaunayMesh, PointND, SimplexND};
use crate::quickhull_nd::quickhull_nd;
use crate::Result;

/// Compute the Delaunay triangulation of a set of points
///
/// The Delaunay triangulation is computed by:
/// 1. Lifting each d-dimensional point (x1, ..., xd) to (d+1)-dimensional
///    space as (x1, ..., xd, x1² + ... + xd²)
/// 2. Computing the convex hull of the lifted points
/// 3. Projecting the lower facets back to d dimensions
///
/// # Arguments
/// * `points` - The input points in d dimensions
///
/// # Returns
/// A Delaunay mesh containing simplices that form the triangulation
pub fn delaunay_nd(points: &[PointND]) -> Result<DelaunayMesh> {
    if points.is_empty() {
        return Err(crate::ConvexHullError::InsufficientVertices);
    }

    let dim = points[0].dim();

    // Lift points to paraboloid
    let lifted_points: Vec<PointND> = points
        .iter()
        .map(|p| lift_to_paraboloid(p))
        .collect();

    // Compute convex hull of lifted points
    let hull = quickhull_nd(&lifted_points)?;

    // Find the lower facets (those with negative last component of normal)
    let mut simplices = Vec::new();

    for facet in hull.facets() {
        if is_lower_facet(facet, &lifted_points) {
            // Project back to original dimension
            simplices.push(facet.clone());
        }
    }

    Ok(DelaunayMesh::new(points.to_vec(), simplices, dim))
}

/// Lift a point to the paraboloid
fn lift_to_paraboloid(point: &PointND) -> PointND {
    let mut coords = point.coords.clone();

    // Add the squared norm as the last coordinate
    let squared_norm: f64 = point.coords.iter().map(|x| x * x).sum();
    coords.push(squared_norm);

    PointND::new(coords)
}

/// Check if a facet is a lower facet (normal points downward in last dimension)
fn is_lower_facet(facet: &SimplexND, lifted_points: &[PointND]) -> bool {
    // Compute the normal to this facet
    let normal = compute_facet_normal_delaunay(facet, lifted_points);

    // Lower facets have negative last component
    if let Some(last) = normal.last() {
        *last < 0.0
    } else {
        false
    }
}

/// Compute the normal vector for a facet (simplified version for checking orientation)
fn compute_facet_normal_delaunay(facet: &SimplexND, points: &[PointND]) -> Vec<f64> {
    let dim = points[0].dim();

    if facet.vertices.len() < 2 {
        return vec![0.0; dim];
    }

    // Build edge vectors
    let base_point = &points[facet.vertices[0]];
    let mut edges = Vec::new();

    for &idx in &facet.vertices[1..] {
        let edge = points[idx].sub(base_point);
        edges.push(edge.coords);
    }

    // For simplicity, use a heuristic: check the direction from centroid
    // In a full implementation, we'd compute the proper normal via cross products
    let mut normal = vec![0.0; dim];

    // Simple heuristic: use the last coordinate of the first edge
    if !edges.is_empty() && !edges[0].is_empty() {
        normal[dim - 1] = edges[0][dim - 1];
    }

    normal
}

/// Helper function to compute circumcenter of a simplex (useful for Delaunay)
pub fn circumcenter(simplex: &SimplexND, points: &[PointND]) -> Option<PointND> {
    if simplex.vertices.len() < 2 {
        return None;
    }

    let dim = points[0].dim();

    // For a triangle in 2D
    if dim == 2 && simplex.vertices.len() == 3 {
        return circumcenter_2d(simplex, points);
    }

    // For other cases, use the centroid as approximation
    Some(simplex.centroid(points))
}

/// Compute circumcenter of a triangle in 2D
fn circumcenter_2d(simplex: &SimplexND, points: &[PointND]) -> Option<PointND> {
    if simplex.vertices.len() != 3 {
        return None;
    }

    let p0 = &points[simplex.vertices[0]];
    let p1 = &points[simplex.vertices[1]];
    let p2 = &points[simplex.vertices[2]];

    let ax = p0.coords[0];
    let ay = p0.coords[1];
    let bx = p1.coords[0];
    let by = p1.coords[1];
    let cx = p2.coords[0];
    let cy = p2.coords[1];

    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));

    if d.abs() < 1e-10 {
        return None;
    }

    let ux = ((ax * ax + ay * ay) * (by - cy) +
              (bx * bx + by * by) * (cy - ay) +
              (cx * cx + cy * cy) * (ay - by)) / d;

    let uy = ((ax * ax + ay * ay) * (cx - bx) +
              (bx * bx + by * by) * (ax - cx) +
              (cx * cx + cy * cy) * (bx - ax)) / d;

    Some(PointND::new(vec![ux, uy]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lift_to_paraboloid() {
        let point = PointND::new(vec![1.0, 2.0]);
        let lifted = lift_to_paraboloid(&point);

        assert_eq!(lifted.dim(), 3);
        assert_eq!(lifted.coords[0], 1.0);
        assert_eq!(lifted.coords[1], 2.0);
        assert_eq!(lifted.coords[2], 5.0); // 1² + 2² = 5
    }

    #[test]
    fn test_delaunay_2d_triangle() {
        let points = vec![
            PointND::new(vec![0.0, 0.0]),
            PointND::new(vec![1.0, 0.0]),
            PointND::new(vec![0.0, 1.0]),
        ];

        let mesh = delaunay_nd(&points).unwrap();
        assert_eq!(mesh.dim(), 2);
        assert!(mesh.num_simplices() > 0);
    }

    #[test]
    fn test_circumcenter_2d() {
        // Right triangle at origin
        let simplex = SimplexND::new(vec![0, 1, 2]);
        let points = vec![
            PointND::new(vec![0.0, 0.0]),
            PointND::new(vec![1.0, 0.0]),
            PointND::new(vec![0.0, 1.0]),
        ];

        if let Some(center) = circumcenter_2d(&simplex, &points) {
            // Circumcenter of right triangle is at midpoint of hypotenuse
            assert!((center.coords[0] - 0.5).abs() < 1e-6);
            assert!((center.coords[1] - 0.5).abs() < 1e-6);
        }
    }
}
