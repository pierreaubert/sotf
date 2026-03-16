//! Delaunay triangulation and Voronoi diagram.
//!
//! Faithful port of [d3-delaunay](https://github.com/d3/d3-delaunay) to Rust,
//! using [delaunator](https://crates.io/crates/delaunator) as the triangulation backend.
//!
//! # Example
//! ```
//! use math_delaunay::Delaunay;
//!
//! let points = vec![(0.0, 0.0), (1.0, 0.0), (0.5, 1.0)];
//! let d = Delaunay::from_points(&points);
//! let v = d.voronoi([0.0, 0.0, 2.0, 2.0]);
//! if let Some(cell) = v.cell_polygon(0) {
//!     println!("Cell 0 has {} vertices", cell.len());
//! }
//! ```

mod delaunay;
mod voronoi;

pub use delaunay::Delaunay;
pub use voronoi::Voronoi;
