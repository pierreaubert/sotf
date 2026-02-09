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
use log::warn;
use std::error::Error;

/// Parse crossover type from string
pub fn parse_crossover_type(type_str: &str) -> Result<CrossoverType, Box<dyn Error>> {
    match type_str.to_lowercase().as_str() {
        "butterworth2" | "bw2" | "butterworth12" => Ok(CrossoverType::Butterworth2),
        "lr2" | "linkwitzriley2" | "linkwitzriley12" => Ok(CrossoverType::LinkwitzRiley2),
        "lr4" | "lr24" | "linkwitzriley4" | "linkwitzriley24" => Ok(CrossoverType::LinkwitzRiley4),
        _ => Err(format!("Unknown crossover type: {}", type_str).into()),
    }
}

/// Convert CrossoverType enum to plugin string format
pub fn crossover_type_to_string(ct: &CrossoverType) -> &'static str {
    match ct {
        CrossoverType::Butterworth2 => "Butterworth12",
        CrossoverType::LinkwitzRiley2 => "LR12",
        CrossoverType::LinkwitzRiley4 => "LR24",
        CrossoverType::None => "None",
    }
}

use super::types::OptimizerConfig;

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
    let (xover_min_freq, xover_max_freq) = crossover_freq_range
        .unwrap_or((config.min_freq, config.max_freq));

    for i in 0..num_combinations {
        // Driver 0 is always normal (false)
        // Driver k (k>0) is inverted if bit (k-1) is set
        let mut inversions = vec![false; n_drivers];
        for k in 1..n_drivers {
            if (i >> (k - 1)) & 1 == 1 {
                inversions[k] = true;
            }
        }

        // Create modified drivers with inverted phase where needed
        let modified_drivers: Vec<DriverMeasurement> = sorted_drivers.iter().enumerate().map(|(idx, curve)| {
            let mut new_curve = curve.clone();
            if inversions[idx] {
                if let Some(ref p) = new_curve.phase {
                    new_curve.phase = Some(p.mapv(|x| x + 180.0));
                } else {
                    // Create phase array of 180s if missing
                    new_curve.phase = Some(ndarray::Array1::from_elem(new_curve.freq.len(), 180.0));
                }
            }
            
            DriverMeasurement {
                freq: new_curve.freq,
                spl: new_curve.spl,
                phase: new_curve.phase,
            }
        }).collect();

        // Note: DriversLossData::new sorts internally, but we already sorted, so order is preserved.
        let drivers_data = DriversLossData::new(modified_drivers, crossover_type);

        // Validate fixed frequencies size match
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

        // Run optimization
        let result = crate::workflow::optimize_drivers_crossover(
            drivers_data.clone(),
            xover_min_freq,
            xover_max_freq,
            sample_rate,
            &config.algorithm,
            config.max_iter,
            config.min_db,
            config.max_db,
            fixed_freqs.clone(),
        )?;

        match best_opt {
            None => {
                best_opt = Some(OptimizationResult { result, inversions, data: drivers_data });
            }
            Some(ref current_best) => {
                if result.post_objective < current_best.result.post_objective {
                    best_opt = Some(OptimizationResult { result, inversions, data: drivers_data });
                }
            }
        }
    }

    let best = best_opt.ok_or("Optimization failed to produce any result")?;
    let result = best.result;
    let sorted_inversions = best.inversions;
    let drivers_data = best.data; // Use the data that produced the best result (includes correct phases)

    eprintln!(
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
    let combined_response = crate::loss::compute_drivers_combined_response(
        &drivers_data,
        &result.gains,
        &result.crossover_freqs,
        Some(&result.delays),
        sample_rate,
    );

    let combined_curve = Curve {
        freq: drivers_data.freq_grid.clone(),
        spl: combined_response,
        phase: None,
    };

    eprintln!(
        "  Crossover optimization: gains={:?}, delays={:?} ms, freqs={:?}, inverts={:?}, final loss={:.6}",
        result.gains.iter().map(|g| format!("{:+.2}", g)).collect::<Vec<_>>(),
        result.delays.iter().map(|d| format!("{:.2}", d)).collect::<Vec<_>>(),
        result.crossover_freqs.iter().map(|f| format!("{:.0}", f)).collect::<Vec<_>>(),
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

    #[test]
    fn test_parse_crossover_type() {
        assert!(matches!(
            parse_crossover_type("lr24"),
            Ok(CrossoverType::LinkwitzRiley4)
        ));
        assert!(matches!(
            parse_crossover_type("LR4"),
            Ok(CrossoverType::LinkwitzRiley4)
        ));
        assert!(matches!(
            parse_crossover_type("butterworth2"),
            Ok(CrossoverType::Butterworth2)
        ));
        assert!(parse_crossover_type("invalid").is_err());
    }
}