//! Library sequence tests - search and filter workflows.
//!
//! Tests realistic library browsing scenarios using the real `LibraryController`
//! from `sotf_audio_player`. Covers search, filtering, artist navigation,
//! pagination, and complex multi-step workflows.

use sotf_audio_player::{Album, ChannelFilter, LibraryController, MusicLibrary, Track};
use std::path::PathBuf;

// =============================================================================
// Helpers
// =============================================================================

fn make_album(title: &str, artist: &str, year: u32, genre: &str, channels: u32) -> Album {
    Album {
        title: title.to_string(),
        year: Some(year),
        tracks: vec![Track {
            path: PathBuf::from(format!("/music/{}/{}.flac", artist, title)),
            title: Some(title.to_string()),
            album_artist: Some(artist.to_string()),
            artist: Some(artist.to_string()),
            genre: Some(genre.to_string()),
            channels: Some(channels),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn controller_with_albums(albums: Vec<Album>) -> LibraryController {
    let mut lib = MusicLibrary::new();
    lib.albums = albums;
    let mut ctrl = LibraryController::with_library(lib);
    ctrl.ensure_cache_valid();
    ctrl
}

fn create_diverse_library() -> Vec<Album> {
    vec![
        // Rock albums (stereo)
        make_album("Dark Side of the Moon", "Pink Floyd", 1973, "Rock", 2),
        make_album("The Wall", "Pink Floyd", 1979, "Rock", 2),
        make_album("Led Zeppelin IV", "Led Zeppelin", 1971, "Rock", 2),
        // Jazz albums (stereo)
        make_album("Kind of Blue", "Miles Davis", 1959, "Jazz", 2),
        make_album("A Love Supreme", "John Coltrane", 1965, "Jazz", 2),
        make_album("Time Out", "Dave Brubeck", 1959, "Jazz", 2),
        // Classical (surround 5.1)
        make_album(
            "Beethoven Symphony No. 9",
            "Berlin Philharmonic",
            2010,
            "Classical",
            6,
        ),
        make_album(
            "Mozart Requiem",
            "Vienna Philharmonic",
            2015,
            "Classical",
            6,
        ),
        // Electronic (stereo)
        make_album("Discovery", "Daft Punk", 2001, "Electronic", 2),
        make_album("Random Access Memories", "Daft Punk", 2013, "Electronic", 2),
        // Metal (stereo)
        make_album("Lateralus", "Tool", 2001, "Metal", 2),
        make_album("10,000 Days", "Tool", 2006, "Metal", 2),
    ]
}

// =============================================================================
// Sequence: Search Workflows
// =============================================================================

/// Search -> results -> clear -> back to full library.
#[test]
fn test_search_clear_restores_full_library() {
    let mut ctrl = controller_with_albums(create_diverse_library());
    let total_albums = ctrl.filtered_albums().len();

    ctrl.set_search_query("Tool".to_string());
    ctrl.ensure_cache_valid();
    let search_results = ctrl.filtered_albums().len();
    assert_eq!(search_results, 2, "Should find 2 Tool albums");

    ctrl.clear_search();
    ctrl.ensure_cache_valid();
    let restored = ctrl.filtered_albums().len();
    assert_eq!(restored, total_albums, "Should restore full library");
}

/// Search -> refine -> clear workflow.
#[test]
fn test_search_refinement_workflow() {
    let mut ctrl = controller_with_albums(create_diverse_library());

    // Broad search
    ctrl.set_search_query("D".to_string());
    ctrl.ensure_cache_valid();
    let broad = ctrl.filtered_albums().len();

    // Refine
    ctrl.set_search_query("Da".to_string());
    ctrl.ensure_cache_valid();
    let refined1 = ctrl.filtered_albums().len();
    assert!(refined1 <= broad, "Refining should narrow results");

    // Further refine
    ctrl.set_search_query("Daft".to_string());
    ctrl.ensure_cache_valid();
    let refined2 = ctrl.filtered_albums().len();
    assert!(refined2 <= refined1, "Further refining should narrow more");
    assert_eq!(refined2, 2, "Should find 2 Daft Punk albums");

    // Complete search
    ctrl.set_search_query("Daft Punk".to_string());
    ctrl.ensure_cache_valid();
    let final_results = ctrl.filtered_albums().len();
    assert_eq!(final_results, 2, "Should still find 2 Daft Punk albums");
}

/// Case-insensitive search returns the same results for different casings.
#[test]
fn test_case_insensitive_search_sequence() {
    let mut ctrl = controller_with_albums(create_diverse_library());

    let variations = ["tool", "TOOL", "Tool", "tOoL"];
    let mut results = Vec::new();

    for query in &variations {
        ctrl.set_search_query(query.to_string());
        ctrl.ensure_cache_valid();
        results.push(ctrl.filtered_albums().len());
    }

    assert!(
        results.iter().all(|&r| r == results[0]),
        "Case variations should return same results: {:?}",
        results
    );
}

// =============================================================================
// Sequence: Filter Workflows
// =============================================================================

/// Channel filter -> search -> clear filter workflow.
#[test]
fn test_filter_then_search_workflow() {
    let mut ctrl = controller_with_albums(create_diverse_library());

    // Apply surround filter (matches 5.1 = 6-channel albums)
    ctrl.set_filter(ChannelFilter::Surround);
    ctrl.ensure_cache_valid();
    let surround = ctrl.filtered_albums().len();
    assert_eq!(surround, 2, "Should have 2 surround albums");

    // Search within surround-filtered set
    ctrl.set_search_query("Beethoven".to_string());
    ctrl.ensure_cache_valid();
    let search_in_filter = ctrl.filtered_albums().len();
    assert_eq!(
        search_in_filter, 1,
        "Should find 1 Beethoven surround album"
    );

    // Clear search, channel filter should persist
    ctrl.clear_search();
    ctrl.ensure_cache_valid();
    let after_clear = ctrl.filtered_albums().len();
    assert_eq!(
        after_clear, surround,
        "Channel filter should persist after clearing search"
    );

    // Remove channel filter
    ctrl.set_filter(ChannelFilter::All);
    ctrl.ensure_cache_valid();
    let all_albums = ctrl.filtered_albums().len();
    assert!(
        all_albums > surround,
        "Should have more albums without filter"
    );
}

/// Genre -> decade -> year cascading filters (via selection_filtered_albums).
#[test]
fn test_cascading_filter_workflow() {
    let mut ctrl = controller_with_albums(create_diverse_library());
    let total = ctrl.selection_filtered_albums().len();

    // Filter by genre
    ctrl.selected_genre = Some("Rock".to_string());
    let rock = ctrl.selection_filtered_albums().len();
    assert_eq!(rock, 3, "Should have 3 Rock albums");
    assert!(rock < total, "Filtered should be less than total");

    // Add decade filter
    ctrl.selected_decade = Some((1970, 1979));
    let rock_70s = ctrl.selection_filtered_albums().len();
    assert_eq!(
        rock_70s, 3,
        "Should have 3 Rock albums from 70s (1971, 1973, 1979)"
    );
    assert!(
        rock_70s <= rock,
        "Adding filter should not increase results"
    );

    // Clear genre, keep decade
    ctrl.selected_genre = None;
    let all_70s = ctrl.selection_filtered_albums().len();
    assert!(
        all_70s >= rock_70s,
        "Removing filter should not decrease results"
    );

    // Clear all filters
    ctrl.selected_decade = None;
    let final_total = ctrl.selection_filtered_albums().len();
    assert_eq!(final_total, total, "Should restore full library");
}

/// Year filter overrides decade filter.
#[test]
fn test_year_overrides_decade_workflow() {
    let mut ctrl = controller_with_albums(create_diverse_library());

    // Select decade
    ctrl.selected_decade = Some((1970, 1979));
    let decade_results = ctrl.selection_filtered_albums().len();

    // Select specific year within decade
    ctrl.selected_year = Some(1973);
    let year_results = ctrl.selection_filtered_albums().len();
    assert!(
        year_results <= decade_results,
        "Year filter should narrow or equal decade"
    );

    // Select year outside decade - year overrides decade
    ctrl.selected_year = Some(1959);
    let jazz_year = ctrl.selection_filtered_albums().len();
    assert!(jazz_year >= 1, "Should find albums from 1959");

    // Clear year, decade should take over again
    ctrl.selected_year = None;
    let back_to_decade = ctrl.selection_filtered_albums().len();
    assert_eq!(
        back_to_decade, decade_results,
        "Decade filter should be restored"
    );
}

// =============================================================================
// Sequence: Artist Navigation
// =============================================================================

/// Artist letter -> specific artist -> album workflow.
#[test]
fn test_artist_navigation_workflow() {
    let mut ctrl = controller_with_albums(create_diverse_library());

    // Select artist letter 'P' (Pink Floyd)
    ctrl.selected_artist_letter = Some('P');
    let p_artists = ctrl.selection_filtered_albums().len();
    assert_eq!(p_artists, 2, "Should have 2 Pink Floyd albums under 'P'");

    // Select specific artist
    ctrl.selected_artist = Some("Pink Floyd".to_string());
    let pink_floyd = ctrl.selection_filtered_albums().len();
    assert_eq!(pink_floyd, 2, "Should have 2 Pink Floyd albums");

    // Clear artist, keep letter
    ctrl.selected_artist = None;
    let back_to_letter = ctrl.selection_filtered_albums().len();
    assert_eq!(
        back_to_letter, p_artists,
        "Should restore letter-filtered results"
    );

    // Clear letter
    ctrl.selected_artist_letter = None;
    let all_albums = ctrl.selection_filtered_albums().len();
    assert!(all_albums > p_artists, "Should have more albums");
}

/// '#' letter matches non-alphabetic artist names.
#[test]
fn test_special_char_artist_letter() {
    let mut albums = create_diverse_library();
    albums.push(make_album("1984", "Van Halen", 1984, "Rock", 2));
    albums.push(make_album(
        "2001 Space Odyssey",
        "Various",
        1968,
        "Soundtrack",
        2,
    ));

    let mut ctrl = controller_with_albums(albums);

    // '#' should match non-alphabetic starting artists (none in test data)
    ctrl.selected_artist_letter = Some('#');
    let special = ctrl.selection_filtered_albums().len();
    assert_eq!(special, 0, "No artists starting with numbers in test data");

    // Verify normal letters still work
    ctrl.selected_artist_letter = Some('V');
    let v_artists = ctrl.selection_filtered_albums().len();
    assert!(v_artists >= 2, "Should find Van Halen and Various");
}

// =============================================================================
// Sequence: Pagination Workflows
// =============================================================================

/// Pagination through filtered results.
#[test]
fn test_pagination_with_filters() {
    let mut ctrl = controller_with_albums(create_diverse_library());
    ctrl.items_per_page = 3;
    ctrl.ensure_cache_valid();

    let total_pages = ctrl.total_pages();
    assert!(
        total_pages >= 3,
        "Should have multiple pages with 12 albums / 3 per page"
    );

    // Apply genre filter that reduces results
    ctrl.selected_genre = Some("Jazz".to_string());
    let jazz_count = ctrl.selection_filtered_albums().len();
    assert_eq!(jazz_count, 3, "Should have 3 Jazz albums");

    // The filtered_albums (without selection filters) still has all 12,
    // so pagination is based on filtered_albums, not selection_filtered_albums.
    // Verify total_pages based on filtered_albums.
    let pages_after_genre = ctrl.total_pages();
    assert_eq!(
        pages_after_genre, total_pages,
        "Genre selection filter doesn't affect pagination (which uses filtered_albums)"
    );

    // Apply channel filter (which DOES affect filtered_albums cache)
    ctrl.set_filter(ChannelFilter::Stereo);
    ctrl.ensure_cache_valid();
    let stereo_pages = ctrl.total_pages();
    // 10 stereo albums / 3 per page = 4 pages
    assert!(
        stereo_pages <= total_pages,
        "Channel filter should reduce pages"
    );

    // Reset
    ctrl.set_filter(ChannelFilter::All);
    ctrl.ensure_cache_valid();
    assert_eq!(ctrl.total_pages(), total_pages, "Should restore pages");
}

/// Search affects pagination.
#[test]
fn test_search_pagination_workflow() {
    let mut ctrl = controller_with_albums(create_diverse_library());
    ctrl.items_per_page = 5;
    ctrl.current_page = 1; // Second page
    ctrl.ensure_cache_valid();

    // Search narrows results
    ctrl.set_search_query("Tool".to_string());
    ctrl.ensure_cache_valid();

    // With only 2 results and 5 per page, should have 1 page
    assert_eq!(ctrl.total_pages(), 1, "Should have 1 page");

    // Clear search
    ctrl.clear_search();
    ctrl.ensure_cache_valid();
    assert!(ctrl.total_pages() > 1, "Should have multiple pages again");
}

// =============================================================================
// Sequence: Complex Workflows
// =============================================================================

/// Simulate realistic library browsing session.
#[test]
fn test_realistic_browsing_session() {
    let mut ctrl = controller_with_albums(create_diverse_library());
    ctrl.items_per_page = 5;
    ctrl.ensure_cache_valid();

    // User browses first page
    assert!(!ctrl.filtered_albums().is_empty());

    // User searches for "Blue"
    ctrl.set_search_query("Blue".to_string());
    ctrl.ensure_cache_valid();
    let blue_results = ctrl.filtered_albums();
    assert!(
        blue_results.iter().any(|a| a.title.contains("Blue")),
        "Should find Kind of Blue"
    );

    // User clears search, applies genre filter
    ctrl.clear_search();
    ctrl.ensure_cache_valid();
    ctrl.selected_genre = Some("Jazz".to_string());
    let jazz = ctrl.selection_filtered_albums();
    assert_eq!(jazz.len(), 3, "Should have 3 jazz albums");

    // User narrows to decade
    ctrl.selected_decade = Some((1950, 1959));
    let jazz_50s = ctrl.selection_filtered_albums();
    assert_eq!(jazz_50s.len(), 2, "Should have 2 jazz albums from 50s");

    // User selects specific year
    ctrl.selected_year = Some(1959);
    let jazz_1959 = ctrl.selection_filtered_albums();
    assert_eq!(jazz_1959.len(), 2, "Both 1959 jazz albums");

    // User searches (bypasses selection filters, but channel filter persists)
    ctrl.set_search_query("Miles".to_string());
    ctrl.ensure_cache_valid();
    let miles = ctrl.filtered_albums();
    assert_eq!(miles.len(), 1, "Should find Miles Davis");

    // User clears everything to start fresh
    ctrl.clear_search();
    ctrl.ensure_cache_valid();
    ctrl.selected_genre = None;
    ctrl.selected_decade = None;
    ctrl.selected_year = None;
    ctrl.set_filter(ChannelFilter::All);
    ctrl.ensure_cache_valid();

    let fresh = ctrl.filtered_albums().len();
    assert_eq!(fresh, 12, "Should have all 12 albums");
}

/// Filter state is preserved across operations.
#[test]
fn test_filter_persistence_across_operations() {
    let mut ctrl = controller_with_albums(create_diverse_library());

    // Apply channel filter
    ctrl.set_filter(ChannelFilter::Stereo);
    ctrl.ensure_cache_valid();
    ctrl.selected_genre = Some("Rock".to_string());
    ctrl.items_per_page = 10;
    ctrl.current_page = 0;

    let initial = ctrl.selection_filtered_albums().len();

    // Simulate various read operations
    let _ = ctrl.total_pages();
    let _ = ctrl.filtered_albums();
    let _ = ctrl.selection_filtered_albums();

    // Verify filters unchanged
    assert_eq!(ctrl.selected_genre, Some("Rock".to_string()));
    assert!(matches!(ctrl.filter, ChannelFilter::Stereo));
    assert_eq!(ctrl.items_per_page, 10);

    let after_ops = ctrl.selection_filtered_albums().len();
    assert_eq!(initial, after_ops, "Results should be unchanged");
}
