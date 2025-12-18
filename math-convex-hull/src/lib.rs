//! 3D Convex Hull and Computational Geometry Library
//!
//! This library implements the Quickhull algorithm for computing convex hulls
//! in 3D space.
//!
//! Based on the C implementation by Leo McCormack and the MATLAB Computational
//! Geometry Toolbox.
//!
//! # 3D Convex Hull Example
//! ```
//! use math_convex_hull::{ConvexHull3D, Vertex};
//!
//! let vertices = vec![
//!     Vertex::new(0.0, 0.0, 0.0),
//!     Vertex::new(1.0, 0.0, 0.0),
//!     Vertex::new(0.0, 1.0, 0.0),
//!     Vertex::new(0.0, 0.0, 1.0),
//! ];
//!
//! let hull = ConvexHull3D::build(&vertices).unwrap();
//! println!("Number of faces: {}", hull.num_faces());
//! ```

mod export;
mod geometry;
mod quickhull;
mod types;

// Make testdata publicly available for tests
pub mod testdata;

// 3D types and functions
pub use export::{export_html, export_obj};
pub use types::{ConvexHull3D, Face, Vertex};

/// Error types for convex hull operations
#[derive(Debug, thiserror::Error)]
pub enum ConvexHullError {
    #[error("Not enough vertices to form a hull (minimum 4 required)")]
    InsufficientVertices,

    #[error("Vertices are coplanar or collinear")]
    DegenerateConfiguration,

    #[error("Maximum iterations exceeded")]
    MaxIterationsExceeded,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid face: {0}")]
    InvalidFace(String),
}

pub type Result<T> = std::result::Result<T, ConvexHullError>;

/// Numerical tolerance for floating-point comparisons
/// Used throughout the library for:
/// - Distance calculations
/// - Determinant checks
/// - Degeneracy detection
pub(crate) const EPSILON: f64 = 1e-10;
