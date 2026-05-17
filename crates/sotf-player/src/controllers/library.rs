//! Library controller — owns library state and encapsulates filtering, sorting,
//! pagination, navigation, directory management, and scanning.
//!
//! Lifted from app-gpui's `LibraryState` with GPUI-specific patterns removed.

use std::path::PathBuf;

use crate::{Album, ChannelFilter, LibrarySortOrder, MusicLibrary, group_and_merge_albums};

/// Default number of items per page for pagination.
const DEFAULT_ITEMS_PER_PAGE: usize = 50;

#[derive(Debug)]
pub struct LibraryController {
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

    /// Filter selections for each sort mode
    pub selected_genre: Option<String>,
    pub selected_decade: Option<(i32, i32)>,
    pub selected_year: Option<i32>,
    pub selected_artist_letter: Option<char>,
    pub selected_artist: Option<String>,
    pub selected_composer_letter: Option<char>,
    pub selected_composer: Option<String>,
    pub selected_album_letter: Option<char>,
    pub selected_track_range: Option<(usize, usize)>,

    /// Show only favorited albums
    pub show_favorites_only: bool,

    /// Scan state
    pub scan_in_progress: bool,
    pub scan_progress_tracks: usize,
    pub scan_progress_albums: usize,

    /// Cache for filtered and sorted albums
    cached_albums: Vec<Album>,
    cache_dirty: bool,

    /// Cache for the second-stage selection filter (genre/decade/year/artist/…).
    /// Stores indices into `cached_albums`. Invalidated whenever any selection
    /// field changes or `cached_albums` is rebuilt. `None` means "not yet
    /// computed" (cold start); empty Vec means "computed, result is empty".
    cached_selection_indices: Option<Vec<usize>>,
}

impl Default for LibraryController {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryController {
    /// Create new library controller with default database.
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

    /// Create library controller with provided library.
    pub fn with_library(library: MusicLibrary) -> Self {
        Self {
            library,
            sort_order: LibrarySortOrder::default(),
            filter: ChannelFilter::default(),
            search_query: String::new(),
            selected_index: 0,
            current_page: 0,
            items_per_page: DEFAULT_ITEMS_PER_PAGE,
            selected_genre: None,
            selected_decade: None,
            selected_year: None,
            selected_artist_letter: None,
            selected_artist: None,
            selected_composer_letter: None,
            selected_composer: None,
            selected_album_letter: None,
            selected_track_range: None,
            show_favorites_only: false,
            scan_in_progress: false,
            scan_progress_tracks: 0,
            scan_progress_albums: 0,
            cached_albums: Vec::new(),
            cache_dirty: true,
            cached_selection_indices: None,
        }
    }

    /// Create library controller for testing (in-memory, no database).
    pub fn new_for_test() -> Self {
        Self::with_library(MusicLibrary::new())
    }

    // =========================================================================
    // Filtering and Sorting
    // =========================================================================

    /// Mark the cache as dirty so it will be recomputed next time it's needed.
    /// Also invalidates the second-stage selection cache, which sits on top of
    /// the primary cached albums.
    pub fn invalidate_cache(&mut self) {
        self.cache_dirty = true;
        self.cached_selection_indices = None;
    }

    /// Ensure the cached filtered albums are up to date.
    pub fn ensure_cache_valid(&mut self) {
        if self.cache_dirty {
            self.recompute_cache();
        }
    }

    /// Recompute the filtered and sorted albums cache.
    fn recompute_cache(&mut self) {
        let mut albums: Vec<&Album> = if self.search_query.is_empty() {
            self.library.albums.iter().collect()
        } else {
            self.library.search_albums(&self.search_query)
        };

        // Apply channel filter
        albums.retain(|album| self.matches_channel_filter(album));

        // Apply favorites filter
        if self.show_favorites_only {
            albums.retain(|album| album.is_favorite);
        }

        // Merge duplicate albums (multi-disc, different editions, etc.)
        let mut merged_albums: Vec<Album> = group_and_merge_albums(albums);

        // Apply sort (on owned albums)
        merged_albums.sort_by(|a, b| self.sort_cmp(a, b));

        self.cached_albums = merged_albums;
        self.cache_dirty = false;
        // Rebuilt cached_albums → any selection-cache snapshot keyed off the
        // old indices is stale.
        self.cached_selection_indices = None;
    }

    /// Get filtered and sorted albums from cache.
    ///
    /// Returns the cached result of search + channel filter + merge + sort.
    /// Callers should call `ensure_cache_valid()` before this if they have `&mut self`.
    pub fn filtered_albums(&self) -> Vec<&Album> {
        self.cached_albums.iter().collect()
    }

    /// Single combined predicate for the sidebar selection filters.
    /// Skips everything when a search query is active.
    fn matches_selection(&self, album: &Album) -> bool {
        if !self.search_query.is_empty() {
            return true;
        }

        if let Some(ref genre) = self.selected_genre
            && !album
                .tracks
                .first()
                .and_then(|t| t.genre.as_ref())
                .is_some_and(|g| g.eq_ignore_ascii_case(genre))
        {
            return false;
        }

        if let Some((decade_start, decade_end)) = self.selected_decade
            && self.selected_year.is_none()
            && !album
                .year
                .map(|y| y as i32)
                .is_some_and(|y| y >= decade_start && y <= decade_end)
        {
            return false;
        }

        if let Some(year) = self.selected_year
            && album.year.map(|y| y as i32) != Some(year)
        {
            return false;
        }

        if let Some(letter) = self.selected_artist_letter
            && self.selected_artist.is_none()
            && !album.artist().chars().next().is_some_and(|c| {
                let first = c.to_ascii_uppercase();
                if letter == '#' {
                    !first.is_ascii_alphabetic()
                } else {
                    first == letter
                }
            })
        {
            return false;
        }

        if let Some(ref artist) = self.selected_artist
            && !album.artist().eq_ignore_ascii_case(artist)
        {
            return false;
        }

        if let Some(letter) = self.selected_composer_letter
            && self.selected_composer.is_none()
            && !album
                .tracks
                .first()
                .and_then(|t| t.composer.as_ref())
                .is_some_and(|c| {
                    c.chars().next().is_some_and(|ch| {
                        let first = ch.to_ascii_uppercase();
                        if letter == '#' {
                            !first.is_ascii_alphabetic()
                        } else {
                            first == letter
                        }
                    })
                })
        {
            return false;
        }

        if let Some(ref composer) = self.selected_composer
            && !album
                .tracks
                .first()
                .and_then(|t| t.composer.as_ref())
                .is_some_and(|c| c.eq_ignore_ascii_case(composer))
        {
            return false;
        }

        if let Some(letter) = self.selected_album_letter
            && !album.title.chars().next().is_some_and(|c| {
                let first = c.to_ascii_uppercase();
                if letter == '#' {
                    !first.is_ascii_alphabetic()
                } else {
                    first == letter
                }
            })
        {
            return false;
        }

        if let Some((min, max)) = self.selected_track_range {
            let count = album.tracks.len();
            if count < min || count > max {
                return false;
            }
        }

        true
    }

    /// Get filtered albums with selection filters applied on top of the cache.
    ///
    /// Uses `cached_selection_indices` when warm to skip the per-keystroke
    /// re-walk of the cached_albums vec. When cold (or after any selection
    /// field changes), falls back to a single-pass `iter().filter()` so the
    /// behaviour stays identical to the un-cached path.
    pub fn selection_filtered_albums(&self) -> Vec<&Album> {
        if let Some(indices) = &self.cached_selection_indices {
            return indices
                .iter()
                .filter_map(|i| self.cached_albums.get(*i))
                .collect();
        }
        // Cold-path: single combined-predicate pass over the cached albums,
        // instead of the legacy chain of ten retain()s.
        self.cached_albums
            .iter()
            .filter(|a| self.matches_selection(a))
            .collect()
    }

    /// Populate `cached_selection_indices` so subsequent `selection_filtered_albums`
    /// calls are O(n) over a tighter list rather than O(n) over all cached
    /// albums. Safe to call from any code that already holds `&mut self`.
    pub fn ensure_selection_cache_valid(&mut self) {
        if self.cached_selection_indices.is_some() {
            return;
        }
        let mut indices = Vec::with_capacity(self.cached_albums.len());
        for (i, album) in self.cached_albums.iter().enumerate() {
            if self.matches_selection(album) {
                indices.push(i);
            }
        }
        self.cached_selection_indices = Some(indices);
    }

    /// Check if album matches current channel filter.
    fn matches_channel_filter(&self, album: &Album) -> bool {
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

    /// Compare two albums according to current sort order.
    fn sort_cmp(&self, a: &Album, b: &Album) -> std::cmp::Ordering {
        match self.sort_order {
            LibrarySortOrder::Year => b
                .year
                .cmp(&a.year)
                .then_with(|| a.artist().cmp(&b.artist()))
                .then_with(|| a.title.cmp(&b.title)),
            LibrarySortOrder::Genre => {
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
            }
            LibrarySortOrder::Artist => a
                .artist()
                .cmp(&b.artist())
                .then_with(|| a.year.cmp(&b.year).reverse())
                .then_with(|| a.title.cmp(&b.title)),
            LibrarySortOrder::Album => a.title.cmp(&b.title),
            LibrarySortOrder::Tracks => b
                .tracks
                .len()
                .cmp(&a.tracks.len())
                .then_with(|| a.artist().cmp(&b.artist()))
                .then_with(|| a.title.cmp(&b.title)),
            LibrarySortOrder::Composer => {
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
            }
            LibrarySortOrder::Popularity => {
                let pop_a: usize = a.tracks.iter().map(|t| t.play_count).sum();
                let pop_b: usize = b.tracks.iter().map(|t| t.play_count).sum();
                pop_b
                    .cmp(&pop_a)
                    .then_with(|| a.artist().cmp(&b.artist()))
                    .then_with(|| a.title.cmp(&b.title))
            }
        }
    }

    /// Set sort order, clear selection filters, and reset selection.
    pub fn set_sort_order(&mut self, order: LibrarySortOrder) {
        self.sort_order = order;
        self.selected_index = 0;
        self.selected_genre = None;
        self.selected_decade = None;
        self.selected_year = None;
        self.selected_artist_letter = None;
        self.selected_artist = None;
        self.selected_composer_letter = None;
        self.selected_composer = None;
        self.selected_album_letter = None;
        self.selected_track_range = None;
        self.invalidate_cache();
    }

    /// Toggle favorites-only filter.
    pub fn toggle_favorites_filter(&mut self) {
        self.show_favorites_only = !self.show_favorites_only;
        self.selected_index = 0;
        self.invalidate_cache();
    }

    /// Set channel filter and reset selection.
    pub fn set_filter(&mut self, filter: ChannelFilter) {
        self.filter = filter;
        self.selected_index = 0;
        self.invalidate_cache();
    }

    /// Cycle to next channel filter.
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

    /// Set search query with smart sort-order switching.
    pub fn set_search_query(&mut self, query: String) {
        if query.is_empty() {
            self.clear_search();
            return;
        }

        self.search_query = query.clone();
        let query_lower = query.to_lowercase();

        // Smart view switching logic
        let mut best_order = None;
        let mut is_exact = false;

        // First pass: look for exact matches (highest priority)
        for album in &self.library.albums {
            if album.title.to_lowercase() == query_lower {
                best_order = Some(LibrarySortOrder::Album);
                is_exact = true;
                break;
            }
            if album.artist().to_lowercase() == query_lower {
                best_order = Some(LibrarySortOrder::Artist);
                is_exact = true;
            }
            if best_order != Some(LibrarySortOrder::Artist)
                && best_order != Some(LibrarySortOrder::Album)
            {
                for track in &album.tracks {
                    if let Some(composer) = &track.composer {
                        if composer.to_lowercase() == query_lower {
                            best_order = Some(LibrarySortOrder::Composer);
                            is_exact = true;
                            break;
                        }
                    }
                }
            }
        }

        // Second pass: if no exact match, look for partial matches
        if !is_exact {
            for album in &self.library.albums {
                if album.title.to_lowercase().contains(&query_lower) {
                    best_order = Some(LibrarySortOrder::Album);
                    break;
                }
                if album.artist().to_lowercase().contains(&query_lower) {
                    best_order = Some(LibrarySortOrder::Artist);
                }
                if best_order != Some(LibrarySortOrder::Artist)
                    && best_order != Some(LibrarySortOrder::Album)
                {
                    for track in &album.tracks {
                        if let Some(composer) = &track.composer {
                            if composer.to_lowercase().contains(&query_lower) {
                                best_order = Some(LibrarySortOrder::Composer);
                                break;
                            }
                        }
                    }
                }
            }
        }

        if let Some(order) = best_order {
            self.sort_order = order;
        }

        self.selected_index = 0;
        self.invalidate_cache();
    }

    /// Clear search query.
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.selected_index = 0;
        self.invalidate_cache();
    }

    /// Clear all selection filters (genre, year, artist, composer, album letter, track range)
    /// and reset channel filter to All.
    pub fn clear_all_filters(&mut self) {
        self.selected_genre = None;
        self.selected_decade = None;
        self.selected_year = None;
        self.selected_artist_letter = None;
        self.selected_artist = None;
        self.selected_composer_letter = None;
        self.selected_composer = None;
        self.selected_album_letter = None;
        self.selected_track_range = None;
        self.filter = ChannelFilter::All;
        self.show_favorites_only = false;
        self.search_query.clear();
        self.selected_index = 0;
        self.invalidate_cache();
    }

    /// Check if any filters are active (genre, year, artist, etc., channel filter, or search).
    pub fn has_active_filters(&self) -> bool {
        self.selected_genre.is_some()
            || self.selected_decade.is_some()
            || self.selected_year.is_some()
            || self.selected_artist_letter.is_some()
            || self.selected_artist.is_some()
            || self.selected_composer_letter.is_some()
            || self.selected_composer.is_some()
            || self.selected_album_letter.is_some()
            || self.selected_track_range.is_some()
            || self.filter != ChannelFilter::All
            || self.show_favorites_only
            || !self.search_query.is_empty()
    }

    // =========================================================================
    // Pagination
    // =========================================================================

    /// Get paginated albums.
    pub fn get_paginated_albums(&self) -> Vec<&Album> {
        let all_albums = self.filtered_albums();
        let start = self.current_page * self.items_per_page;
        let end = (start + self.items_per_page).min(all_albums.len());
        if start >= all_albums.len() {
            return Vec::new();
        }
        all_albums[start..end].to_vec()
    }

    /// Get total pages.
    pub fn total_pages(&self) -> usize {
        let total_items = self.filtered_albums().len();
        if total_items == 0 {
            1
        } else {
            total_items.div_ceil(self.items_per_page)
        }
    }

    /// Go to next page.
    pub fn next_page(&mut self) {
        if self.current_page + 1 < self.total_pages() {
            self.current_page += 1;
            self.selected_index = 0;
        }
    }

    /// Go to previous page.
    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
            self.selected_index = 0;
        }
    }

    /// Reset to first page.
    pub fn reset_page(&mut self) {
        self.current_page = 0;
    }

    // =========================================================================
    // Navigation
    // =========================================================================

    /// Total item count (from cache, without selection filters).
    pub fn item_count(&self) -> usize {
        self.filtered_albums().len()
    }

    /// Select next item.
    pub fn select_next(&mut self) {
        let count = self.filtered_albums().len();
        if count > 0 {
            self.selected_index = (self.selected_index + 1) % count;
        }
    }

    /// Select previous item.
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

    /// Page down navigation.
    pub fn page_down(&mut self, page_size: usize) {
        let count = self.filtered_albums().len();
        if count > 0 {
            self.selected_index = (self.selected_index + page_size).min(count - 1);
        }
    }

    /// Page up navigation.
    pub fn page_up(&mut self, page_size: usize) {
        self.selected_index = self.selected_index.saturating_sub(page_size);
    }

    /// Grid-specific navigation (left).
    pub fn select_grid_left(&mut self, grid_columns: usize) {
        if !self.selected_index.is_multiple_of(grid_columns) {
            self.selected_index -= 1;
        }
    }

    /// Grid-specific navigation (right).
    pub fn select_grid_right(&mut self, grid_columns: usize) {
        let count = self.filtered_albums().len();
        if count > 0
            && self.selected_index % grid_columns < grid_columns - 1
            && self.selected_index < count - 1
        {
            self.selected_index += 1;
        }
    }

    /// Grid-specific navigation (up).
    pub fn select_grid_up(&mut self, grid_columns: usize) {
        if self.selected_index >= grid_columns {
            self.selected_index -= grid_columns;
        }
    }

    /// Grid-specific navigation (down).
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

    /// Get currently selected album (from cache, without selection filters).
    pub fn selected_album(&self) -> Option<&Album> {
        let albums = self.filtered_albums();
        albums.get(self.selected_index).copied()
    }

    // =========================================================================
    // Directory Management
    // =========================================================================

    /// Add directory to library.
    pub fn add_directory(&mut self, path: PathBuf) -> Result<bool, String> {
        self.library.add_directory(path)
    }

    /// Remove directory at index.
    pub fn remove_directory(&mut self, index: usize) -> Option<PathBuf> {
        let result = self.library.remove_directory(index);
        if result.is_some() {
            self.invalidate_cache();
        }
        result
    }

    /// Get directory tree items for display.
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

    /// Scan library synchronously with progress callback.
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

    /// Load library from database.
    pub fn load_from_database(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.library.load_from_database()?;
        self.invalidate_cache();
        Ok(())
    }

    /// Clean up database by removing tracks for files that no longer exist.
    pub fn clean_database(&mut self) -> Result<usize, Box<dyn std::error::Error>> {
        let removed = self.library.clean_database()?;
        if removed > 0 {
            self.invalidate_cache();
        }
        Ok(removed)
    }
}
