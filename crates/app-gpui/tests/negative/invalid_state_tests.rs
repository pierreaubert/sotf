//! Invalid State Tests
//!
//! These tests verify that invalid states are either rejected or handled gracefully
//! by the REAL production controllers (LibraryController, QueueController, PlaybackController).

use std::path::PathBuf;

use sotf_audio_player::controllers::library::LibraryController;
use sotf_audio_player::controllers::playback::PlaybackController;
use sotf_audio_player::controllers::queue::QueueController;
use sotf_audio_player::{Album, ChannelFilter, MusicLibrary, Track};

/// Helper: create an album with N stereo tracks.
fn make_album(title: &str, artist: &str, track_count: usize) -> Album {
    Album {
        title: title.to_string(),
        tracks: (0..track_count)
            .map(|i| Track {
                path: PathBuf::from(format!("/music/{}/track_{}.flac", title, i + 1)),
                title: Some(format!("Track {}", i + 1)),
                artist: Some(artist.to_string()),
                channels: Some(2),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

fn make_album_with_genre(title: &str, artist: &str, genre: &str) -> Album {
    Album {
        title: title.to_string(),
        tracks: vec![Track {
            path: PathBuf::from(format!("/music/{}/track.flac", title)),
            title: Some("Track 1".to_string()),
            artist: Some(artist.to_string()),
            album_artist: Some(artist.to_string()),
            genre: Some(genre.to_string()),
            channels: Some(2),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn make_album_with_year(title: &str, artist: &str, year: u32) -> Album {
    Album {
        title: title.to_string(),
        year: Some(year),
        tracks: vec![Track {
            path: PathBuf::from(format!("/music/{}/track.flac", title)),
            title: Some("Track 1".to_string()),
            artist: Some(artist.to_string()),
            album_artist: Some(artist.to_string()),
            channels: Some(2),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn library_with_albums(albums: Vec<Album>) -> LibraryController {
    let mut lib = MusicLibrary::new();
    lib.albums = albums;
    let mut ctrl = LibraryController::with_library(lib);
    ctrl.ensure_cache_valid();
    ctrl
}

// =============================================================================
// Library State Validation
// =============================================================================

/// Test: Page index beyond valid range returns empty paginated results
#[test]
fn test_page_index_beyond_valid_returns_empty() {
    let albums: Vec<Album> = (0..50)
        .map(|i| make_album(&format!("Album {}", i), "Artist", 1))
        .collect();
    let mut ctrl = library_with_albums(albums);
    ctrl.items_per_page = 10;
    ctrl.current_page = 100; // Way beyond valid range

    let paginated = ctrl.get_paginated_albums();
    assert!(
        paginated.is_empty(),
        "Paginated result should be empty for out-of-range page"
    );
}

/// Test: Page index valid with empty library
#[test]
fn test_total_pages_empty_library() {
    let ctrl = library_with_albums(vec![]);
    assert_eq!(ctrl.total_pages(), 1, "Empty library should report 1 page");
}

/// Test: Filtering to empty result doesn't crash
#[test]
fn test_filter_to_empty_result() {
    let albums = vec![
        make_album_with_genre("Rock Album", "Artist", "Rock"),
        make_album_with_genre("Pop Album", "Artist", "Pop"),
    ];
    let mut ctrl = library_with_albums(albums);

    // Filter by non-existent genre
    ctrl.selected_genre = Some("Jazz".to_string());

    let filtered = ctrl.selection_filtered_albums();
    assert_eq!(filtered.len(), 0, "Should return empty, not crash");
}

/// Test: Search with unicode characters
#[test]
fn test_search_unicode() {
    let albums = vec![
        make_album("日本語アルバム", "アーティスト", 1),
        make_album("English Album", "Artist", 1),
    ];
    let mut ctrl = library_with_albums(albums);
    ctrl.set_search_query("日本語".to_string());
    ctrl.ensure_cache_valid();

    let filtered = ctrl.filtered_albums();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "日本語アルバム");
}

// =============================================================================
// Queue State Validation
// =============================================================================

/// Test: Next track on empty queue returns Stop
#[test]
fn test_next_track_empty_queue() {
    use sotf_audio_player::controllers::queue::QueuePlaybackEffect;

    let mut ctrl = QueueController::new();
    let effect = ctrl.next_track();
    assert_eq!(effect, QueuePlaybackEffect::Stop);
}

/// Test: Next track at end of single-track album returns Stop
#[test]
fn test_next_track_at_end() {
    use sotf_audio_player::controllers::queue::QueuePlaybackEffect;

    let mut ctrl = QueueController::new();
    ctrl.add(make_album("Album", "Artist", 1));
    ctrl.start();

    let effect = ctrl.next_track();
    assert_eq!(effect, QueuePlaybackEffect::Stop);
}

/// Test: Remove from empty queue is no-op
#[test]
fn test_remove_from_empty_queue() {
    use sotf_audio_player::controllers::queue::QueuePlaybackEffect;

    let mut ctrl = QueueController::new();
    let (effect, was_current) = ctrl.remove(0);
    assert_eq!(effect, QueuePlaybackEffect::None);
    assert!(!was_current);
}

/// Test: Jump to invalid index is no-op
#[test]
fn test_jump_to_invalid_index() {
    use sotf_audio_player::controllers::queue::QueuePlaybackEffect;

    let mut ctrl = QueueController::new();
    ctrl.add(make_album("Album", "Artist", 2));
    ctrl.start();

    let effect = ctrl.jump_to(99);
    assert_eq!(effect, QueuePlaybackEffect::None);
}

// =============================================================================
// Playback State Validation
// =============================================================================

/// Test: Volume clamping prevents invalid values
#[test]
fn test_volume_clamping() {
    let mut ctrl = PlaybackController::new();

    ctrl.set_volume(-0.5);
    assert_eq!(ctrl.volume, 0.0, "Negative volume not clamped");

    ctrl.set_volume(2.0);
    assert_eq!(ctrl.volume, 1.0, "Volume > 1.0 not clamped");

    ctrl.set_volume(0.5);
    assert_eq!(ctrl.volume, 0.5, "Valid volume was modified");
}

/// Test: Effective volume is 0.0 when muted
#[test]
fn test_effective_volume_muted() {
    let mut ctrl = PlaybackController::new();
    ctrl.set_volume(0.8);
    ctrl.muted = true;
    assert_eq!(ctrl.effective_volume(), 0.0);

    ctrl.toggle_mute();
    assert_eq!(ctrl.effective_volume(), 0.8);
}

/// Test: Volume steps stay within bounds
#[test]
fn test_volume_increase_clamped() {
    let mut ctrl = PlaybackController::new();
    ctrl.set_volume(0.98);
    ctrl.increase_volume(); // Should not exceed 1.0
    assert!(ctrl.volume <= 1.0, "Volume exceeded 1.0 after increase");
}

#[test]
fn test_volume_decrease_clamped() {
    let mut ctrl = PlaybackController::new();
    ctrl.set_volume(0.02);
    ctrl.decrease_volume(); // Should not go below 0.0
    assert!(ctrl.volume >= 0.0, "Volume went below 0.0 after decrease");
}

// =============================================================================
// Filter Combination Tests
// =============================================================================

/// Test: Multiple selection filters combine correctly
#[test]
fn test_multiple_filters_combine() {
    let albums = vec![
        {
            let mut a = make_album_with_genre("A1", "Artist 1", "Rock");
            a.year = Some(2020);
            a
        },
        {
            let mut a = make_album_with_genre("A2", "Artist 1", "Jazz");
            a.year = Some(2020);
            a
        },
        {
            let mut a = make_album_with_genre("A3", "Artist 2", "Rock");
            a.year = Some(2020);
            // multichannel
            a.tracks[0].channels = Some(6);
            a
        },
        {
            let mut a = make_album_with_genre("A4", "Artist 2", "Rock");
            a.year = Some(2019);
            a
        },
    ];

    let mut ctrl = library_with_albums(albums);

    // Apply multiple filters
    ctrl.selected_genre = Some("Rock".to_string());
    ctrl.selected_year = Some(2020);
    ctrl.set_filter(ChannelFilter::Stereo);
    ctrl.ensure_cache_valid();

    let filtered = ctrl.selection_filtered_albums();

    // Only A1 matches all criteria (Rock + 2020 + Stereo)
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "A1");
}

/// Test: Search bypasses selection filters
#[test]
fn test_search_bypasses_selection_filters() {
    let albums = vec![
        {
            let mut a = make_album_with_genre("Target Album", "Artist 1", "Jazz");
            a.year = Some(2020);
            a
        },
        {
            let mut a = make_album_with_genre("Other Album", "Artist 2", "Rock");
            a.year = Some(2019);
            a
        },
    ];

    let mut ctrl = library_with_albums(albums);

    // Apply restrictive selection filters
    ctrl.selected_genre = Some("Rock".to_string());
    ctrl.selected_year = Some(2019);

    // Without search: only "Other Album" matches
    assert_eq!(ctrl.selection_filtered_albums().len(), 1);
    assert_eq!(ctrl.selection_filtered_albums()[0].title, "Other Album");

    // With search: bypasses selection filters, finds Target
    ctrl.set_search_query("Target".to_string());
    ctrl.ensure_cache_valid();

    let filtered = ctrl.selection_filtered_albums();
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
        make_album_with_year("90s Album", "Artist", 1995),
        make_album_with_year("2000s Album", "Artist", 2005),
        make_album_with_year("2010s Album", "Artist", 2015),
    ];

    let mut ctrl = library_with_albums(albums);
    ctrl.selected_decade = Some((2000, 2009));

    let filtered = ctrl.selection_filtered_albums();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "2000s Album");
}

/// Test: Year filter overrides decade
#[test]
fn test_year_overrides_decade() {
    let albums = vec![
        make_album_with_year("2005 Album", "Artist", 2005),
        make_album_with_year("2007 Album", "Artist", 2007),
    ];

    let mut ctrl = library_with_albums(albums);
    ctrl.selected_decade = Some((2000, 2009));
    ctrl.selected_year = Some(2005);

    let filtered = ctrl.selection_filtered_albums();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "2005 Album");
}

// =============================================================================
// Artist Letter Filter Tests
// =============================================================================

/// Test: Artist letter filter with '#' for non-alphabetic
#[test]
fn test_artist_letter_special_char() {
    let albums = vec![
        make_album("Album 1", "123 Artist", 1),
        make_album("Album 2", "Aaron", 1),
        make_album("Album 3", "Bob", 1),
    ];

    // Need album_artist set for artist() to work
    let albums: Vec<Album> = albums
        .into_iter()
        .map(|mut a| {
            let artist = a.tracks[0].artist.clone();
            for t in &mut a.tracks {
                t.album_artist = artist.clone();
            }
            a
        })
        .collect();

    let mut ctrl = library_with_albums(albums);
    ctrl.selected_artist_letter = Some('#');

    let filtered = ctrl.selection_filtered_albums();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].artist(), "123 Artist");
}

/// Test: Artist letter filter case insensitive
#[test]
fn test_artist_letter_case_insensitive() {
    let albums = vec![
        make_album("Album 1", "Aaron", 1),
        make_album("Album 2", "adam", 1),
        make_album("Album 3", "Bob", 1),
    ];

    let albums: Vec<Album> = albums
        .into_iter()
        .map(|mut a| {
            let artist = a.tracks[0].artist.clone();
            for t in &mut a.tracks {
                t.album_artist = artist.clone();
            }
            a
        })
        .collect();

    let mut ctrl = library_with_albums(albums);
    ctrl.selected_artist_letter = Some('A');

    let filtered = ctrl.selection_filtered_albums();
    assert_eq!(filtered.len(), 2);
}
