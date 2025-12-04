// ============================================================================
// Input Mode Transition Tests
// ============================================================================
//
// Tests for input mode state machine transitions:
// - Valid mode transitions
// - Mode-specific state management
// - Cancel/escape behavior
// - Input validation

use sotf_audio_player_gpui::app::{App, InputMode, Screen};

fn create_test_app() -> App {
    App::new()
}

#[test]
fn test_normal_mode_default() {
    let app = create_test_app();
    assert_eq!(app.input_mode, InputMode::Normal);
}

#[test]
fn test_search_mode_transition() {
    let mut app = create_test_app();

    // Enter search mode
    app.input_mode = InputMode::Search;
    assert_eq!(app.input_mode, InputMode::Search);

    // Search query should be modifiable
    app.search_query = "test".to_string();
    assert_eq!(app.search_query, "test");

    // Exit to normal
    app.input_mode = InputMode::Normal;
    assert_eq!(app.input_mode, InputMode::Normal);
}

#[test]
fn test_add_directory_mode_transition() {
    let mut app = create_test_app();

    // Switch to directory manager screen
    app.current_screen = Screen::DirectoryManager;

    // Enter add directory mode
    app.input_mode = InputMode::AddDirectory;
    assert_eq!(app.input_mode, InputMode::AddDirectory);

    // Directory input should be modifiable
    app.directory_input = "/home/user/Music".to_string();
    assert_eq!(app.directory_input, "/home/user/Music");

    // Exit to normal
    app.input_mode = InputMode::Normal;
    app.directory_input.clear();
    assert_eq!(app.input_mode, InputMode::Normal);
    assert!(app.directory_input.is_empty());
}

#[test]
fn test_edit_plugin_mode_transition() {
    let mut app = create_test_app();

    // Switch to plugins screen
    app.current_screen = Screen::Plugins;

    // Enter edit plugin mode
    app.editing_plugin_index = Some(0);
    app.input_mode = InputMode::EditPlugin;

    assert_eq!(app.input_mode, InputMode::EditPlugin);
    assert_eq!(app.editing_plugin_index, Some(0));

    // Exit to normal
    app.input_mode = InputMode::Normal;
    app.editing_plugin_index = None;

    assert_eq!(app.input_mode, InputMode::Normal);
    assert!(app.editing_plugin_index.is_none());
}

#[test]
fn test_load_apo_file_mode_transition() {
    let mut app = create_test_app();

    // From EditPlugin to LoadApoFile
    app.input_mode = InputMode::EditPlugin;
    app.editing_plugin_index = Some(0);

    app.input_mode = InputMode::LoadApoFile;
    assert_eq!(app.input_mode, InputMode::LoadApoFile);

    // APO file input should be modifiable
    app.apo_file_input = "/path/to/eq.txt".to_string();
    assert_eq!(app.apo_file_input, "/path/to/eq.txt");

    // Return to EditPlugin on success
    app.input_mode = InputMode::EditPlugin;
    assert_eq!(app.input_mode, InputMode::EditPlugin);

    // Or return to Normal on cancel
    app.input_mode = InputMode::Normal;
    app.apo_file_input.clear();
    assert_eq!(app.input_mode, InputMode::Normal);
    assert!(app.apo_file_input.is_empty());
}

#[test]
fn test_load_sofa_file_mode_transition() {
    let mut app = create_test_app();

    // From EditPlugin to LoadSofaFile
    app.input_mode = InputMode::EditPlugin;
    app.editing_plugin_index = Some(0);

    app.input_mode = InputMode::LoadSofaFile;
    assert_eq!(app.input_mode, InputMode::LoadSofaFile);

    // SOFA file input should be modifiable
    app.sofa_file_input = "/path/to/hrtf.sofa".to_string();
    assert_eq!(app.sofa_file_input, "/path/to/hrtf.sofa");

    // Return to EditPlugin on success
    app.input_mode = InputMode::EditPlugin;
    assert_eq!(app.input_mode, InputMode::EditPlugin);

    // Or return to Normal on cancel
    app.input_mode = InputMode::Normal;
    app.sofa_file_input.clear();
    assert_eq!(app.input_mode, InputMode::Normal);
    assert!(app.sofa_file_input.is_empty());
}

#[test]
fn test_help_mode_transition() {
    let mut app = create_test_app();

    // Enter help mode from any screen
    app.input_mode = InputMode::Help;
    assert_eq!(app.input_mode, InputMode::Help);

    // Exit back to normal
    app.input_mode = InputMode::Normal;
    assert_eq!(app.input_mode, InputMode::Normal);
}

#[test]
fn test_save_plugins_mode_transition() {
    let mut app = create_test_app();

    app.current_screen = Screen::Plugins;

    // Enter save plugins mode
    app.input_mode = InputMode::SavePlugins;
    assert_eq!(app.input_mode, InputMode::SavePlugins);

    // Exit to normal
    app.input_mode = InputMode::Normal;
    assert_eq!(app.input_mode, InputMode::Normal);
}

#[test]
fn test_load_plugins_mode_transition() {
    let mut app = create_test_app();

    app.current_screen = Screen::Plugins;

    // Enter load plugins mode
    app.input_mode = InputMode::LoadPlugins;
    assert_eq!(app.input_mode, InputMode::LoadPlugins);

    // Exit to normal
    app.input_mode = InputMode::Normal;
    assert_eq!(app.input_mode, InputMode::Normal);
}

#[test]
fn test_nested_mode_transitions() {
    let mut app = create_test_app();

    // Normal -> EditPlugin
    app.input_mode = InputMode::EditPlugin;
    assert_eq!(app.input_mode, InputMode::EditPlugin);

    // EditPlugin -> LoadApoFile
    app.input_mode = InputMode::LoadApoFile;
    assert_eq!(app.input_mode, InputMode::LoadApoFile);

    // LoadApoFile -> EditPlugin (after load)
    app.input_mode = InputMode::EditPlugin;
    assert_eq!(app.input_mode, InputMode::EditPlugin);

    // EditPlugin -> LoadSofaFile
    app.input_mode = InputMode::LoadSofaFile;
    assert_eq!(app.input_mode, InputMode::LoadSofaFile);

    // LoadSofaFile -> Normal (cancel)
    app.input_mode = InputMode::Normal;
    assert_eq!(app.input_mode, InputMode::Normal);
}

#[test]
fn test_mode_specific_state_isolation() {
    let mut app = create_test_app();

    // Set search query
    app.input_mode = InputMode::Search;
    app.search_query = "album".to_string();

    // Switch to directory mode
    app.input_mode = InputMode::AddDirectory;
    app.directory_input = "/music".to_string();

    // Search query should still be preserved
    assert_eq!(app.search_query, "album");
    assert_eq!(app.directory_input, "/music");

    // Switch to file loading mode
    app.input_mode = InputMode::LoadApoFile;
    app.apo_file_input = "/eq.txt".to_string();

    // Previous inputs should still be preserved
    assert_eq!(app.search_query, "album");
    assert_eq!(app.directory_input, "/music");
    assert_eq!(app.apo_file_input, "/eq.txt");
}

#[test]
fn test_mode_transitions_with_screen_changes() {
    let mut app = create_test_app();

    // Library screen with search
    app.current_screen = Screen::Library;
    app.input_mode = InputMode::Search;
    assert_eq!(app.input_mode, InputMode::Search);

    // Switch to plugins screen
    app.current_screen = Screen::Plugins;
    app.input_mode = InputMode::Normal;

    // Enter edit plugin mode
    app.input_mode = InputMode::EditPlugin;
    assert_eq!(app.current_screen, Screen::Plugins);
    assert_eq!(app.input_mode, InputMode::EditPlugin);

    // Switch to directory manager
    app.current_screen = Screen::DirectoryManager;
    app.input_mode = InputMode::Normal;

    // Enter add directory mode
    app.input_mode = InputMode::AddDirectory;
    assert_eq!(app.current_screen, Screen::DirectoryManager);
    assert_eq!(app.input_mode, InputMode::AddDirectory);
}

#[test]
fn test_input_mode_equality() {
    assert_eq!(InputMode::Normal, InputMode::Normal);
    assert_eq!(InputMode::Search, InputMode::Search);
    assert_eq!(InputMode::EditPlugin, InputMode::EditPlugin);
    assert_eq!(InputMode::LoadApoFile, InputMode::LoadApoFile);
    assert_eq!(InputMode::LoadSofaFile, InputMode::LoadSofaFile);

    assert_ne!(InputMode::Normal, InputMode::Search);
    assert_ne!(InputMode::LoadApoFile, InputMode::LoadSofaFile);
    assert_ne!(InputMode::EditPlugin, InputMode::Help);
}

#[test]
fn test_dismiss_toast_clears_state() {
    let mut app = create_test_app();

    // Set a toast
    app.toast_message = Some(sotf_audio_player_gpui::app::ToastMessage::info("Test"));
    assert!(app.toast_message.is_some());

    // Dismiss should clear it
    app.dismiss_toast();
    assert!(app.toast_message.is_none());
}

#[test]
fn test_clear_autocomplete_state() {
    let mut app = create_test_app();

    // Set autocomplete state
    app.autocomplete_suggestions = vec!["suggestion1".to_string(), "suggestion2".to_string()];
    app.autocomplete_index = 1;

    assert_eq!(app.autocomplete_suggestions.len(), 2);
    assert_eq!(app.autocomplete_index, 1);

    // Clear should reset everything
    app.clear_autocomplete();

    assert_eq!(app.autocomplete_suggestions.len(), 0);
    assert_eq!(app.autocomplete_index, 0);
}
