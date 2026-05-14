//! IIR and FIR filter library for audio processing.
//!
//! This crate provides digital filter implementations for audio signal processing,
//! including biquad IIR filters, FIR filters, SVF filters, and crossovers.
//!
//! # Generic precision
//!
//! All filter types are generic over [`FilterFloat`] (`f32` or `f64`), defaulting to
//! `f64` for backward compatibility. Use `f32` when throughput matters more than
//! precision (e.g., real-time processing of many channels). Convenience aliases like
//! [`BiquadF32`], [`SvfFilterF32`], etc. are provided.
//!
//! # Features
//!
//! - **Biquad IIR filters**: Peak, Lowpass, Highpass, Lowshelf, Highshelf, Bandpass, Notch
//! - **SVF filters**: Zero-delay feedback topology for artifact-free parameter changes
//! - **FIR filters**: Windowed sinc filters with various window types
//! - **Crossovers**: Linkwitz-Riley IIR and linear-phase FIR crossovers
//! - **Offline filtering**: Zero-phase `filtfilt` for analysis (no phase distortion)
//! - **Frequency response computation**: For both IIR and FIR filters
//! - **Multiple output formats**: APO, RME, AU Preset
//!
//! # Example
//!
//! ```rust
//! use math_audio_iir_fir::{Biquad, BiquadFilterType};
//!
//! // f64 (default)
//! let filter = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 3.0);
//! let response_db: f64 = filter.log_result(1000.0);
//! assert!((response_db - 3.0_f64).abs() < 0.1);
//!
//! // f32 — same API, lower precision, higher throughput
//! let filter_f32 = Biquad::<f32>::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 3.0);
//! let response_f32: f32 = filter_f32.log_result(1000.0);
//! assert!((response_f32 - 3.0_f32).abs() < 0.5);
//! ```
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

// Module declarations
pub mod denormals;
mod error;
mod traits;

pub use traits::FilterFloat;
/// Forward-reverse (zero-phase) filtering for offline signal processing.
pub mod filtfilt;
mod fir;
/// Linear-phase FIR crossover (windowed sinc).
pub mod fir_crossover;
mod fir_design;
mod iir;
/// Linkwitz-Riley 4th-order IIR crossover.
pub mod lr4_crossover;
/// Linkwitz-Riley 8th-order IIR crossover (48 dB/octave).
pub mod lr8_crossover;
mod phase_smooth;
/// Zero-Delay Feedback State Variable Filter (Zavalishin TPT topology).
pub mod svf;

// Re-export error types
pub use error::{IirError, Result};

// Re-export IIR types and functions
pub use iir::{
    Biquad, BiquadBank, BiquadCoefficients, BiquadFilterType, FilterRow, KautzFilter, KautzSection,
    Peq, WarpedBiquad, bark_lambda, compute_peq_response, peq_allpass, peq_butterworth_highpass,
    peq_butterworth_lowpass, peq_butterworth_q, peq_equal, peq_format_apo, peq_format_aupreset,
    peq_format_camilladsp, peq_format_easyeffects, peq_format_pipewire, peq_format_rme_channel,
    peq_format_rme_room, peq_format_roon, peq_format_wavelet, peq_linkwitzriley_highpass,
    peq_linkwitzriley_lowpass, peq_linkwitzriley_q, peq_loudness_gain, peq_preamp_gain,
    peq_preamp_gain_max, peq_print, peq_spl, unwarp_frequency, warp_frequency,
};

// Re-export FIR types and functions
pub use fir::{
    Fir, FirBank, FirFilterType, WindowType, compute_fir_bank_response, fir_bank_preamp_gain,
    fir_bank_spl, generate_window,
};

// Re-export FIR design types and functions (frequency response matching)
pub use fir_design::{
    FirDesignConfig, FirPhase, PreRingingAnalysis, PreRingingConfig, analyze_pre_ringing,
    generate_fir_from_response, generate_kirkeby_correction, save_fir_to_wav, suppress_pre_ringing,
};

// Re-export phase smoothing functions
pub use phase_smooth::{interpolate_phase_complex, smooth_phase_via_group_delay, unwrap_phase};

// Re-export SVF filter types
pub use svf::{SvfFilter, SvfFilterType};

// Re-export crossover types
pub use fir_crossover::{DEFAULT_FIR_CROSSOVER_TAPS, FirCrossover, MultibandFirCrossover};
pub use lr4_crossover::{CROSSOVER_PRESETS, Lr4Crossover, MultibandLr4Crossover};
pub use lr8_crossover::{Lr8Crossover, MultibandLr8Crossover};

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

/// Lower bound of human hearing (Hz).
pub const AUDIBLE_MIN_FREQ: f64 = 20.0;
/// Upper bound of human hearing (Hz).
pub const AUDIBLE_MAX_FREQ: f64 = 20_000.0;

// ============================================================================
// Convenience type aliases for f32 instantiation
// ============================================================================

/// 32-bit biquad filter.
pub type BiquadF32 = Biquad<f32>;
/// 32-bit biquad coefficients.
pub type BiquadCoefficientsF32 = BiquadCoefficients<f32>;
/// 32-bit parametric EQ chain.
pub type PeqF32 = Peq<f32>;
/// 32-bit biquad bank.
pub type BiquadBankF32 = BiquadBank<f32>;
/// 32-bit SVF filter.
pub type SvfFilterF32 = SvfFilter<f32>;
/// 32-bit FIR filter.
pub type FirF32 = Fir<f32>;
/// 32-bit FIR filter bank.
pub type FirBankF32 = FirBank<f32>;
/// 32-bit LR4 crossover.
pub type Lr4CrossoverF32 = Lr4Crossover<f32>;
/// 32-bit multiband LR4 crossover.
pub type MultibandLr4CrossoverF32 = MultibandLr4Crossover<f32>;
/// 32-bit LR8 crossover.
pub type Lr8CrossoverF32 = Lr8Crossover<f32>;
/// 32-bit multiband LR8 crossover.
pub type MultibandLr8CrossoverF32 = MultibandLr8Crossover<f32>;
/// 32-bit FIR crossover.
pub type FirCrossoverF32 = FirCrossover<f32>;
/// 32-bit multiband FIR crossover.
pub type MultibandFirCrossoverF32 = MultibandFirCrossover<f32>;

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
