//! Library management methods.
//!
//! Contains methods for library filtering, sorting, directories, and scanning.

use std::path::PathBuf;

use sotf_audio_player::Album;

use super::state::App;
use super::state::library::{ChannelFilter, LibrarySortOrder};
use super::types::ToastMessage;

impl App {
    pub fn filtered_albums(&self) -> Vec<Album> {
        // Convert state types to library types
        let sort_order = self.library_state.sort_order.to_library_sort_order();
        let channel_filter = self.library_state.filter.to_library_channel_filter();

        let mut albums = self.library_state.library.get_filtered_albums(
            &self.library_state.search_query,
            sort_order,
            channel_filter,
        );

        log::debug!(
            "filtered_albums: query='{}', library_count={}, result_count={}",
            self.library_state.search_query,
            self.library_state.library.albums.len(),
            albums.len()
        );

        // When there's an active search query, skip all selection filters.
        // This ensures search results are not filtered by letter/genre/decade/etc
        // that the user may have selected before entering search mode.
        if !self.library_state.search_query.is_empty() {
            return albums;
        }

        // Filter by selected genre if set
        if let Some(ref genre) = self.library_state.selected_genre {
            albums.retain(|album| {
                album
                    .tracks
                    .first()
                    .and_then(|t| t.genre.as_ref())
                    .is_some_and(|g| g.eq_ignore_ascii_case(genre))
            });
        }

        // Filter by selected decade if set (but no specific year)
        if let Some((decade_start, decade_end)) = self.library_state.selected_decade {
            if self.library_state.selected_year.is_none() {
                albums.retain(|album| {
                    album
                        .year
                        .map(|y| y as i32)
                        .is_some_and(|y| y >= decade_start && y <= decade_end)
                });
            }
        }

        // Filter by selected year if set
        if let Some(year) = self.library_state.selected_year {
            albums.retain(|album| album.year.map(|y| y as i32) == Some(year));
        }

        // Filter by selected artist letter if set (but no specific artist)
        if let Some(letter) = self.library_state.selected_artist_letter {
            if self.library_state.selected_artist.is_none() {
                albums.retain(|album| {
                    album.artist().chars().next().map_or(false, |c| {
                        let first = c.to_ascii_uppercase();
                        if letter == '#' {
                            !first.is_ascii_alphabetic()
                        } else {
                            first == letter
                        }
                    })
                });
            }
        }

        // Filter by selected artist if set
        if let Some(ref artist) = self.library_state.selected_artist {
            albums.retain(|album| album.artist().eq_ignore_ascii_case(artist));
        }

        // Filter by selected composer letter if set (but no specific composer)
        if let Some(letter) = self.library_state.selected_composer_letter {
            if self.library_state.selected_composer.is_none() {
                albums.retain(|album| {
                    album
                        .tracks
                        .first()
                        .and_then(|t| t.composer.as_ref())
                        .map_or(false, |c| {
                            c.chars().next().map_or(false, |ch| {
                                let first = ch.to_ascii_uppercase();
                                if letter == '#' {
                                    !first.is_ascii_alphabetic()
                                } else {
                                    first == letter
                                }
                            })
                        })
                });
            }
        }

        // Filter by selected composer if set
        if let Some(ref composer) = self.library_state.selected_composer {
            albums.retain(|album| {
                album
                    .tracks
                    .first()
                    .and_then(|t| t.composer.as_ref())
                    .is_some_and(|c| c.eq_ignore_ascii_case(composer))
            });
        }

        // Filter by selected album letter if set
        if let Some(letter) = self.library_state.selected_album_letter {
            albums.retain(|album| {
                album.title.chars().next().map_or(false, |c| {
                    let first = c.to_ascii_uppercase();
                    if letter == '#' {
                        !first.is_ascii_alphabetic()
                    } else {
                        first == letter
                    }
                })
            });
        }

        // Filter by selected track range if set
        if let Some((min, max)) = self.library_state.selected_track_range {
            albums.retain(|album| {
                let count = album.tracks.len();
                count >= min && count <= max
            });
        }

        albums
    }

    /// Set library sort order
    pub fn set_library_sort_order(&mut self, order: LibrarySortOrder) {
        self.library_state.sort_order = order;
        // Reset selection and page to top when changing sort order
        self.library_state.selected_index = 0;
        // Clear all selection filters when changing sort order
        self.library_state.selected_genre = None;
        self.library_state.selected_decade = None;
        self.library_state.selected_year = None;
        self.library_state.selected_artist_letter = None;
        self.library_state.selected_artist = None;
        self.library_state.selected_composer_letter = None;
        self.library_state.selected_composer = None;
        self.library_state.selected_album_letter = None;
        self.library_state.selected_track_range = None;
        self.reset_page();
    }

    /// Set channel filter
    pub fn set_channel_filter(&mut self, filter: ChannelFilter) {
        self.library_state.filter = filter;
        // Reset selection and page to top when changing filter
        self.library_state.selected_index = 0;
        self.reset_page();
    }

    /// Cycle to next channel filter
    pub fn cycle_channel_filter(&mut self) {
        self.library_state.filter = match self.library_state.filter {
            ChannelFilter::All => ChannelFilter::Mono,
            ChannelFilter::Mono => ChannelFilter::Stereo,
            ChannelFilter::Stereo => ChannelFilter::Surround,
            ChannelFilter::Surround => ChannelFilter::Surround71,
            ChannelFilter::Surround71 => ChannelFilter::SurroundPlus,
            ChannelFilter::SurroundPlus => ChannelFilter::Mixed,
            ChannelFilter::Mixed => ChannelFilter::All,
            ChannelFilter::Specific(_) => ChannelFilter::All,
        };
        // Reset selection and page
        self.library_state.selected_index = 0;
        self.reset_page();
    }

    /// Get paginated albums for grid view
    pub fn get_paginated_albums(&self) -> Vec<Album> {
        let all_albums = self.filtered_albums();
        if all_albums.is_empty() {
            return Vec::new();
        }
        let end = self.library_state.items_per_page.min(all_albums.len());
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

        let available_width = self.ui_state.window_width - 32.0; // Minus padding
        let columns = (available_width / 176.0).floor().max(1.0) as usize;
        self.library_state.library_columns = columns;

        // Estimate available height for grid
        // Header (40) + Stats (100) + Filter (40) + Pagination (50) + Footer (60) = ~290px
        let available_height = (self.ui_state.window_height - 290.0).max(256.0);
        let rows = (available_height / 256.0).floor().max(1.0) as usize;

        // Initial load: 3 screens worth of items
        let new_items_per_page = columns * rows * 3;

        // Only update if we are initializing, resizing significantly, or forcing reset
        if force_reset || self.library_state.items_per_page < new_items_per_page {
            self.library_state.items_per_page = new_items_per_page;
        }
    }

    /// Load more albums (infinite scroll)
    pub fn load_more_albums(&mut self) {
        let total = self.filtered_albums().len();
        if self.library_state.items_per_page < total {
            // Add 5 rows worth of items
            let more = self.library_state.library_columns * 5;
            self.library_state.items_per_page = (self.library_state.items_per_page + more).min(total);
        }
    }

    /// Add a directory to the library (interactive version with UI feedback)
    pub fn add_directory(&mut self, path: PathBuf) {
        match self.library_state.library.add_directory(path) {
            Ok(needs_scan) => {
                if needs_scan {
                    self.needs_rescan = true;
                    self.ui_state.toast_message =
                        Some(ToastMessage::success("Directory added. Press 's' to scan."));
                } else {
                    self.ui_state.toast_message = Some(ToastMessage::warning("Directory already exists."));
                }
            }
            Err(msg) => {
                self.ui_state.toast_message = Some(ToastMessage::error(msg));
            }
        }
    }

    /// Add a directory without triggering rescan (for startup initialization)
    pub fn add_directory_quiet(&mut self, path: PathBuf) {
        let _ = self.library_state.library.add_directory(path);
    }

    /// Full rescan of the library
    pub fn rescan_library(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.library_state.scan_in_progress {
            return Ok(());
        }

        self.library_state.scan_in_progress = true;
        self.library_state.scan_progress_tracks = 0;
        self.library_state.scan_progress_albums = 0;
        // Toast removed to avoid clutter
        // self.ui_state.toast_message = Some(ToastMessage::info("Full library rescan..."));

        // Start background scanner with force=true
        let directories: Vec<std::path::PathBuf> = self
            .library_state
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
        if let Err(e) = self.replay_gain_manager.start_scan() {
            self.ui_state.toast_message = Some(ToastMessage::error(format!(
                "Failed to start ReplayGain scan: {}",
                e
            )));
        }
    }

    /// Scan for Bliss audio analysis (tempo, features for similarity)
    pub fn scan_bliss(&mut self) {
        if let Err(e) = self.bliss_manager.start_scan() {
            self.ui_state.toast_message = Some(ToastMessage::error(format!(
                "Failed to start bliss analysis scan: {}",
                e
            )));
        }
    }

    /// Compute waveforms for tracks
    pub fn compute_waveform(&mut self) {
        if let Err(e) = self.waveform_manager.start_scan() {
            self.ui_state.toast_message = Some(ToastMessage::error(format!(
                "Failed to start waveform analysis: {}",
                e
            )));
        }
    }

    /// Clean up database by removing tracks for files that no longer exist
    pub fn clean_database(&mut self) {
        match self.library_state.library.clean_database() {
            Ok(removed) => {
                if removed > 0 {
                    self.ui_state.toast_message = Some(ToastMessage::success(format!(
                        "Removed {} missing tracks from database",
                        removed
                    )));
                } else {
                    self.ui_state.toast_message =
                        Some(ToastMessage::info("No missing tracks found in database"));
                }
            }
            Err(e) => {
                self.ui_state.toast_message = Some(ToastMessage::error(format!(
                    "Failed to clean database: {}",
                    e
                )));
            }
        }
    }

    /// Remove the selected directory from the library
    /// This also cleans up the database and removes albums/tracks from that directory
    pub fn remove_selected_directory(&mut self) {
        // We need to map from tree index to actual directory index
        let tree_items = self.get_directory_tree_items();
        if let Some((path, level, _)) = tree_items.get(self.selected_directory_index) {
            // Only allow removing level 0 directories (main directories, not subdirectories)
            if *level == 0 {
                // Find the actual index in the directories vector
                if let Some(dir_index) = self
                    .library_state
                    .library
                    .directories
                    .iter()
                    .position(|d| d.path == *path)
                {
                    if self.library_state.library.remove_directory(dir_index).is_some() {
                        // Adjust selected_directory_index if needed
                        let tree_items = self.get_directory_tree_items();
                        if self.selected_directory_index >= tree_items.len()
                            && self.selected_directory_index > 0
                        {
                            self.selected_directory_index = tree_items.len() - 1;
                        }
                        // Database cleanup is now done in library.remove_directory()
                        // No rescan needed since data is already cleaned
                        self.ui_state.toast_message =
                            Some(ToastMessage::success("Directory removed and cleaned up."));
                    }
                }
            } else {
                self.ui_state.toast_message = Some(ToastMessage::error("Cannot remove subdirectory."));
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
            self.ui_state.toast_message = Some(ToastMessage::info("Cancelling scan..."));
        }
    }

    /// Scan the library with progress tracking
    pub fn scan_library(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.library_state.scan_in_progress {
            return Ok(());
        }

        self.library_state.scan_in_progress = true;
        self.library_state.scan_progress_tracks = 0;
        self.library_state.scan_progress_albums = 0;
        // Toast removed to avoid clutter, progress is shown in UI
        // self.ui_state.toast_message = Some(ToastMessage::info("Scanning library..."));

        // Start background scanner
        let directories: Vec<std::path::PathBuf> = self
            .library_state
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
                        self.library_state.scan_progress_tracks = tracks;
                        self.library_state.scan_progress_albums = albums;
                    }
                    sotf_audio_player::LibraryScanMessage::Complete { tracks, albums } => {
                        self.library_state.scan_progress_tracks = tracks;
                        self.library_state.scan_progress_albums = albums;
                        self.ui_state.toast_message = Some(ToastMessage::success(format!(
                            "Scan complete. Found {} tracks in {} albums.",
                            tracks, albums
                        )));
                        done = true;
                        reload_needed = true;
                    }
                    sotf_audio_player::LibraryScanMessage::Error { message } => {
                        log::error!("Library scan failed: {}", message);
                        self.ui_state.toast_message =
                            Some(ToastMessage::error(format!("Scan failed: {}", message)));
                        done = true;
                    }
                }
            }

            if done {
                self.library_scanner = None;
                self.library_state.scan_in_progress = false;
                self.needs_rescan = false;
            }
        }

        if reload_needed {
            // Reload library to show new items
            if let Err(e) = self.load_library_from_database() {
                log::error!("Failed to reload library after scan: {}", e);
                self.ui_state.toast_message = Some(ToastMessage::error(
                    "Scan complete but failed to reload library.",
                ));
            }
        }
    }

    /// Get flattened directory tree for display
    pub fn get_directory_tree_items(&self) -> Vec<(PathBuf, usize, bool)> {
        self.library_state.library.get_directory_tree_items()
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
                    .library_state
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
