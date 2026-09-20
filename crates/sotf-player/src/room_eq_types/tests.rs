use super::build::build_room_eq_plugin_graph_config;
use super::build::build_speakers_from_recordings;
use super::ctc::ctc_system_config_for_speaker_names;
use super::delay_detection_state::DelayDetectionState;
use super::delay_detection_status::DelayDetectionStatus;
use super::misc::estimate_probe_sequence_ms;
use super::misc::parse_eq_filters_from_json;
use super::multi_measurement_ui_config::MultiMeasurementUiConfig;
use super::room_eq_measurements_file::RoomEqMeasurementsFile;
use super::room_eq_optimization_mode::{
    RoomEqEasyLayout, SimpleCrossoverChoice, apply_room_eq_easy_layout, apply_simple_preset,
    validate_room_eq_easy_layout,
};
use super::room_eq_optimizer_config::RoomEqOptimizerConfig;
use super::target_response_ui_config::TargetResponseUiConfig;
use super::types::read_first_wav_channel_f32;
use crate::recording_types::{ChannelRecording, ChannelRecordingState, RecordingResult};
pub use autoeq::roomeq::SimplePresetConfig;
use math_audio_iir_fir::BiquadFilterType;

mod bare;
mod misc;
mod routed;

use misc::assert_room_eq_matrix_nodes_have_width;
use misc::collect_labels;
use routed::routed_bass_output;
use routed::routed_physical_sub_output;

#[test]
fn ctc_system_config_maps_speaker_names_to_logical_roles() {
    let system = ctc_system_config_for_speaker_names(["L", "R", "LFE [mic 1]"], None)
        .expect("speaker names produce a system config");

    assert_eq!(system.model, autoeq::roomeq::SystemModel::HomeCinema);
    assert_eq!(system.speakers.get("L").map(String::as_str), Some("L"));
    assert_eq!(system.speakers.get("R").map(String::as_str), Some("R"));
    assert_eq!(
        system.speakers.get("LFE [mic 1]").map(String::as_str),
        Some("LFE [mic 1]")
    );
    assert!(system.subwoofers.is_some());
}

#[test]
fn ctc_system_config_skips_empty_speaker_sets() {
    assert!(ctc_system_config_for_speaker_names([""], None).is_none());
}

#[test]
fn read_first_wav_channel_handles_extensible_pcm32() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pcm32_extensible.wav");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&40u32.to_le_bytes());
    bytes.extend_from_slice(&65534u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&48_000u32.to_le_bytes());
    bytes.extend_from_slice(&(48_000u32 * 4).to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&32u16.to_le_bytes());
    bytes.extend_from_slice(&22u16.to_le_bytes());
    bytes.extend_from_slice(&32u16.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&[
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b,
        0x71,
    ]);
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&4u32.to_le_bytes());
    bytes.extend_from_slice(&i32::MAX.to_le_bytes());
    let riff_size = (bytes.len() - 8) as u32;
    bytes[4..8].copy_from_slice(&riff_size.to_le_bytes());
    std::fs::write(&path, bytes).unwrap();

    let (samples, sample_rate) = read_first_wav_channel_f32(&path).unwrap();
    assert_eq!(sample_rate, 48_000);
    assert_eq!(samples.len(), 1);
    assert!((samples[0] - 1.0).abs() < 1e-6);
}

#[test]
fn load_from_json_room_config_format() {
    let json = r#"{
            "version": "1.1.0",
            "speakers": {
                "R": {
                    "frequencies": [20.0, 100.0, 1000.0],
                    "magnitude_db": [-10.0, -3.0, 0.0],
                    "phase_deg": [5.0, 10.0, 15.0],
                    "name": "R"
                },
                "L": {
                    "frequencies": [20.0, 100.0, 1000.0],
                    "magnitude_db": [-9.0, -2.0, 1.0],
                    "name": "L"
                }
            },
            "optimizer": {}
        }"#;
    let channels = RoomEqMeasurementsFile::load_from_json(json, None).unwrap();
    assert_eq!(channels.len(), 2);
    for ch in &channels {
        assert_eq!(ch.measurement.frequencies.len(), 3);
        assert_eq!(ch.measurement.magnitude_db.len(), 3);
    }
}

#[test]
fn load_from_json_room_config_real_file() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("data_generated/recording-adam-20260114-142539/recordings.json");
    if !path.exists() {
        // Skip if test data not available
        return;
    }
    let json = std::fs::read_to_string(&path).unwrap();
    let base_dir = path.parent();
    let channels = RoomEqMeasurementsFile::load_from_json(&json, base_dir).unwrap();
    assert!(
        !channels.is_empty(),
        "Should load at least one channel from real recording file"
    );
    for ch in &channels {
        assert!(
            !ch.measurement.frequencies.is_empty(),
            "Channel '{}' should have frequency data",
            ch.channel_name
        );
    }
}

#[test]
fn load_from_json_preserves_multi_position_measurements() {
    // RoomConfig saved by app-gpui groups multi-position recordings into
    // MeasurementSource::Multiple. Loading must keep every measurement so
    // multi_mic_measurements stays populated for downstream optimization.
    let json = r#"{
            "version": "1.1.0",
            "speakers": {
                "L": {
                    "measurements": [
                        {
                            "frequencies": [20.0, 100.0, 1000.0],
                            "magnitude_db": [-10.0, -3.0, 0.0],
                            "name": "L (Pos 1)"
                        },
                        {
                            "frequencies": [20.0, 100.0, 1000.0],
                            "magnitude_db": [-9.0, -2.5, 0.5],
                            "name": "L (Pos 2)"
                        },
                        {
                            "frequencies": [20.0, 100.0, 1000.0],
                            "magnitude_db": [-8.5, -2.0, 1.0],
                            "name": "L (Pos 3)"
                        }
                    ]
                }
            },
            "optimizer": {}
        }"#;
    let channels = RoomEqMeasurementsFile::load_from_json(json, None).unwrap();
    assert_eq!(channels.len(), 1);
    let l = &channels[0];
    assert_eq!(l.channel_name, "L");
    assert_eq!(l.measurement.frequencies.len(), 3);
    assert_eq!(
        l.multi_mic_measurements.len(),
        2,
        "two extra positions should populate multi_mic_measurements"
    );
    assert_eq!(l.multi_mic_measurements[0].frequencies.len(), 3);
    assert_eq!(l.multi_mic_measurements[1].frequencies.len(), 3);
    assert_eq!(l.provenance.len(), 3);
    assert_eq!(l.provenance[0].name.as_deref(), Some("L (Pos 1)"));
    assert_eq!(l.provenance[1].name.as_deref(), Some("L (Pos 2)"));
    assert_eq!(l.provenance[2].name.as_deref(), Some("L (Pos 3)"));
    // Display names are retained verbatim, not parsed into asserted seat identity.
    assert!(
        l.provenance
            .iter()
            .all(|source| source.mic_position_index.is_none())
    );
}

#[test]
fn load_from_json_rejects_incomplete_single_driver_positions() {
    let json = r#"{
        "version": "1.1.0",
        "speakers": {
            "L": { "measurements": [
                { "frequencies": [100.0, 1000.0], "magnitude_db": [-3.0, 0.0] },
                { "frequencies": [], "magnitude_db": [], "name": "Missing position" }
            ] }
        }
    }"#;
    let error = RoomEqMeasurementsFile::load_from_json(json, None).unwrap_err();
    assert!(
        error.contains("Speaker L: loaded 1 of 2 measurement positions"),
        "{error}"
    );
}

#[test]
fn load_from_json_rejects_empty_single_measurement() {
    let json = r#"{
        "version": "1.1.0",
        "speakers": {
            "L": { "frequencies": [], "magnitude_db": [] }
        }
    }"#;
    let error = RoomEqMeasurementsFile::load_from_json(json, None).unwrap_err();
    assert!(
        error.contains("Speaker L: loaded 0 of 1 measurement positions"),
        "{error}"
    );
}

#[test]
fn build_speakers_from_recordings_groups_per_channel() {
    // Three output channels (L, R, LFE) × four mics × one position
    // is the genelec-2_1 shape that originally produced 12 EQ chains
    // when each (channel, mic) pair was emitted as its own
    // SpeakerConfig. The helper must collapse this to 3 entries with
    // four MeasurementRefs each.
    let mut recordings = Vec::new();
    for (channel_index, channel_label) in [(0, "L"), (1, "R"), (2, "LFE")] {
        for mic_idx in 0..4 {
            let display = format!("{} (Mic {})", channel_label, mic_idx + 1);
            let mut rec =
                ChannelRecording::with_mic_position(channel_index, display.clone(), mic_idx, 0);
            rec.state = ChannelRecordingState::Done;
            let safe = display.replace([' ', '(', ')'], "_");
            rec.result = Some(RecordingResult {
                sample_rate_hz: None,
                channel: channel_index,
                wav_path: Some(format!("/tmp/recording/{}.wav", safe)),
                csv_path: Some(format!("/tmp/recording/{}.csv", safe)),
                frequencies: vec![100.0],
                magnitude_db: vec![0.0],
                phase_deg: vec![0.0],
                impulse_response: None,
                impulse_time_ms: None,
                excess_group_delay_ms: None,
                thd_percent: None,
                harmonic_distortion_db: None,
                rt60_ms: None,
                clarity_c50_db: None,
                clarity_c80_db: None,
                spectrogram_db: None,
                quality: None,
            });
            recordings.push(rec);
        }
    }

    let channel_names = vec!["L".to_string(), "R".to_string(), "LFE".to_string()];
    let speakers = build_speakers_from_recordings(&recordings, &channel_names, None);

    assert_eq!(
        speakers.len(),
        3,
        "expected one SpeakerConfig per channel, got {}",
        speakers.len()
    );
    for channel in ["L", "R", "LFE"] {
        let entry = speakers
            .get(channel)
            .unwrap_or_else(|| panic!("missing speaker entry for {}", channel));
        match entry {
            autoeq::SpeakerConfig::Single(autoeq::MeasurementSource::Multiple(m)) => {
                assert_eq!(
                    m.measurements.len(),
                    4,
                    "expected 4 mic measurements for channel {}",
                    channel
                );
                for r in &m.measurements {
                    let inline = r.inline_data().expect("ref must be inline");
                    assert!(
                        inline.csv_path.as_deref().is_some_and(|p| !p.contains('/')),
                        "csv_path must be a session-relative filename"
                    );
                    assert!(
                        inline.wav_path.as_deref().is_some_and(|p| !p.contains('/')),
                        "wav_path must be a session-relative filename"
                    );
                }
            }
            other => {
                panic!("expected MeasurementSource::Multiple for {channel}, got {other:?}")
            }
        }
    }
}

#[test]
fn multi_measurement_ui_config_serde_roundtrip() {
    let config = MultiMeasurementUiConfig {
        enabled: true,
        strategy: "weighted_sum".to_string(),
        variance_lambda: 2.5,
        weights: vec![0.3, 0.7],
        bootstrap_uncertainty: None,
    };
    let json = serde_json::to_string(&config).unwrap();
    let roundtrip: MultiMeasurementUiConfig = serde_json::from_str(&json).unwrap();
    assert!(roundtrip.enabled);
    assert_eq!(roundtrip.strategy, "weighted_sum");
    assert_eq!(roundtrip.variance_lambda, 2.5);
    assert_eq!(roundtrip.weights, vec![0.3, 0.7]);
}

#[test]
fn multi_measurement_ui_config_default_deserialize() {
    // Ensure missing multi_measurement field in existing configs deserializes to default
    let json = r#"{
            "mode": "Iir",
            "multi_speaker_mode": "Combined",
            "algorithm": "autoeq:de",
            "num_filters": 7,
            "min_q": 0.5, "max_q": 6.0,
            "min_db": -12.0, "max_db": 4.0,
            "min_freq": 20.0, "max_freq": 1600.0,
            "max_iter": 50000,
            "peq_model": "pk",
            "population": 300,
            "refine": false,
            "local_algo": "cobyla",
            "loss_type": "flat",
            "psychoacoustic": true,
            "asymmetric_loss": true,
            "target_curve": "flat",
            "system_type": "stereo"
        }"#;
    let config: RoomEqOptimizerConfig = serde_json::from_str(json).unwrap();
    assert!(!config.multi_measurement.enabled);
    assert_eq!(config.multi_measurement.strategy, "average");
    assert_eq!(config.multi_measurement.variance_lambda, 1.0);
    assert!(config.multi_measurement.weights.is_empty());
}

#[test]
fn multi_measurement_strategy_strings_match_constants() {
    let valid_strategies = [
        "average",
        "weighted_sum",
        "minimax",
        "variance_penalized",
        "spatial_robustness",
        "minimax_uncertainty",
    ];
    let default = MultiMeasurementUiConfig::default();
    assert!(
        valid_strategies.contains(&default.strategy.as_str()),
        "Default strategy '{}' not in valid set",
        default.strategy
    );
}

#[test]
fn multi_measurement_strategy_accepts_display_labels() {
    let cases = [
        ("Average", autoeq::roomeq::MultiMeasurementStrategy::Average),
        (
            "Minimize Variance",
            autoeq::roomeq::MultiMeasurementStrategy::VariancePenalized,
        ),
        ("MinMax", autoeq::roomeq::MultiMeasurementStrategy::Minimax),
        (
            "Weighted Sum",
            autoeq::roomeq::MultiMeasurementStrategy::WeightedSum,
        ),
    ];

    for (label, expected) in cases {
        let mut config = RoomEqOptimizerConfig::default();
        config.multi_measurement.enabled = true;
        config.multi_measurement.strategy = label.to_string();

        let backend = config.to_optimizer_config();
        assert_eq!(backend.multi_measurement.unwrap().strategy, expected);
    }
}

#[test]
fn simple_preset_canonicalizes_multi_position_strategy_label() {
    let preset = SimplePresetConfig {
        multi_position_strategy: "Minimize Variance".to_string(),
        ..Default::default()
    };
    let mut config = RoomEqOptimizerConfig::default();

    apply_simple_preset(&preset, &mut config);

    assert!(config.multi_measurement.enabled);
    assert_eq!(config.multi_measurement.strategy, "variance_penalized");
}

#[test]
fn easy_layouts_validate_canonical_and_common_aliases() {
    assert!(validate_room_eq_easy_layout(RoomEqEasyLayout::Stereo20, ["L", "R"]).is_ok());
    assert!(validate_room_eq_easy_layout(RoomEqEasyLayout::Stereo21, ["FL", "FR", "Sub"]).is_ok());
    assert!(
        validate_room_eq_easy_layout(
            RoomEqEasyLayout::Surround51,
            [
                "Front Left",
                "Front Right",
                "Center",
                "LFE",
                "Rear Left",
                "Rear Right"
            ],
        )
        .is_ok()
    );
}

#[test]
fn easy_layout_validation_reports_missing_extra_and_duplicate_roles() {
    let error = validate_room_eq_easy_layout(RoomEqEasyLayout::Stereo21, ["L", "Left", "Height"])
        .unwrap_err();

    assert_eq!(error.missing_roles, vec!["FR", "LFE"]);
    assert_eq!(error.unexpected_channels, vec!["Height"]);
    assert_eq!(error.duplicate_roles, vec!["FL"]);
    assert!(error.to_string().contains("2.1 layout requires channels"));
}

#[test]
fn easy_layout_apply_configures_bass_management_and_clears_stale_stereo_state() {
    let mut preset = SimplePresetConfig::default();
    let mut config = RoomEqOptimizerConfig::default();

    RoomEqEasyLayout::Stereo21.configure_preset_defaults(&mut preset);
    apply_room_eq_easy_layout(
        RoomEqEasyLayout::Stereo21,
        ["L", "R", "LFE"],
        &mut preset,
        &mut config,
    )
    .unwrap();
    assert_eq!(preset.crossover, SimpleCrossoverChoice::Lr24);
    assert_eq!(preset.bass_management, "Standard");
    assert!(config.schroeder_split.enabled);

    apply_room_eq_easy_layout(
        RoomEqEasyLayout::Stereo20,
        ["L", "R"],
        &mut preset,
        &mut config,
    )
    .unwrap();
    assert_eq!(preset.crossover, SimpleCrossoverChoice::Lr24);
    assert!(preset.bass_management.is_empty());
    assert!(!config.schroeder_split.enabled);
}

#[test]
fn easy_layout_apply_is_atomic_when_measurements_do_not_match() {
    let mut preset = SimplePresetConfig {
        crossover: SimpleCrossoverChoice::Lr48,
        bass_management: "Cardioid".to_string(),
        ..Default::default()
    };
    let original_preset = preset.clone();
    let mut config = RoomEqOptimizerConfig {
        num_filters: 13,
        ..Default::default()
    };
    let original_config = config.clone();

    assert!(
        apply_room_eq_easy_layout(
            RoomEqEasyLayout::Surround51,
            ["L", "R", "LFE"],
            &mut preset,
            &mut config,
        )
        .is_err()
    );
    assert_eq!(preset.crossover, original_preset.crossover);
    assert_eq!(preset.bass_management, original_preset.bass_management);
    assert_eq!(config.num_filters, original_config.num_filters);
    assert_eq!(
        config.schroeder_split.enabled,
        original_config.schroeder_split.enabled
    );
}

#[test]
fn probe_arrival_map_returns_none_when_idle() {
    let dd = DelayDetectionState::default();
    assert_eq!(dd.status, DelayDetectionStatus::Idle);
    assert!(dd.probe_arrival_map().is_none());
}

#[test]
fn probe_arrival_map_returns_none_when_failed() {
    let dd = DelayDetectionState {
        status: DelayDetectionStatus::Failed("mic unplugged".to_string()),
        ..Default::default()
    };
    assert!(dd.probe_arrival_map().is_none());
}

#[test]
fn status_progress_returns_none_when_idle_or_failed() {
    let idle = DelayDetectionStatus::Idle;
    assert_eq!(idle.progress(10_000, 5_000), None);
    let failed = DelayDetectionStatus::Failed("x".to_string());
    assert_eq!(failed.progress(10_000, 5_000), None);
}

#[test]
fn status_progress_computes_fraction_when_running() {
    let running = DelayDetectionStatus::Running {
        started_at_ms: 1_000,
    };
    // 3000 ms elapsed out of 10000 estimated = 30%
    let p = running.progress(10_000, 4_000).unwrap();
    assert!((p - 0.3).abs() < 1e-6);
    // Clamps to 1.0 after the estimated total elapses.
    let p = running.progress(10_000, 50_000).unwrap();
    assert_eq!(p, 1.0);
}

#[test]
fn status_progress_returns_none_for_zero_total() {
    let running = DelayDetectionStatus::Running { started_at_ms: 0 };
    assert_eq!(running.progress(0, 1000), None);
}

#[test]
fn estimate_probe_sequence_ms_sums_channels_gaps_and_headroom() {
    // 3 channels × (1000 ms probe + 500 ms gap) + 1000 ms head/tail
    let total = estimate_probe_sequence_ms(3, 1000.0, 500.0);
    assert_eq!(total, 3 * 1500 + 1000);
}

#[test]
fn estimate_probe_sequence_ms_zero_channels_is_zero() {
    assert_eq!(estimate_probe_sequence_ms(0, 1000.0, 500.0), 0);
}

#[test]
fn test_parse_filters_autoeq_format() {
    let json: Vec<serde_json::Value> = serde_json::from_str(
        r#"[
            {"filter_type": "peak", "freq": 200.0, "q": 2.0, "db_gain": -5.0}
        ]"#,
    )
    .unwrap();
    let filters = parse_eq_filters_from_json(&json);
    assert_eq!(filters.len(), 1);
    assert_eq!(filters[0].frequency, 200.0);
    assert_eq!(filters[0].q, 2.0);
    assert_eq!(filters[0].gain_db, -5.0);
    assert_eq!(filters[0].filter_type, BiquadFilterType::Peak);
}

#[test]
fn test_parse_filters_engine_format() {
    let json: Vec<serde_json::Value> = serde_json::from_str(
        r#"[
            {"filter_type": "peak", "frequency": 100.0, "q": 1.5, "gain_db": -3.0}
        ]"#,
    )
    .unwrap();
    let filters = parse_eq_filters_from_json(&json);
    assert_eq!(filters.len(), 1);
    assert_eq!(filters[0].frequency, 100.0);
    assert_eq!(filters[0].q, 1.5);
    assert_eq!(filters[0].gain_db, -3.0);
}

#[test]
fn test_parse_filters_all_filter_types() {
    let json: Vec<serde_json::Value> = serde_json::from_str(
        r#"[
            {"filter_type": "peak", "freq": 100.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "pk", "freq": 200.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "lowshelf", "freq": 300.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "ls", "freq": 400.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "highshelf", "freq": 500.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "hs", "freq": 600.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "lowpass", "freq": 700.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "lp", "freq": 800.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "highpass", "freq": 900.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "hp", "freq": 1000.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "notch", "freq": 1100.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "unknown_type", "freq": 1200.0, "q": 1.0, "db_gain": 0.0}
        ]"#,
    )
    .unwrap();
    let filters = parse_eq_filters_from_json(&json);
    assert_eq!(filters.len(), 12);
    assert_eq!(filters[0].filter_type, BiquadFilterType::Peak);
    assert_eq!(filters[1].filter_type, BiquadFilterType::Peak);
    assert_eq!(filters[2].filter_type, BiquadFilterType::Lowshelf);
    assert_eq!(filters[3].filter_type, BiquadFilterType::Lowshelf);
    assert_eq!(filters[4].filter_type, BiquadFilterType::Highshelf);
    assert_eq!(filters[5].filter_type, BiquadFilterType::Highshelf);
    assert_eq!(filters[6].filter_type, BiquadFilterType::Lowpass);
    assert_eq!(filters[7].filter_type, BiquadFilterType::Lowpass);
    assert_eq!(filters[8].filter_type, BiquadFilterType::Highpass);
    assert_eq!(filters[9].filter_type, BiquadFilterType::Highpass);
    assert_eq!(filters[10].filter_type, BiquadFilterType::Notch);
    assert_eq!(filters[11].filter_type, BiquadFilterType::Peak); // unknown → Peak
}

#[test]
fn test_parse_filters_missing_fields_use_defaults() {
    let json: Vec<serde_json::Value> = serde_json::from_str(
        r#"[
            {"filter_type": "peak"}
        ]"#,
    )
    .unwrap();
    let filters = parse_eq_filters_from_json(&json);
    assert_eq!(filters.len(), 1);
    assert_eq!(filters[0].frequency, 1000.0);
    assert_eq!(filters[0].q, 1.0);
    assert_eq!(filters[0].gain_db, 0.0);
}

#[test]
fn test_parse_filters_empty_array() {
    let json: Vec<serde_json::Value> = Vec::new();
    let filters = parse_eq_filters_from_json(&json);
    assert!(filters.is_empty());
}

#[test]
fn test_parse_filters_warped_topology_preserved() {
    use sotf_audio::plugins::eq::EqFilterTopology;

    let json: Vec<serde_json::Value> = serde_json::from_str(
        r#"[
            {
                "filter_type": "peak",
                "freq": 80.0,
                "q": 2.0,
                "db_gain": -4.0,
                "topology": "warped_biquad",
                "lambda": 0.5
            }
        ]"#,
    )
    .unwrap();
    let filters = parse_eq_filters_from_json(&json);
    assert_eq!(filters.len(), 1);
    let f = &filters[0];
    assert!(matches!(f.topology, EqFilterTopology::WarpedBiquad));
    assert_eq!(f.lambda, Some(0.5));
    assert_eq!(f.frequency, 80.0);
    assert_eq!(f.gain_db, -4.0);
}

#[test]
fn test_parse_filters_kautz_topology_preserved() {
    use sotf_audio::plugins::eq::EqFilterTopology;

    let json: Vec<serde_json::Value> = serde_json::from_str(
        r#"[
            {
                "filter_type": "peak",
                "freq": 100.0,
                "q": 1.0,
                "db_gain": 0.0,
                "topology": "kautz_filter",
                "kautz_sections": [
                    {"pole_freq": 45.0, "q": 12.0, "gain": -3.0},
                    {"pole_freq": 80.0, "q": 8.0, "gain": -2.0}
                ]
            }
        ]"#,
    )
    .unwrap();
    let filters = parse_eq_filters_from_json(&json);
    assert_eq!(filters.len(), 1);
    let f = &filters[0];
    assert!(matches!(f.topology, EqFilterTopology::KautzFilter));
    assert_eq!(f.kautz_sections.len(), 2);
    assert_eq!(f.kautz_sections[0].pole_freq, 45.0);
    assert_eq!(f.kautz_sections[1].q, 8.0);
}

#[test]
fn test_parse_filters_biquad_default_when_topology_missing() {
    use sotf_audio::plugins::eq::EqFilterTopology;

    let json: Vec<serde_json::Value> = serde_json::from_str(
        r#"[{"filter_type": "peak", "freq": 1000.0, "q": 1.0, "db_gain": 0.0}]"#,
    )
    .unwrap();
    let filters = parse_eq_filters_from_json(&json);
    assert_eq!(filters.len(), 1);
    assert!(matches!(filters[0].topology, EqFilterTopology::Biquad));
    assert!(filters[0].lambda.is_none());
    assert!(filters[0].kautz_sections.is_empty());
}

#[test]
fn test_build_room_eq_graph_emits_route_dsp_and_output_correction() {
    let output = routed_bass_output();
    let graph = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
    let labels = collect_labels(&graph);
    assert_eq!(
        labels
            .iter()
            .filter(|l| l.starts_with("room_eq_route_"))
            .count(),
        2
    );
    assert_eq!(
        labels
            .iter()
            .filter(|l| l.as_str() == "pre_room_eq")
            .count(),
        1
    );
    assert_eq!(
        labels
            .iter()
            .filter(|l| l.as_str() == "post_room_eq")
            .count(),
        1
    );
    assert_eq!(
        labels
            .iter()
            .filter(|l| l.as_str() == "sub_post_room_eq")
            .count(),
        1
    );
    assert!(labels.iter().any(|l| l == "room_eq_output_sum_1"));
    assert_room_eq_matrix_nodes_have_width(&graph, 2);
}

#[test]
fn test_build_room_eq_graph_applies_shared_sub_chain_to_physical_sub_routes() {
    let output = routed_physical_sub_output();
    let graph = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
    let labels = collect_labels(&graph);
    assert!(labels.iter().any(|l| l == "room_eq_output_sum_2"));
    assert!(!labels.iter().any(|l| l == "room_eq_output_sum_1"));
    assert_eq!(
        labels
            .iter()
            .filter(|l| l.as_str() == "post_room_eq")
            .count(),
        1
    );
    assert_eq!(
        labels
            .iter()
            .filter(|l| l.as_str() == "sub_post_room_eq")
            .count(),
        1
    );
    assert_room_eq_matrix_nodes_have_width(&graph, 3);
}

#[test]
fn test_physical_graph_shares_inputs_not_distinct_routes() {
    let graph = build_room_eq_plugin_graph_config(&routed_bass_output(), 48_000.0).unwrap();
    let labels = collect_labels(&graph);
    assert_eq!(
        labels
            .iter()
            .filter(|l| l.as_str() == "room_eq_input_0")
            .count(),
        1
    );
    for label in [
        "room_eq_route_0",
        "room_eq_route_1",
        "room_eq_output_sum_0",
        "room_eq_output_sum_1",
        "room_eq_physical_outputs",
    ] {
        assert_eq!(
            labels.iter().filter(|l| l.as_str() == label).count(),
            1,
            "{label}"
        );
    }
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|n| n.plugin_type == "crossover")
            .count(),
        2
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|n| n.plugin_type == "delay")
            .count(),
        2
    );
}

#[test]
fn test_physical_graph_keeps_added_distinct_route() {
    let mut output = routed_physical_sub_output();
    let before = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
    let routes = &mut output
        .metadata
        .as_mut()
        .unwrap()
        .bass_management
        .as_mut()
        .unwrap()
        .routing_graph
        .as_mut()
        .unwrap()
        .routes;
    let mut distinct = routes[1].clone();
    distinct.delay_ms += 1.0;
    routes.push(distinct);
    let after = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
    let count = |g: &sotf_audio::engine::PluginGraphConfig| {
        collect_labels(g)
            .iter()
            .filter(|l| l.starts_with("room_eq_route_"))
            .count()
    };
    assert_eq!(count(&before), 2);
    assert_eq!(count(&after), 3);
}

// =========================================================================
// RoomEqOptimizerConfig schema / version compatibility tests (QA-CORE-001)
// =========================================================================

#[test]
fn room_eq_optimizer_config_serde_roundtrip() {
    let config = RoomEqOptimizerConfig {
        target_response: TargetResponseUiConfig {
            enabled: true,
            shape: "from_measurement".into(),
            slope_db_per_octave: -0.5,
            reference_freq: 1000.0,
            curve_path: None,
            bass_shelf_db: 1.0,
            bass_shelf_freq: 120.0,
            treble_shelf_db: -0.5,
            treble_shelf_freq: 9000.0,
            broadband_precorrection: true,
        },
        ..Default::default()
    };

    let json = serde_json::to_string(&config).unwrap();
    let decoded: RoomEqOptimizerConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.target_response.shape, config.target_response.shape);
    assert_eq!(
        decoded.target_response.broadband_precorrection,
        config.target_response.broadband_precorrection
    );
    assert_eq!(decoded.num_filters, config.num_filters);
}

#[test]
fn room_eq_optimizer_config_ignores_unknown_legacy_fields() {
    // 0.5.121 and earlier persisted `target_tilt` and `broadband_target_matching`.
    // Those fields were removed in 0.5.122 in favour of `target_response`.
    // This test documents that such legacy configs still deserialize (unknown
    // fields are ignored) and `target_response` falls back to defaults.
    let json = r#"{
        "multi_speaker_mode": "Combined",
        "algorithm": "autoeq:cmaes",
        "num_filters": 7,
        "min_q": 0.5,
        "max_q": 6.0,
        "min_db": -12.0,
        "max_db": 4.0,
        "min_freq": 20.0,
        "max_freq": 1600.0,
        "max_iter": 50000,
        "peq_model": "pk",
        "population": 300,
        "refine": false,
        "local_algo": "cobyla",
        "loss_type": "flat",
        "psychoacoustic": true,
        "asymmetric_loss": true,
        "target_curve": "flat",
        "system_type": "stereo",
        "target_tilt": {"slope_db_per_octave": -0.8},
        "broadband_target_matching": {"enabled": true},
        "future_unknown_field": "ignored"
    }"#;

    let config: RoomEqOptimizerConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.target_response.shape, "harman");
    assert!(config.target_response.enabled);
    assert!(!config.target_response.broadband_precorrection);
}

#[test]
fn room_eq_optimizer_config_missing_optional_fields_use_defaults() {
    // Only the fields without #[serde(default)] are required.
    let json = r#"{
        "multi_speaker_mode": "Combined",
        "algorithm": "autoeq:cmaes",
        "num_filters": 7,
        "min_q": 0.5,
        "max_q": 6.0,
        "min_db": -12.0,
        "max_db": 4.0,
        "min_freq": 20.0,
        "max_freq": 1600.0,
        "max_iter": 50000,
        "peq_model": "pk",
        "population": 300,
        "refine": false,
        "local_algo": "cobyla",
        "loss_type": "flat",
        "psychoacoustic": true,
        "asymmetric_loss": true,
        "target_curve": "flat",
        "system_type": "stereo"
    }"#;

    let config: RoomEqOptimizerConfig = serde_json::from_str(json).unwrap();
    assert_eq!(
        config.mode,
        super::room_eq_optimization_mode::RoomEqOptimizationMode::Iir
    );
    assert!(!config.multi_measurement.enabled);
    assert!(!config.excursion_protection.enabled);
    assert!(!config.schroeder_split.enabled);
    assert!(!config.phase_alignment.enabled);
    assert!(!config.multi_seat.enabled);
    assert_eq!(config.bo_initial_samples, 0);
    assert_eq!(config.bo_acquisition, "qei");
}
