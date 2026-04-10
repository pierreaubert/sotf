//! Multi-subwoofer flat-response loss.

use super::flat::flat_loss;
use super::{DriversLossData, compute_drivers_combined_response};

/// Multi-subwoofer flat loss.
///
/// Computes the combined response of multiple subwoofers with configurable
/// gains and delays, normalizes it, and returns the flat loss (weighted MSE)
/// against a zero target over the evaluation range.
///
/// # Arguments
/// * `data` - Drivers loss data containing sub measurements and a frequency grid
/// * `gains` - Gain in dB for each sub
/// * `delays` - Delay in ms for each sub
/// * `sample_rate` - Sample rate
/// * `min_freq` - Min freq for evaluation
/// * `max_freq` - Max freq for evaluation
///
/// # Returns
/// * Loss value
pub fn multisub_flat_loss(
    data: &DriversLossData,
    gains: &[f64],
    delays: &[f64],
    sample_rate: f64,
    min_freq: f64,
    max_freq: f64,
) -> f64 {
    // Pass empty crossover freqs (ignored because CrossoverType::None)
    let crossover_freqs = vec![];
    let combined_response =
        compute_drivers_combined_response(data, gains, &crossover_freqs, Some(delays), sample_rate);

    // Normalize the response (subtract the mean in the evaluation range)
    let mut sum = 0.0;
    let mut count = 0;
    for i in 0..data.freq_grid.len() {
        let freq = data.freq_grid[i];
        if freq >= min_freq && freq <= max_freq {
            sum += combined_response[i];
            count += 1;
        }
    }
    let mean = if count > 0 { sum / count as f64 } else { 0.0 };
    let normalized = &combined_response - mean;

    // Compute flatness loss (RMS deviation from zero)
    flat_loss(&data.freq_grid, &normalized, min_freq, max_freq)
}
