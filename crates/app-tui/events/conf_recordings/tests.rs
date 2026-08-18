use super::adjust::adjust_recording_field;
use super::consts::{BASS_ANCHOR_CAPTURE_RESULT, PROBE_CAPTURE_RESULT, RECORDING_RESULT};
use super::handle::handle_recording_keys;
use super::misc::ctc_raw_capture_channel_indices;
use super::misc::init_recording_channels;
use super::misc::save_recordings;
use super::misc::set_recording_field_from_string;
use super::misc::update_channel_mappings_for_config;
use super::poll::{poll_bass_anchor_capture, poll_probe_capture, poll_recording};

use crate::events::tests::make_app;
use sotf_audio_player::recording_types::{
    ChannelMapping, ChannelRecording, ChannelRecordingState, RecordingResult, RecordingStep,
    SpeakerConfiguration,
};
use std::sync::{Arc, Mutex};

#[test]
fn init_recording_channels_creates_channels() {
    let mut app = make_app();
    app.recording.model.playback_config.channel_mappings = vec![
        ChannelMapping::single(0, "FL"),
        ChannelMapping::single(1, "FR"),
    ];
    app.recording.model.playback_config.num_channels = 2;

    init_recording_channels(&mut app);
    assert_eq!(app.recording.model.channel_recordings.len(), 2);
    assert_eq!(app.recording.model.current_recording_channel, Some(0));
    assert_eq!(app.recording.model.channel_recordings[0].channel_name, "FL");
    assert_eq!(app.recording.model.channel_recordings[1].channel_name, "FR");
}

#[test]
fn init_recording_channels_expands_speaker_mic_position_matrix() {
    let mut app = make_app();
    app.recording.model.playback_config.channel_mappings = vec![
        ChannelMapping::single(0, "FL"),
        ChannelMapping::single(1, "FR"),
    ];
    app.recording.model.recording_config.channel_mappings = vec![0, 1];
    app.recording.model.recording_config.num_positions = 2;

    init_recording_channels(&mut app);

    assert_eq!(app.recording.model.channel_recordings.len(), 8);
    assert_eq!(
        app.recording.model.channel_recordings[0].channel_name,
        "FL (Pos 1 / Mic 1)"
    );
    assert_eq!(
        app.recording.model.channel_recordings[1].channel_name,
        "FL (Pos 1 / Mic 2)"
    );
    assert_eq!(
        app.recording.model.channel_recordings[2].channel_name,
        "FR (Pos 1 / Mic 1)"
    );
    assert_eq!(
        app.recording.model.channel_recordings[4].channel_name,
        "FL (Pos 2 / Mic 1)"
    );
    assert_eq!(app.recording.model.channel_recordings[4].channel_index, 0);
    assert_eq!(
        app.recording.model.channel_recordings[4].mic_position_index,
        1
    );
}

#[test]
fn ctc_raw_capture_selects_both_ears_for_same_speaker_position() {
    let mut app = make_app();
    app.recording.model.playback_config.channel_mappings = vec![
        ChannelMapping::single(0, "FL"),
        ChannelMapping::single(1, "FR"),
    ];
    app.recording.model.recording_config.channel_mappings = vec![0, 1];
    app.recording.model.recording_config.num_positions = 2;
    init_recording_channels(&mut app);

    assert_eq!(ctc_raw_capture_channel_indices(&app, 1), vec![0, 1]);
    assert_eq!(ctc_raw_capture_channel_indices(&app, 4), vec![4, 5]);
}

#[test]
fn init_recording_channels_reinits_on_config_change() {
    let mut app = make_app();
    // Start with 2 channels
    app.recording.model.playback_config.channel_mappings = vec![
        ChannelMapping::single(0, "FL"),
        ChannelMapping::single(1, "FR"),
    ];
    init_recording_channels(&mut app);
    assert_eq!(app.recording.model.channel_recordings.len(), 2);

    // Change to 3 channels
    app.recording.model.playback_config.channel_mappings = vec![
        ChannelMapping::single(0, "FL"),
        ChannelMapping::single(1, "FR"),
        ChannelMapping::single(2, "C"),
    ];
    init_recording_channels(&mut app);
    assert_eq!(app.recording.model.channel_recordings.len(), 3);
    assert_eq!(app.recording.model.channel_recordings[2].channel_name, "C");
}

#[test]
fn init_recording_channels_handles_empty_config() {
    let mut app = make_app();
    app.recording.model.playback_config.channel_mappings = vec![];
    init_recording_channels(&mut app);
    assert_eq!(app.recording.model.channel_recordings.len(), 0);
    assert_eq!(app.recording.model.current_recording_channel, None);
}

#[test]
fn save_recordings_rejects_path_separators_in_name() {
    let mut app = make_app();
    app.recording.model.save_name = "../../evil".to_string();
    save_recordings(&mut app);
    assert!(app.recording.save.error.is_some());
    assert!(
        app.recording
            .save
            .error
            .as_ref()
            .unwrap()
            .contains("path separators")
    );
}

#[test]
fn save_recordings_rejects_backslash_in_name() {
    let mut app = make_app();
    app.recording.model.save_name = "foo\\bar".to_string();
    save_recordings(&mut app);
    assert!(app.recording.save.error.is_some());
    assert!(
        app.recording
            .save
            .error
            .as_ref()
            .unwrap()
            .contains("path separators")
    );
}

#[test]
fn save_recordings_requires_completed_channels() {
    let mut app = make_app();
    app.recording.model.save_name = "test".to_string();
    // No completed recordings
    save_recordings(&mut app);
    assert!(app.recording.save.error.is_some());
    assert!(
        app.recording
            .save
            .error
            .as_ref()
            .unwrap()
            .contains("No completed")
    );
}

#[test]
fn recording_step_default_is_config() {
    assert_eq!(RecordingStep::default(), RecordingStep::Config);
}

#[test]
fn update_channel_mappings_creates_correct_channels() {
    let mut app = make_app();
    update_channel_mappings_for_config(&mut app, SpeakerConfiguration::Stereo);
    assert_eq!(app.recording.model.playback_config.num_channels, 2);
    assert_eq!(
        app.recording.model.playback_config.channel_mappings.len(),
        2
    );
}

#[test]
fn adjust_device_populates_config() {
    let mut app = make_app();
    app.recording.available_playback_devices = vec![
        ("id0".to_string(), "Device 0".to_string()),
        ("id1".to_string(), "Device 1".to_string()),
    ];
    app.recording.available_recording_devices = vec![
        ("rid0".to_string(), "Mic 0".to_string()),
        ("rid1".to_string(), "Mic 1".to_string()),
    ];
    app.recording.selected_field = 0;
    adjust_recording_field(&mut app, 1);
    assert_eq!(app.recording.model.playback_config.device_name, "Device 1");
    assert_eq!(app.recording.model.playback_config.device_id, "id1");

    app.recording.selected_field = 1;
    adjust_recording_field(&mut app, 1);
    assert_eq!(app.recording.model.recording_config.device_name, "Mic 1");
    assert_eq!(app.recording.model.recording_config.device_id, "rid1");
}

#[test]
fn tab_on_config_cycles_fields() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    let mut app = make_app();
    app.recording.selected_field = 0;

    let tab_key = KeyEvent {
        code: KeyCode::Tab,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    handle_recording_keys(&mut app, tab_key);
    assert_eq!(app.recording.selected_field, 1);
    assert_eq!(
        app.recording.model.step,
        sotf_audio_player::recording_types::RecordingStep::Config
    );
}

#[test]
fn right_on_config_adjusts_field() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    let mut app = make_app();
    app.recording.selected_field = 4; // signal_duration_secs (numerical)
    let before = app.recording.model.signal_duration_secs;
    let right_key = KeyEvent {
        code: KeyCode::Right,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    handle_recording_keys(&mut app, right_key);
    assert_eq!(app.recording.model.signal_duration_secs, before + 1.0);
    // Should stay on Config step
    assert_eq!(
        app.recording.model.step,
        sotf_audio_player::recording_types::RecordingStep::Config
    );
}

fn done_recording_result(mag_db: f32) -> RecordingResult {
    RecordingResult {
        channel: 0,
        wav_path: None,
        csv_path: Some("FL.csv".to_string()),
        frequencies: vec![100.0, 1000.0, 10000.0],
        magnitude_db: vec![mag_db; 3],
        phase_deg: vec![0.0; 3],
        impulse_response: None,
        impulse_time_ms: None,
        thd_percent: None,
        harmonic_distortion_db: None,
        excess_group_delay_ms: None,
        rt60_ms: None,
        clarity_c50_db: None,
        clarity_c80_db: None,
        spectrogram_db: None,
        quality: None,
    }
}

#[test]
fn level_edit_rejects_positive_db_with_message() {
    // R3: a user-typed level above 0 dBFS must be rejected with a clear
    // message instead of silently clamping into a clipped sweep.
    let mut app = make_app();
    app.recording.selected_field = 5; // Level
    app.recording.edit_buffer = "6.0".to_string();
    let before = app.recording.model.signal_level_db;
    set_recording_field_from_string(&mut app);
    assert_eq!(app.recording.model.signal_level_db, before);
    assert!(
        app.recording.model.status_message.contains("0 dB"),
        "unexpected status: {}",
        app.recording.model.status_message
    );
}

#[test]
fn level_edit_accepts_negative_db() {
    let mut app = make_app();
    app.recording.selected_field = 5; // Level
    app.recording.edit_buffer = "-12.5".to_string();
    set_recording_field_from_string(&mut app);
    assert_eq!(app.recording.model.signal_level_db, -12.5);
}

#[test]
fn poll_probe_capture_treats_cancelled_as_idle() {
    use sotf_audio_player::recording_types::ProbeCaptureStatus;
    let mut app = make_app();
    app.recording.model.probe_capture.status = ProbeCaptureStatus::Running { started_at_ms: 0 };
    let slot = PROBE_CAPTURE_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    *slot.lock().unwrap() = Some(Err(
        sotf_audio::signal_recorder::CANCELLED_ERR.to_string()
    ));
    assert!(poll_probe_capture(&mut app));
    assert!(matches!(
        app.recording.model.probe_capture.status,
        ProbeCaptureStatus::Idle
    ));
}

#[test]
fn poll_bass_anchor_capture_treats_cancelled_as_idle() {
    use sotf_audio_player::recording_types::BassAnchorCaptureStatus;
    let mut app = make_app();
    app.recording.model.bass_anchor_capture.status =
        BassAnchorCaptureStatus::Running { started_at_ms: 0 };
    let slot = BASS_ANCHOR_CAPTURE_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    *slot.lock().unwrap() = Some(Err(
        sotf_audio::signal_recorder::CANCELLED_ERR.to_string()
    ));
    assert!(poll_bass_anchor_capture(&mut app));
    assert!(matches!(
        app.recording.model.bass_anchor_capture.status,
        BassAnchorCaptureStatus::Idle
    ));
}

#[test]
fn poll_recording_sets_and_clears_low_level_warning() {
    // R10: the shared static is process-global, so both scenarios run
    // sequentially in one test to avoid racing another test on the slot.
    let mut app = make_app();
    let mut ch = ChannelRecording::new(0, "FL".to_string());
    ch.state = ChannelRecordingState::Recording;
    app.recording.model.channel_recordings = vec![ch];
    let slot = RECORDING_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    // Very low transfer-function level → warning set and surfaced.
    *slot.lock().unwrap() = Some(Ok((vec![(0, done_recording_result(-80.0))], None)));
    assert!(poll_recording(&mut app));
    assert_eq!(
        app.recording.model.channel_recordings[0].state,
        ChannelRecordingState::Done
    );
    let warning = app
        .recording
        .model
        .noise_floor_warning
        .as_ref()
        .expect("low-level warning set");
    assert!(warning.contains("Very low measured level"));
    assert!(warning.contains("FL"));
    assert!(
        app.recording
            .model
            .status_message
            .contains("Very low measured level"),
        "warning should be surfaced in the status line: {}",
        app.recording.model.status_message
    );

    // Healthy level on the next take → warning cleared.
    app.recording.model.channel_recordings[0].state = ChannelRecordingState::Recording;
    *slot.lock().unwrap() = Some(Ok((vec![(0, done_recording_result(-10.0))], None)));
    assert!(poll_recording(&mut app));
    assert!(app.recording.model.noise_floor_warning.is_none());

    // R8: a user-requested cancel returns the channel to Empty (idle) —
    // NOT Error with "Recording failed: cancelled". Same process-global
    // slot, kept sequential inside this test.
    app.recording.model.channel_recordings[0].state = ChannelRecordingState::Recording;
    *slot.lock().unwrap() = Some(Err(sotf_audio::signal_recorder::CANCELLED_ERR.to_string()));
    assert!(poll_recording(&mut app));
    assert_eq!(
        app.recording.model.channel_recordings[0].state,
        ChannelRecordingState::Empty
    );
    assert_eq!(app.recording.model.status_message, "Recording cancelled");

    // A genuine failure still marks the channel Error.
    app.recording.model.channel_recordings[0].state = ChannelRecordingState::Recording;
    *slot.lock().unwrap() = Some(Err("device gone".to_string()));
    assert!(poll_recording(&mut app));
    assert_eq!(
        app.recording.model.channel_recordings[0].state,
        ChannelRecordingState::Error
    );
    assert!(
        app.recording
            .model
            .status_message
            .contains("Recording failed: device gone"),
        "unexpected status: {}",
        app.recording.model.status_message
    );
}

#[test]
fn save_recordings_writes_canonical_recordings_json() {
    // B5: the session file is always `recordings.json`, regardless of the
    // user-chosen session name; B4: previously dropped metadata fields are
    // persisted via the shared builder.
    let mut app = make_app();
    let tmp = tempfile::tempdir().unwrap();
    app.recording.output_directory = tmp.path().to_string_lossy().to_string();
    app.recording.model.save_name = "my_session".to_string();
    app.recording.model.playback_config.channel_mappings =
        vec![ChannelMapping::single(0, "FL")];
    app.recording.model.playback_config.num_channels = 1;
    let mut ch = ChannelRecording::new(0, "FL".to_string());
    ch.state = ChannelRecordingState::Done;
    ch.result = Some(done_recording_result(-10.0));
    app.recording.model.channel_recordings = vec![ch];

    save_recordings(&mut app);
    assert!(app.recording.save.error.is_none(), "save_recordings errored: {:?}", app.recording.save.error);
    let rx = app
        .recording
        .save
        .receiver
        .take()
        .expect("save thread spawned");
    let result = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("save thread answered");
    result.expect("save succeeded");

    let canonical = tmp.path().join("recordings.json");
    assert!(canonical.exists(), "expected {}", canonical.display());
    assert!(!tmp.path().join("my_session.json").exists());

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&canonical).unwrap()).unwrap();
    let cfg = &json["recording_config"];
    assert!(cfg.is_object(), "recording_config persisted");
    // Fields the old `..Default::default()` literal used to drop (B4).
    assert!(cfg.get("bass_probe_freq_hz").is_some());
    assert_eq!(cfg["bass_octave_duration_s"].as_f64(), Some(3.0));
    assert_eq!(cfg["pre_silence_s"].as_f64(), Some(2.0));
    assert_eq!(
        cfg["signal_type"].as_str(),
        Some(app.recording.model.signal_type.as_str())
    );
}

fn quality_summary(trustworthy: bool, score: f32) -> sotf_audio_player::recording_types::TakeQualitySummary {
    sotf_audio_player::recording_types::TakeQualitySummary {
        trustworthy,
        score,
        issues: if trustworthy {
            Vec::new()
        } else {
            vec!["low coherence".to_string()]
        },
        mean_coherence: Some(0.71),
        median_snr_db: Some(18.2),
        clip_fraction: 0.0,
        drift_ppm: Some(-45.0),
        drift_corrected: false,
        dropped_samples: 0,
        accepted_count: 4,
        rejected_count: 1,
    }
}

fn result_with_quality(
    mag_db: f32,
    quality: sotf_audio_player::recording_types::TakeQualitySummary,
) -> RecordingResult {
    let mut r = done_recording_result(mag_db);
    r.quality = Some(quality);
    r
}

#[test]
fn poll_recording_parks_untrustworthy_take_for_review() {
    // Process-global result slot: scenarios run sequentially in one test.
    let mut app = make_app();
    let mut ch = ChannelRecording::new(0, "FL".to_string());
    ch.state = ChannelRecordingState::Recording;
    app.recording.model.channel_recordings = vec![ch];
    let slot = RECORDING_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    // Untrustworthy take → ReviewNeeded, result kept, review status shown.
    *slot.lock().unwrap() = Some(Ok((
        vec![(0, result_with_quality(-10.0, quality_summary(false, 0.42)))],
        None,
    )));
    assert!(poll_recording(&mut app));
    assert_eq!(
        app.recording.model.channel_recordings[0].state,
        ChannelRecordingState::ReviewNeeded
    );
    assert!(app.recording.model.channel_recordings[0].result.is_some());
    assert!(
        app.recording.model.status_message.contains("needs review"),
        "unexpected status: {}",
        app.recording.model.status_message
    );

    // 'a' accepts the parked take despite the warnings.
    let key = crossterm::event::KeyEvent {
        code: crossterm::event::KeyCode::Char('a'),
        modifiers: crossterm::event::KeyModifiers::NONE,
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    };
    app.recording.model.step = RecordingStep::Capture;
    handle_recording_keys(&mut app, key);
    assert_eq!(
        app.recording.model.channel_recordings[0].state,
        ChannelRecordingState::Done
    );
    assert!(
        app.recording
            .model
            .status_message
            .contains("Accepted 1 take(s) despite quality warnings"),
        "unexpected status: {}",
        app.recording.model.status_message
    );

    // Trustworthy take goes straight to Done with the complete message.
    app.recording.model.channel_recordings[0].state = ChannelRecordingState::Recording;
    *slot.lock().unwrap() = Some(Ok((
        vec![(0, result_with_quality(-10.0, quality_summary(true, 0.95)))],
        None,
    )));
    assert!(poll_recording(&mut app));
    assert_eq!(
        app.recording.model.channel_recordings[0].state,
        ChannelRecordingState::Done
    );
    assert!(
        app.recording
            .model
            .status_message
            .contains("Channel FL recording complete"),
        "unexpected status: {}",
        app.recording.model.status_message
    );

    // Dropped samples surface a dropout warning in the status line.
    app.recording.model.channel_recordings[0].state = ChannelRecordingState::Recording;
    let mut q = quality_summary(true, 0.95);
    q.dropped_samples = 128;
    *slot.lock().unwrap() = Some(Ok((vec![(0, result_with_quality(-10.0, q))], None)));
    assert!(poll_recording(&mut app));
    assert!(
        app.recording
            .model
            .status_message
            .contains("128 samples dropped during capture"),
        "unexpected status: {}",
        app.recording.model.status_message
    );
}

#[test]
fn num_sweeps_field_adjust_and_edit_never_yield_two() {
    let mut app = make_app();
    // Field index 13 == NumSweeps (after NumPositions).
    app.recording.selected_field = 13;
    assert_eq!(app.recording.model.num_sweeps, 4);

    adjust_recording_field(&mut app, 1);
    assert_eq!(app.recording.model.num_sweeps, 5);
    // Stepping down from 3 skips 2 (outlier rejection needs >= 3).
    adjust_recording_field(&mut app, -1);
    adjust_recording_field(&mut app, -1);
    adjust_recording_field(&mut app, -1);
    assert_eq!(app.recording.model.num_sweeps, 1);
    adjust_recording_field(&mut app, 1);
    assert_eq!(app.recording.model.num_sweeps, 3);

    // Typing "2" is rejected loudly, not silently changed.
    app.recording.edit_buffer = "2".to_string();
    set_recording_field_from_string(&mut app);
    assert_eq!(app.recording.model.num_sweeps, 3);
    assert!(
        app.recording
            .model
            .status_message
            .contains("2 sweeps cannot reject outliers"),
        "unexpected status: {}",
        app.recording.model.status_message
    );

    // Values above the max clamp to 8.
    app.recording.edit_buffer = "64".to_string();
    set_recording_field_from_string(&mut app);
    assert_eq!(app.recording.model.num_sweeps, 8);
}

#[test]
fn num_positions_field_rebuilds_position_major_channel_list() {
    let mut app = make_app();
    // Field index 12 == NumPositions (after CtcLoopbackInput).
    app.recording.selected_field = 12;
    assert_eq!(app.recording.model.recording_config.num_positions, 1);
    let speakers = app.recording.model.playback_config.channel_mappings.len();
    let mics = app.recording.model.recording_config.channel_mappings.len().max(1);

    adjust_recording_field(&mut app, 1);
    assert_eq!(app.recording.model.recording_config.num_positions, 2);
    assert_eq!(
        app.recording.model.channel_recordings.len(),
        speakers * mics * 2,
        "second position doubles the channel list"
    );
    assert!(
        app.recording
            .model
            .channel_recordings
            .iter()
            .any(|c| c.channel_name.contains("(Pos 2)")),
        "position-suffixed channel names: {:?}",
        app.recording
            .model
            .channel_recordings
            .iter()
            .map(|c| c.channel_name.clone())
            .collect::<Vec<_>>()
    );

    // Clamped at 1..=8 in both directions.
    app.recording.edit_buffer = "0".to_string();
    set_recording_field_from_string(&mut app);
    assert_eq!(app.recording.model.recording_config.num_positions, 1);
    app.recording.edit_buffer = "99".to_string();
    set_recording_field_from_string(&mut app);
    assert_eq!(app.recording.model.recording_config.num_positions, 8);
}
