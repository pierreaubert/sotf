use gpui::TestAppContext;
use sotf_audio_player::{Album, Track};
use sotf_audio_player_gpui::app::state::library::{LibrarySortOrder, LibraryState};
use std::path::PathBuf;

fn create_test_album(title: &str, artist: &str, composer: Option<&str>) -> Album {
    Album {
        id: None,
        title: title.to_string(),
        year: Some(2020),
        tracks: vec![Track {
            path: PathBuf::from(format!("/test/{}.flac", title)),
            title: Some(format!("Track 1 from {}", title)),
            artist: Some(artist.to_string()),
            track_number: Some(1),
            duration_secs: Some(300),
            channels: Some(2),
            sample_rate: Some(44100),
            bit_depth: Some(16),
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: Some("Rock".to_string()),
            composer: composer.map(|s| s.to_string()),
            disc_number: Some(1),
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: Some(artist.to_string()),
            ensemble: None,
            edition: None,
            is_favorite: false,
            play_count: 0,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
        edition: None,
        dynamic_range: None,
        is_favorite: false,
    }
}

#[gpui::test]
fn test_smart_search_album_exact(_cx: &mut TestAppContext) {
    let mut state = LibraryState::new_for_test();
    state.library.albums.push(create_test_album("Lateralus", "Tool", None));
    state.library.albums.push(create_test_album("Aenima", "Tool", None));
    state.library.albums.push(create_test_album("Dark Side of the Moon", "Pink Floyd", None));

    // Default sort order is Album
    state.sort_order = LibrarySortOrder::Year;

    // Search for exact album title
    state.set_search_query("Lateralus".to_string());
    assert_eq!(state.sort_order, LibrarySortOrder::Album);
}

#[gpui::test]
fn test_smart_search_artist_exact(_cx: &mut TestAppContext) {
    let mut state = LibraryState::new_for_test();
    state.library.albums.push(create_test_album("Lateralus", "Tool", None));
    state.library.albums.push(create_test_album("Dark Side of the Moon", "Pink Floyd", None));

    state.sort_order = LibrarySortOrder::Album;

    // Search for exact artist name
    state.set_search_query("Tool".to_string());
    assert_eq!(state.sort_order, LibrarySortOrder::Artist);
}

#[gpui::test]
fn test_smart_search_composer_exact(_cx: &mut TestAppContext) {
    let mut state = LibraryState::new_for_test();
    state.library.albums.push(create_test_album("Symphony No. 5", "VPO", Some("Beethoven")));
    state.library.albums.push(create_test_album("Lateralus", "Tool", None));

    state.sort_order = LibrarySortOrder::Album;

    // Search for exact composer name
    state.set_search_query("Beethoven".to_string());
    assert_eq!(state.sort_order, LibrarySortOrder::Composer);
}

#[gpui::test]
fn test_smart_search_priority_album_over_artist(_cx: &mut TestAppContext) {
    let mut state = LibraryState::new_for_test();
    // Album titled "Tool" by artist "Someone Else"
    state.library.albums.push(create_test_album("Tool", "Someone Else", None));
    // Album by artist "Tool"
    state.library.albums.push(create_test_album("Lateralus", "Tool", None));

    state.sort_order = LibrarySortOrder::Year;

    // Exact album match "Tool" should take priority over exact artist match "Tool"
    state.set_search_query("Tool".to_string());
    assert_eq!(state.sort_order, LibrarySortOrder::Album);
}

#[gpui::test]
fn test_smart_search_partial_match(_cx: &mut TestAppContext) {
    let mut state = LibraryState::new_for_test();
    state.library.albums.push(create_test_album("Lateralus", "Tool", None));

    state.sort_order = LibrarySortOrder::Year;

    // Partial artist match
    state.set_search_query("Too".to_string());
    assert_eq!(state.sort_order, LibrarySortOrder::Artist);
}

#[gpui::test]
fn test_smart_search_no_match_stays_same(_cx: &mut TestAppContext) {
    let mut state = LibraryState::new_for_test();
    state.library.albums.push(create_test_album("Lateralus", "Tool", None));

    state.sort_order = LibrarySortOrder::Year;

    // No match for "Xylo"
    state.set_search_query("Xylo".to_string());
    assert_eq!(state.sort_order, LibrarySortOrder::Year);
}
