//! # d3rs - D3.js-inspired plotting library for GPUI
//!
//! A Rust plotting library that brings D3.js concepts to GPUI using idiomatic Rust patterns.
//!
//! ## Features
//!
//! - **Scales**: Linear and logarithmic scales with tick generation
//! - **Axes**: Four orientations (Top, Right, Bottom, Left) with customizable formatting
//! - **Colors**: RGB/HSL with interpolation and categorical schemes
//! - **Shapes**: Bars, lines, areas, scatter plots, arcs, pies, symbols, stacks
//! - **Curves**: Linear, step, basis, cardinal, catmull-rom, monotone, natural
//! - **Grids**: Dots and lines at tick intersections
//! - **Legends**: Configurable position and formatting
//! - **Arrays**: Statistics, search, binning, transformations (d3-array)
//! - **Interpolation**: Numeric, color (HSL/LAB/HCL/Cubehelix), transform, string, zoom (d3-interpolate)
//! - **Contours**: Marching squares, density estimation (d3-contour)
//! - **Fetch**: CSV/TSV/JSON parsing utilities (d3-fetch)
//!
//! ## Example
//!
//! ```rust,no_run
//! use d3rs::scale::{LinearScale, Scale};
//!
//! let scale = LinearScale::new()
//!     .domain(0.0, 100.0)
//!     .range(0.0, 500.0);
//!
//! let output = scale.scale(50.0); // 250.0
//! ```

#![cfg_attr(feature = "gpui", recursion_limit = "512")]

pub mod array;
pub mod interpolate;
pub mod scale;
pub mod color;
#[cfg(feature = "gpui")]
pub mod axis;
pub mod shape;
#[cfg(feature = "gpui")]
pub mod grid;
pub mod legend;
pub mod contour;
pub mod fetch;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::scale::{LinearScale, LogScale, Scale};
    pub use crate::color::{D3Color, ColorScheme};
    #[cfg(feature = "gpui")]
    pub use crate::axis::{AxisConfig, AxisOrientation, AxisTheme, DefaultAxisTheme, render_axis};
    #[cfg(feature = "gpui")]
    pub use crate::grid::{GridConfig, render_grid};
    #[cfg(feature = "gpui")]
    pub use crate::shape::{
        BarConfig, BarDatum, render_bars,
        LineConfig, LinePoint, CurveType, render_line,
        ScatterConfig, ScatterPoint, render_scatter,
    };
}
