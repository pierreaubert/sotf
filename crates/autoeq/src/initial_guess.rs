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
    /// Smoothing strength expressed as bands per octave for constant-octave smoothing.
    /// Values are rounded to the nearest integer and clamped to at least 1.
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
    /// Pre-detected frequency problems as `(frequency_hz, q, gain_db)`
    /// triples, typically produced by a higher-level analysis
    /// (e.g. roomeq's SSIR / decomposed-correction mode detection).
    ///
    /// When this list is **non-empty**, `create_smart_initial_guesses`
    /// uses these triples as the canonical "problems to correct"
    /// instead of running its own naive `find_peaks` over the
    /// smoothed deviation. This prevents the optimizer from placing
    /// filters at invented frequencies while the real room modes
    /// (typically detected with far better Q / prominence than
    /// find_peaks can infer) go unused.
    ///
    /// Order matters: the list is treated as "most important first"
    /// so when there are fewer filters than problems the top entries
    /// are the ones kept.
    ///
    /// Defaults to empty (old behaviour — auto-detect from the
    /// smoothed deviation).
    pub pre_detected_problems: Vec<(f64, f64, f64)>,
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
            pre_detected_problems: Vec::new(),
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

/// Smooth response on a log-frequency grid using constant-octave windows.
fn smooth_problem_response(freq_grid: &Array1<f64>, data: &Array1<f64>, sigma: f64) -> Array1<f64> {
    let curve = crate::Curve {
        freq: freq_grid.clone(),
        spl: data.clone(),
        phase: None,
        ..Default::default()
    };
    let bands_per_octave = sigma.max(1.0).round() as usize;
    crate::read::smooth_one_over_n_octave(&curve, bands_per_octave).spl
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
    let mut problems: Vec<FrequencyProblem> = if !config.pre_detected_problems.is_empty() {
        // Caller already ran a higher-quality analysis (e.g. SSIR
        // room-mode detection) and handed us the problems explicitly.
        // Use those verbatim — they're almost certainly better than
        // anything `find_peaks` over a smoothed curve could recover.
        config
            .pre_detected_problems
            .iter()
            .map(|&(frequency, q, gain_db)| FrequencyProblem {
                frequency,
                magnitude: gain_db,
                q_factor: q,
            })
            .collect()
    } else {
        // Legacy path: smooth the response and locate peaks/dips
        // ourselves. Used when the caller has no upstream analysis.
        let smoothed = smooth_problem_response(freq_grid, target_response, config.smoothing_sigma);

        // The deviation curve is `target - normalized_measurement`.
        // Positive deviation = measurement below target → PEQ should boost.
        // Negative deviation = measurement above target → PEQ should cut.
        // Peaks in the deviation are boost targets; dips are cut targets.
        let peaks = find_peaks(&smoothed, config.min_peak_height, config.min_peak_distance);
        let inverted = -&smoothed;
        let dips = find_peaks(&inverted, config.min_peak_height, config.min_peak_distance);

        let mut auto_problems = Vec::new();

        // Add peaks in deviation (measurement below target → need boost)
        for &peak_idx in &peaks {
            if peak_idx < freq_grid.len() {
                auto_problems.push(FrequencyProblem {
                    frequency: freq_grid[peak_idx],
                    magnitude: smoothed[peak_idx], // Positive = boost
                    q_factor: 0.7,                 // Lower Q for boosts
                });
            }
        }

        // Add dips in deviation (measurement above target → need cut)
        for &dip_idx in &dips {
            if dip_idx < freq_grid.len() {
                auto_problems.push(FrequencyProblem {
                    frequency: freq_grid[dip_idx],
                    magnitude: smoothed[dip_idx], // Negative = cut
                    q_factor: 1.0,
                });
            }
        }

        auto_problems
    };

    // Sort by magnitude (most problematic first). The caller-supplied
    // list is already expected to be sorted, but re-sorting is cheap
    // and guarantees the invariant regardless of input order.
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
            let freq_scale = if config.variation_factor > 0.0 {
                1.0 + main_rng.random_range(-config.variation_factor..config.variation_factor)
            } else {
                1.0
            };
            let freq_var = problem.frequency * freq_scale;
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
        assert!(constraints[0]); // Frequency (indexed)
        assert!(!constraints[1]); // Q factor (continuous)
        assert!(!constraints[2]); // Gain (continuous)

        // Second filter
        assert!(constraints[3]); // Frequency (indexed)
        assert!(!constraints[4]); // Q factor (continuous)
        assert!(!constraints[5]); // Gain (continuous)

        // Test continuous frequency case
        let constraints_continuous = generate_integrality_constraints(2, false);
        assert_eq!(constraints_continuous.len(), 6);
        assert!(!constraints_continuous[0]); // Frequency (continuous)
        assert!(!constraints_continuous[1]); // Q factor (continuous)
        assert!(!constraints_continuous[2]); // Gain (continuous)
        assert!(!constraints_continuous[3]); // Frequency (continuous)
        assert!(!constraints_continuous[4]); // Q factor (continuous)
        assert!(!constraints_continuous[5]); // Gain (continuous)
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

    #[test]
    fn test_create_smart_initial_guesses_stable_across_grid_density() {
        use crate::cli::PeqModel;

        let coarse_freq_grid = Array1::from(vec![100.0, 300.0, 1000.0, 3000.0, 10000.0]);
        let coarse_response = Array1::from(vec![0.0, 0.0, 6.0, 0.0, 0.0]);

        let dense_freq_grid = Array1::logspace(10.0, 100.0_f64.log10(), 10_000.0_f64.log10(), 81);
        let dense_response = dense_freq_grid.mapv(|f| {
            let distance_oct = (f / 1000.0).log2();
            6.0 * (-0.5 * (distance_oct / 0.15).powi(2)).exp()
        });

        let bounds = vec![
            (100.0_f64.log10(), 10_000.0_f64.log10()),
            (0.5, 3.0),
            (-6.0, 6.0),
        ];
        let config = SmartInitConfig {
            num_guesses: 1,
            variation_factor: 0.0,
            seed: Some(42),
            ..SmartInitConfig::default()
        };

        let coarse_guess = create_smart_initial_guesses(
            &coarse_response,
            &coarse_freq_grid,
            1,
            &bounds,
            &config,
            PeqModel::Pk,
        )[0][0];
        let dense_guess = create_smart_initial_guesses(
            &dense_response,
            &dense_freq_grid,
            1,
            &bounds,
            &config,
            PeqModel::Pk,
        )[0][0];

        let coarse_freq = 10.0_f64.powf(coarse_guess);
        let dense_freq = 10.0_f64.powf(dense_guess);

        assert!(coarse_freq > 700.0 && coarse_freq < 1400.0);
        assert!(dense_freq > 700.0 && dense_freq < 1400.0);
        assert!((dense_freq / coarse_freq).log2().abs() < 0.2);
    }

    #[test]
    fn test_create_smart_initial_guesses_with_pre_detected_problems() {
        use crate::cli::PeqModel;
        let target_response = Array1::from(vec![0.0, 0.0, 0.0]);
        let freq_grid = Array1::from(vec![100.0, 1000.0, 10000.0]);
        let bounds = vec![
            (100.0_f64.log10(), 10000.0_f64.log10()),
            (0.5, 3.0),
            (-6.0, 6.0),
        ];
        let config = SmartInitConfig {
            num_guesses: 1,
            pre_detected_problems: vec![(500.0, 5.0, -3.0)],
            variation_factor: 0.0,
            seed: Some(42),
            ..SmartInitConfig::default()
        };
        let guesses = create_smart_initial_guesses(
            &target_response,
            &freq_grid,
            1,
            &bounds,
            &config,
            PeqModel::Pk,
        );
        assert_eq!(guesses.len(), 1);
        let freq = 10.0_f64.powf(guesses[0][0]);
        assert!(
            (freq - 500.0).abs() < 10.0,
            "should use pre-detected problem frequency, got {freq}"
        );
    }

    #[test]
    fn test_create_smart_initial_guesses_empty_response() {
        use crate::cli::PeqModel;
        let target_response = Array1::from(vec![0.0, 0.0, 0.0]);
        let freq_grid = Array1::from(vec![100.0, 1000.0, 10000.0]);
        // 2 filters * 3 params = 6 bounds entries
        let bounds = vec![
            (100.0_f64.log10(), 10000.0_f64.log10()),
            (0.5, 3.0),
            (-6.0, 6.0),
            (100.0_f64.log10(), 10000.0_f64.log10()),
            (0.5, 3.0),
            (-6.0, 6.0),
        ];
        let config = SmartInitConfig {
            num_guesses: 1,
            variation_factor: 0.0,
            seed: Some(42),
            ..SmartInitConfig::default()
        };
        let guesses = create_smart_initial_guesses(
            &target_response,
            &freq_grid,
            2,
            &bounds,
            &config,
            PeqModel::Pk,
        );
        assert_eq!(guesses.len(), 1);
        assert_eq!(guesses[0].len(), 6); // 2 filters * 3 params
        // All values should be within bounds
        for (i, &val) in guesses[0].iter().enumerate() {
            let (lo, hi) = bounds[i];
            assert!(
                val >= lo && val <= hi,
                "param {i} = {val} out of bounds [{lo}, {hi}]"
            );
        }
    }
}
