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

                // Try to trace contours starting from this cell
                for edge in 0..4 {
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
        let mut edge = start_edge;

        loop {
            let idx = (j * self.width + i) * 4 + edge;
            if visited[idx] {
                break;
            }
            visited[idx] = true;

            // Get the interpolated point on this edge
            if let Some(point) = self.edge_point(values, i, j, edge, threshold) {
                points.push(point);
            }

            // Find the next edge
            let case = self.cell_case(values, i, j, threshold);
            if let Some((next_i, next_j, next_edge)) = self.next_edge(i, j, edge, case) {
                if next_i >= self.width - 1 || next_j >= self.height - 1 {
                    break;
                }
                i = next_i;
                j = next_j;
                edge = next_edge;

                // Check if we've returned to the start
                if i == start_i && j == start_j && edge == start_edge {
                    break;
                }
            } else {
                break;
            }
        }

        // Close the ring if we have enough points
        if points.len() >= 3 {
            points.push(points[0]);
            Some(ContourRing::new(points))
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
        if t < 0.0 || t > 1.0 {
            return None;
        }

        let px = x0 + t * (x1 - x0);
        let py = y0 + t * (y1 - y0);

        // Transform to output coordinates
        let x = self.x0 + (px / (self.width - 1) as f64) * (self.x1 - self.x0);
        let y = self.y0 + (py / (self.height - 1) as f64) * (self.y1 - self.y0);

        Some(Point::new(x, y))
    }

    /// Find the next edge to traverse.
    fn next_edge(&self, i: usize, j: usize, edge: usize, case: u8) -> Option<(usize, usize, usize)> {
        // Marching squares case table
        // Each case determines which edges are connected
        match (edge, case) {
            // From bottom edge (0)
            (0, 1) | (0, 14) => Some((i, j, 3)),
            (0, 2) | (0, 13) => Some((i, j, 1)),
            (0, 3) | (0, 12) => Some((i, j, 1)),
            (0, 4) | (0, 11) => Some((i, j, 1)),
            (0, 6) | (0, 9) => Some((i, j, 1)),
            (0, 7) | (0, 8) => Some((i, j, 3)),
            (0, 5) => Some((i, j, 1)), // Saddle
            (0, 10) => Some((i, j, 3)), // Saddle

            // From right edge (1)
            (1, 1) | (1, 14) => Some((i + 1, j, 0)),
            (1, 2) | (1, 13) => Some((i, j, 2)),
            (1, 3) | (1, 12) => Some((i + 1, j, 0)),
            (1, 4) | (1, 11) => Some((i, j, 2)),
            (1, 6) | (1, 9) => Some((i, j, 2)),
            (1, 7) | (1, 8) => Some((i, j, 2)),
            (1, 5) => Some((i, j, 2)), // Saddle
            (1, 10) => Some((i + 1, j, 0)), // Saddle

            // From top edge (2)
            (2, 1) | (2, 14) => Some((i, j + 1, 1)),
            (2, 2) | (2, 13) => Some((i, j + 1, 1)),
            (2, 3) | (2, 12) => Some((i, j + 1, 1)),
            (2, 4) | (2, 11) => Some((i, j, 3)),
            (2, 6) | (2, 9) => Some((i, j, 3)),
            (2, 7) | (2, 8) => Some((i, j, 3)),
            (2, 5) => Some((i, j, 3)), // Saddle
            (2, 10) => Some((i, j + 1, 1)), // Saddle

            // From left edge (3)
            (3, 1) | (3, 14) => Some((i, j, 0)),
            (3, 2) | (3, 13) => Some((i, j, 0)),
            (3, 3) | (3, 12) => Some((i, j, 0)),
            (3, 4) | (3, 11) => {
                if i > 0 { Some((i - 1, j, 2)) } else { None }
            }
            (3, 6) | (3, 9) => Some((i, j, 0)),
            (3, 7) | (3, 8) => {
                if i > 0 { Some((i - 1, j, 2)) } else { None }
            }
            (3, 5) => Some((i, j, 0)), // Saddle
            (3, 10) => {
                if i > 0 { Some((i - 1, j, 2)) } else { None }
            } // Saddle

            _ => None,
        }
    }
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
