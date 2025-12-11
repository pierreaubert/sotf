//! Basic types for room acoustics simulations

use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// 3D point in space
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Point3D {
    /// X coordinate
    pub x: f64,
    /// Y coordinate
    pub y: f64,
    /// Z coordinate
    pub z: f64,
}

impl Point3D {
    /// Create a new 3D point
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Create a zero point (origin)
    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    /// Calculate Euclidean distance to another point
    pub fn distance_to(&self, other: &Point3D) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Convert to spherical coordinates (r, theta, phi)
    /// theta: polar angle (0 to PI)
    /// phi: azimuthal angle (-PI to PI)
    pub fn to_spherical(&self) -> (f64, f64, f64) {
        let r = (self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        let theta = if r > 1e-10 { (self.z / r).acos() } else { 0.0 };
        let phi = self.y.atan2(self.x);
        (r, theta, phi)
    }

    /// Create from spherical coordinates
    pub fn from_spherical(r: f64, theta: f64, phi: f64) -> Self {
        Self {
            x: r * theta.sin() * phi.cos(),
            y: r * theta.sin() * phi.sin(),
            z: r * theta.cos(),
        }
    }

    /// Compute dot product with another point (treating as vectors)
    pub fn dot(&self, other: &Point3D) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Compute cross product with another point (treating as vectors)
    pub fn cross(&self, other: &Point3D) -> Point3D {
        Point3D {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }

    /// Compute the length (magnitude) of the vector
    pub fn length(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    /// Normalize the vector to unit length
    pub fn normalize(&self) -> Option<Point3D> {
        let len = self.length();
        if len > 1e-10 {
            Some(Point3D {
                x: self.x / len,
                y: self.y / len,
                z: self.z / len,
            })
        } else {
            None
        }
    }

    /// Scale the vector by a scalar
    pub fn scale(&self, s: f64) -> Point3D {
        Point3D {
            x: self.x * s,
            y: self.y * s,
            z: self.z * s,
        }
    }

    /// Add another point/vector
    pub fn add(&self, other: &Point3D) -> Point3D {
        Point3D {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }

    /// Subtract another point/vector
    pub fn sub(&self, other: &Point3D) -> Point3D {
        Point3D {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }
}

impl std::ops::Add for Point3D {
    type Output = Point3D;
    fn add(self, other: Point3D) -> Point3D {
        Point3D {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}

impl std::ops::Sub for Point3D {
    type Output = Point3D;
    fn sub(self, other: Point3D) -> Point3D {
        Point3D {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }
}

impl std::ops::Mul<f64> for Point3D {
    type Output = Point3D;
    fn mul(self, s: f64) -> Point3D {
        Point3D {
            x: self.x * s,
            y: self.y * s,
            z: self.z * s,
        }
    }
}

/// Listening position (alias for Point3D)
pub type ListeningPosition = Point3D;

/// Surface element (triangular or quadrilateral)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfaceElement {
    /// Node indices that form this element
    pub nodes: Vec<usize>,
}

impl SurfaceElement {
    /// Create a triangular element
    pub fn triangle(n0: usize, n1: usize, n2: usize) -> Self {
        Self {
            nodes: vec![n0, n1, n2],
        }
    }

    /// Create a quadrilateral element
    pub fn quad(n0: usize, n1: usize, n2: usize, n3: usize) -> Self {
        Self {
            nodes: vec![n0, n1, n2, n3],
        }
    }

    /// Check if this is a triangular element
    pub fn is_triangle(&self) -> bool {
        self.nodes.len() == 3
    }

    /// Check if this is a quadrilateral element
    pub fn is_quad(&self) -> bool {
        self.nodes.len() == 4
    }
}

/// Room mesh for BEM/FEM (surface mesh for BEM, volume mesh for FEM)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMesh {
    /// Node positions
    pub nodes: Vec<Point3D>,
    /// Surface elements
    pub elements: Vec<SurfaceElement>,
}

impl RoomMesh {
    /// Create an empty mesh
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            elements: Vec::new(),
        }
    }

    /// Get the number of nodes
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Get the number of elements
    pub fn num_elements(&self) -> usize {
        self.elements.len()
    }

    /// Compute the centroid of an element
    pub fn element_centroid(&self, element_idx: usize) -> Point3D {
        let element = &self.elements[element_idx];
        let mut centroid = Point3D::zero();
        for &node_idx in &element.nodes {
            centroid = centroid + self.nodes[node_idx];
        }
        centroid * (1.0 / element.nodes.len() as f64)
    }

    /// Compute the normal of a triangular element
    pub fn element_normal(&self, element_idx: usize) -> Option<Point3D> {
        let element = &self.elements[element_idx];
        if element.nodes.len() < 3 {
            return None;
        }

        let p0 = self.nodes[element.nodes[0]];
        let p1 = self.nodes[element.nodes[1]];
        let p2 = self.nodes[element.nodes[2]];

        let v1 = p1 - p0;
        let v2 = p2 - p0;
        v1.cross(&v2).normalize()
    }

    /// Compute the area of a triangular element
    pub fn element_area(&self, element_idx: usize) -> f64 {
        let element = &self.elements[element_idx];
        if element.nodes.len() < 3 {
            return 0.0;
        }

        let p0 = self.nodes[element.nodes[0]];
        let p1 = self.nodes[element.nodes[1]];
        let p2 = self.nodes[element.nodes[2]];

        let v1 = p1 - p0;
        let v2 = p2 - p0;
        v1.cross(&v2).length() * 0.5
    }
}

impl Default for RoomMesh {
    fn default() -> Self {
        Self::new()
    }
}

/// Constants for room acoustics
pub mod constants {
    /// Speed of sound at 20°C in m/s
    pub const SPEED_OF_SOUND_20C: f64 = 343.0;

    /// Reference pressure for SPL calculation (20 μPa)
    pub const REFERENCE_PRESSURE: f64 = 20e-6;

    /// Air density at 20°C in kg/m³
    pub const AIR_DENSITY_20C: f64 = 1.204;
}

/// Calculate wavenumber k = 2πf/c
pub fn wavenumber(frequency: f64, speed_of_sound: f64) -> f64 {
    2.0 * PI * frequency / speed_of_sound
}

/// Convert complex pressure to SPL in dB
pub fn pressure_to_spl(pressure: num_complex::Complex64) -> f64 {
    let magnitude = pressure.norm();
    if magnitude > 1e-20 {
        20.0 * (magnitude / constants::REFERENCE_PRESSURE).log10()
    } else {
        -120.0 // Very low SPL
    }
}

/// Generate logarithmically spaced frequencies
pub fn log_space(start: f64, end: f64, num: usize) -> Vec<f64> {
    if num < 2 {
        return vec![start];
    }
    let log_start = start.ln();
    let log_end = end.ln();
    (0..num)
        .map(|i| {
            let log_val = log_start + (log_end - log_start) * i as f64 / (num - 1) as f64;
            log_val.exp()
        })
        .collect()
}

/// Generate linearly spaced frequencies
pub fn lin_space(start: f64, end: f64, num: usize) -> Vec<f64> {
    if num < 2 {
        return vec![start];
    }
    (0..num)
        .map(|i| start + (end - start) * i as f64 / (num - 1) as f64)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_point_distance() {
        let p1 = Point3D::new(0.0, 0.0, 0.0);
        let p2 = Point3D::new(3.0, 4.0, 0.0);
        assert_relative_eq!(p1.distance_to(&p2), 5.0, epsilon = 1e-10);
    }

    #[test]
    fn test_point_spherical() {
        let p = Point3D::new(1.0, 0.0, 0.0);
        let (r, theta, phi) = p.to_spherical();
        assert_relative_eq!(r, 1.0, epsilon = 1e-10);
        assert_relative_eq!(theta, std::f64::consts::FRAC_PI_2, epsilon = 1e-10);
        assert_relative_eq!(phi, 0.0, epsilon = 1e-10);
    }

    #[test]
    fn test_log_space() {
        let freqs = log_space(20.0, 20000.0, 200);
        assert_eq!(freqs.len(), 200);
        assert_relative_eq!(freqs[0], 20.0, epsilon = 1e-6);
        assert_relative_eq!(freqs[199], 20000.0, epsilon = 1e-6);
        // Check logarithmic spacing
        assert!(freqs[1] / freqs[0] > 1.0);
    }

    #[test]
    fn test_wavenumber() {
        let k = wavenumber(1000.0, 343.0);
        assert_relative_eq!(
            k,
            2.0 * std::f64::consts::PI * 1000.0 / 343.0,
            epsilon = 1e-10
        );
    }
}
