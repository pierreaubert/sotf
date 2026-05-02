use autoeq::roomeq::{
    MeasurementSource, OptimizerConfig, PipelineControl, PipelineEvent, PipelineObserver,
    PipelineStepId, PipelineStepStatus, ProcessingMode, RoomConfig, RoomPipeline,
    RoomPipelineRequest, SpeakerConfig, default_config_version, optimize_room,
    optimize_room_with_probe_arrivals,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
        cea2034_cache: None,
    }
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
