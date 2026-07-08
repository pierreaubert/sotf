use super::channel_recording::ChannelRecording;
use super::recording_device_config::RecordingDeviceConfig;
use super::types::MicrophonePreset;
use super::types::MicrophonePresetsConfig;

#[test]
fn test_calibration_for_channel_returns_none_for_out_of_bounds() {
    let config = RecordingDeviceConfig::default();
    assert!(config.calibration_for_channel(5).is_none());
}

#[test]
fn test_calibration_for_channel_returns_path() {
    let config = RecordingDeviceConfig {
        mic_calibration_paths: vec![Some("/path/to/cal.txt".to_string())],
        ..Default::default()
    };
    assert_eq!(config.calibration_for_channel(0), Some("/path/to/cal.txt"));
}

#[test]
fn test_calibration_for_channel_returns_none_for_none_entry() {
    let config = RecordingDeviceConfig {
        mic_calibration_paths: vec![None, Some("/path.txt".to_string())],
        ..Default::default()
    };
    assert!(config.calibration_for_channel(0).is_none());
    assert_eq!(config.calibration_for_channel(1), Some("/path.txt"));
}

#[test]
fn test_set_calibration_grows_vec_beyond_channel_mappings() {
    let mut config = RecordingDeviceConfig::default();
    // Default has 1 channel_mapping, set calibration for channel 3
    config.set_calibration_for_channel(3, Some("/path.txt".to_string()));
    assert_eq!(config.mic_calibration_paths.len(), 4);
    assert_eq!(config.calibration_for_channel(3), Some("/path.txt"));
    // Intermediate entries should be None
    assert!(config.calibration_for_channel(1).is_none());
    assert!(config.calibration_for_channel(2).is_none());
}

#[test]
fn test_set_calibration_overwrites_existing() {
    let mut config = RecordingDeviceConfig::default();
    config.set_calibration_for_channel(0, Some("/old.txt".to_string()));
    config.set_calibration_for_channel(0, Some("/new.txt".to_string()));
    assert_eq!(config.calibration_for_channel(0), Some("/new.txt"));
}

#[test]
fn test_set_calibration_clear() {
    let mut config = RecordingDeviceConfig::default();
    config.set_calibration_for_channel(0, Some("/path.txt".to_string()));
    config.set_calibration_for_channel(0, None);
    assert!(config.calibration_for_channel(0).is_none());
}

#[test]
fn test_sync_calibration_paths_pads_to_channel_mappings() {
    let mut config = RecordingDeviceConfig {
        channel_mappings: vec![0, 1, 2],
        mic_calibration_paths: vec![Some("/path.txt".to_string())],
        ..Default::default()
    };
    config.sync_calibration_paths();
    assert_eq!(config.mic_calibration_paths.len(), 3);
    assert_eq!(config.calibration_for_channel(0), Some("/path.txt"));
    assert!(config.calibration_for_channel(1).is_none());
    assert!(config.calibration_for_channel(2).is_none());
}

#[test]
fn test_microphone_preset_serde_roundtrip() {
    let preset = MicrophonePreset {
        name: "UMIK-1".to_string(),
        device_name: "UMIK-1 USB".to_string(),
        channel_mappings: vec![0, 1],
        mic_calibration_paths: vec![
            Some("/cal/ch0.txt".to_string()),
            Some("/cal/ch1.txt".to_string()),
        ],
    };
    let json = serde_json::to_string(&preset).unwrap();
    let deserialized: MicrophonePreset = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name, "UMIK-1");
    assert_eq!(deserialized.mic_calibration_paths.len(), 2);
}

#[test]
fn test_presets_config_serde_roundtrip() {
    let config = MicrophonePresetsConfig {
        presets: vec![MicrophonePreset {
            name: "Test".to_string(),
            device_name: "Device".to_string(),
            channel_mappings: vec![0],
            mic_calibration_paths: vec![None],
        }],
    };
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: MicrophonePresetsConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.presets.len(), 1);
}

#[test]
fn test_channel_recording_lfe_detection() {
    // Exact match
    let rec = ChannelRecording::new(0, "LFE".to_string());
    assert_eq!(rec.sweep_start_freq, 10.0);
    assert_eq!(rec.sweep_end_freq, 500.0);

    // Case-insensitive
    let rec = ChannelRecording::new(0, "lfe".to_string());
    assert_eq!(rec.sweep_start_freq, 10.0);
    assert_eq!(rec.sweep_end_freq, 500.0);

    // "Sub" variant
    let rec = ChannelRecording::new(0, "Sub".to_string());
    assert_eq!(rec.sweep_start_freq, 10.0);
    assert_eq!(rec.sweep_end_freq, 500.0);

    // Non-LFE channel
    let rec = ChannelRecording::new(0, "L".to_string());
    assert_eq!(rec.sweep_start_freq, 20.0);
    assert_eq!(rec.sweep_end_freq, 20000.0);
}

#[test]
fn test_channel_recording_serde_backward_compat() {
    // Old format without sweep freq fields should deserialize with defaults
    let json = r#"{
            "channel_index": 0,
            "channel_name": "L",
            "state": "Empty",
            "result": null
        }"#;
    let rec: ChannelRecording = serde_json::from_str(json).unwrap();
    assert_eq!(rec.sweep_start_freq, 20.0);
    assert_eq!(rec.sweep_end_freq, 20000.0);
}

#[test]
fn test_recording_device_config_backward_compat_deserialization() {
    // Old format without mic_calibration_paths field
    let json = r#"{
            "device_id": "test",
            "device_name": "Test Device",
            "num_channels": 1,
            "sample_rate": 48000,
            "available_sample_rates": [48000],
            "channel_mappings": [0]
        }"#;
    let config: RecordingDeviceConfig = serde_json::from_str(json).unwrap();
    assert!(config.mic_calibration_paths.is_empty());
    assert!(config.calibration_for_channel(0).is_none());
}

/// In multi-mic mode, channel names get a " (Mic N)" suffix.
/// The LFE/Sub detection must still work so those channels get
/// the narrow 10-500 Hz sweep range, not the default 20-20000 Hz.
#[test]
fn test_lfe_sweep_bounds_with_mic_suffix() {
    // Single-mic: plain name → LFE detection works
    let single = ChannelRecording::new(0, "LFE".to_string());
    assert_eq!(single.sweep_start_freq, 10.0, "single-mic LFE start");
    assert_eq!(single.sweep_end_freq, 500.0, "single-mic LFE end");

    let single_sub = ChannelRecording::new(0, "Sub".to_string());
    assert_eq!(single_sub.sweep_start_freq, 10.0, "single-mic Sub start");
    assert_eq!(single_sub.sweep_end_freq, 500.0, "single-mic Sub end");

    // Multi-mic: name has " (Mic N)" suffix → LFE detection must still work
    let multi_lfe = ChannelRecording::with_mic(0, "LFE (Mic 1)".to_string(), 0);
    assert_eq!(multi_lfe.sweep_start_freq, 10.0, "multi-mic LFE start");
    assert_eq!(multi_lfe.sweep_end_freq, 500.0, "multi-mic LFE end");

    let multi_sub = ChannelRecording::with_mic(0, "Sub (Mic 2)".to_string(), 1);
    assert_eq!(multi_sub.sweep_start_freq, 10.0, "multi-mic Sub start");
    assert_eq!(multi_sub.sweep_end_freq, 500.0, "multi-mic Sub end");

    // Non-LFE channels must still get full range
    let multi_l = ChannelRecording::with_mic(0, "L (Mic 1)".to_string(), 0);
    assert_eq!(multi_l.sweep_start_freq, 20.0, "multi-mic L start");
    assert_eq!(multi_l.sweep_end_freq, 20000.0, "multi-mic L end");
}

// =========================================================================
// Recording config schema compatibility tests (QA-CORE-001)
// =========================================================================

#[test]
fn microphone_presets_config_ignores_unknown_fields() {
    let json = r#"{
        "presets": [
            {
                "name": "UMIK-1",
                "device_name": "UMIK-1 USB",
                "channel_mappings": [0, 1],
                "mic_calibration_paths": ["/cal/ch0.txt", null],
                "future_field": "ignored"
            }
        ],
        "unknown_nested": {"x": 1}
    }"#;

    let config: MicrophonePresetsConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.presets.len(), 1);
    assert_eq!(config.presets[0].name, "UMIK-1");
}

#[test]
fn microphone_presets_config_serde_roundtrip() {
    let config = MicrophonePresetsConfig {
        presets: vec![MicrophonePreset {
            name: "Test".to_string(),
            device_name: "Device".to_string(),
            channel_mappings: vec![0, 1],
            mic_calibration_paths: vec![Some("/cal/ch0.txt".to_string()), None],
        }],
    };

    let json = serde_json::to_string(&config).unwrap();
    let decoded: MicrophonePresetsConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.presets.len(), config.presets.len());
    assert_eq!(decoded.presets[0].name, config.presets[0].name);
}

#[test]
fn recording_device_config_missing_optional_defaults() {
    // Old format without mic_calibration_paths, num_positions, ctc fields
    let json = r#"{
        "device_id": "test",
        "device_name": "Test Device",
        "num_channels": 1,
        "sample_rate": 48000,
        "available_sample_rates": [48000],
        "channel_mappings": [0]
    }"#;

    let config: RecordingDeviceConfig = serde_json::from_str(json).unwrap();
    assert!(config.mic_calibration_paths.is_empty());
    assert_eq!(config.num_positions, 1);
}

#[test]
fn playback_device_config_serde_roundtrip() {
    use super::playback_device_config::PlaybackDeviceConfig;

    let config = PlaybackDeviceConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    let decoded: PlaybackDeviceConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.device_id, config.device_id);
    assert_eq!(decoded.num_channels, config.num_channels);
    assert_eq!(decoded.sample_rate, config.sample_rate);
}
