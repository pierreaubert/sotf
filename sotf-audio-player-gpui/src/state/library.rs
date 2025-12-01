//! Library state management.
//!
//! Contains the library-specific state: albums, filtering, sorting.
//! This can be used as an independent Entity in GPUI for better separation.

use std::path::PathBuf;

use sotf_audio_player::{Album, MusicLibrary};

/// Library sort order options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibrarySortOrder {
    #[default]
    Artist,
    Album,
    Title,
    Year,
}

/// Channel filter options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChannelFilter {
    #[default]
    All, // Show all albums
    Mono,          // Only 1-channel albums
    Stereo,        // Only 2-channel albums
    Multichannel,  // Only albums with > 2 channels
    Mixed,         // Only albums with mixed channel counts
    Specific(u32), // Only albums with specific channel count
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

    /// Scan state
    pub scan_in_progress: bool,
    pub scan_progress_tracks: usize,
    pub scan_progress_albums: usize,
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
            items_per_page: 50,
            scan_in_progress: false,
            scan_progress_tracks: 0,
            scan_progress_albums: 0,
        }
    }

    /// Create library state for testing (in-memory, no database)
    pub fn new_for_test() -> Self {
        Self::with_library(MusicLibrary::new())
    }

    // =========================================================================
    // Filtering and Sorting
    // =========================================================================

    /// Get filtered and sorted albums
    pub fn filtered_albums(&self) -> Vec<&Album> {
        let mut albums: Vec<&Album> = if self.search_query.is_empty() {
            self.library.albums.iter().collect()
        } else {
            self.library.search_albums(&self.search_query)
        };

        // Apply channel filter
        albums.retain(|album| self.matches_filter(album));

        // Apply sort
        self.sort_albums(&mut albums);

        albums
    }

    /// Check if album matches current filter
    fn matches_filter(&self, album: &Album) -> bool {
        match self.filter {
            ChannelFilter::All => true,
            ChannelFilter::Mono => album.uniform_channel_count() == Some(1),
            ChannelFilter::Stereo => album.uniform_channel_count() == Some(2),
            ChannelFilter::Multichannel => album
                .uniform_channel_count()
                .map(|c| c > 2)
                .unwrap_or(false),
            ChannelFilter::Mixed => album.uniform_channel_count().is_none(),
            ChannelFilter::Specific(n) => album.uniform_channel_count() == Some(n),
        }
    }

    /// Sort albums according to current sort order
    fn sort_albums(&self, albums: &mut Vec<&Album>) {
        match self.sort_order {
            LibrarySortOrder::Artist => {
                albums.sort_by(|a, b| {
                    a.artist()
                        .cmp(&b.artist())
                        .then_with(|| a.year.cmp(&b.year).reverse())
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
            LibrarySortOrder::Album | LibrarySortOrder::Title => {
                albums.sort_by(|a, b| a.title.cmp(&b.title));
            }
            LibrarySortOrder::Year => {
                albums.sort_by(|a, b| {
                    b.year
                        .cmp(&a.year)
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
    }

    /// Set channel filter and reset selection
    pub fn set_filter(&mut self, filter: ChannelFilter) {
        self.filter = filter;
        self.selected_index = 0;
    }

    /// Cycle to next channel filter
    pub fn cycle_filter(&mut self) {
        self.filter = match self.filter {
            ChannelFilter::All => ChannelFilter::Mono,
            ChannelFilter::Mono => ChannelFilter::Stereo,
            ChannelFilter::Stereo => ChannelFilter::Multichannel,
            ChannelFilter::Multichannel => ChannelFilter::Mixed,
            ChannelFilter::Mixed | ChannelFilter::Specific(_) => ChannelFilter::All,
        };
        self.selected_index = 0;
    }

    /// Set search query and reset selection
    pub fn set_search_query(&mut self, query: String) {
        self.search_query = query;
        self.selected_index = 0;
    }

    /// Clear search query
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.selected_index = 0;
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
        self.library.remove_directory(index)
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

        result
    }

    /// Load library from database
    pub fn load_from_database(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.library.load_from_database()?;
        Ok(())
    }
}
