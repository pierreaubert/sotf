//! Shape rendering module
//!
//! This module provides functions for rendering common chart shapes like bars, lines, and scatter plots.

mod bar;
mod line;
mod scatter;

pub use bar::{BarConfig, BarDatum, render_bars};
pub use line::{LineConfig, LinePoint, CurveType, render_line};
pub use scatter::{ScatterConfig, ScatterPoint, render_scatter};
