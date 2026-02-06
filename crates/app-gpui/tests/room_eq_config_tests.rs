//! Room EQ configuration and serialization tests.

use sotf_audio_player_gpui::{
    ChannelMeasurement, RecordingResult, RoomEqState, RoomEqStep,
};

#[test]
fn test_room_eq_state_defaults() {
    let state = RoomEqState::default();
    assert_eq!(state.step, RoomEqStep::LoadData);
    assert_eq!(state.optimizer_config.num_filters, 5);
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
    });

    state.init_speaker_configs();

    let config = state.to_room_config();

    assert_eq!(config.speakers.len(), 1);
    assert!(config.speakers.contains_key("L"));

    // Check optimizer config
    assert_eq!(config.optimizer.num_filters, 5);
    assert_eq!(config.optimizer.mode, "iir");
}

#[test]
fn test_room_eq_to_room_config_advanced() {
    let mut state = RoomEqState::default();

    // Set advanced parameters
    state.optimizer_config.target_tilt.enabled = true;
    state.optimizer_config.target_tilt.tilt_type = "harman".to_string();
    state.optimizer_config.target_tilt.slope = -1.0;

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
    });
    state.init_speaker_configs();

    let config = state.to_room_config();

    // Verify advanced sections are present
    assert!(config.optimizer.target_tilt.is_some());
    let tilt = config.optimizer.target_tilt.as_ref().unwrap();
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
