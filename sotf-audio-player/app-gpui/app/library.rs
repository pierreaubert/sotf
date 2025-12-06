//! Library management methods.
//!
//! Contains methods for library filtering, sorting, directories, and scanning.

use std::path::PathBuf;

use sotf_audio_player::Album;

use super::state::App;
use super::types::{ChannelFilter, LibrarySortOrder, ToastMessage, ToastType};

impl App {
    pub fn filtered_albums(&self) -> Vec<Album> {
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

        // Group and merge albums
        let mut merged_albums = sotf_audio_player::library::group_and_merge_albums(albums);

        // Finally, sort
        match self.library_sort_order {
            LibrarySortOrder::Year => {
                merged_albums.sort_by(|a, b| {
                    b.year
                        .cmp(&a.year)
                        .then_with(|| a.artist().cmp(&b.artist()))
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
            LibrarySortOrder::Genre => {
                merged_albums.sort_by(|a, b| {
                    let genre_a = a.tracks.first().and_then(|t| t.genre.as_ref()).map(|s| s.to_lowercase());
                    let genre_b = b.tracks.first().and_then(|t| t.genre.as_ref()).map(|s| s.to_lowercase());
                    genre_a
                        .cmp(&genre_b)
                        .then_with(|| a.artist().cmp(&b.artist()))
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
            LibrarySortOrder::Artist => {
                merged_albums.sort_by(|a, b| {
                    a.artist()
                        .cmp(&b.artist())
                        .then_with(|| a.year.cmp(&b.year).reverse())
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
            LibrarySortOrder::Album => {
                merged_albums.sort_by(|a, b| a.title.cmp(&b.title));
            }
            LibrarySortOrder::Tracks => {
                merged_albums.sort_by(|a, b| {
                    b.tracks.len()
                        .cmp(&a.tracks.len())
                        .then_with(|| a.artist().cmp(&b.artist()))
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
            LibrarySortOrder::Composer => {
                merged_albums.sort_by(|a, b| {
                    let composer_a = a.tracks.first().and_then(|t| t.composer.as_ref()).map(|s| s.to_lowercase());
                    let composer_b = b.tracks.first().and_then(|t| t.composer.as_ref()).map(|s| s.to_lowercase());
                    composer_a
                        .cmp(&composer_b)
                        .then_with(|| a.artist().cmp(&b.artist()))
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
        }

        merged_albums
    }

    /// Set library sort order
    pub fn set_library_sort_order(&mut self, order: LibrarySortOrder) {
        self.library_sort_order = order;
        // Reset selection and page to top when changing sort order
        self.selected_album_index = 0;
        self.reset_page();
    }


    /// Set channel filter
    pub fn set_channel_filter(&mut self, filter: ChannelFilter) {
        self.channel_filter = filter;
        // Reset selection and page to top when changing filter
        self.selected_album_index = 0;
        self.reset_page();
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
        // Reset selection and page
        self.selected_album_index = 0;
        self.reset_page();
    }

    /// Get paginated albums for grid view
    pub fn get_paginated_albums(&self) -> Vec<Album> {
        let all_albums = self.filtered_albums();
        if all_albums.is_empty() {
            return Vec::new();
        }
        let end = self.library_items_per_page.min(all_albums.len());
        all_albums[0..end].to_vec()
    }

    /// Reset to first page
    pub fn reset_page(&mut self) {
        self.recalculate_pagination(true);
    }

    /// Recalculate items per page based on window size
    pub fn recalculate_pagination(&mut self, force_reset: bool) {
        // Estimate grid dimensions
        // Card min width is 160px + 16px gap = 176px
        // Card height is approx 240px + 16px gap = 256px
        // Sidebar is approx 0 in compact, or split in expanded

        let available_width = self.window_width - 32.0; // Minus padding
        let columns = (available_width / 176.0).floor().max(1.0) as usize;
        self.library_columns = columns;

        // Estimate available height for grid
        // Header (40) + Stats (100) + Filter (40) + Pagination (50) + Footer (60) = ~290px
        let available_height = (self.window_height - 290.0).max(256.0);
        let rows = (available_height / 256.0).floor().max(1.0) as usize;

        // Initial load: 3 screens worth of items
        let new_items_per_page = columns * rows * 3;

        // Only update if we are initializing, resizing significantly, or forcing reset
        if force_reset || self.library_items_per_page < new_items_per_page {
            self.library_items_per_page = new_items_per_page;
        }
    }

    /// Load more albums (infinite scroll)
    pub fn load_more_albums(&mut self) {
        let total = self.filtered_albums().len();
        if self.library_items_per_page < total {
            // Add 5 rows worth of items
            let more = self.library_columns * 5;
            self.library_items_per_page = (self.library_items_per_page + more).min(total);
        }
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



    /// Full rescan of the library
    pub fn rescan_library(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.toast_message = Some(ToastMessage::info("Starting full library rescan..."));
        // TODO: Implement actual full rescan logic in MusicLibrary if different from scan
        // For now, we can clear and rescan or just call scan with a force flag if available
        // Assuming scan_library is incremental, we might need a force_scan_library
        self.scan_library()
    }
    
    /// Scan for ReplayGain
    pub fn scan_replay_gain(&mut self) {
         self.toast_message = Some(ToastMessage::info("ReplayGain scan started (not implemented yet)..."));
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
        self.scan_library().ok();
    }

    /// Cancel the ongoing library scan
    pub fn cancel_library_scan(&mut self) {
        if let Some(scanner) = &self.library_scanner {
            scanner.cancel();
            self.toast_message = Some(ToastMessage::info("Cancelling scan..."));
        }
    }

    /// Scan the library with progress tracking
    pub fn scan_library(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.scan_in_progress {
            return Ok(());
        }

        self.scan_in_progress = true;
        self.scan_progress_tracks = 0;
        self.scan_progress_albums = 0;
        self.toast_message = Some(ToastMessage::persistent(
            "Scanning library...",
            ToastType::Info,
        ));

        // Start background scanner
        let directories: Vec<std::path::PathBuf> = self.library.directories.iter().map(|d| d.path.clone()).collect();
        self.library_scanner = Some(sotf_audio_player::LibraryScanner::start(directories));
        
        Ok(())
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
