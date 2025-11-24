//! Quickhull algorithm implementation for 3D convex hulls
//!
//! Based on:
//! - Barber, C.B., Dobkin, D.P., and Huhdanpaa, H.T., "The Quickhull algorithm
//!   for convex hulls," ACM Trans. on Mathematical Software, 22(4):469-483, 1996.

use crate::geometry::{are_coplanar, find_extreme_points};
use crate::types::{ConvexHull3D, Face, Vertex};
use crate::{ConvexHullError, EPSILON, Result};
use std::collections::{HashMap, HashSet};

const MAX_ITERATIONS: usize = 100000;

/// Internal representation of a face during hull construction
#[derive(Debug, Clone)]
struct HullFace {
    vertices: [usize; 3],
    normal: Vertex,
    centroid: Vertex,
    outside_points: Vec<usize>,
}

impl HullFace {
    fn new(v0: usize, v1: usize, v2: usize, vertices: &[Vertex]) -> Self {
        let vertices_arr = [v0, v1, v2];
        let face = Face::new(v0, v1, v2);
        let normal = face.normal(vertices);
        let centroid = face.centroid(vertices);

        Self {
            vertices: vertices_arr,
            normal,
            centroid,
            outside_points: Vec::new(),
        }
    }

    fn is_visible_from(&self, point: &Vertex, vertices: &[Vertex]) -> bool {
        let v0 = &vertices[self.vertices[0]];
        let to_point = point.sub(v0);
        self.normal.dot(&to_point) > EPSILON
    }

    fn assign_point(&mut self, point_idx: usize) {
        self.outside_points.push(point_idx);
    }

    fn furthest_point(&self, vertices: &[Vertex]) -> Option<usize> {
        let mut max_distance = 0.0;
        let mut max_idx = None;

        for &idx in &self.outside_points {
            let point = &vertices[idx];
            let v0 = &vertices[self.vertices[0]];
            let to_point = point.sub(v0);
            let distance = self.normal.dot(&to_point);

            if distance > max_distance {
                max_distance = distance;
                max_idx = Some(idx);
            }
        }

        max_idx
    }

    fn to_face(&self) -> Face {
        Face::new(self.vertices[0], self.vertices[1], self.vertices[2])
    }
}

/// Edge representation for horizon computation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Edge {
    v0: usize,
    v1: usize,
}

impl Edge {
    fn new(v0: usize, v1: usize) -> Self {
        // Normalize edge orientation for consistent hashing
        if v0 < v1 {
            Self { v0, v1 }
        } else {
            Self { v0: v1, v1: v0 }
        }
    }
}

/// Build a convex hull using the Quickhull algorithm
pub fn quickhull_3d(vertices: &[Vertex]) -> Result<ConvexHull3D> {
    if vertices.len() < 4 {
        return Err(ConvexHullError::InsufficientVertices);
    }

    // Find initial simplex (tetrahedron)
    let initial_simplex = find_initial_simplex(vertices)?;

    // Compute the centroid of the initial simplex - this is guaranteed to be inside the hull
    let simplex_centroid = Vertex {
        x: (vertices[initial_simplex[0]].x
            + vertices[initial_simplex[1]].x
            + vertices[initial_simplex[2]].x
            + vertices[initial_simplex[3]].x)
            / 4.0,
        y: (vertices[initial_simplex[0]].y
            + vertices[initial_simplex[1]].y
            + vertices[initial_simplex[2]].y
            + vertices[initial_simplex[3]].y)
            / 4.0,
        z: (vertices[initial_simplex[0]].z
            + vertices[initial_simplex[1]].z
            + vertices[initial_simplex[2]].z
            + vertices[initial_simplex[3]].z)
            / 4.0,
    };

    // Build initial hull from simplex
    let mut hull_faces = create_initial_hull(&initial_simplex, vertices);

    // Track which points have been processed
    let mut processed = HashSet::new();
    for &idx in &initial_simplex {
        processed.insert(idx);
    }

    // Assign all remaining points to faces
    // Process in single pass: for each face, collect all its visible points
    let unprocessed_points: Vec<usize> = (0..vertices.len())
        .filter(|i| !processed.contains(i))
        .collect();

    for point_idx in unprocessed_points {
        let vertex = &vertices[point_idx];
        for face in &mut hull_faces {
            if face.is_visible_from(vertex, vertices) {
                face.assign_point(point_idx);
                break;
            }
        }
    }

    // Iteratively add points to the hull
    let mut iterations = 0;
    loop {
        iterations += 1;
        if iterations > MAX_ITERATIONS {
            log::error!(
                "Max iterations exceeded after {} iterations with {} faces",
                iterations,
                hull_faces.len()
            );
            return Err(ConvexHullError::MaxIterationsExceeded);
        }

        if iterations % 100 == 0 {
            let total_outside_points: usize =
                hull_faces.iter().map(|f| f.outside_points.len()).sum();
            log::debug!(
                "Iteration {}: {} faces, {} outside points remaining",
                iterations,
                hull_faces.len(),
                total_outside_points
            );
        }

        // Find face with furthest outside point
        let (face_idx, point_idx) = match find_face_with_furthest_point(&hull_faces, vertices) {
            Some(result) => result,
            None => break, // No more outside points
        };

        // Get the point to add
        let point = vertices[point_idx];

        // Find all faces visible from the point
        let visible_faces: Vec<usize> = hull_faces
            .iter()
            .enumerate()
            .filter(|(_, face)| face.is_visible_from(&point, vertices))
            .map(|(i, _)| i)
            .collect();

        if visible_faces.is_empty() {
            // This shouldn't happen, but handle gracefully
            hull_faces[face_idx]
                .outside_points
                .retain(|&p| p != point_idx);
            continue;
        }

        // Find the horizon edges
        let horizon = find_horizon(&hull_faces, &visible_faces);

        // Collect all outside points from visible faces
        let mut orphaned_points = Vec::new();
        for &face_idx in &visible_faces {
            orphaned_points.extend(hull_faces[face_idx].outside_points.iter().copied());
        }
        orphaned_points.retain(|&p| p != point_idx);

        // Remove visible faces (in reverse order to maintain indices)
        let mut visible_faces_sorted = visible_faces.clone();
        visible_faces_sorted.sort_unstable_by(|a, b| b.cmp(a));
        for &idx in &visible_faces_sorted {
            hull_faces.remove(idx);
        }

        // Create new faces from horizon edges to the new point
        let mut new_faces = Vec::new();
        for edge in horizon {
            // Create face from horizon edge + new point
            // Try both orientations and check which one has outward-pointing normal
            let face1 = HullFace::new(edge.v0, edge.v1, point_idx, vertices);
            let face2 = HullFace::new(edge.v1, edge.v0, point_idx, vertices);

            // Check orientation using a point we know is inside the hull
            // (the centroid of the initial simplex)
            // The correct face should have its normal pointing AWAY from interior points

            // Check which orientation has normal pointing away from interior
            let v0 = &vertices[face1.vertices[0]];
            let to_interior1 = simplex_centroid.sub(v0);
            let dot1 = face1.normal.dot(&to_interior1);

            // If dot product is negative, normal points away from interior (correct)
            // If dot product is positive, normal points toward interior (incorrect)
            if dot1 < 0.0 {
                new_faces.push(face1);
            } else {
                new_faces.push(face2);
            }
        }

        // Reassign orphaned points to new faces
        for point_idx in orphaned_points {
            let point = vertices[point_idx];
            let mut assigned = false;

            // First try new faces (most likely to be visible)
            for face in &mut new_faces {
                if face.is_visible_from(&point, vertices) {
                    face.assign_point(point_idx);
                    assigned = true;
                    break;
                }
            }

            // If not visible from any new face, try existing faces
            if !assigned {
                for face in &mut hull_faces {
                    if face.is_visible_from(&point, vertices) {
                        face.assign_point(point_idx);
                        break;
                    }
                }
            }
        }

        // Add new faces to hull
        hull_faces.extend(new_faces);

        // Mark point as processed
        processed.insert(point_idx);
    }

    // Convert hull faces to final format
    let faces: Vec<Face> = hull_faces.iter().map(|f| f.to_face()).collect();

    Ok(ConvexHull3D::new(vertices.to_vec(), faces))
}

/// Find the initial simplex (tetrahedron) to start the algorithm
fn find_initial_simplex(vertices: &[Vertex]) -> Result<[usize; 4]> {
    // Find the 6 extreme points
    let extremes = find_extreme_points(vertices);

    // Find the pair with maximum distance
    let mut max_distance = 0.0;
    let mut v0 = 0;
    let mut v1 = 0;

    for i in 0..6 {
        for j in (i + 1)..6 {
            let dist = vertices[extremes[i]].distance(&vertices[extremes[j]]);
            if dist > max_distance {
                max_distance = dist;
                v0 = extremes[i];
                v1 = extremes[j];
            }
        }
    }

    if max_distance < EPSILON {
        return Err(ConvexHullError::DegenerateConfiguration);
    }

    // Find the point furthest from the line v0-v1
    let line_dir = vertices[v1].sub(&vertices[v0]).normalize();
    let mut max_distance = 0.0;
    let mut v2 = 0;

    for (i, vertex) in vertices.iter().enumerate() {
        if i == v0 || i == v1 {
            continue;
        }

        let to_point = vertex.sub(&vertices[v0]);
        let projection = line_dir.scale(to_point.dot(&line_dir));
        let rejection = to_point.sub(&projection);
        let dist = rejection.magnitude();

        if dist > max_distance {
            max_distance = dist;
            v2 = i;
        }
    }

    if max_distance < EPSILON {
        return Err(ConvexHullError::DegenerateConfiguration);
    }

    // Find the point furthest from the plane formed by v0, v1, v2
    let normal = vertices[v1]
        .sub(&vertices[v0])
        .cross(&vertices[v2].sub(&vertices[v0]))
        .normalize();

    let mut max_distance = 0.0;
    let mut v3 = 0;

    for (i, vertex) in vertices.iter().enumerate() {
        if i == v0 || i == v1 || i == v2 {
            continue;
        }

        let to_point = vertex.sub(&vertices[v0]);
        let dist = normal.dot(&to_point).abs();

        if dist > max_distance {
            max_distance = dist;
            v3 = i;
        }
    }

    if max_distance < EPSILON {
        return Err(ConvexHullError::DegenerateConfiguration);
    }

    // Verify the simplex is not degenerate
    if are_coplanar(
        &vertices[v0],
        &vertices[v1],
        &vertices[v2],
        &vertices[v3],
        EPSILON,
    ) {
        return Err(ConvexHullError::DegenerateConfiguration);
    }

    Ok([v0, v1, v2, v3])
}

/// Create the initial hull from the simplex
fn create_initial_hull(simplex: &[usize; 4], vertices: &[Vertex]) -> Vec<HullFace> {
    let [v0, v1, v2, v3] = *simplex;

    // Create 4 faces of the tetrahedron
    let mut faces = vec![
        HullFace::new(v0, v1, v2, vertices),
        HullFace::new(v0, v2, v3, vertices),
        HullFace::new(v0, v3, v1, vertices),
        HullFace::new(v1, v3, v2, vertices),
    ];

    // Ensure all normals point outward from the centroid
    let centroid = Vertex {
        x: (vertices[v0].x + vertices[v1].x + vertices[v2].x + vertices[v3].x) / 4.0,
        y: (vertices[v0].y + vertices[v1].y + vertices[v2].y + vertices[v3].y) / 4.0,
        z: (vertices[v0].z + vertices[v1].z + vertices[v2].z + vertices[v3].z) / 4.0,
    };

    for face in &mut faces {
        let v0 = &vertices[face.vertices[0]];
        let to_centroid = centroid.sub(v0);

        // If normal points inward, flip the face
        if face.normal.dot(&to_centroid) > 0.0 {
            face.vertices.swap(1, 2);
            face.normal = face.normal.scale(-1.0);
        }
    }

    faces
}

/// Find the face with the furthest outside point
fn find_face_with_furthest_point(
    hull_faces: &[HullFace],
    vertices: &[Vertex],
) -> Option<(usize, usize)> {
    let mut max_distance = 0.0;
    let mut result = None;

    for (face_idx, face) in hull_faces.iter().enumerate() {
        if let Some(point_idx) = face.furthest_point(vertices) {
            let point = &vertices[point_idx];
            let v0 = &vertices[face.vertices[0]];
            let to_point = point.sub(v0);
            let distance = face.normal.dot(&to_point);

            if distance > max_distance {
                max_distance = distance;
                result = Some((face_idx, point_idx));
            }
        }
    }

    result
}

/// Find the horizon edges from a set of visible faces
fn find_horizon(hull_faces: &[HullFace], visible_faces: &[usize]) -> Vec<Edge> {
    // Collect oriented edges and track which face they came from
    let mut edge_to_faces: HashMap<Edge, Vec<usize>> = HashMap::new(); // normalized edge -> [face_idx]

    for &face_idx in visible_faces {
        let face = &hull_faces[face_idx];
        let oriented_edges = [
            (face.vertices[0], face.vertices[1]),
            (face.vertices[1], face.vertices[2]),
            (face.vertices[2], face.vertices[0]),
        ];

        for (v0, v1) in oriented_edges {
            let normalized = Edge::new(v0, v1);
            edge_to_faces.entry(normalized).or_default().push(face_idx);
        }
    }

    // Horizon edges are shared by exactly one visible face and one (or more) non-visible faces
    // Since we only look at visible faces, they appear exactly once in our collection
    let mut horizon = Vec::new();
    for (normalized_edge, face_list) in edge_to_faces {
        if face_list.len() == 1 {
            // This is a horizon edge
            let face_idx = face_list[0];
            let face = &hull_faces[face_idx];

            // Find the actual oriented edge as it appears in the visible face
            let oriented_edges = [
                (face.vertices[0], face.vertices[1]),
                (face.vertices[1], face.vertices[2]),
                (face.vertices[2], face.vertices[0]),
            ];

            for (v0, v1) in oriented_edges {
                if Edge::new(v0, v1) == normalized_edge {
                    // Found the edge - use its orientation from the visible face
                    // We keep the same orientation as in the visible face
                    horizon.push(Edge { v0, v1 });
                    break;
                }
            }
        }
    }

    horizon
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_tetrahedron() {
        let vertices = vec![
            Vertex::new(0.0, 0.0, 0.0),
            Vertex::new(1.0, 0.0, 0.0),
            Vertex::new(0.0, 1.0, 0.0),
            Vertex::new(0.0, 0.0, 1.0),
        ];

        let hull = quickhull_3d(&vertices).unwrap();
        assert_eq!(hull.num_faces(), 4);
        assert_eq!(hull.num_vertices(), 4);
    }

    #[test]
    fn test_cube() {
        let vertices = vec![
            Vertex::new(0.0, 0.0, 0.0),
            Vertex::new(1.0, 0.0, 0.0),
            Vertex::new(1.0, 1.0, 0.0),
            Vertex::new(0.0, 1.0, 0.0),
            Vertex::new(0.0, 0.0, 1.0),
            Vertex::new(1.0, 0.0, 1.0),
            Vertex::new(1.0, 1.0, 1.0),
            Vertex::new(0.0, 1.0, 1.0),
        ];

        let hull = quickhull_3d(&vertices).unwrap();
        // A cube has 8 vertices and 12 triangular faces (2 per square face)
        assert_eq!(hull.num_vertices(), 8);
        assert_eq!(hull.num_faces(), 12);
    }

    #[test]
    fn test_insufficient_vertices() {
        let vertices = vec![
            Vertex::new(0.0, 0.0, 0.0),
            Vertex::new(1.0, 0.0, 0.0),
            Vertex::new(0.0, 1.0, 0.0),
        ];

        let result = quickhull_3d(&vertices);
        assert!(matches!(result, Err(ConvexHullError::InsufficientVertices)));
    }
}
