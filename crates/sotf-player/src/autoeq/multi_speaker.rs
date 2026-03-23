//! Multi-speaker / Room EQ optimization module
//!
//! Provides the ability to optimize EQ filters for multiple speakers simultaneously.
//! This module wraps the `autoeq::roomeq` library for room optimization.
//!
//! # Architecture
//!
//! The primary entry point is `run_room_optimization` which uses `autoeq::roomeq::optimize_room`
//! for parallel multi-channel optimization. The older `run_multi_speaker_optimization` is kept
//! for backward compatibility but delegates to the new implementation.
//!
//! # Usage
//!
//! ```ignore
//! use sotf_audio_player::autoeq::multi_speaker::{
//!     run_room_optimization, RoomOptimizationConfig, ChannelConfig,
//! };
//! use autoeq::roomeq::{RoomConfig, SpeakerConfig, OptimizerConfig, MeasurementSource};
//!
//! // Build room config with in-memory curves
//! let mut speakers = std::collections::HashMap::new();
//! speakers.insert("left".to_string(), SpeakerConfig::Single(
//!     MeasurementSource::InMemory(left_curve)
//! ));
//! speakers.insert("right".to_string(), SpeakerConfig::Single(
//!     MeasurementSource::InMemory(right_curve)
//! ));
//!
//! let config = RoomConfig {
//!     speakers,
//!     optimizer: OptimizerConfig::default(),
//!     ..Default::default()
//! };
//!
//! let result = run_room_optimization(&config, 48000.0, None)?;
//! ```

use std::collections::HashMap;

// Re-export roomeq types for convenience
// Note: Types are re-exported from autoeq::roomeq (not from the private types submodule)
pub use autoeq::roomeq::{
    // Progress and callback types
    CallbackAction,
    // Output types
    ChannelDspChain,
    // Result types
    ChannelOptimizationResult,
    DspChainOutput,
    PluginConfigWrapper,
    RoomOptimizationCallback,
    RoomOptimizationProgress,
    RoomOptimizationResult,
    SpeakerOptimizationResult as RoomSpeakerOptResult,
    // Optimization functions
    optimize_room,
    optimize_speaker,
    save_dsp_chain,
};

// Re-export types that are publicly available from autoeq::roomeq
pub use autoeq::roomeq::{
    CrossoverConfig,
    CurveData,
    DBAConfig,
    DriverDspChain,
    FirConfig,
    MeasurementSource,
    MultiSubGroup,
    OptimizationMetadata,
    OptimizerConfig,
    // V2 types
    ProcessingMode,
    // Config types (re-exported via `pub use types::*` in roomeq/mod.rs)
    RoomConfig,
    SpeakerConfig,
    SpeakerGroup,
    SubwooferStrategy,
    TargetCurveConfig,
};

use super::speaker::{CallbackConfig, OptimizationStage, SpeakerOptimizationResult};

// ============================================================================
// Room Optimization Entry Point
// ============================================================================

/// Run room optimization using autoeq::roomeq
///
/// This is the primary entry point for multi-channel room optimization.
/// Uses parallel processing via rayon for all speakers.
///
/// # Arguments
/// * `config` - Room configuration with speakers, optimizer settings, etc.
/// * `sample_rate` - Sample rate for filter design (e.g., 48000.0)
/// * `callback` - Optional progress callback
///
/// # Returns
/// Result containing per-channel DSP chains and optimization metrics
pub fn run_room_optimization(
    config: &RoomConfig,
    sample_rate: f64,
    callback: Option<RoomOptimizationCallback>,
) -> Result<RoomOptimizationResult, String> {
    optimize_room(config, sample_rate, callback, None).map_err(|e| e.to_string())
}

/// Convert RoomOptimizationResult to legacy SingleSpeakerResult format
///
/// This is useful for integrating with existing UI code that expects per-speaker results.
pub fn to_single_speaker_results(room_result: &RoomOptimizationResult) -> Vec<SingleSpeakerResult> {
    room_result
        .channel_results
        .iter()
        .map(|(name, channel_result)| {
            let n = channel_result.initial_curve.freq.len();
            SingleSpeakerResult {
                name: name.clone(),
                biquads: channel_result.biquads.clone(),
                initial_loss: channel_result.pre_score,
                final_loss: channel_result.post_score,
                frequencies: channel_result.initial_curve.freq.to_vec(),
                input_curve: channel_result.initial_curve.spl.to_vec(),
                target_curve: vec![0.0; n], // Target is flat (0 dB deviation)
                deviation_curve: vec![0.0; n],
                filter_response: vec![0.0; n],
                error_curve: vec![0.0; n],
                corrected_curve: channel_result.final_curve.spl.to_vec(),
                individual_filter_responses: Vec::new(),
            }
        })
        .collect()
}

// ============================================================================
// Legacy Data Structures (for backward compatibility)
// ============================================================================

/// Measurement data for a single speaker in multi-speaker optimization
#[derive(Debug, Clone)]
pub struct SpeakerMeasurementData {
    /// Speaker/channel name (e.g., "Left", "Right", "Center")
    pub name: String,
    /// Input measurement curve
    pub input_curve: autoeq::Curve,
    /// Target curve for this speaker
    pub target_curve: autoeq::Curve,
    /// Weight for this speaker in the combined loss (default 1.0)
    pub weight: f64,
}

impl SpeakerMeasurementData {
    /// Create a new speaker measurement with default weight
    pub fn new(name: &str, input_curve: autoeq::Curve, target_curve: autoeq::Curve) -> Self {
        Self {
            name: name.to_string(),
            input_curve,
            target_curve,
            weight: 1.0,
        }
    }

    /// Set the weight for this speaker
    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }
}

/// Configuration for multi-speaker optimization (legacy)
#[derive(Debug, Clone)]
pub struct MultiSpeakerOptimizationConfig {
    /// Speakers to optimize together
    pub speakers: Vec<SpeakerMeasurementData>,
    /// Optimization arguments (shared across all speakers)
    pub args: autoeq::Args,
    /// Callback configuration
    pub callback_config: Option<CallbackConfig>,
}

impl Default for MultiSpeakerOptimizationConfig {
    fn default() -> Self {
        Self {
            speakers: Vec::new(),
            args: autoeq::Args::speaker_defaults(),
            callback_config: Some(CallbackConfig::default()),
        }
    }
}

/// Result for a single speaker in multi-speaker optimization
#[derive(Clone, Debug)]
pub struct SingleSpeakerResult {
    /// Speaker name
    pub name: String,
    /// Optimized biquad filters for this speaker
    pub biquads: Vec<math_audio_iir_fir::Biquad>,
    /// Initial loss for this speaker
    pub initial_loss: f64,
    /// Final loss for this speaker
    pub final_loss: f64,
    /// Visualization curves
    pub frequencies: Vec<f64>,
    pub input_curve: Vec<f64>,
    pub target_curve: Vec<f64>,
    pub deviation_curve: Vec<f64>,
    pub filter_response: Vec<f64>,
    pub error_curve: Vec<f64>,
    pub corrected_curve: Vec<f64>,
    pub individual_filter_responses: Vec<Vec<f64>>,
}

/// Result of multi-speaker optimization
#[derive(Clone, Debug)]
pub struct MultiSpeakerOptimizationResult {
    /// Per-speaker optimization results
    pub speaker_results: Vec<SingleSpeakerResult>,
    /// Combined initial loss (weighted average)
    pub combined_initial_loss: f64,
    /// Combined final loss (weighted average)
    pub combined_final_loss: f64,
    /// Optimization history: (iteration, combined_loss)
    pub optimization_history: Vec<(usize, f64)>,
}

/// Progress update for multi-speaker optimization (legacy)
#[derive(Debug, Clone)]
pub struct MultiSpeakerProgress {
    /// Current iteration number (across all speakers)
    pub iteration: usize,
    /// Current combined loss value
    pub combined_loss: f64,
    /// Total iterations expected (maxeval * num_speakers)
    pub max_iterations: usize,
    /// Stage of optimization
    pub stage: OptimizationStage,
    /// Convergence metric
    pub convergence: f64,
}

/// Callback function type for multi-speaker optimization (legacy)
pub type MultiSpeakerOptimizationCallback =
    Box<dyn FnMut(&MultiSpeakerProgress) -> autoeq::de::CallbackAction + Send>;

// ============================================================================
// Legacy Entry Point (uses roomeq internally)
// ============================================================================

/// Run multi-speaker optimization (legacy API)
///
/// This function provides backward compatibility with the old API.
/// Internally converts to RoomConfig and calls optimize_room.
///
/// # Arguments
/// * `config` - Multi-speaker optimization configuration
/// * `callback` - Optional progress callback
///
/// # Returns
/// Result containing per-speaker filters and combined metrics
pub fn run_multi_speaker_optimization(
    config: &MultiSpeakerOptimizationConfig,
    callback: Option<MultiSpeakerOptimizationCallback>,
) -> Result<MultiSpeakerOptimizationResult, String> {
    if config.speakers.is_empty() {
        return Err("No speakers to optimize".to_string());
    }

    log::info!(
        "Starting multi-speaker optimization: {} speakers",
        config.speakers.len()
    );

    // Convert legacy config to RoomConfig
    let mut speakers_map: HashMap<String, SpeakerConfig> = HashMap::new();
    for speaker in &config.speakers {
        speakers_map.insert(
            speaker.name.clone(),
            SpeakerConfig::Single(MeasurementSource::InMemory(speaker.input_curve.clone())),
        );
    }

    let room_config = RoomConfig {
        version: autoeq::roomeq::default_config_version(),
        system: None,
        speakers: speakers_map,
        crossovers: None,
        target_curve: None,
        optimizer: OptimizerConfig {
            loss_type: format!("{:?}", config.args.loss).to_lowercase(),
            algorithm: config.args.algo.clone(),
            strategy: config.args.strategy.clone(),
            num_filters: config.args.num_filters,
            min_q: config.args.min_q,
            max_q: config.args.max_q,
            min_db: config.args.min_db,
            max_db: config.args.max_db,
            min_freq: config.args.min_freq,
            max_freq: config.args.max_freq,
            max_iter: config.args.maxeval,
            population: config.args.population,
            peq_model: format!("{:?}", config.args.peq_model).to_lowercase(),
            mode: "iir".to_string(),
            processing_mode: ProcessingMode::LowLatency,
            fir: None,
            seed: None,
            mixed_config: None,
            refine: true,
            local_algo: "cobyla".to_string(),
            psychoacoustic: true,
            smooth_n: config.args.smooth_n,
            asymmetric_loss: true,
            tolerance: config.args.tolerance,
            atolerance: config.args.atolerance,
            allow_delay: None,
            target_tilt: None,
            excursion_protection: None,
            schroeder_split: None,
            phase_alignment: None,
            multi_seat: None,
            gd_opt: None,
            vog: None,
            broadband_target_matching: None,
            multi_measurement: None,
            mixed_phase: None,
            decomposed_correction: None,
            target_response: None,
        },
        recording_config: None,
    };

    // Wrap legacy callback
    // Note: autoeq::de::CallbackAction and autoeq::roomeq::CallbackAction are different types
    // with the same variants, so we need to convert between them
    let callback_wrapped: Option<RoomOptimizationCallback> = callback.map(|mut cb| {
        let cb_wrapped: RoomOptimizationCallback =
            Box::new(move |progress: &RoomOptimizationProgress| {
                let legacy_progress = MultiSpeakerProgress {
                    iteration: progress.iteration,
                    combined_loss: progress.loss,
                    max_iterations: progress.max_iterations,
                    stage: OptimizationStage::Eq,
                    convergence: 0.0,
                };
                // Convert from de::CallbackAction to roomeq::CallbackAction
                match cb(&legacy_progress) {
                    autoeq::de::CallbackAction::Continue => CallbackAction::Continue,
                    autoeq::de::CallbackAction::Stop => CallbackAction::Stop,
                }
            });
        cb_wrapped
    });

    // Run optimization using roomeq
    let result = optimize_room(
        &room_config,
        config.args.sample_rate,
        callback_wrapped,
        None,
    )
    .map_err(|e| e.to_string())?;

    // Convert result to legacy format
    let speaker_results = to_single_speaker_results(&result);

    log::info!(
        "Multi-speaker optimization completed: {:.4} -> {:.4}",
        result.combined_pre_score,
        result.combined_post_score
    );

    Ok(MultiSpeakerOptimizationResult {
        speaker_results,
        combined_initial_loss: result.combined_pre_score,
        combined_final_loss: result.combined_post_score,
        optimization_history: vec![
            (0, result.combined_pre_score),
            (room_config.optimizer.max_iter, result.combined_post_score),
        ],
    })
}

/// Convert multi-speaker result to individual SpeakerOptimizationResult instances
///
/// This is useful for integrating with existing UI code that expects per-speaker results.
pub fn to_speaker_results(
    multi_result: &MultiSpeakerOptimizationResult,
) -> Vec<SpeakerOptimizationResult> {
    multi_result
        .speaker_results
        .iter()
        .map(|sr| SpeakerOptimizationResult {
            biquads: sr.biquads.clone(),
            frequencies: sr.frequencies.clone(),
            input_curve: sr.input_curve.clone(),
            target_curve: sr.target_curve.clone(),
            deviation_curve: sr.deviation_curve.clone(),
            filter_response: sr.filter_response.clone(),
            error_curve: sr.error_curve.clone(),
            corrected_curve: sr.corrected_curve.clone(),
            normalized_curve: sr.input_curve.clone(),
            individual_filter_responses: sr.individual_filter_responses.clone(),
            output_path: String::new(),
            on_axis_curve: vec![0.0; sr.frequencies.len()],
            lw_curve: vec![0.0; sr.frequencies.len()],
            er_curve: vec![0.0; sr.frequencies.len()],
            sp_curve: vec![0.0; sr.frequencies.len()],
            pir_curve: vec![0.0; sr.frequencies.len()],
            er_di_curve: vec![0.0; sr.frequencies.len()],
            sp_di_curve: vec![0.0; sr.frequencies.len()],
            optimization_history: multi_result.optimization_history.clone(),
            initial_loss: sr.initial_loss,
            final_loss: sr.final_loss,
            crossover_freqs: None,
            driver_gains: None,
            driver_delays: None,
        })
        .collect()
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Build a RoomConfig from speaker curves
///
/// Convenience function to create a RoomConfig from in-memory curves.
pub fn build_room_config_from_curves(
    speakers: &[(String, autoeq::Curve)],
    optimizer: OptimizerConfig,
) -> RoomConfig {
    let mut speakers_map: HashMap<String, SpeakerConfig> = HashMap::new();
    for (name, curve) in speakers {
        speakers_map.insert(
            name.clone(),
            SpeakerConfig::Single(MeasurementSource::InMemory(curve.clone())),
        );
    }

    RoomConfig {
        version: autoeq::roomeq::default_config_version(),
        system: None,
        speakers: speakers_map,
        crossovers: None,
        target_curve: None,
        optimizer,
        recording_config: None,
    }
}

/// Create default optimizer config from autoeq::Args
pub fn optimizer_config_from_args(args: &autoeq::Args) -> OptimizerConfig {
    OptimizerConfig {
        loss_type: format!("{:?}", args.loss).to_lowercase(),
        algorithm: args.algo.clone(),
        strategy: args.strategy.clone(),
        num_filters: args.num_filters,
        min_q: args.min_q,
        max_q: args.max_q,
        min_db: args.min_db,
        max_db: args.max_db,
        min_freq: args.min_freq,
        max_freq: args.max_freq,
        max_iter: args.maxeval,
        population: args.population,
        peq_model: format!("{:?}", args.peq_model).to_lowercase(),
        mode: "iir".to_string(),
        processing_mode: ProcessingMode::LowLatency,
        fir: None,
        seed: None,
        mixed_config: None,
        mixed_phase: None,
        refine: true,
        local_algo: "cobyla".to_string(),
        psychoacoustic: true,
        smooth_n: args.smooth_n,
        asymmetric_loss: true,
        tolerance: args.tolerance,
        atolerance: args.atolerance,
        allow_delay: None,
        target_tilt: None,
        excursion_protection: None,
        schroeder_split: None,
        phase_alignment: None,
        multi_seat: None,
        gd_opt: None,
        vog: None,
        broadband_target_matching: None,
        multi_measurement: None,
        decomposed_correction: None,
        target_response: None,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_curve(base_level: f64) -> autoeq::Curve {
        let n = 100;
        let freq: Vec<f64> = (0..n)
            .map(|i| 20.0 * (1000.0f64).powf(i as f64 / n as f64))
            .collect();
        let spl: Vec<f64> = freq
            .iter()
            .map(|f| base_level + (f / 1000.0).ln() * 2.0)
            .collect();
        autoeq::Curve {
            freq: ndarray::Array1::from_vec(freq),
            spl: ndarray::Array1::from_vec(spl),
            phase: None,
        }
    }

    #[test]
    fn test_speaker_measurement_data_creation() {
        let input = make_test_curve(80.0);
        let target = make_test_curve(85.0);
        let speaker = SpeakerMeasurementData::new("Test", input, target);

        assert_eq!(speaker.name, "Test");
        assert!((speaker.weight - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_speaker_measurement_data_with_weight() {
        let input = make_test_curve(80.0);
        let target = make_test_curve(85.0);
        let speaker = SpeakerMeasurementData::new("Test", input, target).with_weight(0.5);

        assert!((speaker.weight - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_config_default() {
        let config = MultiSpeakerOptimizationConfig::default();
        assert!(config.speakers.is_empty());
        assert!(config.callback_config.is_some());
    }

    #[test]
    fn test_empty_speakers_error() {
        let config = MultiSpeakerOptimizationConfig::default();
        let result = run_multi_speaker_optimization(&config, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No speakers"));
    }

    #[test]
    fn test_build_room_config_from_curves() {
        let curve1 = make_test_curve(80.0);
        let curve2 = make_test_curve(82.0);
        let speakers = vec![("left".to_string(), curve1), ("right".to_string(), curve2)];

        let config = build_room_config_from_curves(&speakers, OptimizerConfig::default());

        assert_eq!(config.speakers.len(), 2);
        assert!(config.speakers.contains_key("left"));
        assert!(config.speakers.contains_key("right"));
    }

    #[test]
    fn test_optimizer_config_from_args() {
        let args = autoeq::Args::speaker_defaults();
        let opt_config = optimizer_config_from_args(&args);

        assert_eq!(opt_config.num_filters, args.num_filters);
        assert!((opt_config.min_freq - args.min_freq).abs() < 0.001);
        assert!((opt_config.max_freq - args.max_freq).abs() < 0.001);
    }
}
