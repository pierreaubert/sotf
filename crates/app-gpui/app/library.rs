//! Library management methods.
//!
//! Thin UI layer that delegates to `LibraryController` via `self.library_state`.

use std::path::PathBuf;

use sotf_audio_player::Album;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use sotf_audio_player::config::install_authorized_runtime_plugin_sandbox;

use super::state::App;
use super::state::library::{ChannelFilter, LibrarySortOrder};
use super::types::ToastMessage;

impl App {
    /// Get filtered and sorted albums (with selection filters applied).
    pub fn filtered_albums(&self) -> Vec<&Album> {
        self.library_state.selection_filtered_albums()
    }

    /// Set library sort order (delegates to controller, then resets page).
    pub fn set_library_sort_order(&mut self, order: LibrarySortOrder) {
        self.library_state.set_sort_order(order);
        self.reset_page();
    }

    /// Set channel filter (delegates to controller, then resets page).
    pub fn set_channel_filter(&mut self, filter: ChannelFilter) {
        self.library_state.set_filter(filter);
        self.reset_page();
    }

    /// Cycle to next channel filter (delegates to controller, then resets page).
    pub fn cycle_channel_filter(&mut self) {
        self.library_state.cycle_filter();
        self.reset_page();
    }

    /// Toggle the favorites-only filter.
    pub fn toggle_favorites_filter(&mut self) {
        self.library_state.toggle_favorites_filter();
        self.reset_page();
    }

    /// Get paginated albums for grid view
    pub fn get_paginated_albums(&self) -> Vec<&Album> {
        let all_albums = self.filtered_albums();
        if all_albums.is_empty() {
            return Vec::new();
        }
        let end = self.library_state.items_per_page.min(all_albums.len());
        all_albums[0..end].to_vec()
    }

    /// Reset to first page
    pub fn reset_page(&mut self) {
        self.library_state.current_page = 0;
    }

    /// Load more albums (infinite scroll)
    pub fn load_more_albums(&mut self) {
        let total = self.filtered_albums().len();
        if self.library_state.items_per_page < total {
            let more = self.library_state.library_columns * 5;
            self.library_state.items_per_page =
                (self.library_state.items_per_page + more).min(total);
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
                    self.ui_state.toast_message =
                        Some(ToastMessage::warning("Directory already exists."));
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
        self.install_external_plugin_runtime_sandbox();
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub fn install_external_plugin_runtime_sandbox(&mut self) {
        let directories = self.external_plugin_media_directories();
        match install_authorized_runtime_plugin_sandbox(directories) {
            Ok(_) => {}
            Err(err) => {
                log::warn!("Failed to install external plugin runtime sandbox policy: {err}");
            }
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    pub fn install_external_plugin_runtime_sandbox(&mut self) {}

    pub fn external_plugin_media_directories(&self) -> Vec<PathBuf> {
        self.library_state
            .library
            .directories
            .iter()
            .map(|dir| dir.path.clone())
            .collect()
    }

    /// Full rescan of the library
    pub fn rescan_library(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.library_state.scan_in_progress {
            return Ok(());
        }

        self.library_state.scan_in_progress = true;
        self.library_state.scan_progress_tracks = 0;
        self.library_state.scan_progress_albums = 0;
        self.scan_total_files = 0;
        self.scan_started_at = Some(std::time::Instant::now());
        self.scan_progress_elapsed_secs = 0;
        self.scan_progress_eta_secs = None;
        self.scan_progress_tracks_per_sec = 0.0;
        self.scan_progress_phase = "Starting".to_string();
        self.scan_status_hidden = false;

        let directories: Vec<std::path::PathBuf> = self
            .library_state
            .library
            .directories
            .iter()
            .map(|d| d.path.clone())
            .collect();
        self.install_external_plugin_runtime_sandbox();
        self.library_scanner = Some(sotf_audio_player::LibraryScanner::start_force(directories));

        Ok(())
    }

    /// Scan for ReplayGain
    pub fn scan_replay_gain(&mut self) {
        if let Err(e) = self.scan_ctrl.replay_gain_manager.start_scan() {
            self.ui_state.toast_message = Some(ToastMessage::error(format!(
                "Failed to start ReplayGain scan: {}",
                e
            )));
        } else if self.scan_ctrl.replay_gain_manager.in_progress {
            self.scan_status_hidden = false;
        }
    }

    /// Scan for Bliss audio analysis (tempo, features for similarity)
    pub fn scan_bliss(&mut self) {
        if let Err(e) = self.scan_ctrl.bliss_manager.start_scan() {
            self.ui_state.toast_message = Some(ToastMessage::error(format!(
                "Failed to start bliss analysis scan: {}",
                e
            )));
        } else if self.scan_ctrl.bliss_manager.in_progress {
            self.scan_status_hidden = false;
        }
    }

    /// Compute waveforms for tracks
    pub fn compute_waveform(&mut self) {
        if let Err(e) = self.scan_ctrl.waveform_manager.start_scan() {
            self.ui_state.toast_message = Some(ToastMessage::error(format!(
                "Failed to start waveform analysis: {}",
                e
            )));
        } else if self.scan_ctrl.waveform_manager.in_progress {
            self.scan_status_hidden = false;
        }
    }

    /// Clean up database by removing tracks for files that no longer exist
    pub fn clean_database(&mut self) {
        match self.library_state.clean_database() {
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

    /// Clear all local album/track data from the app database.
    pub fn clear_local_library(&mut self) {
        match self.library_state.clear_library_content() {
            Ok(removed) => {
                self.selected_directory_index = 0;
                self.needs_rescan = false;
                self.install_external_plugin_runtime_sandbox();
                self.ui_state.toast_message = Some(ToastMessage::success(format!(
                    "Cleared local library data ({} tracks removed)",
                    removed
                )));
            }
            Err(e) => {
                self.ui_state.toast_message = Some(ToastMessage::error(format!(
                    "Failed to clear local library data: {}",
                    e
                )));
            }
        }
    }

    /// Remove the selected directory from the library
    pub fn remove_selected_directory(&mut self) {
        let tree_items = self.get_directory_tree_items();
        if let Some((path, level, _)) = tree_items.get(self.selected_directory_index) {
            if *level == 0 {
                if let Some(dir_index) = self
                    .library_state
                    .library
                    .directories
                    .iter()
                    .position(|d| d.path == *path)
                    && self.library_state.remove_directory(dir_index).is_some()
                {
                    let tree_items = self.get_directory_tree_items();
                    if self.selected_directory_index >= tree_items.len()
                        && self.selected_directory_index > 0
                    {
                        self.selected_directory_index = tree_items.len() - 1;
                    }
                    self.ui_state.toast_message =
                        Some(ToastMessage::success("Directory removed and cleaned up."));
                    self.install_external_plugin_runtime_sandbox();
                }
            } else {
                self.ui_state.toast_message =
                    Some(ToastMessage::error("Cannot remove subdirectory."));
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
        self.scan_total_files = 0;
        self.scan_started_at = Some(std::time::Instant::now());
        self.scan_progress_elapsed_secs = 0;
        self.scan_progress_eta_secs = None;
        self.scan_progress_tracks_per_sec = 0.0;
        self.scan_progress_phase = "Starting".to_string();
        self.scan_status_hidden = false;

        let directories: Vec<std::path::PathBuf> = self
            .library_state
            .library
            .directories
            .iter()
            .map(|d| d.path.clone())
            .collect();
        self.install_external_plugin_runtime_sandbox();
        self.library_scanner = Some(sotf_audio_player::LibraryScanner::start(directories));

        Ok(())
    }

    /// Update library scan progress
    pub fn update_library_scan(&mut self) {
        let mut reload_needed = false;

        if let Some(scanner) = self.library_scanner.take() {
            let mut done = false;
            while let Some(msg) = scanner.try_recv() {
                match msg {
                    sotf_audio_player::LibraryScanMessage::Progress {
                        tracks,
                        albums,
                        total_files,
                        phase,
                    } => {
                        self.library_state.scan_progress_tracks = tracks;
                        self.library_state.scan_progress_albums = albums;
                        self.scan_total_files = total_files;
                        self.scan_progress_phase = phase.to_string();
                        self.update_library_scan_timing();
                    }
                    sotf_audio_player::LibraryScanMessage::Complete { tracks, albums } => {
                        self.library_state.scan_progress_tracks = tracks;
                        self.library_state.scan_progress_albums = albums;
                        if self.scan_total_files == 0 {
                            self.scan_total_files = tracks;
                        }
                        self.update_library_scan_timing();
                        self.scan_progress_eta_secs = Some(0);
                        self.scan_progress_phase = "Complete".to_string();
                        self.ui_state.toast_message = Some(ToastMessage::success(format!(
                            "Scan complete. Library now has {} tracks in {} albums.",
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
                self.library_state.scan_in_progress = false;
                self.needs_rescan = false;
                self.scan_started_at = None;
            } else {
                self.library_scanner = Some(scanner);
            }
        }

        if self.library_state.scan_in_progress {
            self.update_library_scan_timing();
        }

        if reload_needed && let Err(e) = self.load_library_from_database() {
            log::error!("Failed to reload library after scan: {}", e);
            self.ui_state.toast_message = Some(ToastMessage::error(
                "Scan complete but failed to reload library.",
            ));
        }
    }

    fn update_library_scan_timing(&mut self) {
        let Some(started_at) = self.scan_started_at else {
            return;
        };
        let elapsed = started_at.elapsed().as_secs();
        self.scan_progress_elapsed_secs = elapsed;

        if elapsed == 0 {
            self.scan_progress_tracks_per_sec = 0.0;
            self.scan_progress_eta_secs = None;
            return;
        }

        let tracks = self.library_state.scan_progress_tracks;
        self.scan_progress_tracks_per_sec = tracks as f32 / elapsed as f32;
        self.scan_progress_eta_secs =
            if self.scan_total_files > tracks && self.scan_progress_tracks_per_sec > 0.0 {
                Some(
                    ((self.scan_total_files - tracks) as f32 / self.scan_progress_tracks_per_sec)
                        as u64,
                )
            } else {
                None
            };
    }

    /// Get flattened directory tree for display
    pub fn get_directory_tree_items(&self) -> Vec<(PathBuf, usize, bool)> {
        self.library_state.library.get_directory_tree_items()
    }

    /// Toggle directory expansion (lazily loads children on first expand)
    pub fn toggle_directory_expansion(&mut self) {
        let tree_items = self.get_directory_tree_items();
        if let Some((path, _level, _)) = tree_items.get(self.selected_directory_index) {
            self.library_state.library.toggle_directory_expanded(path);
        }
    }
}
