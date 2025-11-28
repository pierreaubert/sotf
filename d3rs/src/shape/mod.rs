//! Shape rendering module
//!
//! This module provides functions for rendering common chart shapes like bars, lines,
//! scatter plots, arcs, pies, areas, and more.
//!
//! # Submodules
//!
//! - `path`: SVG-like path building utilities
//! - `arc`: Arc generator for pie and donut charts
//! - `pie`: Pie layout generator
//! - `area`: Area shape generator
//! - `curve`: Curve interpolation algorithms
//! - `symbol`: Symbol generators for data markers
//! - `stack`: Stack layout for stacked charts
//! - `bar`: Bar chart rendering
//! - `line`: Line chart rendering
//! - `scatter`: Scatter plot rendering
//!
//! # Example
//!
//! ```rust
//! use d3rs::shape::path::PathBuilder;
//! use d3rs::shape::pie::Pie;
//! use d3rs::shape::symbol::{Symbol, SymbolType};
//!
//! // Create a custom path
//! let path = PathBuilder::new()
//!     .move_to(0.0, 0.0)
//!     .line_to(100.0, 0.0)
//!     .line_to(100.0, 100.0)
//!     .close_path()
//!     .build();
//!
//! // Create pie slices
//! let values = vec![10.0, 20.0, 30.0, 40.0];
//! let slices = Pie::new().generate(&values, |v| *v);
//!
//! // Create a symbol
//! let star = Symbol::star(64.0);
//! let star_path = star.generate();
//! ```

pub mod path;
pub mod arc;
pub mod pie;
pub mod area;
pub mod curve;
pub mod symbol;
pub mod stack;

mod bar;
mod line;
mod scatter;

// Re-export existing chart rendering functions
pub use bar::{BarConfig, BarDatum, render_bars};
pub use line::{LineConfig, LinePoint, CurveType, render_line};
pub use scatter::{ScatterConfig, ScatterPoint, render_scatter};

// Re-export new shape utilities
pub use path::{Path, PathBuilder, PathCommand, Point};
pub use arc::{Arc, ArcDatum, arc_points};
pub use pie::{Pie, PieSlice, pie, donut, half_pie};
pub use area::{Area, SimpleArea, area_points};
pub use curve::Curve;
pub use symbol::{Symbol, SymbolType, symbol_radius};
pub use stack::{Stack, StackSeries, StackOrder, StackOffset, stack, stack_expand, streamgraph};
