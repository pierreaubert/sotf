//! Smart initial guess generation for filter optimization
//!
//! This module provides intelligent initialization strategies for filter parameters
//! based on frequency response analysis.

use ndarray::Array1;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Smart initialization configuration
#[derive(Debug, Clone)]
pub struct SmartInitConfig {
    /// Number of different initial guesses to generate
    pub num_guesses: usize,
    /// Sigma for Gaussian smoothing of frequency response
    pub smoothing_sigma: f64,
    /// Minimum peak/dip height to consider
    pub min_peak_height: f64,
    /// Minimum distance between peaks/dips (in frequency points)
    pub min_peak_distance: usize,
    /// Critical frequencies to always consider (Hz)
    pub critical_frequencies: Vec<f64>,
    /// Random variation factor for guess diversification
    pub variation_factor: f64,
    /// Random seed for deterministic initialization (None = random)
    pub seed: Option<u64>,
}

impl Default for SmartInitConfig {
    fn default() -> Self {
        Self {
            num_guesses: 5,
            smoothing_sigma: 2.0,
            min_peak_height: 1.0,
            min_peak_distance: 10,
            critical_frequencies: vec![100.0, 300.0, 1000.0, 3000.0, 8000.0, 16000.0],
            variation_factor: 0.1,
            seed: None,
        }
    }
}

/// Frequency problem descriptor for smart initialization
#[derive(Debug, Clone)]
struct FrequencyProblem {
    /// Frequency in Hz
    frequency: f64,
    /// Magnitude of the problem (positive = boost needed, negative = cut needed)
    magnitude: f64,
    /// Suggested Q factor for this frequency
    q_factor: f64,
}

/// Smooth array with Gaussian kernel
fn smooth_gaussian(data: &Array1<f64>, sigma: f64) -> Array1<f64> {
    // Simple moving average as approximation
    let window = (sigma * 3.0) as usize;
    let mut smoothed = data.clone();

    for i in 0..data.len() {
        let start = i.saturating_sub(window);
        let end = (i + window + 1).min(data.len());
        let sum: f64 = data.slice(ndarray::s![start..end]).sum();
        smoothed[i] = sum / (end - start) as f64;
    }

    smoothed
}

/// Find peaks in a signal
fn find_peaks(data: &Array1<f64>, min_height: f64, min_distance: usize) -> Vec<usize> {
    let mut peaks = Vec::new();
    let n = data.len();

    if n < 3 {
        return peaks;
    }

    for i in 1..n - 1 {
        // Check if local maximum
        if data[i] > data[i - 1] && data[i] > data[i + 1] && data[i] >= min_height {
            // Check minimum distance to previous peak
            if peaks.is_empty() || i - peaks[peaks.len() - 1] >= min_distance {
                peaks.push(i);
            }
        }
    }

    peaks
}

/// Create smart initial guesses for filter optimization
///
/// Analyzes the target frequency response to identify problematic frequencies
/// (peaks that need cuts and dips that need boosts) and generates initial
/// filter parameter guesses targeting these problems.
///
/// # Arguments
/// * `target_response` - Target frequency response to analyze
/// * `freq_grid` - Frequency grid corresponding to the response
/// * `num_filters` - Number of filters to optimize
/// * `bounds` - Parameter bounds for validation
/// * `config` - Smart initialization configuration
/// * `peq_model` - PEQ model to determine parameter layout
///
/// # Returns
/// Vector of initial guess parameter vectors
pub fn create_smart_initial_guesses(
    target_response: &Array1<f64>,
    freq_grid: &Array1<f64>,
    num_filters: usize,
    bounds: &[(f64, f64)],
    config: &SmartInitConfig,
    peq_model: crate::cli::PeqModel,
) -> Vec<Vec<f64>> {
    // Create RNG based on config seed
    let mut main_rng: Box<dyn rand::RngCore> = if let Some(seed) = config.seed {
        Box::new(StdRng::seed_from_u64(seed))
    } else {
        Box::new(rand::rng())
    };
    // Smooth the response to reduce noise
    let smoothed = smooth_gaussian(target_response, config.smoothing_sigma);

    // Find peaks (need cuts) and dips (need boosts)
    let peaks = find_peaks(&smoothed, config.min_peak_height, config.min_peak_distance);
    let inverted = -&smoothed;
    let dips = find_peaks(&inverted, config.min_peak_height, config.min_peak_distance);

    let mut problems = Vec::new();

    // Add peaks (need cuts)
    for &peak_idx in &peaks {
        if peak_idx < freq_grid.len() {
            problems.push(FrequencyProblem {
                frequency: freq_grid[peak_idx],
                magnitude: -smoothed[peak_idx].abs(), // Negative for cuts
                q_factor: 1.0,
            });
        }
    }

    // Add dips (need boosts)
    for &dip_idx in &dips {
        if dip_idx < freq_grid.len() {
            problems.push(FrequencyProblem {
                frequency: freq_grid[dip_idx],
                magnitude: smoothed[dip_idx].abs(), // Positive for boosts
                q_factor: 0.7,                      // Lower Q for boosts
            });
        }
    }

    // Sort by magnitude (most problematic first)
    problems.sort_by(|a, b| {
        b.magnitude
            .abs()
            .partial_cmp(&a.magnitude.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Generate initial guesses
    let mut initial_guesses = Vec::new();
    let params_per_filter = crate::param_utils::params_per_filter(peq_model);

    for _guess_idx in 0..config.num_guesses {
        let mut guess = Vec::with_capacity(num_filters * params_per_filter);
        let mut used_problems = problems.clone();

        // Fill with critical frequencies if not enough problems found
        while used_problems.len() < num_filters {
            for &critical_freq in &config.critical_frequencies {
                if critical_freq >= freq_grid[0] && critical_freq <= freq_grid[freq_grid.len() - 1]
                {
                    used_problems.push(FrequencyProblem {
                        frequency: critical_freq,
                        magnitude: 0.5,
                        q_factor: 1.0,
                    });
                }
                if used_problems.len() >= num_filters {
                    break;
                }
            }

            // Fill remaining with random frequencies if needed
            while used_problems.len() < num_filters {
                let rand_freq = main_rng.random_range(freq_grid[0]..freq_grid[freq_grid.len() - 1]);
                used_problems.push(FrequencyProblem {
                    frequency: rand_freq,
                    magnitude: main_rng.random_range(-2.0..2.0),
                    q_factor: 1.0,
                });
            }
        }

        // Create parameter vector for this guess
        for i in 0..num_filters {
            let problem = &used_problems[i % used_problems.len()];

            // Add some randomization to diversify guesses
            let freq_var = problem.frequency
                * (1.0 + main_rng.random_range(-config.variation_factor..config.variation_factor));
            let gain_var = problem.magnitude * (1.0 + main_rng.random_range(-0.2..0.2));
            let q_var = problem.q_factor * (1.0 + main_rng.random_range(-0.3..0.3));

            // Convert to log10(freq) and constrain to bounds based on model
            match peq_model {
                crate::cli::PeqModel::Pk
                | crate::cli::PeqModel::HpPk
                | crate::cli::PeqModel::HpPkLp
                | crate::cli::PeqModel::LsPk
                | crate::cli::PeqModel::LsPkHs => {
                    // Fixed filter types: [freq, Q, gain]
                    let base_idx = i * 3;
                    let log_freq = freq_var
                        .log10()
                        .max(bounds[base_idx].0)
                        .min(bounds[base_idx].1);
                    let q_constrained = q_var
                        .max(bounds[base_idx + 1].0)
                        .min(bounds[base_idx + 1].1);
                    let gain_constrained = gain_var
                        .max(bounds[base_idx + 2].0)
                        .min(bounds[base_idx + 2].1);
                    guess.extend_from_slice(&[log_freq, q_constrained, gain_constrained]);
                }
                crate::cli::PeqModel::FreePkFree | crate::cli::PeqModel::Free => {
                    // Free filter types: [type, freq, Q, gain]
                    let base_idx = i * 4;
                    let filter_type = 0.0; // Default to Peak filter
                    let log_freq = freq_var
                        .log10()
                        .max(bounds[base_idx + 1].0)
                        .min(bounds[base_idx + 1].1);
                    let q_constrained = q_var
                        .max(bounds[base_idx + 2].0)
                        .min(bounds[base_idx + 2].1);
                    let gain_constrained = gain_var
                        .max(bounds[base_idx + 3].0)
                        .min(bounds[base_idx + 3].1);
                    guess.extend_from_slice(&[
                        filter_type,
                        log_freq,
                        q_constrained,
                        gain_constrained,
                    ]);
                }
            }
        }

        initial_guesses.push(guess);
    }

    initial_guesses
}

/// Generate integrality constraints for filter optimization
///
/// In the AutoEQ parameter encoding:
/// - Parameter 1 (frequency index): integer (when using frequency indexing)
/// - Parameter 2 (Q factor): continuous
/// - Parameter 3 (gain): continuous
///
/// # Arguments
/// * `num_filters` - Number of filters
/// * `use_freq_indexing` - Whether frequency is encoded as integer index (true) or continuous log10(Hz) (false)
///
/// # Returns
/// Vector of boolean values: true for integer parameters, false for continuous
pub fn generate_integrality_constraints(num_filters: usize, use_freq_indexing: bool) -> Vec<bool> {
    let mut constraints = Vec::with_capacity(num_filters * 4);

    for _i in 0..num_filters {
        // constraints.push(true);  // Filter type - integer, not yet implemented
        constraints.push(use_freq_indexing); // Frequency - integer if indexing, continuous if log10(Hz)
        constraints.push(false); // Q factor - continuous
        constraints.push(false); // Gain - continuous
    }

    constraints
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    #[test]
    fn test_generate_integrality_constraints() {
        let constraints = generate_integrality_constraints(2, true);
        // 2 filters × 3 params each = 6 total params
        // Pattern: [true, false, false] repeated (freq indexed, Q continuous, gain continuous)
        assert_eq!(constraints.len(), 6);
        assert_eq!(constraints[0], true); // Frequency (indexed)
        assert_eq!(constraints[1], false); // Q factor (continuous)
        assert_eq!(constraints[2], false); // Gain (continuous)

        // Second filter
        assert_eq!(constraints[3], true); // Frequency (indexed)
        assert_eq!(constraints[4], false); // Q factor (continuous)
        assert_eq!(constraints[5], false); // Gain (continuous)

        // Test continuous frequency case
        let constraints_continuous = generate_integrality_constraints(2, false);
        assert_eq!(constraints_continuous.len(), 6);
        assert_eq!(constraints_continuous[0], false); // Frequency (continuous)
        assert_eq!(constraints_continuous[1], false); // Q factor (continuous)
        assert_eq!(constraints_continuous[2], false); // Gain (continuous)
        assert_eq!(constraints_continuous[3], false); // Frequency (continuous)
        assert_eq!(constraints_continuous[4], false); // Q factor (continuous)
        assert_eq!(constraints_continuous[5], false); // Gain (continuous)
    }

    #[test]
    fn test_create_smart_initial_guesses() {
        use crate::cli::PeqModel;

        // Create a simple test case with a peak and dip
        let target_response = Array1::from(vec![0.0, 3.0, 0.0, -2.0, 0.0]);
        let freq_grid = Array1::from(vec![100.0, 200.0, 400.0, 800.0, 1600.0]);
        let bounds = vec![
            (100.0_f64.log10(), 1600.0_f64.log10()), // log10(freq)
            (0.5, 3.0),                              // Q
            (-6.0, 6.0),                             // Gain
        ];
        let config = SmartInitConfig::default();

        let guesses = create_smart_initial_guesses(
            &target_response,
            &freq_grid,
            1,
            &bounds,
            &config,
            PeqModel::Pk,
        );

        assert_eq!(guesses.len(), config.num_guesses);
        for guess in &guesses {
            assert_eq!(guess.len(), 3); // 1 filter × 3 params
            // Check bounds
            assert!(guess[0] >= bounds[0].0 && guess[0] <= bounds[0].1);
            assert!(guess[1] >= bounds[1].0 && guess[1] <= bounds[1].1);
            assert!(guess[2] >= bounds[2].0 && guess[2] <= bounds[2].1);
        }
    }
}
