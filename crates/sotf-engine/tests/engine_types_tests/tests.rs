use sotf_audio::engine::{
    AudioEngineState, AudioFrame, DecoderCommand, DecoderMessage, PlaybackCommand, PlaybackState,
    PluginConfig, ProcessingMessage, ThreadEvent,
};
use sotf_types::{
    IsolatedExternalPluginSandboxBackend, IsolatedExternalPluginSandboxStatus,
    IsolatedExternalPluginWorkerEvent, IsolatedExternalPluginWorkerStatus,
};
use std::path::PathBuf;

#[test]
fn test_audio_frame_creation() {
    let data = vec![0.5, -0.5, 0.3, -0.3]; // 2 frames, 2 channels
    let frame = AudioFrame::new(data.clone(), 2, 2, 48000);

    assert_eq!(frame.num_frames, 2);
    assert_eq!(frame.num_channels, 2);
    assert_eq!(frame.sample_rate, 48000);
    assert_eq!(frame.data, data);
}

#[test]
fn test_audio_frame_try_new_reports_overflow() {
    let err = AudioFrame::try_new(Vec::new(), usize::MAX, 2, 48000).unwrap_err();
    assert!(err.contains("overflow usize"), "{err}");
}

#[test]
fn test_audio_frame_try_silent_reports_overflow() {
    let err = AudioFrame::try_silent(usize::MAX, 2, 48000).unwrap_err();
    assert!(err.contains("overflow usize"), "{err}");
}

#[test]
fn test_audio_frame_num_samples() {
    let frame = AudioFrame::new(vec![0.0; 100], 50, 2, 48000);
    assert_eq!(frame.num_samples(), 100);

    let mono_frame = AudioFrame::new(vec![0.0; 100], 100, 1, 48000);
    assert_eq!(mono_frame.num_samples(), 100);

    let surround_frame = AudioFrame::new(vec![0.0; 600], 100, 6, 48000);
    assert_eq!(surround_frame.num_samples(), 600);
}

#[test]
fn test_audio_frame_silent() {
    let frame = AudioFrame::silent(512, 2, 48000);

    assert_eq!(frame.num_frames, 512);
    assert_eq!(frame.num_channels, 2);
    assert_eq!(frame.sample_rate, 48000);
    assert_eq!(frame.data.len(), 1024); // 512 frames * 2 channels

    // All samples should be zero
    for &sample in &frame.data {
        assert_eq!(sample, 0.0);
    }
}

#[test]
fn test_audio_frame_clear() {
    let mut frame = AudioFrame::new(vec![1.0, 0.5, -0.5, -1.0], 2, 2, 48000);

    // Verify non-zero initially
    assert!(frame.data.iter().any(|&s| s != 0.0));

    frame.clear();

    // All samples should be zero after clear
    for &sample in &frame.data {
        assert_eq!(sample, 0.0);
    }
}

#[test]
fn test_audio_frame_clone() {
    let original = AudioFrame::new(vec![0.1, 0.2, 0.3, 0.4], 2, 2, 48000);
    let cloned = original.clone();

    assert_eq!(original.data, cloned.data);
    assert_eq!(original.num_frames, cloned.num_frames);
    assert_eq!(original.num_channels, cloned.num_channels);
    assert_eq!(original.sample_rate, cloned.sample_rate);
}

#[test]
fn test_audio_frame_various_configurations() {
    // Mono
    let mono = AudioFrame::silent(256, 1, 44100);
    assert_eq!(mono.num_samples(), 256);

    // Stereo
    let stereo = AudioFrame::silent(256, 2, 48000);
    assert_eq!(stereo.num_samples(), 512);

    // 5.1 Surround
    let surround = AudioFrame::silent(256, 6, 48000);
    assert_eq!(surround.num_samples(), 1536);

    // 7.1 Surround
    let surround71 = AudioFrame::silent(256, 8, 96000);
    assert_eq!(surround71.num_samples(), 2048);
}

#[test]
fn test_decoder_message_frame() {
    let frame = AudioFrame::silent(512, 2, 48000);
    let msg = DecoderMessage::Frame(frame.clone());

    if let DecoderMessage::Frame(f) = msg {
        assert_eq!(f.num_frames, 512);
        assert_eq!(f.num_channels, 2);
    } else {
        panic!("Expected Frame message");
    }
}

#[test]
fn test_decoder_message_end_of_stream() {
    let msg = DecoderMessage::EndOfStream;

    assert!(matches!(msg, DecoderMessage::EndOfStream));
}

#[test]
fn test_decoder_message_flush() {
    let msg = DecoderMessage::Flush;

    assert!(matches!(msg, DecoderMessage::Flush));
}

#[test]
fn test_decoder_message_clone() {
    let frame = AudioFrame::silent(256, 2, 48000);
    let msg = DecoderMessage::Frame(frame);
    let cloned = msg.clone();

    if let (DecoderMessage::Frame(f1), DecoderMessage::Frame(f2)) = (msg, cloned) {
        assert_eq!(f1.num_frames, f2.num_frames);
    }
}

#[test]
fn test_processing_message_frame() {
    let frame = AudioFrame::silent(512, 2, 48000);
    let msg = ProcessingMessage::Frame(frame);

    assert!(matches!(msg, ProcessingMessage::Frame(_)));
}

#[test]
fn test_processing_message_end_of_stream() {
    let msg = ProcessingMessage::EndOfStream;

    assert!(matches!(msg, ProcessingMessage::EndOfStream));
}

#[test]
fn test_processing_message_flush() {
    let msg = ProcessingMessage::Flush;

    assert!(matches!(msg, ProcessingMessage::Flush));
}

#[test]
fn test_decoder_command_play() {
    let path = PathBuf::from("/path/to/file.flac");
    let source = sotf_audio::decoder::AudioSource::File(path.clone());
    let cmd = DecoderCommand::Play(source);

    if let DecoderCommand::Play(s) = cmd {
        assert_eq!(s.as_path().unwrap(), path.as_path());
    } else {
        panic!("Expected Play command");
    }
}

#[test]
fn test_decoder_command_seek() {
    let cmd = DecoderCommand::Seek(30.5);

    if let DecoderCommand::Seek(pos) = cmd {
        assert!((pos - 30.5).abs() < 0.001);
    } else {
        panic!("Expected Seek command");
    }
}

#[test]
fn test_decoder_command_variants() {
    let commands = vec![
        DecoderCommand::Pause,
        DecoderCommand::Resume,
        DecoderCommand::Stop,
        DecoderCommand::Shutdown,
        DecoderCommand::StartSilentSource(6),
    ];

    for cmd in commands {
        // Just verify they can be created and matched
        match cmd {
            DecoderCommand::Pause => {}
            DecoderCommand::Resume => {}
            DecoderCommand::Stop => {}
            DecoderCommand::Shutdown => {}
            DecoderCommand::StartSilentSource(channels) => {
                assert_eq!(channels, 6);
            }
            _ => {}
        }
    }
}

#[test]
fn test_playback_command_set_volume() {
    let cmd = PlaybackCommand::SetVolume(0.75);

    if let PlaybackCommand::SetVolume(vol) = cmd {
        assert!((vol - 0.75).abs() < 0.001);
    } else {
        panic!("Expected SetVolume command");
    }
}

#[test]
fn test_playback_command_mute() {
    let mute_on = PlaybackCommand::Mute(true);
    let mute_off = PlaybackCommand::Mute(false);

    assert!(matches!(mute_on, PlaybackCommand::Mute(true)));
    assert!(matches!(mute_off, PlaybackCommand::Mute(false)));
}

#[test]
fn test_playback_command_update_channels() {
    let cmd = PlaybackCommand::UpdateChannels(6);

    if let PlaybackCommand::UpdateChannels(ch) = cmd {
        assert_eq!(ch, 6);
    } else {
        panic!("Expected UpdateChannels command");
    }
}

#[test]
fn test_playback_state_equality() {
    assert_eq!(PlaybackState::Stopped, PlaybackState::Stopped);
    assert_eq!(PlaybackState::Playing, PlaybackState::Playing);
    assert_eq!(PlaybackState::Paused, PlaybackState::Paused);

    assert_ne!(PlaybackState::Stopped, PlaybackState::Playing);
    assert_ne!(PlaybackState::Playing, PlaybackState::Paused);
}

#[test]
fn test_playback_state_serialization() {
    let states = [
        PlaybackState::Stopped,
        PlaybackState::Playing,
        PlaybackState::Paused,
    ];

    for state in states {
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: PlaybackState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }
}

#[test]
fn test_audio_engine_state_default() {
    let state = AudioEngineState::default();

    assert_eq!(state.playback_state, PlaybackState::Stopped);
    assert_eq!(state.current_file, None);
    assert_eq!(state.position, 0.0);
    assert_eq!(state.duration, None);
    assert_eq!(state.sample_rate, 48000);
    assert_eq!(state.num_channels, 2);
    assert_eq!(state.volume, 1.0);
    assert!(!state.muted);
    assert!(!state.processing_bypassed);
    assert_eq!(state.underruns, 0);
    assert_eq!(state.playback_output_device, None);
    assert_eq!(state.playback_callback_count, 0);
    assert_eq!(state.playback_buffer_fill_percent, 0);
    assert_eq!(state.playback_stream_error_count, 0);
    assert_eq!(state.playback_frames_received, 0);
    assert_eq!(state.playback_frames_written, 0);
    assert_eq!(state.playback_frames_dropped, 0);
    assert_eq!(state.playback_effective_sample_rate, 0);
    assert_eq!(state.last_error, None);
    assert!(!state.seeking);
    assert!(state.isolated_external_plugin_worker_statuses.is_empty());
}

#[test]
fn test_audio_engine_state_serialization() {
    let mut state = AudioEngineState::default();
    state.playback_state = PlaybackState::Playing;
    state.current_file = Some(PathBuf::from("/path/to/file.flac"));
    state.position = 45.5;
    state.duration = Some(180.0);
    state.volume = 0.8;
    state.muted = true;

    let json = serde_json::to_string(&state).unwrap();
    let deserialized: AudioEngineState = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.playback_state, PlaybackState::Playing);
    assert_eq!(
        deserialized.current_file,
        Some(PathBuf::from("/path/to/file.flac"))
    );
    assert!((deserialized.position - 45.5).abs() < 0.001);
    assert_eq!(deserialized.duration, Some(180.0));
    assert!((deserialized.volume - 0.8).abs() < 0.001);
    assert!(deserialized.muted);
    assert!(
        deserialized
            .isolated_external_plugin_worker_statuses
            .is_empty()
    );
}

#[test]
fn test_audio_engine_state_deserializes_without_worker_statuses_field() {
    let mut state = AudioEngineState::default();
    state.playback_state = PlaybackState::Playing;
    state.current_file = Some(PathBuf::from("/path/to/file.flac"));
    state.position = 12.5;
    state.sample_rate = 44100;
    state.num_channels = 6;
    state.duration = Some(321.0);
    state.last_error = Some("none".to_string());

    let mut value = serde_json::to_value(&state).unwrap();
    value
        .as_object_mut()
        .expect("state json object")
        .remove("isolated_external_plugin_worker_statuses");

    let deserialized: AudioEngineState = serde_json::from_value(value).unwrap();

    assert_eq!(deserialized.playback_state, PlaybackState::Playing);
    assert_eq!(
        deserialized.current_file,
        Some(PathBuf::from("/path/to/file.flac"))
    );
    assert!((deserialized.position - 12.5).abs() < 0.001);
    assert_eq!(deserialized.sample_rate, 44100);
    assert_eq!(deserialized.num_channels, 6);
    assert_eq!(deserialized.duration, Some(321.0));
    assert_eq!(deserialized.last_error.as_deref(), Some("none"));
    assert!(
        deserialized
            .isolated_external_plugin_worker_statuses
            .is_empty()
    );
}

#[test]
fn test_audio_engine_state_clone() {
    let mut state = AudioEngineState::default();
    state.playback_state = PlaybackState::Playing;
    state.position = 30.0;
    state.underruns = 5;

    let cloned = state.clone();

    assert_eq!(cloned.playback_state, state.playback_state);
    assert_eq!(cloned.position, state.position);
    assert_eq!(cloned.underruns, state.underruns);
}

#[test]
fn test_thread_event_decoder_end_of_stream() {
    let event = ThreadEvent::DecoderEndOfStream;

    assert!(matches!(event, ThreadEvent::DecoderEndOfStream));
}

#[test]
fn test_thread_event_decoder_error() {
    let event = ThreadEvent::DecoderError("File not found".to_string());

    if let ThreadEvent::DecoderError(msg) = event {
        assert_eq!(msg, "File not found");
    } else {
        panic!("Expected DecoderError event");
    }
}

#[test]
fn test_thread_event_playback_underrun() {
    let event = ThreadEvent::PlaybackUnderrun(101);

    assert!(matches!(event, ThreadEvent::PlaybackUnderrun(101)));
}

#[test]
fn test_thread_event_playback_stats() {
    let event = ThreadEvent::PlaybackStats {
        callback_count: 12,
        buffer_fill_percent: 75,
        stream_error_count: 1,
        frames_received: 40,
        frames_written: 39,
        frames_dropped: 1,
        effective_sample_rate: 48_000,
    };

    if let ThreadEvent::PlaybackStats {
        callback_count,
        buffer_fill_percent,
        stream_error_count,
        frames_received,
        frames_written,
        frames_dropped,
        effective_sample_rate,
    } = event
    {
        assert_eq!(callback_count, 12);
        assert_eq!(buffer_fill_percent, 75);
        assert_eq!(stream_error_count, 1);
        assert_eq!(frames_received, 40);
        assert_eq!(frames_written, 39);
        assert_eq!(frames_dropped, 1);
        assert_eq!(effective_sample_rate, 48_000);
    } else {
        panic!("Expected PlaybackStats event");
    }
}

#[test]
fn test_thread_event_position_update() {
    let event = ThreadEvent::PositionUpdate(45.5);

    if let ThreadEvent::PositionUpdate(pos) = event {
        assert!((pos - 45.5).abs() < 0.001);
    } else {
        panic!("Expected PositionUpdate event");
    }
}

#[test]
fn test_thread_event_seek_complete() {
    let event = ThreadEvent::SeekComplete;

    assert!(matches!(event, ThreadEvent::SeekComplete));
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn test_thread_event_isolated_external_plugin_worker_statuses() {
    let status = IsolatedExternalPluginWorkerStatus {
        plugin_index: 0,
        node_id: 11,
        plugin_instance_id: None,
        event: Some(IsolatedExternalPluginWorkerEvent::Started { pid: 1234 }),
        error: Some("timeout".to_string()),
        worker_start_count: 1,
        worker_exit_count: 0,
        worker_launch_failure_count: 2,
        block_timeout_count: 3,
        block_worker_failure_count: 4,
        block_wrong_sequence_count: 5,
        sandbox_status: IsolatedExternalPluginSandboxStatus::Unsupported,
        sandbox_backend: IsolatedExternalPluginSandboxBackend::MacosProcessIsolation,
        sandbox_reason: Some("sandbox backend unavailable".to_string()),
    };
    let event = ThreadEvent::IsolatedExternalPluginWorkerStatuses(vec![status.clone()]);

    if let ThreadEvent::IsolatedExternalPluginWorkerStatuses(statuses) = event {
        assert_eq!(statuses.len(), 1);
        assert_eq!(status, statuses[0]);
    } else {
        panic!("Expected IsolatedExternalPluginWorkerStatuses event");
    }
}

#[test]
fn test_plugin_config_creation() {
    let config = PluginConfig::new("volume", serde_json::json!({"gain": 0.5}));

    assert_eq!(config.plugin_type, "volume");
    assert_eq!(config.parameters["gain"], 0.5);
}

#[test]
fn test_plugin_config_serialization() {
    let config = PluginConfig::new(
        "eq",
        serde_json::json!({
            "bands": [
                {"freq": 100, "gain": 3.0, "q": 1.0},
                {"freq": 1000, "gain": -2.0, "q": 2.0}
            ]
        }),
    );

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: PluginConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.plugin_type, "eq");
    assert!(deserialized.parameters["bands"].is_array());
}

#[test]
fn test_plugin_config_clone() {
    let config = PluginConfig::new("limiter", serde_json::json!({"threshold": -1.0}));
    let cloned = config.clone();

    assert_eq!(config.plugin_type, cloned.plugin_type);
    assert_eq!(config.parameters, cloned.parameters);
}

#[test]
fn test_plugin_config_various_types() {
    // Test with different parameter types
    let configs = vec![
        PluginConfig::new("bypass", serde_json::json!({})),
        PluginConfig::new("gain", serde_json::json!({"value": 1.5})),
        PluginConfig::new("delay", serde_json::json!({"ms": 100, "feedback": 0.3})),
        PluginConfig::new(
            "compressor",
            serde_json::json!({
                "threshold": -20.0,
                "ratio": 4.0,
                "attack": 10.0,
                "release": 100.0
            }),
        ),
    ];

    for config in configs {
        // Verify serialization works
        let json = serde_json::to_string(&config).unwrap();
        let _: PluginConfig = serde_json::from_str(&json).unwrap();
    }
}
