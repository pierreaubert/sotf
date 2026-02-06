//! Library state management.
//!
//! Contains the library-specific state: albums, filtering, sorting.
//! This can be used as an independent Entity in GPUI for better separation.

use std::path::PathBuf;

use crate::app::constants;
use crate::app::manager::{Manager, ManagerError};
use sotf_audio_player::{Album, MusicLibrary};

/// Library management events
#[derive(Debug, Clone)]
pub enum LibraryEvent {
    SetSortOrder(LibrarySortOrder),
    SetFilter(ChannelFilter),
    SetSearchQuery(String),
    ClearSearch,
    CycleFilter,
    NextPage,
    PrevPage,
    SelectNext,
    SelectPrev,
    SelectGridRight(usize),
    SelectGridLeft(usize),
    SelectGridDown(usize),
    SelectGridUp(usize),
    PageDown(usize),
    PageUp(usize),
    Scan,
}

/// Library queries
#[derive(Debug, Clone)]
pub enum LibraryQuery {
    ItemCount,
    SelectedAlbum,
    FilteredAlbums,
}

/// Library query responses
#[derive(Debug)]
pub enum LibraryResponse {
    Count(usize),
    Album(Option<Album>), // Return owned clone for isolation? Or reference?
    // The trait response is owned. Album is expensive to clone.
    // Ideally we return Arc<Album> or similar.
    // For now, let's use what we have.
    None,
}

/// Library sort order options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibrarySortOrder {
    #[default]
    Year,
    Genre,
    Artist,
    Album,
    Tracks,
    Composer,
}

impl LibrarySortOrder {
    /// Convert to sotf_audio_player LibrarySortOrder
    pub fn to_library_sort_order(self) -> sotf_audio_player::LibrarySortOrder {
        match self {
            Self::Year => sotf_audio_player::LibrarySortOrder::Year,
            Self::Genre => sotf_audio_player::LibrarySortOrder::Genre,
            Self::Artist => sotf_audio_player::LibrarySortOrder::Artist,
            Self::Album => sotf_audio_player::LibrarySortOrder::Album,
            Self::Tracks => sotf_audio_player::LibrarySortOrder::Tracks,
            Self::Composer => sotf_audio_player::LibrarySortOrder::Composer,
        }
    }
}

/// Channel filter options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChannelFilter {
    #[default]
    All, // Show all albums
    Mono,          // Only 1-channel albums
    Stereo,        // Only 2-channel albums
    Surround,      // 5.0/5.1 albums (5-6 channels)
    Surround71,    // 7.1 albums (8 channels)
    SurroundPlus,  // More than 8 channels
    Mixed,         // Only albums with mixed channel counts
    Specific(u32), // Only albums with specific channel count
}

impl ChannelFilter {
    /// Convert to sotf_audio_player ChannelFilter
    pub fn to_library_channel_filter(self) -> sotf_audio_player::ChannelFilter {
        match self {
            Self::All => sotf_audio_player::ChannelFilter::All,
            Self::Mono => sotf_audio_player::ChannelFilter::Mono,
            Self::Stereo => sotf_audio_player::ChannelFilter::Stereo,
            Self::Surround => sotf_audio_player::ChannelFilter::Surround,
            Self::Surround71 => sotf_audio_player::ChannelFilter::Surround71,
            Self::SurroundPlus => sotf_audio_player::ChannelFilter::SurroundPlus,
            Self::Mixed => sotf_audio_player::ChannelFilter::Mixed,
            Self::Specific(n) => sotf_audio_player::ChannelFilter::Specific(n),
        }
    }
}

/// Library state - can be used as a GPUI Entity
#[derive(Debug)]
pub struct LibraryState {
    /// The underlying music library (albums, directories)
    pub library: MusicLibrary,

    /// Current sort order
    pub sort_order: LibrarySortOrder,

    /// Current channel filter
    pub filter: ChannelFilter,

    /// Current search query
    pub search_query: String,

    /// Selected album index
    pub selected_index: usize,

    /// Current page (0-indexed)
    pub current_page: usize,

    /// Items per page
    pub items_per_page: usize,

    /// Number of columns in grid layout
    pub library_columns: usize,

    /// Filter selections for each sort mode
    pub selected_genre: Option<String>,
    pub selected_decade: Option<(i32, i32)>, // (start, end) e.g., (2020, 2029)
    pub selected_year: Option<i32>,
    pub selected_artist_letter: Option<char>, // First letter filter
    pub selected_artist: Option<String>,
    pub selected_composer_letter: Option<char>, // First letter filter
    pub selected_composer: Option<String>,
    pub selected_album_letter: Option<char>,
    pub selected_track_range: Option<(usize, usize)>, // (min, max) track count

    /// Scan state
    pub scan_in_progress: bool,
    pub scan_progress_tracks: usize,
    pub scan_progress_albums: usize,

    /// Cache for filtered and sorted albums
    pub cached_albums: Vec<Album>,
    pub cache_dirty: bool,
}

impl Default for LibraryState {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryState {
    /// Create new library state with default database
    pub fn new() -> Self {
        let library = MusicLibrary::with_database().unwrap_or_else(|e| {
            log::warn!(
                "Failed to initialize database, using in-memory library: {}",
                e
            );
            MusicLibrary::new()
        });
        Self::with_library(library)
    }

    /// Create library state with provided library
    pub fn with_library(library: MusicLibrary) -> Self {
        Self {
            library,
            sort_order: LibrarySortOrder::default(),
            filter: ChannelFilter::default(),
            search_query: String::new(),
            selected_index: 0,
            current_page: 0,
            items_per_page: constants::library::DEFAULT_ITEMS_PER_PAGE,
            library_columns: 4, // Default grid columns
            selected_genre: None,
            selected_decade: None,
            selected_year: None,
            selected_artist_letter: None,
            selected_artist: None,
            selected_composer_letter: None,
            selected_composer: None,
            selected_album_letter: None,
            selected_track_range: None,
            scan_in_progress: false,
            scan_progress_tracks: 0,
            scan_progress_albums: 0,
            cached_albums: Vec::new(),
            cache_dirty: true,
        }
    }

    /// Create library state for testing (in-memory, no database)
    pub fn new_for_test() -> Self {
        Self::with_library(MusicLibrary::new())
    }

    // =========================================================================
    // Filtering and Sorting
    // =========================================================================

    /// Mark the cache as dirty so it will be recomputed next time it's needed
    pub fn invalidate_cache(&mut self) {
        self.cache_dirty = true;
    }

    /// Ensure the cached filtered albums are up to date
    pub fn ensure_cache_valid(&mut self) {
        if self.cache_dirty {
            self.recompute_cache();
        }
    }

    /// Recompute the filtered and sorted albums cache
    fn recompute_cache(&mut self) {
        let mut albums: Vec<&Album> = if self.search_query.is_empty() {
            self.library.albums.iter().collect()
        } else {
            self.library.search_albums(&self.search_query)
        };

        // Apply channel filter
        albums.retain(|album| self.matches_filter(album));

        // Apply sort
        self.sort_albums(&mut albums);

        // Update the cache by cloning the references into owned items
        // Note: This is still a clone, but it only happens when filters change,
        // and we avoid it on every render.
        self.cached_albums = albums.into_iter().cloned().collect();
        self.cache_dirty = false;
    }

    /// Get filtered and sorted albums (uses cache)
    pub fn filtered_albums(&self) -> Vec<&Album> {
        // If dirty, we can't recompute here because &self is immutable.
        // Callers should call ensure_cache_valid() before this if they have &mut self.
        // If they only have &self, they get the last cached version.
        self.cached_albums.iter().collect()
    }

    /// Check if album matches current filter
    fn matches_filter(&self, album: &Album) -> bool {
        match self.filter {
            ChannelFilter::All => true,
            ChannelFilter::Mono => album.uniform_channel_count() == Some(1),
            ChannelFilter::Stereo => album.uniform_channel_count() == Some(2),
            ChannelFilter::Surround => {
                matches!(album.uniform_channel_count(), Some(5) | Some(6))
            }
            ChannelFilter::Surround71 => album.uniform_channel_count() == Some(8),
            ChannelFilter::SurroundPlus => album.uniform_channel_count().is_some_and(|ch| ch > 8),
            ChannelFilter::Mixed => album.uniform_channel_count().is_none(),
            ChannelFilter::Specific(n) => album.uniform_channel_count() == Some(n),
        }
    }

    /// Sort albums according to current sort order
    fn sort_albums(&self, albums: &mut Vec<&Album>) {
        match self.sort_order {
            LibrarySortOrder::Year => {
                albums.sort_by(|a, b| {
                    b.year
                        .cmp(&a.year)
                        .then_with(|| a.artist().cmp(&b.artist()))
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
            LibrarySortOrder::Genre => {
                albums.sort_by(|a, b| {
                    let genre_a = a
                        .tracks
                        .first()
                        .and_then(|t| t.genre.as_ref())
                        .map(|s| s.to_lowercase());
                    let genre_b = b
                        .tracks
                        .first()
                        .and_then(|t| t.genre.as_ref())
                        .map(|s| s.to_lowercase());
                    genre_a
                        .cmp(&genre_b)
                        .then_with(|| a.artist().cmp(&b.artist()))
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
            LibrarySortOrder::Artist => {
                albums.sort_by(|a, b| {
                    a.artist()
                        .cmp(&b.artist())
                        .then_with(|| a.year.cmp(&b.year).reverse())
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
            LibrarySortOrder::Album => {
                albums.sort_by(|a, b| a.title.cmp(&b.title));
            }
            LibrarySortOrder::Tracks => {
                albums.sort_by(|a, b| {
                    b.tracks
                        .len()
                        .cmp(&a.tracks.len())
                        .then_with(|| a.artist().cmp(&b.artist()))
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
            LibrarySortOrder::Composer => {
                albums.sort_by(|a, b| {
                    let composer_a = a
                        .tracks
                        .first()
                        .and_then(|t| t.composer.as_ref())
                        .map(|s| s.to_lowercase());
                    let composer_b = b
                        .tracks
                        .first()
                        .and_then(|t| t.composer.as_ref())
                        .map(|s| s.to_lowercase());
                    composer_a
                        .cmp(&composer_b)
                        .then_with(|| a.artist().cmp(&b.artist()))
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
        }
    }

    /// Set sort order and reset selection
    pub fn set_sort_order(&mut self, order: LibrarySortOrder) {
        self.sort_order = order;
        self.selected_index = 0;
        self.invalidate_cache();
    }

    /// Set channel filter and reset selection
    pub fn set_filter(&mut self, filter: ChannelFilter) {
        self.filter = filter;
        self.selected_index = 0;
        self.invalidate_cache();
    }

    /// Cycle to next channel filter
    pub fn cycle_filter(&mut self) {
        self.filter = match self.filter {
            ChannelFilter::All => ChannelFilter::Mono,
            ChannelFilter::Mono => ChannelFilter::Stereo,
            ChannelFilter::Stereo => ChannelFilter::Surround,
            ChannelFilter::Surround => ChannelFilter::Surround71,
            ChannelFilter::Surround71 => ChannelFilter::SurroundPlus,
            ChannelFilter::SurroundPlus => ChannelFilter::Mixed,
            ChannelFilter::Mixed | ChannelFilter::Specific(_) => ChannelFilter::All,
        };
        self.selected_index = 0;
        self.invalidate_cache();
    }

    /// Set search query and reset selection
    pub fn set_search_query(&mut self, query: String) {
        self.search_query = query;
        self.selected_index = 0;
        self.invalidate_cache();
    }

    /// Clear search query
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.selected_index = 0;
        self.invalidate_cache();
    }

    // =========================================================================
    // Pagination
    // =========================================================================

    /// Get paginated albums
    pub fn get_paginated_albums(&self) -> Vec<&Album> {
        let all_albums = self.filtered_albums();
        let start = self.current_page * self.items_per_page;
        let end = (start + self.items_per_page).min(all_albums.len());
        if start >= all_albums.len() {
            return Vec::new();
        }
        all_albums[start..end].to_vec()
    }

    /// Get total pages
    pub fn total_pages(&self) -> usize {
        let total_items = self.filtered_albums().len();
        if total_items == 0 {
            1
        } else {
            (total_items + self.items_per_page - 1) / self.items_per_page
        }
    }

    /// Go to next page
    pub fn next_page(&mut self) {
        if self.current_page + 1 < self.total_pages() {
            self.current_page += 1;
            self.selected_index = 0;
        }
    }

    /// Go to previous page
    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
            self.selected_index = 0;
        }
    }

    /// Reset to first page
    pub fn reset_page(&mut self) {
        self.current_page = 0;
    }

    // =========================================================================
    // Navigation
    // =========================================================================

    /// Total item count
    pub fn item_count(&self) -> usize {
        self.filtered_albums().len()
    }

    /// Select next item
    pub fn select_next(&mut self) {
        let count = self.filtered_albums().len();
        if count > 0 {
            self.selected_index = (self.selected_index + 1) % count;
        }
    }

    /// Select previous item
    pub fn select_prev(&mut self) {
        let count = self.filtered_albums().len();
        if count > 0 {
            self.selected_index = if self.selected_index == 0 {
                count - 1
            } else {
                self.selected_index - 1
            };
        }
    }

    /// Page down navigation
    pub fn page_down(&mut self, page_size: usize) {
        let count = self.filtered_albums().len();
        if count > 0 {
            self.selected_index = (self.selected_index + page_size).min(count - 1);
        }
    }

    /// Page up navigation
    pub fn page_up(&mut self, page_size: usize) {
        self.selected_index = self.selected_index.saturating_sub(page_size);
    }

    /// Grid-specific navigation (left)
    pub fn select_grid_left(&mut self, grid_columns: usize) {
        if self.selected_index % grid_columns > 0 {
            self.selected_index -= 1;
        }
    }

    /// Grid-specific navigation (right)
    pub fn select_grid_right(&mut self, grid_columns: usize) {
        let count = self.filtered_albums().len();
        if count > 0
            && self.selected_index % grid_columns < grid_columns - 1
            && self.selected_index < count - 1
        {
            self.selected_index += 1;
        }
    }

    /// Grid-specific navigation (up)
    pub fn select_grid_up(&mut self, grid_columns: usize) {
        if self.selected_index >= grid_columns {
            self.selected_index -= grid_columns;
        }
    }

    /// Grid-specific navigation (down)
    pub fn select_grid_down(&mut self, grid_columns: usize) {
        let count = self.filtered_albums().len();
        if count > 0 {
            let next = self.selected_index + grid_columns;
            if next < count {
                self.selected_index = next;
            } else if self.selected_index < count - 1 {
                self.selected_index = count - 1;
            }
        }
    }

    // =========================================================================
    // Selection
    // =========================================================================

    /// Get currently selected album
    pub fn selected_album(&self) -> Option<&Album> {
        let albums = self.filtered_albums();
        albums.get(self.selected_index).copied()
    }

    // =========================================================================
    // Directory Management
    // =========================================================================

    /// Add directory to library
    pub fn add_directory(&mut self, path: PathBuf) -> Result<bool, String> {
        self.library.add_directory(path)
    }

    /// Remove directory at index
    pub fn remove_directory(&mut self, index: usize) -> Option<PathBuf> {
        let result = self.library.remove_directory(index);
        if result.is_some() {
            self.invalidate_cache();
        }
        result
    }

    /// Get directory tree items for display
    pub fn get_directory_tree_items(&self) -> Vec<(PathBuf, usize, bool)> {
        let mut items = Vec::new();
        for dir_info in &self.library.directories {
            items.push((dir_info.path.clone(), 0, dir_info.expanded));
            if dir_info.expanded {
                for subdir in &dir_info.subdirectories {
                    items.push((subdir.path.clone(), 1, false));
                }
            }
        }
        items
    }

    // =========================================================================
    // Scanning
    // =========================================================================

    /// Scan library with progress callback
    pub fn scan(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use parking_lot::Mutex;
        use std::sync::Arc;

        self.scan_in_progress = true;
        self.scan_progress_tracks = 0;
        self.scan_progress_albums = 0;

        let progress_tracks = Arc::new(Mutex::new(0usize));
        let progress_albums = Arc::new(Mutex::new(0usize));

        let pt = Arc::clone(&progress_tracks);
        let pa = Arc::clone(&progress_albums);

        let result = self.library.scan_with_progress(move |tracks, albums| {
            *pt.lock() = tracks;
            *pa.lock() = albums;
        });

        self.scan_progress_tracks = *progress_tracks.lock();
        self.scan_progress_albums = *progress_albums.lock();
        self.scan_in_progress = false;

        self.selected_index = 0;
        self.invalidate_cache();

        result
    }

    /// Load library from database
    pub fn load_from_database(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.library.load_from_database()?;
        self.invalidate_cache();
        Ok(())
    }

    /// Clean up database by removing tracks for files that no longer exist
    pub fn clean_database(&mut self) -> Result<usize, Box<dyn std::error::Error>> {
        let removed = self.library.clean_database()?;
        if removed > 0 {
            self.invalidate_cache();
        }
        Ok(removed)
    }
}

impl Manager for LibraryState {
    type State = Self;
    type Event = LibraryEvent;
    type Query = LibraryQuery;
    type Response = LibraryResponse;

    fn handle_event(&mut self, event: Self::Event) -> Result<(), ManagerError> {
        match event {
            LibraryEvent::SetSortOrder(order) => self.set_sort_order(order),
            LibraryEvent::SetFilter(filter) => self.set_filter(filter),
            LibraryEvent::SetSearchQuery(q) => self.set_search_query(q),
            LibraryEvent::ClearSearch => self.clear_search(),
            LibraryEvent::CycleFilter => self.cycle_filter(),
            LibraryEvent::NextPage => self.next_page(),
            LibraryEvent::PrevPage => self.prev_page(),
            LibraryEvent::SelectNext => self.select_next(),
            LibraryEvent::SelectPrev => self.select_prev(),
            LibraryEvent::SelectGridRight(cols) => self.select_grid_right(cols),
            LibraryEvent::SelectGridLeft(cols) => self.select_grid_left(cols),
            LibraryEvent::SelectGridDown(cols) => self.select_grid_down(cols),
            LibraryEvent::SelectGridUp(cols) => self.select_grid_up(cols),
            LibraryEvent::PageDown(size) => self.page_down(size),
            LibraryEvent::PageUp(size) => self.page_up(size),
            LibraryEvent::Scan => self.scan().map_err(|e| ManagerError::from(e.to_string()))?,
        }
        Ok(())
    }

    fn query(&self, query: Self::Query) -> Self::Response {
        match query {
            LibraryQuery::ItemCount => LibraryResponse::Count(self.item_count()),
            LibraryQuery::SelectedAlbum => {
                // Return a clone for now as Response must be owned
                LibraryResponse::Album(self.selected_album().cloned())
            }
            LibraryQuery::FilteredAlbums => LibraryResponse::None, // Not implemented fully yet
        }
    }

    fn state(&self) -> &Self::State {
        self
    }
}
