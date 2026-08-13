use super::consts::{ISO_BAND_FREQS, ISO_BAND_QS, ISO_FILTER_COUNT};
use super::iso226::ISO226_NUM_FREQS;
use math_audio_iir_fir::{Biquad, BiquadFilterType};

pub(super) fn safe_frequency(freq: f64, sample_rate: f64) -> f64 {
    freq.clamp(1.0, sample_rate * 0.45)
}

pub(super) fn band_type(index: usize) -> BiquadFilterType {
    if index == 0 {
        BiquadFilterType::Lowshelf
    } else if index == ISO_FILTER_COUNT - 1 {
        BiquadFilterType::Highshelf
    } else {
        BiquadFilterType::Peak
    }
}

/// Joint least-squares fit of the complete cascade to all ISO 226 table points
/// below the safe design limit. A small ridge term keeps the solution bounded
/// when low sample rates make adjacent high-frequency bases nearly collinear.
pub(super) fn fit_iso_gains(
    deltas: &[(f64, f64); ISO226_NUM_FREQS],
    sample_rate: f64,
) -> [f64; ISO_FILTER_COUNT] {
    let mut normal = [[0.0; ISO_FILTER_COUNT]; ISO_FILTER_COUNT];
    let mut rhs = [0.0; ISO_FILTER_COUNT];
    let max_freq = sample_rate * 0.45;

    for &(frequency, target) in deltas {
        if frequency > max_freq {
            continue;
        }
        let mut basis = [0.0; ISO_FILTER_COUNT];
        for band in 0..ISO_FILTER_COUNT {
            let filter = Biquad::new(
                band_type(band),
                safe_frequency(ISO_BAND_FREQS[band], sample_rate),
                sample_rate,
                ISO_BAND_QS[band],
                1.0,
            );
            basis[band] = filter.log_result(frequency);
        }
        let weight = if (frequency - 1000.0).abs() < 1.0 {
            8.0
        } else {
            1.0
        };
        for row in 0..ISO_FILTER_COUNT {
            rhs[row] += weight * basis[row] * target;
            for col in 0..ISO_FILTER_COUNT {
                normal[row][col] += weight * basis[row] * basis[col];
            }
        }
    }
    for (index, row) in normal.iter_mut().enumerate() {
        row[index] += 5.0e-2;
    }
    let mut gains = solve(normal, rhs).map(|gain| gain.clamp(-30.0, 30.0));
    // Robust bounded coordinate refinement against the actual nonlinear RBJ
    // mapping. It runs only for control updates, never in the callback.
    let mut best = fit_error(&gains, deltas, sample_rate);
    for step in [8.0, 4.0, 2.0, 1.0, 0.5, 0.25, 0.1, 0.05] {
        for _ in 0..4 {
            for band in 0..ISO_FILTER_COUNT {
                for direction in [-1.0, 1.0] {
                    let original = gains[band];
                    let candidate = (original + direction * step).clamp(-30.0, 30.0);
                    gains[band] = candidate;
                    let error = fit_error(&gains, deltas, sample_rate);
                    if error < best {
                        best = error;
                    } else {
                        gains[band] = original;
                    }
                }
            }
        }
    }
    gains
}

fn fit_error(
    gains: &[f64; ISO_FILTER_COUNT],
    deltas: &[(f64, f64); ISO226_NUM_FREQS],
    sample_rate: f64,
) -> f64 {
    let mut error = 0.0;
    for &(frequency, target) in deltas {
        if frequency > sample_rate * 0.45 {
            continue;
        }
        let mut realized = 0.0;
        for band in 0..ISO_FILTER_COUNT {
            realized += Biquad::new(
                band_type(band),
                safe_frequency(ISO_BAND_FREQS[band], sample_rate),
                sample_rate,
                ISO_BAND_QS[band],
                gains[band],
            )
            .log_result(frequency);
        }
        let weight = if (frequency - 1000.0).abs() < 1.0 {
            8.0
        } else {
            1.0
        };
        error += weight * (realized - target).powi(2);
    }
    error + gains.iter().map(|gain| 0.05 * gain * gain).sum::<f64>()
}

fn solve(
    mut matrix: [[f64; ISO_FILTER_COUNT]; ISO_FILTER_COUNT],
    mut rhs: [f64; ISO_FILTER_COUNT],
) -> [f64; ISO_FILTER_COUNT] {
    for pivot in 0..ISO_FILTER_COUNT {
        let mut best = pivot;
        for row in pivot + 1..ISO_FILTER_COUNT {
            if matrix[row][pivot].abs() > matrix[best][pivot].abs() {
                best = row;
            }
        }
        if best != pivot {
            matrix.swap(best, pivot);
            rhs.swap(best, pivot);
        }
        let divisor = matrix[pivot][pivot];
        if divisor.abs() < 1.0e-12 {
            continue;
        }
        for value in &mut matrix[pivot][pivot..] {
            *value /= divisor;
        }
        rhs[pivot] /= divisor;
        let pivot_row = matrix[pivot];
        for row in 0..ISO_FILTER_COUNT {
            if row == pivot {
                continue;
            }
            let factor = matrix[row][pivot];
            for (value, pivot_value) in matrix[row][pivot..].iter_mut().zip(&pivot_row[pivot..]) {
                *value -= factor * pivot_value;
            }
            rhs[row] -= factor * rhs[pivot];
        }
    }
    rhs
}
