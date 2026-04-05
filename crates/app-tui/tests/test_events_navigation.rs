//! Navigation integration tests.
//!
//! Verify that key sequences drive the TUI through the expected screen,
//! input-mode, and wizard-step transitions.

use super::PlayerCommand;
use super::handle_key_event;
use super::tests::{key, make_app};
use crate::app::{ConfigureSubScreen, HeadphoneEqStep, InputMode, Screen, SpinoramaStep};
use crossterm::event::KeyCode;
use sotf_audio_player::recording_types::RecordingStep;
use sotf_audio_player::room_eq_types::RoomEqStep;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Send a sequence of plain key presses (no modifiers).
fn send_keys(app: &mut crate::app::App, codes: &[KeyCode]) {
    for &code in codes {
        handle_key_event(app, key(code));
    }
}

/// Create an app already on the Library screen (past Loading).
fn app_on_library() -> crate::app::App {
    let mut app = make_app();
    app.current_screen = Screen::Library;
    app
}

/// Create an app on Configure > SpinoramaEq with speakers loaded,
/// ready for wizard navigation tests.
fn app_on_spinorama_select() -> crate::app::App {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.configure_sub_screen = ConfigureSubScreen::SpinoramaEq;
    app.input_mode = InputMode::ConfigureSpinoramaEq;
    app.spinorama_eq.step = SpinoramaStep::Select;
    // Pre-populate speaker list so Enter can select one
    app.spinorama_eq.available_speakers = vec![
        "Speaker A".to_string(),
        "Speaker B".to_string(),
        "Speaker C".to_string(),
    ];
    app.spinorama_eq.update_filter();
    app
}

/// Create an app on Configure > RoomEq, tab content focused.
fn app_on_room_eq() -> crate::app::App {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.configure_sub_screen = ConfigureSubScreen::RoomEq;
    app.input_mode = InputMode::ConfigureRoomEq;
    app.room_eq.step_tab_focused = true;
    app
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 1. Direct screen switching (uppercase letters) ───────────────────────────

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

    // ── 2. Tab cycling through screens ───────────────────────────────────────────

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

    // ── 3. Esc navigation ───────────────────────────────────────────────────────

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
    }

    #[test]
    fn esc_from_meters_goes_to_normal() {
        let mut app = app_on_library();
        app.input_mode = InputMode::LevelMeters;

        send_keys(&mut app, &[KeyCode::Esc]);
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(!app.should_quit);
    }

    // ── 4. Configure sub-screen navigation ──────────────────────────────────────

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

        // Wraps back to Directories
        send_keys(&mut app, &[KeyCode::Right]);
        assert_eq!(app.configure_sub_screen, ConfigureSubScreen::Directories);

        // And Left wraps backwards
        send_keys(&mut app, &[KeyCode::Left]);
        assert_eq!(app.configure_sub_screen, ConfigureSubScreen::Servers);
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

    // ── 5. Input mode transitions ────────────────────────────────────────────────

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

    // ── 6. Spinorama wizard navigation ──────────────────────────────────────────

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

    // ── 6a-2. Spinorama step tab bar navigation ──────────────────────────────────

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

    // ── 6a-3. Spinorama Configure → Select navigation ───────────────────────────

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

    // ── 6b. BackTab navigates back in wizards ─────────────────────────────────

    // ── 7. BackTab goes back in all wizards ──────────────────────────────────────
    //
    // BackTab from content goes to the previous step.
    // Left/Right adjusts values within the current step.

    // ── 7a. Room EQ ──

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

    // ── 7b. Headphone EQ ──

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

    // ── 7c. Recording ──

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

    // ── 8. Full navigation sequences (keys only, no manual state mutation) ───────

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

    // ── 9. Focused pane navigation ───────────────────────────────────────────────

    // ── 10. Plugins screen navigation ────────────────────────────────────────────

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

    // ── 11. Library screen navigation ────────────────────────────────────────────

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

    // ── 9. Channel conflict dialog ───────────────────────────────────────────────

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

    // ── 12. Step tab bar Left/Right cycles steps (all wizards) ───────────────
    //
    // Left/Right on the step tab bar navigate between wizard steps with wrapping.
    // From content, Esc focuses the step tab bar first.

    // ── 12a. Spinorama ──

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

    // ── 12b. Room EQ ──

    #[test]
    fn room_eq_step_tab_right_cycles_all_steps() {
        let mut app = app_on_room_eq();
        app.room_eq.step = RoomEqStep::LoadData;
        app.room_eq.step_tab_focused = true;

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

    // ── 12c. Headphone EQ ──

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

    // ── 12d. Recording ──

    #[test]
    fn recording_step_tab_right_cycles_all_steps() {
        let mut app = app_on_recording();
        app.recording.output_directory = "/tmp/test".to_string();
        app.recording.step_tab_focused = true;

        send_keys(&mut app, &[KeyCode::Right]);
        assert_eq!(app.recording.step, RecordingStep::Capture);

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

    // ── 13. Tab cycles fields within Configure-type steps ────────────────────

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
        app.room_eq.selected_field = 23;
        send_keys(&mut app, &[KeyCode::Tab]);
        assert_eq!(app.room_eq.selected_field, 0);
        assert_eq!(app.room_eq.step, RoomEqStep::Configure);
    }

    #[test]
    fn headphone_eq_configure_tab_cycles_fields() {
        let mut app = app_on_headphone_eq();
        app.headphone_eq.step = HeadphoneEqStep::Configure;
        app.headphone_eq.config_selected_field = 0;

        send_keys(&mut app, &[KeyCode::Tab]);
        assert_eq!(app.headphone_eq.config_selected_field, 1);
        assert_eq!(app.headphone_eq.step, HeadphoneEqStep::Configure);

        // Wrap at max field (17)
        app.headphone_eq.config_selected_field = 17;
        send_keys(&mut app, &[KeyCode::Tab]);
        assert_eq!(app.headphone_eq.config_selected_field, 0);
        assert_eq!(app.headphone_eq.step, HeadphoneEqStep::Configure);
    }

    #[test]
    fn recording_config_tab_cycles_fields() {
        let mut app = app_on_recording();
        app.recording.step = RecordingStep::Config;
        app.recording.selected_field = 0;

        send_keys(&mut app, &[KeyCode::Tab]);
        assert_eq!(app.recording.selected_field, 1);
        assert_eq!(app.recording.step, RecordingStep::Config);

        // Wrap at max field (9)
        app.recording.selected_field = 9;
        send_keys(&mut app, &[KeyCode::Tab]);
        assert_eq!(app.recording.selected_field, 0);
        assert_eq!(app.recording.step, RecordingStep::Config);
    }

    // ── Directory autocomplete arrow navigation ─────────────────────────

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
}
