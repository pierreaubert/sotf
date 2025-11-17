//! N-Dimensional Convex Hull and Computational Geometry Library
//!
//! This library implements the Quickhull algorithm for computing convex hulls
//! in arbitrary dimensions, as well as Delaunay triangulation.
//!
//! Based on the C implementation by Leo McCormack and the MATLAB Computational
//! Geometry Toolbox.
//!
//! # 3D Convex Hull Example
//! ```
//! use convexhull3d::{ConvexHull3D, Vertex};
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
//!
//! # N-D Convex Hull Example
//! ```
//! use convexhull3d::{PointND, ConvexHullND};
//!
//! let points = vec![
//!     PointND::new(vec![0.0, 0.0]),
//!     PointND::new(vec![1.0, 0.0]),
//!     PointND::new(vec![1.0, 1.0]),
//!     PointND::new(vec![0.0, 1.0]),
//! ];
//!
//! let hull = ConvexHullND::build(&points).unwrap();
//! println!("Number of facets: {}", hull.num_facets());
//! ```
//!
//! # Delaunay Triangulation Example
//! ```
//! use convexhull3d::{PointND, DelaunayMesh};
//!
//! let points = vec![
//!     PointND::new(vec![0.0, 0.0]),
//!     PointND::new(vec![1.0, 0.0]),
//!     PointND::new(vec![0.0, 1.0]),
//!     PointND::new(vec![0.5, 0.5]),
//! ];
//!
//! let mesh = DelaunayMesh::build(&points).unwrap();
//! println!("Number of simplices: {}", mesh.num_simplices());
//! ```

mod types;
mod quickhull;
mod geometry;
mod export;
mod nd_types;
mod quickhull_nd;
mod delaunay;

// Make testdata publicly available for tests
pub mod testdata;

// 3D types and functions
pub use types::{Vertex, Face, ConvexHull3D};
pub use export::{export_obj, export_html};

// N-D types and functions
pub use nd_types::{PointND, SimplexND, ConvexHullND, DelaunayMesh};
pub use delaunay::{delaunay_nd, circumcenter};

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
