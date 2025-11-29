//! Contour generation module
//!
//! This module provides functions for generating contour lines and density estimation,
//! inspired by d3-contour.
//!
//! # Features
//!
//! - **Marching Squares**: Generate contour polygons from a 2D grid
//! - **Density Estimation**: Kernel density estimation for 2D point clouds
//! - **Threshold Generation**: Automatic threshold calculation
//!
//! # Example
//!
//! ```rust
//! use d3rs::contour::{contours, ContourGenerator};
//!
//! // Create a simple 3x3 grid
//! let values = vec![
//!     0.0, 0.0, 0.0,
//!     0.0, 1.0, 0.0,
//!     0.0, 0.0, 0.0,
//! ];
//!
//! let contour_gen = ContourGenerator::new(3, 3);
//! let result = contour_gen.contour(&values, 0.5);
//! ```

mod marching_squares;
mod density;
mod thresholds;

pub use marching_squares::{ContourGenerator, Contour, ContourRing, contours, contour};
pub use density::{DensityEstimator, density_2d, gaussian_kernel};
pub use thresholds::{threshold_sturges, threshold_scott, threshold_freedman_diaconis};
