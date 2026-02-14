//! Error types for IIR filter operations.
//!
//! This module provides structured error handling for IIR filter creation
//! and manipulation, following the Microsoft Rust Guidelines pattern.

use thiserror::Error;

/// Errors that can occur during IIR filter operations.
#[derive(Debug, Error)]
pub enum IirError {
    /// Q factor is invalid (must be > 0).
    #[error("invalid Q factor: {q} (must be > 0)")]
    InvalidQ {
        /// The invalid Q value
        q: f64,
    },

    /// Frequency is invalid (must be > 0 and < Nyquist).
    #[error("invalid frequency: {freq} Hz (must be > 0 and < Nyquist frequency {nyquist} Hz)")]
    InvalidFrequency {
        /// The invalid frequency value
        freq: f64,
        /// The Nyquist frequency (sample_rate / 2)
        nyquist: f64,
    },

    /// Sample rate is invalid (must be > 0).
    #[error("invalid sample rate: {sample_rate} Hz (must be > 0)")]
    InvalidSampleRate {
        /// The invalid sample rate value
        sample_rate: f64,
    },

    /// Gain value is invalid (non-finite).
    #[error("invalid gain: {gain_db} dB (must be finite)")]
    InvalidGain {
        /// The invalid gain value
        gain_db: f64,
    },
}

/// A specialized `Result` type for IIR operations.
pub type Result<T> = std::result::Result<T, IirError>;

impl IirError {
    /// Returns `true` if this is a frequency-related error.
    pub fn is_frequency_error(&self) -> bool {
        matches!(self, IirError::InvalidFrequency { .. })
    }

    /// Returns `true` if this is a Q-factor error.
    pub fn is_q_error(&self) -> bool {
        matches!(self, IirError::InvalidQ { .. })
    }

    /// Returns `true` if this is a sample rate error.
    pub fn is_sample_rate_error(&self) -> bool {
        matches!(self, IirError::InvalidSampleRate { .. })
    }

    /// Returns `true` if this is a gain error.
    pub fn is_gain_error(&self) -> bool {
        matches!(self, IirError::InvalidGain { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = IirError::InvalidQ { q: -1.0 };
        assert_eq!(err.to_string(), "invalid Q factor: -1 (must be > 0)");
    }

    #[test]
    fn test_frequency_error_display() {
        let err = IirError::InvalidFrequency {
            freq: 25000.0,
            nyquist: 24000.0,
        };
        assert!(err.to_string().contains("25000"));
        assert!(err.to_string().contains("24000"));
    }

    #[test]
    fn test_is_frequency_error() {
        let freq_err = IirError::InvalidFrequency {
            freq: 0.0,
            nyquist: 24000.0,
        };
        let q_err = IirError::InvalidQ { q: -1.0 };

        assert!(freq_err.is_frequency_error());
        assert!(!q_err.is_frequency_error());
    }

    #[test]
    fn test_is_q_error() {
        let q_err = IirError::InvalidQ { q: 0.0 };
        let gain_err = IirError::InvalidGain {
            gain_db: f64::INFINITY,
        };

        assert!(q_err.is_q_error());
        assert!(!gain_err.is_q_error());
    }
}
