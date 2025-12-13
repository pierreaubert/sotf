//! Library management methods.
//!
//! Contains methods for library filtering, sorting, directories, and scanning.

use std::path::PathBuf;

use sotf_audio_player::Album;

use super::state::App;
use super::types::{ChannelFilter, LibrarySortOrder, ToastMessage};

impl App {
    pub fn filtered_albums(&self) -> Vec<Album> {
        self.library.get_filtered_albums(
            &self.search_query,
            self.library_sort_order,
            self.channel_filter,
        )
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
        if self.scan_in_progress {
            return Ok(());
        }

        self.scan_in_progress = true;
        self.scan_progress_tracks = 0;
        self.scan_progress_albums = 0;
        // Toast removed to avoid clutter
        // self.toast_message = Some(ToastMessage::info("Full library rescan..."));

        // Start background scanner with force=true
        let directories: Vec<std::path::PathBuf> = self
            .library
            .directories
            .iter()
            .map(|d| d.path.clone())
            .collect();
        self.library_scanner = Some(sotf_audio_player::LibraryScanner::start_force(directories));

        Ok(())
    }

    /// Scan for ReplayGain
    pub fn scan_replay_gain(&mut self) {
        match self.replay_gain_manager.start_scan() {
            Ok(msg) => {
                self.toast_message = Some(ToastMessage::info(msg));
            }
            Err(e) => {
                self.toast_message = Some(ToastMessage::error(format!(
                    "Failed to start ReplayGain scan: {}",
                    e
                )));
            }
        }
    }

    /// Scan for Bliss audio analysis (tempo, features for similarity)
    pub fn scan_bliss(&mut self) {
        match self.bliss_manager.start_scan() {
            Ok(msg) => {
                self.toast_message = Some(ToastMessage::info(msg));
            }
            Err(e) => {
                self.toast_message = Some(ToastMessage::error(format!(
                    "Failed to start bliss analysis scan: {}",
                    e
                )));
            }
        }
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
        // Toast removed to avoid clutter, progress is shown in UI
        // self.toast_message = Some(ToastMessage::info("Scanning library..."));

        // Start background scanner
        let directories: Vec<std::path::PathBuf> = self
            .library
            .directories
            .iter()
            .map(|d| d.path.clone())
            .collect();
        self.library_scanner = Some(sotf_audio_player::LibraryScanner::start(directories));

        Ok(())
    }

    /// Update library scan progress
    pub fn update_library_scan(&mut self) {
        let mut reload_needed = false;

        // Handle library scanner updates
        if let Some(scanner) = &self.library_scanner {
            let mut done = false;
            // Drain all available messages
            while let Some(msg) = scanner.try_recv() {
                match msg {
                    sotf_audio_player::LibraryScanMessage::Progress { tracks, albums } => {
                        self.scan_progress_tracks = tracks;
                        self.scan_progress_albums = albums;
                    }
                    sotf_audio_player::LibraryScanMessage::Complete { tracks, albums } => {
                        self.scan_progress_tracks = tracks;
                        self.scan_progress_albums = albums;
                        self.toast_message = Some(ToastMessage::success(format!(
                            "Scan complete. Found {} tracks in {} albums.",
                            tracks, albums
                        )));
                        done = true;
                        reload_needed = true;
                    }
                    sotf_audio_player::LibraryScanMessage::Error { message } => {
                        log::error!("Library scan failed: {}", message);
                        self.toast_message =
                            Some(ToastMessage::error(format!("Scan failed: {}", message)));
                        done = true;
                    }
                }
            }

            if done {
                self.library_scanner = None;
                self.scan_in_progress = false;
                self.needs_rescan = false;
            }
        }

        if reload_needed {
            // Reload library to show new items
            if let Err(e) = self.load_library_from_database() {
                log::error!("Failed to reload library after scan: {}", e);
                self.toast_message = Some(ToastMessage::error(
                    "Scan complete but failed to reload library.",
                ));
            }
        }
    }

    /// Get flattened directory tree for display
    pub fn get_directory_tree_items(&self) -> Vec<(PathBuf, usize, bool)> {
        self.library.get_directory_tree_items()
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
