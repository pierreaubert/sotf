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
