//! # gpui-px - High-level charting API for GPUI
//!
//! Plotly Express-style API built on top of d3rs primitives.
//!
//! ## Example
//!
//! ```rust,no_run
//! use gpui_px::{scatter, line, bar};
//!
//! // Scatter plot in 3 lines
//! let chart = scatter(&x_data, &y_data)
//!     .title("My Chart")
//!     .build()?;
//!
//! // Line chart
//! let chart = line(&x_data, &y_data)
//!     .color(0x1f77b4)
//!     .build()?;
//!
//! // Bar chart
//! let chart = bar(&categories, &values)
//!     .build()?;
//! ```

mod bar;
mod error;
mod line;
mod scatter;

pub use bar::{bar, BarChart};
pub use error::ChartError;
pub use line::{line, LineChart};
pub use scatter::{scatter, ScatterChart};

// Re-export d3rs types users might need
pub use d3rs::color::D3Color;
pub use d3rs::shape::CurveType;

// ============================================================================
// Shared Constants
// ============================================================================

/// Default chart color (Plotly blue)
pub(crate) const DEFAULT_COLOR: u32 = 0x1f77b4;

/// Default chart width in pixels
pub(crate) const DEFAULT_WIDTH: f32 = 600.0;

/// Default chart height in pixels
pub(crate) const DEFAULT_HEIGHT: f32 = 400.0;

/// Default padding fraction for auto-domain calculation
pub(crate) const DEFAULT_PADDING_FRACTION: f64 = 0.05;

/// Default title font size
pub(crate) const DEFAULT_TITLE_FONT_SIZE: f32 = 16.0;

/// Title area height (font size + padding)
pub(crate) const TITLE_AREA_HEIGHT: f32 = 24.0;

// ============================================================================
// Shared Utilities
// ============================================================================

/// Calculate extent (min, max) with padding.
///
/// Returns `(min - padding, max + padding)` where padding is calculated
/// as `range * padding_fraction`.
pub(crate) fn extent_padded(values: &[f64], padding_fraction: f64) -> (f64, f64) {
    let (min, max) = values
        .iter()
        .copied()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), val| {
            (min.min(val), max.max(val))
        });

    let range = max - min;
    let padding = if range.abs() < f64::EPSILON {
        1.0 // Default padding for constant values
    } else {
        range * padding_fraction
    };
    (min - padding, max + padding)
}

/// Validate that a data array is not empty and contains only finite values.
pub(crate) fn validate_data_array(values: &[f64], field: &'static str) -> Result<(), ChartError> {
    if values.is_empty() {
        return Err(ChartError::EmptyData { field });
    }
    if values.iter().any(|x| !x.is_finite()) {
        return Err(ChartError::InvalidData {
            field,
            reason: "contains NaN or Infinity",
        });
    }
    Ok(())
}

/// Validate that two arrays have the same length.
pub(crate) fn validate_data_length(
    x_len: usize,
    y_len: usize,
    x_field: &'static str,
    y_field: &'static str,
) -> Result<(), ChartError> {
    if x_len != y_len {
        return Err(ChartError::DataLengthMismatch {
            x_field,
            y_field,
            x_len,
            y_len,
        });
    }
    Ok(())
}

/// Validate chart dimensions are positive.
pub(crate) fn validate_dimensions(width: f32, height: f32) -> Result<(), ChartError> {
    if width <= 0.0 {
        return Err(ChartError::InvalidDimension {
            field: "width",
            value: width,
        });
    }
    if height <= 0.0 {
        return Err(ChartError::InvalidDimension {
            field: "height",
            value: height,
        });
    }
    Ok(())
}
