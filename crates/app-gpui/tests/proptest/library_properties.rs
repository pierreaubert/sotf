//! Property-based tests for LibraryController filtering, sorting, and pagination.
//!
//! Tests the real `LibraryController` from `sotf-player` with generated data.

use proptest::prelude::*;
use sotf_audio_player::{
    Album, ChannelFilter, LibraryController, LibrarySortOrder, MusicLibrary, Track,
};
use std::path::PathBuf;

// =============================================================================
// Strategies for generating real Album/Track data
// =============================================================================

fn make_track(
    album_title: &str,
    index: usize,
    channels: u32,
    artist: &str,
    genre: Option<&str>,
) -> Track {
    Track {
        path: PathBuf::from(format!("/music/{}/track_{}.flac", album_title, index)),
        title: Some(format!("Track {}", index)),
        channels: Some(channels),
        artist: Some(artist.to_string()),
        album_artist: Some(artist.to_string()),
        genre: genre.map(|g| g.to_string()),
        ..Default::default()
    }
}

fn album_strategy() -> impl Strategy<Value = Album> {
    (
        "[A-Za-z0-9]{1,15}",
        "[A-Za-z]{1,10}",
        prop::option::of(1950u32..2030),
        prop::option::of(prop::sample::select(vec![
            "Rock",
            "Jazz",
            "Classical",
            "Electronic",
            "Pop",
            "Metal",
        ])),
        prop::sample::select(vec![1u32, 2, 2, 2, 5, 6, 8]),
        1usize..8,
    )
        .prop_map(|(title, artist, year, genre, channels, track_count)| {
            let tracks: Vec<Track> = (1..=track_count)
                .map(|i| make_track(&title, i, channels, &artist, genre))
                .collect();

            Album {
                title,
                year,
                tracks,
                ..Default::default()
            }
        })
}

fn channel_filter_strategy() -> impl Strategy<Value = ChannelFilter> {
    prop_oneof![
        Just(ChannelFilter::All),
        Just(ChannelFilter::Stereo),
        Just(ChannelFilter::Mono),
        Just(ChannelFilter::Surround),
        Just(ChannelFilter::Surround71),
    ]
}

fn sort_order_strategy() -> impl Strategy<Value = LibrarySortOrder> {
    prop_oneof![
        Just(LibrarySortOrder::Album),
        Just(LibrarySortOrder::Artist),
        Just(LibrarySortOrder::Year),
        Just(LibrarySortOrder::Genre),
        Just(LibrarySortOrder::Tracks),
    ]
}

/// Build a LibraryController with in-memory library populated with albums.
fn controller_with_albums(albums: Vec<Album>) -> LibraryController {
    let mut lib = MusicLibrary::new();
    lib.albums = albums;
    let mut ctrl = LibraryController::with_library(lib);
    ctrl.ensure_cache_valid();
    ctrl
}

// =============================================================================
// Property Tests
// =============================================================================

proptest! {
    /// INVARIANT: Channel filter never increases result count vs All.
    #[test]
    fn channel_filter_reduces_results(
        albums in prop::collection::vec(album_strategy(), 0..50),
        filter in channel_filter_strategy()
    ) {
        let mut ctrl_all = controller_with_albums(albums.clone());
        ctrl_all.set_filter(ChannelFilter::All);
        ctrl_all.ensure_cache_valid();
        let count_all = ctrl_all.filtered_albums().len();

        let mut ctrl_filtered = controller_with_albums(albums);
        ctrl_filtered.set_filter(filter);
        ctrl_filtered.ensure_cache_valid();
        let count_filtered = ctrl_filtered.filtered_albums().len();

        prop_assert!(
            count_filtered <= count_all,
            "Filter {:?} increased results: {} > {}",
            filter, count_filtered, count_all
        );
    }

    /// INVARIANT: Search results are a subset of unfiltered results.
    #[test]
    fn search_results_subset(
        albums in prop::collection::vec(album_strategy(), 0..50),
        query in "[a-zA-Z0-9 ]{0,10}"
    ) {
        let mut ctrl = controller_with_albums(albums);
        let count_all = ctrl.filtered_albums().len();

        ctrl.set_search_query(query);
        ctrl.ensure_cache_valid();
        let count_search = ctrl.filtered_albums().len();

        prop_assert!(
            count_search <= count_all,
            "Search returned MORE results: {} > {}",
            count_search, count_all
        );
    }

    /// INVARIANT: Clearing search restores unfiltered count (for same channel filter).
    #[test]
    fn clear_search_restores_count(
        albums in prop::collection::vec(album_strategy(), 0..30),
        query in "[a-zA-Z]{1,5}",
        filter in channel_filter_strategy()
    ) {
        let mut ctrl = controller_with_albums(albums);
        ctrl.set_filter(filter);
        ctrl.ensure_cache_valid();
        let initial_count = ctrl.filtered_albums().len();

        ctrl.set_search_query(query);
        ctrl.ensure_cache_valid();

        ctrl.clear_search();
        ctrl.ensure_cache_valid();
        let restored_count = ctrl.filtered_albums().len();

        prop_assert_eq!(
            restored_count, initial_count,
            "Clear search didn't restore count"
        );
    }

    /// INVARIANT: Total pages >= 1, and page count is consistent with item count.
    #[test]
    fn total_pages_consistent(
        albums in prop::collection::vec(album_strategy(), 0..60),
        items_per_page in 1usize..30
    ) {
        let mut ctrl = controller_with_albums(albums);
        ctrl.items_per_page = items_per_page;
        ctrl.ensure_cache_valid();

        let total = ctrl.total_pages();
        let item_count = ctrl.filtered_albums().len();

        let expected = if item_count == 0 {
            1
        } else {
            item_count.div_ceil(items_per_page)
        };

        prop_assert_eq!(total, expected);
    }

    /// INVARIANT: Sorting does not change the number of results.
    #[test]
    fn sorting_preserves_count(
        albums in prop::collection::vec(album_strategy(), 0..40),
        order in sort_order_strategy()
    ) {
        let mut ctrl = controller_with_albums(albums);
        let count_before = ctrl.filtered_albums().len();

        ctrl.set_sort_order(order);
        ctrl.ensure_cache_valid();
        let count_after = ctrl.filtered_albums().len();

        prop_assert_eq!(count_before, count_after);
    }

    /// INVARIANT: selected_index resets to 0 after set_sort_order.
    #[test]
    fn sort_resets_selection(
        albums in prop::collection::vec(album_strategy(), 1..20),
        order in sort_order_strategy()
    ) {
        let mut ctrl = controller_with_albums(albums);
        ctrl.selected_index = 5;

        ctrl.set_sort_order(order);

        prop_assert_eq!(ctrl.selected_index, 0);
    }

    /// INVARIANT: Genre selection filter only returns albums matching that genre.
    #[test]
    fn genre_filter_correctness(
        albums in prop::collection::vec(album_strategy(), 5..30)
    ) {
        let mut ctrl = controller_with_albums(albums);
        ctrl.ensure_cache_valid();
        let count_all = ctrl.selection_filtered_albums().len();

        ctrl.selected_genre = Some("Rock".to_string());
        let filtered = ctrl.selection_filtered_albums();

        prop_assert!(filtered.len() <= count_all);

        for album in &filtered {
            let genre = album
                .tracks
                .first()
                .and_then(|t| t.genre.as_ref())
                .map(|g: &String| g.to_lowercase());
            prop_assert_eq!(
                genre.as_deref(),
                Some("rock"),
                "Album '{}' has wrong genre",
                album.title
            );
        }
    }

    /// INVARIANT: Decade filter only returns albums within the decade range.
    #[test]
    fn decade_filter_correctness(
        albums in prop::collection::vec(album_strategy(), 5..30),
        decade_start in (1950i32..2020).prop_map(|y| (y / 10) * 10)
    ) {
        let decade_end = decade_start + 9;

        let mut ctrl = controller_with_albums(albums);
        ctrl.ensure_cache_valid();
        ctrl.selected_decade = Some((decade_start, decade_end));

        let filtered = ctrl.selection_filtered_albums();

        for album in &filtered {
            if let Some(year) = album.year {
                let year = year as i32;
                prop_assert!(
                    year >= decade_start && year <= decade_end,
                    "Album year {} outside decade {}-{}",
                    year, decade_start, decade_end
                );
            }
        }
    }

    /// INVARIANT: search active -> selection filters are bypassed.
    #[test]
    fn search_bypasses_selection_filters(
        albums in prop::collection::vec(album_strategy(), 5..30),
        query in "[a-zA-Z]{1,5}"
    ) {
        let mut ctrl = controller_with_albums(albums);

        // Set restrictive selection filter
        ctrl.selected_genre = Some("NonexistentGenre12345".to_string());
        let without_search = ctrl.selection_filtered_albums().len();

        // Now set search -- selection filters should be bypassed
        ctrl.set_search_query(query);
        ctrl.ensure_cache_valid();
        ctrl.selected_genre = Some("NonexistentGenre12345".to_string());
        let with_search = ctrl.selection_filtered_albums();

        // With search active, genre filter should NOT be applied
        let search_only_count = ctrl.filtered_albums().len();
        prop_assert_eq!(
            with_search.len(),
            search_only_count,
            "Selection filters were applied during search"
        );

        // Without search, restrictive genre should give 0 or few results
        prop_assert!(without_search <= search_only_count || search_only_count == 0);
    }

    /// INVARIANT: Navigation wraps correctly.
    #[test]
    fn navigation_wraps(album_count in 2usize..20) {
        let albums: Vec<Album> = (0..album_count)
            .map(|i| Album {
                title: format!("Album {}", i),
                tracks: vec![Track {
                    path: PathBuf::from(format!("/music/album_{}/track.flac", i)),
                    title: Some(format!("Track {}", i)),
                    channels: Some(2),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .collect();

        let mut ctrl = controller_with_albums(albums);
        ctrl.ensure_cache_valid();
        let count = ctrl.filtered_albums().len();

        // Go to last, then next wraps to first
        ctrl.selected_index = count - 1;
        ctrl.select_next();
        prop_assert_eq!(ctrl.selected_index, 0, "Didn't wrap to first");

        // Go backward from first wraps to last
        ctrl.select_prev();
        prop_assert_eq!(ctrl.selected_index, count - 1, "Didn't wrap to last");
    }

    /// INVARIANT: Paginated albums are a contiguous slice of filtered albums.
    #[test]
    fn paginated_albums_are_correct_slice(
        albums in prop::collection::vec(album_strategy(), 1..50),
        items_per_page in 1usize..15,
        page in 0usize..10
    ) {
        let mut ctrl = controller_with_albums(albums);
        ctrl.items_per_page = items_per_page;
        ctrl.ensure_cache_valid();

        let total = ctrl.total_pages();
        ctrl.current_page = page.min(total.saturating_sub(1));

        let paginated = ctrl.get_paginated_albums();
        let all = ctrl.filtered_albums();
        let start = ctrl.current_page * items_per_page;

        prop_assert!(paginated.len() <= items_per_page);

        for (i, album) in paginated.iter().enumerate() {
            prop_assert_eq!(
                &album.title,
                &all[start + i].title,
                "Paginated album at {} doesn't match filtered album at {}",
                i,
                start + i
            );
        }
    }
}
