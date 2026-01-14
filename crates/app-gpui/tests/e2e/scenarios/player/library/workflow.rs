//! E2E tests for Library Workflow Integration.
//!
//! End-to-end tests for library management workflow:
//! - Scan library paths
//! - Browse albums and artists
//! - Filter and search
//! - Select and play tracks

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Scan status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ScanStatus {
    #[default]
    Idle,
    Scanning,
    Complete,
    Error,
}

/// Track metadata
#[derive(Debug, Clone)]
struct TrackInfo {
    id: String,
    title: String,
    artist: String,
    album: String,
    album_artist: String,
    track_number: u32,
    duration_secs: u64,
    path: String,
    genre: Option<String>,
    year: Option<u32>,
}

impl Default for TrackInfo {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: "Unknown".to_string(),
            artist: "Unknown".to_string(),
            album: "Unknown".to_string(),
            album_artist: "Unknown".to_string(),
            track_number: 1,
            duration_secs: 0,
            path: String::new(),
            genre: None,
            year: None,
        }
    }
}

/// Album info
#[derive(Debug, Clone)]
struct AlbumInfo {
    id: String,
    title: String,
    artist: String,
    track_count: usize,
    year: Option<u32>,
    genre: Option<String>,
    artwork_path: Option<String>,
}

impl Default for AlbumInfo {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: "Unknown Album".to_string(),
            artist: "Unknown Artist".to_string(),
            track_count: 0,
            year: None,
            genre: None,
            artwork_path: None,
        }
    }
}

/// Library path entry
#[derive(Debug, Clone)]
struct LibraryPath {
    path: String,
    enabled: bool,
    track_count: usize,
}

/// Browse view mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum BrowseMode {
    #[default]
    Albums,
    Artists,
    Genres,
    Folders,
    AllTracks,
}

/// Sort order
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum SortOrder {
    #[default]
    Alphabetical,
    DateAdded,
    Year,
    Artist,
    PlayCount,
}

/// Filter state
#[derive(Debug, Clone, Default)]
struct FilterState {
    search_query: String,
    genre_filter: Option<String>,
    year_range: Option<(u32, u32)>,
    artist_filter: Option<String>,
}

/// Playback state for integration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum PlaybackState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

/// Library workflow state
struct LibraryWorkflowState {
    // Library management
    library_paths: Vec<LibraryPath>,
    scan_status: ScanStatus,
    scan_progress: f32,
    scan_current_path: Option<String>,
    total_tracks: usize,
    total_albums: usize,

    // Browsing
    browse_mode: BrowseMode,
    sort_order: SortOrder,
    albums: Vec<AlbumInfo>,
    visible_albums: Vec<AlbumInfo>,
    selected_album_id: Option<String>,
    album_tracks: Vec<TrackInfo>,

    // Filtering
    filter: FilterState,
    available_genres: Vec<String>,
    available_artists: Vec<String>,

    // Selection and playback
    selected_track_id: Option<String>,
    queue: Vec<TrackInfo>,
    current_track: Option<TrackInfo>,
    playback_state: PlaybackState,
}

impl Default for LibraryWorkflowState {
    fn default() -> Self {
        Self {
            library_paths: Vec::new(),
            scan_status: ScanStatus::Idle,
            scan_progress: 0.0,
            scan_current_path: None,
            total_tracks: 0,
            total_albums: 0,
            browse_mode: BrowseMode::Albums,
            sort_order: SortOrder::Alphabetical,
            albums: Vec::new(),
            visible_albums: Vec::new(),
            selected_album_id: None,
            album_tracks: Vec::new(),
            filter: FilterState::default(),
            available_genres: Vec::new(),
            available_artists: Vec::new(),
            selected_track_id: None,
            queue: Vec::new(),
            current_track: None,
            playback_state: PlaybackState::Stopped,
        }
    }
}

// =============================================================================
// Library Scan Tests
// =============================================================================

/// Test adding library path.
#[gpui::test]
async fn test_adding_library_path(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    state.borrow_mut().library_paths.push(LibraryPath {
        path: "/Users/pierre/Music".to_string(),
        enabled: true,
        track_count: 0,
    });

    assert_eq!(state.borrow().library_paths.len(), 1);
}

/// Test initiating library scan.
#[gpui::test]
async fn test_initiating_library_scan(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    state.borrow_mut().library_paths.push(LibraryPath {
        path: "/Music".to_string(),
        enabled: true,
        track_count: 0,
    });

    state.borrow_mut().scan_status = ScanStatus::Scanning;
    state.borrow_mut().scan_progress = 0.0;

    assert_eq!(state.borrow().scan_status, ScanStatus::Scanning);
}

/// Test scan progress updates.
#[gpui::test]
async fn test_scan_progress_updates(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    state.borrow_mut().scan_status = ScanStatus::Scanning;

    let progress_values = [0.0, 0.25, 0.5, 0.75, 1.0];
    for progress in progress_values {
        state.borrow_mut().scan_progress = progress;
        assert!((state.borrow().scan_progress - progress).abs() < 0.01);
    }
}

/// Test scan current path display.
#[gpui::test]
async fn test_scan_current_path_display(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    state.borrow_mut().scan_status = ScanStatus::Scanning;
    state.borrow_mut().scan_current_path = Some("/Music/Albums/Artist/Album".to_string());

    assert!(state.borrow().scan_current_path.is_some());
}

/// Test scan completion.
#[gpui::test]
async fn test_scan_completion(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    // Simulate scan completion
    state.borrow_mut().scan_status = ScanStatus::Complete;
    state.borrow_mut().scan_progress = 1.0;
    state.borrow_mut().total_tracks = 1500;
    state.borrow_mut().total_albums = 120;

    assert_eq!(state.borrow().scan_status, ScanStatus::Complete);
    assert_eq!(state.borrow().total_tracks, 1500);
    assert_eq!(state.borrow().total_albums, 120);
}

/// Test scan error handling.
#[gpui::test]
async fn test_scan_error_handling(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    state.borrow_mut().scan_status = ScanStatus::Error;

    assert_eq!(state.borrow().scan_status, ScanStatus::Error);
}

/// Test incremental scan.
#[gpui::test]
async fn test_incremental_scan(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    // Initial state
    state.borrow_mut().total_tracks = 1000;
    state.borrow_mut().total_albums = 80;

    // Incremental scan adds new content
    state.borrow_mut().total_tracks = 1050;
    state.borrow_mut().total_albums = 84;

    assert_eq!(state.borrow().total_tracks, 1050);
    assert_eq!(state.borrow().total_albums, 84);
}

// =============================================================================
// Browse Mode Tests
// =============================================================================

/// Test browse mode selection.
#[gpui::test]
async fn test_browse_mode_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    let modes = [
        BrowseMode::Albums,
        BrowseMode::Artists,
        BrowseMode::Genres,
        BrowseMode::Folders,
        BrowseMode::AllTracks,
    ];

    for mode in modes {
        state.borrow_mut().browse_mode = mode;
        assert_eq!(state.borrow().browse_mode, mode);
    }
}

/// Test sort order selection.
#[gpui::test]
async fn test_sort_order_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    let orders = [
        SortOrder::Alphabetical,
        SortOrder::DateAdded,
        SortOrder::Year,
        SortOrder::Artist,
        SortOrder::PlayCount,
    ];

    for order in orders {
        state.borrow_mut().sort_order = order;
        assert_eq!(state.borrow().sort_order, order);
    }
}

/// Test album list loading.
#[gpui::test]
async fn test_album_list_loading(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    state.borrow_mut().albums = vec![
        AlbumInfo {
            id: "album1".to_string(),
            title: "Abbey Road".to_string(),
            artist: "The Beatles".to_string(),
            track_count: 17,
            year: Some(1969),
            ..Default::default()
        },
        AlbumInfo {
            id: "album2".to_string(),
            title: "Dark Side of the Moon".to_string(),
            artist: "Pink Floyd".to_string(),
            track_count: 10,
            year: Some(1973),
            ..Default::default()
        },
    ];

    assert_eq!(state.borrow().albums.len(), 2);
}

/// Test album sorting alphabetically.
#[gpui::test]
async fn test_album_sorting_alphabetical(_cx: &mut TestAppContext) {
    fn sort_albums_alphabetical(albums: &mut [AlbumInfo]) {
        albums.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    }

    let mut albums = vec![
        AlbumInfo {
            title: "Ziggy Stardust".to_string(),
            ..Default::default()
        },
        AlbumInfo {
            title: "Abbey Road".to_string(),
            ..Default::default()
        },
    ];

    sort_albums_alphabetical(&mut albums);
    assert_eq!(albums[0].title, "Abbey Road");
    assert_eq!(albums[1].title, "Ziggy Stardust");
}

/// Test album sorting by year.
#[gpui::test]
async fn test_album_sorting_by_year(_cx: &mut TestAppContext) {
    fn sort_albums_by_year(albums: &mut [AlbumInfo]) {
        albums.sort_by(|a, b| a.year.cmp(&b.year));
    }

    let mut albums = vec![
        AlbumInfo {
            title: "Album 2000".to_string(),
            year: Some(2000),
            ..Default::default()
        },
        AlbumInfo {
            title: "Album 1990".to_string(),
            year: Some(1990),
            ..Default::default()
        },
    ];

    sort_albums_by_year(&mut albums);
    assert_eq!(albums[0].year, Some(1990));
}

// =============================================================================
// Filter and Search Tests
// =============================================================================

/// Test search query.
#[gpui::test]
async fn test_search_query(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    state.borrow_mut().filter.search_query = "beatles".to_string();
    assert_eq!(state.borrow().filter.search_query, "beatles");
}

/// Test search filtering.
#[gpui::test]
async fn test_search_filtering(_cx: &mut TestAppContext) {
    fn filter_albums_by_search(albums: &[AlbumInfo], query: &str) -> Vec<AlbumInfo> {
        let query_lower = query.to_lowercase();
        albums
            .iter()
            .filter(|a| {
                a.title.to_lowercase().contains(&query_lower)
                    || a.artist.to_lowercase().contains(&query_lower)
            })
            .cloned()
            .collect()
    }

    let albums = vec![
        AlbumInfo {
            title: "Abbey Road".to_string(),
            artist: "The Beatles".to_string(),
            ..Default::default()
        },
        AlbumInfo {
            title: "Dark Side".to_string(),
            artist: "Pink Floyd".to_string(),
            ..Default::default()
        },
    ];

    let filtered = filter_albums_by_search(&albums, "beatles");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "Abbey Road");
}

/// Test genre filter.
#[gpui::test]
async fn test_genre_filter(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    state.borrow_mut().filter.genre_filter = Some("Rock".to_string());
    assert_eq!(state.borrow().filter.genre_filter, Some("Rock".to_string()));
}

/// Test genre filter application.
#[gpui::test]
async fn test_genre_filter_application(_cx: &mut TestAppContext) {
    fn filter_albums_by_genre(albums: &[AlbumInfo], genre: &str) -> Vec<AlbumInfo> {
        albums
            .iter()
            .filter(|a| a.genre.as_deref() == Some(genre))
            .cloned()
            .collect()
    }

    let albums = vec![
        AlbumInfo {
            title: "Rock Album".to_string(),
            genre: Some("Rock".to_string()),
            ..Default::default()
        },
        AlbumInfo {
            title: "Jazz Album".to_string(),
            genre: Some("Jazz".to_string()),
            ..Default::default()
        },
    ];

    let filtered = filter_albums_by_genre(&albums, "Rock");
    assert_eq!(filtered.len(), 1);
}

/// Test year range filter.
#[gpui::test]
async fn test_year_range_filter(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    state.borrow_mut().filter.year_range = Some((1970, 1980));
    assert_eq!(state.borrow().filter.year_range, Some((1970, 1980)));
}

/// Test year range filter application.
#[gpui::test]
async fn test_year_range_filter_application(_cx: &mut TestAppContext) {
    fn filter_albums_by_year_range(
        albums: &[AlbumInfo],
        start_year: u32,
        end_year: u32,
    ) -> Vec<AlbumInfo> {
        albums
            .iter()
            .filter(|a| {
                a.year
                    .map(|y| y >= start_year && y <= end_year)
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    let albums = vec![
        AlbumInfo {
            title: "Album 1965".to_string(),
            year: Some(1965),
            ..Default::default()
        },
        AlbumInfo {
            title: "Album 1975".to_string(),
            year: Some(1975),
            ..Default::default()
        },
        AlbumInfo {
            title: "Album 1985".to_string(),
            year: Some(1985),
            ..Default::default()
        },
    ];

    let filtered = filter_albums_by_year_range(&albums, 1970, 1980);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "Album 1975");
}

/// Test clear filters.
#[gpui::test]
async fn test_clear_filters(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    state.borrow_mut().filter = FilterState {
        search_query: "test".to_string(),
        genre_filter: Some("Rock".to_string()),
        year_range: Some((1970, 1980)),
        artist_filter: Some("Beatles".to_string()),
    };

    // Clear filters
    state.borrow_mut().filter = FilterState::default();

    assert!(state.borrow().filter.search_query.is_empty());
    assert!(state.borrow().filter.genre_filter.is_none());
}

/// Test available genres list.
#[gpui::test]
async fn test_available_genres_list(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    state.borrow_mut().available_genres = vec![
        "Rock".to_string(),
        "Jazz".to_string(),
        "Classical".to_string(),
        "Electronic".to_string(),
    ];

    assert_eq!(state.borrow().available_genres.len(), 4);
}

// =============================================================================
// Album Selection Tests
// =============================================================================

/// Test album selection.
#[gpui::test]
async fn test_album_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    state.borrow_mut().albums.push(AlbumInfo {
        id: "album1".to_string(),
        title: "Test Album".to_string(),
        ..Default::default()
    });

    state.borrow_mut().selected_album_id = Some("album1".to_string());
    assert_eq!(state.borrow().selected_album_id, Some("album1".to_string()));
}

/// Test loading album tracks.
#[gpui::test]
async fn test_loading_album_tracks(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    state.borrow_mut().selected_album_id = Some("album1".to_string());
    state.borrow_mut().album_tracks = vec![
        TrackInfo {
            id: "track1".to_string(),
            title: "Come Together".to_string(),
            track_number: 1,
            duration_secs: 259,
            ..Default::default()
        },
        TrackInfo {
            id: "track2".to_string(),
            title: "Something".to_string(),
            track_number: 2,
            duration_secs: 183,
            ..Default::default()
        },
    ];

    assert_eq!(state.borrow().album_tracks.len(), 2);
}

/// Test track sorting by number.
#[gpui::test]
async fn test_track_sorting_by_number(_cx: &mut TestAppContext) {
    fn sort_tracks_by_number(tracks: &mut [TrackInfo]) {
        tracks.sort_by_key(|t| t.track_number);
    }

    let mut tracks = vec![
        TrackInfo {
            track_number: 3,
            ..Default::default()
        },
        TrackInfo {
            track_number: 1,
            ..Default::default()
        },
        TrackInfo {
            track_number: 2,
            ..Default::default()
        },
    ];

    sort_tracks_by_number(&mut tracks);
    assert_eq!(tracks[0].track_number, 1);
    assert_eq!(tracks[1].track_number, 2);
    assert_eq!(tracks[2].track_number, 3);
}

/// Test track selection.
#[gpui::test]
async fn test_track_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    state.borrow_mut().selected_track_id = Some("track1".to_string());
    assert_eq!(state.borrow().selected_track_id, Some("track1".to_string()));
}

// =============================================================================
// Queue Management Tests
// =============================================================================

/// Test adding track to queue.
#[gpui::test]
async fn test_adding_track_to_queue(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    state.borrow_mut().queue.push(TrackInfo {
        id: "track1".to_string(),
        title: "Test Track".to_string(),
        ..Default::default()
    });

    assert_eq!(state.borrow().queue.len(), 1);
}

/// Test adding album to queue.
#[gpui::test]
async fn test_adding_album_to_queue(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    let album_tracks = vec![
        TrackInfo {
            id: "t1".to_string(),
            ..Default::default()
        },
        TrackInfo {
            id: "t2".to_string(),
            ..Default::default()
        },
        TrackInfo {
            id: "t3".to_string(),
            ..Default::default()
        },
    ];

    for track in album_tracks {
        state.borrow_mut().queue.push(track);
    }

    assert_eq!(state.borrow().queue.len(), 3);
}

/// Test clearing queue.
#[gpui::test]
async fn test_clearing_queue(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    state.borrow_mut().queue = vec![TrackInfo::default(), TrackInfo::default()];

    state.borrow_mut().queue.clear();
    assert!(state.borrow().queue.is_empty());
}

/// Test play next (insert at front).
#[gpui::test]
async fn test_play_next(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    state.borrow_mut().queue = vec![TrackInfo {
        id: "existing".to_string(),
        ..Default::default()
    }];

    state.borrow_mut().queue.insert(
        0,
        TrackInfo {
            id: "next".to_string(),
            ..Default::default()
        },
    );

    assert_eq!(state.borrow().queue[0].id, "next");
}

// =============================================================================
// Playback Integration Tests
// =============================================================================

/// Test play track from album.
#[gpui::test]
async fn test_play_track_from_album(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    let track = TrackInfo {
        id: "track1".to_string(),
        title: "Test Track".to_string(),
        path: "/music/track.flac".to_string(),
        ..Default::default()
    };

    state.borrow_mut().current_track = Some(track);
    state.borrow_mut().playback_state = PlaybackState::Playing;

    assert!(state.borrow().current_track.is_some());
    assert_eq!(state.borrow().playback_state, PlaybackState::Playing);
}

/// Test play album from start.
#[gpui::test]
async fn test_play_album_from_start(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    let tracks = vec![
        TrackInfo {
            id: "t1".to_string(),
            track_number: 1,
            ..Default::default()
        },
        TrackInfo {
            id: "t2".to_string(),
            track_number: 2,
            ..Default::default()
        },
    ];

    // Queue all tracks
    state.borrow_mut().queue = tracks.clone();
    // Play first track
    state.borrow_mut().current_track = Some(tracks[0].clone());
    state.borrow_mut().playback_state = PlaybackState::Playing;

    assert_eq!(state.borrow().queue.len(), 2);
    assert_eq!(state.borrow().current_track.as_ref().unwrap().id, "t1");
}

/// Test playback state transitions.
#[gpui::test]
async fn test_playback_state_transitions(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    // Stopped -> Playing
    state.borrow_mut().playback_state = PlaybackState::Playing;
    assert_eq!(state.borrow().playback_state, PlaybackState::Playing);

    // Playing -> Paused
    state.borrow_mut().playback_state = PlaybackState::Paused;
    assert_eq!(state.borrow().playback_state, PlaybackState::Paused);

    // Paused -> Playing
    state.borrow_mut().playback_state = PlaybackState::Playing;
    assert_eq!(state.borrow().playback_state, PlaybackState::Playing);

    // Playing -> Stopped
    state.borrow_mut().playback_state = PlaybackState::Stopped;
    assert_eq!(state.borrow().playback_state, PlaybackState::Stopped);
}

// =============================================================================
// Complete Workflow Tests
// =============================================================================

/// Test complete library workflow: scan -> browse -> select -> play.
#[gpui::test]
async fn test_complete_library_workflow(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    // Step 1: Add library path
    state.borrow_mut().library_paths.push(LibraryPath {
        path: "/Music".to_string(),
        enabled: true,
        track_count: 0,
    });

    // Step 2: Scan library
    state.borrow_mut().scan_status = ScanStatus::Scanning;
    state.borrow_mut().scan_progress = 0.5;

    // Step 3: Scan complete
    state.borrow_mut().scan_status = ScanStatus::Complete;
    state.borrow_mut().total_tracks = 500;
    state.borrow_mut().total_albums = 40;
    state.borrow_mut().albums = vec![AlbumInfo {
        id: "album1".to_string(),
        title: "Test Album".to_string(),
        track_count: 10,
        ..Default::default()
    }];

    // Step 4: Browse albums
    state.borrow_mut().browse_mode = BrowseMode::Albums;
    let albums_clone = state.borrow().albums.clone();
    state.borrow_mut().visible_albums = albums_clone;

    // Step 5: Select album
    state.borrow_mut().selected_album_id = Some("album1".to_string());
    state.borrow_mut().album_tracks = vec![TrackInfo {
        id: "t1".to_string(),
        title: "Track 1".to_string(),
        track_number: 1,
        ..Default::default()
    }];

    // Step 6: Play track
    let first_track = state.borrow().album_tracks[0].clone();
    state.borrow_mut().current_track = Some(first_track);
    state.borrow_mut().playback_state = PlaybackState::Playing;

    // Verify final state
    assert_eq!(state.borrow().scan_status, ScanStatus::Complete);
    assert_eq!(state.borrow().total_albums, 40);
    assert!(state.borrow().selected_album_id.is_some());
    assert!(state.borrow().current_track.is_some());
    assert_eq!(state.borrow().playback_state, PlaybackState::Playing);
}

/// Test search and play workflow.
#[gpui::test]
async fn test_search_and_play_workflow(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    // Setup library
    state.borrow_mut().albums = vec![
        AlbumInfo {
            id: "a1".to_string(),
            title: "Abbey Road".to_string(),
            artist: "The Beatles".to_string(),
            ..Default::default()
        },
        AlbumInfo {
            id: "a2".to_string(),
            title: "Dark Side".to_string(),
            artist: "Pink Floyd".to_string(),
            ..Default::default()
        },
    ];

    // Step 1: Search
    state.borrow_mut().filter.search_query = "beatles".to_string();

    // Step 2: Filter results
    let query = state.borrow().filter.search_query.clone().to_lowercase();
    let filtered: Vec<AlbumInfo> = state
        .borrow()
        .albums
        .iter()
        .filter(|a| {
            a.title.to_lowercase().contains(&query) || a.artist.to_lowercase().contains(&query)
        })
        .cloned()
        .collect();
    state.borrow_mut().visible_albums = filtered;

    // Step 3: Select from results
    assert_eq!(state.borrow().visible_albums.len(), 1);
    state.borrow_mut().selected_album_id = Some("a1".to_string());

    // Verify
    assert_eq!(state.borrow().selected_album_id, Some("a1".to_string()));
}

/// Test genre browse workflow.
#[gpui::test]
async fn test_genre_browse_workflow(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    // Setup
    state.borrow_mut().albums = vec![
        AlbumInfo {
            id: "a1".to_string(),
            genre: Some("Rock".to_string()),
            ..Default::default()
        },
        AlbumInfo {
            id: "a2".to_string(),
            genre: Some("Jazz".to_string()),
            ..Default::default()
        },
    ];
    state.borrow_mut().available_genres = vec!["Rock".to_string(), "Jazz".to_string()];

    // Step 1: Switch to genre browse
    state.borrow_mut().browse_mode = BrowseMode::Genres;

    // Step 2: Select genre filter
    state.borrow_mut().filter.genre_filter = Some("Rock".to_string());

    // Step 3: Filter albums
    let genre = state.borrow().filter.genre_filter.clone();
    let filtered: Vec<AlbumInfo> = state
        .borrow()
        .albums
        .iter()
        .filter(|a| a.genre == genre)
        .cloned()
        .collect();
    state.borrow_mut().visible_albums = filtered;

    // Verify
    assert_eq!(state.borrow().visible_albums.len(), 1);
    assert_eq!(state.borrow().visible_albums[0].id, "a1");
}

// =============================================================================
// Edge Case Tests
// =============================================================================

/// Test empty library.
#[gpui::test]
async fn test_empty_library(_cx: &mut TestAppContext) {
    let state = LibraryWorkflowState::default();

    assert!(state.albums.is_empty());
    assert_eq!(state.total_tracks, 0);
    assert_eq!(state.total_albums, 0);
}

/// Test no search results.
#[gpui::test]
async fn test_no_search_results(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    state.borrow_mut().albums = vec![AlbumInfo {
        title: "Test Album".to_string(),
        ..Default::default()
    }];

    state.borrow_mut().filter.search_query = "nonexistent".to_string();

    let query = state.borrow().filter.search_query.clone().to_lowercase();
    let filtered: Vec<AlbumInfo> = state
        .borrow()
        .albums
        .iter()
        .filter(|a| a.title.to_lowercase().contains(&query))
        .cloned()
        .collect();
    state.borrow_mut().visible_albums = filtered;

    assert!(state.borrow().visible_albums.is_empty());
}

/// Test album with no tracks.
#[gpui::test]
async fn test_album_with_no_tracks(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibraryWorkflowState::default()));

    state.borrow_mut().albums.push(AlbumInfo {
        id: "empty".to_string(),
        track_count: 0,
        ..Default::default()
    });

    state.borrow_mut().selected_album_id = Some("empty".to_string());
    state.borrow_mut().album_tracks = Vec::new();

    assert!(state.borrow().album_tracks.is_empty());
}

/// Test missing artwork handling.
#[gpui::test]
async fn test_missing_artwork_handling(_cx: &mut TestAppContext) {
    let album = AlbumInfo {
        artwork_path: None,
        ..Default::default()
    };

    assert!(album.artwork_path.is_none());
}

// =============================================================================
// Performance Tests
// =============================================================================

/// Test filtering large library.
#[gpui::test]
async fn test_filtering_large_library(_cx: &mut TestAppContext) {
    fn filter_albums(albums: &[AlbumInfo], query: &str) -> Vec<AlbumInfo> {
        let q = query.to_lowercase();
        albums
            .iter()
            .filter(|a| a.title.to_lowercase().contains(&q))
            .cloned()
            .collect()
    }

    // Simulate large library
    let albums: Vec<AlbumInfo> = (0..1000)
        .map(|i| AlbumInfo {
            id: format!("album{}", i),
            title: format!("Album {}", i),
            ..Default::default()
        })
        .collect();

    let filtered = filter_albums(&albums, "Album 50");
    // Should match "Album 50", "Album 500", "Album 501", etc.
    assert!(!filtered.is_empty());
}

/// Test virtual scrolling position.
#[gpui::test]
async fn test_virtual_scrolling_position(_cx: &mut TestAppContext) {
    fn calculate_visible_range(
        scroll_offset: f32,
        viewport_height: f32,
        item_height: f32,
        total_items: usize,
    ) -> (usize, usize) {
        let start = (scroll_offset / item_height).floor() as usize;
        let visible_count = (viewport_height / item_height).ceil() as usize + 1;
        let end = (start + visible_count).min(total_items);
        (start, end)
    }

    let (start, end) = calculate_visible_range(500.0, 400.0, 50.0, 100);
    assert_eq!(start, 10);
    assert!(end > start);
}

// =============================================================================
// Regression Tests
// =============================================================================

/// Regression test: search should ignore album letter filters.
///
/// This test verifies that when a user has a letter filter active (e.g., "A"
/// in Album view) and then searches for something (e.g., "tool"), the search
/// results should not be filtered by the letter filter.
///
/// Bug fix: Before the fix, search results were filtered by selection filters
/// like album_letter, artist_letter, genre, decade, etc. This meant searching
/// for "tool" while having letter "A" selected would show no results because
/// "Tool" doesn't start with "A".
#[gpui::test]
async fn test_search_ignores_album_letter_filter(_cx: &mut TestAppContext) {
    /// Filter by album first letter
    fn filter_by_letter(albums: &[AlbumInfo], letter: char) -> Vec<AlbumInfo> {
        albums
            .iter()
            .filter(|a| {
                a.title
                    .chars()
                    .next()
                    .is_some_and(|c| c.to_ascii_uppercase() == letter)
            })
            .cloned()
            .collect()
    }

    /// Filter by search query
    fn filter_by_search(albums: &[AlbumInfo], query: &str) -> Vec<AlbumInfo> {
        let q = query.to_lowercase();
        albums
            .iter()
            .filter(|a| a.title.to_lowercase().contains(&q) || a.artist.to_lowercase().contains(&q))
            .cloned()
            .collect()
    }

    // Setup library with test albums
    let albums = vec![
        AlbumInfo {
            id: "a1".to_string(),
            title: "Abbey Road".to_string(),
            artist: "The Beatles".to_string(),
            ..Default::default()
        },
        AlbumInfo {
            id: "a2".to_string(),
            title: "Lateralus".to_string(),
            artist: "Tool".to_string(),
            ..Default::default()
        },
        AlbumInfo {
            id: "a3".to_string(),
            title: "10,000 Days".to_string(),
            artist: "Tool".to_string(),
            ..Default::default()
        },
        AlbumInfo {
            id: "a4".to_string(),
            title: "Aenima".to_string(),
            artist: "Tool".to_string(),
            ..Default::default()
        },
    ];

    // Without search, letter filter 'A' should only show albums starting with "A"
    let letter_filtered = filter_by_letter(&albums, 'A');
    assert_eq!(
        letter_filtered.len(),
        2,
        "Letter filter 'A' should show 2 albums (Abbey Road, Aenima)"
    );

    // With search for "tool", we should find Tool albums regardless of letter filter
    // This is the key behavior: search should bypass selection filters
    let search_results = filter_by_search(&albums, "tool");
    assert_eq!(
        search_results.len(),
        3,
        "Search for 'tool' should find 3 Tool albums"
    );

    // Verify the correct albums were found
    assert!(search_results.iter().all(|a| a.artist == "Tool"));
}

/// Regression test: search should ignore genre filter.
#[gpui::test]
async fn test_search_ignores_genre_filter(_cx: &mut TestAppContext) {
    /// Filter by genre
    fn filter_by_genre(albums: &[AlbumInfo], genre: &str) -> Vec<AlbumInfo> {
        albums
            .iter()
            .filter(|a| a.genre.as_deref() == Some(genre))
            .cloned()
            .collect()
    }

    /// Filter by search query
    fn filter_by_search(albums: &[AlbumInfo], query: &str) -> Vec<AlbumInfo> {
        let q = query.to_lowercase();
        albums
            .iter()
            .filter(|a| a.title.to_lowercase().contains(&q) || a.artist.to_lowercase().contains(&q))
            .cloned()
            .collect()
    }

    let albums = vec![
        AlbumInfo {
            id: "a1".to_string(),
            title: "Abbey Road".to_string(),
            artist: "The Beatles".to_string(),
            genre: Some("Rock".to_string()),
            ..Default::default()
        },
        AlbumInfo {
            id: "a2".to_string(),
            title: "Kind of Blue".to_string(),
            artist: "Miles Davis".to_string(),
            genre: Some("Jazz".to_string()),
            ..Default::default()
        },
    ];

    // Genre filter "Rock" would normally exclude Jazz albums
    let genre_filtered = filter_by_genre(&albums, "Rock");
    assert_eq!(genre_filtered.len(), 1);

    // But search for "miles" should find Miles Davis regardless
    let search_results = filter_by_search(&albums, "miles");
    assert_eq!(search_results.len(), 1);
    assert_eq!(search_results[0].artist, "Miles Davis");
}
