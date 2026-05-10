//! Binaural transfer-matrix utilities.
//!
//! The routines here deliberately avoid roomEQ-specific concepts such as
//! speaker roles, head-position names, or artifact formats. Callers provide
//! frequency-bin matrices and receive regularized inverse filters.

use nalgebra::DMatrix;
use num_complex::Complex64;
use rustfft::FftPlanner;
use std::f64::consts::PI;

/// Frequency-domain transfer matrix for one listener/head position.
///
/// `values` is row-major with shape `num_ears x num_speakers`.
/// For binaural room correction `num_ears` is normally 2.
#[derive(Debug, Clone, PartialEq)]
pub struct TransferMatrixBin {
    pub num_ears: usize,
    pub num_speakers: usize,
    pub values: Vec<Complex64>,
}

impl TransferMatrixBin {
    pub fn new(num_ears: usize, num_speakers: usize, values: Vec<Complex64>) -> Self {
        assert_eq!(values.len(), num_ears * num_speakers);
        Self {
            num_ears,
            num_speakers,
            values,
        }
    }

    fn as_matrix(&self) -> DMatrix<Complex64> {
        DMatrix::from_row_slice(self.num_ears, self.num_speakers, &self.values)
    }
}

/// Result of one frequency-bin regularized inverse solve.
#[derive(Debug, Clone, PartialEq)]
pub struct MatrixInverseBin {
    /// Row-major correction matrix with shape `num_speakers x num_ears`.
    pub values: Vec<Complex64>,
    /// Estimated condition number of the primary transfer matrix.
    pub condition_number: f64,
    /// Mean squared reconstruction error over all supplied positions.
    pub reconstruction_error: f64,
    /// Worst-position mean squared reconstruction error.
    pub worst_position_error: f64,
}

/// Solve a regularized binaural inverse for one frequency bin.
///
/// Minimizes `sum_p ||H_p F - target||^2 + beta ||F||^2`.
/// `target` is row-major with shape `num_ears x num_ears`; pass identity for
/// ordinary cross-talk cancellation.
pub fn solve_regularized_inverse_bin(
    positions: &[TransferMatrixBin],
    target: &[Complex64],
    beta: f64,
    max_gain_db: Option<f64>,
) -> Result<MatrixInverseBin, String> {
    let weights = vec![1.0; positions.len()];
    solve_weighted_regularized_inverse_bin(positions, &weights, target, beta, max_gain_db)
}

/// Solve a weighted regularized binaural inverse for one frequency bin.
///
/// Each position contributes `weights[p] * ||H_p F - target||^2`.
pub fn solve_weighted_regularized_inverse_bin(
    positions: &[TransferMatrixBin],
    weights: &[f64],
    target: &[Complex64],
    beta: f64,
    max_gain_db: Option<f64>,
) -> Result<MatrixInverseBin, String> {
    let first = positions
        .first()
        .ok_or_else(|| "at least one transfer matrix position is required".to_string())?;
    if weights.len() != positions.len() {
        return Err(format!(
            "weights len {} != positions len {}",
            weights.len(),
            positions.len()
        ));
    }
    if target.len() != first.num_ears * first.num_ears {
        return Err(format!(
            "target has {} entries, expected {}",
            target.len(),
            first.num_ears * first.num_ears
        ));
    }
    if beta < 0.0 || !beta.is_finite() {
        return Err("beta must be finite and non-negative".to_string());
    }

    for (idx, matrix) in positions.iter().enumerate() {
        if !weights[idx].is_finite() || weights[idx] < 0.0 {
            return Err("weights must be finite and non-negative".to_string());
        }
        if matrix.num_ears != first.num_ears || matrix.num_speakers != first.num_speakers {
            return Err("all transfer matrices must have the same shape".to_string());
        }
    }

    let speakers = first.num_speakers;
    let ears = first.num_ears;
    let target_matrix = DMatrix::from_row_slice(ears, ears, target);
    let mut normal = DMatrix::<Complex64>::zeros(speakers, speakers);
    let mut rhs = DMatrix::<Complex64>::zeros(speakers, ears);

    for (matrix, weight) in positions.iter().zip(weights) {
        let h = matrix.as_matrix();
        let h_h = h.adjoint();
        let w = Complex64::new(*weight, 0.0);
        normal += (&h_h * &h) * w;
        rhs += (h_h * &target_matrix) * w;
    }

    for idx in 0..speakers {
        normal[(idx, idx)] += Complex64::new(beta, 0.0);
    }

    let mut inverse = normal
        .try_inverse()
        .ok_or_else(|| "regularized normal matrix is singular".to_string())?
        * rhs;

    if let Some(max_gain_db) = max_gain_db {
        let max_gain = 10.0_f64.powf(max_gain_db / 20.0);
        for value in inverse.iter_mut() {
            let mag = value.norm();
            if mag > max_gain && mag > 0.0 {
                *value *= max_gain / mag;
            }
        }
    }

    let mut position_errors = Vec::with_capacity(positions.len());
    for matrix in positions {
        let delivered = matrix.as_matrix() * &inverse;
        let mut position_error = 0.0;
        let mut count = 0usize;
        for row in 0..ears {
            for col in 0..ears {
                position_error += (delivered[(row, col)] - target_matrix[(row, col)]).norm_sqr();
                count += 1;
            }
        }
        position_errors.push(if count == 0 {
            0.0
        } else {
            position_error / count as f64
        });
    }

    let mut values = Vec::with_capacity(speakers * ears);
    for row in 0..speakers {
        for col in 0..ears {
            values.push(inverse[(row, col)]);
        }
    }

    let reconstruction_error = if position_errors.is_empty() {
        0.0
    } else {
        position_errors.iter().sum::<f64>() / position_errors.len() as f64
    };
    let worst_position_error = position_errors.iter().copied().fold(0.0, f64::max);

    Ok(MatrixInverseBin {
        values,
        condition_number: condition_number(first),
        reconstruction_error,
        worst_position_error,
    })
}

/// Approximate a minimax binaural inverse with iterative worst-position reweighting.
///
/// The exact minimax problem is convex but requires a constrained optimizer. This
/// routine keeps the closed-form weighted Tikhonov solve and repeatedly increases
/// weights for positions whose reconstruction error is close to the current
/// worst case. It returns the lowest worst-position solution encountered.
pub fn solve_minimax_regularized_inverse_bin(
    positions: &[TransferMatrixBin],
    target: &[Complex64],
    beta: f64,
    max_gain_db: Option<f64>,
    iterations: usize,
) -> Result<MatrixInverseBin, String> {
    if positions.is_empty() {
        return Err("at least one transfer matrix position is required".to_string());
    }
    let iterations = iterations.max(1);
    let mut weights = vec![1.0; positions.len()];
    let mut best =
        solve_weighted_regularized_inverse_bin(positions, &weights, target, beta, max_gain_db)?;

    for _ in 1..iterations {
        let errors = position_errors(positions, &best.values, target)?;
        let worst = errors.iter().copied().fold(0.0, f64::max).max(1e-18);
        for (weight, error) in weights.iter_mut().zip(errors) {
            let ratio = (error / worst).clamp(0.0, 1.0);
            *weight = (*weight * (1.0 + 2.0 * ratio * ratio)).clamp(1e-6, 1e6);
        }
        let candidate =
            solve_weighted_regularized_inverse_bin(positions, &weights, target, beta, max_gain_db)?;
        if candidate.worst_position_error < best.worst_position_error {
            best = candidate;
        }
    }

    Ok(best)
}

/// Per-position reconstruction errors for a row-major correction matrix.
pub fn position_errors(
    positions: &[TransferMatrixBin],
    correction: &[Complex64],
    target: &[Complex64],
) -> Result<Vec<f64>, String> {
    let first = positions
        .first()
        .ok_or_else(|| "at least one transfer matrix position is required".to_string())?;
    if correction.len() != first.num_speakers * first.num_ears {
        return Err(format!(
            "correction has {} entries, expected {}",
            correction.len(),
            first.num_speakers * first.num_ears
        ));
    }
    if target.len() != first.num_ears * first.num_ears {
        return Err(format!(
            "target has {} entries, expected {}",
            target.len(),
            first.num_ears * first.num_ears
        ));
    }
    let f = DMatrix::from_row_slice(first.num_speakers, first.num_ears, correction);
    let target_matrix = DMatrix::from_row_slice(first.num_ears, first.num_ears, target);
    let mut errors = Vec::with_capacity(positions.len());
    for matrix in positions {
        if matrix.num_ears != first.num_ears || matrix.num_speakers != first.num_speakers {
            return Err("all transfer matrices must have the same shape".to_string());
        }
        let delivered = matrix.as_matrix() * &f;
        let mut error = 0.0;
        let mut count = 0usize;
        for row in 0..first.num_ears {
            for col in 0..first.num_ears {
                error += (delivered[(row, col)] - target_matrix[(row, col)]).norm_sqr();
                count += 1;
            }
        }
        errors.push(if count == 0 {
            0.0
        } else {
            error / count as f64
        });
    }
    Ok(errors)
}

/// Estimate the condition number using singular values.
pub fn condition_number(matrix: &TransferMatrixBin) -> f64 {
    let svd = matrix.as_matrix().svd(false, false);
    let mut min_sv = f64::INFINITY;
    let mut max_sv = 0.0;
    for sv in svd.singular_values.iter().copied() {
        if sv > max_sv {
            max_sv = sv;
        }
        if sv > 0.0 && sv < min_sv {
            min_sv = sv;
        }
    }
    if min_sv.is_finite() && min_sv > 0.0 {
        max_sv / min_sv
    } else {
        f64::INFINITY
    }
}

/// Convert an RFFT half-spectrum into a real FIR.
///
/// `half_spectrum.len()` must be `fft_size / 2 + 1`. A non-zero
/// `bulk_delay_samples` multiplies the spectrum by `e^-jwd` before IFFT.
pub fn half_spectrum_to_fir(
    half_spectrum: &[Complex64],
    fir_taps: usize,
    bulk_delay_samples: f64,
) -> Result<Vec<f64>, String> {
    if half_spectrum.len() < 2 {
        return Err("half spectrum must contain at least DC and Nyquist".to_string());
    }
    if fir_taps == 0 {
        return Err("fir_taps must be greater than zero".to_string());
    }
    let fft_size = (half_spectrum.len() - 1) * 2;
    if fir_taps > fft_size {
        return Err(format!(
            "fir_taps ({}) cannot exceed implied fft_size ({})",
            fir_taps, fft_size
        ));
    }

    let mut spectrum = vec![Complex64::new(0.0, 0.0); fft_size];
    for (bin, value) in half_spectrum.iter().enumerate() {
        let phase = -2.0 * PI * bin as f64 * bulk_delay_samples / fft_size as f64;
        spectrum[bin] = *value * Complex64::from_polar(1.0, phase);
    }
    for bin in 1..(half_spectrum.len() - 1) {
        spectrum[fft_size - bin] = spectrum[bin].conj();
    }

    let mut planner = FftPlanner::<f64>::new();
    let fft = planner.plan_fft_inverse(fft_size);
    fft.process(&mut spectrum);

    Ok(spectrum
        .into_iter()
        .take(fir_taps)
        .map(|value| value.re / fft_size as f64)
        .collect())
}

/// Deconvolve a recorded sweep into a real impulse response.
pub fn deconvolve_sweep_to_ir(
    recording: &[f64],
    reference: &[f64],
    fft_size: usize,
) -> Result<Vec<f64>, String> {
    if recording.is_empty() || reference.is_empty() {
        return Err("recording and reference must be non-empty".to_string());
    }
    if fft_size < recording.len().max(reference.len()).next_power_of_two() {
        return Err("fft_size is too small for recording/reference".to_string());
    }
    let mut y = vec![Complex64::new(0.0, 0.0); fft_size];
    let mut x = vec![Complex64::new(0.0, 0.0); fft_size];
    for (idx, value) in recording.iter().enumerate() {
        y[idx] = Complex64::new(*value, 0.0);
    }
    for (idx, value) in reference.iter().enumerate() {
        x[idx] = Complex64::new(*value, 0.0);
    }
    let mut planner = FftPlanner::<f64>::new();
    let fft = planner.plan_fft_forward(fft_size);
    fft.process(&mut y);
    fft.process(&mut x);
    let peak = x.iter().map(|v| v.norm()).fold(0.0, f64::max).max(1e-20);
    let eps_sq = (peak * 1e-3).powi(2);
    for idx in 0..fft_size {
        let denom = x[idx].norm_sqr() + eps_sq;
        y[idx] = y[idx] * x[idx].conj() / denom;
    }
    let ifft = planner.plan_fft_inverse(fft_size);
    ifft.process(&mut y);
    Ok(y.into_iter().map(|v| v.re / fft_size as f64).collect())
}

/// Find the largest absolute sample in an impulse response.
pub fn direct_peak_sample(ir: &[f64]) -> usize {
    ir.iter()
        .enumerate()
        .max_by(|a, b| {
            a.1.abs()
                .partial_cmp(&b.1.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

/// Circularly shift an IR so `reference_peak` becomes sample zero.
pub fn align_ir_to_reference_peak(ir: &[f64], reference_peak: usize) -> Vec<f64> {
    if ir.is_empty() {
        return Vec::new();
    }
    let shift = reference_peak % ir.len();
    let mut out = Vec::with_capacity(ir.len());
    out.extend_from_slice(&ir[shift..]);
    out.extend_from_slice(&ir[..shift]);
    out
}

/// Suppress log-sweep harmonic distortion residue tails in a circularly
/// deconvolved IR.
///
/// After log-sweep deconvolution and direct-sound alignment, harmonic residues
/// appear at negative times, i.e. near the end of the circular IR. This zeros
/// short windows around the expected harmonic offsets.
pub fn suppress_log_sweep_harmonic_residues(
    ir: &mut [f64],
    sample_rate: f64,
    sweep_duration_s: f64,
    sweep_start_hz: f64,
    sweep_end_hz: f64,
    max_harmonic: usize,
    window_ms: f64,
) {
    if ir.is_empty()
        || sample_rate <= 0.0
        || sweep_duration_s <= 0.0
        || sweep_start_hz <= 0.0
        || sweep_end_hz <= sweep_start_hz
        || max_harmonic < 2
    {
        return;
    }
    let direct = direct_peak_sample(ir) as isize;
    let log_ratio = (sweep_end_hz / sweep_start_hz).ln();
    let half = ((window_ms.max(0.0) / 1000.0) * sample_rate).round() as isize;
    let len = ir.len() as isize;
    for harmonic in 2..=max_harmonic {
        let offset_s = sweep_duration_s * (harmonic as f64).ln() / log_ratio;
        let center = direct - (offset_s * sample_rate).round() as isize;
        for delta in -half..=half {
            let idx = (center + delta).rem_euclid(len) as usize;
            ir[idx] = 0.0;
        }
    }
}

/// Direct-window an IR and FFT it into a half-spectrum.
pub fn direct_windowed_half_spectrum(
    ir: &[f64],
    sample_rate: f64,
    fft_size: usize,
    start_ms: f64,
    length_ms: f64,
    fade_ms: f64,
) -> Result<Vec<Complex64>, String> {
    if fft_size == 0 || ir.is_empty() {
        return Err("ir and fft_size must be non-empty".to_string());
    }
    let mut windowed = vec![0.0; fft_size];
    let copy_len = ir.len().min(fft_size);
    windowed[..copy_len].copy_from_slice(&ir[..copy_len]);
    apply_direct_window(&mut windowed, sample_rate, start_ms, length_ms, fade_ms);
    Ok(real_fft_half_spectrum(&windowed, fft_size))
}

/// Window an IR relative to its direct-sound peak and FFT it into a half-spectrum.
///
/// Samples keep their original time index in the FFT buffer. That preserves the
/// acoustic arrival phase while avoiding absolute sample-zero windows that miss
/// delayed speaker arrivals.
pub fn direct_peak_windowed_half_spectrum(
    ir: &[f64],
    sample_rate: f64,
    fft_size: usize,
    start_ms: f64,
    length_ms: f64,
    fade_ms: f64,
) -> Result<Vec<Complex64>, String> {
    if fft_size == 0 || ir.is_empty() {
        return Err("ir and fft_size must be non-empty".to_string());
    }
    let direct_sample = direct_peak_sample(ir);
    let mut windowed = vec![0.0; fft_size];
    let copy_len = ir.len().min(fft_size);
    let start = direct_sample as isize + ((start_ms / 1000.0) * sample_rate).round() as isize;
    let length = ((length_ms / 1000.0) * sample_rate).round().max(1.0) as isize;
    let fade = ((fade_ms / 1000.0) * sample_rate).round().max(0.0) as isize;
    let end = start + length;

    for idx in 0..copy_len {
        let idx_i = idx as isize;
        if idx_i < start || idx_i >= end {
            continue;
        }
        let mut value = ir[idx];
        if fade > 0 && idx_i >= end - fade {
            let n = end - idx_i;
            let phase = PI * n as f64 / fade as f64;
            value *= 0.5 - 0.5 * phase.cos();
        }
        windowed[idx] = value;
    }

    Ok(real_fft_half_spectrum(&windowed, fft_size))
}

/// Frequency-dependent complex windowed spectrum.
pub fn fdw_complex_half_spectrum(
    ir: &[f64],
    sample_rate: f64,
    fft_size: usize,
    direct_sample: usize,
    cycles: f64,
    min_window_ms: f64,
    max_window_ms: f64,
) -> Result<Vec<Complex64>, String> {
    if ir.is_empty() || fft_size < 2 {
        return Err("ir must be non-empty and fft_size >= 2".to_string());
    }
    if sample_rate <= 0.0 || !sample_rate.is_finite() {
        return Err("sample_rate must be positive and finite".to_string());
    }
    let num_bins = fft_size / 2 + 1;
    let mut out = Vec::with_capacity(num_bins);
    for bin in 0..num_bins {
        let freq = bin as f64 * sample_rate / fft_size as f64;
        if bin == 0 {
            out.push(Complex64::new(ir.iter().sum::<f64>(), 0.0));
            continue;
        }
        let window_ms = ((cycles / freq) * 1000.0).clamp(min_window_ms, max_window_ms);
        let half = ((window_ms / 1000.0) * sample_rate * 0.5).round().max(1.0) as isize;
        let center = direct_sample.min(ir.len() - 1) as isize;
        let mut sum = Complex64::new(0.0, 0.0);
        for delta in -half..=half {
            let idx = center + delta;
            if idx < 0 || idx >= ir.len() as isize {
                continue;
            }
            let t = idx as f64 / sample_rate;
            let phase = -2.0 * PI * freq * t;
            let x = delta as f64 / half as f64;
            let w = 0.5 + 0.5 * (PI * x).cos();
            sum += Complex64::from_polar(ir[idx as usize] * w, phase);
        }
        out.push(sum);
    }
    Ok(out)
}

fn apply_direct_window(
    samples: &mut [f64],
    sample_rate: f64,
    start_ms: f64,
    length_ms: f64,
    fade_ms: f64,
) {
    let start = ((start_ms / 1000.0) * sample_rate).round().max(0.0) as usize;
    let length = ((length_ms / 1000.0) * sample_rate).round().max(1.0) as usize;
    let fade = ((fade_ms / 1000.0) * sample_rate).round().max(0.0) as usize;
    let end = (start + length).min(samples.len());
    for (idx, sample) in samples.iter_mut().enumerate() {
        if idx < start || idx >= end {
            *sample = 0.0;
            continue;
        }
        if fade > 0 && idx >= end.saturating_sub(fade) {
            let n = end - idx;
            let phase = PI * n as f64 / fade as f64;
            *sample *= 0.5 - 0.5 * phase.cos();
        }
    }
}

fn real_fft_half_spectrum(input: &[f64], fft_size: usize) -> Vec<Complex64> {
    let mut buffer = vec![Complex64::new(0.0, 0.0); fft_size];
    let copy_len = input.len().min(fft_size);
    for idx in 0..copy_len {
        buffer[idx] = Complex64::new(input[idx], 0.0);
    }
    let mut planner = FftPlanner::<f64>::new();
    let fft = planner.plan_fft_forward(fft_size);
    fft.process(&mut buffer);
    buffer.truncate(fft_size / 2 + 1);
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_solves_identity_for_well_conditioned_2x2() {
        let h = TransferMatrixBin::new(
            2,
            2,
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(0.2, 0.0),
                Complex64::new(0.15, 0.0),
                Complex64::new(0.9, 0.0),
            ],
        );
        let target = vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
        ];

        let solved = solve_regularized_inverse_bin(&[h.clone()], &target, 1e-9, None).unwrap();
        let f = DMatrix::from_row_slice(2, 2, &solved.values);
        let delivered = h.as_matrix() * f;

        assert!((delivered[(0, 0)].re - 1.0).abs() < 1e-6);
        assert!(delivered[(0, 1)].norm() < 1e-6);
        assert!(delivered[(1, 0)].norm() < 1e-6);
        assert!((delivered[(1, 1)].re - 1.0).abs() < 1e-6);
    }

    #[test]
    fn inverse_limits_large_gains() {
        let h = TransferMatrixBin::new(
            2,
            2,
            vec![
                Complex64::new(0.001, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.001, 0.0),
            ],
        );
        let target = vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
        ];

        let solved = solve_regularized_inverse_bin(&[h], &target, 1e-12, Some(6.0)).unwrap();
        let max_mag = solved.values.iter().map(|v| v.norm()).fold(0.0, f64::max);
        assert!(max_mag <= 10.0_f64.powf(6.0 / 20.0) + 1e-9);
    }

    #[test]
    fn half_spectrum_identity_yields_delayed_impulse() {
        let spectrum = vec![Complex64::new(1.0, 0.0); 9];
        let fir = half_spectrum_to_fir(&spectrum, 16, 4.0).unwrap();
        let peak = fir
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
            .map(|(idx, _)| idx)
            .unwrap();
        assert_eq!(peak, 4);
        assert!((fir[peak] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn minimax_reduces_or_matches_worst_position_error() {
        let target = vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
        ];
        let positions = vec![
            TransferMatrixBin::new(
                2,
                2,
                vec![
                    Complex64::new(1.0, 0.0),
                    Complex64::new(0.2, 0.0),
                    Complex64::new(0.2, 0.0),
                    Complex64::new(1.0, 0.0),
                ],
            ),
            TransferMatrixBin::new(
                2,
                2,
                vec![
                    Complex64::new(0.7, 0.0),
                    Complex64::new(0.45, 0.0),
                    Complex64::new(0.35, 0.0),
                    Complex64::new(0.8, 0.0),
                ],
            ),
        ];
        let average = solve_regularized_inverse_bin(&positions, &target, 0.01, Some(12.0)).unwrap();
        let minimax =
            solve_minimax_regularized_inverse_bin(&positions, &target, 0.01, Some(12.0), 8)
                .unwrap();
        assert!(minimax.worst_position_error <= average.worst_position_error + 1e-9);
    }

    #[test]
    fn deconvolution_alignment_and_harmonic_suppression_are_stable() {
        let mut reference = vec![0.0; 64];
        reference[0] = 1.0;
        let mut recording = vec![0.0; 64];
        recording[7] = 0.5;
        let ir = deconvolve_sweep_to_ir(&recording, &reference, 64).unwrap();
        assert_eq!(direct_peak_sample(&ir), 7);
        let aligned = align_ir_to_reference_peak(&ir, 7);
        assert_eq!(direct_peak_sample(&aligned), 0);

        let mut residue = vec![1.0; 128];
        suppress_log_sweep_harmonic_residues(&mut residue, 48_000.0, 1.0, 20.0, 20_000.0, 3, 1.0);
        assert!(residue.iter().any(|v| *v == 0.0));
    }

    #[test]
    fn harmonic_suppression_tracks_delayed_direct_peak() {
        let sample_rate = 1_000.0_f64;
        let duration = 1.0_f64;
        let start_hz = 10.0_f64;
        let end_hz = 1_000.0_f64;
        let harmonic = 2usize;
        let len = 512usize;
        let direct = 123usize;
        let offset = (duration * (harmonic as f64).ln() / (end_hz / start_hz).ln() * sample_rate)
            .round() as usize;
        let residue = (direct + len - offset % len) % len;

        let mut ir = vec![0.0; len];
        ir[direct] = 1.0;
        ir[residue] = 0.5;
        suppress_log_sweep_harmonic_residues(
            &mut ir,
            sample_rate,
            duration,
            start_hz,
            end_hz,
            harmonic,
            0.0,
        );

        assert_eq!(ir[residue], 0.0);
        assert_eq!(ir[direct], 1.0);
    }

    #[test]
    fn fdw_complex_half_spectrum_returns_fft_bins() {
        let mut ir = vec![0.0; 128];
        ir[8] = 1.0;
        let spectrum = fdw_complex_half_spectrum(&ir, 48_000.0, 128, 8, 8.0, 3.0, 30.0).unwrap();
        assert_eq!(spectrum.len(), 65);
        assert!(spectrum[1].norm() > 0.0);
    }
}
