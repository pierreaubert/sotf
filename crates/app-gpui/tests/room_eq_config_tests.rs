//! Room EQ configuration and serialization tests.

use sotf_audio_player::{
    EQFilter, PluginGraph, PluginSettings, PluginType,
    recording_types::{CtcMatrixExportStrategy, DelayProbeChannelResult, DelayProbeResults},
    room_eq_types::{DelayDetectionStatus, parse_eq_filters_from_json},
};
use sotf_audio_player_gpui::{
    ChannelMeasurement, ChannelRecording, ChannelRecordingState, RecordingResult, RecordingState,
    RoomEqOptimizerConfig, RoomEqState, RoomEqStep,
};
use std::collections::HashMap;
use std::path::PathBuf;

#[test]
fn test_room_eq_state_defaults() {
    let state = RoomEqState::default();
    assert_eq!(state.step, RoomEqStep::LoadData);
    assert_eq!(state.optimizer_config.num_filters, 7);
    assert!(state.channel_measurements.is_empty());
}

#[test]
fn test_room_eq_to_room_config_simple() {
    let mut state = RoomEqState::default();

    // Add a dummy measurement
    state.channel_measurements.push(ChannelMeasurement {
        channel_name: "L".to_string(),
        measurement: RecordingResult {
            channel: 0,
            frequencies: vec![100.0, 1000.0],
            magnitude_db: vec![70.0, 75.0],
            phase_deg: Vec::new(),
            wav_path: None,
            csv_path: None,
            impulse_response: None,
            impulse_time_ms: None,
            thd_percent: None,
            harmonic_distortion_db: None,
            excess_group_delay_ms: None,
            rt60_ms: None,
            clarity_c50_db: None,
            clarity_c80_db: None,
            spectrogram_db: None,
        },
        is_group: false,
        group_drivers: Vec::new(),
        multi_mic_measurements: Vec::new(),
    });

    state.init_speaker_configs();

    let config = state.to_room_config();

    assert_eq!(config.speakers.len(), 1);
    assert!(config.speakers.contains_key("L"));

    // Check optimizer config
    assert_eq!(config.optimizer.num_filters, 7);
    assert_eq!(
        config.optimizer.processing_mode,
        autoeq::roomeq::ProcessingMode::LowLatency
    );
}

#[test]
fn test_room_eq_to_room_config_preserves_raw_sweep_ctc_config() {
    let mut state = RoomEqState {
        channel_measurements: vec![
            make_dummy_measurement("L"),
            make_dummy_measurement("R"),
            make_dummy_measurement("LFE [mic 1]"),
        ],
        ctc_config: Some(autoeq::roomeq::CtcConfig {
            enabled: true,
            matrix_source: "raw_sweep".to_string(),
            reference_sweep: Some(PathBuf::from("ctc_reference_sweep.wav")),
            measurements: Some(autoeq::roomeq::CtcMeasurementConfig {
                speakers: vec!["L".to_string(), "R".to_string(), "LFE [mic 1]".to_string()],
                mics: vec!["left_ear".to_string(), "right_ear".to_string()],
                head_positions: Vec::new(),
                files: Vec::new(),
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    state.init_speaker_configs();

    let config = state.to_room_config();
    let ctc = config.ctc.expect("ctc config");
    assert!(!ctc.enabled, "app-gpui must not enable CTC yet");
    assert_eq!(ctc.matrix_source, "raw_sweep");
    assert_eq!(
        ctc.reference_sweep,
        Some(PathBuf::from("ctc_reference_sweep.wav"))
    );
    let system = config
        .system
        .expect("LFE requires system config for bass management");
    assert_eq!(system.speakers.get("L").map(String::as_str), Some("L"));
    assert_eq!(system.speakers.get("R").map(String::as_str), Some("R"));
    assert_eq!(
        system.speakers.get("LFE [mic 1]").map(String::as_str),
        Some("LFE [mic 1]")
    );
    let subwoofers = system
        .subwoofers
        .expect("CTC home-cinema config with LFE must include subwoofer config");
    assert_eq!(subwoofers.crossover.as_deref(), Some("bass_management"));
    let crossover = config
        .crossovers
        .as_ref()
        .and_then(|crossovers| crossovers.get("bass_management"))
        .expect("bass-management crossover config");
    assert_eq!(crossover.crossover_type, "LR24");
    assert_eq!(crossover.frequency, Some(80.0));
}

#[test]
fn test_room_eq_to_room_config_disables_imported_ctc_config() {
    let mut state = RoomEqState {
        channel_measurements: vec![make_dummy_measurement("L"), make_dummy_measurement("R")],
        ctc_config: Some(autoeq::roomeq::CtcConfig {
            enabled: true,
            matrix_source: "measured".to_string(),
            measurements: Some(autoeq::roomeq::CtcMeasurementConfig {
                speakers: vec!["L".to_string(), "R".to_string()],
                mics: vec!["left_ear".to_string(), "right_ear".to_string()],
                head_positions: Vec::new(),
                files: Vec::new(),
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    state.init_speaker_configs();

    let config = state.to_room_config();
    let ctc = config.ctc.expect("ctc config metadata is preserved");
    assert!(!ctc.enabled, "imported CTC must be clamped off");
    assert_eq!(ctc.matrix_source, "measured");
    assert!(
        config.system.is_none(),
        "disabled CTC alone must not require a system config"
    );
}

#[test]
fn test_room_eq_to_room_config_emits_bass_management_without_ctc() {
    let mut state = RoomEqState {
        channel_measurements: vec![
            make_dummy_measurement("L"),
            make_dummy_measurement("R"),
            make_dummy_measurement("LFE"),
        ],
        ..Default::default()
    };
    state.init_speaker_configs();

    let config = state.to_room_config();
    let system = config
        .system
        .expect("LFE/sub output requires a system config for bass management");
    assert_eq!(system.speakers.get("L").map(String::as_str), Some("L"));
    assert_eq!(system.speakers.get("R").map(String::as_str), Some("R"));
    assert_eq!(system.speakers.get("LFE").map(String::as_str), Some("LFE"));
    let subwoofers = system
        .subwoofers
        .expect("LFE/sub output requires subwoofer config");
    assert_eq!(subwoofers.crossover.as_deref(), Some("bass_management"));
    assert!(
        config
            .crossovers
            .as_ref()
            .is_some_and(|crossovers| crossovers.contains_key("bass_management")),
        "generated bass-management crossover must be present"
    );
}

#[test]
fn test_room_eq_to_room_config_preserves_imported_system_and_crossovers() {
    let mut state = RoomEqState {
        channel_measurements: vec![
            make_dummy_measurement("L"),
            make_dummy_measurement("R"),
            make_dummy_measurement("LFE"),
        ],
        ..Default::default()
    };
    state.init_speaker_configs();

    let mut speakers = HashMap::new();
    speakers.insert("L".to_string(), "L".to_string());
    speakers.insert("R".to_string(), "R".to_string());
    speakers.insert("LFE".to_string(), "LFE".to_string());
    state.imported_system = Some(autoeq::roomeq::SystemConfig {
        model: autoeq::roomeq::SystemModel::HomeCinema,
        speakers,
        subwoofers: Some(autoeq::roomeq::SubwooferSystemConfig {
            config: autoeq::roomeq::SubwooferStrategy::Single,
            crossover: Some("cli_xover".to_string()),
            mapping: HashMap::new(),
        }),
        bass_management: Some(autoeq::roomeq::BassManagementConfig {
            max_sub_boost_db: 3.0,
            ..Default::default()
        }),
    });
    let mut crossovers = HashMap::new();
    crossovers.insert(
        "cli_xover".to_string(),
        autoeq::roomeq::CrossoverConfig {
            crossover_type: "LR48".to_string(),
            frequency: Some(55.0),
            frequencies: None,
            frequency_range: None,
        },
    );
    state.imported_crossovers = Some(crossovers);

    let config = state.to_room_config();
    let system = config.system.expect("imported system must be preserved");
    assert_eq!(
        system
            .subwoofers
            .as_ref()
            .and_then(|subs| subs.crossover.as_deref()),
        Some("cli_xover")
    );
    assert_eq!(
        system
            .bass_management
            .as_ref()
            .map(|bm| bm.max_sub_boost_db),
        Some(3.0)
    );
    let crossover = config
        .crossovers
        .as_ref()
        .and_then(|crossovers| crossovers.get("cli_xover"))
        .expect("imported crossover must be preserved");
    assert_eq!(crossover.crossover_type, "LR48");
    assert_eq!(crossover.frequency, Some(55.0));
}

#[test]
fn test_load_from_recording_marks_ctc_fallback_as_measured() {
    fn ctc_ir_recording(speaker_idx: usize, mic_idx: usize) -> ChannelRecording {
        let mut rec = ChannelRecording::with_mic_position(
            speaker_idx,
            format!(
                "{} (Mic {})",
                if speaker_idx == 0 { "L" } else { "R" },
                mic_idx + 1
            ),
            mic_idx,
            0,
        );
        rec.state = ChannelRecordingState::Done;
        rec.result = Some(RecordingResult {
            channel: speaker_idx,
            frequencies: vec![100.0],
            magnitude_db: vec![0.0],
            phase_deg: vec![0.0],
            wav_path: None,
            csv_path: None,
            impulse_response: Some(vec![1.0, 0.5]),
            impulse_time_ms: None,
            thd_percent: None,
            harmonic_distortion_db: None,
            excess_group_delay_ms: None,
            rt60_ms: None,
            clarity_c50_db: None,
            clarity_c80_db: None,
            spectrogram_db: None,
        });
        rec
    }

    let dir = tempfile::tempdir().unwrap();
    let mut recording = RecordingState {
        recording_directory: Some(dir.path().to_string_lossy().to_string()),
        ctc_reference_sweep_path: Some(
            dir.path()
                .join("ctc_reference_sweep.wav")
                .to_string_lossy()
                .to_string(),
        ),
        channel_recordings: vec![
            ctc_ir_recording(0, 0),
            ctc_ir_recording(0, 1),
            ctc_ir_recording(1, 0),
            ctc_ir_recording(1, 1),
        ],
        ..Default::default()
    };
    recording.recording_config.channel_mappings = vec![0, 1];
    recording.recording_config.ctc_matrix_strategy = CtcMatrixExportStrategy::RawSweep;

    let mut room_eq = RoomEqState::default();
    room_eq.load_from_recording(&recording);

    let ctc = room_eq.ctc_config.expect("ctc config");
    assert_eq!(ctc.matrix_source, "measured");
    assert!(ctc.reference_sweep.is_none());
    assert!(
        ctc.measurements
            .as_ref()
            .unwrap()
            .files
            .iter()
            .all(|file| file.ir.is_some() && file.raw_sweep.is_none())
    );
}

#[test]
fn test_load_from_recording_applies_probe_delay_results() {
    let mut recording = RecordingState::default();
    recording.playback_config.device_name = "Output Device".to_string();
    recording.recording_config.device_name = "Input Device".to_string();
    recording.probe_capture.sample_rate = 96_000;
    recording.probe_capture.input_channel = 1;
    recording.probe_capture.results = Some(DelayProbeResults {
        sample_rate: 96_000,
        channels: vec![
            DelayProbeChannelResult {
                channel_name: "L".to_string(),
                channel_index: 0,
                arrival_ms: 2.5,
                gain_db: -12.0,
                snr_db: 42.0,
            },
            DelayProbeChannelResult {
                channel_name: "R".to_string(),
                channel_index: 1,
                arrival_ms: 4.0,
                gain_db: -13.0,
                snr_db: 41.0,
            },
        ],
        alignment_delays_ms: vec![1.5, 0.0],
    });

    let mut room_eq = RoomEqState::default();
    room_eq.load_from_recording(&recording);

    assert_eq!(
        room_eq.delay_detection.status,
        DelayDetectionStatus::Complete
    );
    assert_eq!(room_eq.delay_detection.sample_rate, 96_000);
    assert_eq!(room_eq.delay_detection.input_channel, 1);
    assert_eq!(
        room_eq.delay_detection.output_device_name.as_deref(),
        Some("Output Device")
    );
    assert_eq!(
        room_eq.delay_detection.input_device_name.as_deref(),
        Some("Input Device")
    );
    let arrivals = room_eq
        .delay_detection
        .probe_arrival_map()
        .expect("probe arrivals");
    assert_eq!(arrivals.get("L").copied(), Some(2.5));
    assert_eq!(arrivals.get("R").copied(), Some(4.0));
}

#[test]
fn test_room_eq_to_room_config_advanced() {
    let mut state = RoomEqState::default();

    // Set advanced parameters
    state.optimizer_config.target_response.enabled = true;
    state.optimizer_config.target_response.shape = "harman".to_string();
    state.optimizer_config.target_response.slope_db_per_octave = -1.0;

    state.optimizer_config.excursion_protection.enabled = true;
    state.optimizer_config.excursion_protection.manual_f3_hz = 45.0;

    state.optimizer_config.schroeder_split.enabled = true;
    state.optimizer_config.schroeder_split.schroeder_freq = 250.0;

    state.optimizer_config.phase_alignment.enabled = true;
    state.optimizer_config.phase_alignment.max_delay_ms = 20.0;

    state.optimizer_config.multi_seat.enabled = true;
    state.optimizer_config.multi_seat.strategy = "primary".to_string();

    // Add measurement
    state.channel_measurements.push(ChannelMeasurement {
        channel_name: "L".to_string(),
        measurement: RecordingResult {
            channel: 0,
            frequencies: vec![100.0],
            magnitude_db: vec![70.0],
            phase_deg: Vec::new(),
            wav_path: None,
            csv_path: None,
            impulse_response: None,
            impulse_time_ms: None,
            thd_percent: None,
            harmonic_distortion_db: None,
            excess_group_delay_ms: None,
            rt60_ms: None,
            clarity_c50_db: None,
            clarity_c80_db: None,
            spectrogram_db: None,
        },
        is_group: false,
        group_drivers: Vec::new(),
        multi_mic_measurements: Vec::new(),
    });
    state.init_speaker_configs();

    let config = state.to_room_config();

    // Verify advanced sections are present
    assert!(config.optimizer.target_response.is_some());
    let tilt = config.optimizer.target_response.as_ref().unwrap();
    assert_eq!(tilt.slope_db_per_octave, -1.0);

    assert!(config.optimizer.excursion_protection.is_some());
    let excursion = config.optimizer.excursion_protection.as_ref().unwrap();
    assert_eq!(excursion.manual_f3_hz, Some(45.0));

    assert!(config.optimizer.schroeder_split.is_some());
    let schroeder = config.optimizer.schroeder_split.as_ref().unwrap();
    assert_eq!(schroeder.schroeder_freq, 250.0);

    assert!(config.optimizer.phase_alignment.is_some());
    let phase = config.optimizer.phase_alignment.as_ref().unwrap();
    assert_eq!(phase.max_delay_ms, 20.0);

    assert!(config.optimizer.multi_seat.is_some());
    let multi_seat = config.optimizer.multi_seat.as_ref().unwrap();
    assert_eq!(
        multi_seat.strategy,
        autoeq::roomeq::MultiSeatStrategy::PrimaryWithConstraints
    );
}

#[test]
fn test_room_eq_to_room_config_modal_basis_multiseat() {
    let mut state = RoomEqState::default();
    state.optimizer_config.multi_seat.enabled = true;
    state.optimizer_config.multi_seat.strategy = "modal_basis".to_string();

    let config = state.to_room_config();
    let multi_seat = config
        .optimizer
        .multi_seat
        .as_ref()
        .expect("multi-seat config should be present");

    assert_eq!(
        multi_seat.strategy,
        autoeq::roomeq::MultiSeatStrategy::ModalBasis
    );
}

#[test]
fn test_room_eq_validation() {
    let mut state = RoomEqState::default();

    // Invalid config: min_freq > max_freq
    state.optimizer_config.min_freq = 1000.0;
    state.optimizer_config.max_freq = 500.0;

    // Add measurement to make it a valid RoomConfig otherwise
    state.channel_measurements.push(ChannelMeasurement {
        channel_name: "L".to_string(),
        measurement: RecordingResult {
            channel: 0,
            frequencies: vec![100.0],
            magnitude_db: vec![70.0],
            phase_deg: Vec::new(),
            wav_path: None,
            csv_path: None,
            impulse_response: None,
            impulse_time_ms: None,
            thd_percent: None,
            harmonic_distortion_db: None,
            excess_group_delay_ms: None,
            rt60_ms: None,
            clarity_c50_db: None,
            clarity_c80_db: None,
            spectrogram_db: None,
        },
        is_group: false,
        group_drivers: Vec::new(),
        multi_mic_measurements: Vec::new(),
    });
    state.init_speaker_configs();

    let validation = state.validate();
    assert!(!validation.is_valid);
    assert!(validation.errors.iter().any(|e| e.contains("min_freq")));
}

#[test]
fn test_calculate_normalization_offset() {
    let frequencies = vec![100.0, 1000.0, 1500.0, 2000.0, 5000.0];
    let spl = vec![70.0, 80.0, 82.0, 84.0, 75.0];

    // Mean of 80, 82, 84 is 82.0
    let offset = RoomEqState::calculate_normalization_offset(&frequencies, &spl);
    assert!((offset - 82.0).abs() < 0.001);
}

#[test]
fn test_calculate_normalization_offset_fallback() {
    let frequencies = vec![100.0, 200.0];
    let spl = vec![70.0, 72.0];

    // No points in 1k-2k range, should use overall mean (71.0)
    let offset = RoomEqState::calculate_normalization_offset(&frequencies, &spl);
    assert!((offset - 71.0).abs() < 0.001);
}

#[test]
fn test_normalization_alignment() {
    let frequencies = vec![100.0, 1000.0, 1500.0, 2000.0, 5000.0];
    let spl = vec![70.0, 80.0, 82.0, 84.0, 75.0];

    let offset = RoomEqState::calculate_normalization_offset(&frequencies, &spl);
    let pts: Vec<(f64, f64)> = frequencies
        .iter()
        .zip(spl.iter())
        .map(|(&f, &db)| (f, db))
        .collect();
    let normalized = RoomEqState::normalize_points(&pts, offset);

    // Check normalized values in 1k-2k range
    let n_1000 = normalized[1].1;
    let n_1500 = normalized[2].1;
    let n_2000 = normalized[3].1;

    let mean_normalized = (n_1000 + n_1500 + n_2000) / 3.0;
    assert!(mean_normalized.abs() < 0.001);
}

/// Helper: build a minimal BackendOptimizerConfig with NO feature flags enabled.
/// This simulates what the roomeq CLI produces when none of the advanced features
/// are specified in the JSON config.
fn make_bare_backend_config() -> autoeq::roomeq::OptimizerConfig {
    autoeq::roomeq::OptimizerConfig {
        processing_mode: autoeq::roomeq::ProcessingMode::LowLatency,
        fir: None,
        mixed_config: None,
        mixed_phase: None,
        loss_type: "flat".to_string(),
        algorithm: "autoeq:de".to_string(),
        num_filters: 7,
        min_q: 0.5,
        max_q: 10.0,
        min_db: -12.0,
        max_db: 12.0,
        min_freq: 20.0,
        max_freq: 12000.0,
        max_iter: 50000,
        population: 50,
        peq_model: "pk".to_string(),
        seed: None,
        refine: true,
        local_algo: "cobyla".to_string(),
        bo_initial_samples: None,
        bo_batch_size: None,
        bo_posterior_std_threshold: None,
        bo_acquisition: None,
        bo_ehvi: None,
        psychoacoustic: true,
        psychoacoustic_smoothing: None,
        smooth_n: 2,
        asymmetric_loss: true,
        asymmetric_loss_config: None,
        perceptual_policy: None,
        audibility_deadband: None,
        high_frequency_correction: None,
        early_late_correction: None,
        validation_bundle: None,
        tolerance: 1e-5,
        atolerance: 1e-5,
        allow_delay: None,
        // All feature flags None = disabled
        excursion_protection: None,
        schroeder_split: None,
        phase_alignment: None,
        multi_seat: None,
        vog: None,
        multi_measurement: None,
        decomposed_correction: None,
        cea2034_correction: None,
        sub_config: None,
        channel_matching: None,
        strategy: "lshade".to_string(),
        target_response: None,
        phase_correction: None,
        min_filter_improvement: 0.01,
        elimination_threshold: 0.005,
        auto_optimizer: None,
        smoothness_penalty: None,
        ssir_wav_path: None,
        max_boost_envelope: None,
        min_cut_envelope: None,
        epa_config: None,
        group_delay: None,
        from_measurement_slope_override: None,
    }
}

fn make_dummy_measurement(channel: &str) -> ChannelMeasurement {
    ChannelMeasurement {
        channel_name: channel.to_string(),
        measurement: RecordingResult {
            channel: 0,
            frequencies: vec![100.0, 1000.0, 5000.0],
            magnitude_db: vec![70.0, 75.0, 72.0],
            phase_deg: Vec::new(),
            wav_path: None,
            csv_path: None,
            impulse_response: None,
            impulse_time_ms: None,
            thd_percent: None,
            harmonic_distortion_db: None,
            excess_group_delay_ms: None,
            rt60_ms: None,
            clarity_c50_db: None,
            clarity_c80_db: None,
            spectrogram_db: None,
        },
        is_group: false,
        group_drivers: Vec::new(),
        multi_mic_measurements: Vec::new(),
    }
}

/// Regression test: When importing from a backend config that has NO advanced features,
/// apply_smart_defaults must NOT force-enable them. This was the root cause of
/// GPUI room EQ producing wrong optimization results.
#[test]
fn test_import_from_backend_preserves_disabled_features() {
    let backend = make_bare_backend_config();
    let mut config = RoomEqOptimizerConfig::default();

    config.import_from_backend(&backend);

    // imported_from_file must be set
    assert!(config.imported_from_file);

    // All features that were None in backend must be disabled
    assert!(!config.target_response.enabled);
    assert!(!config.excursion_protection.enabled);
    assert!(!config.schroeder_split.enabled);
    assert!(!config.target_response.broadband_precorrection);
    assert!(!config.vog.enabled);
    assert!(!config.phase_alignment.enabled);
    assert!(!config.multi_seat.enabled);
    assert!(!config.multi_measurement.enabled);
    assert!(!config.allow_delay);

    // Core params must match
    assert_eq!(config.num_filters, 7);
    assert_eq!(config.max_q, 10.0);
    assert_eq!(config.max_db, 12.0);
    assert_eq!(config.max_freq, 12000.0);
}

#[test]
fn test_bo_options_roundtrip_to_backend() {
    let mut config = RoomEqOptimizerConfig {
        algorithm: "autoeq:bo".to_string(),
        bo_initial_samples: 32,
        bo_batch_size: 4,
        bo_posterior_std_threshold: 0.015,
        bo_acquisition: "thompson".to_string(),
        bo_ehvi: true,
        ..Default::default()
    };

    let backend = config.to_optimizer_config();
    assert_eq!(backend.algorithm, "autoeq:bo");
    assert_eq!(backend.bo_initial_samples, Some(32));
    assert_eq!(backend.bo_batch_size, Some(4));
    assert_eq!(backend.bo_posterior_std_threshold, Some(0.015));
    assert_eq!(backend.bo_acquisition.as_deref(), Some("thompson"));
    assert_eq!(backend.bo_ehvi, Some(true));

    config = RoomEqOptimizerConfig::default();
    config.import_from_backend(&backend);
    assert_eq!(config.algorithm, "autoeq:bo");
    assert_eq!(config.bo_initial_samples, 32);
    assert_eq!(config.bo_batch_size, 4);
    assert_eq!(config.bo_posterior_std_threshold, 0.015);
    assert_eq!(config.bo_acquisition, "thompson");
    assert!(config.bo_ehvi);
}

/// Regression test: After import + apply_smart_defaults, the resulting RoomConfig
/// must NOT have features that the backend config didn't have.
#[test]
fn test_import_then_smart_defaults_matches_backend() {
    let backend = make_bare_backend_config();
    let mut state = RoomEqState::default();

    state.channel_measurements.push(make_dummy_measurement("L"));
    state.channel_measurements.push(make_dummy_measurement("R"));
    state.init_speaker_configs();

    state.optimizer_config.import_from_backend(&backend);
    state.apply_smart_defaults(None);

    let room_config = state.to_room_config();
    let opt = &room_config.optimizer;

    // Features that were None in backend must remain None after smart defaults
    assert!(
        opt.target_response.is_none(),
        "target_response should be None"
    );
    assert!(
        opt.excursion_protection.is_none(),
        "excursion_protection should be None"
    );
    assert!(
        opt.schroeder_split.is_none(),
        "schroeder_split should be None"
    );

    // Core params must match
    assert_eq!(opt.num_filters, 7);
    assert_eq!(opt.max_q, 10.0);
    assert_eq!(opt.max_db, 12.0);
    assert_eq!(opt.max_freq, 12000.0);
    assert!(opt.refine);
    assert_eq!(opt.local_algo, "cobyla");
}

/// Test that fresh measurements (no import) still get smart defaults.
#[test]
fn test_fresh_measurement_gets_smart_defaults() {
    let mut state = RoomEqState::default();

    state.channel_measurements.push(make_dummy_measurement("L"));
    state.channel_measurements.push(make_dummy_measurement("R"));
    state.init_speaker_configs();

    // No import_from_backend call — this is a fresh recording
    assert!(!state.optimizer_config.imported_from_file);

    state.apply_smart_defaults(None);

    let room_config = state.to_room_config();
    let opt = &room_config.optimizer;

    // Smart defaults should enable features for fresh measurements
    assert!(
        opt.target_response.is_some(),
        "target_response should be enabled"
    );
    assert!(
        opt.excursion_protection.is_some(),
        "excursion_protection should be enabled"
    );
    // Schroeder split is only enabled when a subwoofer is present.
    // This test has L+R only (no sub), so it should be None.
    assert!(
        opt.schroeder_split.is_none(),
        "schroeder_split should be disabled for stereo without subwoofer"
    );
    assert!(
        opt.target_response
            .as_ref()
            .is_some_and(|tr| tr.broadband_precorrection),
        "broadband pre-correction should be enabled"
    );
}

/// Test that importing a backend WITH features enabled preserves them through smart defaults.
#[test]
fn test_import_with_features_enabled_preserves_them() {
    let mut backend = make_bare_backend_config();
    backend.target_response = Some(autoeq::roomeq::TargetResponseConfig {
        shape: autoeq::roomeq::TargetShape::Harman,
        slope_db_per_octave: -1.0,
        reference_freq: 1000.0,
        curve_path: None,
        preference: autoeq::roomeq::UserPreference {
            bass_shelf_db: 2.0,
            bass_shelf_freq: 200.0,
            treble_shelf_db: 0.0,
            treble_shelf_freq: 8000.0,
        },
        broadband_precorrection: false,
        role_targets: None,
    });
    backend.schroeder_split = Some(autoeq::roomeq::SchroederSplitConfig {
        enabled: true,
        schroeder_freq: 250.0,
        room_dimensions: None,
        low_freq_config: autoeq::roomeq::LowFreqFilterConfig {
            max_q: 8.0,
            min_q: 0.5,
            allow_boost: true,
            max_db: None,
        },
        high_freq_config: autoeq::roomeq::HighFreqFilterConfig {
            max_q: 2.0,
            shelving_only: true,
        },
    });

    let mut state = RoomEqState::default();
    state.channel_measurements.push(make_dummy_measurement("L"));
    state.init_speaker_configs();

    state.optimizer_config.import_from_backend(&backend);
    state.apply_smart_defaults(None);

    let room_config = state.to_room_config();
    let opt = &room_config.optimizer;

    // target_response was enabled in backend
    let tilt = opt
        .target_response
        .as_ref()
        .expect("target_response should be Some");
    assert_eq!(tilt.slope_db_per_octave, -1.0);
    assert_eq!(tilt.preference.bass_shelf_db, 2.0);

    // schroeder_split was enabled in backend
    let ss = opt
        .schroeder_split
        .as_ref()
        .expect("schroeder_split should be Some");
    assert_eq!(ss.schroeder_freq, 250.0);
    assert_eq!(ss.low_freq_config.max_q, 8.0);
    assert!(ss.low_freq_config.allow_boost);
    assert_eq!(ss.high_freq_config.max_q, 2.0);

    // excursion_protection was NOT in backend
    assert!(opt.excursion_protection.is_none());
}

/// Regression: broadband pre-correction now lives inside `target_response`.
/// When backend has `target_response.broadband_precorrection = false`,
/// the GPUI must treat it as disabled.
#[test]
fn test_import_broadband_with_enabled_false() {
    let mut backend = make_bare_backend_config();
    backend.target_response = Some(autoeq::roomeq::TargetResponseConfig {
        shape: autoeq::roomeq::TargetShape::Flat,
        slope_db_per_octave: 0.0,
        reference_freq: 1000.0,
        curve_path: None,
        preference: autoeq::roomeq::UserPreference::default(),
        broadband_precorrection: false,
        role_targets: None,
    });

    let mut config = RoomEqOptimizerConfig::default();
    config.import_from_backend(&backend);

    assert!(!config.target_response.broadband_precorrection);
}

// ============================================================================
// Room EQ channel ordering tests
//
// When applying room EQ to the player, per-channel filters must be indexed
// by audio channel order (0=FL, 1=FR, 2=C, 3=LFE, etc.), NOT alphabetically.
// These tests verify the ordering for all standard surround configurations.
// ============================================================================

use sotf_audio_player::room_eq_types::{DriverDspChain, DspChainOutputExt};
use sotf_audio_player_gpui::{ChannelDspChain, ChannelOptResult, DspChainOutput, DspPluginConfig};

/// Build a `ChannelDspChain` with no optional curves / impulse responses
/// populated. These tests only exercise channel ordering, broadband /
/// main-EQ separation, and rack compatibility — none of which look at
/// the curve/IR fields — so spelling them out at every call site is
/// pure noise.
fn chain(
    name: &str,
    plugins: Vec<DspPluginConfig>,
    drivers: Option<Vec<DriverDspChain>>,
) -> ChannelDspChain {
    ChannelDspChain {
        channel: name.to_string(),
        plugins,
        drivers,
        initial_curve: None,
        final_curve: None,
        eq_response: None,
        target_curve: None,
        pre_ir: None,
        post_ir: None,
        fir_temporal_masking: None,
        direct_early_late_correction: None,
    }
}

/// Build a `DriverDspChain` with no optional `initial_curve`.
fn driver(name: &str, index: usize, plugins: Vec<DspPluginConfig>) -> DriverDspChain {
    DriverDspChain {
        name: name.to_string(),
        index,
        plugins,
        initial_curve: None,
    }
}

/// Build a `DspChainOutput` from a channels map with default version and
/// no metadata.
fn output(channels: std::collections::HashMap<String, ChannelDspChain>) -> DspChainOutput {
    DspChainOutput {
        version: "1.0.0".to_string(),
        global_plugins: Vec::new(),
        channels,
        metadata: None,
    }
}

/// Simulate the channel-to-filter mapping logic from apply_room_eq_to_player.
/// Given channel names in output order and a DSP output HashMap, returns
/// the ordered list of channel names as they would be mapped to audio indices.
#[allow(dead_code)]
fn build_per_channel_order(
    channel_result_names: &[&str],
    dsp_channels: &std::collections::HashMap<String, ChannelDspChain>,
) -> Vec<String> {
    let mut ordered = Vec::new();
    for name in channel_result_names {
        if dsp_channels.contains_key(*name) {
            ordered.push(name.to_string());
        } else {
            ordered.push(format!("{}(empty)", name));
        }
    }
    ordered
}

/// Build mock optimization results and DSP output for a given speaker config.
/// Each channel gets a unique EQ filter frequency to verify ordering.
fn build_mock_results(speaker_labels: &[&str]) -> (Vec<ChannelOptResult>, DspChainOutput) {
    let mut channel_results = Vec::new();
    let mut channels = std::collections::HashMap::new();

    for (idx, &label) in speaker_labels.iter().enumerate() {
        // Each channel gets a unique filter frequency = (idx+1) * 100 Hz
        let unique_freq = (idx + 1) as f64 * 100.0;

        channel_results.push(ChannelOptResult {
            channel_name: label.to_string(),
            pre_score: 1.0,
            post_score: 0.5,
            eq_filters: vec![sotf_audio_player_gpui::app::types::EqFilterConfig {
                filter_type: "peak".to_string(),
                frequency: unique_freq,
                q: 1.0,
                gain_db: -3.0,
            }],
            broadband_filters: vec![],
            preamp_gain_db: 0.0,
            crossover_freqs: None,
            driver_gains: None,
            original_response: None,
            corrected_response: None,
            normalized_response: None,
            target_curve: None,
            group_delay_before: None,
            group_delay_after: None,
            phase_response_before: None,
            phase_response_after: None,
            impulse_response: None,
        });

        channels.insert(
            label.to_string(),
            chain(
                label,
                vec![DspPluginConfig {
                    plugin_type: "EQ".to_string(),
                    parameters: serde_json::json!({
                        "filters": [{
                            "filter_type": "peak",
                            "frequency": unique_freq,
                            "q": 1.0,
                            "gain_db": -3.0
                        }]
                    }),
                }],
                None,
            ),
        );
    }

    let dsp_output = output(channels);

    (channel_results, dsp_output)
}

/// Extract per-channel filter frequencies in the order they'd be applied
/// to audio channels (reproducing the logic from apply_room_eq_to_player).
fn extract_filter_freqs(channel_result_names: &[String], dsp_output: &DspChainOutput) -> Vec<f64> {
    let mut freqs = Vec::new();
    for name in channel_result_names {
        if let Some(chain) = dsp_output.channels.get(name) {
            for plugin in &chain.plugins {
                if plugin.plugin_type.eq_ignore_ascii_case("eq")
                    && let Some(filters) =
                        plugin.parameters.get("filters").and_then(|f| f.as_array())
                {
                    for f in filters {
                        if let Some(freq) = f.get("frequency").and_then(|v| v.as_f64()) {
                            freqs.push(freq);
                        }
                    }
                }
            }
        } else {
            freqs.push(0.0); // placeholder for missing channel
        }
    }
    freqs
}

/// Test that channel ordering is correct for a given speaker config.
/// The channel_result_names (from recordings) should map filter[i] to audio channel i.
fn assert_channel_ordering(config_name: &str, labels: &[&str]) {
    let (results, dsp_output) = build_mock_results(labels);

    // channel_result_names preserves the output channel order
    let channel_result_names: Vec<String> =
        results.iter().map(|r| r.channel_name.clone()).collect();

    // Extract filter frequencies in the order they'd be applied
    let freqs = extract_filter_freqs(&channel_result_names, &dsp_output);

    // Each channel's unique frequency should match its index: freq = (idx+1) * 100
    for (idx, &freq) in freqs.iter().enumerate() {
        let expected = (idx + 1) as f64 * 100.0;
        assert_eq!(
            freq, expected,
            "{}: channel {} ('{}') has filter freq {}, expected {} — wrong channel ordering!",
            config_name, idx, labels[idx], freq, expected
        );
    }

    // Verify we didn't lose any channels
    assert_eq!(
        freqs.len(),
        labels.len(),
        "{}: expected {} channels, got {}",
        config_name,
        labels.len(),
        freqs.len()
    );
}

/// Verify that alphabetical sort would produce WRONG ordering for 5.1+
/// (this is the bug we fixed — alphabetical sort doesn't match audio channel indices).
#[test]
fn test_alphabetical_sort_is_wrong_for_surround() {
    let labels_5_1 = ["FL", "FR", "C", "LFE", "SL", "SR"];
    let mut sorted = labels_5_1.to_vec();
    sorted.sort();
    // Alphabetical: C, FL, FR, LFE, SL, SR — wrong order for audio channels!
    assert_ne!(
        sorted, labels_5_1,
        "Alphabetical sort must differ from audio channel order for 5.1"
    );
}

#[test]
fn test_channel_ordering_2_0() {
    assert_channel_ordering("2.0", &["L", "R"]);
}

#[test]
fn test_channel_ordering_2_1() {
    assert_channel_ordering("2.1", &["L", "R", "LFE"]);
}

#[test]
fn test_channel_ordering_5_0() {
    assert_channel_ordering("5.0", &["FL", "FR", "C", "SL", "SR"]);
}

#[test]
fn test_channel_ordering_5_1() {
    assert_channel_ordering("5.1", &["FL", "FR", "C", "LFE", "SL", "SR"]);
}

#[test]
fn test_channel_ordering_7_1() {
    assert_channel_ordering("7.1", &["FL", "FR", "C", "LFE", "SL", "SR", "RL", "RR"]);
}

#[test]
fn test_channel_ordering_5_1_2() {
    assert_channel_ordering("5.1.2", &["FL", "FR", "C", "LFE", "SL", "SR", "TFL", "TFR"]);
}

#[test]
fn test_channel_ordering_5_1_4() {
    assert_channel_ordering(
        "5.1.4",
        &[
            "FL", "FR", "C", "LFE", "SL", "SR", "TFL", "TFR", "TRL", "TRR",
        ],
    );
}

#[test]
fn test_channel_ordering_7_1_2() {
    assert_channel_ordering(
        "7.1.2",
        &["FL", "FR", "C", "LFE", "SL", "SR", "RL", "RR", "TFL", "TFR"],
    );
}

#[test]
fn test_channel_ordering_7_1_4() {
    assert_channel_ordering(
        "7.1.4",
        &[
            "FL", "FR", "C", "LFE", "SL", "SR", "RL", "RR", "TFL", "TFR", "TRL", "TRR",
        ],
    );
}

#[test]
fn test_channel_ordering_9_1_4() {
    assert_channel_ordering(
        "9.1.4",
        &[
            "FL", "FR", "C", "LFE", "SL", "SR", "RL", "RR", "FWL", "FWR", "TFL", "TFR", "TRL",
            "TRR",
        ],
    );
}

#[test]
fn test_channel_ordering_9_1_6() {
    assert_channel_ordering(
        "9.1.6",
        &[
            "FL", "FR", "C", "LFE", "SL", "SR", "RL", "RR", "FWL", "FWR", "TFL", "TFR", "TSL",
            "TSR", "TRL", "TRR",
        ],
    );
}

/// Test that HashMap key ordering doesn't affect the result.
/// This verifies the fix: we use channel_result_names (output order),
/// NOT dsp_output.channels.keys() (arbitrary HashMap order).
#[test]
fn test_hashmap_insertion_order_irrelevant() {
    // Insert channels into HashMap in reverse order
    let labels = ["FL", "FR", "C", "LFE", "SL", "SR"];
    let mut channels = std::collections::HashMap::new();

    for (idx, &label) in labels.iter().enumerate().rev() {
        let freq = (idx + 1) as f64 * 100.0;
        channels.insert(
            label.to_string(),
            chain(
                label,
                vec![DspPluginConfig {
                    plugin_type: "EQ".to_string(),
                    parameters: serde_json::json!({
                        "filters": [{"filter_type": "peak", "frequency": freq, "q": 1.0, "gain_db": -3.0}]
                    }),
                }],
                None,
            ),
        );
    }

    // channel_result_names in correct output order
    let channel_result_names: Vec<String> = labels.iter().map(|s| s.to_string()).collect();
    let freqs = extract_filter_freqs(&channel_result_names, &output(channels));

    for (idx, &freq) in freqs.iter().enumerate() {
        let expected = (idx + 1) as f64 * 100.0;
        assert_eq!(freq, expected, "Channel {} has wrong freq", idx);
    }
}

/// Regression: apply_smart_defaults must not wipe seed/refine/local_algo
/// that were imported from a backend config file.
#[test]
fn test_import_preserves_seed_and_refine() {
    let mut backend = make_bare_backend_config();
    backend.seed = Some(42);
    backend.refine = false;
    backend.local_algo = "cobyla".to_string();

    let mut state = RoomEqState::default();
    state.channel_measurements.push(make_dummy_measurement("L"));
    state.init_speaker_configs();

    state.optimizer_config.import_from_backend(&backend);
    state.apply_smart_defaults(None);

    assert_eq!(
        state.optimizer_config.seed,
        Some(42),
        "seed must survive smart defaults"
    );
    assert!(
        !state.optimizer_config.refine,
        "refine=false must survive smart defaults"
    );
    assert_eq!(
        state.optimizer_config.local_algo, "cobyla",
        "local_algo must survive smart defaults"
    );
}

// ============================================================================
// Save-to-rack integration tests
//
// These tests simulate the full apply_room_eq_to_player data flow:
// 1. Build DspChainOutput from optimizer results
// 2. Parse EQ filters from JSON (using parse_eq_filters_from_json)
// 3. Apply to PluginGraph (insert or update EQ)
// 4. Verify the resulting plugin chain
// ============================================================================

/// Simulate the save-to-rack flow: extract per-channel filters from DSP output,
/// insert or update EQ in the plugin graph, return the resulting graph.
fn simulate_save_to_rack(
    channel_result_names: &[&str],
    dsp_output: &DspChainOutput,
    graph: &mut PluginGraph,
) -> (usize, Vec<Vec<EQFilter>>) {
    let mut per_channel_filters: Vec<Vec<EQFilter>> = Vec::new();
    for name in channel_result_names {
        if let Some(chain) = dsp_output.channels.get(*name) {
            let mut channel_eq_filters: Vec<EQFilter> = Vec::new();
            for plugin in &chain.plugins {
                if plugin.plugin_type.eq_ignore_ascii_case("eq")
                    && let Some(filters) =
                        plugin.parameters.get("filters").and_then(|f| f.as_array())
                {
                    channel_eq_filters.extend(parse_eq_filters_from_json(filters));
                }
            }
            per_channel_filters.push(channel_eq_filters);
        } else {
            per_channel_filters.push(Vec::new());
        }
    }

    let total_filters: usize = per_channel_filters.iter().map(|f| f.len()).sum();
    let num_channels = per_channel_filters.len();
    let global_filters = per_channel_filters.first().cloned().unwrap_or_default();

    if total_filters > 0 {
        let new_settings = PluginSettings::EQ {
            channels: num_channels,
            filters: global_filters,
            channel_filters: Some(per_channel_filters.clone()),
            per_channel_mode: true,
            max_filters: 10,
            tdf2: false,
            topology: 0.0,
        };

        if let Some(eq_idx) = graph.find_plugin_index(&PluginType::EQ) {
            if let Some(eq_plugin) = graph.get_plugin_mut(eq_idx) {
                eq_plugin.settings = new_settings;
            }
        } else {
            let insert_idx = graph.user_plugin_insert_index();
            if graph.insert_plugin(insert_idx, &PluginType::EQ).is_ok()
                && let Some(eq_plugin) = graph.get_plugin_mut(insert_idx)
            {
                eq_plugin.settings = new_settings;
            }
        }
    }

    (total_filters, per_channel_filters)
}

type FilterTriple = (f64, f64, f64);
type ChannelFilters<'a> = (&'a str, Vec<FilterTriple>);

/// Build a DspChainOutput using autoeq format keys ("freq", "db_gain")
/// which is what the real optimizer produces.
fn build_autoeq_dsp_output(channels: &[ChannelFilters<'_>]) -> DspChainOutput {
    let mut map = std::collections::HashMap::new();
    for (name, filters) in channels {
        let filter_json: Vec<serde_json::Value> = filters
            .iter()
            .map(|&(freq, q, gain)| {
                serde_json::json!({
                    "filter_type": "peak",
                    "freq": freq,
                    "q": q,
                    "db_gain": gain
                })
            })
            .collect();
        map.insert(
            name.to_string(),
            chain(
                name,
                vec![DspPluginConfig {
                    plugin_type: "eq".to_string(),
                    parameters: serde_json::json!({ "filters": filter_json }),
                }],
                None,
            ),
        );
    }
    output(map)
}

#[test]
fn test_save_to_rack_stereo() {
    let dsp = build_autoeq_dsp_output(&[
        ("L", vec![(100.0, 1.5, -3.0)]),
        ("R", vec![(200.0, 2.0, -5.0)]),
    ]);
    assert!(dsp.is_rack_compatible());

    let mut graph = PluginGraph::with_default_rack();
    assert!(graph.find_plugin_index(&PluginType::EQ).is_none());

    let (total, per_ch) = simulate_save_to_rack(&["L", "R"], &dsp, &mut graph);
    assert_eq!(total, 2);
    assert_eq!(per_ch.len(), 2);
    assert_eq!(per_ch[0][0].frequency, 100.0);
    assert_eq!(per_ch[0][0].gain_db, -3.0);
    assert_eq!(per_ch[1][0].frequency, 200.0);
    assert_eq!(per_ch[1][0].gain_db, -5.0);

    // EQ plugin should now exist in graph
    let eq_idx = graph.find_plugin_index(&PluginType::EQ).unwrap();
    let eq = graph.get_plugin(eq_idx).unwrap();
    if let PluginSettings::EQ {
        channels,
        per_channel_mode,
        channel_filters,
        ..
    } = &eq.settings
    {
        assert_eq!(*channels, 2);
        assert!(*per_channel_mode);
        let cf = channel_filters.as_ref().unwrap();
        assert_eq!(cf[0][0].frequency, 100.0);
        assert_eq!(cf[1][0].frequency, 200.0);
    } else {
        panic!("Expected EQ settings");
    }
}

#[test]
fn test_save_to_rack_update_existing_eq() {
    let mut graph = PluginGraph::with_default_rack();
    // Pre-add an EQ with default flat settings
    graph.add_user_plugin(&PluginType::EQ).unwrap();
    let eq_idx_before = graph.find_plugin_index(&PluginType::EQ).unwrap();
    let plugin_count_before = graph.len();

    // Apply room EQ
    let dsp = build_autoeq_dsp_output(&[
        ("L", vec![(80.0, 0.7, 2.0)]),
        ("R", vec![(160.0, 1.2, -4.0)]),
    ]);
    let (total, _) = simulate_save_to_rack(&["L", "R"], &dsp, &mut graph);
    assert_eq!(total, 2);

    // Should NOT have added a new plugin
    assert_eq!(graph.len(), plugin_count_before);
    // Position should be unchanged
    assert_eq!(
        graph.find_plugin_index(&PluginType::EQ).unwrap(),
        eq_idx_before
    );

    // Settings should be updated
    let eq = graph.get_plugin(eq_idx_before).unwrap();
    if let PluginSettings::EQ {
        per_channel_mode,
        channel_filters,
        ..
    } = &eq.settings
    {
        assert!(*per_channel_mode);
        let cf = channel_filters.as_ref().unwrap();
        assert_eq!(cf[0][0].frequency, 80.0);
        assert_eq!(cf[1][0].frequency, 160.0);
    } else {
        panic!("Expected EQ settings");
    }
}

#[test]
fn test_save_to_rack_5_1_surround() {
    let labels = ["FL", "FR", "C", "LFE", "SL", "SR"];
    let channels: Vec<ChannelFilters<'_>> = labels
        .iter()
        .enumerate()
        .map(|(i, &name)| (name, vec![((i + 1) as f64 * 100.0, 1.0, -3.0)]))
        .collect();
    let dsp = build_autoeq_dsp_output(&channels);

    let mut graph = PluginGraph::with_default_rack();
    let (total, per_ch) = simulate_save_to_rack(&labels, &dsp, &mut graph);

    assert_eq!(total, 6);
    assert_eq!(per_ch.len(), 6);
    for (i, ch_filters) in per_ch.iter().enumerate() {
        let expected_freq = (i + 1) as f64 * 100.0;
        assert_eq!(
            ch_filters[0].frequency, expected_freq,
            "Channel {} ({}) should have freq {}",
            i, labels[i], expected_freq
        );
    }

    let eq = graph
        .get_plugin(graph.find_plugin_index(&PluginType::EQ).unwrap())
        .unwrap();
    if let PluginSettings::EQ { channels, .. } = &eq.settings {
        assert_eq!(*channels, 6);
    } else {
        panic!("Expected EQ settings");
    }
}

#[test]
fn test_save_to_rack_no_filters_detected() {
    let dsp = output(
        [
            (
                "L".to_string(),
                chain(
                    "L",
                    vec![DspPluginConfig {
                        plugin_type: "eq".to_string(),
                        parameters: serde_json::json!({ "filters": [] }),
                    }],
                    None,
                ),
            ),
            (
                "R".to_string(),
                chain(
                    "R",
                    vec![DspPluginConfig {
                        plugin_type: "eq".to_string(),
                        parameters: serde_json::json!({ "filters": [] }),
                    }],
                    None,
                ),
            ),
        ]
        .into_iter()
        .collect(),
    );

    let mut graph = PluginGraph::with_default_rack();
    let (total, _) = simulate_save_to_rack(&["L", "R"], &dsp, &mut graph);
    assert_eq!(total, 0);
    // No EQ should be inserted
    assert!(graph.find_plugin_index(&PluginType::EQ).is_none());
}

#[test]
fn test_save_to_rack_missing_channel() {
    // DSP output has only L, but we ask for L, R, C
    let dsp = build_autoeq_dsp_output(&[("L", vec![(100.0, 1.5, -3.0)])]);

    let mut graph = PluginGraph::with_default_rack();
    let (total, per_ch) = simulate_save_to_rack(&["L", "R", "C"], &dsp, &mut graph);

    assert_eq!(total, 1);
    assert_eq!(per_ch.len(), 3);
    assert_eq!(per_ch[0].len(), 1); // L has filter
    assert_eq!(per_ch[1].len(), 0); // R missing -> empty
    assert_eq!(per_ch[2].len(), 0); // C missing -> empty
}

#[test]
fn test_save_to_rack_multiple_eq_plugins_merged() {
    let dsp = output(
        [(
            "L".to_string(),
            chain(
                "L",
                vec![
                    DspPluginConfig {
                        plugin_type: "eq".to_string(),
                        parameters: serde_json::json!({ "filters": [
                            {"filter_type": "peak", "freq": 100.0, "q": 1.0, "db_gain": -2.0}
                        ]}),
                    },
                    DspPluginConfig {
                        plugin_type: "EQ".to_string(), // different casing
                        parameters: serde_json::json!({ "filters": [
                            {"filter_type": "peak", "freq": 500.0, "q": 2.0, "db_gain": -4.0}
                        ]}),
                    },
                ],
                None,
            ),
        )]
        .into_iter()
        .collect(),
    );

    let mut graph = PluginGraph::with_default_rack();
    let (total, per_ch) = simulate_save_to_rack(&["L"], &dsp, &mut graph);
    assert_eq!(total, 2);
    assert_eq!(per_ch[0].len(), 2);
    assert_eq!(per_ch[0][0].frequency, 100.0);
    assert_eq!(per_ch[0][1].frequency, 500.0);
}

#[test]
fn test_save_to_rack_non_eq_plugins_skipped() {
    let dsp = output(
        [(
            "L".to_string(),
            chain(
                "L",
                vec![
                    DspPluginConfig {
                        plugin_type: "gain".to_string(),
                        parameters: serde_json::json!({ "gain_db": -6.0 }),
                    },
                    DspPluginConfig {
                        plugin_type: "eq".to_string(),
                        parameters: serde_json::json!({ "filters": [
                            {"filter_type": "peak", "freq": 1000.0, "q": 1.5, "db_gain": -3.0}
                        ]}),
                    },
                    DspPluginConfig {
                        plugin_type: "delay".to_string(),
                        parameters: serde_json::json!({ "delay_ms": 5.0 }),
                    },
                ],
                None,
            ),
        )]
        .into_iter()
        .collect(),
    );

    let mut graph = PluginGraph::with_default_rack();
    let (total, per_ch) = simulate_save_to_rack(&["L"], &dsp, &mut graph);
    assert_eq!(total, 1);
    assert_eq!(per_ch[0][0].frequency, 1000.0);
}

#[test]
fn test_save_to_rack_plugin_type_case_insensitive() {
    let dsp = output(
        [
            (
                "L".to_string(),
                chain(
                    "L",
                    vec![DspPluginConfig {
                        plugin_type: "EQ".to_string(), // uppercase
                        parameters: serde_json::json!({ "filters": [
                            {"filter_type": "peak", "freq": 100.0, "q": 1.0, "db_gain": -1.0}
                        ]}),
                    }],
                    None,
                ),
            ),
            (
                "R".to_string(),
                chain(
                    "R",
                    vec![DspPluginConfig {
                        plugin_type: "Eq".to_string(), // mixed case
                        parameters: serde_json::json!({ "filters": [
                            {"filter_type": "peak", "freq": 200.0, "q": 1.0, "db_gain": -2.0}
                        ]}),
                    }],
                    None,
                ),
            ),
        ]
        .into_iter()
        .collect(),
    );

    let mut graph = PluginGraph::with_default_rack();
    let (total, per_ch) = simulate_save_to_rack(&["L", "R"], &dsp, &mut graph);
    assert_eq!(total, 2);
    assert_eq!(per_ch[0][0].frequency, 100.0);
    assert_eq!(per_ch[1][0].frequency, 200.0);
}

#[test]
fn test_multi_driver_not_rack_compatible() {
    let dsp = output(
        [(
            "L".to_string(),
            chain(
                "L",
                vec![DspPluginConfig {
                    plugin_type: "eq".to_string(),
                    parameters: serde_json::json!({ "filters": [
                        {"filter_type": "peak", "freq": 100.0, "q": 1.0, "db_gain": -3.0}
                    ]}),
                }],
                Some(vec![
                    driver(
                        "woofer",
                        0,
                        vec![DspPluginConfig {
                            plugin_type: "crossover".to_string(),
                            parameters: serde_json::json!({ "type": "lowpass", "freq": 2000.0 }),
                        }],
                    ),
                    driver(
                        "tweeter",
                        1,
                        vec![DspPluginConfig {
                            plugin_type: "crossover".to_string(),
                            parameters: serde_json::json!({ "type": "highpass", "freq": 2000.0 }),
                        }],
                    ),
                ]),
            ),
        )]
        .into_iter()
        .collect(),
    );

    assert!(
        !dsp.is_rack_compatible(),
        "Multi-driver DSP output should NOT be rack compatible"
    );
}
