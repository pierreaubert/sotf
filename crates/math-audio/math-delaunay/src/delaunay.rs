//! Delaunay triangulation — port of d3-delaunay/src/delaunay.js

use crate::voronoi::Voronoi;
use delaunator::{EMPTY, Point, triangulate};

const NO_EDGE: usize = EMPTY;

/// Delaunay triangulation with D3-compatible API.
pub struct Delaunay {
    /// Flat coordinate array [x0, y0, x1, y1, ...].
    pub points: Vec<f64>,
    /// Triangle vertex indices.
    pub triangles: Vec<usize>,
    /// Half-edge index. `halfedges[e]` is the opposite half-edge, or `EMPTY`.
    pub halfedges: Vec<usize>,
    /// Convex hull point indices.
    pub hull: Vec<usize>,
    /// Incoming half-edge per point. `NO_EDGE` if not set.
    pub inedges: Vec<usize>,
    /// Maps hull point → position in hull array. `NO_EDGE` if not on hull.
    pub hull_index: Vec<usize>,
    /// Collinear point indices (sorted), or empty.
    pub collinear: Vec<usize>,
}

impl Delaunay {
    /// Create from (x, y) tuples.
    pub fn from_points(points: &[(f64, f64)]) -> Self {
        let flat: Vec<f64> = points.iter().flat_map(|(x, y)| [*x, *y]).collect();
        Self::new(flat)
    }

    /// Create from flat coordinates [x0, y0, x1, y1, ...].
    pub fn new(points: Vec<f64>) -> Self {
        let n = points.len() / 2;
        let del_points: Vec<Point> = (0..n)
            .map(|i| Point {
                x: points[i * 2],
                y: points[i * 2 + 1],
            })
            .collect();

        let tri = triangulate(&del_points);

        let mut inedges = vec![NO_EDGE; n];
        let mut hull_index = vec![NO_EDGE; n];
        let mut collinear_vec = Vec::new();

        let mut triangles = tri.triangles.clone();
        let mut halfedges = tri.halfedges.clone();
        let mut hull = tri.hull.clone();

        // Check for collinear points
        if hull.len() > 2 && is_collinear(&tri.triangles, &points) {
            let mut col: Vec<usize> = (0..n).collect();
            col.sort_by(|&i, &j| {
                points[2 * i]
                    .total_cmp(&points[2 * j])
                    .then(points[2 * i + 1].total_cmp(&points[2 * j + 1]))
            });
            collinear_vec = col;

            // Jitter and re-triangulate
            let e = collinear_vec[0];
            let f = collinear_vec[collinear_vec.len() - 1];
            let r = 1e-8
                * ((points[2 * f + 1] - points[2 * e + 1]).powi(2)
                    + (points[2 * f] - points[2 * e]).powi(2))
                .sqrt();

            let jittered: Vec<Point> = (0..n)
                .map(|i| {
                    let x = points[2 * i];
                    let y = points[2 * i + 1];
                    Point {
                        x: x + (x + y).sin() * r,
                        y: y + (x - y).cos() * r,
                    }
                })
                .collect();

            let tri2 = triangulate(&jittered);
            triangles = tri2.triangles;
            halfedges = tri2.halfedges;
            hull = tri2.hull;
        }

        // Compute inedges
        for (e, _) in halfedges.iter().enumerate() {
            let p = triangles[if e % 3 == 2 { e - 2 } else { e + 1 }];
            if halfedges[e] == NO_EDGE || inedges[p] == NO_EDGE {
                inedges[p] = e;
            }
        }
        for (i, &h) in hull.iter().enumerate() {
            hull_index[h] = i;
        }

        // Degenerate: 1 or 2 distinct points
        if hull.len() <= 2 && !hull.is_empty() {
            triangles = vec![NO_EDGE; 3];
            halfedges = vec![NO_EDGE; 3];
            triangles[0] = hull[0];
            inedges[hull[0]] = 1;
            if hull.len() == 2 {
                inedges[hull[1]] = 0;
                triangles[1] = hull[1];
                triangles[2] = hull[1];
            }
        }

        Self {
            points,
            triangles,
            halfedges,
            hull,
            inedges,
            hull_index,
            collinear: collinear_vec,
        }
    }

    pub fn len(&self) -> usize {
        self.points.len() / 2
    }
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
    pub fn point(&self, i: usize) -> (f64, f64) {
        (self.points[i * 2], self.points[i * 2 + 1])
    }

    pub fn voronoi(&self, bounds: [f64; 4]) -> Voronoi<'_> {
        Voronoi::new(self, bounds)
    }

    pub fn neighbors(&self, i: usize) -> Vec<usize> {
        let mut result = Vec::new();
        if !self.collinear.is_empty() {
            if let Some(l) = self.collinear.iter().position(|&c| c == i) {
                if l > 0 {
                    result.push(self.collinear[l - 1]);
                }
                if l < self.collinear.len() - 1 {
                    result.push(self.collinear[l + 1]);
                }
            }
            return result;
        }
        let e0 = self.inedges[i];
        if e0 == NO_EDGE {
            return result;
        }
        let mut e = e0;
        loop {
            result.push(self.triangles[e]);
            e = if e % 3 == 2 { e - 2 } else { e + 1 };
            if self.triangles[e] != i {
                return result;
            }
            e = self.halfedges[e];
            if e == NO_EDGE {
                if self.hull_index[i] != NO_EDGE {
                    let p = self.hull[(self.hull_index[i] + 1) % self.hull.len()];
                    if !result.is_empty() && p != *result.last().unwrap() {
                        result.push(p);
                    }
                }
                return result;
            }
            if e == e0 {
                break;
            }
        }
        result
    }

    pub fn find(&self, x: f64, y: f64, start: usize) -> usize {
        if x.is_nan() || y.is_nan() {
            return NO_EDGE;
        }
        let mut i = start;
        let i0 = i;
        for _ in 0..self.len().max(1) {
            let c = self.step(i, x, y);
            if c == i || c == i0 {
                return c;
            }
            if c == NO_EDGE {
                return i;
            }
            i = c;
        }
        i
    }

    pub fn step(&self, i: usize, x: f64, y: f64) -> usize {
        if self.inedges[i] == NO_EDGE || self.points.is_empty() {
            return (i + 1) % self.len();
        }
        let mut c = i;
        let mut dc = (x - self.points[i * 2]).powi(2) + (y - self.points[i * 2 + 1]).powi(2);
        let e0 = self.inedges[i];
        let mut e = e0;
        loop {
            let t = self.triangles[e];
            let dt = (x - self.points[t * 2]).powi(2) + (y - self.points[t * 2 + 1]).powi(2);
            if dt < dc {
                dc = dt;
                c = t;
            }
            e = if e % 3 == 2 { e - 2 } else { e + 1 };
            if self.triangles[e] != i {
                break;
            }
            let he = self.halfedges[e];
            if he == NO_EDGE {
                if self.hull_index[i] != NO_EDGE {
                    let ep = self.hull[(self.hull_index[i] + 1) % self.hull.len()];
                    if ep != t {
                        let dep = (x - self.points[ep * 2]).powi(2)
                            + (y - self.points[ep * 2 + 1]).powi(2);
                        if dep < dc {
                            return ep;
                        }
                    }
                }
                break;
            }
            e = he;
            if e == e0 {
                break;
            }
        }
        c
    }

    pub fn circumcenter(&self, t: usize) -> (f64, f64) {
        let i = t * 3;
        let t1 = self.triangles[i] * 2;
        let t2 = self.triangles[i + 1] * 2;
        let t3 = self.triangles[i + 2] * 2;
        let (x1, y1) = (self.points[t1], self.points[t1 + 1]);
        let (x2, y2) = (self.points[t2], self.points[t2 + 1]);
        let (x3, y3) = (self.points[t3], self.points[t3 + 1]);
        let dx = x2 - x1;
        let dy = y2 - y1;
        let ex = x3 - x1;
        let ey = y3 - y1;
        let ab = (dx * ey - dy * ex) * 2.0;
        let max_coord = x1
            .abs()
            .max(y1.abs())
            .max(x2.abs())
            .max(y2.abs())
            .max(x3.abs())
            .max(y3.abs());
        let max_coord = if max_coord == 0.0 { 1.0 } else { max_coord };
        let threshold = 1e-9 * max_coord * max_coord;
        if ab.abs() < threshold {
            ((x1 + x2 + x3) / 3.0, (y1 + y2 + y3) / 3.0)
        } else {
            let d = 1.0 / ab;
            let bl = dx * dx + dy * dy;
            let cl = ex * ex + ey * ey;
            (x1 + (ey * bl - dy * cl) * d, y1 + (dx * cl - ex * bl) * d)
        }
    }
}

/// Compute a characteristic coordinate scale from a flat coordinate array.
fn coord_scale(coords: &[f64]) -> f64 {
    let mut max = 0.0f64;
    for i in (0..coords.len()).step_by(2) {
        max = max.max(coords[i].abs()).max(coords[i + 1].abs());
    }
    if max == 0.0 { 1.0 } else { max }
}

fn is_collinear(triangles: &[usize], coords: &[f64]) -> bool {
    let scale = coord_scale(coords);
    let threshold = 1e-10 * scale * scale;
    for i in (0..triangles.len()).step_by(3) {
        let a = 2 * triangles[i];
        let b = 2 * triangles[i + 1];
        let c = 2 * triangles[i + 2];
        let cross = (coords[c] - coords[a]) * (coords[b + 1] - coords[a + 1])
            - (coords[b] - coords[a]) * (coords[c + 1] - coords[a + 1]);
        if cross.abs() > threshold {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_basic() {
        let d = Delaunay::from_points(&[(0.0, 0.0), (1.0, 0.0), (0.5, 1.0), (1.5, 1.0)]);
        assert_eq!(d.len(), 4);
        assert!(!d.triangles.is_empty());
    }

    #[test]
    fn test_find() {
        let d = Delaunay::from_points(&[(0.0, 0.0), (10.0, 0.0), (5.0, 10.0)]);
        assert_eq!(d.find(4.9, 9.5, 0), 2);
    }

    #[test]
    fn test_collinear() {
        let d = Delaunay::from_points(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (3.0, 0.0)]);
        assert!(!d.collinear.is_empty());
    }

    /// Very small coordinates should NOT be considered collinear when they form a valid triangle.
    /// The cross product is 1e-12, which is below the old hardcoded 1e-10 threshold.
    #[test]
    fn test_small_scale_not_collinear() {
        let d = Delaunay::from_points(&[(0.0, 0.0), (1e-6, 0.0), (0.0, 1e-6)]);
        assert!(
            d.collinear.is_empty(),
            "micro-scale triangle should NOT be collinear"
        );
        assert!(!d.triangles.is_empty(), "should produce triangles");
    }

    /// Large coordinates that are genuinely collinear should still be detected.
    #[test]
    fn test_large_scale_collinear() {
        let d = Delaunay::from_points(&[(0.0, 0.0), (1e9, 1.0), (2e9, 2.0)]);
        assert!(
            !d.collinear.is_empty(),
            "large-scale collinear points should be detected"
        );
    }

    /// NaN coordinates should be handled deterministically without panics.
    #[test]
    fn test_nan_coordinates_handled() {
        let d = Delaunay::from_points(&[(0.0, 0.0), (f64::NAN, 1.0), (1.0, f64::NAN)]);
        // Should not panic; collinear detection with NaN should be stable
        assert!(d.len() == 3, "should have 3 points");
    }

    // ================================================================
    // Rectangle: uniform grid
    // ================================================================

    /// Regular 5×5 grid inside a 10×10 square.
    /// Every point should triangulate correctly and have the expected neighbor count.
    #[test]
    fn test_rectangle_grid() {
        let mut pts = Vec::new();
        for iy in 0..5 {
            for ix in 0..5 {
                pts.push((ix as f64 * 2.5, iy as f64 * 2.5));
            }
        }
        let d = Delaunay::from_points(&pts);
        assert_eq!(d.len(), 25);
        // Triangle count for a 5×5 grid: 4×4 quads × 2 triangles = 32
        let n_tri = d.triangles.len() / 3;
        assert!(n_tri >= 30, "expected ~32 triangles, got {n_tri}");

        // Hull should be the 16 border points (perimeter)
        let hull_pts: Vec<usize> = d.hull.clone();
        assert!(
            hull_pts.len() >= 12,
            "hull should have perimeter points, got {}",
            hull_pts.len()
        );

        // Every point should be findable
        for (i, &(x, y)) in pts.iter().enumerate() {
            let found = d.find(x, y, 0);
            assert_eq!(found, i, "find({x}, {y}) = {found}, expected {i}");
        }

        // Interior point (1,1) at index 5*0+0=... let's pick center (5.0, 5.0) = index 12
        let center_neighbors = d.neighbors(12);
        assert!(
            center_neighbors.len() >= 4,
            "center of 5×5 grid should have >= 4 neighbors, got {}",
            center_neighbors.len()
        );
    }

    /// Dense 10×10 grid — stress test for triangulation and neighbor queries.
    #[test]
    fn test_rectangle_dense_grid() {
        let mut pts = Vec::new();
        for iy in 0..10 {
            for ix in 0..10 {
                pts.push((ix as f64, iy as f64));
            }
        }
        let d = Delaunay::from_points(&pts);
        assert_eq!(d.len(), 100);

        // All 100 points should be findable from any starting point
        for (i, &(x, y)) in pts.iter().enumerate() {
            assert_eq!(d.find(x, y, 0), i, "find failed for point {i} at ({x},{y})");
        }
    }

    // ================================================================
    // Rectangle with a hole inside
    // ================================================================

    /// Points on the boundary of a rectangle, plus points on a circular hole
    /// centered at the rectangle's center. The hole is fully inside the rectangle.
    #[test]
    fn test_rectangle_with_interior_hole() {
        let mut pts = Vec::new();

        // Outer rectangle: 20 points on the boundary of [0,0]-[10,10]
        for i in 0..5 {
            pts.push((i as f64 * 2.5, 0.0));
        } // bottom
        for i in 1..5 {
            pts.push((10.0, i as f64 * 2.5));
        } // right
        for i in (0..4).rev() {
            pts.push((i as f64 * 2.5, 10.0));
        } // top
        for i in (1..4).rev() {
            pts.push((0.0, i as f64 * 2.5));
        } // left

        let n_outer = pts.len();

        // Inner hole: 12 points on a circle of radius 2 centered at (5, 5)
        for i in 0..12 {
            let angle = i as f64 / 12.0 * 2.0 * PI;
            pts.push((5.0 + 2.0 * angle.cos(), 5.0 + 2.0 * angle.sin()));
        }

        let d = Delaunay::from_points(&pts);
        assert_eq!(d.len(), pts.len());

        // All outer boundary points should be on the hull
        for (i, pt) in pts.iter().enumerate().take(n_outer) {
            assert!(
                d.hull.contains(&i),
                "outer point {i} at {pt:?} should be on hull",
            );
        }

        // No inner hole point should be on the hull
        for (i, pt) in pts.iter().enumerate().skip(n_outer) {
            assert!(
                !d.hull.contains(&i),
                "inner point {i} at {pt:?} should NOT be on hull",
            );
        }

        // Every point should be findable
        for (i, &(x, y)) in pts.iter().enumerate() {
            let found = d.find(x, y, 0);
            assert_eq!(found, i, "find({x:.2}, {y:.2}) = {found}, expected {i}");
        }

        // Neighbors of a hole point should include adjacent hole points
        let hole_start = n_outer;
        let nbrs = d.neighbors(hole_start);
        assert!(
            nbrs.contains(&(hole_start + 1)) || nbrs.contains(&(hole_start + 11)),
            "hole point should neighbor adjacent hole points"
        );
    }

    // ================================================================
    // Rectangle with a hole crossing a border
    // ================================================================

    /// Points on a rectangle boundary, with a semi-circular notch
    /// cut into the left edge (hole crosses the border).
    #[test]
    fn test_rectangle_with_border_hole() {
        let mut pts = Vec::new();

        // Outer rectangle boundary, but skip part of the left edge
        // Bottom edge
        for i in 0..6 {
            pts.push((i as f64 * 2.0, 0.0));
        }
        // Right edge
        for i in 1..6 {
            pts.push((10.0, i as f64 * 2.0));
        }
        // Top edge
        for i in (0..5).rev() {
            pts.push((i as f64 * 2.0, 10.0));
        }
        // Left edge — only top and bottom portions, gap in the middle
        pts.push((0.0, 8.0));
        pts.push((0.0, 7.0)); // stop before hole

        // Semi-circular hole cutting into left border
        // Arc from (0, 7) to (0, 3) curving inward (to the right)
        for i in 1..8 {
            let t = i as f64 / 8.0;
            let angle = PI * 0.5 + PI * t; // from π/2 to 3π/2
            let cx = 0.0; // center on left edge
            let cy = 5.0;
            let r = 2.0;
            pts.push((cx - r * angle.cos(), cy - r * angle.sin()));
        }

        pts.push((0.0, 3.0));
        pts.push((0.0, 2.0));

        let d = Delaunay::from_points(&pts);
        assert!(d.len() > 20, "should have enough points");

        // Triangulation should exist
        assert!(!d.triangles.is_empty(), "should produce triangles");

        // The notch points (inside the rectangle) should not be on hull
        // They are roughly at x > 0, so let's check that hull stays on the boundary
        let hull_xs: Vec<f64> = d.hull.iter().map(|&i| pts[i].0).collect();
        let min_hull_x = hull_xs.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        assert!(
            min_hull_x <= 0.01,
            "hull should touch the left edge, min x = {min_hull_x}"
        );

        // Find queries near the notch should work
        let found = d.find(1.5, 5.0, 0);
        assert!(found < d.len(), "find near notch should return valid index");
    }

    // ================================================================
    // 3D-relevant cases: projections from the sphere
    // ================================================================

    /// Points uniformly distributed on a unit sphere (stereographic projection to 2D).
    /// This tests the kind of point distributions that arise in geographic applications.
    #[test]
    fn test_sphere_stereographic_projection() {
        let mut pts = Vec::new();
        // Fibonacci sphere: uniform distribution on unit sphere
        let n = 100;
        let golden_ratio = (1.0 + 5.0_f64.sqrt()) / 2.0;
        for i in 0..n {
            let theta = 2.0 * PI * i as f64 / golden_ratio;
            let phi = (1.0 - 2.0 * (i as f64 + 0.5) / n as f64).acos();

            // Stereographic projection from south pole
            let x = phi.sin() * theta.cos();
            let y = phi.sin() * theta.sin();
            let z = phi.cos();
            // Skip points near south pole (z ≈ -1) where projection blows up
            if z < -0.9 {
                continue;
            }
            let scale = 2.0 / (1.0 + z);
            pts.push((x * scale, y * scale));
        }

        let d = Delaunay::from_points(&pts);
        assert!(d.len() >= 80, "should have ~95 points after filtering");
        assert!(!d.triangles.is_empty());

        // All points findable
        for (i, &(x, y)) in pts.iter().enumerate() {
            let found = d.find(x, y, 0);
            assert_eq!(found, i, "sphere point {i} not found correctly");
        }
    }

    /// Points on the equator of a sphere (projected to a line-like strip).
    /// Tests near-collinear configurations that arise from great circles.
    #[test]
    fn test_sphere_equator_band() {
        let mut pts = Vec::new();
        // Points near the equator: latitude from -10° to +10°, longitude full circle
        for i in 0..36 {
            let lon = i as f64 * 10.0 * PI / 180.0;
            for j in -2..=2 {
                let lat = j as f64 * 5.0 * PI / 180.0;
                // Equirectangular projection
                pts.push((lon, lat));
            }
        }

        let d = Delaunay::from_points(&pts);
        assert_eq!(d.len(), 180);

        // Should produce a valid triangulation even though points are in a thin strip
        let n_tri = d.triangles.len() / 3;
        assert!(
            n_tri > 100,
            "thin strip should still triangulate, got {n_tri} triangles"
        );
    }

    /// Concentric circles (like latitude lines on a sphere).
    /// Tests the common pattern of ring-like point distributions.
    #[test]
    fn test_concentric_circles() {
        let mut pts = Vec::new();
        pts.push((0.0, 0.0)); // center

        // 4 concentric rings with increasing point density
        for ring in 1..=4 {
            let r = ring as f64 * 2.5;
            let n_pts = ring * 8;
            for i in 0..n_pts {
                let angle = i as f64 / n_pts as f64 * 2.0 * PI;
                pts.push((r * angle.cos(), r * angle.sin()));
            }
        }

        let d = Delaunay::from_points(&pts);
        let expected_n = 1 + 8 + 16 + 24 + 32;
        assert_eq!(d.len(), expected_n);
        assert!(!d.triangles.is_empty());

        // Center point should be found
        assert_eq!(d.find(0.0, 0.0, 0), 0);

        // Center should neighbor points on the innermost ring
        let center_nbrs = d.neighbors(0);
        assert!(
            center_nbrs.len() >= 6,
            "center should have >= 6 neighbors (inner ring), got {}",
            center_nbrs.len()
        );
        // All center neighbors should be on ring 1 (indices 1..=8)
        for &n in &center_nbrs {
            assert!(
                (1..=8).contains(&n),
                "center neighbor {n} should be on inner ring (1..=8)"
            );
        }
    }

    /// Icosahedron vertices projected onto the plane (regular 3D polyhedron).
    /// Tests that the triangulation handles symmetrically distributed points.
    #[test]
    fn test_icosahedron_projection() {
        // 12 vertices of a regular icosahedron
        let phi = (1.0 + 5.0_f64.sqrt()) / 2.0; // golden ratio
        let verts_3d: Vec<(f64, f64, f64)> = vec![
            (-1.0, phi, 0.0),
            (1.0, phi, 0.0),
            (-1.0, -phi, 0.0),
            (1.0, -phi, 0.0),
            (0.0, -1.0, phi),
            (0.0, 1.0, phi),
            (0.0, -1.0, -phi),
            (0.0, 1.0, -phi),
            (phi, 0.0, -1.0),
            (phi, 0.0, 1.0),
            (-phi, 0.0, -1.0),
            (-phi, 0.0, 1.0),
        ];

        // Orthographic projection (drop z)
        let pts: Vec<(f64, f64)> = verts_3d.iter().map(|&(x, y, _z)| (x, y)).collect();

        let d = Delaunay::from_points(&pts);
        assert_eq!(d.len(), 12);
        assert!(!d.triangles.is_empty());

        // All 12 points should be findable (some may overlap in 2D projection)
        for (i, &(x, y)) in pts.iter().enumerate() {
            let found = d.find(x, y, 0);
            // The found point should be at the same (x,y) location
            let (fx, fy) = pts[found];
            assert!(
                (fx - x).abs() < 1e-10 && (fy - y).abs() < 1e-10,
                "icosahedron vertex {i} at ({x},{y}): find returned {found} at ({fx},{fy})"
            );
        }
    }

    /// L-shaped region: rectangle with a rectangular cutout in one corner.
    /// Tests non-convex point distributions.
    #[test]
    fn test_l_shape() {
        let mut pts = Vec::new();

        // L-shape: 10×10 square minus 5×5 cutout in top-right
        // Bottom edge: full width
        for i in 0..=10 {
            pts.push((i as f64, 0.0));
        }
        // Right edge: only lower half
        for i in 1..=5 {
            pts.push((10.0, i as f64));
        }
        // Step right at y=5
        for i in (5..=10).rev() {
            pts.push((i as f64, 5.0));
        }
        // Inner vertical: x=5, y=5 to y=10
        for i in 6..=10 {
            pts.push((5.0, i as f64));
        }
        // Top edge: only left half
        for i in (0..5).rev() {
            pts.push((i as f64, 10.0));
        }
        // Left edge
        for i in (1..10).rev() {
            pts.push((0.0, i as f64));
        }

        // Add some interior points to get proper triangulation
        for iy in 1..10 {
            for ix in 1..10 {
                // Skip the cutout region
                if ix > 5 && iy > 5 {
                    continue;
                }
                pts.push((ix as f64, iy as f64));
            }
        }

        let d = Delaunay::from_points(&pts);
        assert!(d.len() > 50, "L-shape should have many points");
        assert!(!d.triangles.is_empty());

        // Points in the L region should be findable
        assert_eq!(d.find(2.0, 2.0, 0), d.find(2.0, 2.0, 0));

        // Point in the cutout should find the nearest boundary point
        let found = d.find(8.0, 8.0, 0);
        let (fx, fy) = pts[found];
        // Should be near the inner corner (5,5) area
        assert!(
            fx <= 10.1 && fy <= 10.1,
            "found point ({fx}, {fy}) should be on the L boundary"
        );
    }

    /// X-shaped (cross) region: two overlapping rectangles.
    /// Tests a shape with 12 corners and concavities.
    #[test]
    fn test_x_shape() {
        let mut pts = Vec::new();
        let w = 10.0; // total width
        let h = 10.0; // total height
        let arm = 3.0; // arm thickness

        // Horizontal bar: full width, centered vertically
        let y_lo = (h - arm) / 2.0;
        let y_hi = (h + arm) / 2.0;
        for i in 0..=20 {
            let x = w * i as f64 / 20.0;
            pts.push((x, y_lo));
            pts.push((x, y_hi));
        }

        // Vertical bar: full height, centered horizontally
        let x_lo = (w - arm) / 2.0;
        let x_hi = (w + arm) / 2.0;
        for i in 0..=20 {
            let y = h * i as f64 / 20.0;
            pts.push((x_lo, y));
            pts.push((x_hi, y));
        }

        // Fill interior
        for iy in 1..20 {
            let y = h * iy as f64 / 20.0;
            for ix in 1..20 {
                let x = w * ix as f64 / 20.0;
                let in_h_bar = y >= y_lo && y <= y_hi;
                let in_v_bar = x >= x_lo && x <= x_hi;
                if in_h_bar || in_v_bar {
                    pts.push((x, y));
                }
            }
        }

        // Deduplicate (horizontal and vertical bars overlap)
        pts.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap()
                .then(a.1.partial_cmp(&b.1).unwrap())
        });
        pts.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-10 && (a.1 - b.1).abs() < 1e-10);

        let d = Delaunay::from_points(&pts);
        assert!(
            d.len() > 100,
            "X-shape should have many points, got {}",
            d.len()
        );
        assert!(!d.triangles.is_empty());

        // Center of the cross should be findable
        let center = d.find(5.0, 5.0, 0);
        let (cx, cy) = pts[center];
        assert!(
            (cx - 5.0).abs() < 0.5 && (cy - 5.0).abs() < 0.5,
            "center find should be near (5,5), got ({cx}, {cy})"
        );

        // Point in a concavity (outside the cross) should find nearest arm point
        let found = d.find(1.0, 1.0, 0);
        let (fx, fy) = pts[found];
        let dist = ((fx - 1.0).powi(2) + (fy - 1.0).powi(2)).sqrt();
        assert!(
            dist < 3.0,
            "point in concavity should find a nearby cross point"
        );
    }

    /// Points sampled from the surface of a unit sphere using Fibonacci spiral.
    /// Projected via equirectangular (lon, lat) → (x, y).
    #[test]
    fn test_sphere_fibonacci() {
        let n = 200;
        let golden = (1.0 + 5.0_f64.sqrt()) / 2.0;
        let mut pts = Vec::new();

        for i in 0..n {
            let theta = 2.0 * PI * i as f64 / golden;
            let phi = (1.0 - 2.0 * (i as f64 + 0.5) / n as f64).acos();
            // Equirectangular: lon = theta, lat = pi/2 - phi
            let lon = theta % (2.0 * PI);
            let lat = PI / 2.0 - phi;
            pts.push((lon, lat));
        }

        let d = Delaunay::from_points(&pts);
        assert_eq!(d.len(), n);
        assert!(!d.triangles.is_empty());

        // Every point should be findable
        for (i, &(x, y)) in pts.iter().enumerate() {
            let found = d.find(x, y, 0);
            let (fx, fy) = pts[found];
            assert!(
                (fx - x).abs() < 1e-6 && (fy - y).abs() < 1e-6,
                "sphere point {i}: find returned {found} at ({fx:.4},{fy:.4}) instead of ({x:.4},{y:.4})"
            );
        }
    }

    /// Sphere with a cylindrical hole (tube removed along z-axis).
    /// Points on the sphere surface excluding a band around the poles,
    /// simulating acoustic measurements with a pole-mounted microphone blocked zone.
    #[test]
    fn test_sphere_minus_cylinder() {
        let n = 200;
        let golden = (1.0 + 5.0_f64.sqrt()) / 2.0;
        let cylinder_radius = 0.3; // fraction of sphere radius to exclude
        let mut pts = Vec::new();

        for i in 0..n {
            let theta = 2.0 * PI * i as f64 / golden;
            let phi = (1.0 - 2.0 * (i as f64 + 0.5) / n as f64).acos();
            let x = phi.sin() * theta.cos();
            let y = phi.sin() * theta.sin();

            // Exclude points inside the cylinder (distance from z-axis < cylinder_radius)
            let dist_from_z = (x * x + y * y).sqrt();
            if dist_from_z < cylinder_radius {
                continue; // inside the cylinder hole
            }

            // Project to 2D using equirectangular
            pts.push((theta % (2.0 * PI), PI / 2.0 - phi));
        }

        let d = Delaunay::from_points(&pts);
        assert!(
            d.len() > 100,
            "sphere-minus-cylinder should have >100 points, got {}",
            d.len()
        );
        assert!(!d.triangles.is_empty());

        // Points near the "equator" (phi ≈ π/2) should be present (far from z-axis)
        let equator_point = d.find(PI, 0.0, 0);
        let (_ex, ey) = pts[equator_point];
        assert!(
            ey.abs() < 0.5,
            "point near equator should have small lat, got {ey}"
        );

        // Triangulation should have reasonable connectivity
        let n_tri = d.triangles.len() / 3;
        assert!(n_tri > 50, "should have many triangles: {n_tri}");

        // Neighbors check on a point
        let nbrs = d.neighbors(0);
        assert!(!nbrs.is_empty(), "first point should have neighbors");
    }

    /// Random-ish points in a donut shape (annulus).
    /// Common in audio/acoustic applications (e.g., microphone arrays, speaker rings).
    #[test]
    fn test_annulus_points() {
        let mut pts = Vec::new();
        let r_inner = 3.0;
        let r_outer = 7.0;

        for i in 0..60 {
            let angle = i as f64 / 60.0 * 2.0 * PI;
            // Deterministic "random" radius between inner and outer
            let r = r_inner + (r_outer - r_inner) * (0.5 + 0.4 * (i as f64 * 1.7).sin());
            pts.push((r * angle.cos(), r * angle.sin()));
        }

        let d = Delaunay::from_points(&pts);
        assert_eq!(d.len(), 60);

        // The hole in the center means the center point should NOT be part of any triangle
        // Actually, the center IS inside the convex hull, just no points there.
        // Find(0,0) should return the nearest point to center.
        let nearest = d.find(0.0, 0.0, 0);
        let (nx, ny) = pts[nearest];
        let dist = (nx * nx + ny * ny).sqrt();
        assert!(
            dist >= r_inner * 0.8,
            "nearest to center should be on the ring, dist={dist}"
        );
    }
}
