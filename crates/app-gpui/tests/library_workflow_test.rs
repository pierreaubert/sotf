use gpui::TestAppContext;
use sotf_audio_player::{Album, Track};
use sotf_audio_player_gpui::app::manager::Manager;
use sotf_audio_player_gpui::app::state::library::{
    ChannelFilter, LibraryEvent, LibraryQuery, LibraryResponse, LibrarySortOrder, LibraryState,
};
use std::path::PathBuf;

#[test]
fn album_detail_resolves_full_album_without_duplicate_tracks() {
    let track = |number, root: &str| Track {
        path: PathBuf::from(format!("/{root}/{number}.flac")),
        title: Some(format!("Track {number}")),
        track_number: Some(number),
        disc_number: Some(1),
        duration_secs: Some(60),
        ..Default::default()
    };
    let album = Album {
        id: Some(42),
        title: "Album".into(),
        tracks: vec![track(1, "a"), track(1, "b"), track(2, "a"), track(2, "b")],
        ..Default::default()
    };
    let mut state = LibraryState::new_for_test();
    state.library.albums.push(album.clone());
    let mut snapshot = album;
    snapshot.tracks.truncate(1);

    let detail = state.album_for_detail(&snapshot);
    assert_eq!(detail.tracks.len(), 2);
    assert_eq!(
        detail
            .tracks
            .iter()
            .filter_map(|t| t.duration_secs)
            .sum::<u64>(),
        120
    );
    assert_eq!(detail.tracks[1].title.as_deref(), Some("Track 2"));
}

#[gpui::test]
fn test_library_workflow(_cx: &mut TestAppContext) {
    // 1. Setup: Create LibraryState directly (unit testing the manager)
    // We use new_for_test() to avoid database loading in tests
    let mut manager = LibraryState::new_for_test();

    // 2. Test: Search
    manager
        .handle_event(LibraryEvent::SetSearchQuery("Rock".to_string()))
        .unwrap();
    assert_eq!(manager.search_query, "Rock");

    // 3. Test: Clear Search
    manager.handle_event(LibraryEvent::ClearSearch).unwrap();
    assert_eq!(manager.search_query, "");

    // 4. Test: Sorting
    manager
        .handle_event(LibraryEvent::SetSortOrder(LibrarySortOrder::Artist))
        .unwrap();
    assert_eq!(manager.sort_order, LibrarySortOrder::Artist);

    // 5. Test: Filtering
    manager
        .handle_event(LibraryEvent::SetFilter(ChannelFilter::Stereo))
        .unwrap();
    assert_eq!(manager.filter, ChannelFilter::Stereo);

    // 6. Test: Queries
    // Count should be 0 for empty test library
    let response = manager.query(LibraryQuery::ItemCount);
    if let LibraryResponse::Count(count) = response {
        assert_eq!(count, 0);
    } else {
        panic!("Wrong response type for ItemCount");
    }

    // 7. Test: Pagination
    manager.handle_event(LibraryEvent::NextPage).unwrap();
    // Assuming default is 0 and empty library, expecting to stay at 0 or handle gracefully
    assert_eq!(manager.current_page, 0);
}
