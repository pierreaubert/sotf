//! Marching Squares algorithm for contour generation
//!
//! Implements the marching squares algorithm for generating contour lines
//! from a 2D scalar field.

use crate::shape::path::Point;

/// A contour ring (polygon) representing a closed contour line.
#[derive(Debug, Clone, Default)]
pub struct ContourRing {
    /// The points forming this ring
    pub points: Vec<Point>,
}

impl ContourRing {
    /// Create a new contour ring.
    pub fn new(points: Vec<Point>) -> Self {
        Self { points }
    }

    /// Check if the ring is closed (first and last points are the same).
    pub fn is_closed(&self) -> bool {
        if self.points.len() < 2 {
            return false;
        }
        let first = &self.points[0];
        let last = &self.points[self.points.len() - 1];
        (first.x - last.x).abs() < 1e-10 && (first.y - last.y).abs() < 1e-10
    }

    /// Get the area of this ring (positive for counter-clockwise, negative for clockwise).
    pub fn area(&self) -> f64 {
        if self.points.len() < 3 {
            return 0.0;
        }

        let mut sum = 0.0;
        for i in 0..self.points.len() - 1 {
            let p0 = &self.points[i];
            let p1 = &self.points[i + 1];
            sum += (p1.x - p0.x) * (p1.y + p0.y);
        }
        sum / 2.0
    }
}

/// A contour at a specific threshold value.
#[derive(Debug, Clone)]
pub struct Contour {
    /// The threshold value for this contour
    pub value: f64,
    /// The outer ring of the contour
    pub coordinates: Vec<ContourRing>,
}

impl Contour {
    /// Create a new contour.
    pub fn new(value: f64) -> Self {
        Self {
            value,
            coordinates: Vec::new(),
        }
    }

    /// Add a ring to this contour.
    pub fn add_ring(&mut self, ring: ContourRing) {
        self.coordinates.push(ring);
    }
}

/// Contour generator using the marching squares algorithm.
///
/// # Example
///
/// ```
/// use d3rs::contour::ContourGenerator;
///
/// // Create a 4x4 grid with values
/// let values = vec![
///     0.0, 0.0, 0.0, 0.0,
///     0.0, 1.0, 1.0, 0.0,
///     0.0, 1.0, 1.0, 0.0,
///     0.0, 0.0, 0.0, 0.0,
/// ];
///
/// let generator = ContourGenerator::new(4, 4);
/// let contour = generator.contour(&values, 0.5);
/// assert_eq!(contour.value, 0.5);
/// ```
#[derive(Debug, Clone)]
pub struct ContourGenerator {
    /// Width of the grid
    width: usize,
    /// Height of the grid
    height: usize,
    /// X origin
    x0: f64,
    /// Y origin
    y0: f64,
    /// X extent
    x1: f64,
    /// Y extent
    y1: f64,
}

impl ContourGenerator {
    /// Create a new contour generator for a grid of the given dimensions.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            x0: 0.0,
            y0: 0.0,
            x1: width as f64,
            y1: height as f64,
        }
    }

    /// Set the x range for the contour output.
    pub fn x(mut self, x0: f64, x1: f64) -> Self {
        self.x0 = x0;
        self.x1 = x1;
        self
    }

    /// Set the y range for the contour output.
    pub fn y(mut self, y0: f64, y1: f64) -> Self {
        self.y0 = y0;
        self.y1 = y1;
        self
    }

    /// Generate a contour at the given threshold value.
    pub fn contour(&self, values: &[f64], threshold: f64) -> Contour {
        let mut contour = Contour::new(threshold);

        if values.len() < (self.width * self.height) {
            return contour;
        }

        // Track which edges have been visited
        let mut visited = vec![false; self.width * self.height * 4];

        // For each cell, check if it crosses the threshold
        for j in 0..self.height - 1 {
            for i in 0..self.width - 1 {
                let case = self.cell_case(values, i, j, threshold);

                if case == 0 || case == 15 {
                    continue; // No contour crosses this cell
                }

                // Try to trace contours starting from edges that cross the threshold
                for &edge in Self::crossing_edges(case) {
                    let idx = (j * self.width + i) * 4 + edge;
                    if visited[idx] {
                        continue;
                    }

                    if let Some(ring) = self.trace_contour(values, threshold, i, j, edge, &mut visited) {
                        if ring.points.len() >= 3 {
                            contour.add_ring(ring);
                        }
                    }
                }
            }
        }

        contour
    }

    /// Get the edges that have a crossing for a given case.
    fn crossing_edges(case: u8) -> &'static [usize] {
        // For each case, return the edges that have a crossing
        // Edge 0 crosses if bits 0 and 1 differ
        // Edge 1 crosses if bits 1 and 2 differ
        // Edge 2 crosses if bits 2 and 3 differ
        // Edge 3 crosses if bits 3 and 0 differ
        match case {
            0 | 15 => &[],                  // No crossings
            1 | 14 => &[0, 3],              // bottom and left
            2 | 13 => &[0, 1],              // bottom and right
            3 | 12 => &[1, 3],              // right and left
            4 | 11 => &[1, 2],              // right and top
            5 => &[0, 1, 2, 3],             // saddle: all edges
            6 | 9 => &[0, 2],               // bottom and top
            7 | 8 => &[2, 3],               // top and left
            10 => &[0, 1, 2, 3],            // saddle: all edges
            _ => &[],
        }
    }

    /// Generate contours at multiple threshold values.
    pub fn contours(&self, values: &[f64], thresholds: &[f64]) -> Vec<Contour> {
        thresholds
            .iter()
            .map(|&t| self.contour(values, t))
            .collect()
    }

    /// Compute the marching squares case for a cell.
    fn cell_case(&self, values: &[f64], i: usize, j: usize, threshold: f64) -> u8 {
        let v00 = values[j * self.width + i];
        let v10 = values[j * self.width + i + 1];
        let v01 = values[(j + 1) * self.width + i];
        let v11 = values[(j + 1) * self.width + i + 1];

        let mut case = 0u8;
        if v00 >= threshold {
            case |= 1;
        }
        if v10 >= threshold {
            case |= 2;
        }
        if v11 >= threshold {
            case |= 4;
        }
        if v01 >= threshold {
            case |= 8;
        }
        case
    }

    /// Trace a contour starting from a cell and edge.
    fn trace_contour(
        &self,
        values: &[f64],
        threshold: f64,
        start_i: usize,
        start_j: usize,
        start_edge: usize,
        visited: &mut [bool],
    ) -> Option<ContourRing> {
        let mut points = Vec::new();
        let mut i = start_i;
        let mut j = start_j;
        let mut entry_edge = start_edge;

        loop {
            // Mark this entry edge as visited
            let idx = (j * self.width + i) * 4 + entry_edge;
            if visited[idx] {
                break;
            }
            visited[idx] = true;

            // Get the interpolated point on the entry edge
            if let Some(point) = self.edge_point(values, i, j, entry_edge, threshold) {
                points.push(point);
            }

            // Find the exit edge within this cell
            let case = self.cell_case(values, i, j, threshold);
            let exit_edge = match Self::exit_edge(entry_edge, case) {
                Some(e) => e,
                None => break,
            };

            // Also mark the exit edge as visited (it's the same contour segment)
            let exit_idx = (j * self.width + i) * 4 + exit_edge;
            visited[exit_idx] = true;

            // Move to the adjacent cell
            match self.move_to_adjacent_cell(i, j, exit_edge) {
                Some((next_i, next_j, next_entry)) => {
                    // Check if we've returned to the start
                    if next_i == start_i && next_j == start_j && next_entry == start_edge {
                        break;
                    }
                    i = next_i;
                    j = next_j;
                    entry_edge = next_entry;
                }
                None => {
                    // Hit boundary, contour is open
                    break;
                }
            }
        }

        // Remove consecutive duplicate points (can occur when values are exactly on threshold)
        let mut deduped = Vec::with_capacity(points.len());
        for pt in points {
            if deduped.is_empty() || !points_equal(&deduped[deduped.len() - 1], &pt) {
                deduped.push(pt);
            }
        }

        // Close the ring if we have enough points
        if deduped.len() >= 3 {
            // Only add closing point if it's different from the last point
            if !points_equal(&deduped[deduped.len() - 1], &deduped[0]) {
                deduped.push(deduped[0]);
            }
            Some(ContourRing::new(deduped))
        } else {
            None
        }
    }

    /// Get the interpolated point on an edge.
    fn edge_point(&self, values: &[f64], i: usize, j: usize, edge: usize, threshold: f64) -> Option<Point> {
        let (x0, y0, x1, y1, v0, v1) = match edge {
            0 => {
                // Bottom edge (i,j) to (i+1,j)
                let v0 = values[j * self.width + i];
                let v1 = values[j * self.width + i + 1];
                (i as f64, j as f64, (i + 1) as f64, j as f64, v0, v1)
            }
            1 => {
                // Right edge (i+1,j) to (i+1,j+1)
                let v0 = values[j * self.width + i + 1];
                let v1 = values[(j + 1) * self.width + i + 1];
                ((i + 1) as f64, j as f64, (i + 1) as f64, (j + 1) as f64, v0, v1)
            }
            2 => {
                // Top edge (i+1,j+1) to (i,j+1)
                let v0 = values[(j + 1) * self.width + i + 1];
                let v1 = values[(j + 1) * self.width + i];
                ((i + 1) as f64, (j + 1) as f64, i as f64, (j + 1) as f64, v0, v1)
            }
            3 => {
                // Left edge (i,j+1) to (i,j)
                let v0 = values[(j + 1) * self.width + i];
                let v1 = values[j * self.width + i];
                (i as f64, (j + 1) as f64, i as f64, j as f64, v0, v1)
            }
            _ => return None,
        };

        if (v1 - v0).abs() < 1e-10 {
            return None;
        }

        let t = (threshold - v0) / (v1 - v0);
        if !(0.0..=1.0).contains(&t) {
            return None;
        }

        let px = x0 + t * (x1 - x0);
        let py = y0 + t * (y1 - y0);

        // Transform to output coordinates
        let x = self.x0 + (px / (self.width - 1) as f64) * (self.x1 - self.x0);
        let y = self.y0 + (py / (self.height - 1) as f64) * (self.y1 - self.y0);

        Some(Point::new(x, y))
    }

    /// Find the exit edge for a given entry edge and case.
    /// Returns the exit edge within this cell (0=bottom, 1=right, 2=top, 3=left).
    /// Returns None for cases 0 and 15 (no contour).
    fn exit_edge(entry_edge: usize, case: u8) -> Option<usize> {
        // Standard marching squares lookup table
        // Entry edge -> exit edge for each case
        // Cases 0 and 15: no contour crossing
        // Cases 5 and 10: saddle points (ambiguous, we pick one interpretation)
        //
        // Case bit layout:
        //   bit 0 (1): bottom-left corner (i, j)
        //   bit 1 (2): bottom-right corner (i+1, j)
        //   bit 2 (4): top-right corner (i+1, j+1)
        //   bit 3 (8): top-left corner (i, j+1)
        //
        // Edge layout:
        //   0: bottom (between corners 0 and 1)
        //   1: right (between corners 1 and 2)
        //   2: top (between corners 2 and 3)
        //   3: left (between corners 3 and 0)

        match case {
            0 | 15 => None, // No contour

            // Single corner cases
            1 => match entry_edge { 0 => Some(3), 3 => Some(0), _ => None },   // bottom-left above
            2 => match entry_edge { 0 => Some(1), 1 => Some(0), _ => None },   // bottom-right above
            4 => match entry_edge { 1 => Some(2), 2 => Some(1), _ => None },   // top-right above
            8 => match entry_edge { 2 => Some(3), 3 => Some(2), _ => None },   // top-left above

            // Opposite corner cases (complement of single corner)
            14 => match entry_edge { 0 => Some(3), 3 => Some(0), _ => None },  // complement of 1
            13 => match entry_edge { 0 => Some(1), 1 => Some(0), _ => None },  // complement of 2
            11 => match entry_edge { 1 => Some(2), 2 => Some(1), _ => None },  // complement of 4
            7 => match entry_edge { 2 => Some(3), 3 => Some(2), _ => None },   // complement of 8

            // Two adjacent corners cases
            3 => match entry_edge { 1 => Some(3), 3 => Some(1), _ => None },   // bottom two above
            6 => match entry_edge { 0 => Some(2), 2 => Some(0), _ => None },   // right two above
            12 => match entry_edge { 1 => Some(3), 3 => Some(1), _ => None },  // top two above
            9 => match entry_edge { 0 => Some(2), 2 => Some(0), _ => None },   // left two above

            // Saddle cases (diagonal corners) - ambiguous, pick one interpretation
            5 => match entry_edge {                                             // 0101: corners 0 and 2
                0 => Some(1), 1 => Some(0),
                2 => Some(3), 3 => Some(2),
                _ => None
            },
            10 => match entry_edge {                                            // 1010: corners 1 and 3
                0 => Some(3), 3 => Some(0),
                1 => Some(2), 2 => Some(1),
                _ => None
            },

            _ => None,
        }
    }

    /// Move to the adjacent cell when exiting through an edge.
    /// Returns (new_i, new_j, entry_edge_in_new_cell) or None if at boundary.
    fn move_to_adjacent_cell(&self, i: usize, j: usize, exit_edge: usize) -> Option<(usize, usize, usize)> {
        match exit_edge {
            0 => {
                // Exit through bottom -> enter cell below through its top
                if j > 0 {
                    Some((i, j - 1, 2))
                } else {
                    None
                }
            }
            1 => {
                // Exit through right -> enter cell to the right through its left
                if i + 1 < self.width - 1 {
                    Some((i + 1, j, 3))
                } else {
                    None
                }
            }
            2 => {
                // Exit through top -> enter cell above through its bottom
                if j + 1 < self.height - 1 {
                    Some((i, j + 1, 0))
                } else {
                    None
                }
            }
            3 => {
                // Exit through left -> enter cell to the left through its right
                if i > 0 {
                    Some((i - 1, j, 1))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// Check if two points are approximately equal.
fn points_equal(a: &Point, b: &Point) -> bool {
    const EPSILON: f64 = 1e-10;
    (a.x - b.x).abs() < EPSILON && (a.y - b.y).abs() < EPSILON
}

/// Generate contours at multiple threshold values.
///
/// # Example
///
/// ```
/// use d3rs::contour::contours;
///
/// let values = vec![
///     0.0, 0.0, 0.0,
///     0.0, 1.0, 0.0,
///     0.0, 0.0, 0.0,
/// ];
///
/// let thresholds = vec![0.25, 0.5, 0.75];
/// let result = contours(&values, 3, 3, &thresholds);
/// assert_eq!(result.len(), 3);
/// ```
pub fn contours(values: &[f64], width: usize, height: usize, thresholds: &[f64]) -> Vec<Contour> {
    ContourGenerator::new(width, height).contours(values, thresholds)
}

/// Generate a single contour at a threshold value.
///
/// # Example
///
/// ```
/// use d3rs::contour::contour;
///
/// let values = vec![
///     0.0, 0.0, 0.0,
///     0.0, 1.0, 0.0,
///     0.0, 0.0, 0.0,
/// ];
///
/// let result = contour(&values, 3, 3, 0.5);
/// assert_eq!(result.value, 0.5);
/// ```
pub fn contour(values: &[f64], width: usize, height: usize, threshold: f64) -> Contour {
    ContourGenerator::new(width, height).contour(values, threshold)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contour_generator() {
        let values = vec![
            0.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 1.0, 0.0,
            0.0, 1.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 0.0,
        ];

        let generator = ContourGenerator::new(4, 4);
        let contour = generator.contour(&values, 0.5);
        assert_eq!(contour.value, 0.5);
    }

    #[test]
    fn test_multiple_contours() {
        let values = vec![
            0.0, 0.0, 0.0,
            0.0, 1.0, 0.0,
            0.0, 0.0, 0.0,
        ];

        let thresholds = vec![0.25, 0.5, 0.75];
        let result = contours(&values, 3, 3, &thresholds);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_contour_ring_area() {
        let ring = ContourRing::new(vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
            Point::new(0.0, 0.0),
        ]);

        // Clockwise ring should have positive area
        assert!(ring.area().abs() > 0.0);
    }

    #[test]
    fn test_contour_ring_closed() {
        let ring = ContourRing::new(vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 0.0),
        ]);

        assert!(ring.is_closed());
    }
}
