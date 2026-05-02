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

    /// Invalid measurement data.
    #[error("invalid measurement: {message}")]
    InvalidMeasurement {
        /// Error message describing the issue.
        message: String,
    },

    /// Invalid configuration.
    #[error("invalid configuration: {message}")]
    InvalidConfiguration {
        /// Error message describing the issue.
        message: String,
    },

    /// Reserved: the recording on disk predates the GD-Opt v2 format and
    /// cannot be loaded. The migration path was removed in Phase GD-1a.1;
    /// users must re-record.
    #[error("unsupported recording format at '{}': {detail}", path.display())]
    UnsupportedRecordingFormat {
        /// Path to the recording file that cannot be loaded.
        path: std::path::PathBuf,
        /// Human-readable detail about why the recording is unsupported.
        detail: String,
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
        matches!(self, AutoeqError::OptimizationFailed { .. })
    }

    /// Returns true if this error is non-retriable because the recording
    /// file on disk predates GD-Opt v2. Callers must ask the user to
    /// re-record rather than surfacing a generic I/O failure.
    pub fn is_unsupported_recording_format(&self) -> bool {
        matches!(self, AutoeqError::UnsupportedRecordingFormat { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_recording_format_display_and_classification() {
        let err = AutoeqError::UnsupportedRecordingFormat {
            path: std::path::PathBuf::from("/tmp/recordings/session.json"),
            detail: "missing v2 fields".to_string(),
        };

        // Display stringifies path + detail sensibly.
        let msg = format!("{err}");
        assert!(msg.contains("/tmp/recordings/session.json"), "got: {msg}");
        assert!(msg.contains("missing v2 fields"), "got: {msg}");
        assert!(msg.contains("unsupported recording format"), "got: {msg}");

        // Classification helper matches only this variant.
        assert!(err.is_unsupported_recording_format());
        assert!(!err.is_io_error());
        assert!(!err.is_cea2034_error());
        assert!(!err.is_optimization_error());
    }
}
