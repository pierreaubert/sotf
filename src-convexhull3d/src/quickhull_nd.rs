//! N-dimensional Quickhull algorithm implementation

use crate::nd_types::{ConvexHullND, PointND, SimplexND};
use crate::{ConvexHullError, Result, EPSILON};
use std::collections::{HashMap, HashSet};

const MAX_DIMENSIONS: usize = 10;
const MAX_ITERATIONS: usize = 1000000;

/// Internal representation of a facet during hull construction
#[derive(Debug, Clone)]
struct HullFacet {
    vertices: Vec<usize>,
    normal: Vec<f64>,
    offset: f64,
    outside_points: Vec<usize>,
}

impl HullFacet {
    fn new(vertex_indices: Vec<usize>, points: &[PointND]) -> Result<Self> {
        let dim = points[0].dim();

        // Compute normal and offset for this facet
        let (normal, offset) = compute_facet_normal(&vertex_indices, points)?;

        Ok(Self {
            vertices: vertex_indices,
            normal,
            offset,
            outside_points: Vec::new(),
        })
    }

    fn is_visible_from(&self, point: &PointND) -> bool {
        let distance: f64 = self.normal.iter()
            .zip(point.coords.iter())
            .map(|(n, p)| n * p)
            .sum::<f64>() - self.offset;
        distance > EPSILON
    }

    fn assign_point(&mut self, point_idx: usize) {
        self.outside_points.push(point_idx);
    }

    fn furthest_point(&self, points: &[PointND]) -> Option<usize> {
        let mut max_distance = 0.0;
        let mut max_idx = None;

        for &idx in &self.outside_points {
            let point = &points[idx];
            let distance: f64 = self.normal.iter()
                .zip(point.coords.iter())
                .map(|(n, p)| n * p)
                .sum::<f64>() - self.offset;

            if distance > max_distance {
                max_distance = distance;
                max_idx = Some(idx);
            }
        }

        max_idx
    }

    fn to_simplex(&self) -> SimplexND {
        SimplexND::new(self.vertices.clone())
    }
}

/// Compute the normal vector and offset for a facet
fn compute_facet_normal(vertex_indices: &[usize], points: &[PointND]) -> Result<(Vec<f64>, f64)> {
    let dim = points[0].dim();

    if vertex_indices.len() != dim {
        return Err(ConvexHullError::InvalidFace(
            format!("Facet must have exactly {} vertices for {}-D space", dim, dim)
        ));
    }

    // Build matrix of edge vectors
    let mut matrix = Vec::new();
    let base_point = &points[vertex_indices[0]];

    for &idx in &vertex_indices[1..] {
        let edge = points[idx].sub(base_point);
        matrix.push(edge.coords);
    }

    // Compute normal via Gaussian elimination with partial pivoting
    let normal = compute_normal_from_matrix(&matrix)?;

    // Compute offset
    let offset: f64 = normal.iter()
        .zip(base_point.coords.iter())
        .map(|(n, p)| n * p)
        .sum();

    Ok((normal, offset))
}

/// Compute normal vector from matrix of edge vectors using Gaussian elimination
fn compute_normal_from_matrix(matrix: &[Vec<f64>]) -> Result<Vec<f64>> {
    let dim = matrix[0].len();
    let n = matrix.len();

    if n != dim - 1 {
        return Err(ConvexHullError::DegenerateConfiguration);
    }

    // Create augmented matrix for solving the null space
    let mut aug = vec![vec![0.0; dim + 1]; dim];

    // Fill the matrix (transpose of edge vectors)
    for i in 0..n {
        for j in 0..dim {
            aug[j][i] = matrix[i][j];
        }
    }

    // Add identity for the remaining dimension
    for i in 0..dim {
        aug[i][dim] = if i == dim - 1 { 1.0 } else { 0.0 };
    }

    // Gaussian elimination
    for i in 0..n.min(dim) {
        // Find pivot
        let mut max_row = i;
        for k in (i + 1)..dim {
            if aug[k][i].abs() > aug[max_row][i].abs() {
                max_row = k;
            }
        }

        // Swap rows
        aug.swap(i, max_row);

        // Make diagonal 1
        let pivot = aug[i][i];
        if pivot.abs() < EPSILON {
            continue;
        }

        for j in 0..=dim {
            aug[i][j] /= pivot;
        }

        // Eliminate column
        for k in 0..dim {
            if k != i {
                let factor = aug[k][i];
                for j in 0..=dim {
                    aug[k][j] -= factor * aug[i][j];
                }
            }
        }
    }

    // Extract normal from last column
    let mut normal: Vec<f64> = aug.iter().map(|row| row[dim]).collect();

    // Normalize
    let mag = normal.iter().map(|x| x * x).sum::<f64>().sqrt();
    if mag < EPSILON {
        return Err(ConvexHullError::DegenerateConfiguration);
    }

    for x in &mut normal {
        *x /= mag;
    }

    Ok(normal)
}

/// Build an N-dimensional convex hull using the Quickhull algorithm
pub fn quickhull_nd(points: &[PointND]) -> Result<ConvexHullND> {
    if points.is_empty() {
        return Err(ConvexHullError::InsufficientVertices);
    }

    let dim = points[0].dim();

    if dim > MAX_DIMENSIONS {
        return Err(ConvexHullError::InvalidFace(
            format!("Dimension {} exceeds maximum {}", dim, MAX_DIMENSIONS)
        ));
    }

    if points.len() < dim + 1 {
        return Err(ConvexHullError::InsufficientVertices);
    }

    // For simplicity, delegate to specialized 2D/3D implementations if available
    // Otherwise, use general N-D algorithm
    match dim {
        1 => quickhull_1d(points),
        2 => quickhull_2d(points),
        _ => quickhull_general(points),
    }
}

/// 1D convex hull (just the min and max points)
fn quickhull_1d(points: &[PointND]) -> Result<ConvexHullND> {
    let mut min_idx = 0;
    let mut max_idx = 0;

    for (i, point) in points.iter().enumerate() {
        if point.coords[0] < points[min_idx].coords[0] {
            min_idx = i;
        }
        if point.coords[0] > points[max_idx].coords[0] {
            max_idx = i;
        }
    }

    let facets = vec![
        SimplexND::new(vec![min_idx]),
        SimplexND::new(vec![max_idx]),
    ];

    Ok(ConvexHullND::new(points.to_vec(), facets, 1))
}

/// 2D convex hull using QuickHull
fn quickhull_2d(points: &[PointND]) -> Result<ConvexHullND> {
    use std::collections::HashSet;

    // Find leftmost and rightmost points
    let mut left_idx = 0;
    let mut right_idx = 0;

    for (i, point) in points.iter().enumerate() {
        if point.coords[0] < points[left_idx].coords[0] {
            left_idx = i;
        }
        if point.coords[0] > points[right_idx].coords[0] {
            right_idx = i;
        }
    }

    let mut hull_indices = Vec::new();
    let mut processed = HashSet::new();

    // Find upper hull
    find_hull_2d(points, left_idx, right_idx, &mut hull_indices, &mut processed);

    // Find lower hull
    find_hull_2d(points, right_idx, left_idx, &mut hull_indices, &mut processed);

    // Remove consecutive duplicates from hull_indices
    hull_indices.dedup();

    // Create edges (facets in 2D)
    let mut facets = Vec::new();
    for i in 0..hull_indices.len() {
        let next = (i + 1) % hull_indices.len();
        facets.push(SimplexND::new(vec![hull_indices[i], hull_indices[next]]));
    }

    Ok(ConvexHullND::new(points.to_vec(), facets, 2))
}

fn find_hull_2d(
    points: &[PointND],
    start: usize,
    end: usize,
    hull: &mut Vec<usize>,
    processed: &mut std::collections::HashSet<usize>,
) {
    // Only push start if it's not already the last element in hull
    if hull.is_empty() || hull.last() != Some(&start) {
        hull.push(start);
        processed.insert(start);
    }

    // Find furthest point from line
    let mut max_dist = 0.0;
    let mut max_idx = None;

    for (i, point) in points.iter().enumerate() {
        if i == start || i == end {
            continue;
        }

        // Skip points already processed (O(1) lookup with HashSet)
        if processed.contains(&i) {
            continue;
        }

        let sign = cross_product_2d(&points[start], &points[end], point);

        // For upper hull going left→right: positive cross product means above the line
        // For lower hull going right→left: positive cross product means below the line
        // So we use the same sign check for both!
        let is_on_correct_side = sign > EPSILON;

        if is_on_correct_side {
            let dist = point_line_distance_2d(&points[start], &points[end], point);
            if dist > max_dist {
                max_dist = dist;
                max_idx = Some(i);
            }
        }
    }

    if let Some(idx) = max_idx {
        find_hull_2d(points, start, idx, hull, processed);
        // The recursive call added points from start to idx
        // Now we need to add points from idx to end
        // idx should already be at the end of hull from the previous call
        find_hull_2d(points, idx, end, hull, processed);
    }
}

fn point_line_distance_2d(p1: &PointND, p2: &PointND, point: &PointND) -> f64 {
    let dx = p2.coords[0] - p1.coords[0];
    let dy = p2.coords[1] - p1.coords[1];
    let num = ((dy * (point.coords[0] - p1.coords[0])) - (dx * (point.coords[1] - p1.coords[1]))).abs();
    let den = (dx * dx + dy * dy).sqrt();
    if den < EPSILON { 0.0 } else { num / den }
}

fn cross_product_2d(p1: &PointND, p2: &PointND, point: &PointND) -> f64 {
    (p2.coords[0] - p1.coords[0]) * (point.coords[1] - p1.coords[1]) -
    (p2.coords[1] - p1.coords[1]) * (point.coords[0] - p1.coords[0])
}

/// General N-dimensional QuickHull
fn quickhull_general(points: &[PointND]) -> Result<ConvexHullND> {
    let dim = points[0].dim();

    // Find initial simplex
    let initial_simplex = find_initial_simplex_nd(points)?;

    // Build initial hull from simplex
    let mut hull_facets = create_initial_hull_nd(&initial_simplex, points)?;

    // Track which points have been processed
    let mut processed = HashSet::new();
    for &idx in &initial_simplex {
        processed.insert(idx);
    }

    // Assign all remaining points to facets
    for (i, point) in points.iter().enumerate() {
        if processed.contains(&i) {
            continue;
        }

        for facet in &mut hull_facets {
            if facet.is_visible_from(point) {
                facet.assign_point(i);
                break;
            }
        }
    }

    // Iteratively add points to the hull
    let mut iterations = 0;
    loop {
        iterations += 1;
        if iterations > MAX_ITERATIONS {
            return Err(ConvexHullError::MaxIterationsExceeded);
        }

        // Find facet with furthest outside point
        let (facet_idx, point_idx) = match find_facet_with_furthest_point_nd(&hull_facets, points) {
            Some(result) => result,
            None => break,
        };

        let point = &points[point_idx];

        // Find all facets visible from the point
        let visible_facets: Vec<usize> = hull_facets
            .iter()
            .enumerate()
            .filter(|(_, facet)| facet.is_visible_from(point))
            .map(|(i, _)| i)
            .collect();

        if visible_facets.is_empty() {
            hull_facets[facet_idx].outside_points.retain(|&p| p != point_idx);
            continue;
        }

        // Collect orphaned points
        let mut orphaned_points = Vec::new();
        for &face_idx in &visible_facets {
            orphaned_points.extend(hull_facets[face_idx].outside_points.iter().copied());
        }
        orphaned_points.retain(|&p| p != point_idx);

        // Find horizon ridges (d-2 dimensional faces)
        let horizon = find_horizon_nd(&hull_facets, &visible_facets, dim);

        // Remove visible facets
        let mut visible_facets_sorted = visible_facets.clone();
        visible_facets_sorted.sort_unstable_by(|a, b| b.cmp(a));
        for &idx in &visible_facets_sorted {
            hull_facets.remove(idx);
        }

        // Create new facets from horizon to new point
        for ridge in horizon {
            let mut new_facet_vertices = ridge;
            new_facet_vertices.push(point_idx);

            if let Ok(new_facet) = HullFacet::new(new_facet_vertices, points) {
                hull_facets.push(new_facet);
            }
        }

        // Reassign orphaned points
        for pidx in orphaned_points {
            let p = &points[pidx];
            for facet in &mut hull_facets {
                if facet.is_visible_from(p) {
                    facet.assign_point(pidx);
                    break;
                }
            }
        }

        processed.insert(point_idx);
    }

    let facets: Vec<SimplexND> = hull_facets.iter().map(|f| f.to_simplex()).collect();

    Ok(ConvexHullND::new(points.to_vec(), facets, dim))
}

fn find_initial_simplex_nd(points: &[PointND]) -> Result<Vec<usize>> {
    let dim = points[0].dim();
    let mut simplex = Vec::with_capacity(dim + 1);

    // Find extreme points in each dimension
    for d in 0..dim {
        let mut min_idx = 0;
        let mut max_idx = 0;

        for (i, point) in points.iter().enumerate() {
            if point.coords[d] < points[min_idx].coords[d] {
                min_idx = i;
            }
            if point.coords[d] > points[max_idx].coords[d] {
                max_idx = i;
            }
        }

        if !simplex.contains(&min_idx) {
            simplex.push(min_idx);
        }
        if !simplex.contains(&max_idx) && simplex.len() < dim + 1 {
            simplex.push(max_idx);
        }
    }

    // Fill remaining vertices if needed
    while simplex.len() < dim + 1 {
        for (i, _) in points.iter().enumerate() {
            if !simplex.contains(&i) {
                simplex.push(i);
                break;
            }
        }
    }

    simplex.truncate(dim + 1);

    Ok(simplex)
}

fn create_initial_hull_nd(simplex: &[usize], points: &[PointND]) -> Result<Vec<HullFacet>> {
    let dim = points[0].dim();
    let mut facets = Vec::new();

    // Generate all (d-1)-subsets of the (d+1)-simplex vertices
    let combinations = generate_combinations(simplex, dim);

    for combo in combinations {
        if let Ok(facet) = HullFacet::new(combo, points) {
            facets.push(facet);
        }
    }

    Ok(facets)
}

fn generate_combinations(items: &[usize], r: usize) -> Vec<Vec<usize>> {
    let n = items.len();
    if r > n {
        return vec![];
    }
    if r == 0 {
        return vec![vec![]];
    }

    let mut result = Vec::new();
    let mut indices: Vec<usize> = (0..r).collect();

    loop {
        result.push(indices.iter().map(|&i| items[i]).collect());

        let mut i = r;
        while i > 0 && indices[i - 1] == n - r + i - 1 {
            i -= 1;
        }

        if i == 0 {
            break;
        }

        indices[i - 1] += 1;
        for j in i..r {
            indices[j] = indices[j - 1] + 1;
        }
    }

    result
}

fn find_facet_with_furthest_point_nd(facets: &[HullFacet], points: &[PointND]) -> Option<(usize, usize)> {
    let mut max_distance = 0.0;
    let mut result = None;

    for (facet_idx, facet) in facets.iter().enumerate() {
        if let Some(point_idx) = facet.furthest_point(points) {
            let point = &points[point_idx];
            let distance: f64 = facet.normal.iter()
                .zip(point.coords.iter())
                .map(|(n, p)| n * p)
                .sum::<f64>() - facet.offset;

            if distance > max_distance {
                max_distance = distance;
                result = Some((facet_idx, point_idx));
            }
        }
    }

    result
}

fn find_horizon_nd(facets: &[HullFacet], visible_facets: &[usize], dim: usize) -> Vec<Vec<usize>> {
    let mut ridge_counts: HashMap<Vec<usize>, usize> = HashMap::new();

    // Generate all ridges (d-2 faces) from visible facets
    for &facet_idx in visible_facets {
        let facet = &facets[facet_idx];
        let ridges = generate_combinations(&facet.vertices, dim - 1);

        for ridge in ridges {
            let mut sorted_ridge = ridge.clone();
            sorted_ridge.sort_unstable();
            *ridge_counts.entry(sorted_ridge).or_insert(0) += 1;
        }
    }

    // Horizon ridges appear exactly once
    ridge_counts
        .into_iter()
        .filter(|(_, count)| *count == 1)
        .map(|(ridge, _)| ridge)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_1d_hull() {
        let points = vec![
            PointND::new(vec![0.0]),
            PointND::new(vec![1.0]),
            PointND::new(vec![0.5]),
            PointND::new(vec![-1.0]),
        ];

        let hull = quickhull_nd(&points).unwrap();
        assert_eq!(hull.dim(), 1);
        assert_eq!(hull.num_facets(), 2);
    }

    #[test]
    fn test_2d_square() {
        let points = vec![
            PointND::new(vec![0.0, 0.0]),
            PointND::new(vec![1.0, 0.0]),
            PointND::new(vec![1.0, 1.0]),
            PointND::new(vec![0.0, 1.0]),
            PointND::new(vec![0.5, 0.5]), // Interior point
        ];

        let hull = quickhull_nd(&points).unwrap();
        assert_eq!(hull.dim(), 2);
        assert_eq!(hull.num_facets(), 4); // 4 edges
    }

    #[test]
    fn test_2d_triangle() {
        // Simple triangle with no interior points
        let points = vec![
            PointND::new(vec![0.0, 0.0]),
            PointND::new(vec![1.0, 0.0]),
            PointND::new(vec![0.5, 1.0]),
        ];

        let hull = quickhull_nd(&points).unwrap();
        assert_eq!(hull.dim(), 2);
        assert_eq!(hull.num_facets(), 3); // 3 edges
    }

    #[test]
    fn test_2d_with_duplicate_points() {
        // Points with duplicates - the duplicate should be ignored
        let points = vec![
            PointND::new(vec![0.0, 0.0]),
            PointND::new(vec![1.0, 0.0]),
            PointND::new(vec![1.0, 1.0]),
            PointND::new(vec![0.0, 1.0]),
            PointND::new(vec![0.0, 0.0]), // Duplicate of first point
        ];

        let hull = quickhull_nd(&points).unwrap();
        assert_eq!(hull.dim(), 2);
        // Should still form a square (4 edges)
        assert_eq!(hull.num_facets(), 4);
    }

    #[test]
    fn test_2d_collinear_points() {
        // All points on a line - produces a degenerate hull (line segment)
        let points = vec![
            PointND::new(vec![0.0, 0.0]),
            PointND::new(vec![1.0, 0.0]),
            PointND::new(vec![2.0, 0.0]),
            PointND::new(vec![3.0, 0.0]),
        ];

        let hull = quickhull_nd(&points).unwrap();
        assert_eq!(hull.dim(), 2);
        // Collinear points in 2D produce just the 2 endpoints
        // The hull should have 2 facets (edges from each endpoint)
        assert_eq!(hull.num_facets(), 2);
    }

    #[test]
    fn test_2d_larger_point_set() {
        // Test with more points to verify performance
        let mut points = vec![
            PointND::new(vec![0.0, 0.0]),
            PointND::new(vec![5.0, 0.0]),
            PointND::new(vec![5.0, 5.0]),
            PointND::new(vec![0.0, 5.0]),
        ];

        // Add interior points
        for i in 1..4 {
            for j in 1..4 {
                points.push(PointND::new(vec![i as f64, j as f64]));
            }
        }

        let hull = quickhull_nd(&points).unwrap();
        assert_eq!(hull.dim(), 2);
        assert_eq!(hull.num_facets(), 4); // Should still be 4 edges (the square boundary)
    }
}
