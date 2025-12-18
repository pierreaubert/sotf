//! IIR and FIR filter library for audio processing.
//!
//! This crate provides digital filter implementations for audio signal processing,
//! including biquad IIR filters and FIR filters. The filters are designed for
//! parametric equalization and audio processing applications.
//!
//! # Features
//!
//! - **Biquad IIR filters**: Peak, Lowpass, Highpass, Lowshelf, Highshelf, Bandpass, Notch
//! - **FIR filters**: Windowed sinc filters with various window types
//! - **Frequency response computation**: For both IIR and FIR filters
//! - **Multiple output formats**: APO, RME, AU Preset
//!
//! # Example
//!
//! ```rust
//! use autoeq_iir::{Biquad, BiquadFilterType, SRATE};
//!
//! // Create a peak filter at 1kHz with Q=2 and +3dB gain
//! let filter = Biquad::new(BiquadFilterType::Peak, 1000.0, SRATE, 2.0, 3.0);
//!
//! // Get the frequency response at 1kHz (in dB)
//! let response_db = filter.log_result(1000.0);
//! assert!((response_db - 3.0).abs() < 0.1);
//! ```
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

// Module declarations
mod error;
mod fir;
mod iir;

// Re-export error types
pub use error::{IirError, Result};

// Re-export IIR types and functions
pub use iir::{
    Biquad, BiquadFilterType, FilterRow, Peq, compute_peq_response, peq_butterworth_highpass,
    peq_butterworth_lowpass, peq_butterworth_q, peq_equal, peq_format_apo, peq_format_aupreset,
    peq_format_rme_channel, peq_format_rme_room, peq_linkwitzriley_highpass,
    peq_linkwitzriley_lowpass, peq_linkwitzriley_q, peq_loudness_gain, peq_preamp_gain,
    peq_preamp_gain_max, peq_print, peq_spl,
};

// Re-export FIR types and functions
pub use fir::{
    Fir, FirBank, FirFilterType, WindowType, compute_fir_bank_response, fir_bank_preamp_gain,
    fir_bank_spl, generate_window,
};

// ============================================================================
// Common Helper Functions and Constants
// ============================================================================

/// Converts bandwidth in octaves to a Q factor.
pub fn bw2q(bw: f64) -> f64 {
    let two_pow_bw = 2.0_f64.powf(bw);
    two_pow_bw.sqrt() / (two_pow_bw - 1.0)
}

/// Converts a Q factor to bandwidth in octaves.
pub fn q2bw(q: f64) -> f64 {
    let q2 = (2.0 * q * q + 1.0) / (2.0 * q * q);
    (q2 + (q2 * q2 - 1.0).sqrt()).log(2.0)
}

// Constants
/// Default Q factor for high/low pass filters
pub const DEFAULT_Q_HIGH_LOW_PASS: f64 = 1.0 / std::f64::consts::SQRT_2;
/// Default Q factor for high/low shelf filters
pub const DEFAULT_Q_HIGH_LOW_SHELF: f64 = 1.0668676536332304; // Value of bw2q(0.9)

/// Sample rate constant (matching Python SRATE)
pub const SRATE: f64 = 48000.0;

// ============================================================================
// Tests for Common Functions
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn test_bw_q_roundtrip() {
        let qs = [0.5, 1.0, 2.0, 5.0];
        for &q in &qs {
            let bw = q2bw(q);
            let q2 = bw2q(bw);
            assert!(
                approx_eq(q, q2, 1e-9),
                "roundtrip failed: q={} -> bw={} -> q2={}",
                q,
                bw,
                q2
            );
        }
    }
}
