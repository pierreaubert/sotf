//! Library sequence tests - search and filter workflows.
//!
//! Tests realistic library browsing scenarios including search, filtering,
//! and navigation sequences that verify state preservation.

use crate::common::state_builder::{
    TestAlbum, TestChannelFilter, TestLibraryState,
};

// =============================================================================
// Helper: Create diverse test library
// =============================================================================

fn create_diverse_library() -> Vec<TestAlbum> {
    vec![
        // Rock albums
        TestAlbum::new("Dark Side of the Moon", "Pink Floyd")
            .with_year(1973)
            .with_genre("Rock")
            .with_channels(2),
        TestAlbum::new("The Wall", "Pink Floyd")
            .with_year(1979)
            .with_genre("Rock")
            .with_channels(2),
        TestAlbum::new("Led Zeppelin IV", "Led Zeppelin")
            .with_year(1971)
            .with_genre("Rock")
            .with_channels(2),
        // Jazz albums
        TestAlbum::new("Kind of Blue", "Miles Davis")
            .with_year(1959)
            .with_genre("Jazz")
            .with_channels(2),
        TestAlbum::new("A Love Supreme", "John Coltrane")
            .with_year(1965)
            .with_genre("Jazz")
            .with_channels(2),
        TestAlbum::new("Time Out", "Dave Brubeck")
            .with_year(1959)
            .with_genre("Jazz")
            .with_channels(2),
        // Classical - multichannel
        TestAlbum::new("Beethoven Symphony No. 9", "Berlin Philharmonic")
            .with_year(2010)
            .with_genre("Classical")
            .with_channels(6),
        TestAlbum::new("Mozart Requiem", "Vienna Philharmonic")
            .with_year(2015)
            .with_genre("Classical")
            .with_channels(6),
        // Electronic
        TestAlbum::new("Discovery", "Daft Punk")
            .with_year(2001)
            .with_genre("Electronic")
            .with_channels(2),
        TestAlbum::new("Random Access Memories", "Daft Punk")
            .with_year(2013)
            .with_genre("Electronic")
            .with_channels(2),
        // Tool albums (for search testing)
        TestAlbum::new("Lateralus", "Tool")
            .with_year(2001)
            .with_genre("Metal")
            .with_channels(2),
        TestAlbum::new("10,000 Days", "Tool")
            .with_year(2006)
            .with_genre("Metal")
            .with_channels(2),
    ]
}

// =============================================================================
// Sequence: Search Workflows
// =============================================================================

/// Test search → results → clear → back to full library
#[test]
fn test_search_clear_restores_full_library() {
    let mut state = TestLibraryState::default().with_albums(create_diverse_library());
    let total_albums = state.filtered_albums().len();

    // Search for specific artist
    state.search_query = "Tool".to_string();
    let search_results = state.filtered_albums().len();
    assert_eq!(search_results, 2, "Should find 2 Tool albums");

    // Clear search
    state.search_query.clear();
    let restored = state.filtered_albums().len();
    assert_eq!(restored, total_albums, "Should restore full library");
}

/// Test search → refine → clear workflow
#[test]
fn test_search_refinement_workflow() {
    let mut state = TestLibraryState::default().with_albums(create_diverse_library());

    // Broad search
    state.search_query = "D".to_string();
    let broad = state.filtered_albums().len();

    // Refine search
    state.search_query = "Da".to_string();
    let refined1 = state.filtered_albums().len();
    assert!(refined1 <= broad, "Refining should narrow results");

    // Further refine
    state.search_query = "Daft".to_string();
    let refined2 = state.filtered_albums().len();
    assert!(refined2 <= refined1, "Further refining should narrow more");
    assert_eq!(refined2, 2, "Should find 2 Daft Punk albums");

    // Complete search
    state.search_query = "Daft Punk".to_string();
    let final_results = state.filtered_albums().len();
    assert_eq!(final_results, 2, "Should still find 2 Daft Punk albums");
}

/// Test case-insensitive search
#[test]
fn test_case_insensitive_search_sequence() {
    let mut state = TestLibraryState::default().with_albums(create_diverse_library());

    let variations = ["tool", "TOOL", "Tool", "tOoL"];
    let mut results = Vec::new();

    for query in &variations {
        state.search_query = query.to_string();
        results.push(state.filtered_albums().len());
    }

    // All variations should return same number of results
    assert!(
        results.iter().all(|&r| r == results[0]),
        "Case variations should return same results: {:?}",
        results
    );
}

// =============================================================================
// Sequence: Filter Workflows
// =============================================================================

/// Test channel filter → search → clear filter workflow
#[test]
fn test_filter_then_search_workflow() {
    let mut state = TestLibraryState::default().with_albums(create_diverse_library());

    // Apply channel filter (multichannel only)
    state.channel_filter = TestChannelFilter::Multichannel;
    let multichannel = state.filtered_albums().len();
    assert_eq!(multichannel, 2, "Should have 2 multichannel albums");

    // Search within multichannel (but search bypasses filters in current impl)
    state.search_query = "Beethoven".to_string();
    let search_in_filter = state.filtered_albums().len();
    // Note: search bypasses selection filters but channel filter is separate
    assert!(search_in_filter >= 1, "Should find Beethoven");

    // Clear search, filter should still be active
    state.search_query.clear();
    let after_clear = state.filtered_albums().len();
    assert_eq!(
        after_clear, multichannel,
        "Channel filter should persist after clearing search"
    );

    // Remove channel filter
    state.channel_filter = TestChannelFilter::All;
    let all_albums = state.filtered_albums().len();
    assert!(all_albums > multichannel, "Should have more albums without filter");
}

/// Test genre filter → decade filter → artist filter workflow
#[test]
fn test_cascading_filter_workflow() {
    let mut state = TestLibraryState::default().with_albums(create_diverse_library());
    let total = state.filtered_albums().len();

    // Filter by genre
    state.selected_genre = Some("Rock".to_string());
    let rock = state.filtered_albums().len();
    assert_eq!(rock, 3, "Should have 3 Rock albums");
    assert!(rock < total, "Filtered should be less than total");

    // Add decade filter
    state.selected_decade = Some((1970, 1979));
    let rock_70s = state.filtered_albums().len();
    assert_eq!(rock_70s, 3, "Should have 3 Rock albums from 70s (1971, 1973, 1979)");
    assert!(rock_70s <= rock, "Adding filter should not increase results");

    // Clear genre, keep decade
    state.selected_genre = None;
    let all_70s = state.filtered_albums().len();
    assert!(all_70s >= rock_70s, "Removing filter should not decrease results");

    // Clear all filters
    state.selected_decade = None;
    let final_total = state.filtered_albums().len();
    assert_eq!(final_total, total, "Should restore full library");
}

/// Test year filter overrides decade filter
#[test]
fn test_year_overrides_decade_workflow() {
    let mut state = TestLibraryState::default().with_albums(create_diverse_library());

    // Select decade
    state.selected_decade = Some((1970, 1979));
    let decade_results = state.filtered_albums().len();

    // Select specific year within decade
    state.selected_year = Some(1973);
    let year_results = state.filtered_albums().len();
    assert!(
        year_results <= decade_results,
        "Year filter should narrow or equal decade"
    );

    // Select year outside decade - should still work (year overrides)
    state.selected_year = Some(1959);
    let jazz_year = state.filtered_albums().len();
    assert!(jazz_year >= 1, "Should find albums from 1959");

    // Clear year, decade should take over again
    state.selected_year = None;
    let back_to_decade = state.filtered_albums().len();
    assert_eq!(
        back_to_decade, decade_results,
        "Decade filter should be restored"
    );
}

// =============================================================================
// Sequence: Artist Navigation
// =============================================================================

/// Test artist letter → artist → album workflow
#[test]
fn test_artist_navigation_workflow() {
    let mut state = TestLibraryState::default().with_albums(create_diverse_library());

    // Select artist letter
    state.selected_artist_letter = Some('P');
    let p_artists = state.filtered_albums().len();
    assert_eq!(p_artists, 2, "Should have 2 Pink Floyd albums");

    // Select specific artist
    state.selected_artist = Some("Pink Floyd".to_string());
    let pink_floyd = state.filtered_albums().len();
    assert_eq!(pink_floyd, 2, "Should have 2 Pink Floyd albums");

    // Clear artist, keep letter
    state.selected_artist = None;
    let back_to_letter = state.filtered_albums().len();
    assert_eq!(
        back_to_letter, p_artists,
        "Should restore letter-filtered results"
    );

    // Clear letter
    state.selected_artist_letter = None;
    let all_albums = state.filtered_albums().len();
    assert!(all_albums > p_artists, "Should have more albums");
}

/// Test special character artist letter
#[test]
fn test_special_char_artist_letter() {
    let mut albums = create_diverse_library();
    albums.push(
        TestAlbum::new("1984", "Van Halen")
            .with_year(1984)
            .with_genre("Rock"),
    );
    albums.push(
        TestAlbum::new("2001 Space Odyssey", "Various")
            .with_year(1968)
            .with_genre("Soundtrack"),
    );

    let mut state = TestLibraryState::default().with_albums(albums);

    // # should match non-alphabetic starting artists
    // But our test data has numeric album titles, not artist names starting with numbers
    state.selected_artist_letter = Some('#');
    let special = state.filtered_albums().len();
    // In our test data, no artists start with numbers
    assert_eq!(special, 0, "No artists starting with numbers in test data");

    // Verify normal letters still work
    state.selected_artist_letter = Some('V');
    let v_artists = state.filtered_albums().len();
    assert!(v_artists >= 2, "Should find Van Halen and Various");
}

// =============================================================================
// Sequence: Pagination Workflows
// =============================================================================

/// Test pagination through filtered results
#[test]
fn test_pagination_with_filters() {
    let mut state = TestLibraryState::default().with_albums(create_diverse_library());
    state.items_per_page = 3;

    // Initial pagination
    let total_pages = state.total_pages();
    assert!(total_pages >= 3, "Should have multiple pages");

    // Apply filter that reduces results
    state.selected_genre = Some("Jazz".to_string());
    state.recalculate_pagination();
    let jazz_pages = state.total_pages();
    assert!(jazz_pages <= total_pages, "Filtered should have fewer pages");

    // Set page beyond new limit
    state.current_page = 10;
    state.recalculate_pagination();
    assert!(
        state.current_page <= jazz_pages.saturating_sub(1),
        "Page should be clamped"
    );

    // Clear filter
    state.selected_genre = None;
    state.recalculate_pagination();
    // Page should stay valid
    assert!(
        state.current_page <= state.total_pages().saturating_sub(1),
        "Page should remain valid"
    );
}

/// Test search affects pagination
#[test]
fn test_search_pagination_workflow() {
    let mut state = TestLibraryState::default().with_albums(create_diverse_library());
    state.items_per_page = 5;
    state.current_page = 1; // Second page

    // Search narrows results
    state.search_query = "Tool".to_string();
    state.recalculate_pagination();

    // With only 2 results and 5 per page, should be on page 0
    assert_eq!(state.current_page, 0, "Should reset to first page");
    assert_eq!(state.total_pages(), 1, "Should have 1 page");

    // Clear search
    state.search_query.clear();
    state.recalculate_pagination();
    assert!(state.total_pages() > 1, "Should have multiple pages again");
}

// =============================================================================
// Sequence: Complex Workflows
// =============================================================================

/// Simulate realistic library browsing session
#[test]
fn test_realistic_browsing_session() {
    let mut state = TestLibraryState::default().with_albums(create_diverse_library());
    state.items_per_page = 5;

    // User browses first page
    assert!(state.filtered_albums().len() > 0);

    // User searches for "Blue"
    state.search_query = "Blue".to_string();
    let blue_results = state.filtered_albums();
    assert!(
        blue_results.iter().any(|a| a.title.contains("Blue")),
        "Should find Kind of Blue"
    );

    // User clears search, applies genre filter
    state.search_query.clear();
    state.selected_genre = Some("Jazz".to_string());
    let jazz = state.filtered_albums();
    assert_eq!(jazz.len(), 3, "Should have 3 jazz albums");

    // User narrows to decade
    state.selected_decade = Some((1950, 1959));
    let jazz_50s = state.filtered_albums();
    assert_eq!(jazz_50s.len(), 2, "Should have 2 jazz albums from 50s");

    // User selects specific year
    state.selected_year = Some(1959);
    let jazz_1959 = state.filtered_albums();
    assert_eq!(jazz_1959.len(), 2, "Both 1959 jazz albums");

    // User searches within (bypasses selection filters)
    state.search_query = "Miles".to_string();
    let miles = state.filtered_albums();
    assert_eq!(miles.len(), 1, "Should find Miles Davis");

    // User clears everything to start fresh
    state.search_query.clear();
    state.selected_genre = None;
    state.selected_decade = None;
    state.selected_year = None;
    state.channel_filter = TestChannelFilter::All;

    let fresh = state.filtered_albums().len();
    assert_eq!(fresh, 12, "Should have all 12 albums");
}

/// Test filter state is preserved when switching between views
#[test]
fn test_filter_persistence_across_operations() {
    let mut state = TestLibraryState::default().with_albums(create_diverse_library());

    // Apply multiple filters
    state.selected_genre = Some("Rock".to_string());
    state.channel_filter = TestChannelFilter::Stereo;
    state.items_per_page = 10;
    state.current_page = 0;

    let initial = state.filtered_albums().len();

    // Simulate various operations that shouldn't affect filters
    let _ = state.total_pages();
    state.recalculate_pagination();
    let _ = state.filtered_albums();

    // Verify filters unchanged
    assert_eq!(state.selected_genre, Some("Rock".to_string()));
    assert_eq!(state.channel_filter, TestChannelFilter::Stereo);
    assert_eq!(state.items_per_page, 10);

    let after_ops = state.filtered_albums().len();
    assert_eq!(initial, after_ops, "Results should be unchanged");
}
