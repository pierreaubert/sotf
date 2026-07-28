//! Integration tests for the SOTF TUI crate (`sotf_audio_player_tui`).
//!
//! These tests exercise the crate's public API as a black box, focusing on:
//!   * full configuration wizard workflows (Headphone EQ, Recording, etc.);
//!   * navigation between screens and wizard steps;
//!   * state persistence roundtrips (plugin chains, app config, server config).

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use sotf_audio_player::{Album, PluginSettings, PluginType, Track};
use sotf_audio_player_tui::app::{
    App, ConfigureSubScreen, FederationMode, HeadphoneEqStep, InputMode, QueueEntry, QueueItem,
    Screen, ServerSection, SpinoramaStep,
};
use sotf_audio_player_tui::events::handle_key_event;
use sotf_audio_player_tui::theme::Theme;
use std::sync::{Mutex, Once};

// ── Test harness helpers ───────────────────────────────────────────────────

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn send_keys(app: &mut App, codes: &[KeyCode]) {
    for &code in codes {
        handle_key_event(app, key(code));
    }
}

fn type_text(app: &mut App, text: &str) {
    for ch in text.chars() {
        handle_key_event(app, key(KeyCode::Char(ch)));
    }
}

fn make_app() -> App {
    init_config();
    App::new(Theme::default(), true) // read-only avoids DB write contention
}

fn init_config() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let dir = tempfile::tempdir()
            .expect("failed to create temp config dir")
            .keep();
        sotf_audio_player::config::set_config_dir_override(dir);
    });
}

/// Serialize tests that mutate fixed config-dir files.
static CONFIG_LOCK: Mutex<()> = Mutex::new(());

/// Put the app on the Library screen, ready for screen-navigation tests.
fn app_on_library() -> App {
    let mut app = make_app();
    app.current_screen = Screen::Library;
    app.input_mode = InputMode::Normal;
    app
}

// ── Screen navigation ───────────────────────────────────────────────────────

#[test]
fn uppercase_shortcuts_switch_screens() {
    let mut app = app_on_library();

    send_keys(&mut app, &[KeyCode::Char('Q')]);
    assert_eq!(app.current_screen, Screen::Queue);

    send_keys(&mut app, &[KeyCode::Char('P')]);
    assert_eq!(app.current_screen, Screen::Plugins);

    send_keys(&mut app, &[KeyCode::Char('O')]);
    assert_eq!(app.current_screen, Screen::Devices);

    send_keys(&mut app, &[KeyCode::Char('C')]);
    assert_eq!(app.current_screen, Screen::Configure);
    assert_eq!(app.input_mode, InputMode::Configure);

    send_keys(&mut app, &[KeyCode::Char('L')]);
    assert_eq!(app.current_screen, Screen::Library);
    assert_eq!(app.input_mode, InputMode::Normal);
}

#[test]
fn tab_cycles_screens_and_configure_sub_screens() {
    let mut app = app_on_library();

    let expected = [
        Screen::Queue,
        Screen::Playlists,
        Screen::Plugins,
        Screen::Devices,
    ];
    for expected_screen in expected {
        send_keys(&mut app, &[KeyCode::Tab]);
        assert_eq!(app.current_screen, expected_screen);
    }

    // Devices -> Tools -> Configure
    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.current_screen, Screen::Tools);
    assert_eq!(app.input_mode, InputMode::Normal);

    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.current_screen, Screen::Configure);
    assert_eq!(app.input_mode, InputMode::Configure);

    // Tab on Configure tab bar cycles sub-screens
    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.current_screen, Screen::Configure);
    assert_eq!(app.configure_sub_screen, ConfigureSubScreen::Recording);
}

#[test]
fn esc_chain_returns_from_configure_to_library() {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.configure_sub_screen = ConfigureSubScreen::SpinoramaEq;
    app.input_mode = InputMode::ConfigureSpinoramaEq;
    app.spinorama_eq.step = SpinoramaStep::Configure;
    app.spinorama_eq.step_tab_focused = false;

    // Esc from Spinorama content -> step tab bar
    send_keys(&mut app, &[KeyCode::Esc]);
    assert!(app.spinorama_eq.step_tab_focused);
    assert_eq!(app.input_mode, InputMode::ConfigureSpinoramaEq);

    // Esc from step tab bar -> configure tab bar
    send_keys(&mut app, &[KeyCode::Esc]);
    assert_eq!(app.input_mode, InputMode::Configure);
    assert!(!app.spinorama_eq.step_tab_focused);

    // Esc from configure tab bar -> Library
    send_keys(&mut app, &[KeyCode::Esc]);
    assert_eq!(app.current_screen, Screen::Library);
    assert_eq!(app.input_mode, InputMode::Normal);
}

// ── Spinorama EQ wizard ─────────────────────────────────────────────────────

#[test]
fn spinorama_full_wizard_workflow() {
    let mut app = app_on_library();
    app.spinorama_eq.model.available_speakers =
        vec!["Adam A7V".to_string(), "Genelec 8030C".to_string()];
    app.spinorama_eq.update_filter();

    // Library -> Configure -> SpinoramaEq
    send_keys(&mut app, &[KeyCode::Char('C')]);
    send_keys(&mut app, &[KeyCode::Char('5')]);
    assert_eq!(app.configure_sub_screen, ConfigureSubScreen::SpinoramaEq);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Select);

    // Select the second speaker
    send_keys(&mut app, &[KeyCode::Down]);
    send_keys(&mut app, &[KeyCode::Enter]);
    assert_eq!(
        app.spinorama_eq.model.selected_speaker,
        Some("Genelec 8030C".to_string())
    );
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Configure);

    // Move to step tab bar and cycle through Optimize / Results / UpdatePlugin
    send_keys(&mut app, &[KeyCode::Esc]);
    assert!(app.spinorama_eq.step_tab_focused);

    send_keys(&mut app, &[KeyCode::Right, KeyCode::Right, KeyCode::Right]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::UpdatePlugin);

    // BackTab returns through the steps
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Results);
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Optimize);
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Configure);
}

// ── Room EQ wizard ──────────────────────────────────────────────────────────

#[test]
fn room_eq_step_navigation_wraps() {
    use sotf_audio_player::room_eq_types::RoomEqStep;

    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.configure_sub_screen = ConfigureSubScreen::RoomEq;
    app.input_mode = InputMode::ConfigureRoomEq;
    app.room_eq.model.step = RoomEqStep::LoadData;
    app.room_eq.step_tab_focused = true;

    let expected = [
        RoomEqStep::Delay,
        RoomEqStep::Process,
        RoomEqStep::Configure,
        RoomEqStep::Optimize,
        RoomEqStep::Review,
        RoomEqStep::Export,
        RoomEqStep::LoadData, // wrap
    ];
    for expected_step in expected {
        send_keys(&mut app, &[KeyCode::Right]);
        assert_eq!(app.room_eq.model.step, expected_step);
    }
}

#[test]
fn room_eq_file_explorer_auto_opens_when_no_data() {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.input_mode = InputMode::Configure;

    send_keys(&mut app, &[KeyCode::Char('3')]);
    assert_eq!(app.configure_sub_screen, ConfigureSubScreen::RoomEq);
    assert_eq!(app.input_mode, InputMode::FileExplorer);

    send_keys(&mut app, &[KeyCode::Esc]);
    assert_eq!(app.input_mode, InputMode::ConfigureRoomEq);
}

// ── Headphone EQ wizard ─────────────────────────────────────────────────────

#[test]
fn headphone_eq_full_wizard_navigation() {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.configure_sub_screen = ConfigureSubScreen::HeadphoneEq;
    app.input_mode = InputMode::ConfigureHeadphoneEq;
    app.headphone_eq.step = HeadphoneEqStep::SelectFile;

    use sotf_audio_player::room_eq_types::OptimizationStatus;

    // Step tab bar: SelectFile -> Configure -> Optimize -> Results -> UpdatePlugin
    app.headphone_eq.step_tab_focused = true;
    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.headphone_eq.step, HeadphoneEqStep::Configure);

    // Enter Configure content and edit a numerical field
    send_keys(&mut app, &[KeyCode::Down]);
    assert!(!app.headphone_eq.step_tab_focused);
    app.headphone_eq.config_selected_field = 0; // num_filters
    let before = app.headphone_eq.model.optimizer_config.num_filters;
    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(
        app.headphone_eq.model.optimizer_config.num_filters,
        before + 1
    );

    // Enter direct-edit mode and commit a new value
    send_keys(&mut app, &[KeyCode::Enter]);
    assert!(app.headphone_eq.editing_value);
    app.headphone_eq.edit_buffer.clear();
    type_text(&mut app, "12");
    send_keys(&mut app, &[KeyCode::Enter]);
    assert!(!app.headphone_eq.editing_value);
    assert_eq!(app.headphone_eq.model.optimizer_config.num_filters, 12);

    // Navigate to Optimize and mark it completed so Results is reachable
    send_keys(&mut app, &[KeyCode::Esc]);
    send_keys(&mut app, &[KeyCode::Right, KeyCode::Down]);
    assert_eq!(app.headphone_eq.step, HeadphoneEqStep::Optimize);
    app.headphone_eq.model.optimization_status = OptimizationStatus::Completed;

    send_keys(&mut app, &[KeyCode::Esc, KeyCode::Right, KeyCode::Down]);
    assert_eq!(app.headphone_eq.step, HeadphoneEqStep::Results);

    send_keys(&mut app, &[KeyCode::Esc, KeyCode::Right, KeyCode::Down]);
    assert_eq!(app.headphone_eq.step, HeadphoneEqStep::UpdatePlugin);
}

#[test]
fn headphone_eq_apply_without_filters_returns_error() {
    let mut app = app_on_library();
    app.headphone_eq.model.filters.clear();
    let result = app.apply_headphone_to_plugins();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("No optimization results"));
}

// ── Recording wizard ────────────────────────────────────────────────────────

#[test]
fn recording_full_wizard_navigation() {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.configure_sub_screen = ConfigureSubScreen::Recording;
    app.input_mode = InputMode::ConfigureRecording;
    app.recording.output_directory = "/tmp".to_string();

    use sotf_audio_player::recording_types::RecordingStep;

    // Step tab bar: Config -> SplCalibration -> Capture -> Probe -> BassAnchor -> Evaluating -> Saving
    app.recording.step_tab_focused = true;
    let expected = [
        RecordingStep::SplCalibration,
        RecordingStep::Capture,
        RecordingStep::Probe,
        RecordingStep::BassAnchor,
        RecordingStep::Evaluating,
        RecordingStep::Saving,
    ];
    for expected_step in expected {
        send_keys(&mut app, &[KeyCode::Right]);
        assert_eq!(app.recording.model.step, expected_step);
    }

    // BackTab from Saving returns to Evaluating
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.recording.model.step, RecordingStep::Evaluating);
}

#[test]
fn recording_capture_requires_output_directory() {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.configure_sub_screen = ConfigureSubScreen::Recording;
    app.input_mode = InputMode::ConfigureRecording;
    app.recording.output_directory.clear();

    use sotf_audio_player::recording_types::RecordingStep;

    app.recording.step_tab_focused = true;
    app.recording.model.step = RecordingStep::SplCalibration;
    send_keys(&mut app, &[KeyCode::Right]);
    // Should be blocked at SplCalibration because output dir is empty
    assert_eq!(app.recording.model.step, RecordingStep::SplCalibration);
    assert!(
        app.recording
            .model
            .status_message
            .contains("output directory"),
        "expected warning about output directory, got: {}",
        app.recording.model.status_message
    );
}

#[test]
fn recording_config_field_editing_roundtrip() {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.configure_sub_screen = ConfigureSubScreen::Recording;
    app.input_mode = InputMode::ConfigureRecording;
    app.recording.model.step = sotf_audio_player::recording_types::RecordingStep::Config;

    // Select OutputDir (field 8) and enter edit mode
    app.recording.selected_field = 8;
    send_keys(&mut app, &[KeyCode::Enter]);
    assert!(app.recording.editing_output_dir);

    type_text(&mut app, "/tmp/sotf-recordings");
    send_keys(&mut app, &[KeyCode::Enter]);
    assert!(!app.recording.editing_output_dir);
    assert_eq!(app.recording.output_directory, "/tmp/sotf-recordings");

    // Select a numeric field and edit it
    app.recording.selected_field = 4; // Duration
    send_keys(&mut app, &[KeyCode::Enter]);
    assert!(app.recording.editing_value);
    app.recording.edit_buffer.clear();
    type_text(&mut app, "9.5");
    send_keys(&mut app, &[KeyCode::Enter]);
    assert!(!app.recording.editing_value);
    assert!((app.recording.model.signal_duration_secs - 9.5).abs() < f32::EPSILON);
}

// ── Directories configuration ───────────────────────────────────────────────

#[test]
fn directories_add_input_roundtrip() {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.configure_sub_screen = ConfigureSubScreen::Directories;
    app.input_mode = InputMode::ConfigureDirectories;

    send_keys(&mut app, &[KeyCode::Char('a')]);
    assert!(app.library_view.editing_directory);
    assert!(app.library_view.directory_input.is_empty());

    type_text(&mut app, "/music/library");
    send_keys(&mut app, &[KeyCode::Esc]);
    assert!(!app.library_view.editing_directory);
    assert!(app.library_view.directory_input.is_empty());

    // Re-open and confirm
    send_keys(&mut app, &[KeyCode::Char('a')]);
    type_text(&mut app, "/music/library");
    let count_before = app.library.directories.len();
    send_keys(&mut app, &[KeyCode::Enter]);
    assert!(!app.library_view.editing_directory);
    assert_eq!(app.library.directories.len(), count_before + 1);
}

// ── Servers configuration ───────────────────────────────────────────────────

#[test]
fn servers_navigation_and_toggle_persists() {
    let _guard = CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.configure_sub_screen = ConfigureSubScreen::Servers;
    app.input_mode = InputMode::ConfigureServers;

    // Default section is Api
    assert_eq!(app.server_state.selected_section, ServerSection::Api);

    // Toggle API enabled
    let was_enabled = app.server_state.config.api.enabled;
    send_keys(&mut app, &[KeyCode::Enter]);
    assert_eq!(app.server_state.config.api.enabled, !was_enabled);

    // Navigate to Mpd section
    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.server_state.selected_section, ServerSection::Mpd);

    // Edit MPD bind address
    app.server_state.selected_field = 1;
    send_keys(&mut app, &[KeyCode::Enter]);
    assert!(app.server_state.editing_value);
    app.server_state.edit_buffer.clear();
    type_text(&mut app, "127.0.0.2");
    send_keys(&mut app, &[KeyCode::Enter]);
    assert!(!app.server_state.editing_value);
    assert_eq!(app.server_state.config.mpd.bind_address, "127.0.0.2");

    // Create a fresh app and verify the persisted server config is loaded
    let app2 = make_app();
    assert_eq!(app2.server_state.config.mpd.bind_address, "127.0.0.2");
    assert_eq!(app2.server_state.config.api.enabled, !was_enabled);
}

// ── Federation sources ──────────────────────────────────────────────────────

#[test]
fn federation_add_source_workflow() {
    let _guard = CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.configure_sub_screen = ConfigureSubScreen::FederationSources;
    app.input_mode = InputMode::ConfigureFederationSources;

    assert_eq!(app.federation.state.mode, FederationMode::List);
    assert!(app.federation.state.sources.is_empty());

    // Enter add mode
    send_keys(&mut app, &[KeyCode::Char('a')]);
    assert_eq!(app.federation.state.mode, FederationMode::AddSource);

    // Confirm source type -> enters EditSource mode with a new source
    send_keys(&mut app, &[KeyCode::Enter]);
    assert_eq!(app.federation.state.mode, FederationMode::EditSource);
    assert!(app.federation.state.edit.is_some());

    // Edit the display name (it follows the connection fields)
    {
        let edit = app.federation.state.edit.as_mut().unwrap();
        edit.selected_field = edit.source.connection.field_names().len();
    }
    send_keys(&mut app, &[KeyCode::Enter]);
    assert!(app.federation.state.edit.as_ref().unwrap().editing_value);
    app.federation
        .state
        .edit
        .as_mut()
        .unwrap()
        .edit_buffer
        .clear();
    type_text(&mut app, "My Source");
    send_keys(&mut app, &[KeyCode::Enter]);
    assert!(!app.federation.state.edit.as_ref().unwrap().editing_value);

    // Save and return to list
    send_keys(&mut app, &[KeyCode::Char('s')]);
    assert_eq!(app.federation.state.mode, FederationMode::List);
    assert_eq!(app.federation.state.sources.len(), 1);
    assert_eq!(app.federation.state.sources[0].display_name, "My Source");
}

// ── State persistence ───────────────────────────────────────────────────────

#[test]
fn plugin_chain_save_load_roundtrip() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut app = make_app();

    // Add an EQ plugin with a custom filter
    app.add_plugin(&PluginType::EQ);
    let eq_idx = (0..app.plugin_rack.graph.len())
        .rev()
        .find(|&i| {
            app.plugin_rack
                .graph
                .get_plugin(i)
                .map(|p| matches!(p.settings, PluginSettings::EQ { .. }))
                .unwrap_or(false)
        })
        .expect("expected an EQ plugin");

    if let Some(plugin) = app.plugin_rack.graph.get_plugin_mut(eq_idx)
        && let PluginSettings::EQ {
            ref mut filters,
            ref mut max_filters,
            ..
        } = plugin.settings
    {
        filters.clear();
        filters.push(sotf_audio_player::EQFilter::new(
            math_audio_iir_fir::BiquadFilterType::Peak,
            1000.0,
            1.5,
            3.0,
        ));
        *max_filters = 1;
    }

    let saved_len = app.plugin_rack.graph.len();
    let preset_name = "integration_test_eq";
    app.plugin_rack
        .graph
        .save_to_file(temp_dir.path(), preset_name)
        .expect("save plugin chain");

    // Reset graph to default rack and reload
    app.plugin_rack.graph = sotf_audio_player::PluginGraph::with_default_rack();
    assert!(app.plugin_rack.graph.len() < saved_len);

    app.plugin_rack
        .graph
        .load_from_file(temp_dir.path(), preset_name)
        .expect("load plugin chain");
    assert_eq!(app.plugin_rack.graph.len(), saved_len);

    let reloaded_eq = app
        .plugin_rack
        .graph
        .get_plugin(eq_idx)
        .expect("reloaded EQ plugin index");
    if let PluginSettings::EQ { filters, .. } = &reloaded_eq.settings {
        assert_eq!(filters.len(), 1);
        assert!((filters[0].frequency - 1000.0).abs() < f64::EPSILON);
    } else {
        panic!("expected EQ plugin after reload");
    }
}

#[test]
fn app_config_save_load_roundtrip() {
    let _guard = CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut app = make_app();

    // Build a fake album + queue
    let album = Album {
        title: "Integration Album".to_string(),
        tracks: vec![Track {
            path: std::path::PathBuf::from("/tmp/fake.flac"),
            title: Some("Track 1".to_string()),
            artist: Some("Integration Artist".to_string()),
            album_artist: Some("Integration Artist".to_string()),
            ..Default::default()
        }],
        ..Default::default()
    };
    app.library.albums.push(album.clone());
    app.queue
        .push(QueueEntry::new(QueueItem::new(album.clone())));
    app.playback.current_queue_index = Some(0);

    // Save a real plugin preset so load_config can restore it.
    let presets_dir =
        sotf_audio_player::config::get_plugin_presets_dir().expect("plugin presets dir");
    app.plugin_rack
        .graph
        .save_to_file(&presets_dir, "my-preset")
        .expect("save plugin preset");
    app.plugin_rack.last_loaded_preset = Some("my-preset.json".to_string());

    app.save_config().expect("save app config");

    // Fresh app: load config and verify restored state
    let mut app2 = make_app();
    app2.library.albums.push(album);
    app2.load_config().expect("load app config");

    assert_eq!(
        app2.plugin_rack.last_loaded_preset,
        Some("my-preset.json".to_string())
    );
    assert_eq!(app2.playback.current_queue_index, Some(0));
    assert_eq!(app2.queue.len(), 1);
    assert_eq!(
        app2.queue[0].item.album.title,
        "Integration Album".to_string()
    );
}

// ── Error paths / edge cases ────────────────────────────────────────────────

#[test]
fn esc_from_library_quits() {
    let mut app = app_on_library();
    send_keys(&mut app, &[KeyCode::Esc]);
    assert!(app.should_quit);
}

#[test]
fn plugins_screen_handles_empty_chain() {
    let mut app = app_on_library();
    app.current_screen = Screen::Plugins;
    app.plugin_rack.graph = sotf_audio_player::PluginGraph::default();

    // Removing from an empty chain should not panic
    send_keys(&mut app, &[KeyCode::Char('d')]);
    assert!(app.plugin_rack.graph.is_empty());

    // Toggling in an empty chain should not panic
    send_keys(&mut app, &[KeyCode::Char(' ')]);
    assert!(app.plugin_rack.graph.is_empty());
}

#[test]
fn spinorama_apply_without_filters_returns_error() {
    let mut app = app_on_library();
    app.spinorama_eq.model.filters.clear();
    let result = app.apply_spinorama_to_plugins();
    assert!(result.is_err());
}
