//! Analytical solutions for wave equation validation
//!
//! This module provides exact solutions to acoustic scattering problems
//! used to validate numerical methods (BEM, FEM).
//!
//! ## Available Solutions
//!
//! - **1D**: Plane wave, standing wave, damped wave
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
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
    /// θ = polar angle from z-axis
    /// φ = azimuthal angle in xy-plane
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

    /// Polar angle (2D) - angle from positive x-axis
    pub fn theta_2d(&self) -> f64 {
        self.y.atan2(self.x)
    }

    /// Spherical polar angle (3D, from z-axis)
    pub fn theta_3d(&self) -> f64 {
        let r = self.radius();
        if r < 1e-15 { 0.0 } else { (self.z / r).acos() }
    }

    /// Spherical azimuthal angle (3D)
    pub fn phi_3d(&self) -> f64 {
        self.y.atan2(self.x)
    }

    /// Distance to another point
    pub fn distance_to(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

impl Default for Point {
    fn default() -> Self {
        Self::new_3d(0.0, 0.0, 0.0)
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
    /// Wave number k = ω/c = 2πf/c
    pub wave_number: f64,
    /// Frequency (Hz)
    pub frequency: f64,
    /// Additional metadata
    pub metadata: serde_json::Value,
}

impl AnalyticalSolution {
    /// Create a new analytical solution
    pub fn new(
        name: impl Into<String>,
        dimensions: usize,
        positions: Vec<Point>,
        pressure: Vec<Complex64>,
        wave_number: f64,
        frequency: f64,
    ) -> Self {
        Self {
            name: name.into(),
            dimensions,
            positions,
            pressure,
            wave_number,
            frequency,
            metadata: serde_json::json!({}),
        }
    }

    /// Compute pressure magnitude |p|
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

    /// Compute L2 error compared to another solution
    pub fn l2_error(&self, other: &AnalyticalSolution) -> f64 {
        assert_eq!(self.pressure.len(), other.pressure.len());

        let sum_sq: f64 = self
            .pressure
            .iter()
            .zip(other.pressure.iter())
            .map(|(a, b)| (a - b).norm_sqr())
            .sum();

        sum_sq.sqrt()
    }

    /// Compute relative L2 error
    pub fn relative_l2_error(&self, other: &AnalyticalSolution) -> f64 {
        let l2_err = self.l2_error(other);
        let norm: f64 = other
            .pressure
            .iter()
            .map(|p| p.norm_sqr())
            .sum::<f64>()
            .sqrt();

        if norm < 1e-15 { l2_err } else { l2_err / norm }
    }

    /// Compute max (L∞) error
    pub fn linf_error(&self, other: &AnalyticalSolution) -> f64 {
        assert_eq!(self.pressure.len(), other.pressure.len());

        self.pressure
            .iter()
            .zip(other.pressure.iter())
            .map(|(a, b)| (a - b).norm())
            .fold(0.0, f64::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_point_creation() {
        let p1d = Point::new_1d(1.0);
        assert_eq!(p1d.x, 1.0);
        assert_eq!(p1d.y, 0.0);
        assert_eq!(p1d.z, 0.0);

        let p2d = Point::from_polar(1.0, PI / 4.0);
        assert!((p2d.radius() - 1.0).abs() < 1e-10);

        let p3d = Point::from_spherical(1.0, PI / 2.0, 0.0);
        assert!((p3d.radius() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_point_distance() {
        let p1 = Point::new_3d(1.0, 0.0, 0.0);
        let p2 = Point::new_3d(4.0, 4.0, 0.0);
        assert!((p1.distance_to(&p2) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_solution_error() {
        let sol1 = AnalyticalSolution::new(
            "test1",
            1,
            vec![Point::new_1d(0.0)],
            vec![Complex64::new(1.0, 0.0)],
            1.0,
            1.0,
        );

        let sol2 = AnalyticalSolution::new(
            "test2",
            1,
            vec![Point::new_1d(0.0)],
            vec![Complex64::new(1.1, 0.0)],
            1.0,
            1.0,
        );

        assert!((sol1.l2_error(&sol2) - 0.1).abs() < 1e-10);
    }
}
