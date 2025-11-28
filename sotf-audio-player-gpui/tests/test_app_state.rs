// ============================================================================
// App State Management Tests
// ============================================================================
//
// Tests for core app state management including:
// - Input mode transitions
// - Toast message updates
// - Directory management
// - Plugin selection and editing

use sotf_audio_player_gpui::app::{App, InputMode, Screen, ToastMessage, ToastType};

fn create_test_app() -> App {
    App::new()
}

#[test]
fn test_app_initial_state() {
    let app = create_test_app();

    assert_eq!(app.current_screen, Screen::Library);
    assert_eq!(app.input_mode, InputMode::Normal);
    assert!(app.toast_message.is_none());
    assert_eq!(app.selected_album_index, 0);
    assert!(!app.is_playing);
    assert!(!app.scan_in_progress);
}

#[test]
fn test_input_mode_transitions() {
    let mut app = create_test_app();

    // Normal -> Search
    app.input_mode = InputMode::Search;
    assert_eq!(app.input_mode, InputMode::Search);

    // Search -> Normal
    app.input_mode = InputMode::Normal;
    assert_eq!(app.input_mode, InputMode::Normal);

    // Normal -> AddDirectory
    app.input_mode = InputMode::AddDirectory;
    assert_eq!(app.input_mode, InputMode::AddDirectory);

    // AddDirectory -> Normal
    app.input_mode = InputMode::Normal;
    assert_eq!(app.input_mode, InputMode::Normal);
}

#[test]
fn test_screen_transitions() {
    let mut app = create_test_app();

    app.current_screen = Screen::Queue;
    assert_eq!(app.current_screen, Screen::Queue);

    // app.current_screen = Screen::Plugins;
    // assert_eq!(app.current_screen, Screen::Plugins);

    // app.current_screen = Screen::Devices;
    // assert_eq!(app.current_screen, Screen::Devices);

    app.current_screen = Screen::DirectoryManager;
    assert_eq!(app.current_screen, Screen::DirectoryManager);

    app.current_screen = Screen::Library;
    assert_eq!(app.current_screen, Screen::Library);
}

#[test]
fn test_toast_message_updates() {
    let mut app = create_test_app();

    // Initially no toast
    assert!(app.toast_message.is_none());

    // Set success toast
    app.toast_message = Some(ToastMessage::success("Test success"));
    assert!(app.toast_message.is_some());
    if let Some(ref toast) = app.toast_message {
        assert_eq!(toast.message, "Test success");
        assert_eq!(toast.toast_type, ToastType::Success);
    }

    // Update to error toast
    app.toast_message = Some(ToastMessage::error("Test error"));
    if let Some(ref toast) = app.toast_message {
        assert_eq!(toast.message, "Test error");
        assert_eq!(toast.toast_type, ToastType::Error);
    }

    // Dismiss toast
    app.dismiss_toast();
    assert!(app.toast_message.is_none());
}

#[test]
fn test_toast_auto_dismiss_check() {
    let mut app = create_test_app();

    // Set a toast that will auto-dismiss
    app.toast_message = Some(ToastMessage::success("Auto-dismiss test"));

    // update_toast should not dismiss immediately
    app.update_toast();
    assert!(app.toast_message.is_some());

    // Set a persistent toast
    app.toast_message = Some(ToastMessage::persistent("Persistent", ToastType::Info));
    app.update_toast();
    assert!(app.toast_message.is_some()); // Should never auto-dismiss
}

#[test]
fn test_search_functionality() {
    let mut app = create_test_app();

    // Enter search mode
    app.input_mode = InputMode::Search;
    app.search_query = "test album".to_string();

    assert_eq!(app.search_query, "test album");
    assert_eq!(app.input_mode, InputMode::Search);

    // Clear search
    app.search_query.clear();
    app.input_mode = InputMode::Normal;

    assert_eq!(app.search_query, "");
    assert_eq!(app.input_mode, InputMode::Normal);
}

#[test]
fn test_file_input_state() {
    let mut app = create_test_app();

    // APO file input
    app.input_mode = InputMode::LoadApoFile;
    app.apo_file_input = "/path/to/eq.txt".to_string();

    assert_eq!(app.input_mode, InputMode::LoadApoFile);
    assert_eq!(app.apo_file_input, "/path/to/eq.txt");

    // Clear APO input
    app.apo_file_input.clear();
    app.input_mode = InputMode::Normal;

    assert_eq!(app.apo_file_input, "");

    // SOFA file input
    app.input_mode = InputMode::LoadSofaFile;
    app.sofa_file_input = "/path/to/hrtf.sofa".to_string();

    assert_eq!(app.input_mode, InputMode::LoadSofaFile);
    assert_eq!(app.sofa_file_input, "/path/to/hrtf.sofa");

    // Clear SOFA input
    app.sofa_file_input.clear();
    app.input_mode = InputMode::Normal;

    assert_eq!(app.sofa_file_input, "");
}

#[test]
fn test_directory_input_state() {
    let mut app = create_test_app();

    app.input_mode = InputMode::AddDirectory;
    app.directory_input = "/home/user/Music".to_string();

    assert_eq!(app.input_mode, InputMode::AddDirectory);
    assert_eq!(app.directory_input, "/home/user/Music");

    app.directory_input.clear();
    assert_eq!(app.directory_input, "");
}

#[test]
fn test_autocomplete_state() {
    let mut app = create_test_app();

    // Set autocomplete suggestions
    app.autocomplete_suggestions = vec![
        "/home/user/Music".to_string(),
        "/home/user/Documents".to_string(),
        "/home/user/Downloads".to_string(),
    ];
    app.autocomplete_index = 1;

    assert_eq!(app.autocomplete_suggestions.len(), 3);
    assert_eq!(app.autocomplete_index, 1);

    // Clear autocomplete
    app.clear_autocomplete();

    assert_eq!(app.autocomplete_suggestions.len(), 0);
    assert_eq!(app.autocomplete_index, 0);
}

#[test]
fn test_plugin_selection() {
    let mut app = create_test_app();

    // Initially no plugin editing
    assert!(app.editing_plugin_index.is_none());
    assert_eq!(app.plugin_param_selection, 0);

    // Enter plugin edit mode
    app.editing_plugin_index = Some(0);
    app.plugin_param_selection = 2;

    assert_eq!(app.editing_plugin_index, Some(0));
    assert_eq!(app.plugin_param_selection, 2);

    // Exit plugin edit mode
    app.editing_plugin_index = None;
    app.plugin_param_selection = 0;

    assert!(app.editing_plugin_index.is_none());
}

#[test]
fn test_scan_progress_state() {
    let mut app = create_test_app();

    assert!(!app.scan_in_progress);
    assert_eq!(app.scan_progress_tracks, 0);
    assert_eq!(app.scan_progress_albums, 0);

    // Start scan
    app.start_library_scan();

    assert!(app.scan_in_progress);
    assert_eq!(app.scan_progress_tracks, 0);
    assert_eq!(app.scan_progress_albums, 0);
    assert!(app.toast_message.is_some());

    // Update progress
    app.scan_progress_tracks = 100;
    app.scan_progress_albums = 10;

    assert_eq!(app.scan_progress_tracks, 100);
    assert_eq!(app.scan_progress_albums, 10);
}

#[test]
fn test_playback_state() {
    let mut app = create_test_app();

    assert!(!app.is_playing);

    app.is_playing = true;
    assert!(app.is_playing);

    app.is_playing = false;
    assert!(!app.is_playing);
}

#[test]
fn test_volume_state() {
    let mut app = create_test_app();

    // Default volume should be reasonable (e.g., 0.7 or 70%)
    assert!(app.volume >= 0.0 && app.volume <= 1.0);

    app.volume = 0.5;
    assert_eq!(app.volume, 0.5);

    app.volume = 1.0;
    assert_eq!(app.volume, 1.0);

    app.volume = 0.0;
    assert_eq!(app.volume, 0.0);
}

#[test]
fn test_needs_plugin_update_flag() {
    let mut app = create_test_app();

    assert!(!app.needs_plugin_update);

    app.needs_plugin_update = true;
    assert!(app.needs_plugin_update);

    app.needs_plugin_update = false;
    assert!(!app.needs_plugin_update);
}

// ============================================================================
// Bug Fix Tests
// ============================================================================

#[test]
fn test_default_sort_order_is_album() {
    let app = create_test_app();

    // Bug fix: Default sort order should be Album, not Artist
    use sotf_audio_player_gpui::app::LibrarySortOrder;
    assert_eq!(app.library_sort_order, LibrarySortOrder::Album);
}

#[test]
fn test_tree_view_groups_by_sort_order() {
    use sotf_audio_player::Album;
    use sotf_audio_player_gpui::app::{LibrarySortOrder, LibraryViewMode};

    let mut app = create_test_app();

    // Add test albums with different first letters in title vs artist
    app.library.albums.push(Album {
        id: None,
        artist: "Zebra Band".to_string(),
        title: "Album One".to_string(),
        year: Some(2020),
        tracks: vec![],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    });

    app.library.albums.push(Album {
        id: None,
        artist: "Apple Artists".to_string(),
        title: "Zebra Album".to_string(),
        year: Some(2021),
        tracks: vec![],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    });

    // Switch to tree view
    app.library_view_mode = LibraryViewMode::TreeView;

    // Test grouping by Artist (default)
    app.library_sort_order = LibrarySortOrder::Artist;
    app.rebuild_letter_tree();

    // Should group by first letter of artist: 'A' (Apple Artists) and 'Z' (Zebra Band)
    assert_eq!(app.letter_tree.len(), 2);
    assert!(app.letter_tree.iter().any(|node| node.letter == 'A'));
    assert!(app.letter_tree.iter().any(|node| node.letter == 'Z'));

    // Test grouping by Album title
    app.library_sort_order = LibrarySortOrder::Album;
    app.rebuild_letter_tree();

    // Bug fix: Should group by first letter of album TITLE: 'A' (Album One) and 'Z' (Zebra Album)
    assert_eq!(app.letter_tree.len(), 2);
    let letter_a = app.letter_tree.iter().find(|node| node.letter == 'A').unwrap();
    let letter_z = app.letter_tree.iter().find(|node| node.letter == 'Z').unwrap();

    // Verify correct grouping: "Album One" under 'A', "Zebra Album" under 'Z'
    assert_eq!(letter_a.album_indices.len(), 1);
    assert_eq!(letter_z.album_indices.len(), 1);
    assert_eq!(app.library.albums[letter_a.album_indices[0]].title, "Album One");
    assert_eq!(app.library.albums[letter_z.album_indices[0]].title, "Zebra Album");
}

#[test]
fn test_tree_view_album_selection() {
    use sotf_audio_player::{Album, Track};
    use sotf_audio_player_gpui::app::{LibrarySortOrder, LibraryViewMode};

    let mut app = create_test_app();

    // Add a test album
    let tracks = vec![Track {
        title: Some("Track 1".to_string()),
        path: "/path/to/track1.flac".into(),
        duration_secs: Some(180),
        track_number: Some(1),
        channels: Some(2),
        replay_gain: None,
        replay_peak: None,
        album_gain: None,
        album_peak: None,
        waveform: None,
        genre: None,
        composer: None,
        disc_number: None,
        conductor: None,
        performer: None,
        isrc: None,
        album_artist: None,
        ensemble: None,
    }];

    app.library.albums.push(Album {
        id: None,
        artist: "Test Artist".to_string(),
        title: "Test Album".to_string(),
        year: Some(2020),
        tracks,
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    });

    // Switch to tree view and build tree
    app.library_view_mode = LibraryViewMode::TreeView;
    app.library_sort_order = LibrarySortOrder::Album;
    app.rebuild_letter_tree();

    // Expand the first letter group
    app.selected_tree_index = 0;
    app.toggle_letter_expansion();

    let tree_items = app.get_tree_items();

    // Should have: Letter header + Album
    assert_eq!(tree_items.len(), 2);

    // Select the album item (index 1 in tree)
    app.selected_tree_index = 1;

    // Bug fix: This should not crash and should add the correct album to queue
    let result = app.add_album_to_queue();

    // Should successfully add album
    assert!(result.is_some());
    assert_eq!(app.queue.len(), 1);
    assert_eq!(app.queue[0].album.title, "Test Album");
}

#[test]
fn test_tree_view_letter_selection_does_not_add_to_queue() {
    use sotf_audio_player::Album;
    use sotf_audio_player_gpui::app::LibraryViewMode;

    let mut app = create_test_app();

    // Add a test album
    app.library.albums.push(Album {
        id: None,
        artist: "Artist".to_string(),
        title: "Album".to_string(),
        year: Some(2020),
        tracks: vec![],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    });

    // Switch to tree view
    app.library_view_mode = LibraryViewMode::TreeView;
    app.rebuild_letter_tree();

    // Select letter header (index 0)
    app.selected_tree_index = 0;

    // Trying to add a letter header should do nothing
    let result = app.add_album_to_queue();
    assert!(result.is_none());
    assert_eq!(app.queue.len(), 0);
}
