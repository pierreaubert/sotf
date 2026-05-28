use sotf_audio_player::MusicLibrary;
use sotf_audio_player::federation_config::{FederationSourceEntry, SourceConnectionConfig};
use sotf_audio_player_gpui::App;
use sotf_audio_player_gpui::app::state::LibraryState;

fn test_app_with_database() -> (tempfile::TempDir, App) {
    let temp_dir = tempfile::tempdir().expect("temp db dir");
    let db_path = temp_dir.path().join("library.sqlite");
    let library = MusicLibrary::with_custom_database_for_testing(&db_path).expect("test database");

    let mut app = App::new();
    app.library_state = LibraryState::with_library(library);
    (temp_dir, app)
}

fn source() -> FederationSourceEntry {
    FederationSourceEntry {
        source_id: "source_1".to_string(),
        display_name: "Primary source".to_string(),
        priority: 0,
        is_enabled: false,
        connection: SourceConnectionConfig::default_for_type("mpd"),
        is_available: None,
    }
}

#[test]
fn toggle_federation_source_persists_enabled_state() {
    let (_temp_dir, mut app) = test_app_with_database();
    app.federation.sources.push(source());

    app.toggle_federation_source(0);

    assert!(app.federation.sources[0].is_enabled);
    let saved = app
        .library_state
        .library
        .get_database()
        .unwrap()
        .load_federation_sources()
        .unwrap();
    let saved = saved
        .into_iter()
        .find(|source| source.source_id == "source_1")
        .expect("persisted federation source");
    assert!(saved.is_enabled);
}

#[test]
fn federation_source_edits_persist_to_database() {
    let (_temp_dir, mut app) = test_app_with_database();
    app.federation.sources.push(source());

    app.update_federation_source_name(0, "Living Room MPD");
    app.update_federation_source_field(0, 0, "musicbox.local");

    let saved = app
        .library_state
        .library
        .get_database()
        .unwrap()
        .load_federation_sources()
        .unwrap();
    let saved = saved
        .into_iter()
        .find(|source| source.source_id == "source_1")
        .expect("persisted federation source");
    assert_eq!(saved.display_name, "Living Room MPD");
    assert_eq!(saved.connection.field_value(0), "musicbox.local");
}
