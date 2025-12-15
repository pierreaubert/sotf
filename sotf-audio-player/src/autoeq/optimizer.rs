//! High-level Room EQ Optimizer API
//!
//! Provides an async-friendly interface for GPUI to run optimizations.
//!
//! Note: The actual optimization logic will be integrated later by adding
//! the `autoeq` crate as a dependency. For now, this provides the API
//! structure and stub implementations.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

use super::output;
use super::types::{
    ChannelConfig, ChannelMeasurements, ChannelOptStatus, ChannelOptimizationResult, CrossoverType,
    Curve, DspChainOutput, OptimizationProgress, OptimizerConfig, SpeakerConfigType,
};

/// Room EQ Optimizer
///
/// High-level API for optimizing room EQ across multiple channels.
/// Designed to be used from async contexts (GPUI).
pub struct RoomEqOptimizer {
    config: OptimizerConfig,
}

impl RoomEqOptimizer {
    /// Create a new optimizer with the given configuration
    pub fn new(config: OptimizerConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(OptimizerConfig::default())
    }

    /// Get the current configuration
    pub fn config(&self) -> &OptimizerConfig {
        &self.config
    }

    /// Update the configuration
    pub fn set_config(&mut self, config: OptimizerConfig) {
        self.config = config;
    }

    /// Optimize a single channel (single-driver speaker)
    ///
    /// # Arguments
    /// * `channel_name` - Name of the channel
    /// * `measurement` - Frequency response measurement
    ///
    /// # Returns
    /// * Optimization result with EQ filters
    pub fn optimize_single_channel(
        &self,
        channel_name: &str,
        measurement: &Curve,
    ) -> Result<ChannelOptimizationResult, String> {
        log::info!("Optimizing single channel: {}", channel_name);

        // Compute pre-score (simple RMS deviation from flat)
        let pre_score =
            compute_flat_deviation(measurement, self.config.min_freq, self.config.max_freq);

        // TODO: Integrate actual optimization from autoeq crate
        // For now, return a placeholder result
        log::warn!("Room EQ optimization not yet implemented - returning placeholder result");

        Ok(ChannelOptimizationResult {
            channel_name: channel_name.to_string(),
            pre_score,
            post_score: pre_score, // No improvement yet
            eq_filters: Vec::new(),
            biquads: Vec::new(),
            crossover_freqs: None,
            driver_gains: None,
            original_response: Some(measurement.clone()),
            corrected_response: Some(measurement.clone()),
        })
    }

    /// Optimize a multi-driver speaker group
    ///
    /// # Arguments
    /// * `channel_name` - Name of the channel
    /// * `driver_measurements` - Measurements for each driver (in order from low to high freq)
    /// * `crossover_type` - Type of crossover to use
    ///
    /// # Returns
    /// * Optimization result with crossover settings and EQ filters
    pub fn optimize_multidriver_channel(
        &self,
        channel_name: &str,
        driver_measurements: Vec<Curve>,
        crossover_type: CrossoverType,
    ) -> Result<ChannelOptimizationResult, String> {
        log::info!(
            "Optimizing multi-driver channel: {} ({} drivers, {:?})",
            channel_name,
            driver_measurements.len(),
            crossover_type
        );

        // Use first driver as combined for pre-score
        let combined = driver_measurements.first().cloned().unwrap_or_default();
        let pre_score =
            compute_flat_deviation(&combined, self.config.min_freq, self.config.max_freq);

        // Initial crossover frequencies (geometric mean between adjacent drivers)
        let crossover_freqs = compute_initial_crossover_freqs(&driver_measurements);
        let driver_gains = vec![0.0; driver_measurements.len()];

        // TODO: Integrate actual optimization from autoeq crate
        log::warn!(
            "Room EQ multi-driver optimization not yet implemented - returning placeholder result"
        );

        Ok(ChannelOptimizationResult {
            channel_name: channel_name.to_string(),
            pre_score,
            post_score: pre_score,
            eq_filters: Vec::new(),
            biquads: Vec::new(),
            crossover_freqs: Some(crossover_freqs),
            driver_gains: Some(driver_gains),
            original_response: Some(combined.clone()),
            corrected_response: Some(combined),
        })
    }

    /// Optimize all channels with progress reporting
    ///
    /// # Arguments
    /// * `channels` - Map of channel name to measurements
    /// * `configs` - Map of channel name to channel configuration
    /// * `progress_tx` - Channel to send progress updates
    ///
    /// # Returns
    /// * Map of channel name to optimization result
    pub async fn optimize_all_channels(
        &self,
        channels: HashMap<String, ChannelMeasurements>,
        configs: HashMap<String, ChannelConfig>,
        mut progress_tx: Option<mpsc::Sender<OptimizationProgress>>,
    ) -> Result<HashMap<String, ChannelOptimizationResult>, String> {
        let total_channels = channels.len();
        let mut results = HashMap::new();
        let mut channel_statuses: HashMap<String, ChannelOptStatus> = channels
            .keys()
            .map(|k| (k.clone(), ChannelOptStatus::Pending))
            .collect();

        for (i, (channel_name, measurements)) in channels.into_iter().enumerate() {
            // Update progress
            channel_statuses.insert(channel_name.clone(), ChannelOptStatus::OptimizingEq);

            if let Some(ref mut tx) = progress_tx {
                let progress = OptimizationProgress {
                    current_channel: Some(channel_name.clone()),
                    current_status: ChannelOptStatus::OptimizingEq,
                    overall_progress: i as f32 / total_channels as f32,
                    message: format!("Optimizing channel: {}", channel_name),
                    channel_statuses: channel_statuses.clone(),
                };
                let _ = tx.send(progress).await;
            }

            // Get channel config
            let config = configs.get(&channel_name);
            let is_multidriver = config
                .map(|c| c.config_type == SpeakerConfigType::MultiDriver)
                .unwrap_or(false);
            let crossover_type = config.and_then(|c| c.crossover_type).unwrap_or_default();

            // Run optimization
            let result = if is_multidriver && !measurements.drivers.is_empty() {
                // Multi-driver optimization
                let driver_curves: Vec<Curve> = measurements
                    .drivers
                    .iter()
                    .map(|m| m.curve.clone())
                    .collect();

                // Update status for crossover
                channel_statuses
                    .insert(channel_name.clone(), ChannelOptStatus::OptimizingCrossover);
                if let Some(ref mut tx) = progress_tx {
                    let progress = OptimizationProgress {
                        current_channel: Some(channel_name.clone()),
                        current_status: ChannelOptStatus::OptimizingCrossover,
                        overall_progress: (i as f32 + 0.3) / total_channels as f32,
                        message: format!("Optimizing crossover: {}", channel_name),
                        channel_statuses: channel_statuses.clone(),
                    };
                    let _ = tx.send(progress).await;
                }

                self.optimize_multidriver_channel(&channel_name, driver_curves, crossover_type)
            } else {
                // Single driver optimization
                self.optimize_single_channel(&channel_name, &measurements.main.curve)
            };

            match result {
                Ok(opt_result) => {
                    channel_statuses.insert(channel_name.clone(), ChannelOptStatus::Completed);
                    results.insert(channel_name, opt_result);
                }
                Err(e) => {
                    log::error!("Failed to optimize channel {}: {}", channel_name, e);
                    channel_statuses.insert(channel_name.clone(), ChannelOptStatus::Failed);
                    return Err(format!(
                        "Failed to optimize channel {}: {}",
                        channel_name, e
                    ));
                }
            }
        }

        // Final progress update
        if let Some(ref mut tx) = progress_tx {
            let progress = OptimizationProgress {
                current_channel: None,
                current_status: ChannelOptStatus::Completed,
                overall_progress: 1.0,
                message: "Optimization complete".to_string(),
                channel_statuses,
            };
            let _ = tx.send(progress).await;
        }

        Ok(results)
    }

    /// Generate DSP chain output from optimization results
    pub fn generate_dsp_output(
        &self,
        results: &HashMap<String, ChannelOptimizationResult>,
        crossover_types: &HashMap<String, CrossoverType>,
    ) -> DspChainOutput {
        output::build_dsp_chain_output(
            results,
            crossover_types,
            self.config.algorithm.to_string_id(),
            self.config.max_iter,
        )
    }
}

impl Default for RoomEqOptimizer {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Compute flat deviation score (RMS deviation from mean in frequency range)
fn compute_flat_deviation(curve: &Curve, min_freq: f64, max_freq: f64) -> f64 {
    if curve.is_empty() {
        return 0.0;
    }

    // Compute mean in range
    let mut sum = 0.0;
    let mut count = 0;
    for i in 0..curve.freq.len() {
        if curve.freq[i] >= min_freq && curve.freq[i] <= max_freq {
            sum += curve.spl[i];
            count += 1;
        }
    }

    if count == 0 {
        return 0.0;
    }

    let mean = sum / count as f64;

    // Compute RMS deviation
    let mut sq_sum = 0.0;
    for i in 0..curve.freq.len() {
        if curve.freq[i] >= min_freq && curve.freq[i] <= max_freq {
            let diff = curve.spl[i] - mean;
            sq_sum += diff * diff;
        }
    }

    (sq_sum / count as f64).sqrt()
}

/// Compute initial crossover frequencies based on driver frequency ranges
fn compute_initial_crossover_freqs(drivers: &[Curve]) -> Vec<f64> {
    let mut freqs = Vec::new();
    for i in 0..(drivers.len().saturating_sub(1)) {
        // Geometric mean between adjacent driver frequency ranges
        let lower_mean = drivers[i].freq.iter().sum::<f64>() / drivers[i].freq.len().max(1) as f64;
        let upper_mean =
            drivers[i + 1].freq.iter().sum::<f64>() / drivers[i + 1].freq.len().max(1) as f64;
        let geom_mean = (lower_mean * upper_mean).sqrt();
        freqs.push(geom_mean);
    }
    freqs
}

/// Async task that runs optimization in a background thread
///
/// Use this from GPUI to avoid blocking the UI thread.
pub async fn run_optimization_task(
    optimizer: Arc<RoomEqOptimizer>,
    channels: HashMap<String, ChannelMeasurements>,
    configs: HashMap<String, ChannelConfig>,
    progress_tx: mpsc::Sender<OptimizationProgress>,
) -> Result<HashMap<String, ChannelOptimizationResult>, String> {
    optimizer
        .optimize_all_channels(channels, configs, Some(progress_tx))
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    fn _create_test_curve() -> Curve {
        let freq: Array1<f64> = Array1::linspace(20.0, 20000.0, 100);
        let mut spl: Array1<f64> = Array1::zeros(100);
        // Add some peaks and dips
        for i in 0..100 {
            let f = freq[i];
            spl[i] = 3.0 * (f / 1000.0_f64).ln().sin();
        }
        Curve { freq, spl }
    }

    #[test]
    fn test_optimizer_creation() {
        let optimizer = RoomEqOptimizer::with_defaults();
        assert_eq!(optimizer.config().num_filters, 10);
    }

    #[test]
    fn test_flat_deviation() {
        let freq = Array1::linspace(20.0, 20000.0, 100);
        let spl = Array1::zeros(100);
        let curve = Curve { freq, spl };

        let score = compute_flat_deviation(&curve, 100.0, 10000.0);
        assert!(score < 0.01, "Flat curve should have near-zero deviation");
    }

    #[test]
    fn test_initial_crossover_freqs() {
        let woofer = Curve {
            freq: Array1::linspace(20.0, 2000.0, 100),
            spl: Array1::zeros(100),
        };
        let tweeter = Curve {
            freq: Array1::linspace(1000.0, 20000.0, 100),
            spl: Array1::zeros(100),
        };

        let freqs = compute_initial_crossover_freqs(&[woofer, tweeter]);
        assert_eq!(freqs.len(), 1);
        assert!(freqs[0] > 500.0 && freqs[0] < 5000.0);
    }
}
