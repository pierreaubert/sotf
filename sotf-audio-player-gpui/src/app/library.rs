//! Library management methods.
//!
//! Contains methods for library filtering, sorting, tree view, directories, and scanning.

use std::path::PathBuf;

use sotf_audio_player::Album;

use super::state::App;
use super::types::{
    ChannelFilter, LetterNode, LibrarySortOrder, LibraryViewMode, ToastMessage, ToastType, TreeItem,
};

impl App {
    pub fn filtered_albums(&self) -> Vec<&Album> {
        use ChannelFilter::*;

        // First filter by search query
        let mut albums: Vec<&Album> = if self.search_query.is_empty() {
            self.library.albums.iter().collect()
        } else {
            self.library.search_albums(&self.search_query)
        };

        // Then filter by channel count
        albums.retain(|album| match self.channel_filter {
            All => true,
            Mono => album.uniform_channel_count() == Some(1),
            Stereo => album.uniform_channel_count() == Some(2),
            Multichannel => {
                if let Some(count) = album.uniform_channel_count() {
                    count > 2
                } else {
                    false
                }
            }
            Mixed => album.uniform_channel_count().is_none(),
            Specific(n) => album.uniform_channel_count() == Some(n),
        });

        // Group by title and artist to consolidate editions
        // We group albums with the same title and artist, and only show the one with the best dynamic range
        let mut groups: std::collections::HashMap<String, Vec<&Album>> = std::collections::HashMap::new();
        for album in albums {
            let title = album.title.trim().to_lowercase();
            let artist = album.artist().trim().to_lowercase();
            let key = format!("{}|{}", title, artist);
            groups.entry(key).or_default().push(album);
        }

        // Select best edition from each group (highest dynamic range)
        let mut deduped_albums: Vec<&Album> = Vec::new();
        for group in groups.values() {
            if let Some(best) = group.iter().max_by(|a, b| {
                let dr_a = a.dynamic_range.unwrap_or(0.0);
                let dr_b = b.dynamic_range.unwrap_or(0.0);
                // Prefer higher dynamic range
                dr_a.partial_cmp(&dr_b).unwrap_or(std::cmp::Ordering::Equal)
            }) {
                deduped_albums.push(*best);
            }
        }
        albums = deduped_albums;

        // Finally, sort
        match self.library_sort_order {
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
            LibrarySortOrder::Title => {
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

        albums
    }

    /// Set library sort order
    pub fn set_library_sort_order(&mut self, order: LibrarySortOrder) {
        self.library_sort_order = order;
        // Reset selection to top when changing sort order
        self.selected_album_index = 0;
        self.selected_tree_index = 0;
        // Rebuild tree view if active (as sort order affects tree structure)
        if self.library_view_mode == LibraryViewMode::TreeView {
            self.rebuild_letter_tree();
        }
    }

    /// Set channel filter
    pub fn set_channel_filter(&mut self, filter: ChannelFilter) {
        self.channel_filter = filter;
        // Reset selection to top when changing filter
        self.selected_album_index = 0;
        self.selected_tree_index = 0;
        // Rebuild tree view if active
        if self.library_view_mode == LibraryViewMode::TreeView {
            self.rebuild_letter_tree();
        }
    }

    /// Cycle to next channel filter
    pub fn cycle_channel_filter(&mut self) {
        self.channel_filter = match self.channel_filter {
            ChannelFilter::All => ChannelFilter::Mono,
            ChannelFilter::Mono => ChannelFilter::Stereo,
            ChannelFilter::Stereo => ChannelFilter::Multichannel,
            ChannelFilter::Multichannel => ChannelFilter::Mixed,
            ChannelFilter::Mixed => ChannelFilter::All,
            ChannelFilter::Specific(_) => ChannelFilter::All,
        };
        // Reset selection and rebuild tree
        self.selected_album_index = 0;
        self.selected_tree_index = 0;
        if self.library_view_mode == LibraryViewMode::TreeView {
            self.rebuild_letter_tree();
        }
    }

    /// Build the letter tree from the current album list (groups by first letter of artist)
    pub fn rebuild_letter_tree(&mut self) {
        use std::collections::HashMap;

        let mut letter_map: HashMap<char, Vec<usize>> = HashMap::new();

        // Group albums by first letter based on sort order
        // When sorting by Album/Title, group by album title
        // When sorting by Artist/Year, group by artist name
        for (idx, album) in self.library.albums.iter().enumerate() {
            let group_by_text = match self.library_sort_order {
                LibrarySortOrder::Album | LibrarySortOrder::Title => album.title.clone(),
                LibrarySortOrder::Artist | LibrarySortOrder::Year => album.artist(),
            };

            let first_letter = group_by_text
                .chars()
                .next()
                .unwrap_or('#')
                .to_ascii_uppercase();
            // Group non-alphabetic characters under '#'
            let letter = if first_letter.is_ascii_alphabetic() {
                first_letter
            } else {
                '#'
            };
            letter_map.entry(letter).or_default().push(idx);
        }

        // Create letter nodes, sorted alphabetically
        let mut letters: Vec<_> = letter_map.into_iter().collect();
        letters.sort_by(|a, b| {
            // Put '#' at the end
            match (a.0, b.0) {
                ('#', _) => std::cmp::Ordering::Greater,
                (_, '#') => std::cmp::Ordering::Less,
                _ => a.0.cmp(&b.0),
            }
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

    /// Toggle library view mode (cycles through Flat → Tree → Grid)
    pub fn toggle_library_view_mode(&mut self) {
        self.library_view_mode = match self.library_view_mode {
            LibraryViewMode::Flat => LibraryViewMode::TreeView,
            LibraryViewMode::TreeView => LibraryViewMode::Grid,
            LibraryViewMode::Grid => LibraryViewMode::Flat,
        };
        self.selected_tree_index = 0;
        self.selected_album_index = 0;
    }

    /// Toggle expansion of the currently selected letter node
    pub fn toggle_letter_expansion(&mut self) {
        if self.library_view_mode != LibraryViewMode::TreeView {
            return;
        }

        // Find which letter node we're on
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

    /// Get the flattened tree items for rendering (returns letter headers and album indices)
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

    /// Get paginated tree items
    pub fn get_paginated_tree_items(&self) -> Vec<TreeItem> {
        let all_items = self.get_tree_items();
        let start = self.library_page * self.library_items_per_page;
        let end = (start + self.library_items_per_page).min(all_items.len());

        all_items[start..end].to_vec()
    }

    /// Get total number of pages for tree view
    pub fn get_tree_total_pages(&self) -> usize {
        let total_items = self.get_tree_items().len();
        if total_items == 0 {
            1
        } else {
            (total_items + self.library_items_per_page - 1) / self.library_items_per_page
        }
    }

    /// Get paginated albums for flat view
    pub fn get_paginated_albums(&self) -> Vec<&Album> {
        let all_albums = self.filtered_albums();
        let start = self.library_page * self.library_items_per_page;
        let end = (start + self.library_items_per_page).min(all_albums.len());

        all_albums[start..end].to_vec()
    }

    /// Get total number of pages for flat view
    pub fn get_flat_total_pages(&self) -> usize {
        let total_items = self.filtered_albums().len();
        if total_items == 0 {
            1
        } else {
            (total_items + self.library_items_per_page - 1) / self.library_items_per_page
        }
    }

    /// Go to next page
    pub fn next_page(&mut self) {
        let total_pages = match self.library_view_mode {
            LibraryViewMode::Flat | LibraryViewMode::Grid => self.get_flat_total_pages(),
            LibraryViewMode::TreeView => self.get_tree_total_pages(),
        };

        if self.library_page + 1 < total_pages {
            self.library_page += 1;
            // Reset selection when changing pages
            match self.library_view_mode {
                LibraryViewMode::Flat | LibraryViewMode::Grid => self.selected_album_index = 0,
                LibraryViewMode::TreeView => self.selected_tree_index = 0,
            }
        }
    }

    /// Go to previous page
    pub fn prev_page(&mut self) {
        if self.library_page > 0 {
            self.library_page -= 1;
            // Reset selection when changing pages
            match self.library_view_mode {
                LibraryViewMode::Flat | LibraryViewMode::Grid => self.selected_album_index = 0,
                LibraryViewMode::TreeView => self.selected_tree_index = 0,
            }
        }
    }

    /// Reset to first page
    pub fn reset_page(&mut self) {
        self.library_page = 0;
    }

    /// Add a directory to the library (interactive version with UI feedback)
    pub fn add_directory(&mut self, path: PathBuf) {
        match self.library.add_directory(path) {
            Ok(needs_scan) => {
                if needs_scan {
                    self.needs_rescan = true;
                    self.toast_message =
                        Some(ToastMessage::success("Directory added. Press 's' to scan."));
                } else {
                    self.toast_message = Some(ToastMessage::warning("Directory already exists."));
                }
            }
            Err(msg) => {
                self.toast_message = Some(ToastMessage::error(msg));
            }
        }
    }

    /// Add a directory without triggering rescan (for startup initialization)
    pub fn add_directory_quiet(&mut self, path: PathBuf) {
        let _ = self.library.add_directory(path);
    }

    /// Remove the selected directory from the library
    pub fn remove_selected_directory(&mut self) {
        // We need to map from tree index to actual directory index
        let tree_items = self.get_directory_tree_items();
        if let Some((path, level, _)) = tree_items.get(self.selected_directory_index) {
            // Only allow removing level 0 directories (main directories, not subdirectories)
            if *level == 0 {
                // Find the actual index in the directories vector
                if let Some(dir_index) = self
                    .library
                    .directories
                    .iter()
                    .position(|d| d.path == *path)
                {
                    if self.library.remove_directory(dir_index).is_some() {
                        // Adjust selected_directory_index if needed
                        let tree_items = self.get_directory_tree_items();
                        if self.selected_directory_index >= tree_items.len()
                            && self.selected_directory_index > 0
                        {
                            self.selected_directory_index = tree_items.len() - 1;
                        }
                        self.needs_rescan = true;
                        self.toast_message = Some(ToastMessage::success("Directory removed."));
                    }
                }
            } else {
                self.toast_message = Some(ToastMessage::error("Cannot remove subdirectory."));
            }
        }
    }

    /// Start library scan (sets up progress tracking flags)
    pub fn start_library_scan(&mut self) {
        self.scan_in_progress = true;
        self.scan_progress_tracks = 0;
        self.scan_progress_albums = 0;
        self.toast_message = Some(ToastMessage::info("Starting library scan..."));
    }

    /// Scan the library with progress tracking
    pub fn scan_library(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use parking_lot::Mutex;
        use std::sync::Arc;

        self.scan_in_progress = true;
        self.scan_progress_tracks = 0;
        self.scan_progress_albums = 0;
        self.toast_message = Some(ToastMessage::persistent(
            "Scanning library...",
            ToastType::Info,
        ));

        // Create shared progress state
        let progress_tracks = Arc::new(Mutex::new(0usize));
        let progress_albums = Arc::new(Mutex::new(0usize));
        let last_update_tracks = Arc::new(Mutex::new(0usize));

        let progress_tracks_clone = Arc::clone(&progress_tracks);
        let progress_albums_clone = Arc::clone(&progress_albums);
        let last_update_clone = Arc::clone(&last_update_tracks);

        // Use progress callback to update shared progress
        let result = self.library.scan_with_progress(move |tracks, albums| {
            let last = last_update_clone.lock();
            let should_update = tracks - *last >= 1000 || tracks == 0;

            if should_update {
                *progress_tracks_clone.lock() = tracks;
                *progress_albums_clone.lock() = albums;
                *last_update_clone.lock() = tracks;
                log::info!("Scan progress: {} tracks, {} albums found", tracks, albums);
            }
        });

        // Update app state with final progress
        self.scan_progress_tracks = *progress_tracks.lock();
        self.scan_progress_albums = *progress_albums.lock();

        self.scan_in_progress = false;
        self.needs_rescan = false;
        self.selected_album_index = 0;
        self.album_list_offset = 0;

        match &result {
            Ok(_) => {
                let album_count = self.library.albums.len();
                let track_count: usize = self.library.albums.iter().map(|a| a.tracks.len()).sum();
                self.toast_message = Some(ToastMessage::success(format!(
                    "Scan complete: {} tracks in {} albums",
                    track_count, album_count
                )));
                log::info!(
                    "Scan complete: {} tracks in {} albums",
                    track_count,
                    album_count
                );
            }
            Err(e) => {
                self.toast_message = Some(ToastMessage::error(format!("Scan failed: {}", e)));
                log::error!("Scan failed: {}", e);
            }
        }

        self.rebuild_letter_tree();

        result
    }

    /// Get flattened directory tree for display
    pub fn get_directory_tree_items(&self) -> Vec<(PathBuf, usize, bool)> {
        let mut items = Vec::new();
        for dir_info in &self.library.directories {
            // Add the main directory (level 0)
            items.push((dir_info.path.clone(), 0, dir_info.expanded));

            // Add subdirectories if expanded (level 1)
            if dir_info.expanded {
                for subdir in &dir_info.subdirectories {
                    items.push((subdir.path.clone(), 1, false));
                }
            }
        }
        items
    }

    /// Toggle directory expansion
    pub fn toggle_directory_expansion(&mut self) {
        // Find which directory in the tree we're selecting
        let tree_items = self.get_directory_tree_items();
        if let Some((path, level, _)) = tree_items.get(self.selected_directory_index) {
            // Only toggle if we're on a main directory (level 0)
            if *level == 0 {
                // Find the directory in our list and toggle it
                if let Some(dir_info) = self
                    .library
                    .directories
                    .iter_mut()
                    .find(|d| d.path == *path)
                {
                    dir_info.expanded = !dir_info.expanded;
                }
            }
            // If we're on a subdirectory (level 1), do nothing - it's already part of the tree
            // Don't add it as a new main directory or trigger a rescan
        }
    }
}
