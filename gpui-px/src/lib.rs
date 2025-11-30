#![recursion_limit = "512"]

//! # gpui-px - High-level charting API for GPUI
//!
//! Plotly Express-style API built on top of d3rs primitives.
//!
//! ## Chart Types
//!
//! ### Scatter Charts
//! Use [`scatter()`] for:
//! - Displaying individual data points with x,y coordinates
//! - Exploring correlations between two continuous variables
//! - Identifying outliers or clusters in data
//! - Showing distributions in 2D space
//!
//! ### Line Charts
//! Use [`line()`] for:
//! - Time series or sequential data
//! - Showing trends over continuous domains
//! - Connecting related data points with smooth or linear interpolation
//! - Comparing multiple series over the same range
//!
//! ### Bar Charts
//! Use [`bar()`] for:
//! - Categorical data with discrete categories
//! - Comparing values across different groups
//! - Displaying counts or aggregated metrics
//! - Visualizing rankings or distributions by category
//!
//! ## Coordinate System
//!
//! All charts use standard mathematical coordinates:
//! - **Y-axis**: 0 at bottom, increases upward
//! - **X-axis**: 0 at left, increases rightward
//!
//! ## Color Format
//!
//! All color parameters accept 24-bit RGB hex values in format `0xRRGGBB`:
//! - `0x1f77b4` - Plotly blue (default)
//! - `0xff7f0e` - Plotly orange
//! - `0x2ca02c` - Plotly green
//! - `0xd62728` - Plotly red
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
//! // Line chart with custom color
//! let chart = line(&x_data, &y_data)
//!     .color(0x1f77b4)  // Plotly blue
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

pub use bar::{BarChart, bar};
pub use error::ChartError;
pub use line::{LineChart, line};
pub use scatter::{ScatterChart, scatter};

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
///
/// ## Special Case: Constant Values
///
/// When all values are identical (range ≈ 0), uses a **hardcoded padding of 1.0**
/// to ensure a meaningful range for visualization. This prevents collapsed
/// axes and ensures the constant value is visible in the chart.
///
/// For example, `[5.0, 5.0, 5.0]` returns `(4.0, 6.0)` instead of `(5.0, 5.0)`.
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

#[cfg(test)]
mod tests {
    use super::*;

    // extent_padded tests
    #[test]
    fn test_extent_padded_normal_values() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let (min, max) = extent_padded(&values, 0.05);
        // Min should be 1.0 - 0.05 * 4.0 = 0.8
        // Max should be 5.0 + 0.05 * 4.0 = 5.2
        assert!((min - 0.8).abs() < 1e-10);
        assert!((max - 5.2).abs() < 1e-10);
    }

    #[test]
    fn test_extent_padded_constant_values() {
        let values = vec![5.0, 5.0, 5.0, 5.0];
        let (min, max) = extent_padded(&values, 0.05);
        // Range is 0, so padding should be 1.0
        assert!((min - 4.0).abs() < 1e-10);
        assert!((max - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_extent_padded_single_value() {
        let values = vec![3.0];
        let (min, max) = extent_padded(&values, 0.1);
        // Range is 0, so padding should be 1.0
        assert!((min - 2.0).abs() < 1e-10);
        assert!((max - 4.0).abs() < 1e-10);
    }

    // validate_data_array tests
    #[test]
    fn test_validate_data_array_valid() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!(validate_data_array(&values, "test").is_ok());
    }

    #[test]
    fn test_validate_data_array_empty() {
        let values: Vec<f64> = vec![];
        let result = validate_data_array(&values, "test");
        assert!(matches!(
            result,
            Err(ChartError::EmptyData { field: "test" })
        ));
    }

    #[test]
    fn test_validate_data_array_nan() {
        let values = vec![1.0, 2.0, f64::NAN, 4.0];
        let result = validate_data_array(&values, "test");
        assert!(matches!(
            result,
            Err(ChartError::InvalidData {
                field: "test",
                reason: "contains NaN or Infinity"
            })
        ));
    }

    #[test]
    fn test_validate_data_array_infinity() {
        let values = vec![1.0, f64::INFINITY, 3.0];
        let result = validate_data_array(&values, "test");
        assert!(matches!(
            result,
            Err(ChartError::InvalidData {
                field: "test",
                reason: "contains NaN or Infinity"
            })
        ));
    }

    #[test]
    fn test_validate_data_array_neg_infinity() {
        let values = vec![1.0, 2.0, f64::NEG_INFINITY];
        let result = validate_data_array(&values, "test");
        assert!(matches!(
            result,
            Err(ChartError::InvalidData {
                field: "test",
                reason: "contains NaN or Infinity"
            })
        ));
    }

    // validate_data_length tests
    #[test]
    fn test_validate_data_length_matching() {
        assert!(validate_data_length(5, 5, "x", "y").is_ok());
    }

    #[test]
    fn test_validate_data_length_mismatched() {
        let result = validate_data_length(3, 5, "x", "y");
        assert!(matches!(
            result,
            Err(ChartError::DataLengthMismatch {
                x_field: "x",
                y_field: "y",
                x_len: 3,
                y_len: 5,
            })
        ));
    }

    #[test]
    fn test_validate_data_length_zero() {
        assert!(validate_data_length(0, 0, "x", "y").is_ok());
    }

    // validate_dimensions tests
    #[test]
    fn test_validate_dimensions_valid() {
        assert!(validate_dimensions(600.0, 400.0).is_ok());
    }

    #[test]
    fn test_validate_dimensions_zero_width() {
        let result = validate_dimensions(0.0, 400.0);
        assert!(matches!(
            result,
            Err(ChartError::InvalidDimension {
                field: "width",
                value: 0.0
            })
        ));
    }

    #[test]
    fn test_validate_dimensions_negative_width() {
        let result = validate_dimensions(-100.0, 400.0);
        assert!(matches!(
            result,
            Err(ChartError::InvalidDimension {
                field: "width",
                value: -100.0
            })
        ));
    }

    #[test]
    fn test_validate_dimensions_zero_height() {
        let result = validate_dimensions(600.0, 0.0);
        assert!(matches!(
            result,
            Err(ChartError::InvalidDimension {
                field: "height",
                value: 0.0
            })
        ));
    }

    #[test]
    fn test_validate_dimensions_negative_height() {
        let result = validate_dimensions(600.0, -50.0);
        assert!(matches!(
            result,
            Err(ChartError::InvalidDimension {
                field: "height",
                value: -50.0
            })
        ));
    }
}
