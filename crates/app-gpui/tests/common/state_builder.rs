//! Test state builder for creating App-like state for testing.
//!
//! Since the full App struct has 300+ fields and is tightly coupled to GPUI,
//! we create lightweight test state structs that mirror the key behaviors
//! we want to test.

/// Lightweight test state for library filtering tests
#[derive(Debug, Clone)]
pub struct TestLibraryState {
    pub search_query: String,
    pub library_sort_order: TestLibrarySortOrder,
    pub channel_filter: TestChannelFilter,
    pub albums: Vec<TestAlbum>,

    // Selection filters
    pub selected_genre: Option<String>,
    pub selected_decade: Option<(i32, i32)>,
    pub selected_year: Option<i32>,
    pub selected_artist_letter: Option<char>,
    pub selected_artist: Option<String>,

    // Pagination
    pub current_page: usize,
    pub items_per_page: usize,
}

impl Default for TestLibraryState {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            library_sort_order: TestLibrarySortOrder::Title,
            channel_filter: TestChannelFilter::All,
            albums: Vec::new(),
            selected_genre: None,
            selected_decade: None,
            selected_year: None,
            selected_artist_letter: None,
            selected_artist: None,
            current_page: 0,
            items_per_page: 20,
        }
    }
}

impl TestLibraryState {
    pub fn with_albums(mut self, albums: Vec<TestAlbum>) -> Self {
        self.albums = albums;
        self
    }

    pub fn with_search(mut self, query: &str) -> Self {
        self.search_query = query.to_string();
        self
    }

    /// Get filtered albums based on search query and filters.
    /// Mimics App::filtered_albums() behavior.
    pub fn filtered_albums(&self) -> Vec<&TestAlbum> {
        let mut result: Vec<&TestAlbum> = self.albums.iter().collect();

        // Apply channel filter
        result.retain(|album| match self.channel_filter {
            TestChannelFilter::All => true,
            TestChannelFilter::Stereo => album.channels == 2,
            TestChannelFilter::Multichannel => album.channels > 2,
            TestChannelFilter::Mono => album.channels == 1,
        });

        // Apply search query (trim whitespace to match typical app behavior)
        let trimmed_query = self.search_query.trim();
        if !trimmed_query.is_empty() {
            let query = trimmed_query.to_lowercase();
            result.retain(|album| {
                album.title.to_lowercase().contains(&query)
                    || album.artist.to_lowercase().contains(&query)
            });
            // When search is active, skip selection filters (matching real behavior)
            return result;
        }

        // Apply selection filters
        if let Some(ref genre) = self.selected_genre {
            result.retain(|album| {
                album
                    .genre
                    .as_ref()
                    .is_some_and(|g| g.eq_ignore_ascii_case(genre))
            });
        }

        if let Some((decade_start, decade_end)) = self.selected_decade
            && self.selected_year.is_none()
        {
            result.retain(|album| {
                album
                    .year
                    .is_some_and(|y| y >= decade_start && y <= decade_end)
            });
        }

        if let Some(year) = self.selected_year {
            result.retain(|album| album.year == Some(year));
        }

        if let Some(letter) = self.selected_artist_letter
            && self.selected_artist.is_none()
        {
            result.retain(|album| {
                album.artist.chars().next().is_some_and(|c| {
                    let first = c.to_ascii_uppercase();
                    if letter == '#' {
                        !first.is_ascii_alphabetic()
                    } else {
                        first == letter
                    }
                })
            });
        }

        if let Some(ref artist) = self.selected_artist {
            result.retain(|album| album.artist.eq_ignore_ascii_case(artist));
        }

        result
    }

    /// Get total number of pages based on filtered albums.
    /// Returns minimum 1 page even for empty library.
    pub fn total_pages(&self) -> usize {
        let filtered_count = self.filtered_albums().len();
        if self.items_per_page == 0 || filtered_count == 0 {
            return 1;
        }
        filtered_count.div_ceil(self.items_per_page)
    }

    /// Recalculate pagination bounds
    pub fn recalculate_pagination(&mut self) {
        let max_page = self.total_pages().saturating_sub(1);
        if self.current_page > max_page {
            self.current_page = max_page;
        }
    }

    /// Clear search query
    pub fn clear_search(&mut self) {
        self.search_query.clear();
    }

    /// Set channel filter
    pub fn set_channel_filter(&mut self, filter: TestChannelFilter) {
        self.channel_filter = filter;
    }
}

/// Lightweight test state for playback tests
#[derive(Debug, Clone)]
pub struct TestPlaybackState {
    pub is_playing: bool,
    pub volume: f32,
    pub muted: bool,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub current_queue_index: Option<usize>,
    pub queue: Vec<TestQueueItem>,
}

impl Default for TestPlaybackState {
    fn default() -> Self {
        Self {
            is_playing: false,
            volume: 1.0,
            muted: false,
            position_secs: 0.0,
            duration_secs: 0.0,
            current_queue_index: None,
            queue: Vec::new(),
        }
    }
}

impl TestPlaybackState {
    pub fn with_volume(mut self, volume: f32) -> Self {
        self.volume = volume.clamp(0.0, 1.0);
        self
    }

    pub fn with_queue(mut self, queue: Vec<TestQueueItem>) -> Self {
        self.queue = queue;
        if !self.queue.is_empty() {
            self.current_queue_index = Some(0);
        }
        self
    }

    /// Set volume, clamped to [0.0, 1.0]
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Advance to next track, preserving volume
    /// Returns Some(track_path) if there's a next track
    pub fn next_track(&mut self) -> Option<String> {
        let current_idx = self.current_queue_index?;
        let item = self.queue.get_mut(current_idx)?;

        // Try next track in current album
        if item.current_track_index + 1 < item.tracks.len() {
            item.current_track_index += 1;
            self.position_secs = 0.0;
            return Some(item.tracks[item.current_track_index].path.clone());
        }

        // Try next album
        if current_idx + 1 < self.queue.len() {
            self.current_queue_index = Some(current_idx + 1);
            self.queue[current_idx + 1].current_track_index = 0;
            self.position_secs = 0.0;
            return Some(self.queue[current_idx + 1].tracks[0].path.clone());
        }

        // No more tracks
        self.is_playing = false;
        None
    }

    /// Seek to position in current track
    pub fn seek_to(&mut self, position: f64) -> Result<(), &'static str> {
        if self.current_queue_index.is_none() {
            return Err("No track loaded");
        }
        self.position_secs = position.clamp(0.0, self.duration_secs);
        Ok(())
    }
}

/// Lightweight test state for input mode tests
#[derive(Debug, Clone)]
pub struct TestInputState {
    pub input_mode: TestInputMode,
    pub search_query: String,
    /// Tracks which global actions were triggered (for negative testing)
    pub triggered_actions: Vec<TestAction>,
}

impl Default for TestInputState {
    fn default() -> Self {
        Self {
            input_mode: TestInputMode::Normal,
            search_query: String::new(),
            triggered_actions: Vec::new(),
        }
    }
}

impl TestInputState {
    /// Enter a specific input mode
    pub fn enter_input_mode(&mut self, mode: TestInputMode) {
        self.input_mode = mode;
    }

    /// Exit to normal mode
    pub fn exit_input_mode(&mut self) {
        self.input_mode = TestInputMode::Normal;
    }

    /// Process a key press based on current input mode
    /// Returns true if the key was consumed by the current mode
    pub fn process_key(&mut self, key: char) -> bool {
        match self.input_mode {
            TestInputMode::Search => {
                // In search mode, keys go to search query
                if key == '\x1b' {
                    // Escape
                    self.search_query.clear();
                    self.input_mode = TestInputMode::Normal;
                } else if key == '\x08' {
                    // Backspace
                    self.search_query.pop();
                } else if !key.is_control() {
                    self.search_query.push(key);
                }
                true // Key consumed
            }
            TestInputMode::Normal => {
                // In normal mode, check for global keybindings
                match key {
                    '0' => {
                        self.triggered_actions.push(TestAction::SetFilterAll);
                        true
                    }
                    '1'..='5' => {
                        self.triggered_actions
                            .push(TestAction::SetFilterRating(key.to_digit(10).unwrap() as u8));
                        true
                    }
                    ' ' => {
                        self.triggered_actions.push(TestAction::PlayPause);
                        true
                    }
                    '+' | '=' => {
                        self.triggered_actions.push(TestAction::VolumeUp);
                        true
                    }
                    '-' | '_' => {
                        self.triggered_actions.push(TestAction::VolumeDown);
                        true
                    }
                    '/' => {
                        self.input_mode = TestInputMode::Search;
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }
}

// =============================================================================
// Test Types (mirrors real types)
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestInputMode {
    Normal,
    Search,
    AddDirectory,
    SavePlugins,
    LoadPlugins,
    EditingParam,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestLibrarySortOrder {
    Title,
    Artist,
    Year,
    Genre,
    Rating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestChannelFilter {
    All,
    Stereo,
    Multichannel,
    Mono,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TestAction {
    PlayPause,
    NextTrack,
    PrevTrack,
    VolumeUp,
    VolumeDown,
    SetVolume(f32),
    SeekTo(f64),
    ToggleSearch,
    ClearSearch,
    TypeInSearch(String),
    SetFilterAll,
    SetFilterRating(u8),
    SetSortOrder(TestLibrarySortOrder),
}

#[derive(Debug, Clone)]
pub struct TestAlbum {
    pub title: String,
    pub artist: String,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub channels: usize,
    pub tracks: Vec<TestTrack>,
}

impl TestAlbum {
    pub fn new(title: &str, artist: &str) -> Self {
        Self {
            title: title.to_string(),
            artist: artist.to_string(),
            year: None,
            genre: None,
            channels: 2,
            tracks: vec![TestTrack::new(&format!("{}.flac", title))],
        }
    }

    pub fn with_year(mut self, year: i32) -> Self {
        self.year = Some(year);
        self
    }

    pub fn with_genre(mut self, genre: &str) -> Self {
        self.genre = Some(genre.to_string());
        self
    }

    pub fn with_channels(mut self, channels: usize) -> Self {
        self.channels = channels;
        self
    }

    pub fn with_tracks(mut self, tracks: Vec<TestTrack>) -> Self {
        self.tracks = tracks;
        self
    }
}

#[derive(Debug, Clone)]
pub struct TestTrack {
    pub path: String,
    pub title: String,
    pub duration_secs: f64,
}

impl TestTrack {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            title: path.to_string(),
            duration_secs: 180.0,
        }
    }

    pub fn with_duration(mut self, duration: f64) -> Self {
        self.duration_secs = duration;
        self
    }
}

#[derive(Debug, Clone)]
pub struct TestQueueItem {
    pub album: TestAlbum,
    pub tracks: Vec<TestTrack>,
    pub current_track_index: usize,
}

impl TestQueueItem {
    pub fn from_album(album: TestAlbum) -> Self {
        let tracks = album.tracks.clone();
        Self {
            album,
            tracks,
            current_track_index: 0,
        }
    }
}

// =============================================================================
// Builder helpers
// =============================================================================

/// Create a set of test albums for property testing
pub fn create_test_albums(count: usize) -> Vec<TestAlbum> {
    (0..count)
        .map(|i| {
            TestAlbum::new(&format!("Album {}", i), &format!("Artist {}", i % 10))
                .with_year(2000 + (i as i32 % 25))
                .with_genre(match i % 5 {
                    0 => "Rock",
                    1 => "Jazz",
                    2 => "Classical",
                    3 => "Electronic",
                    _ => "Pop",
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_state_default() {
        let state = TestLibraryState::default();
        assert!(state.search_query.is_empty());
        assert_eq!(state.filtered_albums().len(), 0);
    }

    #[test]
    fn test_library_state_with_albums() {
        let albums = vec![
            TestAlbum::new("Album A", "Artist 1"),
            TestAlbum::new("Album B", "Artist 2"),
        ];
        let state = TestLibraryState::default().with_albums(albums);
        assert_eq!(state.filtered_albums().len(), 2);
    }

    #[test]
    fn test_search_filters_albums() {
        let albums = vec![
            TestAlbum::new("Tool Album", "Tool"),
            TestAlbum::new("Other Album", "Other Artist"),
        ];
        let state = TestLibraryState::default()
            .with_albums(albums)
            .with_search("tool");
        let filtered = state.filtered_albums();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "Tool Album");
    }

    #[test]
    fn test_playback_volume_preserved() {
        let mut state = TestPlaybackState::default().with_volume(0.42);
        assert_eq!(state.volume, 0.42);

        // Simulate track change
        let tracks = vec![TestTrack::new("track1.flac"), TestTrack::new("track2.flac")];
        let album = TestAlbum::new("Album", "Artist").with_tracks(tracks);
        state.queue = vec![TestQueueItem::from_album(album)];
        state.current_queue_index = Some(0);

        state.next_track();

        // Volume should be preserved
        assert_eq!(state.volume, 0.42);
    }

    #[test]
    fn test_input_mode_isolation() {
        let mut state = TestInputState::default();
        state.enter_input_mode(TestInputMode::Search);

        // These keys should NOT trigger global actions in search mode
        state.process_key('5');
        state.process_key(' ');
        state.process_key('+');

        // No actions should be triggered
        assert!(state.triggered_actions.is_empty());

        // Instead, characters should go to search query
        assert_eq!(state.search_query, "5 +");
    }
}
