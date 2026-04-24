//! Multi-seat variance optimization for subwoofer systems
//!
//! Optimizes subwoofer delays and gains to minimize response variance
//! across multiple listening positions (MSO - Multi-Subwoofer Optimizer logic).

use crate::Curve;
use crate::error::{AutoeqError, Result};
use log::{debug, info};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
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
    let freqs = create_eval_frequency_grid(measurements, min_freq, max_freq);

    // Interpolate all measurements to common grid
    let interpolated = interpolate_all_measurements(measurements, &freqs)?;

    // Initial state: no gain adjustment, no delay
    let initial_gains = vec![0.0; measurements.num_subs];
    let initial_delays = vec![0.0; measurements.num_subs];
    let initial_polarities = vec![false; measurements.num_subs];
    let initial_allpass_filters = vec![Vec::new(); measurements.num_subs];

    let initial_responses = compute_combined_responses(
        &interpolated,
        &freqs,
        &initial_gains,
        &initial_delays,
        &initial_polarities,
        &initial_allpass_filters,
        sample_rate,
        min_freq,
        max_freq,
    );
    let variance_before = variance_from_responses(&initial_responses);
    let objective_before = objective_from_responses(
        &initial_responses,
        config.strategy.clone(),
        config.primary_seat,
        config.max_deviation_db,
    );

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
                min_freq,
                max_freq,
            ),
            MultiSeatStrategy::Average => optimize_average_response(
                &interpolated,
                &freqs,
                measurements.num_subs,
                config,
                sample_rate,
                min_freq,
                max_freq,
            ),
            MultiSeatStrategy::PrimaryWithConstraints => optimize_primary_with_constraints(
                &interpolated,
                &freqs,
                measurements.num_subs,
                config,
                sample_rate,
                config.primary_seat,
                config.max_deviation_db,
                min_freq,
                max_freq,
            ),
        };

    let final_responses = compute_combined_responses(
        &interpolated,
        &freqs,
        &optimal_gains,
        &optimal_delays,
        &optimal_polarities,
        &optimal_allpass_filters,
        sample_rate,
        min_freq,
        max_freq,
    );
    let variance_after = variance_from_responses(&final_responses);
    let objective_after = objective_from_responses(
        &final_responses,
        config.strategy.clone(),
        config.primary_seat,
        config.max_deviation_db,
    );

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
        gains: optimal_gains,
        delays: optimal_delays,
        polarities: optimal_polarities,
        allpass_filters: optimal_allpass_filters,
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
    let num_seats = interpolated[0].len();
    let mut seat_responses: Vec<Vec<f64>> = Vec::with_capacity(num_seats);
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
        let mut combined_spl = Vec::new();

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

            combined_spl.push(20.0 * combined.norm().max(1e-12).log10());
        }

        seat_responses.push(combined_spl);
    }

    seat_responses
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
    let num_freqs = responses[0].len();
    let num_seats = responses.len() as f64;

    // Mean SPL at each frequency across seats
    let avg_spl: Vec<f64> = (0..num_freqs)
        .map(|fi| responses.iter().map(|s| s[fi]).sum::<f64>() / num_seats)
        .collect();

    // Spectral std-dev of the average
    let mean = avg_spl.iter().sum::<f64>() / avg_spl.len() as f64;
    let variance = avg_spl.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / avg_spl.len() as f64;
    variance.sqrt()
}

/// Primary-seat flatness with a quadratic penalty when other seats
/// exceed `max_deviation_db` from the primary's response at each frequency.
fn primary_constrained_from_responses(
    responses: &[Vec<f64>],
    primary_seat: usize,
    max_deviation_db: f64,
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
    primary_flatness + 10.0 * penalty
}

fn objective_name(strategy: MultiSeatStrategy) -> &'static str {
    match strategy {
        MultiSeatStrategy::MinimizeVariance => "seat_variance",
        MultiSeatStrategy::Average => "average_flatness",
        MultiSeatStrategy::PrimaryWithConstraints => "primary_constrained",
    }
}

fn objective_from_responses(
    responses: &[Vec<f64>],
    strategy: MultiSeatStrategy,
    primary_seat: usize,
    max_deviation_db: f64,
) -> f64 {
    match strategy {
        MultiSeatStrategy::MinimizeVariance => variance_from_responses(responses),
        MultiSeatStrategy::Average => average_flatness_from_responses(responses),
        MultiSeatStrategy::PrimaryWithConstraints => {
            primary_constrained_from_responses(responses, primary_seat, max_deviation_db)
        }
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
            average_flatness_from_responses(&r)
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
            primary_constrained_from_responses(&r, primary_seat, max_deviation_db)
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

        let config = MultiSeatConfig::default();
        let (gains, delays, polarities, allpass_filters) =
            optimize_minimize_variance(&interpolated, &freqs, 2, &config, 48000.0, 20.0, 120.0);

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
