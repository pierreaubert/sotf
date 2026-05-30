use autoeq::roomeq::{
    CtcConfig, CtcMeasurementConfig, CtcMeasurementFileConfig, CtcRegularizationConfig,
    CtcWindowConfig, MeasurementSource, OptimizerConfig, PipelineControl, PipelineEvent,
    PipelineObserver, PipelineStepId, PipelineStepStatus, ProcessingMode, RoomConfig, RoomPipeline,
    RoomPipelineRequest, SpeakerConfig, SystemConfig, SystemModel, default_config_version,
    optimize_room, optimize_room_with_probe_arrivals,
};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

fn test_curve(base_level: f64) -> autoeq::Curve {
    let n = 80;
    let freq: Vec<f64> = (0..n)
        .map(|i| 20.0 * (1000.0f64).powf(i as f64 / n as f64))
        .collect();
    let spl: Vec<f64> = freq
        .iter()
        .map(|f| base_level + (f / 1000.0).ln() * 1.5)
        .collect();
    autoeq::Curve {
        freq: ndarray::Array1::from_vec(freq),
        spl: ndarray::Array1::from_vec(spl),
        phase: None,
        ..Default::default()
    }
}

fn stereo_config() -> RoomConfig {
    let mut speakers = HashMap::new();
    speakers.insert(
        "left".to_string(),
        SpeakerConfig::Single(MeasurementSource::InMemory(test_curve(80.0))),
    );
    speakers.insert(
        "right".to_string(),
        SpeakerConfig::Single(MeasurementSource::InMemory(test_curve(82.0))),
    );

    RoomConfig {
        version: default_config_version(),
        system: None,
        speakers,
        crossovers: None,
        target_curve: None,
        optimizer: OptimizerConfig {
            max_iter: 100,
            population: 12,
            num_filters: 1,
            processing_mode: ProcessingMode::LowLatency,
            refine: false,
            seed: Some(7),
            ..OptimizerConfig::default()
        },
        recording_config: None,
        ctc: None,
        cea2034_cache: None,
    }
}

fn workflow_stereo_config() -> RoomConfig {
    let mut config = stereo_config();
    config.system = Some(SystemConfig {
        model: SystemModel::Stereo,
        speakers: HashMap::from([
            ("L".to_string(), "left".to_string()),
            ("R".to_string(), "right".to_string()),
        ]),
        subwoofers: None,
        bass_management: None,
    });
    config
}

fn collect_events(
    config: &RoomConfig,
) -> (
    Vec<PipelineEvent>,
    autoeq::Result<autoeq::RoomOptimizationResult>,
) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let observer_events = Arc::clone(&events);
    let observer: Box<dyn PipelineObserver> = Box::new(move |event: &PipelineEvent| {
        observer_events.lock().unwrap().push(event.clone());
        PipelineControl::Continue
    });

    let result = RoomPipeline::new(RoomPipelineRequest {
        config,
        sample_rate: 48_000.0,
        output_dir: None,
        probe_arrival_overrides: None,
    })
    .run(Some(observer));

    let events = Arc::try_unwrap(events).unwrap().into_inner().unwrap();
    (events, result)
}

fn event_index(
    events: &[PipelineEvent],
    step_id: PipelineStepId,
    status: PipelineStepStatus,
) -> usize {
    events
        .iter()
        .position(|event| event.step_id == step_id && event.status == status)
        .unwrap_or_else(|| panic!("missing event {step_id:?}/{status:?}"))
}

#[test]
fn topology_workflow_emits_live_iteration_progress() {
    let config = workflow_stereo_config();
    let (events, result) = collect_events(&config);
    result.expect("workflow pipeline optimization");

    event_index(
        &events,
        PipelineStepId::TopologyWorkflowExecution,
        PipelineStepStatus::Started,
    );
    event_index(
        &events,
        PipelineStepId::TopologyWorkflowExecution,
        PipelineStepStatus::Completed,
    );

    let progress_events: Vec<&PipelineEvent> = events
        .iter()
        .filter(|event| {
            event.step_id == PipelineStepId::GenericChannelOptimization
                && event.status == PipelineStepStatus::InProgress
                && event.iteration.unwrap_or(0) > 0
        })
        .collect();

    assert!(
        !progress_events.is_empty(),
        "workflow channel optimization should emit per-iteration progress events"
    );
    assert!(
        progress_events
            .iter()
            .all(|event| event.channel.as_ref().is_some_and(|name| !name.is_empty()))
    );
    assert!(progress_events.iter().any(|event| {
        event
            .loss
            .is_some_and(|loss| loss.is_finite() && loss > 0.0)
    }));
    assert!(
        progress_events
            .iter()
            .any(|event| event.overall_progress > 0.0)
    );
}

#[test]
fn topology_workflow_iteration_progress_can_cancel_run() {
    let config = workflow_stereo_config();
    let observer: Box<dyn PipelineObserver> = Box::new(|event: &PipelineEvent| {
        if event.step_id == PipelineStepId::GenericChannelOptimization
            && event.status == PipelineStepStatus::InProgress
            && event.iteration.unwrap_or(0) > 0
        {
            PipelineControl::Stop
        } else {
            PipelineControl::Continue
        }
    });

    let result = RoomPipeline::new(RoomPipelineRequest {
        config: &config,
        sample_rate: 48_000.0,
        output_dir: None,
        probe_arrival_overrides: None,
    })
    .run(Some(observer));

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("stopped by observer")
    );
}

#[test]
fn pipeline_events_follow_stage_order() {
    let config = stereo_config();
    let (events, result) = collect_events(&config);
    result.expect("pipeline optimization");

    let config_start = event_index(
        &events,
        PipelineStepId::ConfigPreparation,
        PipelineStepStatus::Started,
    );
    let validation_start = event_index(
        &events,
        PipelineStepId::Validation,
        PipelineStepStatus::Started,
    );
    let generic_start = event_index(
        &events,
        PipelineStepId::GenericChannelOptimization,
        PipelineStepStatus::Started,
    );
    let generic_done = event_index(
        &events,
        PipelineStepId::GenericChannelOptimization,
        PipelineStepStatus::Completed,
    );
    let sanity_done = event_index(
        &events,
        PipelineStepId::SanityCheck,
        PipelineStepStatus::Completed,
    );

    assert!(config_start < validation_start);
    assert!(validation_start < generic_start);
    assert!(generic_start < generic_done);
    assert!(generic_done < sanity_done);
}

#[test]
fn pipeline_events_use_structured_step_ids_for_core_stages() {
    let config = stereo_config();
    let (events, result) = collect_events(&config);
    result.expect("pipeline optimization");

    for step_id in [
        PipelineStepId::ConfigPreparation,
        PipelineStepId::Validation,
        PipelineStepId::TopologyRouteSelection,
        PipelineStepId::GenericChannelOptimization,
        PipelineStepId::ImpulseResponseComputation,
        PipelineStepId::ChannelMatching,
        PipelineStepId::MetadataRefresh,
        PipelineStepId::SanityCheck,
    ] {
        assert!(
            events.iter().any(|event| event.step_id == step_id),
            "missing structured step id {step_id:?}"
        );
    }
}

#[test]
fn pipeline_observer_can_cancel_run() {
    let config = stereo_config();
    let observer: Box<dyn PipelineObserver> = Box::new(|event: &PipelineEvent| {
        if event.step_id == PipelineStepId::Validation
            && event.status == PipelineStepStatus::Started
        {
            PipelineControl::Stop
        } else {
            PipelineControl::Continue
        }
    });

    let result = RoomPipeline::new(RoomPipelineRequest {
        config: &config,
        sample_rate: 48_000.0,
        output_dir: None,
        probe_arrival_overrides: None,
    })
    .run(Some(observer));

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("stopped by observer")
    );
}

#[test]
fn compatibility_wrappers_return_pipeline_result_shape() {
    let config = stereo_config();
    let pipeline_result = RoomPipeline::new(RoomPipelineRequest {
        config: &config,
        sample_rate: 48_000.0,
        output_dir: None,
        probe_arrival_overrides: None,
    })
    .run(None)
    .expect("direct pipeline optimization");

    let wrapper_result =
        optimize_room(&config, 48_000.0, None, None).expect("wrapper optimization");

    assert_eq!(
        wrapper_result.channels.len(),
        pipeline_result.channels.len()
    );
    assert_eq!(
        wrapper_result.channel_results.len(),
        pipeline_result.channel_results.len()
    );
    for name in pipeline_result.channels.keys() {
        assert!(wrapper_result.channels.contains_key(name));
        assert!(wrapper_result.channel_results.contains_key(name));
    }

    let probe_arrivals = HashMap::new();
    let probe_result =
        optimize_room_with_probe_arrivals(&config, 48_000.0, None, None, &probe_arrivals)
            .expect("probe wrapper optimization");
    assert_eq!(probe_result.channels.len(), pipeline_result.channels.len());
}

#[test]
fn optimize_room_with_ctc_writes_metadata_and_artifact() {
    let dir = tempdir().unwrap();
    let left_wav = dir.path().join("ctc_left.wav");
    let right_wav = dir.path().join("ctc_right.wav");
    write_stereo_impulse(&left_wav, 30_000, 6_000);
    write_stereo_impulse(&right_wav, 6_000, 30_000);

    let mut config = stereo_config();
    config.system = Some(SystemConfig {
        model: SystemModel::Stereo,
        speakers: HashMap::from([
            ("L".to_string(), "left".to_string()),
            ("R".to_string(), "right".to_string()),
        ]),
        subwoofers: None,
        bass_management: None,
    });
    config.ctc = Some(CtcConfig {
        enabled: true,
        matrix_source: "measured".to_string(),
        measurements: Some(CtcMeasurementConfig {
            speakers: vec!["L".to_string(), "R".to_string()],
            mics: vec!["left_ear".to_string(), "right_ear".to_string()],
            head_positions: vec![],
            files: vec![
                CtcMeasurementFileConfig {
                    head_position: "primary".to_string(),
                    speaker: "L".to_string(),
                    ir: Some(left_wav),
                    raw_sweep: None,
                    loopback: None,
                },
                CtcMeasurementFileConfig {
                    head_position: "primary".to_string(),
                    speaker: "R".to_string(),
                    ir: Some(right_wav),
                    raw_sweep: None,
                    loopback: None,
                },
            ],
        }),
        hrtf: None,
        window: CtcWindowConfig::default(),
        regularization: CtcRegularizationConfig {
            beta_db: -60.0,
            beta_lf_db: -60.0,
            beta_hf_db: -60.0,
            max_gain_db: 12.0,
        },
        robustness: "average".to_string(),
        include_room_eq_dsp: true,
        fir_taps: 64,
        reference_sweep: None,
        sweep_duration_s: None,
        sweep_start_hz: None,
        sweep_end_hz: None,
        harmonic_suppression_harmonics: 5,
        harmonic_suppression_window_ms: 2.0,
        minimax_iterations: 8,
    });

    let result =
        optimize_room(&config, 48_000.0, None, Some(dir.path())).expect("room optimization");
    let ctc = result.metadata.ctc.expect("ctc metadata");
    assert_eq!(ctc.source, "measured");
    assert_eq!(ctc.speakers, vec!["L", "R"]);
    assert_eq!(ctc.head_positions, 1);
    assert!(ctc.room_eq_correction_channels.is_empty());
    assert!(ctc.delivered_response.is_some());
    assert!(Path::new(&ctc.artifact).exists());

    let artifact: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ctc.artifact).unwrap()).unwrap();
    assert_eq!(artifact["version"], "ctc-recommended-v1");
    assert_eq!(artifact["filters"].as_array().unwrap().len(), 4);
    assert!(artifact["room_eq_correction_applied"].is_boolean());
    assert!(artifact["delivered_response"]["mean_crosstalk_db"].is_number());
}

fn write_stereo_impulse(path: &Path, left: i16, right: i16) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    writer.write_sample::<i16>(left).unwrap();
    writer.write_sample::<i16>(right).unwrap();
    for _ in 1..64 {
        writer.write_sample::<i16>(0).unwrap();
        writer.write_sample::<i16>(0).unwrap();
    }
    writer.finalize().unwrap();
}
