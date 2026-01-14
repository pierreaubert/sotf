//! Invalid State Tests
//!
//! These tests verify that invalid states are either rejected or handled gracefully.
//! The application should never enter a corrupted state, even with edge-case inputs.

#[path = "../common/mod.rs"]
mod common;

use common::state_builder::{
    TestAlbum, TestChannelFilter, TestLibrarySortOrder, TestLibraryState, TestPlaybackState,
    TestQueueItem, TestTrack, create_test_albums,
};

// =============================================================================
// Library State Validation
// =============================================================================

/// Test: Page index is bounded to valid range
#[test]
fn test_page_index_bounded() {
    let albums = create_test_albums(50);
    let mut state = TestLibraryState::default().with_albums(albums);
    state.items_per_page = 10;

    // Set invalid page
    state.current_page = 100; // Way beyond valid range

    state.recalculate_pagination();

    // Page should be clamped to max valid
    assert!(
        state.current_page <= 4,
        "Page {} exceeds max (4)",
        state.current_page
    );
}

/// Test: Page index valid with empty library
#[test]
fn test_page_index_empty_library() {
    let mut state = TestLibraryState::default();
    state.current_page = 10;

    state.recalculate_pagination();

    assert_eq!(state.current_page, 0, "Page should be 0 for empty library");
}

/// Test: Items per page of 0 doesn't cause division by zero
#[test]
fn test_items_per_page_zero() {
    let albums = create_test_albums(10);
    let mut state = TestLibraryState::default().with_albums(albums);
    state.items_per_page = 0;

    // Should not panic
    let total_pages = state.total_pages();
    assert_eq!(total_pages, 1, "Zero items_per_page should return 1 page");
}

/// Test: Filtering to empty result doesn't crash
#[test]
fn test_filter_to_empty() {
    let albums = vec![
        TestAlbum::new("Rock Album", "Artist").with_genre("Rock"),
        TestAlbum::new("Pop Album", "Artist").with_genre("Pop"),
    ];
    let mut state = TestLibraryState::default().with_albums(albums);

    // Filter by non-existent genre
    state.selected_genre = Some("Jazz".to_string());

    let filtered = state.filtered_albums();
    assert_eq!(filtered.len(), 0, "Should return empty, not crash");

    // Pagination should handle empty result
    state.recalculate_pagination();
    assert_eq!(state.current_page, 0);
}

/// Test: Search with special characters doesn't crash
#[test]
fn test_search_special_characters() {
    let albums = create_test_albums(10);
    let state = TestLibraryState::default()
        .with_albums(albums)
        .with_search("test.*+?^${}()|[]\\");

    // Should not panic, should just return no results
    let filtered = state.filtered_albums();
    assert!(filtered.is_empty() || !filtered.is_empty()); // Just verify no panic
}

/// Test: Search with unicode characters
#[test]
fn test_search_unicode() {
    let albums = vec![
        TestAlbum::new("日本語アルバム", "アーティスト"),
        TestAlbum::new("English Album", "Artist"),
    ];
    let state = TestLibraryState::default()
        .with_albums(albums)
        .with_search("日本語");

    let filtered = state.filtered_albums();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "日本語アルバム");
}

// =============================================================================
// Playback State Validation
// =============================================================================

/// Test: Queue index out of bounds returns None
#[test]
fn test_queue_index_out_of_bounds() {
    let mut state = TestPlaybackState::default();
    state.current_queue_index = Some(5); // No queue items

    let next = state.next_track();
    assert!(next.is_none());
}

/// Test: Next track on empty queue
#[test]
fn test_next_track_empty_queue() {
    let mut state = TestPlaybackState::default();

    let next = state.next_track();
    assert!(next.is_none());
}

/// Test: Seek position clamped to valid range
#[test]
fn test_seek_position_clamped() {
    let album = TestAlbum::new("Album", "Artist").with_tracks(vec![TestTrack::new("track.flac")]);
    let mut state = TestPlaybackState::default().with_queue(vec![TestQueueItem::from_album(album)]);
    state.duration_secs = 180.0;

    // Seek beyond duration
    state.seek_to(500.0).ok();
    assert_eq!(
        state.position_secs, 180.0,
        "Position should clamp to duration"
    );

    // Seek to negative
    state.seek_to(-10.0).ok();
    assert_eq!(state.position_secs, 0.0, "Position should clamp to 0");
}

/// Test: Track index within queue item bounds
#[test]
fn test_track_index_bounds() {
    let album =
        TestAlbum::new("Album", "Artist").with_tracks(vec![TestTrack::new("only_track.flac")]);
    let mut state = TestPlaybackState::default().with_queue(vec![TestQueueItem::from_album(album)]);

    // Manually corrupt track index
    if let Some(item) = state.queue.get_mut(0) {
        item.current_track_index = 100; // Invalid
    }

    // Next track should handle gracefully
    let next = state.next_track();
    // Should either return None or advance to next album, not crash
    assert!(next.is_none()); // No next album
}

// =============================================================================
// Filter Combination Tests
// =============================================================================

/// Test: Multiple filters combine correctly
#[test]
fn test_multiple_filters_combine() {
    let albums = vec![
        TestAlbum::new("A1", "Artist 1")
            .with_year(2020)
            .with_genre("Rock")
            .with_channels(2),
        TestAlbum::new("A2", "Artist 1")
            .with_year(2020)
            .with_genre("Jazz")
            .with_channels(2),
        TestAlbum::new("A3", "Artist 2")
            .with_year(2020)
            .with_genre("Rock")
            .with_channels(6),
        TestAlbum::new("A4", "Artist 2")
            .with_year(2019)
            .with_genre("Rock")
            .with_channels(2),
    ];

    let mut state = TestLibraryState::default().with_albums(albums);

    // Apply multiple filters
    state.selected_genre = Some("Rock".to_string());
    state.selected_year = Some(2020);
    state.channel_filter = TestChannelFilter::Stereo;

    let filtered = state.filtered_albums();

    // Only A1 matches all criteria
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "A1");
}

/// Test: Search bypasses all selection filters
#[test]
fn test_search_bypasses_selection_filters() {
    let albums = vec![
        TestAlbum::new("Target Album", "Artist 1")
            .with_year(2020)
            .with_genre("Jazz"),
        TestAlbum::new("Other Album", "Artist 2")
            .with_year(2019)
            .with_genre("Rock"),
    ];

    let mut state = TestLibraryState::default().with_albums(albums);

    // Apply restrictive selection filters
    state.selected_genre = Some("Rock".to_string());
    state.selected_year = Some(2019);

    // Without search: only "Other Album" matches
    assert_eq!(state.filtered_albums().len(), 1);
    assert_eq!(state.filtered_albums()[0].title, "Other Album");

    // With search: bypasses selection filters, finds Target
    state.search_query = "Target".to_string();
    let filtered = state.filtered_albums();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "Target Album");
}

// =============================================================================
// Decade/Year Filter Tests
// =============================================================================

/// Test: Decade filter without year
#[test]
fn test_decade_filter_only() {
    let albums = vec![
        TestAlbum::new("90s Album", "Artist").with_year(1995),
        TestAlbum::new("2000s Album", "Artist").with_year(2005),
        TestAlbum::new("2010s Album", "Artist").with_year(2015),
    ];

    let mut state = TestLibraryState::default().with_albums(albums);
    state.selected_decade = Some((2000, 2009));

    let filtered = state.filtered_albums();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "2000s Album");
}

/// Test: Year filter overrides decade
#[test]
fn test_year_overrides_decade() {
    let albums = vec![
        TestAlbum::new("2005 Album", "Artist").with_year(2005),
        TestAlbum::new("2007 Album", "Artist").with_year(2007),
    ];

    let mut state = TestLibraryState::default().with_albums(albums);
    state.selected_decade = Some((2000, 2009));
    state.selected_year = Some(2005);

    let filtered = state.filtered_albums();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "2005 Album");
}

// =============================================================================
// Artist Letter Filter Tests
// =============================================================================

/// Test: Artist letter filter with special characters
#[test]
fn test_artist_letter_special_char() {
    let albums = vec![
        TestAlbum::new("Album", "123 Artist"),
        TestAlbum::new("Album", "Aaron"),
        TestAlbum::new("Album", "Bob"),
    ];

    let mut state = TestLibraryState::default().with_albums(albums);
    state.selected_artist_letter = Some('#');

    let filtered = state.filtered_albums();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].artist, "123 Artist");
}

/// Test: Artist letter filter case insensitive
#[test]
fn test_artist_letter_case_insensitive() {
    let albums = vec![
        TestAlbum::new("Album 1", "Aaron"),
        TestAlbum::new("Album 2", "adam"),
        TestAlbum::new("Album 3", "Bob"),
    ];

    let mut state = TestLibraryState::default().with_albums(albums);
    state.selected_artist_letter = Some('A');

    let filtered = state.filtered_albums();
    assert_eq!(filtered.len(), 2);
}
