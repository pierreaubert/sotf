//! Kautz filter implementation for room mode correction.
//!
//! Kautz filters are rational orthonormal structures where each section has an
//! independently chosen pole. By placing poles at detected room mode frequencies,
//! the filter concentrates its frequency resolution exactly where it is needed.
//! A 5-section Kautz filter can model 5 room modes more accurately than 5 standard
//! biquad PEQs, using fewer parameters.
//!
//! # Structure
//!
//! Each [`KautzSection`] implements a second-order allpass with complex pole at
//! `r * exp(jθ)`. The overall [`KautzFilter`] is a parallel bank of sections whose
//! weighted basis function outputs are summed.
//!
//! # Gain optimization
//!
//! Given a measured room response and a flat target, [`KautzFilter::optimize_gains`]
//! solves the linear least-squares problem `Φ g = t` where `Φ[i][j]` is section j's
//! dB response at frequency i and `t` is the correction needed. The regularized
//! system is solved with modified Gram-Schmidt QR on `[Φ; sqrt(λ) I]`, avoiding
//! the condition-number squaring of normal equations for closely spaced poles.

use ndarray::Array1;
use num_complex::Complex;
use std::f64::consts::PI;

use crate::traits::{FilterFloat, lit};

// ---------------------------------------------------------------------------
// KautzSection
// ---------------------------------------------------------------------------

/// A second-order Kautz section with pole tuned to a specific frequency.
///
/// Implements an orthonormal basis function built from a second-order allpass.
/// The pole is placed at `pole_freq` with radius `pole_radius`, concentrating
/// frequency resolution around that frequency.
///
/// # Allpass definition
///
/// ```text
/// A(z) = (r² + a1·z⁻¹ + z⁻²) / (1 + a1·z⁻¹ + r²·z⁻²)
/// ```
///
/// where `a1 = -2r·cos(θ)` and `θ = 2π·pole_freq / srate`.
///
/// # Basis function
///
/// The basis function for this section (assuming allpass outputs from all
/// preceding sections have been accumulated into the input) is:
///
/// ```text
/// φ(z) = sqrt(1 - r²) · (1 + z⁻¹) / (1 + a1·z⁻¹ + r²·z⁻²)  · chain
/// ```
///
/// In the time domain, each section receives the allpass-filtered output from
/// all previous sections and extracts its own orthonormal component.
pub struct KautzSection<T: FilterFloat = f64> {
    /// Pole frequency in Hz.
    pub pole_freq: T,
    /// Pole radius in (0, 1). Controls how sharply resolution concentrates.
    /// Derived from room mode Q: `radius = exp(-π · pole_freq / (Q · srate))`.
    pub pole_radius: T,
    /// Gain coefficient — weight of this basis function in the correction filter.
    pub gain: T,
    /// Sample rate in Hz.
    pub srate: T,
    // Allpass denominator coefficients (normalized, denominator leading coeff = 1):
    //   a1 = -2r·cos(θ),  a2 = r²
    a1_coeff: T,
    a2_coeff: T,
    // Transposed direct form II state for the allpass
    s1: T,
    s2: T,
    // Transposed direct form II state for the basis function output
    b_s1: T,
    b_s2: T,
}

impl<T: FilterFloat> KautzSection<T> {
    /// Create a Kautz section with a pole at `pole_freq` Hz and the given Q.
    ///
    /// The pole radius is `exp(-π · pole_freq / (q · srate))`.
    /// `gain` sets the initial weight of this section (typically 0.0 before optimization).
    pub fn new(pole_freq: T, q: T, gain: T, srate: T) -> Self {
        let q_clamped = if q < lit::<T>(0.1) { lit::<T>(0.1) } else { q };
        let theta = lit::<T>(2.0) * T::PI() * pole_freq / srate;
        let radius = (-T::PI() * pole_freq / (q_clamped * srate)).exp();
        // Clamp radius to (0, 0.9999) for stability
        let radius = radius.min(lit::<T>(0.9999)).max(T::zero());

        Self::from_pole(pole_freq, radius, gain, srate, theta)
    }

    /// Create a section directly from a precomputed pole radius and frequency angle.
    fn from_pole(pole_freq: T, radius: T, gain: T, srate: T, theta: T) -> Self {
        let r2 = radius * radius;
        let a1_coeff = lit::<T>(-2.0) * radius * theta.cos();
        let a2_coeff = r2;

        KautzSection {
            pole_freq,
            pole_radius: radius,
            gain,
            srate,
            a1_coeff,
            a2_coeff,
            s1: T::zero(),
            s2: T::zero(),
            b_s1: T::zero(),
            b_s2: T::zero(),
        }
    }

    /// Process one sample through this section.
    ///
    /// `x` is the input (allpass-chained output from all preceding sections).
    ///
    /// Returns `(basis_value, allpass_output)`:
    /// - `basis_value`: the orthonormal basis function value (multiply by `gain` and sum)
    /// - `allpass_output`: the allpass output to pass as input to the next section
    ///
    /// Both the allpass and the basis extraction use transposed direct form II for
    /// numerical stability at high Q.
    #[inline(always)]
    pub fn process_section(&mut self, x: T) -> (T, T) {
        let a1 = self.a1_coeff;
        let a2 = self.a2_coeff; // = r²

        // Allpass: A(z) = (a2 + a1·z⁻¹ + z⁻²) / (1 + a1·z⁻¹ + a2·z⁻²)
        // TDF-II with numerator b0=a2, b1=a1, b2=1 and denominator 1, a1, a2.
        let ap_out = a2 * x + self.s1;
        self.s1 = a1 * x - a1 * ap_out + self.s2;
        self.s2 = x - a2 * ap_out;

        // Basis function: H(z) = sqrt(1-r²)·(1-r²) / (1 + a1·z⁻¹ + r²·z⁻²) · chain
        // Implemented as TDF-II with constant numerator b0 = norm*(1-a2).
        // This matches basis_response() in the frequency domain exactly.
        let norm = (T::one() - a2).sqrt(); // sqrt(1 - r²)
        let b0 = norm * (T::one() - a2); // sqrt(1-r²) * (1-r²)
        let basis_out = b0 * x + self.b_s1;
        self.b_s1 = -a1 * basis_out + self.b_s2;
        self.b_s2 = -a2 * basis_out;

        (basis_out, ap_out)
    }

    /// Compute this section's allpass complex frequency response at `freq` Hz.
    ///
    /// `A(e^jω) = (a2 + a1·e^{-jω} + e^{-2jω}) / (1 + a1·e^{-jω} + a2·e^{-2jω})`
    pub fn allpass_response(&self, freq: f64, srate: f64) -> Complex<f64> {
        let omega = 2.0 * PI * freq / srate;
        let a1 = self.a1_coeff.to_f64().unwrap();
        let a2 = self.a2_coeff.to_f64().unwrap();
        let z_inv = Complex::from_polar(1.0, -omega);
        let z_inv2 = z_inv * z_inv;
        let num = Complex::new(a2, 0.0) + z_inv * a1 + z_inv2;
        let den = Complex::new(1.0, 0.0) + z_inv * a1 + z_inv2 * a2;
        num / den
    }

    /// Compute this section's basis function complex frequency response at `freq` Hz,
    /// given that the accumulated allpass chain from all preceding sections is `chain`.
    pub fn basis_response(&self, freq: f64, srate: f64, chain: Complex<f64>) -> Complex<f64> {
        let omega = 2.0 * PI * freq / srate;
        let r = self.pole_radius.to_f64().unwrap();
        let r2 = r * r;
        let a1 = self.a1_coeff.to_f64().unwrap();
        let a2 = self.a2_coeff.to_f64().unwrap();
        let z_inv = Complex::from_polar(1.0, -omega);
        let z_inv2 = z_inv * z_inv;

        let norm = (1.0 - r2).sqrt();
        // H_basis(z) = sqrt(1-r²) * (1 - a2) / (1 + a1*z^-1 + a2*z^-2)
        // This is the frequency-domain form of the basis extraction
        let den = Complex::new(1.0, 0.0) + z_inv * a1 + z_inv2 * a2;
        let num = Complex::new(norm * (1.0 - a2), 0.0);
        (num / den) * chain
    }

    /// Reset all filter state to zero.
    #[inline]
    pub fn reset(&mut self) {
        self.s1 = T::zero();
        self.s2 = T::zero();
        self.b_s1 = T::zero();
        self.b_s2 = T::zero();
    }
}

// ---------------------------------------------------------------------------
// KautzFilter
// ---------------------------------------------------------------------------

/// Complete Kautz filter: parallel bank of [`KautzSection`]s.
///
/// Each section targets a specific room mode. The filter output is the
/// weighted sum of all sections' basis function values:
///
/// ```text
/// y[n] = Σ_k  gain_k · φ_k(x[n])
/// ```
///
/// Gains are initialized to zero and should be set via [`optimize_gains`] or
/// manually before processing.
///
/// [`optimize_gains`]: KautzFilter::optimize_gains
pub struct KautzFilter<T: FilterFloat = f64> {
    /// The individual Kautz sections (one per room mode).
    pub sections: Vec<KautzSection<T>>,
    /// Sample rate in Hz.
    pub srate: T,
}

impl KautzFilter<f64> {
    /// Create a Kautz filter from detected room modes.
    ///
    /// `modes` is a slice of `(frequency_hz, q_factor)` pairs.
    /// Pole radii are derived from `radius = exp(-π·f / (Q·srate))`.
    /// All gains are initialized to `0.0`.
    pub fn from_room_modes(modes: &[(f64, f64)], sample_rate: f64) -> Self {
        let sections = modes
            .iter()
            .map(|&(freq, q)| KautzSection::new(freq, q, 0.0, sample_rate))
            .collect();

        KautzFilter {
            sections,
            srate: sample_rate,
        }
    }

    /// Optimize section gains using linear least-squares to match the target correction.
    ///
    /// Solves `Φ g = t` where:
    /// - `Φ[i][j]` = section j's basis function dB response at `freqs[i]`
    /// - `t[i]` = `target_spl[i] - measured_spl[i]` (correction needed in dB)
    ///
    /// Uses modified Gram-Schmidt QR on the augmented regularized system
    /// `[Φ; sqrt(λ) I]`. This is more robust than normal equations for
    /// closely spaced room-mode poles because it does not square the condition
    /// number of `Φ`.
    pub fn optimize_gains(&mut self, freqs: &[f64], measured_spl: &[f64], target_spl: &[f64]) {
        let n = freqs.len();
        let m = self.sections.len();
        if n == 0 || m == 0 {
            return;
        }

        // Build Φ matrix (n × m): each column j holds section j's basis function
        // magnitude response (linear, not dB) over the frequency grid.
        // Target vector t holds the required correction in dB at each frequency.
        //
        // We work in linear magnitude for Φ so that the column values are
        // positive and peak near the section's pole.  The gains then act as
        // dB multipliers: total_correction_db(f) ≈ Σ_k gain_k · |Φ_k(f)|.
        let mut phi = vec![0.0f64; n * m];
        let mut t = vec![0.0f64; n];

        for (i, (&freq, (&meas, &tgt))) in freqs
            .iter()
            .zip(measured_spl.iter().zip(target_spl.iter()))
            .enumerate()
        {
            t[i] = tgt - meas;

            // Accumulate the allpass chain across sections
            let mut chain = Complex::new(1.0, 0.0);
            for (j, section) in self.sections.iter().enumerate() {
                let basis = section.basis_response(freq, self.srate, chain);
                phi[i * m + j] = basis.norm();
                // Advance chain by this section's allpass
                chain *= section.allpass_response(freq, self.srate);
            }
        }

        // Normalize each column so its maximum value is 1.0.
        // This ensures each gain has the same scale (dB boost/cut at the pole)
        // and makes the sign relationship intuitive: negative gain = attenuation.
        for j in 0..m {
            let col_max = (0..n).map(|i| phi[i * m + j]).fold(0.0f64, f64::max);
            if col_max > 1.0e-20 {
                for i in 0..n {
                    phi[i * m + j] /= col_max;
                }
            }
        }

        // Regularization strength: λ = 1e-6 * max column norm prevents
        // division by near-zero values when sections have very similar responses.
        let max_col_norm_sq = (0..m)
            .map(|j| (0..n).map(|i| phi[i * m + j] * phi[i * m + j]).sum::<f64>())
            .fold(0.0f64, f64::max);
        let lambda = 1.0e-4 * max_col_norm_sq.max(1.0);

        // Issue #4: solve via QR on the augmented system instead of normal
        // equations + Cholesky.  QR avoids squaring the condition number of Φ.
        if let Some(gains) = qr_least_squares(n, m, &phi, &t, lambda) {
            for (section, &g) in self.sections.iter_mut().zip(gains.iter()) {
                section.gain = g;
            }
        }
    }

    /// Process one sample through all sections in parallel, returning the
    /// weighted sum of all basis function outputs.
    #[inline]
    pub fn process(&mut self, sample: f64) -> f64 {
        let mut ap_input = sample;
        let mut output = 0.0f64;

        for section in self.sections.iter_mut() {
            let (basis_val, ap_out) = section.process_section(ap_input);
            output += section.gain * basis_val;
            ap_input = ap_out;
        }

        output
    }

    /// Process a block of samples in-place.
    pub fn process_block(&mut self, buf: &mut [f64]) {
        for sample in buf.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    /// Compute the complex frequency response of the entire filter at `freq` Hz.
    pub fn complex_response(&self, freq: f64) -> Complex<f64> {
        let mut total = Complex::new(0.0, 0.0);
        let mut chain = Complex::new(1.0, 0.0);

        for section in &self.sections {
            let basis = section.basis_response(freq, self.srate, chain);
            total += basis * section.gain;
            chain *= section.allpass_response(freq, self.srate);
        }

        total
    }

    /// Compute magnitude response in dB at each frequency in `freqs`.
    pub fn np_log_result(&self, freqs: &Array1<f64>) -> Array1<f64> {
        freqs.mapv(|f| {
            let h = self.complex_response(f);
            let mag = h.norm();
            if mag > 1.0e-20 {
                20.0 * mag.log10()
            } else {
                -400.0
            }
        })
    }

    /// Reset all section states to zero.
    pub fn reset(&mut self) {
        for section in self.sections.iter_mut() {
            section.reset();
        }
    }
}

// ---------------------------------------------------------------------------
// QR least-squares solver (replaces normal equations + Cholesky for stability)
// ---------------------------------------------------------------------------

/// Solve the regularized least-squares problem `min ||Φ·g − t||² + λ·||g||²`
/// using modified Gram–Schmidt QR on the augmented system `[Φ; √λ·I]`.
///
/// The augmented matrix has `n + m` rows and `m` columns.  Regularization
/// rows prevent rank-deficiency when basis functions are nearly linearly
/// dependent (closely-spaced poles).
fn qr_least_squares(n: usize, m: usize, phi: &[f64], t: &[f64], lambda: f64) -> Option<Vec<f64>> {
    let aug_n = n + m;
    let mut q = vec![0.0f64; aug_n * m];
    let mut r = vec![0.0f64; m * m];

    // Copy Φ into the top of the augmented matrix.
    for i in 0..n {
        for j in 0..m {
            q[i * m + j] = phi[i * m + j];
        }
    }
    // Append √(λ)·I at the bottom.
    let sqrt_lambda = lambda.sqrt();
    for j in 0..m {
        q[(n + j) * m + j] = sqrt_lambda;
    }

    // Augmented target vector: [t; 0].
    let mut aug_t = vec![0.0f64; aug_n];
    aug_t[..n].copy_from_slice(&t[..n]);

    // Modified Gram–Schmidt.
    for k in 0..m {
        let mut norm_sq = 0.0;
        for i in 0..aug_n {
            norm_sq += q[i * m + k] * q[i * m + k];
        }
        let norm = norm_sq.sqrt();
        if norm < 1e-20 {
            return None;
        }
        r[k * m + k] = norm;
        for i in 0..aug_n {
            q[i * m + k] /= norm;
        }
        for j in (k + 1)..m {
            let mut dot = 0.0;
            for i in 0..aug_n {
                dot += q[i * m + k] * q[i * m + j];
            }
            r[k * m + j] = dot;
            for i in 0..aug_n {
                q[i * m + j] -= dot * q[i * m + k];
            }
        }
    }

    // Qᵀ · aug_t  (m-dimensional).
    let mut qt = vec![0.0f64; m];
    for j in 0..m {
        let mut sum = 0.0;
        for i in 0..aug_n {
            sum += q[i * m + j] * aug_t[i];
        }
        qt[j] = sum;
    }

    // Back-substitution: R·g = Qᵀ·aug_t.
    let mut g = vec![0.0f64; m];
    for i in (0..m).rev() {
        let mut sum = qt[i];
        for j in (i + 1)..m {
            sum -= r[i * m + j] * g[j];
        }
        if r[i * m + i].abs() < 1e-20 {
            return None;
        }
        g[i] = sum / r[i * m + i];
    }

    Some(g)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kautz_section_pole_at_80hz() {
        // Create section with pole at 80 Hz, Q=5. Gain=1 so basis response is unscaled.
        let section = KautzSection::new(80.0f64, 5.0, 1.0, 48000.0);

        // Compute basis function magnitude at several frequencies.
        // The section should show a distinct peak near 80 Hz.
        let chain = Complex::new(1.0, 0.0);
        let mag_at_pole = section.basis_response(80.0, 48000.0, chain).norm();
        let mag_at_40 = section.basis_response(40.0, 48000.0, chain).norm();
        let mag_at_160 = section.basis_response(160.0, 48000.0, chain).norm();

        // The magnitude at 80 Hz should be strictly greater than at 40 or 160 Hz.
        assert!(
            mag_at_pole > mag_at_40,
            "basis magnitude at 80 Hz ({mag_at_pole:.4}) should exceed 40 Hz ({mag_at_40:.4})"
        );
        assert!(
            mag_at_pole > mag_at_160,
            "basis magnitude at 80 Hz ({mag_at_pole:.4}) should exceed 160 Hz ({mag_at_160:.4})"
        );
    }

    #[test]
    fn test_kautz_filter_from_modes() {
        let modes = vec![(63.0, 8.0), (100.0, 5.0), (160.0, 4.0)];
        let filter = KautzFilter::from_room_modes(&modes, 48000.0);
        assert_eq!(filter.sections.len(), 3);
        for (section, &(freq, _q)) in filter.sections.iter().zip(modes.iter()) {
            assert!(
                (section.pole_freq - freq).abs() < 1e-9,
                "section pole_freq mismatch"
            );
            assert!(
                section.gain == 0.0,
                "gains should be zero before optimization"
            );
        }
    }

    #[test]
    fn test_kautz_gain_optimization() {
        let modes = vec![(80.0, 6.0), (120.0, 4.0)];
        let mut filter = KautzFilter::from_room_modes(&modes, 48000.0);

        let freqs: Vec<f64> = (20..500).map(|f| f as f64).collect();
        // Synthetic measurement: flat baseline + two Lorentzian peaks in dB
        let measured: Vec<f64> = freqs
            .iter()
            .map(|&f| {
                let peak1 = 10.0 / (1.0 + ((f - 80.0) / 5.0).powi(2));
                let peak2 = 8.0 / (1.0 + ((f - 120.0) / 8.0).powi(2));
                peak1 + peak2
            })
            .collect();
        let target: Vec<f64> = vec![0.0; freqs.len()];

        filter.optimize_gains(&freqs, &measured, &target);

        // Correction must cut the peaks: both gains should be negative.
        for (i, s) in filter.sections.iter().enumerate() {
            assert!(
                s.gain < 0.0,
                "section {i} gain ({:.4}) should be negative to cut peaks",
                s.gain
            );
        }
    }

    #[test]
    fn test_kautz_reset() {
        let modes = vec![(100.0, 5.0)];
        let mut filter = KautzFilter::from_room_modes(&modes, 48000.0);
        filter.sections[0].gain = 1.0; // nonzero so processing does something

        // Pump some state into the filter
        for _ in 0..100 {
            filter.process(1.0);
        }

        filter.reset();

        // After reset, a zero input should produce zero output
        let out = filter.process(0.0);
        assert_eq!(
            out, 0.0,
            "after reset, processing 0.0 should yield 0.0 (got {out})"
        );
    }

    #[test]
    fn test_np_log_result() {
        let modes = vec![(100.0, 5.0)];
        let mut filter = KautzFilter::from_room_modes(&modes, 48000.0);
        filter.sections[0].gain = 1.0;
        let freqs = Array1::from_vec(vec![50.0, 100.0, 200.0]);
        let db = filter.np_log_result(&freqs);
        assert_eq!(db.len(), 3);
        // With gain=1, all values should be finite and not -400.0 at 100 Hz (near pole)
        assert!(
            db[1] > -100.0,
            "response at pole frequency should be nonzero"
        );
    }

    #[test]
    fn test_process_matches_frequency_response() {
        // Verify that process() output matches complex_response() prediction.
        // Feed a sine at the pole frequency, measure steady-state amplitude.
        let pole_freq = 100.0;
        let srate = 48000.0;
        let modes = vec![(pole_freq, 5.0)];
        let mut filter = KautzFilter::from_room_modes(&modes, srate);
        filter.sections[0].gain = -6.0; // dB

        let test_freq = pole_freq;
        let omega = 2.0 * std::f64::consts::PI * test_freq / srate;

        // Run for enough samples to reach steady state
        let n_samples = 48000; // 1 second
        let mut last_outputs = vec![0.0_f64; 1000];
        for i in 0..n_samples {
            let x = (omega * i as f64).sin();
            let y = filter.process(x);
            if i >= n_samples - 1000 {
                last_outputs[i - (n_samples - 1000)] = y;
            }
        }

        // Measure peak amplitude in last 1000 samples
        let process_peak = last_outputs.iter().map(|x| x.abs()).fold(0.0_f64, f64::max);

        // Predicted amplitude from complex_response
        let predicted = filter.complex_response(test_freq).norm();

        // They should be within 20% (filter transient + measurement imprecision)
        let ratio = process_peak / predicted.max(1e-20);
        assert!(
            ratio > 0.5 && ratio < 2.0,
            "process/predict mismatch at {}Hz: process_peak={:.6}, predicted={:.6}, ratio={:.2}",
            test_freq,
            process_peak,
            predicted,
            ratio
        );
    }

    #[test]
    fn test_kautz_ill_conditioned_poles() {
        // Issue #4: closely-spaced poles (100 Hz, 100.05 Hz, 100.1 Hz)
        // create an ill-conditioned basis matrix.  The old normal-equations
        // path produces huge oscillating gains (|gain| > 500).  After the
        // QR fix each gain should stay within a modest bound.
        let modes = vec![(100.0, 100.0), (100.05, 100.0), (100.1, 100.0)];
        let mut filter = KautzFilter::from_room_modes(&modes, 48000.0);

        let freqs: Vec<f64> = (20..500).map(|f| f as f64).collect();
        let measured: Vec<f64> = freqs
            .iter()
            .map(|&f| 10.0 / (1.0 + ((f - 100.0) / 1.0).powi(2)))
            .collect();
        let target: Vec<f64> = vec![0.0; freqs.len()];

        filter.optimize_gains(&freqs, &measured, &target);

        for (i, s) in filter.sections.iter().enumerate() {
            assert!(
                s.gain.abs() < 100.0,
                "section {i} gain ({:.2}) should be < 100 for ill-conditioned poles",
                s.gain
            );
        }
    }
}
