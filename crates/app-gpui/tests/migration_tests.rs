use sotf_audio_player_gpui::RoomEqMeasurementsFile;

#[test]
fn test_migration_v1_to_v2() {
    // V1 JSON (missing version)
    let v1_json = r#"{
        "channels": [
            {
                "channel_name": "L",
                "measurement": {
                    "channel": 0,
                    "wav_path": "test.wav",
                    "csv_path": null,
                    "frequencies": [],
                    "magnitude_db": [],
                    "phase_deg": [],
                    "impulse_response": null,
                    "impulse_time_ms": null,
                    "thd_percent": null,
                    "harmonic_distortion_db": null,
                    "excess_group_delay_ms": null,
                    "rt60_ms": null,
                    "clarity_c50_db": null,
                    "clarity_c80_db": null,
                    "spectrogram_db": null
                },
                "is_group": false,
                "group_drivers": []
            }
        ],
        "configuration": null
    }"#;

    let result = RoomEqMeasurementsFile::from_json_str(v1_json).expect("Migration failed");

    assert_eq!(result.version, 2);
    assert_eq!(result.channels.len(), 1);
    assert_eq!(result.channels[0].channel_name, "L");
}

#[test]
fn test_load_v2() {
    // V2 JSON (with version)
    let v2_json = r#"{
        "version": 2,
        "channels": [],
        "configuration": null
    }"#;

    let result = RoomEqMeasurementsFile::from_json_str(v2_json).expect("Loading V2 failed");
    assert_eq!(result.version, 2);
}

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
        if let autoeq::MeasurementSource::Single(ref_) = source {
            let inline = ref_.inline_data().expect("Expected inline data");
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
    assert_eq!(rec_cfg.playback_device_name, Some("Test Device".to_string()));
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
        SpeakerConfig::Single(MeasurementSource::Single(MeasurementRef::Inline(
            InlineMeasurement {
                frequencies: vec![100.0, 1000.0, 10000.0],
                magnitude_db: vec![-5.0, 0.0, -3.0],
                phase_deg: Some(vec![0.0, 45.0, 90.0]),
                name: Some("L".to_string()),
                wav_path: Some("recording_L.wav".to_string()),
                csv_path: Some("recording_L.csv".to_string()),
            },
        ))),
    );

    let room_config = RoomConfig {
        version: "1.1.0".to_string(),
        speakers,
        crossovers: None,
        target_curve: None,
        group_delay: None,
        optimizer: OptimizerConfig::default(),
        recording_config: Some(RecordingConfiguration {
            playback_device_name: Some("Test Device".to_string()),
            signal_type: Some("Sweep".to_string()),
            ..Default::default()
        }),
    };

    // Serialize
    let json = serde_json::to_string_pretty(&room_config).expect("Failed to serialize");

    // Deserialize
    let parsed: RoomConfig = serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(parsed.version, "1.1.0");
    assert_eq!(parsed.speakers.len(), 1);
    assert!(parsed.recording_config.is_some());
}
