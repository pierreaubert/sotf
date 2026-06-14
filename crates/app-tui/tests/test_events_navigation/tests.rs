use super::app::app_on_library;
use super::app::app_on_room_eq;
use super::app::app_on_spinorama_select;
use super::misc::send_keys;
use crate::app::{ConfigureSubScreen, InputMode, Screen, SpinoramaStep};
use crossterm::event::KeyCode;
use sotf_audio_player::room_eq_types::RoomEqStep;
use sotf_audio_player::{Album, MetadataImportCandidate, Track};
use std::path::PathBuf;

#[path = "tests/app_mod.rs"]
mod app_mod;

#[test]
fn uppercase_c_goes_to_configure() {
    let mut app = app_on_library();
    send_keys(&mut app, &[KeyCode::Char('C')]);
    assert_eq!(app.current_screen, Screen::Configure);
}

#[test]
fn uppercase_l_goes_to_library() {
    let mut app = app_on_library();
    app.current_screen = Screen::Plugins;
    send_keys(&mut app, &[KeyCode::Char('L')]);
    assert_eq!(app.current_screen, Screen::Library);
}

#[test]
fn uppercase_q_goes_to_queue() {
    let mut app = app_on_library();
    send_keys(&mut app, &[KeyCode::Char('Q')]);
    assert_eq!(app.current_screen, Screen::Queue);
}

#[test]
fn uppercase_p_goes_to_plugins() {
    let mut app = app_on_library();
    send_keys(&mut app, &[KeyCode::Char('P')]);
    assert_eq!(app.current_screen, Screen::Plugins);
}

#[test]
fn uppercase_o_goes_to_devices() {
    let mut app = app_on_library();
    send_keys(&mut app, &[KeyCode::Char('O')]);
    assert_eq!(app.current_screen, Screen::Devices);
}

#[test]
fn uppercase_n_goes_to_configure() {
    let mut app = app_on_library();
    send_keys(&mut app, &[KeyCode::Char('N')]);
    assert_eq!(app.current_screen, Screen::Configure);
}

#[test]
fn tab_cycles_through_all_screens() {
    let mut app = app_on_library();
    assert_eq!(app.current_screen, Screen::Library);
    assert_eq!(app.input_mode, InputMode::Normal);

    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.current_screen, Screen::Queue);

    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.current_screen, Screen::Playlists);

    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.current_screen, Screen::Plugins);

    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.current_screen, Screen::Devices);

    // Tab from Devices enters Configure but with configure owning Tab
    // Actually, from Devices the normal Tab handler runs:
    // Devices → Configure
    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.current_screen, Screen::Configure);
}

#[test]
fn tab_from_configure_cycles_sub_screen() {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.input_mode = InputMode::Configure;

    // Tab on Configure tab bar cycles sub-screens
    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.current_screen, Screen::Configure);
    assert_eq!(app.configure_sub_screen, ConfigureSubScreen::Recording);
}

#[test]
fn tab_from_meters_returns_to_library() {
    let mut app = app_on_library();
    app.input_mode = InputMode::LevelMeters;

    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.input_mode, InputMode::Normal);
    assert_eq!(app.current_screen, Screen::Library);
}

#[test]
fn esc_from_configure_tab_bar_goes_to_library() {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.input_mode = InputMode::Configure;

    send_keys(&mut app, &[KeyCode::Esc]);
    assert_eq!(app.current_screen, Screen::Library);
    assert_eq!(app.input_mode, InputMode::Normal);
}

#[test]
fn esc_from_configure_content_goes_to_tab_bar() {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.configure_sub_screen = ConfigureSubScreen::Directories;
    app.input_mode = InputMode::ConfigureDirectories;

    send_keys(&mut app, &[KeyCode::Esc]);
    assert_eq!(app.input_mode, InputMode::Configure);
    assert_eq!(app.current_screen, Screen::Configure);
}

#[test]
fn esc_from_library_quits() {
    let mut app = app_on_library();
    send_keys(&mut app, &[KeyCode::Esc]);
    assert!(app.should_quit);
    assert_eq!(app.current_screen, Screen::Library);
    assert_eq!(app.input_mode, InputMode::Normal);
}

#[test]
fn metadata_editor_opens_edits_previews_imports_and_closes() {
    let mut app = app_on_library();
    app.library.albums = vec![metadata_test_album()];
    app.needs_filter_update = true;
    app.filtered_albums();

    send_keys(&mut app, &[KeyCode::Char('m')]);
    assert_eq!(app.input_mode, InputMode::MetadataEditor);
    assert!(app.metadata_editor.is_some());

    send_keys(
        &mut app,
        &[
            KeyCode::Down,
            KeyCode::Down,
            KeyCode::Down,
            KeyCode::Enter,
            KeyCode::Backspace,
            KeyCode::Backspace,
            KeyCode::Backspace,
            KeyCode::Backspace,
            KeyCode::Char('2'),
            KeyCode::Char('0'),
            KeyCode::Char('2'),
            KeyCode::Char('4'),
            KeyCode::Enter,
        ],
    );
    assert_eq!(
        app.metadata_editor.as_ref().unwrap().fields.year.as_str(),
        "2024"
    );
    assert!(app.metadata_editor.as_ref().unwrap().preview.is_some());

    app.metadata_editor
        .as_mut()
        .unwrap()
        .search_results
        .push(MetadataImportCandidate {
            provider_id: "musicbrainz".to_string(),
            provider_entity_id: "release-1".to_string(),
            title: None,
            artist: None,
            album_artist: Some("Imported Artist".to_string()),
            album_title: Some("Imported Album".to_string()),
            year: Some(2026),
            track_number: None,
            disc_number: None,
            isrc: None,
            score: 95,
        });
    send_keys(&mut app, &[KeyCode::Char('i')]);
    let editor = app.metadata_editor.as_ref().unwrap();
    assert_eq!(editor.fields.title, "Imported Album");
    assert_eq!(editor.fields.album_artist, "Imported Artist");
    assert!(editor.preview.is_some());

    send_keys(&mut app, &[KeyCode::Esc]);
    assert_eq!(app.input_mode, InputMode::Normal);
    assert!(app.metadata_editor.is_none());
}

fn metadata_test_album() -> Album {
    Album {
        id: Some(7),
        title: "Original Album".to_string(),
        year: Some(1999),
        tracks: vec![Track {
            path: PathBuf::from("/tmp/sotf-tui-metadata-test.flac"),
            title: Some("Original Track".to_string()),
            artist: Some("Original Artist".to_string()),
            album_artist: Some("Original Artist".to_string()),
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[test]
fn esc_from_meters_goes_to_normal() {
    let mut app = app_on_library();
    app.input_mode = InputMode::LevelMeters;

    send_keys(&mut app, &[KeyCode::Esc]);
    assert_eq!(app.input_mode, InputMode::Normal);
    assert!(!app.should_quit);
}

#[test]
fn configure_digit_keys_select_tabs() {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.input_mode = InputMode::Configure;

    send_keys(&mut app, &[KeyCode::Char('1')]);
    assert_eq!(app.configure_sub_screen, ConfigureSubScreen::Directories);
    assert!(app.input_mode.is_configure_sub_screen());

    // Go back to tab bar
    app.input_mode = InputMode::Configure;
    send_keys(&mut app, &[KeyCode::Char('2')]);
    assert_eq!(app.configure_sub_screen, ConfigureSubScreen::Recording);
    assert!(app.input_mode.is_configure_sub_screen());

    app.input_mode = InputMode::Configure;
    send_keys(&mut app, &[KeyCode::Char('3')]);
    assert_eq!(app.configure_sub_screen, ConfigureSubScreen::RoomEq);
    // RoomEq auto-opens file explorer when no data is loaded
    assert_eq!(app.input_mode, InputMode::FileExplorer);
    // Close auto-opened file explorer before continuing
    send_keys(&mut app, &[KeyCode::Esc]);

    app.input_mode = InputMode::Configure;
    send_keys(&mut app, &[KeyCode::Char('4')]);
    assert_eq!(app.configure_sub_screen, ConfigureSubScreen::HeadphoneEq);
    assert!(app.input_mode.is_configure_sub_screen());

    app.input_mode = InputMode::Configure;
    send_keys(&mut app, &[KeyCode::Char('5')]);
    assert_eq!(app.configure_sub_screen, ConfigureSubScreen::SpinoramaEq);
    assert!(app.input_mode.is_configure_sub_screen());
}

#[test]
fn configure_tab_bar_left_right_cycles() {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.input_mode = InputMode::Configure;
    app.configure_sub_screen = ConfigureSubScreen::Directories;

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.configure_sub_screen, ConfigureSubScreen::Recording);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.configure_sub_screen, ConfigureSubScreen::RoomEq);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.configure_sub_screen, ConfigureSubScreen::HeadphoneEq);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.configure_sub_screen, ConfigureSubScreen::SpinoramaEq);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(
        app.configure_sub_screen,
        ConfigureSubScreen::FederationSources
    );

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.configure_sub_screen, ConfigureSubScreen::Servers);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(
        app.configure_sub_screen,
        ConfigureSubScreen::MetadataServices
    );

    // Wraps back to Directories
    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.configure_sub_screen, ConfigureSubScreen::Directories);

    // And Left wraps backwards
    send_keys(&mut app, &[KeyCode::Left]);
    assert_eq!(
        app.configure_sub_screen,
        ConfigureSubScreen::MetadataServices
    );
}

#[test]
fn configure_enter_enters_sub_screen() {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.input_mode = InputMode::Configure;
    app.configure_sub_screen = ConfigureSubScreen::SpinoramaEq;

    send_keys(&mut app, &[KeyCode::Enter]);
    assert!(app.input_mode.is_configure_sub_screen());
    assert_eq!(app.configure_sub_screen, ConfigureSubScreen::SpinoramaEq);
}

#[test]
fn slash_enters_search_mode() {
    let mut app = app_on_library();
    send_keys(&mut app, &[KeyCode::Char('/')]);
    assert_eq!(app.input_mode, InputMode::Search);
}

#[test]
fn esc_exits_search_mode() {
    let mut app = app_on_library();
    app.input_mode = InputMode::Search;
    send_keys(&mut app, &[KeyCode::Esc]);
    assert_eq!(app.input_mode, InputMode::Normal);
}

#[test]
fn question_mark_enters_help_mode() {
    let mut app = app_on_library();
    send_keys(&mut app, &[KeyCode::Char('?')]);
    assert_eq!(app.input_mode, InputMode::ShowHelp);
}

#[test]
fn esc_exits_help_mode() {
    let mut app = app_on_library();
    app.input_mode = InputMode::ShowHelp;
    send_keys(&mut app, &[KeyCode::Esc]);
    assert_eq!(app.input_mode, InputMode::Normal);
}

#[test]
fn esc_exits_error_mode() {
    let mut app = app_on_library();
    app.input_mode = InputMode::ShowError;
    send_keys(&mut app, &[KeyCode::Esc]);
    assert_eq!(app.input_mode, InputMode::Normal);
}

#[test]
fn spinorama_enter_selects_speaker_and_advances_to_configure() {
    let mut app = app_on_spinorama_select();
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Select);

    send_keys(&mut app, &[KeyCode::Enter]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Configure);
    assert_eq!(
        app.spinorama_eq.selected_speaker,
        Some("Speaker A".to_string())
    );
}

#[test]
fn spinorama_down_navigates_speaker_list() {
    let mut app = app_on_spinorama_select();
    assert_eq!(app.spinorama_eq.selected_speaker_idx, 0);

    send_keys(&mut app, &[KeyCode::Down]);
    assert_eq!(app.spinorama_eq.selected_speaker_idx, 1);

    send_keys(&mut app, &[KeyCode::Down]);
    assert_eq!(app.spinorama_eq.selected_speaker_idx, 2);

    // At bottom, stays there
    send_keys(&mut app, &[KeyCode::Down]);
    assert_eq!(app.spinorama_eq.selected_speaker_idx, 2);
}

#[test]
fn spinorama_up_at_top_goes_to_step_tab() {
    let mut app = app_on_spinorama_select();
    assert_eq!(app.spinorama_eq.selected_speaker_idx, 0);
    assert!(app.input_mode.is_configure_sub_screen());
    assert!(!app.spinorama_eq.step_tab_focused);

    // Up at first item → step tab bar (not top-level configure tab bar)
    send_keys(&mut app, &[KeyCode::Up]);
    assert!(app.spinorama_eq.step_tab_focused);
    assert!(app.input_mode.is_configure_sub_screen());

    // Up again → top-level configure tab bar
    send_keys(&mut app, &[KeyCode::Up]);
    assert_eq!(app.input_mode, InputMode::Configure);
    assert!(!app.spinorama_eq.step_tab_focused);
}

#[test]
fn spinorama_configure_right_adjusts_field() {
    let mut app = app_on_spinorama_select();
    app.spinorama_eq.step = SpinoramaStep::Configure;
    app.spinorama_eq.selected_field = 1; // num_filters
    let before = app.spinorama_eq.config.num_filters;
    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.spinorama_eq.config.num_filters, before + 1);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Configure);
}

#[test]
fn spinorama_configure_enter_enters_edit_mode() {
    let mut app = app_on_spinorama_select();
    app.spinorama_eq.step = SpinoramaStep::Configure;
    app.spinorama_eq.selected_field = 1; // num_filters (numerical)
    send_keys(&mut app, &[KeyCode::Enter]);
    assert!(app.spinorama_eq.editing_value);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Configure);
}

#[test]
fn spinorama_results_backtab_goes_to_optimize() {
    let mut app = app_on_spinorama_select();
    app.spinorama_eq.step = SpinoramaStep::Results;

    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Optimize);
}

#[test]
fn spinorama_update_plugin_backtab_goes_to_results() {
    let mut app = app_on_spinorama_select();
    app.spinorama_eq.step = SpinoramaStep::UpdatePlugin;

    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Results);
}

#[test]
fn spinorama_esc_goes_to_step_tab_then_configure_tab() {
    let mut app = app_on_spinorama_select();
    app.spinorama_eq.step = SpinoramaStep::Configure;
    assert!(app.input_mode.is_configure_sub_screen());
    assert!(!app.spinorama_eq.step_tab_focused);

    // First Esc → step tab bar
    send_keys(&mut app, &[KeyCode::Esc]);
    assert!(app.spinorama_eq.step_tab_focused);
    assert!(app.input_mode.is_configure_sub_screen());
    assert_eq!(app.current_screen, Screen::Configure);

    // Second Esc → top-level configure tab bar
    send_keys(&mut app, &[KeyCode::Esc]);
    assert_eq!(app.input_mode, InputMode::Configure);
    assert!(!app.spinorama_eq.step_tab_focused);
    assert_eq!(app.current_screen, Screen::Configure);
}

#[test]
fn spinorama_configure_up_at_first_field_goes_to_step_tab() {
    let mut app = app_on_spinorama_select();
    app.spinorama_eq.step = SpinoramaStep::Configure;
    app.spinorama_eq.selected_field = 0;

    send_keys(&mut app, &[KeyCode::Up]);
    assert!(app.spinorama_eq.step_tab_focused);
    assert!(app.input_mode.is_configure_sub_screen());
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Configure);
}

#[test]
fn spinorama_step_tab_left_right_changes_step() {
    let mut app = app_on_spinorama_select();
    app.spinorama_eq.step = SpinoramaStep::Configure;
    app.spinorama_eq.step_tab_focused = true;

    // Right → Optimize
    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Optimize);
    assert!(app.spinorama_eq.step_tab_focused);

    // Right → Results
    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Results);

    // Left → back to Optimize
    send_keys(&mut app, &[KeyCode::Left]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Optimize);
}

#[test]
fn spinorama_step_tab_down_returns_to_content() {
    let mut app = app_on_spinorama_select();
    app.spinorama_eq.step = SpinoramaStep::Configure;
    app.spinorama_eq.step_tab_focused = true;

    send_keys(&mut app, &[KeyCode::Down]);
    assert!(!app.spinorama_eq.step_tab_focused);
    assert!(app.input_mode.is_configure_sub_screen());
}

#[test]
fn spinorama_step_tab_enter_returns_to_content() {
    let mut app = app_on_spinorama_select();
    app.spinorama_eq.step = SpinoramaStep::Optimize;
    app.spinorama_eq.step_tab_focused = true;

    send_keys(&mut app, &[KeyCode::Enter]);
    assert!(!app.spinorama_eq.step_tab_focused);
    assert!(app.input_mode.is_configure_sub_screen());
}

#[test]
fn spinorama_configure_backtab_goes_to_select() {
    let mut app = app_on_spinorama_select();
    app.spinorama_eq.step = SpinoramaStep::Configure;

    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Select);
}

#[test]
fn spinorama_configure_step_tab_left_goes_to_select() {
    // Left arrow from Configure goes back to Select via the step tab bar
    let mut app = app_on_spinorama_select();
    app.spinorama_eq.step = SpinoramaStep::Configure;
    app.spinorama_eq.selected_field = 0;

    // Up → step tab bar
    send_keys(&mut app, &[KeyCode::Up]);
    assert!(app.spinorama_eq.step_tab_focused);

    // Left → Select
    send_keys(&mut app, &[KeyCode::Left]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Select);
}

#[test]
fn room_eq_esc_goes_to_tab_bar() {
    let mut app = app_on_room_eq();
    app.room_eq.step = RoomEqStep::Configure;
    app.room_eq.step_tab_focused = false;
    assert!(app.input_mode.is_configure_sub_screen());

    // Esc from content → step tab bar
    send_keys(&mut app, &[KeyCode::Esc]);
    assert!(app.room_eq.step_tab_focused);

    // Esc from step tab → configure tab bar
    send_keys(&mut app, &[KeyCode::Esc]);
    assert_eq!(app.input_mode, InputMode::Configure);
}

#[test]
fn room_eq_optimize_backtab_goes_to_configure() {
    let mut app = app_on_room_eq();
    app.room_eq.step = RoomEqStep::Optimize;
    app.room_eq.step_tab_focused = false;
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.room_eq.step, RoomEqStep::Configure);
}

#[test]
fn room_eq_review_backtab_goes_to_optimize() {
    let mut app = app_on_room_eq();
    app.room_eq.step = RoomEqStep::Review;
    app.room_eq.step_tab_focused = false;
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.room_eq.step, RoomEqStep::Optimize);
}

#[test]
fn room_eq_export_backtab_goes_to_review() {
    let mut app = app_on_room_eq();
    app.room_eq.step = RoomEqStep::Export;
    app.room_eq.step_tab_focused = false;
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.room_eq.step, RoomEqStep::Review);
}

#[test]
fn room_eq_backtab_chain() {
    let mut app = app_on_room_eq();
    app.room_eq.step = RoomEqStep::Export;
    app.room_eq.step_tab_focused = false;

    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.room_eq.step, RoomEqStep::Review);
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.room_eq.step, RoomEqStep::Optimize);
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.room_eq.step, RoomEqStep::Configure);
}

#[test]
fn sequence_c_5_enter_reaches_spinorama_configure() {
    let mut app = app_on_library();
    app.spinorama_eq.available_speakers = vec!["Test Speaker".to_string()];
    app.spinorama_eq.update_filter();

    send_keys(&mut app, &[KeyCode::Char('C')]);
    assert_eq!(app.current_screen, Screen::Configure);
    assert_eq!(app.input_mode, InputMode::Configure);

    send_keys(&mut app, &[KeyCode::Char('5')]);
    assert_eq!(app.configure_sub_screen, ConfigureSubScreen::SpinoramaEq);
    assert!(app.input_mode.is_configure_sub_screen());
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Select);

    send_keys(&mut app, &[KeyCode::Enter]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Configure);
    assert_eq!(
        app.spinorama_eq.selected_speaker,
        Some("Test Speaker".to_string())
    );
}

#[test]
fn sequence_c_5_enter_esc_right_down_reaches_spinorama_optimize() {
    let mut app = app_on_library();
    app.spinorama_eq.available_speakers = vec!["Test Speaker".to_string()];
    app.spinorama_eq.update_filter();

    send_keys(
        &mut app,
        &[
            KeyCode::Char('C'), // → Configure
            KeyCode::Char('5'), // → SpinoramaEq, step=Select
            KeyCode::Enter,     // → select speaker → Configure step
            KeyCode::Esc,       // → step tab bar focused
            KeyCode::Right,     // → step tab: Configure → Optimize
            KeyCode::Down,      // → enter Optimize content
        ],
    );
    assert_eq!(app.current_screen, Screen::Configure);
    assert_eq!(app.configure_sub_screen, ConfigureSubScreen::SpinoramaEq);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Optimize);
    assert!(!app.spinorama_eq.step_tab_focused);
}

#[test]
fn sequence_c_5_enter_esc_right_right_down_reaches_spinorama_results() {
    let mut app = app_on_library();
    app.spinorama_eq.available_speakers = vec!["Test Speaker".to_string()];
    app.spinorama_eq.update_filter();

    send_keys(
        &mut app,
        &[
            KeyCode::Char('C'), // → Configure
            KeyCode::Char('5'), // → SpinoramaEq, step=Select
            KeyCode::Enter,     // → select speaker → Configure step
            KeyCode::Esc,       // → step tab bar focused
            KeyCode::Right,     // → step tab: Configure → Optimize
            KeyCode::Right,     // → step tab: Optimize → Results
            KeyCode::Down,      // → enter Results content
        ],
    );
    assert_eq!(app.current_screen, Screen::Configure);
    assert_eq!(app.configure_sub_screen, ConfigureSubScreen::SpinoramaEq);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Results);
    assert!(!app.spinorama_eq.step_tab_focused);
}

#[test]
fn sequence_spinorama_full_forward_and_back() {
    let mut app = app_on_library();
    app.spinorama_eq.available_speakers = vec!["Test Speaker".to_string()];
    app.spinorama_eq.update_filter();

    // Forward via step tab bar: Select → Configure → Optimize → Results → UpdatePlugin
    send_keys(
        &mut app,
        &[
            KeyCode::Char('C'),
            KeyCode::Char('5'),
            KeyCode::Enter, // Select → Configure
            KeyCode::Esc,   // → step tab bar
            KeyCode::Right, // Configure → Optimize
            KeyCode::Right, // Optimize → Results
            KeyCode::Right, // Results → UpdatePlugin
            KeyCode::Down,  // → enter UpdatePlugin content
        ],
    );
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::UpdatePlugin);
    assert!(!app.spinorama_eq.step_tab_focused);

    // Backward: UpdatePlugin → Results → Optimize → Configure
    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Results);

    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Optimize);

    send_keys(&mut app, &[KeyCode::BackTab]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Configure);
}

#[test]
fn sequence_tab_through_all_screens() {
    let mut app = app_on_library();

    let expected = [
        Screen::Queue,
        Screen::Playlists,
        Screen::Plugins,
        Screen::Devices,
    ];

    for &expected_screen in &expected {
        send_keys(&mut app, &[KeyCode::Tab]);
        assert_eq!(app.current_screen, expected_screen);
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.current_screen, Screen::Configure);
}

#[test]
fn sequence_esc_chain_from_spinorama_to_library() {
    let mut app = app_on_library();
    app.spinorama_eq.available_speakers = vec!["Test Speaker".to_string()];
    app.spinorama_eq.update_filter();

    // Navigate into Spinorama Configure step via keys
    send_keys(
        &mut app,
        &[
            KeyCode::Char('C'),
            KeyCode::Char('5'),
            KeyCode::Enter, // Select → Configure
        ],
    );
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Configure);
    assert!(app.input_mode.is_configure_sub_screen());

    // Esc from Spinorama content → step tab bar
    send_keys(&mut app, &[KeyCode::Esc]);
    assert!(app.spinorama_eq.step_tab_focused);
    assert!(app.input_mode.is_configure_sub_screen());
    assert_eq!(app.current_screen, Screen::Configure);

    // Esc from step tab bar → configure tab bar
    send_keys(&mut app, &[KeyCode::Esc]);
    assert_eq!(app.input_mode, InputMode::Configure);
    assert!(!app.spinorama_eq.step_tab_focused);
    assert_eq!(app.current_screen, Screen::Configure);

    // Esc from Configure tab bar → Library
    send_keys(&mut app, &[KeyCode::Esc]);
    assert_eq!(app.current_screen, Screen::Library);
}

#[test]
fn plugins_screen_enter_starts_edit_mode() {
    let mut app = app_on_library();
    app.current_screen = Screen::Plugins;

    // 'e' or Enter enters edit mode (only if there is a plugin selected)
    // With the default plugin chain there should be plugins
    if !app.plugin_graph.is_empty() {
        send_keys(&mut app, &[KeyCode::Char('e')]);
        assert_eq!(app.input_mode, InputMode::EditPlugin);
    }
}

#[test]
fn plugins_edit_mode_esc_returns_to_normal() {
    let mut app = app_on_library();
    app.current_screen = Screen::Plugins;
    app.input_mode = InputMode::EditPlugin;
    app.editing_plugin_index = Some(0);

    send_keys(&mut app, &[KeyCode::Esc]);
    assert_eq!(app.input_mode, InputMode::Normal);
}

#[test]
fn library_search_then_esc_returns_to_normal() {
    let mut app = app_on_library();

    // / → search mode
    send_keys(&mut app, &[KeyCode::Char('/')]);
    assert_eq!(app.input_mode, InputMode::Search);

    // Type a query
    send_keys(&mut app, &[KeyCode::Char('t'), KeyCode::Char('e')]);
    assert_eq!(app.search_query, "te");

    // Esc → back to normal
    send_keys(&mut app, &[KeyCode::Esc]);
    assert_eq!(app.input_mode, InputMode::Normal);
    // Search query is preserved
    assert_eq!(app.search_query, "te");
}

#[test]
fn spinorama_step_tab_right_cycles_all_steps() {
    let mut app = app_on_spinorama_select();
    app.spinorama_eq.step_tab_focused = true;

    // Right chain: Select → Configure → Optimize → Results → UpdatePlugin → Select (wrap)
    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Configure);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Optimize);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Results);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::UpdatePlugin);

    // Wraps back to Select
    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Select);
}

#[test]
fn spinorama_step_tab_left_wraps_from_select_to_update_plugin() {
    let mut app = app_on_spinorama_select();
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Select);
    app.spinorama_eq.step_tab_focused = true;

    send_keys(&mut app, &[KeyCode::Left]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::UpdatePlugin);
}

#[test]
fn spinorama_step_tab_left_right_round_trip() {
    let mut app = app_on_spinorama_select();
    app.spinorama_eq.step = SpinoramaStep::Optimize;
    app.spinorama_eq.step_tab_focused = true;

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Results);

    send_keys(&mut app, &[KeyCode::Left]);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Optimize);
}

#[test]
fn room_eq_step_tab_right_cycles_all_steps() {
    let mut app = app_on_room_eq();
    app.room_eq.step = RoomEqStep::LoadData;
    app.room_eq.step_tab_focused = true;

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.room_eq.step, RoomEqStep::Delay);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.room_eq.step, RoomEqStep::Process);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.room_eq.step, RoomEqStep::Configure);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.room_eq.step, RoomEqStep::Optimize);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.room_eq.step, RoomEqStep::Review);

    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.room_eq.step, RoomEqStep::Export);

    // Wraps back to LoadData
    send_keys(&mut app, &[KeyCode::Right]);
    assert_eq!(app.room_eq.step, RoomEqStep::LoadData);
}

#[test]
fn room_eq_step_tab_left_wraps_from_load_data_to_export() {
    let mut app = app_on_room_eq();
    app.room_eq.step = RoomEqStep::LoadData;
    app.room_eq.step_tab_focused = true;

    send_keys(&mut app, &[KeyCode::Left]);
    assert_eq!(app.room_eq.step, RoomEqStep::Export);
}

#[test]
fn spinorama_configure_tab_cycles_fields() {
    let mut app = app_on_spinorama_select();
    app.spinorama_eq.step = SpinoramaStep::Configure;
    app.spinorama_eq.selected_field = 0;

    // Tab moves to next field
    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.spinorama_eq.selected_field, 1);
    // Still on Configure step (not advanced to Optimize)
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Configure);

    // Tab at last field wraps to 0
    app.spinorama_eq.selected_field = 24;
    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.spinorama_eq.selected_field, 0);
    assert_eq!(app.spinorama_eq.step, SpinoramaStep::Configure);
}

#[test]
fn room_eq_configure_tab_cycles_fields() {
    let mut app = app_on_room_eq();
    app.room_eq.step = RoomEqStep::Configure;
    app.room_eq.step_tab_focused = false;
    app.room_eq.selected_field = 0;

    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.room_eq.selected_field, 1);
    assert_eq!(app.room_eq.step, RoomEqStep::Configure);

    // Wrap at max field (23)
    app.room_eq.selected_field = 28;
    send_keys(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.room_eq.selected_field, 0);
    assert_eq!(app.room_eq.step, RoomEqStep::Configure);
}
