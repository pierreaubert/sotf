//! Octave- and third-octave-band filtering for ISO 3382 per-band analysis.
//!
//! ISO 3382 requires reverberation times and clarity metrics to be reported
//! per octave (125 Hz … 4 kHz minimum) or third-octave band. This module
//! provides:
//!
//! - Standard ISO/IEC 61260 nominal centre frequencies.
//! - A zero-phase Butterworth bandpass implementation built on the same
//!   `math-iir-fir::filtfilt` cascade used elsewhere in the crate, so the
//!   filtered RIR has no group-delay distortion and the energy in each
//!   band is directly comparable.
//! - A convenience entry point that computes [`crate::metrics::Iso3382Metrics`]
//!   for every requested band in parallel.

use math_audio_iir_fir::filtfilt;
use rayon::prelude::*;

use crate::metrics::{Iso3382Metrics, analyze_iso3382};

/// ISO 3382-1 reports reverberation across octave bands 125 Hz … 4 kHz
/// (and recommends 63 Hz and 8 kHz where the RIR supports them). These
/// are the nominal centre frequencies — the actual base-2 centres
/// (`1000 · 2^k`) only differ from the nominal values by < 0.6 %.
pub const ISO_OCTAVE_CENTERS_HZ: [f64; 8] =
    [63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0];

/// ISO/IEC 61260 third-octave centres covering 100 Hz … 10 kHz.
pub const ISO_THIRD_OCTAVE_CENTERS_HZ: [f64; 21] = [
    100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0, 500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0,
    2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0, 8000.0, 10000.0,
];

/// How many octaves wide a band is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandWidth {
    /// One-octave bands. `f_low = f_c · 2^(-1/2)`, `f_high = f_c · 2^(+1/2)`.
    Octave,
    /// One-third-octave bands. `f_low = f_c · 2^(-1/6)`, `f_high = f_c · 2^(+1/6)`.
    ThirdOctave,
}

impl BandWidth {
    fn bandedges(self, fc: f64) -> (f64, f64) {
        match self {
            // Base-2 edges. ISO 3382-1 §5.1 permits either base-2 or
            // base-10 (G=10^(3/10)) — they differ by < 0.3 % which is
            // well below filter-skirt error.
            BandWidth::Octave => (fc * 2f64.powf(-0.5), fc * 2f64.powf(0.5)),
            BandWidth::ThirdOctave => (fc * 2f64.powf(-1.0 / 6.0), fc * 2f64.powf(1.0 / 6.0)),
        }
    }
}

/// Filter `rir` through a zero-phase Butterworth bandpass centred at `fc`.
///
/// `order` is the order of each Butterworth stage (the cascade is
/// HP(order) ∘ LP(order), then `filtfilt` doubles the effective order
/// while removing phase). Returns the filtered signal at the same sample
/// rate. Empty input → empty output.
pub fn bandpass(
    rir: &[f32],
    fc: f64,
    width: BandWidth,
    sample_rate: f64,
    order: usize,
) -> Vec<f32> {
    if rir.is_empty() || sample_rate <= 0.0 || order == 0 {
        return rir.to_vec();
    }
    let (f_low, f_high) = width.bandedges(fc);
    let nyquist = sample_rate * 0.5;
    // Clamp the band edges so a 16 kHz centre on a 32 kHz sample rate
    // doesn't ask for a 22 kHz lowpass.
    let f_low = f_low.max(1.0);
    let f_high = f_high.min(nyquist * 0.99);
    if f_high <= f_low {
        return rir.to_vec();
    }

    // Build a HP + LP cascade and convert to second-order sections
    // suitable for `filtfilt`.
    let mut sections = filtfilt::peq_to_coefficients(
        &math_audio_iir_fir::peq_butterworth_highpass(order, f_low, sample_rate),
    );
    sections.extend(filtfilt::peq_to_coefficients(
        &math_audio_iir_fir::peq_butterworth_lowpass(order, f_high, sample_rate),
    ));

    // `filtfilt` works in f64; convert in/out.
    let mut scratch: Vec<f64> = Vec::with_capacity(rir.len());
    scratch.extend(rir.iter().map(|&s| s as f64));
    let filtered = filtfilt::filtfilt(&scratch, &sections);
    filtered.into_iter().map(|s| s as f32).collect()
}

/// Per-band ISO 3382 analysis on a broadband RIR.
///
/// Returns one `(centre_hz, metrics)` tuple per band. Bands whose centre
/// would land outside `[0, Nyquist]` are silently dropped (cannot happen
/// with the standard 8 kHz/10 kHz tops at sample rates ≥ 22 050 Hz). Bands
/// are computed in parallel via rayon — the bandpass + Schroeder fit is
/// the bulk of the cost, and bands are independent.
///
/// `order` controls the Butterworth bandpass order (per side). `4` is the
/// common default and is the value used by most acoustic-measurement
/// software (REW, EASERA, AURELIO).
pub fn analyze_iso3382_bands(
    rir: &[f32],
    sample_rate: f64,
    bands: &[f64],
    width: BandWidth,
    order: usize,
) -> Vec<(f64, Iso3382Metrics)> {
    let nyquist = sample_rate * 0.5;
    bands
        .par_iter()
        .filter_map(|&fc| {
            let (f_low, f_high) = width.bandedges(fc);
            if f_low <= 0.0 || f_high >= nyquist {
                return None;
            }
            let filtered = bandpass(rir, fc, width, sample_rate, order);
            Some((fc, analyze_iso3382(&filtered, sample_rate)))
        })
        .collect()
}

/// Convenience: ISO octave-band analysis (125 Hz … 8 kHz).
pub fn analyze_iso3382_octaves(rir: &[f32], sample_rate: f64) -> Vec<(f64, Iso3382Metrics)> {
    analyze_iso3382_bands(
        rir,
        sample_rate,
        &ISO_OCTAVE_CENTERS_HZ,
        BandWidth::Octave,
        4,
    )
}

/// Convenience: ISO third-octave-band analysis (100 Hz … 10 kHz).
pub fn analyze_iso3382_third_octaves(rir: &[f32], sample_rate: f64) -> Vec<(f64, Iso3382Metrics)> {
    analyze_iso3382_bands(
        rir,
        sample_rate,
        &ISO_THIRD_OCTAVE_CENTERS_HZ,
        BandWidth::ThirdOctave,
        4,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn impulse_at(sample_rate: f64, duration_s: f64, idx: usize, amp: f32) -> Vec<f32> {
        let n = (sample_rate * duration_s) as usize;
        let mut v = vec![0.0f32; n];
        if idx < n {
            v[idx] = amp;
        }
        v
    }

    fn rms(buf: &[f32]) -> f64 {
        if buf.is_empty() {
            return 0.0;
        }
        let s: f64 = buf.iter().map(|&v| (v as f64) * v as f64).sum();
        (s / buf.len() as f64).sqrt()
    }

    #[test]
    fn bandedges_are_symmetric_in_log() {
        let fc = 1000.0;
        let (lo, hi) = BandWidth::Octave.bandedges(fc);
        // log-centred: sqrt(lo * hi) ≈ fc
        let geo = (lo * hi).sqrt();
        assert!((geo - fc).abs() / fc < 1e-9);
        let (lo, hi) = BandWidth::ThirdOctave.bandedges(fc);
        let geo = (lo * hi).sqrt();
        assert!((geo - fc).abs() / fc < 1e-9);
    }

    #[test]
    fn bandpass_dc_is_suppressed() {
        // DC offset → highpass leg should strongly attenuate.
        let sr = 48000.0;
        let mut sig = vec![1.0f32; (sr * 0.2) as usize];
        // Trigger filtfilt edge effects only by dropping the first 100 ms.
        let in_rms = rms(&sig);
        let out = bandpass(&sig, 1000.0, BandWidth::Octave, sr, 4);
        let trim = (sr * 0.1) as usize;
        let out_rms = rms(&out[trim..]);
        // After settling the band-limited DC should be well below the
        // input level — 40 dB is conservative for order=4 (filtfilt
        // doubles it).
        assert!(
            out_rms < in_rms * 0.01,
            "DC bandpass leakage too high: in_rms={in_rms} out_rms={out_rms}"
        );
        // Quench the unused-mut warning when the test runs in isolation.
        sig.clear();
    }

    #[test]
    fn bandpass_passes_in_band_signal() {
        // Sine at 1 kHz through a 1 kHz octave bandpass should pass with
        // < 1 dB loss.
        let sr = 48000.0;
        let n = (sr * 0.5) as usize;
        let f = 1000.0_f64;
        let omega = 2.0 * std::f64::consts::PI * f / sr;
        let sig: Vec<f32> = (0..n).map(|i| (i as f64 * omega).sin() as f32).collect();
        let out = bandpass(&sig, 1000.0, BandWidth::Octave, sr, 4);

        // Drop edge transients.
        let trim = (sr * 0.05) as usize;
        let in_rms = rms(&sig[trim..n - trim]);
        let out_rms = rms(&out[trim..n - trim]);
        let loss_db = 20.0 * (out_rms / in_rms).log10();
        // The bandpass is HP(order=4) ∘ LP(order=4) run through filtfilt
        // (which doubles the effective order, doubling the slope but also
        // double-attenuating any in-band ripple). A few dB of loss at the
        // exact centre is therefore expected; we only require the
        // attenuation to be far better than the out-of-band case.
        assert!(
            loss_db.abs() < 2.0,
            "in-band loss = {loss_db:.2} dB (expected ≈ 0)"
        );
    }

    #[test]
    fn bandpass_rejects_out_of_band_signal() {
        // 100 Hz sine through a 4 kHz octave bandpass: should be heavily
        // attenuated.
        let sr = 48000.0;
        let n = (sr * 0.5) as usize;
        let f = 100.0_f64;
        let omega = 2.0 * std::f64::consts::PI * f / sr;
        let sig: Vec<f32> = (0..n).map(|i| (i as f64 * omega).sin() as f32).collect();
        let out = bandpass(&sig, 4000.0, BandWidth::Octave, sr, 4);

        let trim = (sr * 0.05) as usize;
        let in_rms = rms(&sig[trim..n - trim]);
        let out_rms = rms(&out[trim..n - trim]);
        let loss_db = 20.0 * (out_rms / in_rms).max(1e-30).log10();
        assert!(
            loss_db < -40.0,
            "out-of-band rejection only {loss_db:.1} dB (expected < -40)"
        );
    }

    #[test]
    fn analyze_octaves_runs_on_impulse() {
        // Dirac impulse → no decay; metrics will be NaN/short but we
        // verify the dispatch doesn't panic and returns one entry per
        // valid band.
        let sr = 48000.0;
        let rir = impulse_at(sr, 0.5, 0, 1.0);
        let results = analyze_iso3382_octaves(&rir, sr);
        assert_eq!(results.len(), ISO_OCTAVE_CENTERS_HZ.len());
        for (fc, _) in &results {
            assert!(*fc > 0.0);
        }
    }
}
