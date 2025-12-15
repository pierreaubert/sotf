//! Quickhull algorithm implementation for 3D convex hulls
//!
//! Based on:
//! - Barber, C.B., Dobkin, D.P., and Huhdanpaa, H.T., "The Quickhull algorithm
//!   for convex hulls," ACM Trans. on Mathematical Software, 22(4):469-483, 1996.
//!
//! Performance optimizations:
//! - Parallel point visibility checks with rayon
//! - Generation-based face deletion (O(1) instead of O(n))
//! - Pre-allocated scratch buffers
//! - Reused HashMap for horizon computation

use crate::geometry::{are_coplanar, find_extreme_points};
use crate::types::{ConvexHull3D, Face, Vertex};
use crate::{ConvexHullError, EPSILON, Result};
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

const MAX_ITERATIONS: usize = 100000;

/// Threshold for parallel processing (below this, sequential is faster)
const PARALLEL_THRESHOLD: usize = 100;

/// Internal representation of a face during hull construction
#[derive(Debug, Clone)]
struct HullFace {
    vertices: [usize; 3],
    normal: Vertex,
    d: f64, // Plane constant: normal.dot(v0), for faster distance computation
    outside_points: Vec<usize>,
    deleted: bool, // Mark as deleted instead of removing
}

impl HullFace {
    fn new(v0: usize, v1: usize, v2: usize, vertices: &[Vertex]) -> Self {
        let vertices_arr = [v0, v1, v2];
        let p0 = &vertices[v0];
        let p1 = &vertices[v1];
        let p2 = &vertices[v2];

        // Compute normal
        let e1 = p1.sub(p0);
        let e2 = p2.sub(p0);
        let normal = e1.cross(&e2).normalize();

        // Pre-compute plane constant for faster distance calculation
        let d = normal.dot(p0);

        Self {
            vertices: vertices_arr,
            normal,
            d,
            outside_points: Vec::new(),
            deleted: false,
        }
    }

    /// Fast signed distance from point to plane (positive = outside)
    #[inline]
    fn signed_distance(&self, point: &Vertex) -> f64 {
        self.normal.dot(point) - self.d
    }

    #[inline]
    fn is_visible_from(&self, point: &Vertex) -> bool {
        self.signed_distance(point) > EPSILON
    }

    fn assign_point(&mut self, point_idx: usize) {
        self.outside_points.push(point_idx);
    }

    fn furthest_point(&self, vertices: &[Vertex]) -> Option<(usize, f64)> {
        let mut max_distance = 0.0;
        let mut max_idx = None;

        for &idx in &self.outside_points {
            let distance = self.signed_distance(&vertices[idx]);
            if distance > max_distance {
                max_distance = distance;
                max_idx = Some(idx);
            }
        }

        max_idx.map(|idx| (idx, max_distance))
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
    #[inline]
    fn new(v0: usize, v1: usize) -> Self {
        // Normalize edge orientation for consistent hashing
        if v0 < v1 {
            Self { v0, v1 }
        } else {
            Self { v0: v1, v1: v0 }
        }
    }

    /// Create with explicit orientation (don't normalize)
    #[inline]
    fn oriented(v0: usize, v1: usize) -> Self {
        Self { v0, v1 }
    }
}

/// Scratch buffers to avoid allocations in hot loop
struct ScratchBuffers {
    visible_face_indices: Vec<usize>,
    orphaned_points: Vec<usize>,
    new_faces: Vec<HullFace>,
    edge_to_face: HashMap<Edge, usize>,
    horizon_edges: Vec<Edge>,
}

impl ScratchBuffers {
    fn new() -> Self {
        Self {
            visible_face_indices: Vec::with_capacity(64),
            orphaned_points: Vec::with_capacity(256),
            new_faces: Vec::with_capacity(64),
            edge_to_face: HashMap::with_capacity(128),
            horizon_edges: Vec::with_capacity(64),
        }
    }

    fn clear(&mut self) {
        self.visible_face_indices.clear();
        self.orphaned_points.clear();
        self.new_faces.clear();
        self.edge_to_face.clear();
        self.horizon_edges.clear();
    }
}

/// Build a convex hull using the Quickhull algorithm
pub fn quickhull_3d(vertices: &[Vertex]) -> Result<ConvexHull3D> {
    if vertices.len() < 4 {
        return Err(ConvexHullError::InsufficientVertices);
    }

    // Find initial simplex (tetrahedron)
    let initial_simplex = find_initial_simplex(vertices)?;

    // Compute the centroid of the initial simplex - guaranteed to be inside the hull
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

    // Track which points are in the initial simplex
    let mut in_simplex = vec![false; vertices.len()];
    for &idx in &initial_simplex {
        in_simplex[idx] = true;
    }

    // Collect unprocessed points
    let unprocessed_points: Vec<usize> = (0..vertices.len()).filter(|&i| !in_simplex[i]).collect();

    // Initial point assignment - use parallel if enough points
    if unprocessed_points.len() >= PARALLEL_THRESHOLD {
        assign_points_parallel(&mut hull_faces, vertices, &unprocessed_points);
    } else {
        assign_points_sequential(&mut hull_faces, vertices, &unprocessed_points);
    }

    // Scratch buffers for the main loop
    let mut scratch = ScratchBuffers::new();

    // Active face count (faces not marked as deleted)
    let mut active_face_count = hull_faces.len();

    // Main iteration loop
    let mut iterations = 0;
    loop {
        iterations += 1;
        if iterations > MAX_ITERATIONS {
            log::error!(
                "Max iterations exceeded after {} iterations with {} faces",
                iterations,
                active_face_count
            );
            return Err(ConvexHullError::MaxIterationsExceeded);
        }

        if iterations % 500 == 0 {
            // Compact faces periodically to avoid too many deleted entries
            compact_faces(&mut hull_faces);
            active_face_count = hull_faces.len();

            let total_outside_points: usize = hull_faces
                .iter()
                .filter(|f| !f.deleted)
                .map(|f| f.outside_points.len())
                .sum();
            log::debug!(
                "Iteration {}: {} faces, {} outside points remaining",
                iterations,
                active_face_count,
                total_outside_points
            );
        }

        // Find face with furthest outside point
        let (face_idx, point_idx, _) = match find_face_with_furthest_point(&hull_faces, vertices) {
            Some(result) => result,
            None => break, // No more outside points
        };

        let point = vertices[point_idx];

        // Clear scratch buffers
        scratch.clear();

        // Find all visible faces (can be parallelized for large face counts)
        if hull_faces.len() >= PARALLEL_THRESHOLD {
            find_visible_faces_parallel(&hull_faces, &point, &mut scratch.visible_face_indices);
        } else {
            for (i, face) in hull_faces.iter().enumerate() {
                if !face.deleted && face.is_visible_from(&point) {
                    scratch.visible_face_indices.push(i);
                }
            }
        }

        if scratch.visible_face_indices.is_empty() {
            // Shouldn't happen, but handle gracefully
            hull_faces[face_idx]
                .outside_points
                .retain(|&p| p != point_idx);
            continue;
        }

        // Find horizon edges and collect orphaned points
        find_horizon_optimized(
            &hull_faces,
            &scratch.visible_face_indices,
            &mut scratch.edge_to_face,
            &mut scratch.horizon_edges,
        );

        // Collect orphaned points from visible faces
        for &face_idx in &scratch.visible_face_indices {
            scratch
                .orphaned_points
                .extend(hull_faces[face_idx].outside_points.iter().copied());
        }
        scratch.orphaned_points.retain(|&p| p != point_idx);

        // Mark visible faces as deleted (O(1) per face instead of O(n) removal)
        for &face_idx in &scratch.visible_face_indices {
            hull_faces[face_idx].deleted = true;
            hull_faces[face_idx].outside_points.clear(); // Free memory
            active_face_count -= 1;
        }

        // Create new faces from horizon edges to the new point
        for edge in &scratch.horizon_edges {
            // Create face with correct orientation (outward normal)
            let face1 = HullFace::new(edge.v0, edge.v1, point_idx, vertices);

            // Check orientation: normal should point away from interior
            let to_interior = simplex_centroid.sub(&vertices[face1.vertices[0]]);
            let dot = face1.normal.dot(&to_interior);

            if dot < 0.0 {
                scratch.new_faces.push(face1);
            } else {
                // Flip orientation
                scratch
                    .new_faces
                    .push(HullFace::new(edge.v1, edge.v0, point_idx, vertices));
            }
        }

        // Reassign orphaned points to new faces first, then existing faces
        for &orphan_idx in &scratch.orphaned_points {
            let orphan = &vertices[orphan_idx];
            let mut assigned = false;

            // Try new faces first (most likely)
            for face in &mut scratch.new_faces {
                if face.is_visible_from(orphan) {
                    face.assign_point(orphan_idx);
                    assigned = true;
                    break;
                }
            }

            // If not assigned, try existing non-deleted faces
            if !assigned {
                for face in hull_faces.iter_mut().filter(|f| !f.deleted) {
                    if face.is_visible_from(orphan) {
                        face.assign_point(orphan_idx);
                        break;
                    }
                }
            }
        }

        // Add new faces to hull
        active_face_count += scratch.new_faces.len();
        hull_faces.append(&mut scratch.new_faces);
    }

    // Final compaction - remove all deleted faces
    compact_faces(&mut hull_faces);

    // Convert to final format
    let faces: Vec<Face> = hull_faces.iter().map(|f| f.to_face()).collect();

    Ok(ConvexHull3D::new(vertices.to_vec(), faces))
}

/// Assign points to faces in parallel
fn assign_points_parallel(hull_faces: &mut [HullFace], vertices: &[Vertex], points: &[usize]) {
    // For each point, find which face (if any) it's visible from
    // Use atomic counter to track assigned points per face
    let face_assignments: Vec<AtomicUsize> =
        (0..hull_faces.len()).map(|_| AtomicUsize::new(0)).collect();

    // Parallel: find face index for each point
    let assignments: Vec<Option<usize>> = points
        .par_iter()
        .map(|&point_idx| {
            let vertex = &vertices[point_idx];
            for (face_idx, face) in hull_faces.iter().enumerate() {
                if face.is_visible_from(vertex) {
                    face_assignments[face_idx].fetch_add(1, Ordering::Relaxed);
                    return Some(face_idx);
                }
            }
            None
        })
        .collect();

    // Pre-allocate outside_points vectors based on counts
    for (face_idx, count) in face_assignments.iter().enumerate() {
        let c = count.load(Ordering::Relaxed);
        if c > 0 {
            hull_faces[face_idx].outside_points.reserve(c);
        }
    }

    // Sequential: actually assign points (to maintain deterministic order)
    for (i, &point_idx) in points.iter().enumerate() {
        if let Some(face_idx) = assignments[i] {
            hull_faces[face_idx].outside_points.push(point_idx);
        }
    }
}

/// Assign points to faces sequentially
fn assign_points_sequential(hull_faces: &mut [HullFace], vertices: &[Vertex], points: &[usize]) {
    for &point_idx in points {
        let vertex = &vertices[point_idx];
        for face in hull_faces.iter_mut() {
            if face.is_visible_from(vertex) {
                face.assign_point(point_idx);
                break;
            }
        }
    }
}

/// Find visible faces in parallel
fn find_visible_faces_parallel(hull_faces: &[HullFace], point: &Vertex, result: &mut Vec<usize>) {
    let visible: Vec<usize> = hull_faces
        .par_iter()
        .enumerate()
        .filter_map(|(i, face)| {
            if !face.deleted && face.is_visible_from(point) {
                Some(i)
            } else {
                None
            }
        })
        .collect();

    result.extend(visible);
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
        let to_centroid = centroid.sub(&vertices[face.vertices[0]]);

        // If normal points inward, flip the face
        if face.normal.dot(&to_centroid) > 0.0 {
            face.vertices.swap(1, 2);
            face.normal = face.normal.scale(-1.0);
            face.d = -face.d;
        }
    }

    faces
}

/// Find the face with the furthest outside point
fn find_face_with_furthest_point(
    hull_faces: &[HullFace],
    vertices: &[Vertex],
) -> Option<(usize, usize, f64)> {
    let mut max_distance = 0.0;
    let mut result = None;

    for (face_idx, face) in hull_faces.iter().enumerate() {
        if face.deleted {
            continue;
        }

        if let Some((point_idx, distance)) = face.furthest_point(vertices)
            && distance > max_distance
        {
            max_distance = distance;
            result = Some((face_idx, point_idx, distance));
        }
    }

    result
}

/// Find horizon edges with optimized HashMap usage
fn find_horizon_optimized(
    hull_faces: &[HullFace],
    visible_faces: &[usize],
    edge_to_face: &mut HashMap<Edge, usize>,
    horizon: &mut Vec<Edge>,
) {
    edge_to_face.clear();
    horizon.clear();

    // Collect edges from visible faces
    for &face_idx in visible_faces {
        let face = &hull_faces[face_idx];
        let edges = [
            (face.vertices[0], face.vertices[1]),
            (face.vertices[1], face.vertices[2]),
            (face.vertices[2], face.vertices[0]),
        ];

        for (v0, v1) in edges {
            let normalized = Edge::new(v0, v1);
            match edge_to_face.entry(normalized) {
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(face_idx);
                }
                std::collections::hash_map::Entry::Occupied(e) => {
                    // Edge is shared by two visible faces - not a horizon edge
                    e.remove();
                }
            }
        }
    }

    // Remaining edges are horizon edges
    for (&normalized_edge, &face_idx) in edge_to_face.iter() {
        let face = &hull_faces[face_idx];

        // Find the actual oriented edge
        let edges = [
            (face.vertices[0], face.vertices[1]),
            (face.vertices[1], face.vertices[2]),
            (face.vertices[2], face.vertices[0]),
        ];

        for (v0, v1) in edges {
            if Edge::new(v0, v1) == normalized_edge {
                horizon.push(Edge::oriented(v0, v1));
                break;
            }
        }
    }
}

/// Remove deleted faces from the vector
fn compact_faces(hull_faces: &mut Vec<HullFace>) {
    hull_faces.retain(|f| !f.deleted);
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
