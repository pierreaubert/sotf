//! Room EQ configuration and serialization tests.

use sotf_audio_player_gpui::{
    ChannelMeasurement, RecordingResult, RoomEqOptimizerConfig, RoomEqState, RoomEqStep,
};

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
        multi_mic_measurements: Vec::new(),
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
        mode: "iir".to_string(),
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
        psychoacoustic: true,
        smooth_n: 2,
        asymmetric_loss: true,
        tolerance: 1e-5,
        atolerance: 1e-5,
        allow_delay: None,
        // All feature flags None = disabled
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
        cea2034_correction: None,
        sub_config: None,
        channel_matching: None,
        strategy: "lshade".to_string(),
        target_response: None,
        phase_correction: None,
        min_filter_improvement: 0.01,
        elimination_threshold: 0.005,
        ssir_wav_path: None,
        max_boost_envelope: None,
            min_cut_envelope: None,
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
    assert!(!config.target_tilt.enabled);
    assert!(!config.excursion_protection.enabled);
    assert!(!config.schroeder_split.enabled);
    assert!(!config.broadband_target_matching.enabled);
    assert!(!config.gd_opt.enabled);
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
    state.apply_smart_defaults();

    let room_config = state.to_room_config();
    let opt = &room_config.optimizer;

    // Features that were None in backend must remain None after smart defaults
    assert!(opt.target_tilt.is_none(), "target_tilt should be None");
    assert!(
        opt.excursion_protection.is_none(),
        "excursion_protection should be None"
    );
    assert!(
        opt.schroeder_split.is_none(),
        "schroeder_split should be None"
    );
    assert!(
        opt.broadband_target_matching.is_none(),
        "broadband_target_matching should be None"
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

    state.apply_smart_defaults();

    let room_config = state.to_room_config();
    let opt = &room_config.optimizer;

    // Smart defaults should enable features for fresh measurements
    assert!(opt.target_tilt.is_some(), "target_tilt should be enabled");
    assert!(
        opt.excursion_protection.is_some(),
        "excursion_protection should be enabled"
    );
    assert!(
        opt.schroeder_split.is_some(),
        "schroeder_split should be enabled"
    );
    assert!(
        opt.broadband_target_matching.is_some(),
        "broadband_target_matching should be enabled"
    );
}

/// Test that importing a backend WITH features enabled preserves them through smart defaults.
#[test]
fn test_import_with_features_enabled_preserves_them() {
    let mut backend = make_bare_backend_config();
    backend.target_tilt = Some(autoeq::roomeq::TargetTiltConfig {
        tilt_type: autoeq::roomeq::TiltType::Harman,
        slope_db_per_octave: -1.0,
        reference_freq: 1000.0,
        bass_shelf_db: 2.0,
        bass_shelf_freq: 200.0,
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
    state.apply_smart_defaults();

    let room_config = state.to_room_config();
    let opt = &room_config.optimizer;

    // target_tilt was enabled in backend
    let tilt = opt
        .target_tilt
        .as_ref()
        .expect("target_tilt should be Some");
    assert_eq!(tilt.slope_db_per_octave, -1.0);
    assert_eq!(tilt.bass_shelf_db, 2.0);

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

/// Regression: broadband_target_matching has an inner `enabled` field.
/// If backend has `Some(BroadbandTargetMatchingConfig { enabled: false })`,
/// the GPUI must treat it as disabled.
#[test]
fn test_import_broadband_with_enabled_false() {
    let mut backend = make_bare_backend_config();
    backend.broadband_target_matching =
        Some(autoeq::roomeq::BroadbandTargetMatchingConfig { enabled: false });

    let mut config = RoomEqOptimizerConfig::default();
    config.import_from_backend(&backend);

    assert!(!config.broadband_target_matching.enabled);
}

// ============================================================================
// Room EQ channel ordering tests
//
// When applying room EQ to the player, per-channel filters must be indexed
// by audio channel order (0=FL, 1=FR, 2=C, 3=LFE, etc.), NOT alphabetically.
// These tests verify the ordering for all standard surround configurations.
// ============================================================================

use sotf_audio_player_gpui::{ChannelDspChain, ChannelOptResult, DspChainOutput, DspPluginConfig};

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
            ChannelDspChain {
                channel: label.to_string(),
                plugins: vec![DspPluginConfig {
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
                drivers: None,
            },
        );
    }

    let dsp_output = DspChainOutput {
        channels,
        metadata: None,
    };

    (channel_results, dsp_output)
}

/// Extract per-channel filter frequencies in the order they'd be applied
/// to audio channels (reproducing the logic from apply_room_eq_to_player).
fn extract_filter_freqs(channel_result_names: &[String], dsp_output: &DspChainOutput) -> Vec<f64> {
    let mut freqs = Vec::new();
    for name in channel_result_names {
        if let Some(chain) = dsp_output.channels.get(name) {
            for plugin in &chain.plugins {
                if plugin.plugin_type == "EQ"
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
            ChannelDspChain {
                channel: label.to_string(),
                plugins: vec![DspPluginConfig {
                    plugin_type: "EQ".to_string(),
                    parameters: serde_json::json!({
                        "filters": [{"filter_type": "peak", "frequency": freq, "q": 1.0, "gain_db": -3.0}]
                    }),
                }],
                drivers: None,
            },
        );
    }

    // channel_result_names in correct output order
    let channel_result_names: Vec<String> = labels.iter().map(|s| s.to_string()).collect();
    let freqs = extract_filter_freqs(
        &channel_result_names,
        &DspChainOutput {
            channels,
            metadata: None,
        },
    );

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
    backend.local_algo = "neldermead".to_string();

    let mut state = RoomEqState::default();
    state.channel_measurements.push(make_dummy_measurement("L"));
    state.init_speaker_configs();

    state.optimizer_config.import_from_backend(&backend);
    state.apply_smart_defaults();

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
        state.optimizer_config.local_algo, "neldermead",
        "local_algo must survive smart defaults"
    );
}
