use super::super::super::PlayerCommand;
use super::super::super::handle_key_event;
use super::super::super::tests::{key, make_app};
use super::super::app::app_on_library;
use super::super::misc::send_keys;
use crate::app::{ConfigureSubScreen, HeadphoneEqStep, InputMode, Screen};
use crossterm::event::KeyCode;
use sotf_audio_player::recording_types::RecordingStep;

fn app_on_headphone_eq() -> crate::app::App {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.configure_sub_screen = ConfigureSubScreen::HeadphoneEq;
    app.input_mode = InputMode::ConfigureHeadphoneEq;
    app
}

#[test]
fn headphone_eq_optimize_backtab_goes_to_configure() {
    let mut app = app_on_headphone_eq();
    app.headphone_eq.step = HeadphoneEqStep::Optimize;
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.headphone_eq.step, HeadphoneEqStep::Configure);
}

#[test]
fn headphone_eq_results_backtab_goes_to_optimize() {
    let mut app = app_on_headphone_eq();
    app.headphone_eq.step = HeadphoneEqStep::Results;
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.headphone_eq.step, HeadphoneEqStep::Optimize);
}

#[test]
fn headphone_eq_update_plugin_backtab_goes_to_results() {
    let mut app = app_on_headphone_eq();
    app.headphone_eq.step = HeadphoneEqStep::UpdatePlugin;
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.headphone_eq.step, HeadphoneEqStep::Results);
}

#[test]
fn headphone_eq_backtab_chain() {
    let mut app = app_on_headphone_eq();
    app.headphone_eq.step = HeadphoneEqStep::UpdatePlugin;

    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.headphone_eq.step, HeadphoneEqStep::Results);
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.headphone_eq.step, HeadphoneEqStep::Optimize);
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.headphone_eq.step, HeadphoneEqStep::Configure);
}

fn app_on_recording() -> crate::app::App {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.configure_sub_screen = ConfigureSubScreen::Recording;
    app.input_mode = InputMode::ConfigureRecording;
    app
}

#[test]
fn recording_capture_backtab_goes_to_config() {
    let mut app = app_on_recording();
    app.recording.step = RecordingStep::Capture;
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.recording.step, RecordingStep::Config);
}

#[test]
fn recording_evaluating_backtab_goes_to_capture() {
    let mut app = app_on_recording();
    app.recording.step = RecordingStep::Evaluating;
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.recording.step, RecordingStep::Capture);
}

#[test]
fn recording_saving_backtab_goes_to_evaluating() {
    let mut app = app_on_recording();
    app.recording.step = RecordingStep::Saving;
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.recording.step, RecordingStep::Evaluating);
}

#[test]
fn recording_backtab_chain() {
    let mut app = app_on_recording();
    app.recording.step = RecordingStep::Saving;

    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.recording.step, RecordingStep::Evaluating);
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.recording.step, RecordingStep::Capture);
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.recording.step, RecordingStep::Config);
}

/// Create an app with the channel conflict dialog open and one fake conflict.
fn app_in_channel_conflict() -> crate::app::App {
    use sotf_audio_player::{ChannelConflict, PluginType};

    let mut app = make_app();
    app.input_mode = InputMode::ChannelConflict;
    app.channel_conflict_selection = 0;
    app.channel_conflict_path = Some(sotf_audio::decoder::AudioSource::File(
        std::path::PathBuf::from("/fake/track.flac"),
    ));
    app.channel_conflict_track_channels = 6;
    app.channel_conflicts = vec![ChannelConflict {
        index: 0,
        plugin_type: PluginType::Upmixer,
        required_channels: 2,
        actual_channels: 6,
    }];
    app
}

#[test]
fn channel_conflict_up_down_navigates_selection() {
    let mut app = app_in_channel_conflict();
    assert_eq!(app.channel_conflict_selection, 0);

    send_keys(&mut app, &[KeyCode::Down]);
    assert_eq!(app.channel_conflict_selection, 1);

    send_keys(&mut app, &[KeyCode::Down]);
    assert_eq!(app.channel_conflict_selection, 2);

    // Clamps at bottom
    send_keys(&mut app, &[KeyCode::Down]);
    assert_eq!(app.channel_conflict_selection, 2);

    send_keys(&mut app, &[KeyCode::Up]);
    assert_eq!(app.channel_conflict_selection, 1);

    send_keys(&mut app, &[KeyCode::Up]);
    assert_eq!(app.channel_conflict_selection, 0);

    // Clamps at top
    send_keys(&mut app, &[KeyCode::Up]);
    assert_eq!(app.channel_conflict_selection, 0);
}

#[test]
fn channel_conflict_enter_on_suspend_returns_play() {
    let mut app = app_in_channel_conflict();
    app.channel_conflict_selection = 0; // Suspend

    let cmd = handle_key_event(&mut app, key(KeyCode::Enter));
    assert_eq!(app.input_mode, InputMode::Normal);
    assert!(matches!(cmd, Some(PlayerCommand::PlayResolved(_))));
    assert!(app.channel_conflicts.is_empty());
    assert!(app.channel_conflict_path.is_none());
}

#[test]
fn channel_conflict_enter_on_remove_returns_play() {
    let mut app = app_in_channel_conflict();
    app.channel_conflict_selection = 1; // Remove

    let cmd = handle_key_event(&mut app, key(KeyCode::Enter));
    assert_eq!(app.input_mode, InputMode::Normal);
    assert!(matches!(cmd, Some(PlayerCommand::PlayResolved(_))));
}

#[test]
fn channel_conflict_enter_on_cancel_stops_playback() {
    let mut app = app_in_channel_conflict();
    app.channel_conflict_selection = 2; // Cancel

    let cmd = handle_key_event(&mut app, key(KeyCode::Enter));
    assert_eq!(app.input_mode, InputMode::Normal);
    assert!(cmd.is_none());
    assert!(!app.is_playing);
}

#[test]
fn channel_conflict_esc_cancels() {
    let mut app = app_in_channel_conflict();

    let cmd = handle_key_event(&mut app, key(KeyCode::Esc));
    assert_eq!(app.input_mode, InputMode::Normal);
    assert!(cmd.is_none());
    assert!(!app.is_playing);
    assert!(app.channel_conflicts.is_empty());
    assert!(app.channel_conflict_path.is_none());
}

#[test]
fn channel_conflict_navigate_then_enter() {
    let mut app = app_in_channel_conflict();

    // Down, Down → selection 2 (Cancel), then Enter
    send_keys(&mut app, &[KeyCode::Down, KeyCode::Down]);
    assert_eq!(app.channel_conflict_selection, 2);

    let cmd = handle_key_event(&mut app, key(KeyCode::Enter));
    assert_eq!(app.input_mode, InputMode::Normal);
    assert!(cmd.is_none());
    assert!(!app.is_playing);
}

#[test]
fn headphone_eq_step_tab_right_cycles_all_steps() {
    let mut app = app_on_headphone_eq();
    app.headphone_eq.step_tab_focused = true;

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.headphone_eq.step, HeadphoneEqStep::Configure);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.headphone_eq.step, HeadphoneEqStep::Optimize);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.headphone_eq.step, HeadphoneEqStep::Results);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.headphone_eq.step, HeadphoneEqStep::UpdatePlugin);

    // Wraps
    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.headphone_eq.step, HeadphoneEqStep::SelectFile);
}

#[test]
fn headphone_eq_step_tab_left_wraps_from_select_to_update_plugin() {
    let mut app = app_on_headphone_eq();
    assert_eq!(app.headphone_eq.step, HeadphoneEqStep::SelectFile);
    app.headphone_eq.step_tab_focused = true;

    send_keys(&mut app, &[KeyCode::Left]);
    assert_eq!(app.headphone_eq.step, HeadphoneEqStep::UpdatePlugin);
}

#[test]
fn recording_step_tab_right_cycles_all_steps() {
    let mut app = app_on_recording();
    app.recording.output_directory = "/tmp/test".to_string();
    app.recording.step_tab_focused = true;

    // Cycle order matches `RecordingStep::all()` —
    // Config → SplCalibration → Capture → Probe → BassAnchor →
    // Evaluating → Saving → Config.
    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.recording.step, RecordingStep::SplCalibration);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.recording.step, RecordingStep::Capture);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.recording.step, RecordingStep::Probe);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.recording.step, RecordingStep::BassAnchor);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.recording.step, RecordingStep::Evaluating);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.recording.step, RecordingStep::Saving);

    // Wraps
    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.recording.step, RecordingStep::Config);
}

#[test]
fn recording_step_tab_left_wraps_from_config_to_saving() {
    let mut app = app_on_recording();
    assert_eq!(app.recording.step, RecordingStep::Config);
    app.recording.step_tab_focused = true;

    send_keys(&mut app, &[KeyCode::Left]);
    assert_eq!(app.recording.step, RecordingStep::Saving);
}

#[test]
fn headphone_eq_configure_tab_cycles_detail_level() {
    use sotf_audio_player::autoeq::DetailLevel;
    let mut app = app_on_headphone_eq();
    app.headphone_eq.step = HeadphoneEqStep::Configure;
    app.headphone_eq.config_selected_field = 100; // preset field

    // Tab cycles detail level: Simple -> Intermediate -> Expert -> Simple
    assert_eq!(app.headphone_eq.detail_level, DetailLevel::Simple);
    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.headphone_eq.detail_level, DetailLevel::Intermediate);
    assert_eq!(app.headphone_eq.step, HeadphoneEqStep::Configure);

    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.headphone_eq.detail_level, DetailLevel::Expert);

    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.headphone_eq.detail_level, DetailLevel::Simple);
}

#[test]
fn recording_config_tab_cycles_fields() {
    let mut app = app_on_recording();
    app.recording.step = RecordingStep::Config;
    app.recording.selected_field = 0;

    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.recording.selected_field, 1);
    assert_eq!(app.recording.step, RecordingStep::Config);

    // Wrap at max field. Field count is dynamic: 10 statics +
    // 2*num_channels per-channel rows (mic cal + input mapping). The
    // default RecordingDeviceConfig has num_channels=1, so the last
    // selectable index is 11.
    let last = crate::app::recording_field_count(&app.recording) - 1;
    app.recording.selected_field = last;
    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.recording.selected_field, 0);
    assert_eq!(app.recording.step, RecordingStep::Config);
}

/// Helper: create an app in directory editing mode with autocomplete menu active.
fn app_editing_directory_with_suggestions(suggestions: Vec<String>) -> crate::app::App {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.configure_sub_screen = ConfigureSubScreen::Directories;
    app.input_mode = InputMode::ConfigureDirectories;
    app.editing_directory = true;
    app.directory_input = "test".to_string();
    app.autocomplete_suggestions = suggestions;
    app.autocomplete_menu_active = true;
    app.autocomplete_index = 0;
    app
}

#[test]
fn directory_autocomplete_down_arrow_cycles_suggestions() {
    let mut app = app_editing_directory_with_suggestions(vec![
        "/test/aaa/".to_string(),
        "/test/bbb/".to_string(),
        "/test/ccc/".to_string(),
    ]);

    // Down arrow should select next suggestion
    send_keys(&mut app, &[KeyCode::Down]);
    assert_eq!(app.autocomplete_index, 1);
    assert_eq!(app.directory_input, "/test/bbb/");

    send_keys(&mut app, &[KeyCode::Down]);
    assert_eq!(app.autocomplete_index, 2);
    assert_eq!(app.directory_input, "/test/ccc/");

    // Wrap around
    send_keys(&mut app, &[KeyCode::Down]);
    assert_eq!(app.autocomplete_index, 0);
    assert_eq!(app.directory_input, "/test/aaa/");
}

#[test]
fn directory_autocomplete_up_arrow_cycles_suggestions() {
    let mut app = app_editing_directory_with_suggestions(vec![
        "/test/aaa/".to_string(),
        "/test/bbb/".to_string(),
        "/test/ccc/".to_string(),
    ]);

    // Up arrow from index 0 should wrap to last
    send_keys(&mut app, &[KeyCode::Up]);
    assert_eq!(app.autocomplete_index, 2);
    assert_eq!(app.directory_input, "/test/ccc/");

    send_keys(&mut app, &[KeyCode::Up]);
    assert_eq!(app.autocomplete_index, 1);
    assert_eq!(app.directory_input, "/test/bbb/");
}
