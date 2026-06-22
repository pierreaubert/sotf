use super::adjust::adjust_recording_field;
use super::handle::handle_recording_keys;
use super::misc::ctc_raw_capture_channel_indices;
use super::misc::init_recording_channels;
use super::misc::save_recordings;
use super::misc::update_channel_mappings_for_config;

use crate::events::tests::make_app;
use sotf_audio_player::recording_types::{ChannelMapping, RecordingStep, SpeakerConfiguration};

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
    assert!(app.recording.save_error.is_some());
    assert!(
        app.recording
            .save_error
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
    assert!(app.recording.save_error.is_some());
    assert!(
        app.recording
            .save_error
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
    assert!(app.recording.save_error.is_some());
    assert!(
        app.recording
            .save_error
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
