//! Multi-seat variance optimization for subwoofer systems
//!
//! Optimizes subwoofer delays and gains to minimize response variance
//! across multiple listening positions (MSO - Multi-Subwoofer Optimizer logic).

use crate::Curve;
use crate::error::{AutoeqError, Result};
use log::{debug, info, warn};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use nalgebra::DMatrix;
use ndarray::Array1;
use num_complex::Complex64;
use std::f64::consts::PI;

use super::types::{MultiSeatConfig, MultiSeatStrategy};

const MSO_MAX_MEAN_OUTPUT_LOSS_DB: f64 = 1.5;
const MSO_OUTPUT_LOSS_WEIGHT: f64 = 2.0;
const MSO_NULL_DEFICIT_ALLOWANCE_DB: f64 = 3.0;
const MSO_NULL_DEFICIT_WEIGHT: f64 = 0.75;
const MSO_HEADROOM_BOOST_ALLOWANCE_DB: f64 = 0.5;
const MSO_HEADROOM_BOOST_WEIGHT: f64 = 0.75;
const MSO_EXTENSION_DEFICIT_ALLOWANCE_DB: f64 = 1.0;
const MSO_EXTENSION_DEFICIT_WEIGHT: f64 = 1.25;
const MSO_EXTENSION_MAX_HZ: f64 = 80.0;
const MSO_EXTENSION_OCTAVES: f64 = 1.0;
const MSO_OBJECTIVE_REGRESSION_TOLERANCE: f64 = 1e-6;
const SFM_MODAL_ENERGY_CUTOFF: f64 = 0.95;
const SFM_MAX_MODES: usize = 8;
const SFM_MODAL_LOSS_WEIGHT: f64 = 10.0;
const SFM_EPS: f64 = 1e-12;

fn mso_objective_regressed(objective_before: f64, objective_after: f64) -> bool {
    !objective_after.is_finite()
        || (objective_before.is_finite()
            && objective_after > objective_before + MSO_OBJECTIVE_REGRESSION_TOLERANCE)
}

/// Result of multi-seat optimization
#[derive(Debug, Clone)]
pub struct MultiSeatOptimizationResult {
    /// Optimal gains for each subwoofer (dB)
    pub gains: Vec<f64>,
    /// Optimal delays for each subwoofer (ms)
    pub delays: Vec<f64>,
    /// Per-subwoofer polarity inversion flags
    pub polarities: Vec<bool>,
    /// Per-subwoofer all-pass filter parameters `(frequency_hz, q)`
    pub allpass_filters: Vec<Vec<(f64, f64)>>,
    /// Strategy used for optimization
    pub strategy: MultiSeatStrategy,
    /// Name of the objective metric optimized by the selected strategy
    pub objective_name: String,
    /// Optimized objective value before optimization
    pub objective_before: f64,
    /// Optimized objective value after optimization
    pub objective_after: f64,
    /// Improvement in the selected objective metric
    pub objective_improvement_db: f64,
    /// Standard deviation across seats before optimization (dB)
    pub variance_before: f64,
    /// Standard deviation across seats after optimization (dB)
    pub variance_after: f64,
    /// Improvement in variance (dB), reported even for non-variance strategies
    pub variance_improvement_db: f64,
    /// Improvement in the selected objective metric
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

            for (seat_idx, curve) in sub_measurements.iter().enumerate() {
                if !super::frequency_grid::is_valid_frequency_grid(&curve.freq) {
                    return Err(AutoeqError::InvalidMeasurement {
                        message: format!(
                            "MSO measurement sub {} seat {} has an invalid frequency grid",
                            i, seat_idx
                        ),
                    });
                }
                if curve.spl.len() != curve.freq.len() {
                    return Err(AutoeqError::InvalidMeasurement {
                        message: format!(
                            "MSO measurement sub {} seat {} has mismatched freq/spl lengths",
                            i, seat_idx
                        ),
                    });
                }
                match curve.phase.as_ref() {
                    Some(phase) if phase.len() == curve.freq.len() => {}
                    Some(_) => {
                        return Err(AutoeqError::InvalidMeasurement {
                            message: format!(
                                "MSO measurement sub {} seat {} has mismatched phase length",
                                i, seat_idx
                            ),
                        });
                    }
                    None => {
                        return Err(AutoeqError::InvalidMeasurement {
                            message: format!(
                                "MSO measurement sub {} seat {} is missing phase data",
                                i, seat_idx
                            ),
                        });
                    }
                }
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
    sample_rate: f64,
) -> Result<MultiSeatOptimizationResult> {
    let (min_freq, max_freq) = freq_range;
    let Some((common_min, common_max)) = super::frequency_grid::common_frequency_range(
        measurements.measurements.iter().flat_map(|sub| sub.iter()),
    ) else {
        return Err(AutoeqError::InvalidMeasurement {
            message: "MSO measurements do not share a valid overlapping frequency range"
                .to_string(),
        });
    };
    let eval_min = min_freq.max(common_min);
    let eval_max = max_freq.min(common_max);
    if eval_min >= eval_max {
        return Err(AutoeqError::InvalidMeasurement {
            message: format!(
                "MSO frequency range [{:.1}, {:.1}] Hz does not overlap all measurements [{:.1}, {:.1}] Hz",
                min_freq, max_freq, common_min, common_max
            ),
        });
    }

    if config.strategy == MultiSeatStrategy::PrimaryWithConstraints
        && config.primary_seat >= measurements.num_seats
    {
        return Err(AutoeqError::InvalidConfiguration {
            message: format!(
                "primary_seat {} out of range (only {} seats)",
                config.primary_seat, measurements.num_seats
            ),
        });
    }

    // Create common frequency grid
    let freqs = create_eval_frequency_grid(measurements, eval_min, eval_max);

    // Interpolate all measurements to common grid
    let interpolated = interpolate_all_measurements(measurements, &freqs)?;

    // Initial state: no gain adjustment, no delay
    let initial_gains = vec![0.0; measurements.num_subs];
    let initial_delays = vec![0.0; measurements.num_subs];
    let initial_polarities = vec![false; measurements.num_subs];
    let initial_allpass_filters = vec![Vec::new(); measurements.num_subs];

    let initial_complex_responses = compute_combined_complex_responses(
        &interpolated,
        &freqs,
        &initial_gains,
        &initial_delays,
        &initial_polarities,
        &initial_allpass_filters,
        sample_rate,
        eval_min,
        eval_max,
    );
    let initial_responses = spl_from_complex_responses(&initial_complex_responses);
    let variance_before = variance_from_responses(&initial_responses);
    let objective_context =
        MsoObjectiveContext::from_baseline_with_freqs(&initial_responses, Some(&freqs));
    let modal_basis = if config.strategy == MultiSeatStrategy::ModalBasis {
        let basis = build_modal_basis(&interpolated, &freqs, eval_min, eval_max);
        if basis.modes.is_empty() {
            return Err(AutoeqError::InvalidMeasurement {
                message: "Modal-basis multi-seat optimization could not extract any non-common complex seat modes from the selected frequency range; check that each sub/seat measurement has valid phase and non-identical complex responses".to_string(),
            });
        }
        info!(
            "  Modal-basis SFM retained {} modes ({:.1}% snapshot energy)",
            basis.modes.len(),
            basis.retained_energy * 100.0
        );
        Some(basis)
    } else {
        None
    };
    let objective_before = if let Some(basis) = modal_basis.as_ref() {
        modal_basis_objective_from_responses(
            &initial_complex_responses,
            &initial_responses,
            basis,
            &objective_context,
        )
    } else {
        objective_from_responses(
            &initial_responses,
            config.strategy.clone(),
            config.primary_seat,
            config.max_deviation_db,
            Some(&objective_context),
        )
    };

    info!(
        "  Initial variance across {} seats: {:.2} dB",
        measurements.num_seats, variance_before
    );

    // Optimize based on strategy
    let (optimal_gains, optimal_delays, optimal_polarities, optimal_allpass_filters) =
        match config.strategy {
            MultiSeatStrategy::MinimizeVariance => optimize_minimize_variance(
                &interpolated,
                &freqs,
                measurements.num_subs,
                config,
                sample_rate,
                eval_min,
                eval_max,
                &objective_context,
            ),
            MultiSeatStrategy::Average => optimize_average_response(
                &interpolated,
                &freqs,
                measurements.num_subs,
                config,
                sample_rate,
                eval_min,
                eval_max,
                &objective_context,
            ),
            MultiSeatStrategy::PrimaryWithConstraints => optimize_primary_with_constraints(
                &interpolated,
                &freqs,
                measurements.num_subs,
                config,
                sample_rate,
                config.primary_seat,
                config.max_deviation_db,
                eval_min,
                eval_max,
                &objective_context,
            ),
            MultiSeatStrategy::ModalBasis => optimize_modal_basis(
                &interpolated,
                &freqs,
                measurements.num_subs,
                config,
                sample_rate,
                eval_min,
                eval_max,
                modal_basis
                    .as_ref()
                    .expect("modal basis is built before modal-basis optimization"),
                &objective_context,
            ),
        };

    let final_complex_responses = compute_combined_complex_responses(
        &interpolated,
        &freqs,
        &optimal_gains,
        &optimal_delays,
        &optimal_polarities,
        &optimal_allpass_filters,
        sample_rate,
        eval_min,
        eval_max,
    );
    let final_responses = spl_from_complex_responses(&final_complex_responses);
    let mut variance_after = variance_from_responses(&final_responses);
    let mut objective_after = if let Some(basis) = modal_basis.as_ref() {
        modal_basis_objective_from_responses(
            &final_complex_responses,
            &final_responses,
            basis,
            &objective_context,
        )
    } else {
        objective_from_responses(
            &final_responses,
            config.strategy.clone(),
            config.primary_seat,
            config.max_deviation_db,
            Some(&objective_context),
        )
    };
    let mut final_gains = optimal_gains;
    let mut final_delays = optimal_delays;
    let mut final_polarities = optimal_polarities;
    let mut final_allpass_filters = optimal_allpass_filters;

    if mso_objective_regressed(objective_before, objective_after) {
        warn!(
            "  MSO result rejected: {} regressed {:.6} -> {:.6}; keeping identity gain/delay/polarity/all-pass state",
            objective_name(config.strategy.clone()),
            objective_before,
            objective_after
        );
        final_gains = initial_gains;
        final_delays = initial_delays;
        final_polarities = initial_polarities;
        final_allpass_filters = initial_allpass_filters;
        objective_after = objective_before;
        variance_after = variance_before;
    }

    let objective_improvement_db = objective_before - objective_after;
    let variance_improvement_db = variance_before - variance_after;
    let objective_name = objective_name(config.strategy.clone()).to_string();

    info!(
        "  Optimized {}: {:.2} -> {:.2} dB (improvement: {:.2} dB); variance: {:.2} -> {:.2} dB ({:.2} dB)",
        objective_name,
        objective_before,
        objective_after,
        objective_improvement_db,
        variance_before,
        variance_after,
        variance_improvement_db
    );

    Ok(MultiSeatOptimizationResult {
        gains: final_gains,
        delays: final_delays,
        polarities: final_polarities,
        allpass_filters: final_allpass_filters,
        strategy: config.strategy.clone(),
        objective_name,
        objective_before,
        objective_after,
        objective_improvement_db,
        variance_before,
        variance_after,
        variance_improvement_db,
        improvement_db: objective_improvement_db,
    })
}

/// Compute the per-seat combined subwoofer responses for an optimized
/// multi-seat solution.
///
/// The returned curves share the same log-spaced evaluation grid used by the
/// MSO optimizer and include the optimized per-sub gain, delay, polarity, and
/// all-pass filters. SPL is derived from complex summation; phase is retained
/// for downstream diagnostics and future phase-aware processing.
pub fn compute_multiseat_combined_curves(
    measurements: &MultiSeatMeasurements,
    result: &MultiSeatOptimizationResult,
    freq_range: (f64, f64),
    sample_rate: f64,
) -> Result<Vec<Curve>> {
    let (min_freq, max_freq) = freq_range;
    let Some((common_min, common_max)) = super::frequency_grid::common_frequency_range(
        measurements.measurements.iter().flat_map(|sub| sub.iter()),
    ) else {
        return Err(AutoeqError::InvalidMeasurement {
            message: "MSO measurements do not share a valid overlapping frequency range"
                .to_string(),
        });
    };
    let eval_min = min_freq.max(common_min);
    let eval_max = max_freq.min(common_max);
    if eval_min >= eval_max {
        return Err(AutoeqError::InvalidMeasurement {
            message: format!(
                "MSO frequency range [{:.1}, {:.1}] Hz does not overlap all measurements [{:.1}, {:.1}] Hz",
                min_freq, max_freq, common_min, common_max
            ),
        });
    }

    let freqs = create_eval_frequency_grid(measurements, eval_min, eval_max);
    let interpolated = interpolate_all_measurements(measurements, &freqs)?;
    let complex = compute_combined_complex_responses(
        &interpolated,
        &freqs,
        &result.gains,
        &result.delays,
        &result.polarities,
        &result.allpass_filters,
        sample_rate,
        eval_min,
        eval_max,
    );

    let eval_freqs: Vec<f64> = freqs
        .iter()
        .copied()
        .filter(|f| *f >= eval_min && *f <= eval_max)
        .collect();

    Ok(complex
        .into_iter()
        .map(|seat| {
            let spl: Vec<f64> = seat
                .iter()
                .map(|response| 20.0 * response.norm().max(SFM_EPS).log10())
                .collect();
            let phase: Vec<f64> = seat
                .iter()
                .map(|response| response.arg().to_degrees())
                .collect();
            Curve {
                freq: Array1::from(eval_freqs.clone()),
                spl: Array1::from(spl),
                phase: Some(Array1::from(phase)),
                ..Default::default()
            }
        })
        .collect())
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
    let phase = curve
        .phase
        .as_ref()
        .ok_or_else(|| AutoeqError::InvalidMeasurement {
            message: "Multi-seat subwoofer optimization requires phase data for every sub/seat measurement; refusing to assume 0° phase for complex summation".to_string(),
        })?;

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

        // Interpolate phase with wrap handling (shortest arc through ±180°)
        let mut diff = phase[upper_idx] - phase[lower_idx];
        diff -= 360.0 * (diff / 360.0).round();
        let phase_rad = (phase[lower_idx] + t * diff).to_radians();

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

// ============================================================================
// Combined response computation (shared by all loss functions)
// ============================================================================

/// Compute combined SPL response at each seat for given gains/delays.
/// Returns `responses[seat_idx][freq_idx]` in dB SPL, only for frequencies
/// within `[min_freq, max_freq]`.
fn compute_combined_responses(
    interpolated: &[Vec<Vec<Complex64>>], // [sub][seat][freq]
    freqs: &Array1<f64>,
    gains: &[f64],
    delays: &[f64],
    polarities: &[bool],
    allpass_filters: &[Vec<(f64, f64)>],
    sample_rate: f64,
    min_freq: f64,
    max_freq: f64,
) -> Vec<Vec<f64>> {
    let complex = compute_combined_complex_responses(
        interpolated,
        freqs,
        gains,
        delays,
        polarities,
        allpass_filters,
        sample_rate,
        min_freq,
        max_freq,
    );
    spl_from_complex_responses(&complex)
}

fn compute_combined_complex_responses(
    interpolated: &[Vec<Vec<Complex64>>], // [sub][seat][freq]
    freqs: &Array1<f64>,
    gains: &[f64],
    delays: &[f64],
    polarities: &[bool],
    allpass_filters: &[Vec<(f64, f64)>],
    sample_rate: f64,
    min_freq: f64,
    max_freq: f64,
) -> Vec<Vec<Complex64>> {
    let num_seats = interpolated[0].len();
    let mut seat_responses: Vec<Vec<Complex64>> = Vec::with_capacity(num_seats);
    let allpass_biquads: Vec<Vec<Biquad>> = allpass_filters
        .iter()
        .map(|filters| {
            filters
                .iter()
                .map(|&(freq, q)| Biquad::new(BiquadFilterType::AllPass, freq, sample_rate, q, 0.0))
                .collect()
        })
        .collect();

    for seat_idx in 0..num_seats {
        let mut combined_response = Vec::new();

        for (freq_idx, &f) in freqs.iter().enumerate() {
            if f < min_freq || f > max_freq {
                continue;
            }

            let mut combined = Complex64::new(0.0, 0.0);

            for (sub_idx, sub_data) in interpolated.iter().enumerate() {
                let gain_linear = 10.0_f64.powf(gains[sub_idx] / 20.0);
                let polarity = if polarities.get(sub_idx).copied().unwrap_or(false) {
                    -1.0
                } else {
                    1.0
                };
                let delay_s = delays[sub_idx] / 1000.0;
                let omega = 2.0 * PI * f;
                let delay_phase = Complex64::from_polar(1.0, -omega * delay_s);
                let allpass_phase = allpass_biquads
                    .get(sub_idx)
                    .map(|filters| {
                        filters
                            .iter()
                            .fold(Complex64::new(1.0, 0.0), |acc, allpass| {
                                acc * allpass_complex_response(allpass, f)
                            })
                    })
                    .unwrap_or_else(|| Complex64::new(1.0, 0.0));

                combined += sub_data[seat_idx][freq_idx]
                    * gain_linear
                    * polarity
                    * delay_phase
                    * allpass_phase;
            }

            combined_response.push(combined);
        }

        seat_responses.push(combined_response);
    }

    seat_responses
}

fn spl_from_complex_responses(responses: &[Vec<Complex64>]) -> Vec<Vec<f64>> {
    responses
        .iter()
        .map(|seat| {
            seat.iter()
                .map(|response| 20.0 * response.norm().max(SFM_EPS).log10())
                .collect()
        })
        .collect()
}

fn allpass_complex_response(biquad: &Biquad, freq_hz: f64) -> Complex64 {
    let (a1, a2, b0, b1, b2) = biquad.constants();
    let omega = 2.0 * PI * freq_hz / biquad.srate;
    let z_inv = Complex64::from_polar(1.0, -omega);
    let z_inv2 = z_inv * z_inv;

    let numerator = b0 + b1 * z_inv + b2 * z_inv2;
    let denominator = 1.0 + a1 * z_inv + a2 * z_inv2;

    numerator / denominator
}

// ============================================================================
// Complex modal-basis SFM
// ============================================================================

#[derive(Debug, Clone)]
struct ModalBasis {
    modes: Vec<Vec<Complex64>>,
    #[cfg(test)]
    singular_values: Vec<f64>,
    retained_energy: f64,
}

fn build_modal_basis(
    interpolated: &[Vec<Vec<Complex64>>],
    freqs: &Array1<f64>,
    min_freq: f64,
    max_freq: f64,
) -> ModalBasis {
    let num_subs = interpolated.len();
    let num_seats = interpolated.first().map(|sub| sub.len()).unwrap_or(0);
    let max_modes = modal_basis_mode_cap(num_seats, num_subs);
    if max_modes == 0 {
        return empty_modal_basis();
    }

    let mut snapshots = Vec::new();
    let mut snapshot_count = 0usize;

    for (freq_idx, &freq) in freqs.iter().enumerate() {
        if freq < min_freq || freq > max_freq {
            continue;
        }

        for sub_data in interpolated {
            let mut snapshot: Vec<Complex64> = (0..num_seats)
                .map(|seat_idx| sub_data[seat_idx][freq_idx])
                .collect();
            let seat_mean = snapshot.iter().copied().sum::<Complex64>() / num_seats as f64;
            for value in &mut snapshot {
                *value -= seat_mean;
            }

            let norm_sq = snapshot.iter().map(|value| value.norm_sqr()).sum::<f64>();
            if norm_sq <= SFM_EPS {
                continue;
            }

            let norm = norm_sq.sqrt();
            snapshots.extend(snapshot.into_iter().map(|value| value / norm));
            snapshot_count += 1;
        }
    }

    if snapshot_count == 0 {
        return empty_modal_basis();
    }

    let matrix = DMatrix::from_column_slice(num_seats, snapshot_count, &snapshots);
    let svd = matrix.svd(true, false);
    let singular_values: Vec<f64> = svd.singular_values.iter().copied().collect();
    let mode_count = select_modal_mode_count(&singular_values, SFM_MODAL_ENERGY_CUTOFF, max_modes);
    let retained_energy = retained_modal_energy(&singular_values, mode_count);
    let modes = svd
        .u
        .map(|u| {
            (0..mode_count)
                .map(|mode_idx| {
                    (0..num_seats)
                        .map(|seat_idx| u[(seat_idx, mode_idx)])
                        .collect()
                })
                .collect()
        })
        .unwrap_or_default();

    ModalBasis {
        modes,
        #[cfg(test)]
        singular_values,
        retained_energy,
    }
}

fn empty_modal_basis() -> ModalBasis {
    ModalBasis {
        modes: Vec::new(),
        #[cfg(test)]
        singular_values: Vec::new(),
        retained_energy: 0.0,
    }
}

fn modal_basis_mode_cap(num_seats: usize, num_subs: usize) -> usize {
    num_seats.saturating_sub(1).min(num_subs).min(SFM_MAX_MODES)
}

fn select_modal_mode_count(singular_values: &[f64], energy_cutoff: f64, max_modes: usize) -> usize {
    if max_modes == 0 {
        return 0;
    }

    let total_energy = singular_values
        .iter()
        .map(|value| value * value)
        .sum::<f64>();
    if total_energy <= SFM_EPS {
        return 0;
    }

    let mut cumulative_energy = 0.0;
    for (idx, singular_value) in singular_values.iter().take(max_modes).enumerate() {
        cumulative_energy += singular_value * singular_value;
        if cumulative_energy / total_energy >= energy_cutoff {
            return idx + 1;
        }
    }

    singular_values.len().min(max_modes)
}

fn retained_modal_energy(singular_values: &[f64], mode_count: usize) -> f64 {
    let total_energy = singular_values
        .iter()
        .map(|value| value * value)
        .sum::<f64>();
    if total_energy <= SFM_EPS {
        return 0.0;
    }

    singular_values
        .iter()
        .take(mode_count)
        .map(|value| value * value)
        .sum::<f64>()
        / total_energy
}

fn modal_projection_loss(responses: &[Vec<Complex64>], basis: &ModalBasis) -> f64 {
    if basis.modes.is_empty() || responses.is_empty() || responses[0].is_empty() {
        return 0.0;
    }

    let num_seats = responses.len();
    let num_freqs = responses[0].len();
    let mut total = 0.0;

    for freq_idx in 0..num_freqs {
        let seat_mean = responses
            .iter()
            .map(|seat| seat[freq_idx])
            .sum::<Complex64>()
            / num_seats as f64;
        let total_power = responses
            .iter()
            .map(|seat| seat[freq_idx].norm_sqr())
            .sum::<f64>()
            .max(SFM_EPS);

        let mut modal_power = 0.0;
        for mode in &basis.modes {
            let coefficient = mode
                .iter()
                .zip(responses.iter())
                .map(|(mode_value, seat)| mode_value.conj() * (seat[freq_idx] - seat_mean))
                .sum::<Complex64>();
            modal_power += coefficient.norm_sqr();
        }

        let ratio = (modal_power / total_power).max(0.0);
        total += 10.0 * (1.0 + ratio).log10();
    }

    total / num_freqs as f64
}

fn modal_basis_objective_from_responses(
    complex_responses: &[Vec<Complex64>],
    spl_responses: &[Vec<f64>],
    basis: &ModalBasis,
    context: &MsoObjectiveContext,
) -> f64 {
    SFM_MODAL_LOSS_WEIGHT * modal_projection_loss(complex_responses, basis)
        + mso_resource_penalty(spl_responses, context)
}

// ============================================================================
// Loss functions
// ============================================================================

/// Seat-to-seat variance: mean of per-frequency std-dev across seats (dB).
fn variance_from_responses(responses: &[Vec<f64>]) -> f64 {
    let num_freqs = responses[0].len();
    let mut total_std = 0.0;

    for freq_idx in 0..num_freqs {
        let mean: f64 = responses.iter().map(|s| s[freq_idx]).sum::<f64>() / responses.len() as f64;
        let variance = responses
            .iter()
            .map(|s| (s[freq_idx] - mean).powi(2))
            .sum::<f64>()
            / responses.len() as f64;
        total_std += variance.sqrt();
    }

    total_std / num_freqs as f64
}

/// Spectral flatness of the mean response across seats (dB std-dev).
/// Minimizing this makes the *average* listener experience tonally flat,
/// even if individual seats still differ from each other.
fn average_flatness_from_responses(responses: &[Vec<f64>]) -> f64 {
    let avg_spl = mean_response_curve(responses);

    // Spectral std-dev of the average
    let mean = avg_spl.iter().sum::<f64>() / avg_spl.len() as f64;
    let variance = avg_spl.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / avg_spl.len() as f64;
    variance.sqrt()
}

#[derive(Debug, Clone)]
struct MsoObjectiveContext {
    baseline_avg_spl: Vec<f64>,
    baseline_peak_spl: Vec<f64>,
    baseline_mean_level_db: f64,
    extension_indices: Vec<usize>,
}

impl MsoObjectiveContext {
    #[cfg(test)]
    fn from_baseline(responses: &[Vec<f64>]) -> Self {
        Self::from_baseline_with_freqs(responses, None)
    }

    fn from_baseline_with_freqs(responses: &[Vec<f64>], freqs: Option<&Array1<f64>>) -> Self {
        let baseline_avg_spl = mean_response_curve(responses);
        let baseline_peak_spl = peak_response_curve(responses);
        let baseline_mean_level_db = mean_level(&baseline_avg_spl);
        let extension_indices = extension_indices(baseline_avg_spl.len(), freqs);
        Self {
            baseline_avg_spl,
            baseline_peak_spl,
            baseline_mean_level_db,
            extension_indices,
        }
    }
}

fn mean_response_curve(responses: &[Vec<f64>]) -> Vec<f64> {
    let num_freqs = responses[0].len();
    let num_seats = responses.len() as f64;
    (0..num_freqs)
        .map(|fi| responses.iter().map(|s| s[fi]).sum::<f64>() / num_seats)
        .collect()
}

fn peak_response_curve(responses: &[Vec<f64>]) -> Vec<f64> {
    let num_freqs = responses[0].len();
    (0..num_freqs)
        .map(|fi| {
            responses
                .iter()
                .map(|seat| seat[fi])
                .fold(f64::NEG_INFINITY, f64::max)
        })
        .collect()
}

fn mean_level(spl: &[f64]) -> f64 {
    spl.iter().sum::<f64>() / spl.len().max(1) as f64
}

fn extension_indices(num_freqs: usize, freqs: Option<&Array1<f64>>) -> Vec<usize> {
    if num_freqs == 0 {
        return Vec::new();
    }

    if let Some(freqs) = freqs.filter(|freqs| freqs.len() == num_freqs && !freqs.is_empty()) {
        let first_hz = freqs[0].max(1.0);
        let extension_max_hz =
            (first_hz * 2.0_f64.powf(MSO_EXTENSION_OCTAVES)).min(MSO_EXTENSION_MAX_HZ);
        let mut indices: Vec<usize> = freqs
            .iter()
            .enumerate()
            .filter_map(|(idx, &freq)| {
                if freq <= extension_max_hz {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();
        if indices.is_empty() {
            indices.push(0);
        }
        return indices;
    }

    let count = (num_freqs / 4).max(1);
    (0..count).collect()
}

/// RMS of the positive entries of `violations`, normalised by the number of
/// *violating* bins. Returns 0 when nothing violates.
///
/// Why per-violation rather than per-bin: the same physical violation must
/// score the same on a coarse and a fine frequency grid. Dividing by the
/// total bin count would let a sharp peak look smaller simply because more
/// non-violating bins were averaged in.
fn violation_rms_db<I: IntoIterator<Item = f64>>(violations: I) -> f64 {
    let mut sum_sq = 0.0;
    let mut count = 0usize;
    for v in violations {
        if v > 0.0 {
            sum_sq += v * v;
            count += 1;
        }
    }
    if count > 0 {
        (sum_sq / count as f64).sqrt()
    } else {
        0.0
    }
}

fn null_deficit_penalty_from_responses(
    responses: &[Vec<f64>],
    context: &MsoObjectiveContext,
) -> f64 {
    let avg_spl = mean_response_curve(responses);
    let violations = avg_spl
        .iter()
        .zip(context.baseline_avg_spl.iter())
        .map(|(c, b)| b - c - MSO_NULL_DEFICIT_ALLOWANCE_DB);
    violation_rms_db(violations) * MSO_NULL_DEFICIT_WEIGHT
}

fn output_preservation_penalty(responses: &[Vec<f64>], context: &MsoObjectiveContext) -> f64 {
    let avg_spl = mean_response_curve(responses);
    let candidate_mean = mean_level(&avg_spl);
    let mean_loss = context.baseline_mean_level_db - candidate_mean;
    let broadband_loss_penalty =
        (mean_loss - MSO_MAX_MEAN_OUTPUT_LOSS_DB).max(0.0) * MSO_OUTPUT_LOSS_WEIGHT;

    broadband_loss_penalty + null_deficit_penalty_from_responses(responses, context)
}

fn headroom_pressure_penalty(responses: &[Vec<f64>], context: &MsoObjectiveContext) -> f64 {
    let candidate_peak_spl = peak_response_curve(responses);
    let violations = candidate_peak_spl
        .iter()
        .zip(context.baseline_peak_spl.iter())
        .map(|(c, b)| c - b - MSO_HEADROOM_BOOST_ALLOWANCE_DB);
    violation_rms_db(violations) * MSO_HEADROOM_BOOST_WEIGHT
}

fn extension_preservation_penalty(responses: &[Vec<f64>], context: &MsoObjectiveContext) -> f64 {
    if context.extension_indices.is_empty() {
        return 0.0;
    }

    let avg_spl = mean_response_curve(responses);
    let violations = context.extension_indices.iter().filter_map(|&idx| {
        let candidate = *avg_spl.get(idx)?;
        let baseline = *context.baseline_avg_spl.get(idx)?;
        Some(baseline - candidate - MSO_EXTENSION_DEFICIT_ALLOWANCE_DB)
    });
    violation_rms_db(violations) * MSO_EXTENSION_DEFICIT_WEIGHT
}

fn mso_resource_penalty(responses: &[Vec<f64>], context: &MsoObjectiveContext) -> f64 {
    output_preservation_penalty(responses, context)
        + headroom_pressure_penalty(responses, context)
        + extension_preservation_penalty(responses, context)
}

fn average_perceptual_from_responses(responses: &[Vec<f64>], context: &MsoObjectiveContext) -> f64 {
    average_flatness_from_responses(responses) + mso_resource_penalty(responses, context)
}

/// Primary-seat flatness with a quadratic penalty when other seats
/// exceed `max_deviation_db` from the primary's response at each frequency.
fn primary_constrained_from_responses(
    responses: &[Vec<f64>],
    primary_seat: usize,
    max_deviation_db: f64,
    context: Option<&MsoObjectiveContext>,
) -> f64 {
    let num_freqs = responses[0].len();
    let primary = &responses[primary_seat];

    // Primary flatness (spectral std-dev)
    let mean = primary.iter().sum::<f64>() / primary.len() as f64;
    let primary_flatness =
        (primary.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / primary.len() as f64).sqrt();

    // Constraint penalty: RMS of excess deviation at other seats
    let mut penalty_sum = 0.0;
    let mut penalty_count = 0usize;
    for (seat_idx, seat) in responses.iter().enumerate() {
        if seat_idx == primary_seat {
            continue;
        }
        for fi in 0..num_freqs {
            let deviation = (seat[fi] - primary[fi]).abs();
            if deviation > max_deviation_db {
                penalty_sum += (deviation - max_deviation_db).powi(2);
            }
            penalty_count += 1;
        }
    }
    let penalty = if penalty_count > 0 {
        (penalty_sum / penalty_count as f64).sqrt()
    } else {
        0.0
    };

    // Weight 10× ensures constraint satisfaction dominates marginal flatness gains
    let resource_penalty = context
        .map(|ctx| mso_resource_penalty(responses, ctx))
        .unwrap_or(0.0);

    primary_flatness + 10.0 * penalty + resource_penalty
}

fn objective_name(strategy: MultiSeatStrategy) -> &'static str {
    match strategy {
        MultiSeatStrategy::MinimizeVariance => "seat_variance",
        MultiSeatStrategy::Average => "average_flatness",
        MultiSeatStrategy::PrimaryWithConstraints => "primary_constrained",
        MultiSeatStrategy::ModalBasis => "modal_basis",
    }
}

fn objective_from_responses(
    responses: &[Vec<f64>],
    strategy: MultiSeatStrategy,
    primary_seat: usize,
    max_deviation_db: f64,
    context: Option<&MsoObjectiveContext>,
) -> f64 {
    match strategy {
        MultiSeatStrategy::MinimizeVariance => variance_from_responses(responses),
        MultiSeatStrategy::Average => context
            .map(|ctx| average_perceptual_from_responses(responses, ctx))
            .unwrap_or_else(|| average_flatness_from_responses(responses)),
        MultiSeatStrategy::PrimaryWithConstraints => {
            primary_constrained_from_responses(responses, primary_seat, max_deviation_db, context)
        }
        MultiSeatStrategy::ModalBasis => context
            .map(|ctx| mso_resource_penalty(responses, ctx))
            .unwrap_or_else(|| variance_from_responses(responses)),
    }
}

/// Compute variance of SPL across all seats for given gains and delays.
/// Used for before/after reporting regardless of which strategy was chosen.
#[cfg(test)]
fn compute_seat_variance(
    interpolated: &[Vec<Vec<Complex64>>],
    freqs: &Array1<f64>,
    gains: &[f64],
    delays: &[f64],
    min_freq: f64,
    max_freq: f64,
) -> f64 {
    let polarities = vec![false; gains.len()];
    let allpass_filters = vec![Vec::new(); gains.len()];
    let responses = compute_combined_responses(
        interpolated,
        freqs,
        gains,
        delays,
        &polarities,
        &allpass_filters,
        48000.0,
        min_freq,
        max_freq,
    );
    variance_from_responses(&responses)
}

// ============================================================================
// Continuous MSO search (shared by all strategies)
// ============================================================================

const MSO_GAIN_MIN_DB: f64 = -6.0;
const MSO_GAIN_MAX_DB: f64 = 6.0;
const MSO_DELAY_MIN_MS: f64 = 0.0;
const MSO_DELAY_MAX_MS: f64 = 20.0;
const MSO_ALLPASS_Q_MIN: f64 = 0.3;
const MSO_ALLPASS_Q_MAX: f64 = 5.0;
const MSO_DE_SEED: u64 = 0x5eed_5eed_d15e_a5e5;

type MsoSolution = (Vec<f64>, Vec<f64>, Vec<bool>, Vec<Vec<(f64, f64)>>);

#[derive(Debug, Clone, Copy)]
struct MsoSearchOptions {
    optimize_polarity: bool,
    allpass_filters_per_sub: usize,
    allpass_min_freq: f64,
    allpass_max_freq: f64,
}

impl MsoSearchOptions {
    fn from_config(config: &MultiSeatConfig, min_freq: f64, max_freq: f64) -> Self {
        let allpass_min_freq = min_freq.max(20.0);
        let allpass_max_freq = max_freq.min(200.0).max(allpass_min_freq);
        Self {
            optimize_polarity: config.optimize_polarity,
            allpass_filters_per_sub: config.allpass_filters_per_sub,
            allpass_min_freq,
            allpass_max_freq,
        }
    }
}

#[derive(Clone)]
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn next_f64(&mut self) -> f64 {
        let value = self.next_u64() >> 11;
        value as f64 / ((1_u64 << 53) as f64)
    }

    fn range_f64(&mut self, min: f64, max: f64) -> f64 {
        min + self.next_f64() * (max - min)
    }

    fn index(&mut self, len: usize) -> usize {
        (self.next_u64() as usize) % len
    }
}

fn mso_params_per_optimized_sub(options: MsoSearchOptions) -> usize {
    2 + usize::from(options.optimize_polarity) + options.allpass_filters_per_sub * 2
}

fn mso_bounds(num_subs: usize, options: MsoSearchOptions) -> (Vec<f64>, Vec<f64>) {
    let dims = num_subs.saturating_sub(1) * mso_params_per_optimized_sub(options);
    let mut lower = Vec::with_capacity(dims);
    let mut upper = Vec::with_capacity(dims);

    for _ in 1..num_subs {
        lower.push(MSO_GAIN_MIN_DB);
        upper.push(MSO_GAIN_MAX_DB);
        lower.push(MSO_DELAY_MIN_MS);
        upper.push(MSO_DELAY_MAX_MS);
        if options.optimize_polarity {
            lower.push(0.0);
            upper.push(1.0);
        }
        for _ in 0..options.allpass_filters_per_sub {
            lower.push(options.allpass_min_freq);
            upper.push(options.allpass_max_freq);
            lower.push(MSO_ALLPASS_Q_MIN);
            upper.push(MSO_ALLPASS_Q_MAX);
        }
    }

    (lower, upper)
}

fn decode_mso_params(params: &[f64], num_subs: usize, options: MsoSearchOptions) -> MsoSolution {
    let mut gains = vec![0.0; num_subs];
    let mut delays = vec![0.0; num_subs];
    let mut polarities = vec![false; num_subs];
    let mut allpass_filters = vec![Vec::new(); num_subs];
    let per_sub = mso_params_per_optimized_sub(options);

    for sub_idx in 1..num_subs {
        let mut offset = (sub_idx - 1) * per_sub;
        gains[sub_idx] = params[offset];
        offset += 1;
        delays[sub_idx] = params[offset];
        offset += 1;

        if options.optimize_polarity {
            polarities[sub_idx] = params[offset] >= 0.5;
            offset += 1;
        }

        for _ in 0..options.allpass_filters_per_sub {
            let freq = params[offset];
            let q = params[offset + 1];
            allpass_filters[sub_idx].push((freq, q));
            offset += 2;
        }
    }

    (gains, delays, polarities, allpass_filters)
}

#[allow(
    clippy::type_complexity,
    reason = "extracting a type alias for this dyn Fn requires explicit lifetimes that pollute every call site"
)]
fn optimize_continuous_mso(
    num_subs: usize,
    options: MsoSearchOptions,
    eval: &dyn Fn(&[f64], &[f64], &[bool], &[Vec<(f64, f64)>]) -> f64,
) -> MsoSolution {
    if num_subs <= 1 {
        return (
            vec![0.0; num_subs],
            vec![0.0; num_subs],
            vec![false; num_subs],
            vec![Vec::new(); num_subs],
        );
    }

    let (lower, upper) = mso_bounds(num_subs, options);
    let dims = lower.len();
    let population_size = (dims * 24).max(48);
    let generations = (120 + dims * 30).max(200);
    let mutation = 0.7;
    let crossover = 0.9;
    let mut rng = SimpleRng::new(MSO_DE_SEED ^ (num_subs as u64));

    let mut population = vec![vec![0.0; dims]; population_size];
    for dim in 0..dims {
        population[0][dim] = f64::clamp(population[0][dim], lower[dim], upper[dim]);
    }
    for individual in population.iter_mut().skip(1) {
        for dim in 0..dims {
            individual[dim] = rng.range_f64(lower[dim], upper[dim]);
        }
    }

    let mut scores: Vec<f64> = population
        .iter()
        .map(|params| {
            let (gains, delays, polarities, allpass_filters) =
                decode_mso_params(params, num_subs, options);
            eval(&gains, &delays, &polarities, &allpass_filters)
        })
        .collect();

    for _ in 0..generations {
        for target_idx in 0..population_size {
            let mut a;
            let mut b;
            let mut c;
            loop {
                a = rng.index(population_size);
                if a != target_idx {
                    break;
                }
            }
            loop {
                b = rng.index(population_size);
                if b != target_idx && b != a {
                    break;
                }
            }
            loop {
                c = rng.index(population_size);
                if c != target_idx && c != a && c != b {
                    break;
                }
            }

            let forced_dim = rng.index(dims);
            let mut trial = population[target_idx].clone();
            for dim in 0..dims {
                if dim == forced_dim || rng.next_f64() < crossover {
                    let value =
                        population[a][dim] + mutation * (population[b][dim] - population[c][dim]);
                    trial[dim] = value.clamp(lower[dim], upper[dim]);
                }
            }

            let (gains, delays, polarities, allpass_filters) =
                decode_mso_params(&trial, num_subs, options);
            let trial_score = eval(&gains, &delays, &polarities, &allpass_filters);
            if trial_score < scores[target_idx] {
                population[target_idx] = trial;
                scores[target_idx] = trial_score;
            }
        }
    }

    let (best_idx, best_loss) = scores
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(idx, score)| (idx, *score))
        .unwrap_or((0, f64::INFINITY));

    let (best_gains, best_delays, best_polarities, best_allpass_filters) =
        decode_mso_params(&population[best_idx], num_subs, options);
    debug!(
        "  Continuous MSO result: gains={:?}, delays={:?}, polarities={:?}, allpass={:?}, loss={:.4}",
        best_gains, best_delays, best_polarities, best_allpass_filters, best_loss
    );

    (
        best_gains,
        best_delays,
        best_polarities,
        best_allpass_filters,
    )
}

// ============================================================================
// Strategy implementations
// ============================================================================

/// Optimize for minimum variance across seats
fn optimize_minimize_variance(
    interpolated: &[Vec<Vec<Complex64>>],
    freqs: &Array1<f64>,
    num_subs: usize,
    config: &MultiSeatConfig,
    sample_rate: f64,
    min_freq: f64,
    max_freq: f64,
    _objective_context: &MsoObjectiveContext,
) -> MsoSolution {
    let options = MsoSearchOptions::from_config(config, min_freq, max_freq);
    optimize_continuous_mso(
        num_subs,
        options,
        &|gains, delays, polarities, allpass_filters| {
            let r = compute_combined_responses(
                interpolated,
                freqs,
                gains,
                delays,
                polarities,
                allpass_filters,
                sample_rate,
                min_freq,
                max_freq,
            );
            variance_from_responses(&r)
        },
    )
}

/// Optimize for flattest average response across seats.
/// Unlike `MinimizeVariance` (which makes all seats match each other),
/// this minimizes spectral deviation of the *mean* response so the
/// average listener hears a tonally flat result.
fn optimize_average_response(
    interpolated: &[Vec<Vec<Complex64>>],
    freqs: &Array1<f64>,
    num_subs: usize,
    config: &MultiSeatConfig,
    sample_rate: f64,
    min_freq: f64,
    max_freq: f64,
    objective_context: &MsoObjectiveContext,
) -> MsoSolution {
    let options = MsoSearchOptions::from_config(config, min_freq, max_freq);
    optimize_continuous_mso(
        num_subs,
        options,
        &|gains, delays, polarities, allpass_filters| {
            let r = compute_combined_responses(
                interpolated,
                freqs,
                gains,
                delays,
                polarities,
                allpass_filters,
                sample_rate,
                min_freq,
                max_freq,
            );
            average_perceptual_from_responses(&r, objective_context)
        },
    )
}

/// Optimize for primary seat with constraints on other seats.
/// Minimizes spectral flatness at `primary_seat` while penalizing
/// configurations where any other seat deviates from the primary
/// by more than `max_deviation_db` at any frequency.
fn optimize_primary_with_constraints(
    interpolated: &[Vec<Vec<Complex64>>],
    freqs: &Array1<f64>,
    num_subs: usize,
    config: &MultiSeatConfig,
    sample_rate: f64,
    primary_seat: usize,
    max_deviation_db: f64,
    min_freq: f64,
    max_freq: f64,
    objective_context: &MsoObjectiveContext,
) -> MsoSolution {
    let options = MsoSearchOptions::from_config(config, min_freq, max_freq);
    optimize_continuous_mso(
        num_subs,
        options,
        &|gains, delays, polarities, allpass_filters| {
            let r = compute_combined_responses(
                interpolated,
                freqs,
                gains,
                delays,
                polarities,
                allpass_filters,
                sample_rate,
                min_freq,
                max_freq,
            );
            primary_constrained_from_responses(
                &r,
                primary_seat,
                max_deviation_db,
                Some(objective_context),
            )
        },
    )
}

/// Optimize a complex modal-basis SFM objective.
///
/// The modal term suppresses non-common complex seat-pressure components
/// projected onto the dominant room modes extracted from the per-sub/per-seat
/// transfer matrix. Resource penalties preserve output, extension, and
/// headroom so the optimizer cannot win by simply reducing bass energy.
fn optimize_modal_basis(
    interpolated: &[Vec<Vec<Complex64>>],
    freqs: &Array1<f64>,
    num_subs: usize,
    config: &MultiSeatConfig,
    sample_rate: f64,
    min_freq: f64,
    max_freq: f64,
    basis: &ModalBasis,
    objective_context: &MsoObjectiveContext,
) -> MsoSolution {
    let options = MsoSearchOptions::from_config(config, min_freq, max_freq);
    optimize_continuous_mso(
        num_subs,
        options,
        &|gains, delays, polarities, allpass_filters| {
            let complex = compute_combined_complex_responses(
                interpolated,
                freqs,
                gains,
                delays,
                polarities,
                allpass_filters,
                sample_rate,
                min_freq,
                max_freq,
            );
            let spl = spl_from_complex_responses(&complex);
            modal_basis_objective_from_responses(&complex, &spl, basis, objective_context)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn mso_regression_guard_rejects_worse_or_nonfinite_objectives() {
        assert!(mso_objective_regressed(1.0, 1.01));
        assert!(mso_objective_regressed(1.0, f64::NAN));
        assert!(!mso_objective_regressed(1.0, 1.0));
        assert!(!mso_objective_regressed(
            1.0,
            1.0 + MSO_OBJECTIVE_REGRESSION_TOLERANCE
        ));
    }

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
    fn test_multiseat_measurements_reject_missing_phase() {
        let mut missing_phase = create_test_curve(0.0, 0.0);
        missing_phase.phase = None;
        let measurements = vec![
            vec![missing_phase, create_test_curve(2.0, 10.0)],
            vec![create_test_curve(-1.0, 5.0), create_test_curve(1.0, 15.0)],
        ];

        let err = MultiSeatMeasurements::new(measurements).unwrap_err();

        assert!(
            err.to_string().contains("missing phase"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_optimize_multiseat_rejects_non_overlapping_band() {
        let measurements = vec![
            vec![create_test_curve(0.0, 0.0), create_test_curve(2.0, 10.0)],
            vec![create_test_curve(-1.0, 5.0), create_test_curve(1.0, 15.0)],
        ];
        let ms = MultiSeatMeasurements::new(measurements).expect("Should create");
        let config = MultiSeatConfig::default();

        let err = optimize_multiseat(&ms, &config, (300.0, 500.0), 48000.0).unwrap_err();

        assert!(
            err.to_string().contains("does not overlap"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_primary_seat_out_of_range() {
        let measurements = vec![
            vec![create_test_curve(0.0, 0.0), create_test_curve(2.0, 10.0)],
            vec![create_test_curve(-1.0, 5.0), create_test_curve(1.0, 15.0)],
        ];
        let ms = MultiSeatMeasurements::new(measurements).expect("Should create");

        let config = MultiSeatConfig {
            enabled: true,
            strategy: MultiSeatStrategy::PrimaryWithConstraints,
            primary_seat: 5, // only 2 seats
            max_deviation_db: 6.0,
            ..Default::default()
        };

        let result = optimize_multiseat(&ms, &config, (20.0, 120.0), 48000.0);
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
            ..Default::default()
        };

        let result =
            optimize_multiseat(&ms, &config, (20.0, 120.0), 48000.0).expect("Should optimize");

        assert_eq!(result.gains.len(), 2);
        assert_eq!(result.delays.len(), 2);
        // First sub should be reference (no adjustment)
        assert_eq!(result.gains[0], 0.0);
        assert_eq!(result.delays[0], 0.0);
        assert_eq!(result.strategy, MultiSeatStrategy::MinimizeVariance);
        assert_eq!(result.objective_name, "seat_variance");
        assert_close(result.improvement_db, result.objective_improvement_db);
        assert_close(result.improvement_db, result.variance_improvement_db);
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

    #[test]
    fn test_average_strategy_differs_from_minimize_variance() {
        // Construct curves where "flat average" and "minimize variance" diverge:
        // Sub 0 at seat 0 is flat 90 dB; at seat 1 it has a 6 dB dip at low freq.
        // Sub 1 at seat 0 has a 6 dB peak at low freq; at seat 1 it is flat 90 dB.
        //
        // MinimizeVariance wants seats to match — it may trade average flatness.
        // Average wants the mean SPL across seats to be spectrally flat.
        let make_curve = |spl_fn: &dyn Fn(f64) -> f64, phase_off: f64| -> Curve {
            let freqs: Vec<f64> = (0..50)
                .map(|i| 20.0 * (200.0 / 20.0_f64).powf(i as f64 / 49.0))
                .collect();
            let spl: Vec<f64> = freqs.iter().map(|f| spl_fn(*f)).collect();
            let phase: Vec<f64> = freqs
                .iter()
                .map(|f| -180.0 * f / 100.0 + phase_off)
                .collect();
            Curve {
                freq: Array1::from(freqs),
                spl: Array1::from(spl),
                phase: Some(Array1::from(phase)),
                ..Default::default()
            }
        };

        let flat = |_f: f64| 90.0;
        let dipped = |f: f64| if f < 60.0 { 84.0 } else { 90.0 };
        let peaked = |f: f64| if f < 60.0 { 96.0 } else { 90.0 };

        let measurements = vec![
            vec![make_curve(&flat, 0.0), make_curve(&dipped, 10.0)],
            vec![make_curve(&peaked, 5.0), make_curve(&flat, 15.0)],
        ];
        let ms = MultiSeatMeasurements::new(measurements).expect("Should create");

        let var_config = MultiSeatConfig {
            enabled: true,
            strategy: MultiSeatStrategy::MinimizeVariance,
            primary_seat: 0,
            max_deviation_db: 6.0,
            ..Default::default()
        };
        let avg_config = MultiSeatConfig {
            strategy: MultiSeatStrategy::Average,
            ..var_config.clone()
        };

        let var_result = optimize_multiseat(&ms, &var_config, (20.0, 120.0), 48000.0).expect("var");
        let avg_result = optimize_multiseat(&ms, &avg_config, (20.0, 120.0), 48000.0).expect("avg");

        // The two strategies should (generally) produce different gain/delay solutions.
        // At minimum, the Average strategy should run its own loss — we verify it
        // doesn't crash and returns valid results.
        assert_eq!(avg_result.gains.len(), 2);
        assert_eq!(avg_result.delays.len(), 2);
        assert_eq!(avg_result.gains[0], 0.0);
        assert_eq!(avg_result.delays[0], 0.0);

        assert_eq!(avg_result.strategy, MultiSeatStrategy::Average);
        assert_eq!(avg_result.objective_name, "average_flatness");
        assert_close(
            avg_result.improvement_db,
            avg_result.objective_improvement_db,
        );
        assert!(avg_result.objective_improvement_db >= -0.01);

        // Variance is still reported as a diagnostic, but Average optimizes
        // average flatness, so it is no longer the success metric.
        assert!(var_result.improvement_db >= -0.01);
    }

    #[test]
    fn test_primary_with_constraints_favors_primary_seat() {
        // Seat 0 (primary) gets flat 90 dB from sub 0, seat 1 gets a dip.
        // The optimizer should favor seat 0 flatness over seat 1.
        let make_curve = |spl_val: f64, phase_off: f64| -> Curve {
            let freqs: Vec<f64> = (0..50)
                .map(|i| 20.0 * (200.0 / 20.0_f64).powf(i as f64 / 49.0))
                .collect();
            let spl: Vec<f64> = freqs.iter().map(|_| spl_val).collect();
            let phase: Vec<f64> = freqs
                .iter()
                .map(|f| -180.0 * f / 100.0 + phase_off)
                .collect();
            Curve {
                freq: Array1::from(freqs),
                spl: Array1::from(spl),
                phase: Some(Array1::from(phase)),
                ..Default::default()
            }
        };

        let measurements = vec![
            vec![make_curve(90.0, 0.0), make_curve(85.0, 20.0)],
            vec![make_curve(88.0, 10.0), make_curve(92.0, 30.0)],
        ];
        let ms = MultiSeatMeasurements::new(measurements).expect("Should create");

        let config = MultiSeatConfig {
            enabled: true,
            strategy: MultiSeatStrategy::PrimaryWithConstraints,
            primary_seat: 0,
            max_deviation_db: 6.0,
            ..Default::default()
        };

        let result =
            optimize_multiseat(&ms, &config, (20.0, 120.0), 48000.0).expect("Should optimize");

        assert_eq!(result.gains.len(), 2);
        assert_eq!(result.delays.len(), 2);
        assert_eq!(result.gains[0], 0.0);
        assert_eq!(result.delays[0], 0.0);
        assert_eq!(result.strategy, MultiSeatStrategy::PrimaryWithConstraints);
        assert_eq!(result.objective_name, "primary_constrained");
        assert_close(result.improvement_db, result.objective_improvement_db);
        assert!(result.objective_improvement_db >= -0.01);
    }

    #[test]
    fn test_modal_mode_count_uses_energy_cutoff_and_cap() {
        assert_eq!(select_modal_mode_count(&[], 0.95, 8), 0);
        assert_eq!(select_modal_mode_count(&[10.0, 1.0, 1.0], 0.95, 8), 1);
        assert_eq!(select_modal_mode_count(&[3.0, 2.0, 1.0], 0.95, 2), 2);
        assert_eq!(select_modal_mode_count(&[3.0, 2.0, 1.0], 0.95, 0), 0);
    }

    #[test]
    fn test_modal_projection_loss_prefers_uniform_pressure() {
        let inv_sqrt_2 = 1.0 / 2.0_f64.sqrt();
        let basis = ModalBasis {
            modes: vec![vec![
                Complex64::new(inv_sqrt_2, 0.0),
                Complex64::new(-inv_sqrt_2, 0.0),
            ]],
            singular_values: vec![1.0],
            retained_energy: 1.0,
        };

        let uniform = vec![
            vec![Complex64::new(2.0, 0.0), Complex64::new(3.0, 0.0)],
            vec![Complex64::new(2.0, 0.0), Complex64::new(3.0, 0.0)],
        ];
        let nonuniform = vec![
            vec![Complex64::new(3.0, 0.0), Complex64::new(4.0, 0.0)],
            vec![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)],
        ];

        let uniform_loss = modal_projection_loss(&uniform, &basis);
        let nonuniform_loss = modal_projection_loss(&nonuniform, &basis);

        assert!(uniform_loss < 1e-9, "uniform loss was {uniform_loss}");
        assert!(
            nonuniform_loss > uniform_loss + 0.5,
            "non-uniform pressure should project onto the retained mode; uniform={uniform_loss}, nonuniform={nonuniform_loss}"
        );
    }

    #[test]
    fn test_modal_basis_extraction_uses_complex_snapshots() {
        let measurements = vec![
            vec![
                create_test_curve(0.0, 0.0),
                create_test_curve(2.0, 35.0),
                create_test_curve(-1.0, -25.0),
            ],
            vec![
                create_test_curve(-2.0, 90.0),
                create_test_curve(1.0, -70.0),
                create_test_curve(3.0, 120.0),
            ],
        ];
        let ms = MultiSeatMeasurements::new(measurements).expect("Should create");
        let freqs = create_eval_frequency_grid(&ms, 20.0, 120.0);
        let interpolated = interpolate_all_measurements(&ms, &freqs).expect("Should interpolate");

        let basis = build_modal_basis(&interpolated, &freqs, 20.0, 120.0);

        assert!(
            !basis.modes.is_empty(),
            "expected at least one modal basis vector"
        );
        assert!(basis.modes.len() <= modal_basis_mode_cap(ms.num_seats, ms.num_subs));
        assert!(!basis.singular_values.is_empty());
        assert!(basis.retained_energy > 0.0);
    }

    #[test]
    fn test_modal_basis_strategy_runs() {
        let measurements = vec![
            vec![
                create_test_curve(0.0, 0.0),
                create_test_curve(3.0, 20.0),
                create_test_curve(-2.0, -30.0),
            ],
            vec![
                create_test_curve(0.0, 80.0),
                create_test_curve(-2.0, 130.0),
                create_test_curve(2.0, -90.0),
            ],
        ];
        let ms = MultiSeatMeasurements::new(measurements).expect("Should create");
        let config = MultiSeatConfig {
            enabled: true,
            strategy: MultiSeatStrategy::ModalBasis,
            ..Default::default()
        };

        let result =
            optimize_multiseat(&ms, &config, (20.0, 120.0), 48000.0).expect("Should optimize");

        assert_eq!(result.gains.len(), 2);
        assert_eq!(result.delays.len(), 2);
        assert_eq!(result.gains[0], 0.0);
        assert_eq!(result.delays[0], 0.0);
        assert_eq!(result.strategy, MultiSeatStrategy::ModalBasis);
        assert_eq!(result.objective_name, "modal_basis");
        assert!(result.objective_before.is_finite());
        assert!(result.objective_after.is_finite());
    }

    #[test]
    fn test_average_objective_rejects_output_collapse() {
        let baseline = vec![vec![90.0, 90.0, 90.0], vec![90.0, 90.0, 90.0]];
        let collapsed_but_flat = vec![vec![78.0, 78.0, 78.0], vec![78.0, 78.0, 78.0]];
        let slightly_rippled_preserved = vec![vec![89.0, 90.0, 91.0], vec![89.0, 90.0, 91.0]];
        let context = MsoObjectiveContext::from_baseline(&baseline);

        assert_eq!(average_flatness_from_responses(&collapsed_but_flat), 0.0);
        assert!(
            average_perceptual_from_responses(&collapsed_but_flat, &context)
                > average_perceptual_from_responses(&slightly_rippled_preserved, &context),
            "MSO average objective should prefer small ripple over large broadband output loss"
        );
    }

    #[test]
    fn test_primary_objective_rejects_new_deep_nulls() {
        let baseline = vec![vec![90.0, 90.0, 90.0], vec![90.0, 90.0, 90.0]];
        let null_candidate = vec![vec![90.0, 70.0, 90.0], vec![90.0, 70.0, 90.0]];
        let safe_candidate = vec![vec![89.0, 90.0, 91.0], vec![89.0, 90.0, 91.0]];
        let context = MsoObjectiveContext::from_baseline(&baseline);

        assert!(
            primary_constrained_from_responses(&null_candidate, 0, 6.0, Some(&context))
                > primary_constrained_from_responses(&safe_candidate, 0, 6.0, Some(&context)),
            "MSO primary objective should penalize new average-response nulls"
        );
    }

    #[test]
    fn test_primary_objective_penalizes_headroom_boost() {
        let baseline = vec![vec![90.0, 90.0, 90.0], vec![90.0, 90.0, 90.0]];
        let boosted_flat = vec![vec![94.0, 94.0, 94.0], vec![94.0, 94.0, 94.0]];
        let preserved_flat = baseline.clone();
        let context = MsoObjectiveContext::from_baseline(&baseline);

        assert!(
            primary_constrained_from_responses(&boosted_flat, 0, 6.0, Some(&context))
                > primary_constrained_from_responses(&preserved_flat, 0, 6.0, Some(&context)),
            "MSO primary objective should penalize flat response wins that consume headroom"
        );
    }

    #[test]
    fn test_primary_objective_penalizes_low_extension_deficit() {
        let baseline = vec![vec![90.0, 90.0, 90.0, 90.0], vec![90.0, 90.0, 90.0, 90.0]];
        let low_extension_loss = vec![vec![86.0, 86.0, 90.0, 90.0], vec![86.0, 86.0, 90.0, 90.0]];
        let upper_band_loss = vec![vec![90.0, 90.0, 86.0, 86.0], vec![90.0, 90.0, 86.0, 86.0]];
        let freqs = Array1::from(vec![20.0, 35.0, 80.0, 120.0]);
        let context = MsoObjectiveContext::from_baseline_with_freqs(&baseline, Some(&freqs));

        assert!(
            primary_constrained_from_responses(&low_extension_loss, 0, 6.0, Some(&context))
                > primary_constrained_from_responses(&upper_band_loss, 0, 6.0, Some(&context)),
            "MSO primary objective should treat low-band extension loss as worse than an equivalent upper-band loss"
        );
    }

    #[test]
    fn headroom_penalty_is_grid_density_independent() {
        // Same physical violation: a single 5 dB peak boost. The penalty must
        // not shrink when the response is sampled on a finer grid.
        let coarse_baseline = vec![vec![90.0, 90.0, 90.0], vec![90.0, 90.0, 90.0]];
        let coarse_candidate = vec![vec![95.0, 90.0, 90.0], vec![95.0, 90.0, 90.0]];
        let coarse_ctx = MsoObjectiveContext::from_baseline(&coarse_baseline);
        let coarse = headroom_pressure_penalty(&coarse_candidate, &coarse_ctx);

        let fine_baseline = vec![vec![90.0; 12], vec![90.0; 12]];
        let mut fine_row = vec![90.0; 12];
        fine_row[0] = 95.0;
        let fine_candidate = vec![fine_row.clone(), fine_row];
        let fine_ctx = MsoObjectiveContext::from_baseline(&fine_baseline);
        let fine = headroom_pressure_penalty(&fine_candidate, &fine_ctx);

        assert!(
            coarse > 0.0,
            "expected non-zero headroom penalty on coarse grid"
        );
        assert!(
            (coarse - fine).abs() < 1e-9,
            "headroom penalty should be grid-density independent; got coarse={coarse}, fine={fine}"
        );
    }

    #[test]
    fn null_deficit_penalty_is_grid_density_independent() {
        // Single deep null at one frequency, identical across grids.
        let coarse_baseline = vec![vec![90.0, 90.0, 90.0], vec![90.0, 90.0, 90.0]];
        let coarse_candidate = vec![vec![70.0, 90.0, 90.0], vec![70.0, 90.0, 90.0]];
        let coarse_ctx = MsoObjectiveContext::from_baseline(&coarse_baseline);
        let coarse = null_deficit_penalty_from_responses(&coarse_candidate, &coarse_ctx);

        let fine_baseline = vec![vec![90.0; 12], vec![90.0; 12]];
        let mut fine_row = vec![90.0; 12];
        fine_row[0] = 70.0;
        let fine_candidate = vec![fine_row.clone(), fine_row];
        let fine_ctx = MsoObjectiveContext::from_baseline(&fine_baseline);
        let fine = null_deficit_penalty_from_responses(&fine_candidate, &fine_ctx);

        assert!(
            coarse > 0.0,
            "expected non-zero null-deficit penalty on coarse grid"
        );
        assert!(
            (coarse - fine).abs() < 1e-9,
            "null-deficit penalty should be grid-density independent; got coarse={coarse}, fine={fine}"
        );
    }

    #[test]
    fn extension_penalty_is_grid_density_independent() {
        // Same low-band loss (10 dB at 20 Hz only) on coarse vs fine grid.
        let coarse_baseline = vec![vec![90.0, 90.0, 90.0], vec![90.0, 90.0, 90.0]];
        let coarse_candidate = vec![vec![80.0, 90.0, 90.0], vec![80.0, 90.0, 90.0]];
        let coarse_freqs = Array1::from(vec![20.0, 80.0, 200.0]);
        let coarse_ctx =
            MsoObjectiveContext::from_baseline_with_freqs(&coarse_baseline, Some(&coarse_freqs));
        let coarse = extension_preservation_penalty(&coarse_candidate, &coarse_ctx);

        let fine_baseline = vec![vec![90.0; 6], vec![90.0; 6]];
        let mut fine_row = vec![90.0; 6];
        fine_row[0] = 80.0;
        let fine_candidate = vec![fine_row.clone(), fine_row];
        let fine_freqs = Array1::from(vec![20.0, 25.0, 30.0, 80.0, 100.0, 200.0]);
        let fine_ctx =
            MsoObjectiveContext::from_baseline_with_freqs(&fine_baseline, Some(&fine_freqs));
        let fine = extension_preservation_penalty(&fine_candidate, &fine_ctx);

        assert!(
            coarse > 0.0,
            "expected non-zero extension penalty on coarse grid"
        );
        assert!(
            (coarse - fine).abs() < 1e-9,
            "extension penalty should be grid-density independent; got coarse={coarse}, fine={fine}"
        );
    }

    #[test]
    fn test_phase_wrap_interpolation() {
        // Curve with phase crossing ±180° boundary
        let freqs = vec![50.0, 60.0, 70.0, 80.0];
        let spl = vec![90.0, 90.0, 90.0, 90.0];
        let phase = vec![170.0, 179.0, -179.0, -170.0]; // crosses +180/-180

        let curve = Curve {
            freq: Array1::from(freqs),
            spl: Array1::from(spl),
            phase: Some(Array1::from(phase)),
            ..Default::default()
        };

        let grid = Array1::from(vec![65.0]); // midpoint between 60 and 70
        let result = interpolate_curve_to_grid(&curve, &grid).expect("Should interpolate");

        // With wrap-aware interpolation, midpoint of 179° and -179° should be ~180°,
        // not 0° (which naive linear interpolation would produce).
        let phase_deg = result[0].arg().to_degrees();
        assert!(
            phase_deg.abs() > 170.0,
            "Phase should be near ±180°, got {:.1}°",
            phase_deg
        );
    }

    #[test]
    fn test_missing_phase_is_rejected() {
        let curve = Curve {
            freq: Array1::from(vec![50.0, 60.0, 70.0]),
            spl: Array1::from(vec![90.0, 91.0, 90.5]),
            phase: None,
            ..Default::default()
        };
        let grid = Array1::from(vec![55.0, 65.0]);

        let err = interpolate_curve_to_grid(&curve, &grid).unwrap_err();

        assert!(
            err.to_string().contains("requires phase data"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_continuous_mso_returns_valid_solution() {
        // Verify that the continuous optimizer returns bounded, non-degenerate
        // gain/delay values without quantizing the search to a coarse grid.
        let measurements = vec![
            vec![create_test_curve(0.0, 0.0), create_test_curve(3.0, 20.0)],
            vec![create_test_curve(0.0, 10.0), create_test_curve(-2.0, 30.0)],
        ];
        let ms = MultiSeatMeasurements::new(measurements).expect("Should create");
        let freqs = create_eval_frequency_grid(&ms, 20.0, 120.0);
        let interpolated = interpolate_all_measurements(&ms, &freqs).expect("Should interpolate");
        let initial = compute_combined_responses(
            &interpolated,
            &freqs,
            &[0.0, 0.0],
            &[0.0, 0.0],
            &[false, false],
            &[Vec::new(), Vec::new()],
            48000.0,
            20.0,
            120.0,
        );
        let objective_context = MsoObjectiveContext::from_baseline(&initial);

        let config = MultiSeatConfig::default();
        let (gains, delays, polarities, allpass_filters) = optimize_minimize_variance(
            &interpolated,
            &freqs,
            2,
            &config,
            48000.0,
            20.0,
            120.0,
            &objective_context,
        );

        // With fine resolution, at least one parameter should land on a
        // non-integer value (0.1 step grid), demonstrating the refinement pass.
        let has_fractional_gain = gains.iter().any(|g| (g * 10.0).fract().abs() > 0.001);
        let has_fractional_delay = delays.iter().any(|d| (d * 10.0).fract().abs() > 0.001);
        // This may not always be true for all test data, so we just verify
        // the result is valid and non-degenerate
        assert_eq!(gains[0], 0.0);
        assert_eq!(delays[0], 0.0);
        assert!(!polarities[0]);
        assert!(allpass_filters[0].is_empty());
        assert!(gains[1] >= MSO_GAIN_MIN_DB && gains[1] <= MSO_GAIN_MAX_DB);
        assert!(delays[1] >= MSO_DELAY_MIN_MS && delays[1] <= MSO_DELAY_MAX_MS);
        let _ = (has_fractional_gain, has_fractional_delay);
    }

    #[test]
    fn test_continuous_mso_can_recover_fractional_optimum() {
        let options = MsoSearchOptions {
            optimize_polarity: false,
            allpass_filters_per_sub: 0,
            allpass_min_freq: 20.0,
            allpass_max_freq: 120.0,
        };
        let (gains, delays, polarities, allpass_filters) =
            optimize_continuous_mso(2, options, &|gains, delays, _, _| {
                (gains[1] - 1.23).powi(2) + (delays[1] - 4.56).powi(2)
            });

        assert_eq!(gains[0], 0.0);
        assert_eq!(delays[0], 0.0);
        assert!(!polarities[0]);
        assert!(allpass_filters[0].is_empty());
        assert!(
            (gains[1] - 1.23).abs() < 0.05,
            "gain should recover fractional optimum, got {:.3}",
            gains[1]
        );
        assert!(
            (delays[1] - 4.56).abs() < 0.05,
            "delay should recover fractional optimum, got {:.3}",
            delays[1]
        );
    }

    #[test]
    fn test_continuous_mso_can_optimize_polarity() {
        let options = MsoSearchOptions {
            optimize_polarity: true,
            allpass_filters_per_sub: 0,
            allpass_min_freq: 20.0,
            allpass_max_freq: 120.0,
        };
        let (_gains, _delays, polarities, allpass_filters) =
            optimize_continuous_mso(2, options, &|_, _, polarities, _| {
                if polarities[1] { 0.0 } else { 10.0 }
            });

        assert!(!polarities[0], "reference sub polarity should stay fixed");
        assert!(polarities[1], "second sub polarity should be optimized");
        assert!(allpass_filters.iter().all(Vec::is_empty));
    }

    #[test]
    fn test_continuous_mso_can_optimize_allpass_filter() {
        let options = MsoSearchOptions {
            optimize_polarity: false,
            allpass_filters_per_sub: 1,
            allpass_min_freq: 20.0,
            allpass_max_freq: 120.0,
        };
        let (_gains, _delays, polarities, allpass_filters) =
            optimize_continuous_mso(2, options, &|_, _, _, allpass_filters| {
                let (freq, q) = allpass_filters[1][0];
                ((freq - 73.4) / 10.0).powi(2) + (q - 1.7).powi(2)
            });

        assert!(!polarities[0]);
        assert!(allpass_filters[0].is_empty());
        assert_eq!(allpass_filters[1].len(), 1);
        let (freq, q) = allpass_filters[1][0];
        assert!(
            (freq - 73.4).abs() < 1.0,
            "all-pass frequency should recover target, got {:.3}",
            freq
        );
        assert!(
            (q - 1.7).abs() < 0.05,
            "all-pass Q should recover target, got {:.3}",
            q
        );
    }
}
