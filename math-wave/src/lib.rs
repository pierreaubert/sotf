//! Analytical solutions for wave and Helmholtz equations
//!
//! This crate provides exact analytical solutions to wave propagation
//! and scattering problems, useful for validating numerical solvers (BEM, FEM).
//!
//! # Features
//!
//! - **1D solutions**: Plane waves, standing waves, damped waves
//! - **2D solutions**: Cylinder scattering (Bessel/Hankel series)
//! - **3D solutions**: Sphere scattering (Mie theory)
//! - **Special functions**: Spherical Bessel/Hankel, Legendre polynomials
//! - **Green's functions**: Helmholtz kernel and derivatives
//!
//! # Example
//!
//! ```rust
//! use math_wave::analytical::{plane_wave_1d, sphere_scattering_3d};
//! use std::f64::consts::PI;
//!
//! // 1D plane wave
//! let wave = plane_wave_1d(1.0, 0.0, 2.0 * PI, 100);
//! assert_eq!(wave.pressure.len(), 100);
//!
//! // 3D sphere scattering
//! let scatter = sphere_scattering_3d(1.0, 1.0, 20, vec![2.0], vec![0.0, PI/2.0]);
//! assert!(scatter.pressure[0].norm() > 0.0);
//! ```

pub mod analytical;
pub mod special;

// Re-export main types at crate root
pub use analytical::{AnalyticalSolution, Point};
