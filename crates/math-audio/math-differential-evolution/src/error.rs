//! Error types for the Differential Evolution optimizer.
//!
//! This module provides structured error handling for DE optimization,
//! following the Microsoft Rust Guidelines pattern of using `thiserror`
//! for library error types with helper methods for error categorization.

use thiserror::Error;

/// Errors that can occur during Differential Evolution optimization.
#[derive(Debug, Error)]
pub enum DEError {
    /// Lower and upper bounds have different lengths.
    #[error("bounds mismatch: lower has {lower_len} elements, upper has {upper_len}")]
    BoundsMismatch {
        /// Length of the lower bounds array
        lower_len: usize,
        /// Length of the upper bounds array
        upper_len: usize,
    },

    /// A lower bound exceeds its corresponding upper bound.
    #[error("invalid bounds at index {index}: lower ({lower}) > upper ({upper})")]
    InvalidBounds {
        /// Index of the invalid bound pair
        index: usize,
        /// The lower bound value
        lower: f64,
        /// The upper bound value
        upper: f64,
    },

    /// Population size is too small (must be >= 4).
    #[error("population size ({pop_size}) must be >= 4 for DE algorithm")]
    PopulationTooSmall {
        /// The invalid population size
        pop_size: usize,
    },

    /// Mutation factor is out of valid range [0, 2].
    #[error("invalid mutation factor: {factor} (must be in [0, 2])")]
    InvalidMutationFactor {
        /// The invalid mutation factor
        factor: f64,
    },

    /// Crossover rate is out of valid range [0, 1].
    #[error("invalid crossover rate: {rate} (must be in [0, 1])")]
    InvalidCrossoverRate {
        /// The invalid crossover rate
        rate: f64,
    },

    /// Initial guess (x0) has wrong dimension.
    #[error("x0 dimension mismatch: expected {expected}, got {got}")]
    X0DimensionMismatch {
        /// Expected dimension
        expected: usize,
        /// Actual dimension provided
        got: usize,
    },

    /// Integrality mask has wrong dimension.
    #[error("integrality mask dimension mismatch: expected {expected}, got {got}")]
    IntegralityDimensionMismatch {
        /// Expected dimension
        expected: usize,
        /// Actual dimension provided
        got: usize,
    },
}

/// A specialized `Result` type for DE operations.
pub type Result<T> = std::result::Result<T, DEError>;

impl DEError {
    /// Returns `true` if this is a bounds-related error.
    ///
    /// This includes `BoundsMismatch` and `InvalidBounds` variants.
    pub fn is_bounds_error(&self) -> bool {
        matches!(
            self,
            DEError::BoundsMismatch { .. } | DEError::InvalidBounds { .. }
        )
    }

    /// Returns `true` if this is a configuration-related error.
    ///
    /// This includes `PopulationTooSmall`, `InvalidMutationFactor`,
    /// and `InvalidCrossoverRate` variants.
    pub fn is_config_error(&self) -> bool {
        matches!(
            self,
            DEError::PopulationTooSmall { .. }
                | DEError::InvalidMutationFactor { .. }
                | DEError::InvalidCrossoverRate { .. }
        )
    }

    /// Returns `true` if this is a dimension mismatch error.
    ///
    /// This includes `X0DimensionMismatch` and `IntegralityDimensionMismatch`.
    pub fn is_dimension_error(&self) -> bool {
        matches!(
            self,
            DEError::X0DimensionMismatch { .. } | DEError::IntegralityDimensionMismatch { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = DEError::BoundsMismatch {
            lower_len: 3,
            upper_len: 5,
        };
        assert_eq!(
            err.to_string(),
            "bounds mismatch: lower has 3 elements, upper has 5"
        );
    }

    #[test]
    fn test_is_bounds_error() {
        let bounds_err = DEError::BoundsMismatch {
            lower_len: 1,
            upper_len: 2,
        };
        let config_err = DEError::PopulationTooSmall { pop_size: 2 };

        assert!(bounds_err.is_bounds_error());
        assert!(!config_err.is_bounds_error());
    }

    #[test]
    fn test_is_config_error() {
        let config_err = DEError::InvalidCrossoverRate { rate: 1.5 };
        let bounds_err = DEError::InvalidBounds {
            index: 0,
            lower: 5.0,
            upper: 3.0,
        };

        assert!(config_err.is_config_error());
        assert!(!bounds_err.is_config_error());
    }

    #[test]
    fn test_is_dimension_error() {
        let dim_err = DEError::X0DimensionMismatch {
            expected: 10,
            got: 5,
        };
        let bounds_err = DEError::BoundsMismatch {
            lower_len: 1,
            upper_len: 2,
        };

        assert!(dim_err.is_dimension_error());
        assert!(!bounds_err.is_dimension_error());
    }
}
