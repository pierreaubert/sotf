//! # d3rs - D3.js-inspired plotting library for GPUI
//!
//! A Rust plotting library that brings D3.js concepts to GPUI using idiomatic Rust patterns.

#![recursion_limit = "512"]
//!
//! ## Features
//!
//! - **Scales**: Linear and logarithmic scales with tick generation
//! - **Axes**: Four orientations (Top, Right, Bottom, Left) with customizable formatting
//! - **Colors**: RGB/HSL with interpolation and categorical schemes
//! - **Shapes**: Bars, lines, areas, scatter plots
//! - **Grids**: Dots and lines at tick intersections
//! - **Legends**: Configurable position and formatting
//!
//! ## Example
//!
//! ```rust,no_run
//! use d3rs::scale::LinearScale;
//!
//! let scale = LinearScale::new()
//!     .domain(0.0, 100.0)
//!     .range(0.0, 500.0);
//!
//! let output = scale.scale(50.0); // 250.0
//! ```

pub mod scale;
pub mod color;
pub mod axis;
pub mod shape;
pub mod grid;
pub mod legend;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::scale::{LinearScale, LogScale, Scale};
    pub use crate::color::{D3Color, ColorScheme};
    pub use crate::axis::{AxisConfig, AxisOrientation, AxisTheme, DefaultAxisTheme, render_axis};
    pub use crate::grid::{GridConfig, render_grid};
    pub use crate::shape::{
        BarConfig, BarDatum, render_bars,
        LineConfig, LinePoint, CurveType, render_line,
        ScatterConfig, ScatterPoint, render_scatter,
    };
}
