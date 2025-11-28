//! Library state management.
//!
//! Contains the library-specific state: albums, filtering, sorting, tree view.
//! This can be used as an independent Entity in GPUI for better separation.

use std::collections::HashMap;
use std::path::PathBuf;

use sotf_audio_player::{Album, MusicLibrary};

/// View mode for library display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibraryViewMode {
    Flat,     // Original list view
    TreeView, // Hierarchical artist → albums
    #[default]
    Grid, // Album grid with thumbnails
}

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
    All,           // Show all albums
    Mono,          // Only 1-channel albums
    Stereo,        // Only 2-channel albums
    Multichannel,  // Only albums with > 2 channels
    Mixed,         // Only albums with mixed channel counts
    Specific(u32), // Only albums with specific channel count
}

/// Letter node in tree view (groups albums by first letter of artist)
#[derive(Debug, Clone)]
pub struct LetterNode {
    pub letter: char,
    pub album_indices: Vec<usize>,
    pub expanded: bool,
}

/// Tree item type for rendering
#[derive(Debug, Clone)]
pub enum TreeItem {
    Letter { letter: char, expanded: bool },
    Album { index: usize },
}

/// Library state - can be used as a GPUI Entity
#[derive(Debug)]
pub struct LibraryState {
    /// The underlying music library (albums, directories)
    pub library: MusicLibrary,

    /// Current view mode
    pub view_mode: LibraryViewMode,

    /// Current sort order
    pub sort_order: LibrarySortOrder,

    /// Current channel filter
    pub filter: ChannelFilter,

    /// Current search query
    pub search_query: String,

    /// Selected album index (for flat/grid view)
    pub selected_index: usize,

    /// Selected tree index (for tree view)
    pub selected_tree_index: usize,

    /// Letter tree for tree view
    pub letter_tree: Vec<LetterNode>,

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
            view_mode: LibraryViewMode::default(),
            sort_order: LibrarySortOrder::default(),
            filter: ChannelFilter::default(),
            search_query: String::new(),
            selected_index: 0,
            selected_tree_index: 0,
            letter_tree: Vec::new(),
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
            ChannelFilter::Multichannel => {
                album.uniform_channel_count().map(|c| c > 2).unwrap_or(false)
            }
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
        self.selected_tree_index = 0;
        if self.view_mode == LibraryViewMode::TreeView {
            self.rebuild_letter_tree();
        }
    }

    /// Set channel filter and reset selection
    pub fn set_filter(&mut self, filter: ChannelFilter) {
        self.filter = filter;
        self.selected_index = 0;
        self.selected_tree_index = 0;
        if self.view_mode == LibraryViewMode::TreeView {
            self.rebuild_letter_tree();
        }
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
        self.selected_tree_index = 0;
        if self.view_mode == LibraryViewMode::TreeView {
            self.rebuild_letter_tree();
        }
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
    // View Mode
    // =========================================================================

    /// Set view mode
    pub fn set_view_mode(&mut self, mode: LibraryViewMode) {
        self.view_mode = mode;
        self.selected_index = 0;
        self.selected_tree_index = 0;
        if mode == LibraryViewMode::TreeView {
            self.rebuild_letter_tree();
        }
    }

    /// Cycle through view modes
    pub fn cycle_view_mode(&mut self) {
        self.set_view_mode(match self.view_mode {
            LibraryViewMode::Flat => LibraryViewMode::TreeView,
            LibraryViewMode::TreeView => LibraryViewMode::Grid,
            LibraryViewMode::Grid => LibraryViewMode::Flat,
        });
    }

    // =========================================================================
    // Tree View
    // =========================================================================

    /// Build letter tree from current albums (groups by first letter of artist)
    pub fn rebuild_letter_tree(&mut self) {
        let mut letter_map: HashMap<char, Vec<usize>> = HashMap::new();

        for (idx, album) in self.library.albums.iter().enumerate() {
            let first_letter = album
                .artist
                .chars()
                .next()
                .unwrap_or('#')
                .to_ascii_uppercase();
            let letter = if first_letter.is_ascii_alphabetic() {
                first_letter
            } else {
                '#'
            };
            letter_map.entry(letter).or_default().push(idx);
        }

        let mut letters: Vec<_> = letter_map.into_iter().collect();
        letters.sort_by(|a, b| match (a.0, b.0) {
            ('#', _) => std::cmp::Ordering::Greater,
            (_, '#') => std::cmp::Ordering::Less,
            _ => a.0.cmp(&b.0),
        });

        self.letter_tree = letters
            .into_iter()
            .map(|(letter, album_indices)| LetterNode {
                letter,
                album_indices,
                expanded: false,
            })
            .collect();

        self.selected_tree_index = 0;
    }

    /// Toggle expansion of currently selected letter node
    pub fn toggle_letter_expansion(&mut self) {
        if self.view_mode != LibraryViewMode::TreeView {
            return;
        }

        let mut current_row = 0;
        for letter_node in &mut self.letter_tree {
            if current_row == self.selected_tree_index {
                letter_node.expanded = !letter_node.expanded;
                return;
            }
            current_row += 1;
            if letter_node.expanded {
                current_row += letter_node.album_indices.len();
            }
        }
    }

    /// Get flattened tree items for rendering
    pub fn get_tree_items(&self) -> Vec<TreeItem> {
        let mut items = Vec::new();
        for letter_node in &self.letter_tree {
            items.push(TreeItem::Letter {
                letter: letter_node.letter,
                expanded: letter_node.expanded,
            });
            if letter_node.expanded {
                for &album_idx in &letter_node.album_indices {
                    items.push(TreeItem::Album { index: album_idx });
                }
            }
        }
        items
    }

    /// Get total tree item count
    pub fn tree_item_count(&self) -> usize {
        self.get_tree_items().len()
    }

    // =========================================================================
    // Pagination
    // =========================================================================

    /// Get paginated albums for flat/grid view
    pub fn get_paginated_albums(&self) -> Vec<&Album> {
        let all_albums = self.filtered_albums();
        let start = self.current_page * self.items_per_page;
        let end = (start + self.items_per_page).min(all_albums.len());
        if start >= all_albums.len() {
            return Vec::new();
        }
        all_albums[start..end].to_vec()
    }

    /// Get paginated tree items
    pub fn get_paginated_tree_items(&self) -> Vec<TreeItem> {
        let all_items = self.get_tree_items();
        let start = self.current_page * self.items_per_page;
        let end = (start + self.items_per_page).min(all_items.len());
        if start >= all_items.len() {
            return Vec::new();
        }
        all_items[start..end].to_vec()
    }

    /// Get total pages for current view mode
    pub fn total_pages(&self) -> usize {
        let total_items = match self.view_mode {
            LibraryViewMode::Flat | LibraryViewMode::Grid => self.filtered_albums().len(),
            LibraryViewMode::TreeView => self.tree_item_count(),
        };
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
            self.selected_tree_index = 0;
        }
    }

    /// Go to previous page
    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
            self.selected_index = 0;
            self.selected_tree_index = 0;
        }
    }

    /// Reset to first page
    pub fn reset_page(&mut self) {
        self.current_page = 0;
    }

    // =========================================================================
    // Navigation
    // =========================================================================

    /// Total item count for current view
    pub fn item_count(&self) -> usize {
        match self.view_mode {
            LibraryViewMode::Flat | LibraryViewMode::Grid => self.filtered_albums().len(),
            LibraryViewMode::TreeView => self.tree_item_count(),
        }
    }

    /// Select next item
    pub fn select_next(&mut self) {
        match self.view_mode {
            LibraryViewMode::Flat | LibraryViewMode::Grid => {
                let count = self.filtered_albums().len();
                if count > 0 {
                    self.selected_index = (self.selected_index + 1) % count;
                }
            }
            LibraryViewMode::TreeView => {
                let count = self.tree_item_count();
                if count > 0 {
                    self.selected_tree_index = (self.selected_tree_index + 1) % count;
                }
            }
        }
    }

    /// Select previous item
    pub fn select_prev(&mut self) {
        match self.view_mode {
            LibraryViewMode::Flat | LibraryViewMode::Grid => {
                let count = self.filtered_albums().len();
                if count > 0 {
                    self.selected_index = if self.selected_index == 0 {
                        count - 1
                    } else {
                        self.selected_index - 1
                    };
                }
            }
            LibraryViewMode::TreeView => {
                let count = self.tree_item_count();
                if count > 0 {
                    self.selected_tree_index = if self.selected_tree_index == 0 {
                        count - 1
                    } else {
                        self.selected_tree_index - 1
                    };
                }
            }
        }
    }

    /// Page down navigation
    pub fn page_down(&mut self, page_size: usize) {
        match self.view_mode {
            LibraryViewMode::Flat | LibraryViewMode::Grid => {
                let count = self.filtered_albums().len();
                if count > 0 {
                    self.selected_index = (self.selected_index + page_size).min(count - 1);
                }
            }
            LibraryViewMode::TreeView => {
                let count = self.tree_item_count();
                if count > 0 {
                    self.selected_tree_index = (self.selected_tree_index + page_size).min(count - 1);
                }
            }
        }
    }

    /// Page up navigation
    pub fn page_up(&mut self, page_size: usize) {
        match self.view_mode {
            LibraryViewMode::Flat | LibraryViewMode::Grid => {
                self.selected_index = self.selected_index.saturating_sub(page_size);
            }
            LibraryViewMode::TreeView => {
                self.selected_tree_index = self.selected_tree_index.saturating_sub(page_size);
            }
        }
    }

    /// Grid-specific navigation (left)
    pub fn select_grid_left(&mut self, grid_columns: usize) {
        if self.view_mode != LibraryViewMode::Grid {
            return;
        }
        if self.selected_index % grid_columns > 0 {
            self.selected_index -= 1;
        }
    }

    /// Grid-specific navigation (right)
    pub fn select_grid_right(&mut self, grid_columns: usize) {
        if self.view_mode != LibraryViewMode::Grid {
            return;
        }
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
        if self.view_mode != LibraryViewMode::Grid {
            return;
        }
        if self.selected_index >= grid_columns {
            self.selected_index -= grid_columns;
        }
    }

    /// Grid-specific navigation (down)
    pub fn select_grid_down(&mut self, grid_columns: usize) {
        if self.view_mode != LibraryViewMode::Grid {
            return;
        }
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

    /// Get currently selected album (for flat/grid view)
    pub fn selected_album(&self) -> Option<&Album> {
        let albums = self.filtered_albums();
        albums.get(self.selected_index).copied()
    }

    /// Get album at index from tree selection
    pub fn tree_selected_album(&self) -> Option<&Album> {
        let items = self.get_tree_items();
        match items.get(self.selected_tree_index)? {
            TreeItem::Album { index } => self.library.albums.get(*index),
            TreeItem::Letter { .. } => None,
        }
    }

    /// Get currently selected album (handles both view modes)
    pub fn get_selected_album(&self) -> Option<&Album> {
        match self.view_mode {
            LibraryViewMode::Flat | LibraryViewMode::Grid => self.selected_album(),
            LibraryViewMode::TreeView => self.tree_selected_album(),
        }
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
        self.rebuild_letter_tree();

        result
    }

    /// Load library from database
    pub fn load_from_database(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.library.load_from_database()?;
        self.rebuild_letter_tree();
        Ok(())
    }
}

