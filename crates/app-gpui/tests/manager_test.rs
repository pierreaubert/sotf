use gpui::TestAppContext;
use sotf_audio_player::{Album, ChannelFilter, Track};
use sotf_audio_player_gpui::app::manager::Manager;
use sotf_audio_player_gpui::app::state::app::{App, AppMessage};
use sotf_audio_player_gpui::app::state::library::{LibraryEvent, LibraryQuery, LibraryResponse};
use std::path::PathBuf;

#[gpui::test]
fn test_manager_protocol(_cx: &mut TestAppContext) {
    // 1. Initialize App
    let mut app = App::new();

    // 2. Initial State Verification
    assert_eq!(
        app.library_state.search_query, "",
        "Search query should be empty initially"
    );

    // 3. Dispatch Event via App::dispatch
    let event = LibraryEvent::SetSearchQuery("jazz".to_string());
    let msg = AppMessage::Library(event);

    let result = app.dispatch(msg);
    assert!(result.is_ok(), "Dispatch should succeed");

    // 4. Verify State Change
    assert_eq!(
        app.library_state.search_query, "jazz",
        "Search query should be updated"
    );

    // 5. Verify Query
    let count_query = LibraryQuery::ItemCount;
    let response = app.library_state.query(count_query);

    if let LibraryResponse::Count(c) = response {
        assert_eq!(c, 0, "Item count should be 0");
    } else {
        panic!("Unexpected response type");
    }
}

fn album_with_channels(title: &str, artist: &str, channels: u32) -> Album {
    Album {
        id: None,
        title: title.to_string(),
        year: Some(2024),
        tracks: vec![Track {
            path: PathBuf::from(format!("/test/{title}.flac")),
            title: Some(format!("{title} Track")),
            artist: Some(artist.to_string()),
            channels: Some(channels),
            ..Default::default()
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
        edition: None,
        dynamic_range: None,
        is_favorite: false,
        uuid: None,
    }
}

#[gpui::test]
fn test_app_library_filter_methods_refresh_cache_immediately(_cx: &mut TestAppContext) {
    let mut app = App::new();
    app.library_state.library.albums = vec![
        album_with_channels("Stereo Album", "Blue Artist", 2),
        album_with_channels("Surround Album", "Green Artist", 6),
        album_with_channels("Other Stereo", "Red Artist", 2),
    ];
    app.library_state.invalidate_cache();
    app.library_state.ensure_cache_valid();

    app.set_channel_filter(ChannelFilter::Surround);
    let titles = app
        .get_paginated_albums()
        .into_iter()
        .map(|album| album.title.clone())
        .collect::<Vec<_>>();
    assert_eq!(titles, vec!["Surround Album"]);

    app.set_channel_filter(ChannelFilter::All);
    app.set_library_search_query("blue".to_string());
    let titles = app
        .get_paginated_albums()
        .into_iter()
        .map(|album| album.title.clone())
        .collect::<Vec<_>>();
    assert_eq!(titles, vec!["Stereo Album"]);

    app.clear_library_search();
    assert_eq!(app.get_paginated_albums().len(), 3);
}
