//! Analytical solutions for BEM validation
//!
//! This module provides exact solutions to acoustic scattering problems
//! used to validate BEM implementations.
//!
//! ## Available Solutions
//!
//! - **1D**: Plane wave propagation
//! - **2D**: Cylinder scattering (Bessel/Hankel series)
//! - **3D**: Sphere scattering (Mie theory)

use num_complex::Complex64;
use serde::{Deserialize, Serialize};

pub mod solutions_1d;
pub mod solutions_2d;
pub mod solutions_3d;

pub use solutions_1d::*;
pub use solutions_2d::*;
pub use solutions_3d::*;

/// Point in space (1D, 2D, or 3D)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point {
    /// x-coordinate
    pub x: f64,
    /// y-coordinate (0 for 1D)
    pub y: f64,
    /// z-coordinate (0 for 1D/2D)
    pub z: f64,
}

impl Point {
    /// Create 1D point
    pub fn new_1d(x: f64) -> Self {
        Self { x, y: 0.0, z: 0.0 }
    }

    /// Create 2D point
    pub fn new_2d(x: f64, y: f64) -> Self {
        Self { x, y, z: 0.0 }
    }

    /// Create 3D point
    pub fn new_3d(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Polar coordinates (r, θ) for 2D
    pub fn from_polar(r: f64, theta: f64) -> Self {
        Self::new_2d(r * theta.cos(), r * theta.sin())
    }

    /// Spherical coordinates (r, θ, φ) for 3D
    pub fn from_spherical(r: f64, theta: f64, phi: f64) -> Self {
        Self::new_3d(
            r * theta.sin() * phi.cos(),
            r * theta.sin() * phi.sin(),
            r * theta.cos(),
        )
    }

    /// Distance from origin
    pub fn radius(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    /// Polar angle (2D)
    pub fn theta_2d(&self) -> f64 {
        self.y.atan2(self.x)
    }

    /// Spherical polar angle (3D, from z-axis)
    pub fn theta_3d(&self) -> f64 {
        (self.z / self.radius()).acos()
    }

    /// Spherical azimuthal angle (3D)
    pub fn phi_3d(&self) -> f64 {
        self.y.atan2(self.x)
    }
}

/// Analytical solution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticalSolution {
    /// Test name
    pub name: String,
    /// Dimensionality (1, 2, or 3)
    pub dimensions: usize,
    /// Evaluation points
    pub positions: Vec<Point>,
    /// Complex pressure values
    pub pressure: Vec<Complex64>,
    /// Wave number
    pub wave_number: f64,
    /// Frequency (Hz)
    pub frequency: f64,
    /// Additional metadata
    pub metadata: serde_json::Value,
}

impl AnalyticalSolution {
    /// Compute pressure magnitude
    pub fn magnitude(&self) -> Vec<f64> {
        self.pressure.iter().map(|p| p.norm()).collect()
    }

    /// Compute pressure phase (radians)
    pub fn phase(&self) -> Vec<f64> {
        self.pressure.iter().map(|p| p.arg()).collect()
    }

    /// Real part of pressure
    pub fn real(&self) -> Vec<f64> {
        self.pressure.iter().map(|p| p.re).collect()
    }

    /// Imaginary part of pressure
    pub fn imag(&self) -> Vec<f64> {
        self.pressure.iter().map(|p| p.im).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_creation() {
        let p1d = Point::new_1d(1.0);
        assert_eq!(p1d.x, 1.0);
        assert_eq!(p1d.y, 0.0);
        assert_eq!(p1d.z, 0.0);

        let p2d = Point::from_polar(1.0, std::f64::consts::PI / 4.0);
        assert!((p2d.radius() - 1.0).abs() < 1e-10);

        let p3d = Point::from_spherical(1.0, std::f64::consts::PI / 2.0, 0.0);
        assert!((p3d.radius() - 1.0).abs() < 1e-10);
    }
}
