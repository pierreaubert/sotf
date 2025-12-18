//! Error types for the autoeq crate.
//!
//! This module provides a unified error type for all autoeq operations,
//! following Microsoft Rust Guidelines for proper error handling.

use thiserror::Error;

/// Error type for autoeq operations.
///
/// This enum captures all possible errors that can occur during EQ optimization,
/// data loading, and result output operations.
#[derive(Debug, Error)]
pub enum AutoeqError {
    /// A required CEA2034 curve is missing from the spin data.
    #[error("missing CEA2034 curve: '{curve_name}'")]
    MissingCea2034Curve {
        /// Name of the missing curve (e.g., "On Axis", "Listening Window").
        curve_name: String,
    },

    /// CEA2034 curves have inconsistent lengths.
    #[error("CEA2034 curve length mismatch: on={on_len}, lw={lw_len}, sp={sp_len}, pir={pir_len}")]
    CurveLengthMismatch {
        /// Length of the On Axis curve.
        on_len: usize,
        /// Length of the Listening Window curve.
        lw_len: usize,
        /// Length of the Sound Power curve.
        sp_len: usize,
        /// Length of the Predicted In-Room curve.
        pir_len: usize,
    },

    /// Failed to load a target curve from a file.
    #[error("failed to load target curve from '{path}': {message}")]
    TargetCurveLoad {
        /// Path to the target curve file.
        path: String,
        /// Error message describing the failure.
        message: String,
    },

    /// An invalid algorithm name was provided.
    #[error("invalid algorithm name: '{name}'")]
    InvalidAlgorithm {
        /// The invalid algorithm name.
        name: String,
    },

    /// A file operation failed (create, write, read).
    #[error("file operation failed for '{path}': {message}")]
    FileOperation {
        /// Path to the file.
        path: String,
        /// Error message describing the failure.
        message: String,
    },

    /// Directory creation failed.
    #[error("failed to create directory '{path}': {message}")]
    DirectoryCreation {
        /// Path to the directory.
        path: String,
        /// Error message describing the failure.
        message: String,
    },

    /// Optimization algorithm failed.
    #[error("optimization failed: {message}")]
    OptimizationFailed {
        /// Error message describing the failure.
        message: String,
    },

    /// NLopt-specific error.
    #[cfg(feature = "nlopt")]
    #[error("NLopt error: {message}")]
    NloptError {
        /// Error message from NLopt.
        message: String,
    },

    /// I/O error wrapper.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Result type alias for autoeq operations.
pub type Result<T> = std::result::Result<T, AutoeqError>;

impl AutoeqError {
    /// Returns true if this is a CEA2034 data error.
    pub fn is_cea2034_error(&self) -> bool {
        matches!(
            self,
            AutoeqError::MissingCea2034Curve { .. } | AutoeqError::CurveLengthMismatch { .. }
        )
    }

    /// Returns true if this is a file/IO error.
    pub fn is_io_error(&self) -> bool {
        matches!(
            self,
            AutoeqError::FileOperation { .. }
                | AutoeqError::DirectoryCreation { .. }
                | AutoeqError::TargetCurveLoad { .. }
                | AutoeqError::Io(_)
        )
    }

    /// Returns true if this is an optimization error.
    pub fn is_optimization_error(&self) -> bool {
        #[cfg(feature = "nlopt")]
        if matches!(self, AutoeqError::NloptError { .. }) {
            return true;
        }
        matches!(self, AutoeqError::OptimizationFailed { .. })
    }
}
