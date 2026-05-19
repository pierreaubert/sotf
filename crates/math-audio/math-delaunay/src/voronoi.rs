//! Voronoi diagram — port of d3-delaunay/src/voronoi.js
//!
//! Constructs Voronoi cells from a Delaunay triangulation, clipped to a bounding box.
//! Uses Cohen-Sutherland clipping with corner walking, matching D3 exactly.

use crate::Delaunay;
use delaunator::EMPTY as NO_EDGE;

/// Voronoi diagram with D3-compatible API.
///
/// **Encapsulation.** The internal buffers are `pub(crate)` rather than
/// `pub` to plug a mutable-state leak — callers previously could mutate
/// `circumcenters` / `vectors` / bounds and silently break every
/// subsequent query. Read-only access is exposed via `delaunay()`,
/// `bounds()`, `xmin()` / `xmax()` / `ymin()` / `ymax()`,
/// `circumcenters()`, and `vectors()`.
pub struct Voronoi<'a> {
    pub(crate) delaunay: &'a Delaunay,
    pub(crate) xmin: f64,
    pub(crate) ymin: f64,
    pub(crate) xmax: f64,
    pub(crate) ymax: f64,
    /// Circumcenters of all triangles, flat [cx0, cy0, cx1, cy1, ...].
    pub(crate) circumcenters: Vec<f64>,
    /// Exterior cell ray vectors, 4 per point [vx_in, vy_in, vx_out, vy_out].
    pub(crate) vectors: Vec<f64>,
}

impl<'a> Voronoi<'a> {
    /// Create a Voronoi diagram from a Delaunay triangulation and bounds [xmin, ymin, xmax, ymax].
    pub fn new(delaunay: &'a Delaunay, [xmin, ymin, xmax, ymax]: [f64; 4]) -> Self {
        let n_triangles = delaunay.triangles.len() / 3;
        let n_points = delaunay.len();
        let mut circumcenters = vec![0.0; n_triangles * 2];
        let mut vectors = vec![0.0; n_points * 4];

        // Compute circumcenters
        for t in 0..n_triangles {
            let (cx, cy) = delaunay.circumcenter(t);
            circumcenters[t * 2] = cx;
            circumcenters[t * 2 + 1] = cy;
        }

        // Compute exterior cell rays for hull edges
        if !delaunay.hull.is_empty() {
            let hull = &delaunay.hull;
            let points = &delaunay.points;
            let mut h = *hull.last().unwrap();
            let mut x1 = points[2 * h];
            let mut y1 = points[2 * h + 1];
            for &hi in hull.iter() {
                let h_prev = h;
                h = hi;
                let x0 = x1;
                let y0 = y1;
                x1 = points[2 * h];
                y1 = points[2 * h + 1];
                let p0 = h_prev * 4;
                let p1 = h * 4;
                vectors[p0 + 2] = y0 - y1; // outgoing vector of previous hull point
                vectors[p0 + 3] = x1 - x0;
                vectors[p1] = y0 - y1; // incoming vector of current hull point
                vectors[p1 + 1] = x1 - x0;
            }
        }

        Self {
            delaunay,
            xmin,
            ymin,
            xmax,
            ymax,
            circumcenters,
            vectors,
        }
    }

    /// The Delaunay triangulation this Voronoi diagram was built from.
    pub fn delaunay(&self) -> &Delaunay {
        self.delaunay
    }

    /// Clipping bounds as `[xmin, ymin, xmax, ymax]`.
    pub fn bounds(&self) -> [f64; 4] {
        [self.xmin, self.ymin, self.xmax, self.ymax]
    }

    /// `xmin` of the clipping bounding box.
    pub fn xmin(&self) -> f64 {
        self.xmin
    }
    /// `ymin` of the clipping bounding box.
    pub fn ymin(&self) -> f64 {
        self.ymin
    }
    /// `xmax` of the clipping bounding box.
    pub fn xmax(&self) -> f64 {
        self.xmax
    }
    /// `ymax` of the clipping bounding box.
    pub fn ymax(&self) -> f64 {
        self.ymax
    }

    /// Read-only access to triangle circumcenters,
    /// flat `[cx0, cy0, cx1, cy1, ...]`.
    pub fn circumcenters(&self) -> &[f64] {
        &self.circumcenters
    }

    /// Read-only access to the per-hull-point exterior ray vectors,
    /// 4 floats per point: `[vx_in, vy_in, vx_out, vy_out]`.
    pub fn vectors(&self) -> &[f64] {
        &self.vectors
    }

    /// Get the polygon for Voronoi cell i, clipped to bounds.
    /// Returns coordinates as `Vec<(f64, f64)>`, or None if the cell is degenerate.
    pub fn cell_polygon(&self, i: usize) -> Option<Vec<(f64, f64)>> {
        let clipped = self.clip(i)?;
        if clipped.len() < 4 {
            return None; // need at least 2 points (4 floats)
        }

        // Convert flat array to Vec<(f64, f64)>, removing duplicate closing point
        let mut result = Vec::new();
        let n = clipped.len();
        // Remove trailing duplicate of first point
        let eps = self.epsilon();
        let mut end = n;
        while end >= 4
            && (clipped[0] - clipped[end - 2]).abs() <= eps
            && (clipped[1] - clipped[end - 1]).abs() <= eps
        {
            end -= 2;
        }
        for i in (0..end).step_by(2) {
            // Skip consecutive duplicates
            if i >= 2
                && (clipped[i] - clipped[i - 2]).abs() <= eps
                && (clipped[i + 1] - clipped[i - 1]).abs() <= eps
            {
                continue;
            }
            result.push((clipped[i], clipped[i + 1]));
        }
        if result.len() < 3 {
            return None;
        }
        Some(result)
    }

    /// Iterate over all cell polygons.
    pub fn cell_polygons(&self) -> Vec<(usize, Vec<(f64, f64)>)> {
        (0..self.delaunay.len())
            .filter_map(|i| self.cell_polygon(i).map(|poly| (i, poly)))
            .collect()
    }

    /// Test if point (x, y) is inside cell i.
    pub fn contains(&self, i: usize, x: f64, y: f64) -> bool {
        if x.is_nan() || y.is_nan() {
            return false;
        }
        self.delaunay.step(i, x, y) == i
    }

    // ========================================================================
    // Internal: cell construction and clipping (direct port of D3)
    // ========================================================================

    /// Construct the raw (unclipped) Voronoi cell for point i.
    /// Returns flat coordinates [x0, y0, x1, y1, ...] or None.
    /// Construct raw (unclipped) cell — exposed for debugging.
    pub(crate) fn cell(&self, i: usize) -> Option<Vec<f64>> {
        let e0 = self.delaunay.inedges[i];
        if e0 == NO_EDGE {
            return None; // coincident point
        }
        let mut points = Vec::new();
        let mut e = e0;
        loop {
            let t = e / 3;
            points.push(self.circumcenters[t * 2]);
            points.push(self.circumcenters[t * 2 + 1]);
            e = if e % 3 == 2 { e - 2 } else { e + 1 };
            if self.delaunay.triangles[e] != i {
                break; // bad triangulation
            }
            let he = self.delaunay.halfedges[e];
            if he == NO_EDGE {
                break; // reached hull
            }
            e = he;
            if e == e0 {
                break; // completed the loop
            }
        }
        Some(points)
    }

    /// Clip cell i to the bounding box.
    fn clip(&self, i: usize) -> Option<Vec<f64>> {
        // Degenerate: single point on hull
        if i == 0 && self.delaunay.hull.len() == 1 {
            return Some(vec![
                self.xmax, self.ymin, self.xmax, self.ymax, self.xmin, self.ymax, self.xmin,
                self.ymin,
            ]);
        }
        let points = self.cell(i)?;
        let v = i * 4;
        let has_vectors = self.vectors[v] != 0.0 || self.vectors[v + 1] != 0.0;
        if has_vectors {
            self.clip_infinite(
                i,
                &points,
                self.vectors[v],
                self.vectors[v + 1],
                self.vectors[v + 2],
                self.vectors[v + 3],
            )
        } else {
            self.clip_finite(i, &points)
        }
    }

    /// Clip a finite cell (interior point, not on hull).
    fn clip_finite(&self, i: usize, points: &[f64]) -> Option<Vec<f64>> {
        let n = points.len();
        let mut p: Option<Vec<f64>> = None;
        let mut x1 = points[n - 2];
        let mut y1 = points[n - 1];
        let mut c1 = self.regioncode(x1, y1);
        let mut e1: u8 = 0;

        for j in (0..n).step_by(2) {
            let x0 = x1;
            let y0 = y1;
            x1 = points[j];
            y1 = points[j + 1];
            let c0 = c1;
            c1 = self.regioncode(x1, y1);
            let e0 = e1;

            if c0 == 0 && c1 == 0 {
                e1 = 0;
                if let Some(ref mut pp) = p {
                    pp.push(x1);
                    pp.push(y1);
                } else {
                    p = Some(vec![x1, y1]);
                }
            } else {
                let seg;
                if c0 == 0 {
                    seg = self.clip_segment(x0, y0, x1, y1, c0, c1);
                    if seg.is_none() {
                        continue;
                    }
                    let s = seg.unwrap();
                    // s = [sx0, sy0, sx1, sy1], sx0=x0, sy0=y0 (unchanged)
                    let sx1 = s[2];
                    let sy1 = s[3];
                    e1 = self.edgecode(sx1, sy1);
                    if e0 != 0 && e1 != 0 {
                        {
                            let len = p.as_ref().unwrap().len();
                            self.edge(i, e0, e1, p.as_mut().unwrap(), len);
                        }
                    }
                    if let Some(ref mut pp) = p {
                        pp.push(sx1);
                        pp.push(sy1);
                    } else {
                        p = Some(vec![sx1, sy1]);
                    }
                } else {
                    seg = self.clip_segment(x1, y1, x0, y0, c1, c0);
                    if seg.is_none() {
                        continue;
                    }
                    let s = seg.unwrap();
                    let sx0 = s[2];
                    let sy0 = s[3];
                    let sx1 = s[0];
                    let sy1 = s[1];
                    e1 = self.edgecode(sx0, sy0);
                    if e0 != 0 && e1 != 0 {
                        {
                            let len = p.as_ref().unwrap().len();
                            self.edge(i, e0, e1, p.as_mut().unwrap(), len);
                        }
                    }
                    if let Some(ref mut pp) = p {
                        pp.push(sx0);
                        pp.push(sy0);
                    } else {
                        p = Some(vec![sx0, sy0]);
                    }
                    e1 = self.edgecode(sx1, sy1);
                    if e0 != 0 && e1 != 0 {
                        {
                            let len = p.as_ref().unwrap().len();
                            self.edge(i, e0, e1, p.as_mut().unwrap(), len);
                        }
                    }
                    if let Some(ref mut pp) = p {
                        pp.push(sx1);
                        pp.push(sy1);
                    } else {
                        p = Some(vec![sx1, sy1]);
                    }
                }
            }
        }

        if let Some(ref mut pp) = p {
            let e0 = e1;
            e1 = self.edgecode(pp[0], pp[1]);
            if e0 != 0 && e1 != 0 {
                let len = pp.len();
                self.edge(i, e0, e1, pp, len);
            }
        } else if self.contains(
            i,
            (self.xmin + self.xmax) / 2.0,
            (self.ymin + self.ymax) / 2.0,
        ) {
            return Some(vec![
                self.xmax, self.ymin, self.xmax, self.ymax, self.xmin, self.ymax, self.xmin,
                self.ymin,
            ]);
        }

        self.simplify(p)
    }

    /// Clip an infinite cell (hull point with exterior rays).
    fn clip_infinite(
        &self,
        i: usize,
        points: &[f64],
        vx0: f64,
        vy0: f64,
        vxn: f64,
        vyn: f64,
    ) -> Option<Vec<f64>> {
        let mut p = points.to_vec();

        // Project the incoming ray
        if let Some([px, py]) = self.project(p[0], p[1], vx0, vy0) {
            p.insert(0, py);
            p.insert(0, px);
        }

        // Project the outgoing ray
        let n = p.len();
        if let Some([px, py]) = self.project(p[n - 2], p[n - 1], vxn, vyn) {
            p.push(px);
            p.push(py);
        }

        if let Some(mut clipped) = self.clip_finite(i, &p) {
            // Walk corners
            let n = clipped.len();
            let mut c1 = self.edgecode(clipped[n - 2], clipped[n - 1]);
            let mut j = 0;
            while j < clipped.len() {
                let c0 = c1;
                c1 = self.edgecode(clipped[j], clipped[j + 1]);
                if c0 != 0 && c1 != 0 {
                    j = self.edge(i, c0, c1, &mut clipped, j);
                    // edge may have inserted points, recheck
                }
                j += 2;
            }
            self.simplify(Some(clipped))
        } else if self.contains(
            i,
            (self.xmin + self.xmax) / 2.0,
            (self.ymin + self.ymax) / 2.0,
        ) {
            Some(vec![
                self.xmin, self.ymin, self.xmax, self.ymin, self.xmax, self.ymax, self.xmin,
                self.ymax,
            ])
        } else {
            None
        }
    }

    /// Cohen-Sutherland segment clipping.
    fn clip_segment(
        &self,
        mut x0: f64,
        mut y0: f64,
        mut x1: f64,
        mut y1: f64,
        mut c0: u8,
        mut c1: u8,
    ) -> Option<[f64; 4]> {
        let flip = c0 < c1;
        if flip {
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut y0, &mut y1);
            std::mem::swap(&mut c0, &mut c1);
        }
        for _ in 0..20 {
            if c0 == 0 && c1 == 0 {
                return if flip {
                    Some([x1, y1, x0, y0])
                } else {
                    Some([x0, y0, x1, y1])
                };
            }
            if c0 & c1 != 0 {
                return None;
            }
            let c = if c0 != 0 { c0 } else { c1 };
            let (x, y);
            if c & 0b1000 != 0 {
                x = x0 + (x1 - x0) * (self.ymax - y0) / (y1 - y0);
                y = self.ymax;
            } else if c & 0b0100 != 0 {
                x = x0 + (x1 - x0) * (self.ymin - y0) / (y1 - y0);
                y = self.ymin;
            } else if c & 0b0010 != 0 {
                y = y0 + (y1 - y0) * (self.xmax - x0) / (x1 - x0);
                x = self.xmax;
            } else {
                y = y0 + (y1 - y0) * (self.xmin - x0) / (x1 - x0);
                x = self.xmin;
            }
            if c0 != 0 {
                x0 = x;
                y0 = y;
                c0 = self.regioncode(x0, y0);
            } else {
                x1 = x;
                y1 = y;
                c1 = self.regioncode(x1, y1);
            }
        }
        None // safety limit reached
    }

    /// Walk corners of the bounding box from edge e0 to e1, inserting corner points.
    /// Returns the index `j` after all insertions (matching D3's `_edge` return value).
    fn edge(&self, i: usize, mut e0: u8, e1: u8, p: &mut Vec<f64>, mut j: usize) -> usize {
        for _ in 0..8 {
            if e0 == e1 {
                break;
            }
            let (x, y);
            match e0 {
                0b0101 => {
                    e0 = 0b0100;
                    continue;
                }
                0b0100 => {
                    e0 = 0b0110;
                    x = self.xmax;
                    y = self.ymin;
                }
                0b0110 => {
                    e0 = 0b0010;
                    continue;
                }
                0b0010 => {
                    e0 = 0b1010;
                    x = self.xmax;
                    y = self.ymax;
                }
                0b1010 => {
                    e0 = 0b1000;
                    continue;
                }
                0b1000 => {
                    e0 = 0b1001;
                    x = self.xmin;
                    y = self.ymax;
                }
                0b1001 => {
                    e0 = 0b0001;
                    continue;
                }
                0b0001 => {
                    e0 = 0b0101;
                    x = self.xmin;
                    y = self.ymin;
                }
                _ => return j,
            }
            // D3: if ((P[j] !== x || P[j + 1] !== y) && this.contains(i, x, y))
            let dup = j + 1 < p.len() && p[j] == x && p[j + 1] == y;
            if !dup && self.contains(i, x, y) {
                p.splice(j..j, [x, y]);
                j += 2;
            }
        }
        j
    }

    /// Project a ray from (x0, y0) in direction (vx, vy) to the bounding box edge.
    fn project(&self, x0: f64, y0: f64, vx: f64, vy: f64) -> Option<[f64; 2]> {
        let mut t = f64::INFINITY;
        let mut x = f64::NAN;
        let mut y = f64::NAN;

        if vy < 0.0 {
            if y0 <= self.ymin {
                return None;
            }
            let c = (self.ymin - y0) / vy;
            if c < t {
                t = c;
                y = self.ymin;
                x = x0 + t * vx;
            }
        } else if vy > 0.0 {
            if y0 >= self.ymax {
                return None;
            }
            let c = (self.ymax - y0) / vy;
            if c < t {
                t = c;
                y = self.ymax;
                x = x0 + t * vx;
            }
        }

        if vx > 0.0 {
            if x0 >= self.xmax {
                return None;
            }
            let c = (self.xmax - x0) / vx;
            if c < t {
                x = self.xmax;
                y = y0 + c * vy;
            }
        } else if vx < 0.0 {
            if x0 <= self.xmin {
                return None;
            }
            let c = (self.xmin - x0) / vx;
            if c < t {
                x = self.xmin;
                y = y0 + c * vy;
            }
        }

        if x.is_nan() { None } else { Some([x, y]) }
    }

    /// Characteristic length scale of the bounding box.
    fn bbox_scale(&self) -> f64 {
        let w = (self.xmax - self.xmin).abs();
        let h = (self.ymax - self.ymin).abs();
        w.max(h).max(1.0)
    }

    /// Geometric epsilon relative to the bounding box size.
    fn epsilon(&self) -> f64 {
        1e-9 * self.bbox_scale()
    }

    /// Edge code: which edge(s) of the bounding box a point lies on.
    fn edgecode(&self, x: f64, y: f64) -> u8 {
        let eps = self.epsilon();
        let mut code = 0u8;
        if (x - self.xmin).abs() <= eps {
            code |= 0b0001;
        } else if (x - self.xmax).abs() <= eps {
            code |= 0b0010;
        }
        if (y - self.ymin).abs() <= eps {
            code |= 0b0100;
        } else if (y - self.ymax).abs() <= eps {
            code |= 0b1000;
        }
        code
    }

    /// Region code: which side(s) of the bounding box a point is outside of.
    fn regioncode(&self, x: f64, y: f64) -> u8 {
        let mut code = 0u8;
        if x < self.xmin {
            code |= 0b0001;
        } else if x > self.xmax {
            code |= 0b0010;
        }
        if y < self.ymin {
            code |= 0b0100;
        } else if y > self.ymax {
            code |= 0b1000;
        }
        code
    }

    /// Remove collinear points from the polygon.
    ///
    /// The 2D cross product `(j − i) × (k − i)` has units of area
    /// (`length²`), so the natural relative tolerance is
    /// `1e-9 · bbox_scale²`. The previous implementation used
    /// `eps² = (1e-9 · bbox_scale)²`, which is nine orders of magnitude
    /// tighter than intended and silently retained near-collinear
    /// vertices that should have been simplified.
    fn simplify(&self, p: Option<Vec<f64>>) -> Option<Vec<f64>> {
        let mut p = p?;
        if p.len() > 4 {
            let bbox = self.bbox_scale();
            let area_eps = bbox * bbox * 1e-9;
            let mut i = 0;
            while i < p.len() && p.len() > 4 {
                let j = (i + 2) % p.len();
                let k = (i + 4) % p.len();
                // 2D cross product for collinearity (axis-aligned or diagonal)
                let cross =
                    (p[j] - p[i]) * (p[k + 1] - p[i + 1]) - (p[j + 1] - p[i + 1]) * (p[k] - p[i]);
                if cross.abs() <= area_eps {
                    p.remove(j + 1);
                    p.remove(j);
                    if i >= 2 {
                        i -= 2;
                    }
                } else {
                    i += 2;
                }
            }
            if p.is_empty() {
                return None;
            }
        }
        Some(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    // ================================================================
    // Helpers
    // ================================================================

    /// Compute the area of a polygon given as Vec<(f64, f64)> using the shoelace formula.
    fn polygon_area(poly: &[(f64, f64)]) -> f64 {
        let n = poly.len();
        let mut area = 0.0;
        for i in 0..n {
            let j = (i + 1) % n;
            area += poly[i].0 * poly[j].1;
            area -= poly[j].0 * poly[i].1;
        }
        area.abs() / 2.0
    }

    /// Check that a polygon is convex.
    fn is_convex(poly: &[(f64, f64)]) -> bool {
        let n = poly.len();
        if n < 3 {
            return false;
        }
        let mut sign = 0i32;
        for i in 0..n {
            let j = (i + 1) % n;
            let k = (i + 2) % n;
            let cross = (poly[j].0 - poly[i].0) * (poly[k].1 - poly[j].1)
                - (poly[j].1 - poly[i].1) * (poly[k].0 - poly[j].0);
            if cross.abs() < 1e-10 {
                continue;
            }
            let s = if cross > 0.0 { 1 } else { -1 };
            if sign == 0 {
                sign = s;
            } else if sign != s {
                return false;
            }
        }
        true
    }

    // ================================================================
    // Basic tests
    // ================================================================

    #[test]
    fn test_voronoi_basic() {
        let points = vec![(0.0, 0.0), (10.0, 0.0), (5.0, 10.0)];
        let d = Delaunay::from_points(&points);
        let v = d.voronoi([0.0, 0.0, 10.0, 10.0]);

        for i in 0..3 {
            let cell = v.cell_polygon(i);
            assert!(cell.is_some(), "cell {i} should exist");
            let cell = cell.unwrap();
            assert!(
                cell.len() >= 3,
                "cell {i} should have >= 3 vertices, got {}",
                cell.len()
            );
        }
    }

    #[test]
    fn test_voronoi_contains() {
        let points = vec![(2.0, 2.0), (8.0, 2.0), (5.0, 8.0)];
        let d = Delaunay::from_points(&points);
        let v = d.voronoi([0.0, 0.0, 10.0, 10.0]);

        assert!(v.contains(0, 1.0, 1.0));
        assert!(v.contains(1, 9.0, 1.0));
    }

    /// Diagonal collinear points in a polygon should be simplified.
    /// Three points on the line y = x should reduce to two points.
    #[test]
    fn test_simplify_diagonal_collinear() {
        let points = vec![(0.0, 0.0), (10.0, 0.0), (5.0, 10.0)];
        let d = Delaunay::from_points(&points);
        let v = d.voronoi([0.0, 0.0, 10.0, 10.0]);

        // Craft a polygon with a diagonal collinear middle point
        let poly = vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 0.0];
        let simplified = v.simplify(Some(poly));
        assert!(simplified.is_some());
        let s = simplified.unwrap();
        assert_eq!(
            s.len(),
            6,
            "should remove the middle diagonal collinear point, got {s:?}"
        );
    }

    /// Near-collinear (but not exactly collinear) middle point should still
    /// be removed by `simplify` when its perpendicular offset from the line
    /// is well within the area-scale epsilon. The OLD `eps²` threshold was
    /// nine orders of magnitude tighter than intended, so this case
    /// silently failed to simplify.
    #[test]
    fn test_simplify_near_collinear() {
        let points = vec![(0.0, 0.0), (10.0, 0.0), (5.0, 10.0)];
        let d = Delaunay::from_points(&points);
        let v = d.voronoi([0.0, 0.0, 10.0, 10.0]);

        // Bounding box scale = max(10, 10) = 10, area_eps = 10² · 1e-9 = 1e-7.
        // We offset the middle vertex perpendicular to y=x by ~5e-9, well
        // below 1e-7 but well above the OLD eps² ≈ 1e-16.
        let poly = vec![0.0, 0.0, 1.0, 1.0 + 5e-9, 2.0, 2.0, 3.0, 0.0];
        let simplified = v.simplify(Some(poly));
        assert!(simplified.is_some());
        let s = simplified.unwrap();
        assert_eq!(
            s.len(),
            6,
            "near-collinear middle point should be removed under the \
             area-scale epsilon, got {s:?}"
        );
    }

    /// Points computed near the clipping boundary should still be recognized as on-edge.
    #[test]
    fn test_edgecode_near_boundary() {
        let points = vec![(0.0, 0.0), (10.0, 0.0), (5.0, 10.0)];
        let d = Delaunay::from_points(&points);
        let v = d.voronoi([0.0, 0.0, 10.0, 10.0]);

        // A point that is mathematically exactly on xmin but computed with tiny fp error
        let code = v.edgecode(1e-15, 5.0);
        assert!(
            code != 0,
            "point extremely close to xmin should have an edge code"
        );
    }

    #[test]
    fn test_voronoi_many_points() {
        let points: Vec<(f64, f64)> = (0..50)
            .map(|i| {
                let t = i as f64 / 49.0;
                (t * 100.0, 50.0 + 30.0 * (i as f64 * 0.3).sin())
            })
            .collect();
        let d = Delaunay::from_points(&points);
        let v = d.voronoi([0.0, 0.0, 100.0, 100.0]);

        let mut cell_count = 0;
        for i in 0..50 {
            if v.cell_polygon(i).is_some() {
                cell_count += 1;
            }
        }
        assert!(
            cell_count >= 45,
            "should have >= 45 valid cells, got {cell_count}"
        );
    }

    // ================================================================
    // Rectangle: uniform grid
    // ================================================================

    /// Voronoi of a 5×5 grid. Each interior cell should be a small rectangle.
    /// Total area of all cells should equal the bounding box area.
    #[test]
    fn test_voronoi_rectangle_grid() {
        let mut pts = Vec::new();
        for iy in 0..5 {
            for ix in 0..5 {
                pts.push((ix as f64 * 2.5, iy as f64 * 2.5));
            }
        }
        let d = Delaunay::from_points(&pts);
        let v = d.voronoi([0.0, 0.0, 10.0, 10.0]);

        let mut total_area = 0.0;
        let mut cell_count = 0;
        for i in 0..25 {
            if let Some(cell) = v.cell_polygon(i) {
                assert!(cell.len() >= 3, "cell {i} should be a polygon");
                let area = polygon_area(&cell);
                assert!(area > 0.0, "cell {i} should have positive area");
                total_area += area;
                cell_count += 1;

                // Each cell should be convex (Voronoi cells are always convex)
                assert!(is_convex(&cell), "cell {i} should be convex");
            }
        }

        assert_eq!(cell_count, 25, "all 25 grid points should have cells");

        // Total area should equal the bounding box area (10 × 10 = 100)
        assert!(
            (total_area - 100.0).abs() < 1.0,
            "total cell area {total_area} should be ~100"
        );

        // Interior cells: center point (5.0, 5.0) at index 12 has spacing 2.5,
        // so its Voronoi cell is 2.5 × 2.5 = 6.25
        let center_cell = v.cell_polygon(12).unwrap();
        let center_area = polygon_area(&center_cell);
        assert!(
            (center_area - 6.25).abs() < 1.0,
            "center cell area {center_area} should be ~6.25"
        );

        // Containment: each point should be inside its own cell
        for (i, &(x, y)) in pts.iter().enumerate() {
            assert!(v.contains(i, x, y), "point {i} should be in its own cell");
        }
    }

    // ================================================================
    // Rectangle with a hole inside
    // ================================================================

    /// Points on outer rectangle + inner circular hole.
    /// Voronoi cells near the hole should be elongated toward the center.
    #[test]
    fn test_voronoi_rectangle_with_interior_hole() {
        let mut pts = Vec::new();

        // Outer rectangle boundary: 16 points
        for i in 0..5 {
            pts.push((i as f64 * 2.5, 0.0));
        }
        for i in 1..5 {
            pts.push((10.0, i as f64 * 2.5));
        }
        for i in (0..4).rev() {
            pts.push((i as f64 * 2.5, 10.0));
        }
        for i in (1..4).rev() {
            pts.push((0.0, i as f64 * 2.5));
        }
        let n_outer = pts.len();

        // Inner hole: 12 points on circle r=2 at center (5, 5)
        for i in 0..12 {
            let angle = i as f64 / 12.0 * 2.0 * PI;
            pts.push((5.0 + 2.0 * angle.cos(), 5.0 + 2.0 * angle.sin()));
        }

        let d = Delaunay::from_points(&pts);
        let v = d.voronoi([0.0, 0.0, 10.0, 10.0]);

        let mut total_area = 0.0;
        let mut valid_cells = 0;
        for i in 0..pts.len() {
            if let Some(cell) = v.cell_polygon(i) {
                total_area += polygon_area(&cell);
                valid_cells += 1;
                assert!(is_convex(&cell), "cell {i} should be convex");
            }
        }

        // All cells should exist
        assert_eq!(valid_cells, pts.len(), "all points should have cells");

        // Total area = bounding box
        assert!(
            (total_area - 100.0).abs() < 2.0,
            "total area {total_area} should be ~100"
        );

        // Inner ring points are packed tighter → their cells should be smaller
        // than the largest outer cells (corner cells are very large)
        let outer_areas: Vec<f64> = (0..n_outer)
            .filter_map(|i| v.cell_polygon(i).map(|c| polygon_area(&c)))
            .collect();
        let inner_areas: Vec<f64> = (n_outer..pts.len())
            .filter_map(|i| v.cell_polygon(i).map(|c| polygon_area(&c)))
            .collect();

        let max_outer = outer_areas.iter().fold(0.0f64, |a, &b| a.max(b));
        let max_inner = inner_areas.iter().fold(0.0f64, |a, &b| a.max(b));
        assert!(
            max_inner < max_outer,
            "largest inner cell ({max_inner:.2}) should be smaller than largest outer cell ({max_outer:.2})"
        );
    }

    // ================================================================
    // Rectangle with a hole crossing a border
    // ================================================================

    /// Points along a rectangle boundary with a semi-circular notch on one edge.
    /// Tests that Voronoi cells handle the irregular boundary correctly.
    #[test]
    fn test_voronoi_rectangle_with_border_hole() {
        let mut pts = Vec::new();

        // Full rectangle boundary plus indentation on left side
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
        // Left edge with notch — points curve inward
        pts.push((0.0, 8.0));
        for i in 1..6 {
            let t = i as f64 / 6.0;
            let angle = PI * 0.5 + PI * t;
            pts.push((-1.5 * angle.cos(), 5.0 - 1.5 * angle.sin()));
        }
        pts.push((0.0, 2.0));

        let d = Delaunay::from_points(&pts);
        let bounds = [-2.0, 0.0, 10.0, 10.0]; // extended left to include notch
        let v = d.voronoi(bounds);

        let mut total_area = 0.0;
        let mut valid_cells = 0;
        for i in 0..pts.len() {
            if let Some(cell) = v.cell_polygon(i) {
                let area = polygon_area(&cell);
                assert!(area > 0.0, "cell {i} should have positive area");
                total_area += area;
                valid_cells += 1;
            }
        }

        let bounds_area = (bounds[2] - bounds[0]) * (bounds[3] - bounds[1]);
        assert!(
            (total_area - bounds_area).abs() < 5.0,
            "total area {total_area} should be ~{bounds_area}"
        );
        assert!(
            valid_cells >= pts.len() - 2,
            "most points should have valid cells: {valid_cells}/{}",
            pts.len()
        );
    }

    // ================================================================
    // 3D-relevant cases
    // ================================================================

    /// Voronoi of sphere points (stereographic projection).
    /// Cells should tile the bounded region without gaps.
    #[test]
    fn test_voronoi_sphere_stereographic() {
        let mut pts = Vec::new();
        let n = 80;
        let golden_ratio = (1.0 + 5.0_f64.sqrt()) / 2.0;
        for i in 0..n {
            let theta = 2.0 * PI * i as f64 / golden_ratio;
            let phi = (1.0 - 2.0 * (i as f64 + 0.5) / n as f64).acos();
            let x = phi.sin() * theta.cos();
            let y = phi.sin() * theta.sin();
            let z = phi.cos();
            if z < -0.8 {
                continue;
            }
            let scale = 2.0 / (1.0 + z);
            pts.push((x * scale, y * scale));
        }

        // Compute bounds that encompass all points plus margin
        let x_min = pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min) - 1.0;
        let x_max = pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max) + 1.0;
        let y_min = pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min) - 1.0;
        let y_max = pts.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max) + 1.0;

        let d = Delaunay::from_points(&pts);
        let v = d.voronoi([x_min, y_min, x_max, y_max]);

        let mut valid = 0;
        let mut total_area = 0.0;
        for (i, cell) in (0..pts.len()).filter_map(|i| v.cell_polygon(i).map(|c| (i, c))) {
            valid += 1;
            total_area += polygon_area(&cell);
            assert!(is_convex(&cell), "sphere voronoi cell {i} should be convex");
        }

        let mut missing = Vec::new();
        for (i, &(x, y)) in pts.iter().enumerate() {
            if v.cell_polygon(i).is_none() {
                let inedge = d.inedges[i];
                let on_hull = d.hull.contains(&i);
                let raw = v.cell(i);
                let raw_len = raw.as_ref().map(|c| c.len()).unwrap_or(0);
                missing.push(format!(
                    "  {i}: ({x:.3},{y:.3}) inedge={} hull={on_hull} raw_len={raw_len}",
                    if inedge == delaunator::EMPTY {
                        "NONE".to_string()
                    } else {
                        inedge.to_string()
                    }
                ));
            }
        }
        if !missing.is_empty() {
            eprintln!(
                "sphere-stereographic: {valid}/{} valid, {} missing:",
                pts.len(),
                missing.len()
            );
            for m in &missing {
                eprintln!("{m}");
            }
        }
        assert_eq!(
            valid,
            pts.len(),
            "ALL cells should be valid: {valid}/{}",
            pts.len()
        );
        let expected = (x_max - x_min) * (y_max - y_min);
        assert!(
            (total_area - expected).abs() < expected * 0.05,
            "total area {total_area} should be ~{expected}"
        );
    }

    /// Voronoi of concentric rings (microphone array / speaker layout pattern).
    /// Tests that annular point distributions produce correct cells.
    #[test]
    fn test_voronoi_concentric_rings() {
        let mut pts = Vec::new();
        pts.push((0.0, 0.0)); // center

        for ring in 1..=3 {
            let r = ring as f64 * 3.0;
            let n = ring * 8;
            for i in 0..n {
                let angle = i as f64 / n as f64 * 2.0 * PI;
                pts.push((r * angle.cos(), r * angle.sin()));
            }
        }

        let d = Delaunay::from_points(&pts);
        let v = d.voronoi([-12.0, -12.0, 12.0, 12.0]);

        // Center cell should be a polygon surrounding the origin
        let center_cell = v.cell_polygon(0);
        assert!(center_cell.is_some(), "center should have a cell");
        let cc = center_cell.unwrap();
        assert!(is_convex(&cc), "center cell should be convex");

        // Center cell should contain the origin
        assert!(v.contains(0, 0.0, 0.0), "center cell should contain origin");

        // Total area check
        let mut total = 0.0;
        for i in 0..pts.len() {
            if let Some(c) = v.cell_polygon(i) {
                total += polygon_area(&c);
            }
        }
        let expected = 24.0 * 24.0;
        assert!(
            (total - expected).abs() < 5.0,
            "total area {total} should be ~{expected}"
        );
    }

    /// Voronoi of L-shaped region.
    /// Tests that cells properly tile the non-convex domain.
    #[test]
    fn test_voronoi_l_shape() {
        let mut pts = Vec::new();
        // L-shape interior grid: 10×10 minus 5×5 cutout in top-right
        for iy in 0..=10 {
            for ix in 0..=10 {
                if ix > 5 && iy > 5 {
                    continue;
                }
                pts.push((ix as f64, iy as f64));
            }
        }

        let d = Delaunay::from_points(&pts);
        let v = d.voronoi([0.0, 0.0, 10.0, 10.0]);

        let mut total_area = 0.0;
        let mut valid = 0;
        for i in 0..pts.len() {
            if let Some(cell) = v.cell_polygon(i) {
                total_area += polygon_area(&cell);
                valid += 1;
            }
        }
        // The bounding box is 10×10=100, but cells outside the L still get bounded by the box
        assert_eq!(valid, pts.len(), "all L-shape points should have cells");
        assert!(
            (total_area - 100.0).abs() < 2.0,
            "total area {total_area} should be ~100 (bounding box)"
        );
    }

    /// Voronoi of X-shaped (cross) region.
    /// Tests complex concavities in the point distribution.
    #[test]
    fn test_voronoi_x_shape() {
        let mut pts = Vec::new();
        let arm = 3.0;
        let size = 10.0;
        let y_lo = (size - arm) / 2.0;
        let y_hi = (size + arm) / 2.0;
        let x_lo = (size - arm) / 2.0;
        let x_hi = (size + arm) / 2.0;

        for iy in 0..=20 {
            let y = size * iy as f64 / 20.0;
            for ix in 0..=20 {
                let x = size * ix as f64 / 20.0;
                if (y >= y_lo && y <= y_hi) || (x >= x_lo && x <= x_hi) {
                    pts.push((x, y));
                }
            }
        }
        pts.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap()
                .then(a.1.partial_cmp(&b.1).unwrap())
        });
        pts.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-10 && (a.1 - b.1).abs() < 1e-10);

        let d = Delaunay::from_points(&pts);
        let v = d.voronoi([0.0, 0.0, 10.0, 10.0]);

        let mut valid = 0;
        let mut total = 0.0;
        for i in 0..pts.len() {
            if let Some(cell) = v.cell_polygon(i) {
                total += polygon_area(&cell);
                valid += 1;
                assert!(
                    is_convex(&cell),
                    "X-shape voronoi cell {i} should be convex"
                );
            }
        }
        eprintln!("X-shape: {valid}/{} cells valid", pts.len());
        assert_eq!(
            valid,
            pts.len(),
            "ALL X-shape cells should be valid: {valid}/{}",
            pts.len()
        );
        assert!(
            (total - 100.0).abs() < 5.0,
            "total area {total} should be ~100"
        );
    }

    /// Voronoi of sphere-minus-cylinder points.
    /// Simulates the common pattern of excluding a region around the z-axis.
    #[test]
    fn test_voronoi_sphere_minus_cylinder() {
        let golden = (1.0 + 5.0_f64.sqrt()) / 2.0;
        let n = 150;
        let cyl_r = 0.3;
        let mut pts = Vec::new();

        for i in 0..n {
            let theta = 2.0 * PI * i as f64 / golden;
            let phi = (1.0 - 2.0 * (i as f64 + 0.5) / n as f64).acos();
            let x = phi.sin() * theta.cos();
            let y = phi.sin() * theta.sin();
            if (x * x + y * y).sqrt() < cyl_r {
                continue;
            }
            pts.push((theta % (2.0 * PI), PI / 2.0 - phi));
        }

        let d = Delaunay::from_points(&pts);
        let v = d.voronoi([0.0, -PI / 2.0, 2.0 * PI, PI / 2.0]);

        let mut valid = 0;
        let mut missing_reasons: Vec<String> = Vec::new();
        for (i, &(x, y)) in pts.iter().enumerate() {
            if v.cell_polygon(i).is_some() {
                valid += 1;
            } else {
                let inedge = d.inedges[i];
                let on_hull = d.hull.contains(&i);
                let has_cell = v.cell(i).is_some();
                let cell_len = v.cell(i).map(|c| c.len()).unwrap_or(0);
                missing_reasons.push(format!(
                    "  cell {i}: pt=({x:.3},{y:.3}) inedge={inedge} hull={on_hull} raw_cell={has_cell}(len={cell_len})"
                ));
            }
        }
        if !missing_reasons.is_empty() {
            eprintln!("Missing {} cells:", missing_reasons.len());
            for r in missing_reasons.iter().take(10) {
                eprintln!("{r}");
            }
        }
        eprintln!("sphere-minus-cylinder: {valid}/{} cells valid", pts.len());
        assert_eq!(
            valid,
            pts.len(),
            "ALL cells should be valid: {valid}/{}",
            pts.len()
        );
    }

    /// Voronoi of icosahedron vertices (projected).
    /// Tests a small highly-symmetric point set.
    #[test]
    fn test_voronoi_icosahedron() {
        let phi = (1.0 + 5.0_f64.sqrt()) / 2.0;
        let pts: Vec<(f64, f64)> = vec![
            (-1.0, phi),
            (1.0, phi),
            (-1.0, -phi),
            (1.0, -phi),
            (0.0, -1.0),
            (0.0, 1.0),
            (phi, 0.0),
            (-phi, 0.0),
        ];

        let d = Delaunay::from_points(&pts);
        let v = d.voronoi([-3.0, -3.0, 3.0, 3.0]);

        let mut valid = 0;
        for i in 0..pts.len() {
            if let Some(cell) = v.cell_polygon(i) {
                assert!(cell.len() >= 3, "icosa cell {i} too small");
                assert!(is_convex(&cell), "icosa cell {i} not convex");
                valid += 1;
            }
        }
        assert_eq!(valid, 8, "all 8 projected icosa vertices should have cells");

        // Containment
        for (i, &(x, y)) in pts.iter().enumerate() {
            assert!(
                v.contains(i, x, y),
                "icosa point {i} should be in its own cell"
            );
        }
    }
}
