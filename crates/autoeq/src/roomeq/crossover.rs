//! Crossover optimization for multi-driver groups
//!
//! # Phase Data Requirement
//!
//! Multi-driver crossover optimization uses complex summation to model
//! interference between drivers at crossover frequencies. For accurate
//! optimization, measurements should include phase data. Without phase data,
//! the optimizer assumes 0° phase, which may result in suboptimal crossover
//! frequencies, gains, and delays.

use crate::Curve;
use crate::loss::{CrossoverType, DriverMeasurement, DriversLossData};
use log::{info, warn};
use ndarray::Array1;
use std::error::Error;

use super::types::OptimizerConfig;

/// Apply polarity inversion to a driver curve.
///
/// When phase data is present, adds 180° to model polarity inversion.
/// When phase is missing, uses a constant 180° phase (pure polarity inversion)
/// rather than adding 180° to minimum-phase reconstruction, which would break
/// the Hilbert-transform relationship between log-magnitude and phase.
fn apply_polarity_inversion_to_driver(curve: &Curve, inverted: bool) -> DriverMeasurement {
    let mut new_curve = curve.clone();
    if inverted {
        let n = new_curve.freq.len();
        let phase = new_curve
            .phase
            .clone()
            .unwrap_or_else(|| Array1::from_elem(n, 0.0));
        new_curve.phase = Some(phase.mapv(|x| x + 180.0));
    }

    DriverMeasurement {
        freq: new_curve.freq,
        spl: new_curve.spl,
        phase: new_curve.phase,
    }
}

/// Optimize crossover for a group of driver measurements using autoeq's workflow
///
/// # Arguments
/// * `drivers` - Vector of driver measurements
/// * `crossover_type` - Type of crossover to use
/// * `sample_rate` - Sample rate for filter design
/// * `config` - Optimizer configuration
/// * `fixed_freqs` - Optional fixed crossover frequencies (skips frequency optimization)
/// * `crossover_freq_range` - Optional (min, max) frequency range for crossover optimization
///   (overrides config.min_freq/max_freq for the crossover search bounds)
///
/// # Returns
/// * Tuple of (optimal_gains, optimal_delays, optimal_crossover_freqs, combined_curve, inversions)
///
/// # Note on Phase Data
/// For accurate crossover optimization, measurements should include phase data.
/// The optimizer uses complex summation to model interference between drivers
/// at crossover frequencies.
#[allow(clippy::type_complexity)]
pub fn optimize_crossover(
    drivers: Vec<Curve>,
    crossover_type: CrossoverType,
    sample_rate: f64,
    config: &OptimizerConfig,
    fixed_freqs: Option<Vec<f64>>,
    crossover_freq_range: Option<(f64, f64)>,
) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>, Curve, Vec<bool>), Box<dyn Error>> {
    // Check for missing phase data and warn
    let missing_phase_count = drivers.iter().filter(|c| c.phase.is_none()).count();
    if missing_phase_count > 0 {
        warn!(
            "Crossover optimization: {} of {} driver measurements are missing phase data. \
            This may result in suboptimal crossover frequencies and driver alignment. \
            For best results, include phase data in your measurements.",
            missing_phase_count,
            drivers.len()
        );
    }

    let n_drivers = drivers.len();
    if n_drivers == 0 {
        return Err("No drivers provided".into());
    }

    // 1. Determine sort order (Low to High freq)
    // We need to pass sorted drivers to the optimizer, but return results in original order.
    let mut permutation: Vec<usize> = (0..n_drivers).collect();

    // Helper to get mean freq of a curve
    let get_mean_freq = |c: &Curve| {
        let min_f = c.freq.iter().copied().fold(f64::INFINITY, f64::min);
        let max_f = c.freq.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (min_f * max_f).sqrt()
    };

    permutation.sort_by(|&a, &b| {
        get_mean_freq(&drivers[a])
            .partial_cmp(&get_mean_freq(&drivers[b]))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let sorted_drivers: Vec<Curve> = permutation.iter().map(|&i| drivers[i].clone()).collect();

    // 2. Try polarity combinations on SORTED drivers
    // For N drivers, we have 2^(N-1) combinations (driver 0 fixed as reference)
    let num_combinations = 1 << (n_drivers - 1);

    struct OptimizationResult {
        result: crate::workflow::DriverOptimizationResult,
        inversions: Vec<bool>,
        data: DriversLossData,
    }

    let mut best_opt: Option<OptimizationResult> = None;

    // Use crossover-specific frequency range if provided, otherwise fall back to config
    let (xover_min_freq, xover_max_freq) =
        crossover_freq_range.unwrap_or((config.min_freq, config.max_freq));

    // Validate fixed frequencies size match once; it does not depend on polarity.
    if let Some(ref freqs) = fixed_freqs {
        let expected = n_drivers - 1;
        if freqs.len() != expected {
            return Err(format!(
                "Expected {} crossover frequencies for {} drivers, got {}",
                expected,
                n_drivers,
                freqs.len()
            )
            .into());
        }
    }

    for i in 0..num_combinations {
        // Driver 0 is always normal (false)
        // Driver k (k>0) is inverted if bit (k-1) is set
        let mut inversions = vec![false; n_drivers];
        for (k, inv) in inversions.iter_mut().enumerate().skip(1) {
            if (i >> (k - 1)) & 1 == 1 {
                *inv = true;
            }
        }

        // Create modified drivers with inverted phase where needed
        let modified_drivers: Vec<DriverMeasurement> = sorted_drivers
            .iter()
            .enumerate()
            .map(|(idx, curve)| apply_polarity_inversion_to_driver(curve, inversions[idx]))
            .collect();

        // Note: DriversLossData::new sorts internally, but we already sorted, so order is preserved.
        let drivers_data = DriversLossData::new(modified_drivers, crossover_type);

        // Run optimization
        let result = crate::workflow::optimize_drivers_crossover(
            drivers_data.clone(),
            xover_min_freq,
            xover_max_freq,
            sample_rate,
            &config.algorithm,
            config.max_iter,
            config.population,
            config.min_db,
            config.max_db,
            fixed_freqs.clone(),
            config.seed,
        )?;

        match best_opt {
            None => {
                best_opt = Some(OptimizationResult {
                    result,
                    inversions,
                    data: drivers_data,
                });
            }
            Some(ref current_best) => {
                if result.post_objective < current_best.result.post_objective {
                    best_opt = Some(OptimizationResult {
                        result,
                        inversions,
                        data: drivers_data,
                    });
                }
            }
        }
    }

    let best = best_opt.ok_or("Optimization failed to produce any result")?;
    let result = best.result;
    let sorted_inversions = best.inversions;
    let drivers_data = best.data; // Use the data that produced the best result (includes correct phases)

    info!(
        "  Optimizing crossover for {} drivers ({:?}){}",
        n_drivers,
        crossover_type,
        if fixed_freqs.is_some() {
            " with fixed frequencies"
        } else {
            ""
        }
    );

    // Compute the combined response (using the best modified data)
    let combined_complex = crate::loss::compute_drivers_combined_response_complex(
        &drivers_data,
        &result.gains,
        &result.crossover_freqs,
        Some(&result.delays),
        sample_rate,
    );
    let combined_spl = combined_complex.mapv(|z| 20.0 * z.norm().max(1e-12).log10());
    let combined_phase = combined_complex.mapv(|z| z.arg().to_degrees());

    let combined_curve = Curve {
        freq: drivers_data.freq_grid.clone(),
        spl: combined_spl,
        phase: Some(combined_phase),
        ..Default::default()
    };

    info!(
        "  Crossover optimization: gains={:?}, delays={:?} ms, freqs={:?}, inverts={:?}, final loss={:.6}",
        result
            .gains
            .iter()
            .map(|g| format!("{:+.2}", g))
            .collect::<Vec<_>>(),
        result
            .delays
            .iter()
            .map(|d| format!("{:.2}", d))
            .collect::<Vec<_>>(),
        result
            .crossover_freqs
            .iter()
            .map(|f| format!("{:.0}", f))
            .collect::<Vec<_>>(),
        sorted_inversions,
        result.post_objective
    );

    // 3. Map results back to original order
    let mut final_gains = vec![0.0; n_drivers];
    let mut final_delays = vec![0.0; n_drivers];
    let mut final_inversions = vec![false; n_drivers];

    for (sorted_idx, &original_idx) in permutation.iter().enumerate() {
        final_gains[original_idx] = result.gains[sorted_idx];
        final_delays[original_idx] = result.delays[sorted_idx];
        final_inversions[original_idx] = sorted_inversions[sorted_idx];
    }

    Ok((
        final_gains,
        final_delays,
        result.crossover_freqs,
        combined_curve,
        final_inversions,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    #[test]
    fn polarity_inversion_with_missing_phase_uses_constant_180_deg() {
        let curve = Curve {
            freq: Array1::from_vec(vec![100.0, 1000.0]),
            spl: Array1::from_vec(vec![0.0, 0.0]),
            phase: None,
            ..Default::default()
        };

        let driver = apply_polarity_inversion_to_driver(&curve, true);

        let phase = driver.phase.expect("phase should be present");
        assert!((phase[0] - 180.0).abs() < 1e-9);
        assert!((phase[1] - 180.0).abs() < 1e-9);
    }

    #[test]
    fn polarity_inversion_with_existing_phase_adds_180_deg() {
        let curve = Curve {
            freq: Array1::from_vec(vec![100.0, 1000.0]),
            spl: Array1::from_vec(vec![0.0, 0.0]),
            phase: Some(Array1::from_vec(vec![30.0, -45.0])),
            ..Default::default()
        };

        let driver = apply_polarity_inversion_to_driver(&curve, true);

        let phase = driver.phase.expect("phase should be present");
        assert!((phase[0] - 210.0).abs() < 1e-9);
        assert!((phase[1] - 135.0).abs() < 1e-9);
    }

    #[test]
    fn no_polarity_inversion_preserves_missing_phase() {
        let curve = Curve {
            freq: Array1::from_vec(vec![100.0, 1000.0]),
            spl: Array1::from_vec(vec![0.0, 0.0]),
            phase: None,
            ..Default::default()
        };

        let driver = apply_polarity_inversion_to_driver(&curve, false);

        assert!(driver.phase.is_none());
    }

    #[test]
    fn combined_curve_preserves_phase_from_complex_sum() {
        let drivers = vec![
            Curve {
                freq: Array1::from_vec(vec![100.0, 1000.0]),
                spl: Array1::from_vec(vec![0.0, 0.0]),
                phase: Some(Array1::from_vec(vec![0.0, 0.0])),
                ..Default::default()
            },
            Curve {
                freq: Array1::from_vec(vec![100.0, 1000.0]),
                spl: Array1::from_vec(vec![0.0, 0.0]),
                phase: Some(Array1::from_vec(vec![180.0, 180.0])),
                ..Default::default()
            },
        ];

        let result = optimize_crossover(
            drivers,
            CrossoverType::None,
            48000.0,
            &OptimizerConfig {
                num_filters: 1,
                max_iter: 10,
                population: 4,
                seed: Some(42),
                ..Default::default()
            },
            None,
            None,
        );

        assert!(result.is_ok());
        let (_, _, _, combined_curve, _) = result.unwrap();
        assert!(
            combined_curve.phase.is_some(),
            "combined curve should preserve phase"
        );
    }

    #[test]
    fn test_parse_crossover_type() {
        assert!(matches!(
            "lr24".parse::<CrossoverType>(),
            Ok(CrossoverType::LinkwitzRiley4)
        ));
        assert!(matches!(
            "LR4".parse::<CrossoverType>(),
            Ok(CrossoverType::LinkwitzRiley4)
        ));
        assert!(matches!(
            "butterworth2".parse::<CrossoverType>(),
            Ok(CrossoverType::Butterworth2)
        ));
        assert!(matches!(
            "lr48".parse::<CrossoverType>(),
            Ok(CrossoverType::LinkwitzRiley8)
        ));
        assert!(matches!(
            "LinearPhase".parse::<CrossoverType>(),
            Ok(CrossoverType::LinearPhase)
        ));
        assert!("invalid".parse::<CrossoverType>().is_err());
    }
}
