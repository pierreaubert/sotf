//! Unit tests for LibraryState
//!
//! Tests the domain-separated library state functionality.

use sotf_audio_player::{Album, Track};
use sotf_audio_player_gpui::state::library::{
    ChannelFilter, LibrarySortOrder, LibraryState, LibraryViewMode,
};

fn make_test_album(title: &str, artist: &str, track_count: usize, channels: u32) -> Album {
    let tracks: Vec<Track> = (0..track_count)
        .map(|i| Track {
            title: Some(format!("Track {}", i + 1)),
            path: format!("/path/to/{}/{}.flac", artist, i + 1).into(),
            duration_secs: Some(180),
            track_number: Some(i as u32 + 1),
            channels: Some(channels),
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
        })
        .collect();

    Album {
        id: None,
        title: title.to_string(),
        artist: artist.to_string(),
        year: Some(2020),
        tracks,
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    }
}

fn new_test_state() -> LibraryState {
    use sotf_audio_player::MusicLibrary;
    LibraryState::with_library(MusicLibrary::new())
}

#[test]
fn test_filter_by_channel_count() {
    let mut state = new_test_state();
    state
        .library
        .albums
        .push(make_test_album("Stereo Album", "Artist A", 5, 2));
    state
        .library
        .albums
        .push(make_test_album("Surround Album", "Artist B", 5, 6));
    state
        .library
        .albums
        .push(make_test_album("Mono Album", "Artist C", 3, 1));

    // All
    state.set_filter(ChannelFilter::All);
    assert_eq!(state.filtered_albums().len(), 3);

    // Stereo only
    state.set_filter(ChannelFilter::Stereo);
    assert_eq!(state.filtered_albums().len(), 1);
    assert_eq!(state.filtered_albums()[0].title, "Stereo Album");

    // Multichannel only
    state.set_filter(ChannelFilter::Multichannel);
    assert_eq!(state.filtered_albums().len(), 1);
    assert_eq!(state.filtered_albums()[0].title, "Surround Album");

    // Mono only
    state.set_filter(ChannelFilter::Mono);
    assert_eq!(state.filtered_albums().len(), 1);
    assert_eq!(state.filtered_albums()[0].title, "Mono Album");
}

#[test]
fn test_sort_by_artist() {
    let mut state = new_test_state();
    state
        .library
        .albums
        .push(make_test_album("Album Z", "Zebra", 3, 2));
    state
        .library
        .albums
        .push(make_test_album("Album A", "Apple", 3, 2));
    state
        .library
        .albums
        .push(make_test_album("Album M", "Mango", 3, 2));

    state.set_sort_order(LibrarySortOrder::Artist);
    let albums = state.filtered_albums();

    assert_eq!(albums[0].artist, "Apple");
    assert_eq!(albums[1].artist, "Mango");
    assert_eq!(albums[2].artist, "Zebra");
}

#[test]
fn test_sort_by_year() {
    let mut state = new_test_state();

    let mut album1 = make_test_album("Old Album", "Artist", 3, 2);
    album1.year = Some(1990);
    let mut album2 = make_test_album("New Album", "Artist", 3, 2);
    album2.year = Some(2023);
    let mut album3 = make_test_album("Mid Album", "Artist", 3, 2);
    album3.year = Some(2010);

    state.library.albums.push(album1);
    state.library.albums.push(album2);
    state.library.albums.push(album3);

    state.set_sort_order(LibrarySortOrder::Year);
    let albums = state.filtered_albums();

    // Year sort is descending (newest first)
    assert_eq!(albums[0].year, Some(2023));
    assert_eq!(albums[1].year, Some(2010));
    assert_eq!(albums[2].year, Some(1990));
}

#[test]
fn test_search_query() {
    let mut state = new_test_state();
    state.library.albums.push(make_test_album(
        "Pink Floyd DSOTM",
        "Pink Floyd",
        10,
        2,
    ));
    state
        .library
        .albums
        .push(make_test_album("Abbey Road", "Beatles", 17, 2));
    state
        .library
        .albums
        .push(make_test_album("The Wall", "Pink Floyd", 26, 2));

    state.set_search_query("Pink".to_string());
    assert_eq!(state.search_query, "Pink");
    assert_eq!(state.selected_index, 0); // Reset on search

    state.clear_search();
    assert!(state.search_query.is_empty());
}

#[test]
fn test_navigation_select_next_prev() {
    let mut state = new_test_state();
    state
        .library
        .albums
        .push(make_test_album("Album 1", "Artist", 3, 2));
    state
        .library
        .albums
        .push(make_test_album("Album 2", "Artist", 3, 2));
    state
        .library
        .albums
        .push(make_test_album("Album 3", "Artist", 3, 2));

    assert_eq!(state.selected_index, 0);

    state.select_next();
    assert_eq!(state.selected_index, 1);

    state.select_next();
    assert_eq!(state.selected_index, 2);

    state.select_next();
    assert_eq!(state.selected_index, 0); // Wraps around

    state.select_prev();
    assert_eq!(state.selected_index, 2); // Wraps around backwards
}

#[test]
fn test_pagination() {
    let mut state = new_test_state();
    state.items_per_page = 10;

    // Add 25 albums
    for i in 0..25 {
        state
            .library
            .albums
            .push(make_test_album(&format!("Album {}", i), "Artist", 3, 2));
    }

    assert_eq!(state.total_pages(), 3); // 25 items / 10 per page = 3 pages
    assert_eq!(state.current_page, 0);

    // First page has 10 items
    assert_eq!(state.get_paginated_albums().len(), 10);

    state.next_page();
    assert_eq!(state.current_page, 1);
    assert_eq!(state.get_paginated_albums().len(), 10);

    state.next_page();
    assert_eq!(state.current_page, 2);
    assert_eq!(state.get_paginated_albums().len(), 5); // Remaining 5

    // Can't go past last page
    state.next_page();
    assert_eq!(state.current_page, 2);

    state.prev_page();
    assert_eq!(state.current_page, 1);
}

#[test]
fn test_tree_view_building() {
    let mut state = new_test_state();
    state
        .library
        .albums
        .push(make_test_album("Album A", "Alpha", 3, 2));
    state
        .library
        .albums
        .push(make_test_album("Album B", "Beta", 3, 2));
    state
        .library
        .albums
        .push(make_test_album("Album A2", "Alpha", 3, 2));

    state.rebuild_letter_tree();

    assert_eq!(state.letter_tree.len(), 2); // A and B letters
    assert_eq!(state.letter_tree[0].letter, 'A');
    assert_eq!(state.letter_tree[0].album_indices.len(), 2); // Two Alpha albums
    assert_eq!(state.letter_tree[1].letter, 'B');
    assert_eq!(state.letter_tree[1].album_indices.len(), 1);
}

#[test]
fn test_view_mode_cycling() {
    let mut state = new_test_state();

    assert_eq!(state.view_mode, LibraryViewMode::Grid); // Default

    state.cycle_view_mode();
    assert_eq!(state.view_mode, LibraryViewMode::Flat);

    state.cycle_view_mode();
    assert_eq!(state.view_mode, LibraryViewMode::TreeView);

    state.cycle_view_mode();
    assert_eq!(state.view_mode, LibraryViewMode::Grid);
}

#[test]
fn test_filter_cycling() {
    let mut state = new_test_state();

    assert_eq!(state.filter, ChannelFilter::All);

    state.cycle_filter();
    assert_eq!(state.filter, ChannelFilter::Mono);

    state.cycle_filter();
    assert_eq!(state.filter, ChannelFilter::Stereo);

    state.cycle_filter();
    assert_eq!(state.filter, ChannelFilter::Multichannel);

    state.cycle_filter();
    assert_eq!(state.filter, ChannelFilter::Mixed);

    state.cycle_filter();
    assert_eq!(state.filter, ChannelFilter::All);
}

#[test]
fn test_selected_album() {
    let mut state = new_test_state();
    state
        .library
        .albums
        .push(make_test_album("First", "Artist", 3, 2));
    state
        .library
        .albums
        .push(make_test_album("Second", "Artist", 3, 2));

    let album = state.selected_album().unwrap();
    assert_eq!(album.title, "First");

    state.select_next();
    let album = state.selected_album().unwrap();
    assert_eq!(album.title, "Second");
}

#[test]
fn test_empty_library() {
    let state = new_test_state();

    assert_eq!(state.filtered_albums().len(), 0);
    assert_eq!(state.total_pages(), 1); // At least 1 page even if empty
    assert!(state.selected_album().is_none());
}
