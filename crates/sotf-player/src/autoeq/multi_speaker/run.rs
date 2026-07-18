use super::misc::room_callback_observer;
use super::multi_speaker_optimization_config::MultiSpeakerOptimizationConfig;
use super::to::to_single_speaker_results;
use super::types::MultiSpeakerOptimizationCallback;
use super::types::MultiSpeakerOptimizationResult;
use super::types::multi_speaker_progress_from_pipeline_event;
pub use autoeq::roomeq::{
    MeasurementSource, OptimizerConfig, ProcessingMode, RoomConfig, SpeakerConfig,
};
pub use autoeq::roomeq::{
    PipelineControl, PipelineEvent, PipelineObserver, RoomOptimizationCallback,
    RoomOptimizationResult, RoomPipeline, RoomPipelineRequest,
};
use std::collections::HashMap;
use std::path::Path;

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
    run_room_optimization_with_output_dir(config, sample_rate, callback, None)
}

/// Run room optimization and write generated FIR/convolution artifacts into
/// `output_dir` when provided.
pub fn run_room_optimization_with_output_dir(
    config: &RoomConfig,
    sample_rate: f64,
    callback: Option<RoomOptimizationCallback>,
    output_dir: Option<&Path>,
) -> Result<RoomOptimizationResult, String> {
    RoomPipeline::new(RoomPipelineRequest {
        config,
        sample_rate,
        output_dir,
        probe_arrival_overrides: None,
    })
    .run(callback.map(room_callback_observer))
    .map_err(|e| e.to_string())
}

/// Run room optimization with per-channel probe-based arrival times.
///
/// Same as [`run_room_optimization`] but lets the delay-detection UI step
/// pass in measured arrival times (keyed by channel name). Channels present
/// in the map use the probe value directly; channels absent from the map
/// fall back to WAV-onset detection inside the optimizer.
///
/// The map uses raw channel names (the same keys as `config.speakers`) and
/// arrival times in milliseconds. Time alignment (delay = max_arrival -
/// channel_arrival) is computed downstream by the autoeq speaker_eq path.
pub fn run_room_optimization_with_probe_arrivals(
    config: &RoomConfig,
    sample_rate: f64,
    callback: Option<RoomOptimizationCallback>,
    probe_arrival_ms: &HashMap<String, f64>,
) -> Result<RoomOptimizationResult, String> {
    run_room_optimization_with_probe_arrivals_and_output_dir(
        config,
        sample_rate,
        callback,
        None,
        probe_arrival_ms,
    )
}

/// Run room optimization with probe arrivals and an optional artifact output
/// directory for generated FIR/convolution WAV files.
pub fn run_room_optimization_with_probe_arrivals_and_output_dir(
    config: &RoomConfig,
    sample_rate: f64,
    callback: Option<RoomOptimizationCallback>,
    output_dir: Option<&Path>,
    probe_arrival_ms: &HashMap<String, f64>,
) -> Result<RoomOptimizationResult, String> {
    RoomPipeline::new(RoomPipelineRequest {
        config,
        sample_rate,
        output_dir,
        probe_arrival_overrides: Some(probe_arrival_ms),
    })
    .run(callback.map(room_callback_observer))
    .map_err(|e| e.to_string())
}

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
    let is_bo_algorithm = config.args.algo.eq_ignore_ascii_case("autoeq:bo")
        || config.args.algo.eq_ignore_ascii_case("bo");

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
            processing_mode: ProcessingMode::LowLatency,
            fir: None,
            seed: None,
            mixed_config: None,
            refine: true,
            local_algo: "cobyla".to_string(),
            bo_initial_samples: (is_bo_algorithm && config.args.bo_initial_samples > 0)
                .then_some(config.args.bo_initial_samples),
            bo_batch_size: (is_bo_algorithm && config.args.bo_batch_size > 0)
                .then_some(config.args.bo_batch_size),
            bo_posterior_std_threshold: (is_bo_algorithm
                && config.args.bo_posterior_std_threshold > 0.0)
                .then_some(config.args.bo_posterior_std_threshold),
            bo_acquisition: (is_bo_algorithm && !config.args.bo_acquisition.is_empty())
                .then(|| config.args.bo_acquisition.clone()),
            bo_ehvi: (is_bo_algorithm && config.args.bo_ehvi).then_some(true),
            psychoacoustic: true,
            psychoacoustic_smoothing: None,
            smooth_n: config.args.smooth_n,
            asymmetric_loss: true,
            asymmetric_loss_config: None,
            perceptual_policy: None,
            audibility_deadband: None,
            high_frequency_correction: None,
            early_late_correction: None,
            validation_bundle: None,
            tolerance: config.args.tolerance,
            atolerance: config.args.atolerance,
            allow_delay: None,
            excursion_protection: None,
            schroeder_split: None,
            phase_alignment: None,
            multi_seat: None,
            inter_channel_timbre_matching: None,
            height_channel_alignment: None,
            removed_vog_alias: None,
            multi_measurement: None,
            mixed_phase: None,
            phase_correction: None,
            decomposed_correction: None,
            target_response: None,
            cea2034_correction: None,
            min_filter_improvement: 0.0,
            elimination_threshold: 0.0,
            sub_config: None,
            channel_matching: None,
            ssir_wav_path: None,
            max_boost_envelope: None,
            min_cut_envelope: None,
            epa_config: None,
            group_delay: None,
            auto_optimizer: None,
            smoothness_penalty: None,
            from_measurement_slope_override: None,
        },
        recording_config: None,
        ctc: None,
        cea2034_cache: None,
    };

    let observer: Option<Box<dyn PipelineObserver>> = callback.map(|mut cb| {
        let observer: Box<dyn PipelineObserver> = Box::new(move |event: &PipelineEvent| {
            let legacy_progress = multi_speaker_progress_from_pipeline_event(event);
            match cb(&legacy_progress) {
                autoeq::de::CallbackAction::Continue => PipelineControl::Continue,
                autoeq::de::CallbackAction::Stop => PipelineControl::Stop,
            }
        });
        observer
    });

    // Run optimization using roomeq
    let result = RoomPipeline::new(RoomPipelineRequest {
        config: &room_config,
        sample_rate: config.args.sample_rate,
        output_dir: None,
        probe_arrival_overrides: None,
    })
    .run(observer)
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
