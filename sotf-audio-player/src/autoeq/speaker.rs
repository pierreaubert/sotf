//! Speaker EQ optimization
//!
//! Provides the optimization logic and result types for speaker equalization.
//! This module contains the business logic that can be used by any frontend (GPUI, TUI, etc.)

use super::params::OptimizationParams;

/// Result of a speaker optimization run
#[derive(Clone, Debug)]
pub struct SpeakerOptimizationResult {
    pub biquads: Vec<autoeq_iir::Biquad>,
    pub frequencies: Vec<f64>,
    pub input_curve: Vec<f64>,     // On-axis or listening window
    pub target_curve: Vec<f64>,    // Calculated target
    pub deviation_curve: Vec<f64>, // Input - Target
    pub filter_response: Vec<f64>, // Sum of biquads
    pub error_curve: Vec<f64>,     // Deviation + Filter
    pub corrected_curve: Vec<f64>, // Input + Filter
    pub individual_filter_responses: Vec<Vec<f64>>,
    pub output_path: String,

    // Spinorama specific curves
    pub er_curve: Vec<f64>,    // Early Reflections
    pub sp_curve: Vec<f64>,    // Sound Power
    pub er_di_curve: Vec<f64>, // Early Reflections Directivity Index
    pub sp_di_curve: Vec<f64>, // Sound Power Directivity Index

    pub optimization_history: Vec<(usize, f64)>,
    pub initial_loss: f64,
    pub final_loss: f64,
}

/// Run speaker optimization (currently a stub with dummy data)
///
/// # Arguments
/// * `speaker_model` - The speaker model name
/// * `params` - Optimization parameters
///
/// # Returns
/// The optimization result with all curves for visualization
pub fn run_speaker_optimization(
    speaker_model: &str,
    _params: &OptimizationParams,
) -> Result<SpeakerOptimizationResult, String> {
    // Simulate delay for dummy task
    if speaker_model == "Dummy Speaker" {
        return Ok(generate_dummy_result());
    }

    Err("Downloading speaker data is not yet implemented. Please select 'Dummy Speaker' for UI testing.".to_string())
}

fn generate_dummy_result() -> SpeakerOptimizationResult {
    let n = 200;
    let frequencies: Vec<f64> = (0..n)
        .map(|i| 20.0 * (1000.0f64).powf(i as f64 / n as f64))
        .collect();
    let input_curve: Vec<f64> = frequencies
        .iter()
        .map(|f| (f / 1000.0).sin() * 5.0)
        .collect();
    let target_curve: Vec<f64> = vec![0.0; n];

    SpeakerOptimizationResult {
        biquads: Vec::new(),
        frequencies: frequencies.clone(),
        input_curve: input_curve.clone(),
        target_curve: target_curve.clone(),
        deviation_curve: input_curve.clone(),
        filter_response: vec![0.0; n],
        error_curve: input_curve.clone(),
        corrected_curve: input_curve.clone(),
        individual_filter_responses: Vec::new(),
        output_path: "/tmp/speaker_eq.txt".to_string(),
        er_curve: input_curve.iter().map(|v| v - 3.0).collect(),
        sp_curve: input_curve.iter().map(|v| v - 5.0).collect(),
        er_di_curve: vec![3.0; n],
        sp_di_curve: vec![5.0; n],
        optimization_history: vec![(0, 1.0), (10, 0.5), (20, 0.1)],
        initial_loss: 1.0,
        final_loss: 0.1,
    }
}
