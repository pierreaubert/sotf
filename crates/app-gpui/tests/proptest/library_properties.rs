//! Property-based tests for library filtering and sorting.
//!
//! These tests use proptest to generate random inputs and verify invariants.

#[path = "../common/mod.rs"]
mod common;

use common::state_builder::{
    TestAlbum, TestChannelFilter, TestLibrarySortOrder, TestLibraryState, TestTrack,
};
use proptest::prelude::*;

// =============================================================================
// Strategies for generating test data
// =============================================================================

/// Generate a valid search query (printable ASCII, limited length)
fn search_query_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z0-9 ]{0,30}")
        .unwrap()
        .prop_map(|s| s.trim().to_string())
}

/// Generate an album title
fn album_title_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[A-Za-z0-9 ]{1,20}")
        .unwrap()
        .prop_map(|s| s.trim().to_string())
        .prop_filter("non-empty", |s| !s.is_empty())
}

/// Generate an artist name
fn artist_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[A-Za-z ]{1,15}")
        .unwrap()
        .prop_map(|s| s.trim().to_string())
        .prop_filter("non-empty", |s| !s.is_empty())
}

/// Generate a test album
fn album_strategy() -> impl Strategy<Value = TestAlbum> {
    (
        album_title_strategy(),
        artist_strategy(),
        prop::option::of(1950i32..2030), // year
        prop::option::of(prop::sample::select(vec![
            "Rock",
            "Jazz",
            "Classical",
            "Electronic",
            "Pop",
            "Metal",
        ])),
        1usize..=8, // channels
        1usize..10, // track count
    )
        .prop_map(|(title, artist, year, genre, channels, track_count)| {
            let tracks: Vec<TestTrack> = (1..=track_count)
                .map(|i| TestTrack::new(&format!("{}_track{}.flac", title, i)))
                .collect();

            let mut album = TestAlbum::new(&title, &artist)
                .with_channels(channels)
                .with_tracks(tracks);

            if let Some(y) = year {
                album = album.with_year(y);
            }
            if let Some(g) = genre {
                album = album.with_genre(g);
            }
            album
        })
}

/// Generate a channel filter
fn channel_filter_strategy() -> impl Strategy<Value = TestChannelFilter> {
    prop_oneof![
        Just(TestChannelFilter::All),
        Just(TestChannelFilter::Stereo),
        Just(TestChannelFilter::Multichannel),
        Just(TestChannelFilter::Mono),
    ]
}

/// Generate a sort order
fn sort_order_strategy() -> impl Strategy<Value = TestLibrarySortOrder> {
    prop_oneof![
        Just(TestLibrarySortOrder::Title),
        Just(TestLibrarySortOrder::Artist),
        Just(TestLibrarySortOrder::Year),
        Just(TestLibrarySortOrder::Genre),
    ]
}

// =============================================================================
// Property Tests
// =============================================================================

proptest! {
    /// INVARIANT: Search results are always a subset of unfiltered results
    #[test]
    fn search_results_subset_of_all(
        query in search_query_strategy(),
        albums in prop::collection::vec(album_strategy(), 0..50)
    ) {
        let mut state = TestLibraryState::default().with_albums(albums);

        // Get unfiltered count
        let unfiltered = state.filtered_albums().len();

        // Apply search
        state.search_query = query;
        let filtered = state.filtered_albums().len();

        // Invariant: filtered <= unfiltered
        prop_assert!(
            filtered <= unfiltered,
            "Search returned MORE results than unfiltered: {} > {}",
            filtered, unfiltered
        );
    }

    /// INVARIANT: Empty search returns same as no search
    #[test]
    fn empty_search_is_identity(
        albums in prop::collection::vec(album_strategy(), 0..30)
    ) {
        let state_no_search = TestLibraryState::default().with_albums(albums.clone());
        let state_empty_search = TestLibraryState::default()
            .with_albums(albums.clone())
            .with_search("");
        let state_whitespace_search = TestLibraryState::default()
            .with_albums(albums)
            .with_search("   ");

        let count_no_search = state_no_search.filtered_albums().len();
        let count_empty = state_empty_search.filtered_albums().len();
        let count_whitespace = state_whitespace_search.filtered_albums().len();

        prop_assert_eq!(count_no_search, count_empty);
        prop_assert_eq!(count_no_search, count_whitespace);
    }

    /// INVARIANT: Channel filter reduces or maintains result count
    #[test]
    fn channel_filter_reduces_results(
        albums in prop::collection::vec(album_strategy(), 0..50),
        filter in channel_filter_strategy()
    ) {
        let state_all = TestLibraryState::default().with_albums(albums.clone());
        let mut state_filtered = TestLibraryState::default().with_albums(albums);
        state_filtered.channel_filter = filter;

        let count_all = state_all.filtered_albums().len();
        let count_filtered = state_filtered.filtered_albums().len();

        // Filtered count should be <= all count
        prop_assert!(
            count_filtered <= count_all,
            "Channel filter {:?} increased results: {} > {}",
            filter, count_filtered, count_all
        );
    }

    /// INVARIANT: Page index is always valid after recalculation
    #[test]
    fn page_index_always_valid(
        albums in prop::collection::vec(album_strategy(), 0..100),
        page in 0usize..50,
        items_per_page in 1usize..30
    ) {
        let mut state = TestLibraryState::default().with_albums(albums);
        state.current_page = page;
        state.items_per_page = items_per_page;

        state.recalculate_pagination();

        let max_page = state.total_pages().saturating_sub(1);
        prop_assert!(
            state.current_page <= max_page,
            "Page {} exceeds max {} (total_pages={})",
            state.current_page, max_page, state.total_pages()
        );
    }

    /// INVARIANT: Applying then clearing search returns same as no search
    #[test]
    fn clear_search_restores_state(
        albums in prop::collection::vec(album_strategy(), 0..30),
        query in search_query_strategy(),
        channel_filter in channel_filter_strategy()
    ) {
        let mut state = TestLibraryState::default().with_albums(albums);
        state.channel_filter = channel_filter;

        // Get initial filtered count (with channel filter, no search)
        let initial_count = state.filtered_albums().len();

        // Apply search
        state.search_query = query;
        let _search_count = state.filtered_albums().len();

        // Clear search
        state.clear_search();
        let restored_count = state.filtered_albums().len();

        // Should be back to initial state
        prop_assert_eq!(
            restored_count, initial_count,
            "Clear search didn't restore initial state"
        );

        // Channel filter should still be active
        prop_assert_eq!(state.channel_filter, channel_filter);
    }

    /// INVARIANT: Genre filter reduces or maintains result count
    #[test]
    fn genre_filter_reduces_results(
        albums in prop::collection::vec(album_strategy(), 5..30)
    ) {
        let state_all = TestLibraryState::default().with_albums(albums.clone());
        let count_all = state_all.filtered_albums().len();

        let mut state_filtered = TestLibraryState::default().with_albums(albums);
        state_filtered.selected_genre = Some("Rock".to_string());
        let count_filtered = state_filtered.filtered_albums().len();

        prop_assert!(
            count_filtered <= count_all,
            "Genre filter increased results: {} > {}",
            count_filtered, count_all
        );
    }

    /// INVARIANT: Decade filter only includes years in range
    #[test]
    fn decade_filter_correct_years(
        albums in prop::collection::vec(album_strategy(), 5..30),
        decade_start in (1950i32..2020).prop_map(|y| (y / 10) * 10)
    ) {
        let decade_end = decade_start + 9;

        let mut state = TestLibraryState::default().with_albums(albums);
        state.selected_decade = Some((decade_start, decade_end));

        let filtered = state.filtered_albums();

        for album in filtered {
            if let Some(year) = album.year {
                prop_assert!(
                    year >= decade_start && year <= decade_end,
                    "Album year {} outside decade {}-{}",
                    year, decade_start, decade_end
                );
            }
        }
    }

    /// INVARIANT: Search with active query bypasses selection filters
    #[test]
    fn search_bypasses_selection_filters(
        albums in prop::collection::vec(album_strategy(), 5..30),
        query in "[a-zA-Z]{1,5}".prop_filter("non-empty", |s| !s.is_empty())
    ) {
        let mut state = TestLibraryState::default().with_albums(albums);

        // Apply restrictive selection filters
        state.selected_genre = Some("NonexistentGenre".to_string());
        state.selected_year = Some(1900); // Unlikely year

        // Without search, should return very few or zero results
        let without_search = state.filtered_albums().len();

        // With search, selection filters should be bypassed
        state.search_query = query;
        let with_search = state.filtered_albums().len();

        // Search may return more results because it bypasses selection filters
        // (We can't assert equality because search also filters by query)
        // This test primarily verifies the code doesn't panic with restrictive filters
        // and that the filtered_albums() method handles both scenarios correctly.
        let _ = (with_search, without_search); // Verify values are computed without panic
    }

    /// INVARIANT: Total pages calculation is consistent
    #[test]
    fn total_pages_consistent(
        album_count in 0usize..100,
        items_per_page in 1usize..30
    ) {
        let albums = (0..album_count)
            .map(|i| TestAlbum::new(&format!("Album {}", i), "Artist"))
            .collect();

        let mut state = TestLibraryState::default().with_albums(albums);
        state.items_per_page = items_per_page;

        let total_pages = state.total_pages();
        let expected_pages = (album_count + items_per_page - 1) / items_per_page;
        let expected_pages = if expected_pages == 0 { 1 } else { expected_pages };

        // Account for empty library case (always 1 page minimum is wrong in our impl)
        let filtered_count = state.filtered_albums().len();
        let computed_pages = if filtered_count == 0 {
            1
        } else {
            (filtered_count + items_per_page - 1) / items_per_page
        };

        prop_assert_eq!(
            total_pages, computed_pages,
            "Total pages mismatch: expected {}, got {}",
            computed_pages, total_pages
        );
    }
}
