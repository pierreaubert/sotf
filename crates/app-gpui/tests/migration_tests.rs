use sotf_audio_player_gpui::{check_needs_migration, sanitize_filename};

#[test]
fn test_load_new_room_config_format() {
    // New RoomConfig format (autoeq format)
    let room_config_json = r#"{
        "version": "1.1.0",
        "speakers": {
            "L": {
                "frequencies": [100.0, 1000.0, 10000.0],
                "magnitude_db": [-5.0, 0.0, -3.0],
                "phase_deg": [0.0, 45.0, 90.0],
                "name": "L",
                "wav_path": "recording_L.wav",
                "csv_path": "recording_L.csv"
            },
            "R": {
                "frequencies": [100.0, 1000.0, 10000.0],
                "magnitude_db": [-4.0, 0.5, -2.5],
                "phase_deg": [5.0, 50.0, 95.0],
                "name": "R",
                "wav_path": "recording_R.wav",
                "csv_path": "recording_R.csv"
            }
        },
        "optimizer": {},
        "recording_config": {
            "playback_device_name": "Test Device",
            "signal_type": "Sweep"
        }
    }"#;

    // Should parse as RoomConfig
    let room_config: autoeq::RoomConfig =
        serde_json::from_str(room_config_json).expect("Failed to parse RoomConfig");

    assert_eq!(room_config.version, "1.1.0");
    assert_eq!(room_config.speakers.len(), 2);
    assert!(room_config.speakers.contains_key("L"));
    assert!(room_config.speakers.contains_key("R"));

    // Verify inline measurement data
    if let autoeq::SpeakerConfig::Single(source) = &room_config.speakers["L"] {
        if let autoeq::MeasurementSource::Single(s) = source {
            let inline = s.measurement.inline_data().expect("Expected inline data");
            assert_eq!(inline.frequencies.len(), 3);
            assert_eq!(inline.magnitude_db.len(), 3);
            assert_eq!(inline.name, Some("L".to_string()));
        } else {
            panic!("Expected Single measurement source");
        }
    } else {
        panic!("Expected Single speaker config");
    }

    // Verify recording config
    assert!(room_config.recording_config.is_some());
    let rec_cfg = room_config.recording_config.as_ref().unwrap();
    assert_eq!(
        rec_cfg.playback_device_name,
        Some("Test Device".to_string())
    );
    assert_eq!(rec_cfg.signal_type, Some("Sweep".to_string()));
}

#[test]
fn test_room_config_roundtrip() {
    use autoeq::{
        InlineMeasurement, MeasurementRef, MeasurementSource, OptimizerConfig,
        RecordingConfiguration, RoomConfig, SpeakerConfig,
    };
    use std::collections::HashMap;

    // Create a RoomConfig programmatically
    let mut speakers = HashMap::new();
    speakers.insert(
        "L".to_string(),
        SpeakerConfig::Single(MeasurementSource::Single(autoeq::read::MeasurementSingle {
            measurement: MeasurementRef::Inline(InlineMeasurement {
                frequencies: vec![100.0, 1000.0, 10000.0],
                magnitude_db: vec![-5.0, 0.0, -3.0],
                phase_deg: Some(vec![0.0, 45.0, 90.0]),
                name: Some("L".to_string()),
                wav_path: Some("recording_L.wav".to_string()),
                csv_path: Some("recording_L.csv".to_string()),
            }),
            speaker_name: None,
        })),
    );

    let room_config = RoomConfig {
        version: "1.1.0".to_string(),
        system: None,
        speakers,
        crossovers: None,
        target_curve: None,
        optimizer: OptimizerConfig::default(),
        provenance: Default::default(),
        recording_config: Some(RecordingConfiguration {
            playback_device_name: Some("Test Device".to_string()),
            signal_type: Some("Sweep".to_string()),
            ..Default::default()
        }),
        ctc: None,
        cea2034_cache: None,
    };

    // Serialize
    let json = serde_json::to_string_pretty(&room_config).expect("Failed to serialize");

    // Deserialize
    let parsed: RoomConfig = serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(parsed.version, "1.1.0");
    assert_eq!(parsed.speakers.len(), 1);
    assert!(parsed.recording_config.is_some());
}

// ============================================================================
// Data Migration Check Tests (from components/migration/mod.rs)
// ============================================================================

#[test]
fn check_needs_migration_is_a_no_op_after_legacy_removal() {
    // The legacy `RoomEqMeasurementsFile` schema has been removed; the
    // helper exists as a stub that always reports "no migration needed"
    // so callers route straight to the autoeq RoomConfig parser.
    let frequencies: Vec<f32> = (0..200).map(|i| i as f32 * 100.0).collect();
    let big_legacy_blob = format!(
        r#"{{"channels": [{{"measurement": {{"frequencies": {:?}}}}}]}}"#,
        frequencies
    );
    assert!(!check_needs_migration(&big_legacy_blob, 2_000_000));
    assert!(!check_needs_migration(r#"{"channels": []}"#, 100));
}

#[test]
fn test_sanitize_filename() {
    assert_eq!(sanitize_filename("L"), "L");
    assert_eq!(sanitize_filename("Front Left"), "Front_Left");
    assert_eq!(sanitize_filename("Ch/1"), "Ch_1");
    assert_eq!(sanitize_filename("test-123_abc"), "test-123_abc");
}
