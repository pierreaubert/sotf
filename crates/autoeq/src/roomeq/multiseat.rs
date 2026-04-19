//! Multi-seat variance optimization for subwoofer systems
//!
//! Optimizes subwoofer delays and gains to minimize response variance
//! across multiple listening positions (MSO - Multi-Subwoofer Optimizer logic).

use crate::Curve;
use crate::error::{AutoeqError, Result};
use log::{debug, info};
use ndarray::Array1;
use num_complex::Complex64;
use std::f64::consts::PI;

use super::types::{MultiSeatConfig, MultiSeatStrategy};

/// Result of multi-seat optimization
#[derive(Debug, Clone)]
pub struct MultiSeatOptimizationResult {
    /// Optimal gains for each subwoofer (dB)
    pub gains: Vec<f64>,
    /// Optimal delays for each subwoofer (ms)
    pub delays: Vec<f64>,
    /// Standard deviation across seats before optimization (dB)
    pub variance_before: f64,
    /// Standard deviation across seats after optimization (dB)
    pub variance_after: f64,
    /// Improvement in variance (dB)
    pub improvement_db: f64,
}

/// Multi-seat measurement set
///
/// Contains measurements of all subwoofers at all seat positions.
#[derive(Debug, Clone)]
pub struct MultiSeatMeasurements {
    /// Measurements indexed as \[sub_index\]\[seat_index\]
    /// Each curve is the response of one subwoofer at one seat
    pub measurements: Vec<Vec<Curve>>,
    /// Number of subwoofers
    pub num_subs: usize,
    /// Number of seats
    pub num_seats: usize,
}

impl MultiSeatMeasurements {
    /// Create from a 2D array of measurements
    pub fn new(measurements: Vec<Vec<Curve>>) -> Result<Self> {
        if measurements.is_empty() {
            return Err(AutoeqError::InvalidConfiguration {
                message: "At least one subwoofer required".to_string(),
            });
        }

        let num_subs = measurements.len();
        let num_seats = measurements[0].len();

        for (i, sub_measurements) in measurements.iter().enumerate() {
            if sub_measurements.len() != num_seats {
                return Err(AutoeqError::InvalidConfiguration {
                    message: format!(
                        "Subwoofer {} has {} seats, expected {}",
                        i,
                        sub_measurements.len(),
                        num_seats
                    ),
                });
            }
        }

        if num_seats < 2 {
            return Err(AutoeqError::InvalidConfiguration {
                message: "At least 2 seats required for multi-seat optimization".to_string(),
            });
        }

        Ok(Self {
            measurements,
            num_subs,
            num_seats,
        })
    }
}

/// Optimize subwoofer gains and delays for minimum variance across seats
///
/// # Algorithm
/// 1. Load measurements for each sub at each seat position
/// 2. For each delay/gain candidate:
///    - Compute combined response at each seat
///    - Calculate std dev of SPL across seats
/// 3. Minimize variance loss function
///
/// # Arguments
/// * `measurements` - Multi-seat measurements
/// * `config` - Multi-seat optimization configuration
/// * `freq_range` - Frequency range for optimization (min_hz, max_hz)
/// * `sample_rate` - Sample rate for filter design
///
/// # Returns
/// * Multi-seat optimization result
pub fn optimize_multiseat(
    measurements: &MultiSeatMeasurements,
    config: &MultiSeatConfig,
    freq_range: (f64, f64),
    _sample_rate: f64,
) -> Result<MultiSeatOptimizationResult> {
    let (min_freq, max_freq) = freq_range;

    // Create common frequency grid
    let freqs = create_eval_frequency_grid(measurements, min_freq, max_freq);

    // Interpolate all measurements to common grid
    let interpolated = interpolate_all_measurements(measurements, &freqs)?;

    // Initial state: no gain adjustment, no delay
    let initial_gains = vec![0.0; measurements.num_subs];
    let initial_delays = vec![0.0; measurements.num_subs];

    let variance_before = compute_seat_variance(
        &interpolated,
        &freqs,
        &initial_gains,
        &initial_delays,
        min_freq,
        max_freq,
    );

    info!(
        "  Initial variance across {} seats: {:.2} dB",
        measurements.num_seats, variance_before
    );

    // Optimize based on strategy
    let (optimal_gains, optimal_delays) = match config.strategy {
        MultiSeatStrategy::MinimizeVariance => optimize_minimize_variance(
            &interpolated,
            &freqs,
            measurements.num_subs,
            min_freq,
            max_freq,
        ),
        MultiSeatStrategy::Average => optimize_average_response(
            &interpolated,
            &freqs,
            measurements.num_subs,
            min_freq,
            max_freq,
        ),
        MultiSeatStrategy::PrimaryWithConstraints => optimize_primary_with_constraints(
            &interpolated,
            &freqs,
            measurements.num_subs,
            config.primary_seat,
            config.max_deviation_db,
            min_freq,
            max_freq,
        ),
    };

    let variance_after = compute_seat_variance(
        &interpolated,
        &freqs,
        &optimal_gains,
        &optimal_delays,
        min_freq,
        max_freq,
    );

    let improvement_db = variance_before - variance_after;

    info!(
        "  Optimized variance: {:.2} dB (improvement: {:.2} dB)",
        variance_after, improvement_db
    );

    Ok(MultiSeatOptimizationResult {
        gains: optimal_gains,
        delays: optimal_delays,
        variance_before,
        variance_after,
        improvement_db,
    })
}

/// Create a common frequency grid for evaluation
fn create_eval_frequency_grid(
    measurements: &MultiSeatMeasurements,
    min_freq: f64,
    max_freq: f64,
) -> Array1<f64> {
    // Find the common frequency range across all measurements
    let mut f_min = min_freq;
    let mut f_max = max_freq;

    for sub_measurements in &measurements.measurements {
        for curve in sub_measurements {
            f_min = f_min.max(*curve.freq.first().unwrap_or(&20.0));
            f_max = f_max.min(*curve.freq.last().unwrap_or(&20000.0));
        }
    }

    // Create log-spaced grid
    let num_points = 50; // Sufficient for sub-bass optimization
    let log_min = f_min.log10();
    let log_max = f_max.log10();

    Array1::from_shape_fn(num_points, |i| {
        let log_f = log_min + (log_max - log_min) * (i as f64 / (num_points - 1) as f64);
        10.0_f64.powf(log_f)
    })
}

/// Interpolate all measurements to a common frequency grid
fn interpolate_all_measurements(
    measurements: &MultiSeatMeasurements,
    freqs: &Array1<f64>,
) -> Result<Vec<Vec<Vec<Complex64>>>> {
    let mut result = Vec::new();

    for sub_measurements in &measurements.measurements {
        let mut sub_interp = Vec::new();
        for curve in sub_measurements {
            let interp = interpolate_curve_to_grid(curve, freqs)?;
            sub_interp.push(interp);
        }
        result.push(sub_interp);
    }

    Ok(result)
}

/// Interpolate a single curve to the common frequency grid
fn interpolate_curve_to_grid(curve: &Curve, freqs: &Array1<f64>) -> Result<Vec<Complex64>> {
    let mut result = Vec::with_capacity(freqs.len());

    for &f in freqs.iter() {
        // Find bracketing indices
        let (lower_idx, upper_idx) = find_bracket_indices(&curve.freq, f);

        // Linear interpolation for SPL
        let f_low = curve.freq[lower_idx];
        let f_high = curve.freq[upper_idx];
        let t = if f_high > f_low {
            (f - f_low) / (f_high - f_low)
        } else {
            0.0
        };

        let spl_interp = curve.spl[lower_idx] + t * (curve.spl[upper_idx] - curve.spl[lower_idx]);

        // Interpolate phase if available
        let phase_rad = if let Some(phase) = &curve.phase {
            let phase_interp = phase[lower_idx] + t * (phase[upper_idx] - phase[lower_idx]);
            phase_interp.to_radians()
        } else {
            0.0 // Assume 0 phase if not provided
        };

        let magnitude = 10.0_f64.powf(spl_interp / 20.0);
        result.push(Complex64::from_polar(magnitude, phase_rad));
    }

    Ok(result)
}

/// Find bracketing indices for interpolation
fn find_bracket_indices(freqs: &Array1<f64>, target: f64) -> (usize, usize) {
    for i in 0..freqs.len().saturating_sub(1) {
        if freqs[i] <= target && freqs[i + 1] >= target {
            return (i, i + 1);
        }
    }

    if target <= freqs[0] {
        (0, 0)
    } else {
        let last = freqs.len().saturating_sub(1);
        (last, last)
    }
}

/// Compute variance of SPL across all seats for given gains and delays
fn compute_seat_variance(
    interpolated: &[Vec<Vec<Complex64>>], // [sub][seat][freq]
    freqs: &Array1<f64>,
    gains: &[f64],
    delays: &[f64],
    min_freq: f64,
    max_freq: f64,
) -> f64 {
    let num_seats = interpolated[0].len();

    // Compute combined response at each seat
    let mut seat_responses: Vec<Vec<f64>> = Vec::new();

    for seat_idx in 0..num_seats {
        let mut combined_spl = Vec::new();

        for (freq_idx, &f) in freqs.iter().enumerate() {
            if f < min_freq || f > max_freq {
                continue;
            }

            let mut combined = Complex64::new(0.0, 0.0);

            for (sub_idx, sub_data) in interpolated.iter().enumerate() {
                let gain_linear = 10.0_f64.powf(gains[sub_idx] / 20.0);
                let delay_s = delays[sub_idx] / 1000.0;
                let omega = 2.0 * PI * f;
                let delay_phase = Complex64::from_polar(1.0, -omega * delay_s);

                combined += sub_data[seat_idx][freq_idx] * gain_linear * delay_phase;
            }

            combined_spl.push(20.0 * combined.norm().max(1e-12).log10());
        }

        seat_responses.push(combined_spl);
    }

    // Compute variance across seats at each frequency, then average
    let num_freqs = seat_responses[0].len();
    let mut total_variance = 0.0;

    for freq_idx in 0..num_freqs {
        let mut seat_spls: Vec<f64> = Vec::new();
        for seat in &seat_responses {
            seat_spls.push(seat[freq_idx]);
        }

        // Compute standard deviation
        let mean = seat_spls.iter().sum::<f64>() / seat_spls.len() as f64;
        let variance =
            seat_spls.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / seat_spls.len() as f64;
        total_variance += variance.sqrt();
    }

    total_variance / num_freqs as f64
}

/// Optimize for minimum variance across seats (grid search)
fn optimize_minimize_variance(
    interpolated: &[Vec<Vec<Complex64>>],
    freqs: &Array1<f64>,
    num_subs: usize,
    min_freq: f64,
    max_freq: f64,
) -> (Vec<f64>, Vec<f64>) {
    // Simple grid search for small number of subs
    let gain_range: Vec<f64> = (-6..=6).map(|g| g as f64).collect();
    let delay_range: Vec<f64> = (0..=20).map(|d| d as f64).collect();

    let mut best_gains = vec![0.0; num_subs];
    let mut best_delays = vec![0.0; num_subs];
    let mut best_variance = f64::INFINITY;

    // For 2 subs, do full grid search
    // For more subs, use iterative coordinate descent
    if num_subs == 2 {
        for &g1 in &gain_range {
            for &d1 in &delay_range {
                let gains = vec![0.0, g1]; // First sub is reference
                let delays = vec![0.0, d1];

                let variance =
                    compute_seat_variance(interpolated, freqs, &gains, &delays, min_freq, max_freq);

                if variance < best_variance {
                    best_variance = variance;
                    best_gains = gains;
                    best_delays = delays;
                }
            }
        }
    } else {
        // Coordinate descent for more subs
        for _ in 0..3 {
            // 3 iterations
            for sub_idx in 1..num_subs {
                // Sub 0 is reference
                for &g in &gain_range {
                    let mut test_gains = best_gains.clone();
                    test_gains[sub_idx] = g;

                    let variance = compute_seat_variance(
                        interpolated,
                        freqs,
                        &test_gains,
                        &best_delays,
                        min_freq,
                        max_freq,
                    );

                    if variance < best_variance {
                        best_variance = variance;
                        best_gains = test_gains;
                    }
                }

                for &d in &delay_range {
                    let mut test_delays = best_delays.clone();
                    test_delays[sub_idx] = d;

                    let variance = compute_seat_variance(
                        interpolated,
                        freqs,
                        &best_gains,
                        &test_delays,
                        min_freq,
                        max_freq,
                    );

                    if variance < best_variance {
                        best_variance = variance;
                        best_delays = test_delays;
                    }
                }
            }
        }
    }

    debug!(
        "  Minimize variance: gains={:?}, delays={:?}, variance={:.2}dB",
        best_gains, best_delays, best_variance
    );

    (best_gains, best_delays)
}

/// Optimize for flattest average response across seats
fn optimize_average_response(
    interpolated: &[Vec<Vec<Complex64>>],
    freqs: &Array1<f64>,
    num_subs: usize,
    min_freq: f64,
    max_freq: f64,
) -> (Vec<f64>, Vec<f64>) {
    // Similar to minimize_variance but optimize for flatness of average
    // For simplicity, use the same approach as minimize_variance
    optimize_minimize_variance(interpolated, freqs, num_subs, min_freq, max_freq)
}

/// Optimize for primary seat with constraints on other seats
fn optimize_primary_with_constraints(
    interpolated: &[Vec<Vec<Complex64>>],
    freqs: &Array1<f64>,
    num_subs: usize,
    primary_seat: usize,
    max_deviation_db: f64,
    min_freq: f64,
    max_freq: f64,
) -> (Vec<f64>, Vec<f64>) {
    // Start with minimize_variance result
    let (initial_gains, initial_delays) =
        optimize_minimize_variance(interpolated, freqs, num_subs, min_freq, max_freq);

    // TODO: Implement constrained optimization that prioritizes primary seat
    // while keeping other seats within max_deviation_db of primary

    debug!(
        "  Primary with constraints (seat {}, max dev {:.1}dB): using minimize_variance result",
        primary_seat, max_deviation_db
    );

    (initial_gains, initial_delays)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_curve(spl_offset: f64, phase_offset: f64) -> Curve {
        let freqs: Vec<f64> = (0..50)
            .map(|i| 20.0 * (200.0 / 20.0_f64).powf(i as f64 / 49.0))
            .collect();

        let spl: Vec<f64> = freqs.iter().map(|_| 90.0 + spl_offset).collect();
        let phase: Vec<f64> = freqs
            .iter()
            .map(|f| -180.0 * f / 100.0 + phase_offset)
            .collect();

        Curve {
            freq: Array1::from(freqs),
            spl: Array1::from(spl),
            phase: Some(Array1::from(phase)),
            ..Default::default()
        }
    }

    #[test]
    fn test_multiseat_measurements_creation() {
        let measurements = vec![
            vec![create_test_curve(0.0, 0.0), create_test_curve(2.0, 10.0)],
            vec![create_test_curve(-1.0, 5.0), create_test_curve(1.0, 15.0)],
        ];

        let ms = MultiSeatMeasurements::new(measurements).expect("Should create successfully");
        assert_eq!(ms.num_subs, 2);
        assert_eq!(ms.num_seats, 2);
    }

    #[test]
    fn test_multiseat_measurements_validation() {
        // Mismatched seat counts
        let measurements = vec![
            vec![create_test_curve(0.0, 0.0), create_test_curve(2.0, 10.0)],
            vec![create_test_curve(-1.0, 5.0)], // Only 1 seat
        ];

        let result = MultiSeatMeasurements::new(measurements);
        assert!(result.is_err());
    }

    #[test]
    fn test_optimize_multiseat_basic() {
        let measurements = vec![
            vec![create_test_curve(0.0, 0.0), create_test_curve(3.0, 20.0)],
            vec![create_test_curve(0.0, 10.0), create_test_curve(-2.0, 30.0)],
        ];

        let ms = MultiSeatMeasurements::new(measurements).expect("Should create");

        let config = MultiSeatConfig {
            enabled: true,
            strategy: MultiSeatStrategy::MinimizeVariance,
            primary_seat: 0,
            max_deviation_db: 6.0,
        };

        let result =
            optimize_multiseat(&ms, &config, (20.0, 120.0), 48000.0).expect("Should optimize");

        assert_eq!(result.gains.len(), 2);
        assert_eq!(result.delays.len(), 2);
        // First sub should be reference (no adjustment)
        assert_eq!(result.gains[0], 0.0);
        assert_eq!(result.delays[0], 0.0);
    }

    #[test]
    fn test_compute_seat_variance() {
        let curve1 = create_test_curve(0.0, 0.0);
        let curve2 = create_test_curve(0.0, 0.0);

        let measurements = vec![vec![curve1.clone(), curve2.clone()]];

        let ms = MultiSeatMeasurements::new(measurements).expect("Should create");
        let freqs = create_eval_frequency_grid(&ms, 30.0, 120.0);
        let interpolated = interpolate_all_measurements(&ms, &freqs).expect("Should interpolate");

        // Identical curves should have zero variance
        let variance = compute_seat_variance(&interpolated, &freqs, &[0.0], &[0.0], 30.0, 120.0);

        assert!(
            variance < 0.01,
            "Identical curves should have near-zero variance, got {}",
            variance
        );
    }
}
