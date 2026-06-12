use super::app_impl::App;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

impl App {
    /// Start library scan (non-blocking background scan)
    pub fn start_library_scan(&mut self) {
        if self.read_only {
            log::info!("Skipping library scan (read-only mode)");
            return;
        }
        if self.scan_in_progress {
            return; // Already scanning
        }

        // Collect directories to scan
        let directories: Vec<std::path::PathBuf> = self
            .library
            .directories
            .iter()
            .map(|d| d.path.clone())
            .collect();

        if directories.is_empty() {
            self.status_message = Some("No directories to scan".to_string());
            return;
        }

        // Start background scanner (with pause support during playback)
        let scanner = sotf_audio_player::LibraryScanner::start_with_pause(
            directories,
            Arc::clone(&self.scanner_pause_flag),
        );
        self.library_scanner = Some(scanner);

        self.scan_in_progress = true;
        self.scan_progress_tracks = 0;
        self.scan_progress_albums = 0;
        self.scanner_pause_override = true;
        self.status_message = Some("Starting library scan...".to_string());
        log::info!("Started background library scan");
    }

    /// Start force library scan (non-blocking background scan, rescans ALL files)
    ///
    /// Unlike `start_library_scan()`, this rescans all files regardless of modification time.
    /// ReplayGain values are preserved (not overwritten).
    pub fn start_force_library_scan(&mut self) {
        if self.read_only {
            log::info!("Skipping force library scan (read-only mode)");
            return;
        }
        if self.scan_in_progress {
            return; // Already scanning
        }

        // Collect directories to scan
        let directories: Vec<std::path::PathBuf> = self
            .library
            .directories
            .iter()
            .map(|d| d.path.clone())
            .collect();

        if directories.is_empty() {
            self.status_message = Some("No directories to scan".to_string());
            return;
        }

        // Start background scanner with force=true (with pause support during playback)
        let scanner = sotf_audio_player::LibraryScanner::start_force_with_pause(
            directories,
            Arc::clone(&self.scanner_pause_flag),
        );
        self.library_scanner = Some(scanner);

        self.scan_in_progress = true;
        self.scan_progress_tracks = 0;
        self.scan_progress_albums = 0;
        self.scanner_pause_override = true;
        self.status_message = Some("Starting FORCE library scan (all files)...".to_string());
        log::info!("Started FORCE background library scan");
    }

    /// Check progress of background library scan
    pub fn check_library_scan_progress(&mut self) {
        if !self.scan_in_progress {
            return;
        }

        // Drain messages one at a time instead of collecting them into a Vec.
        // This bounds memory usage when the UI thread falls behind the scanner.
        let scanner = match &self.library_scanner {
            Some(s) => s,
            None => return,
        };

        enum Completion {
            Complete,
            Error,
        }
        let mut completion = None;

        while let Some(msg) = scanner.try_recv() {
            use sotf_audio_player::LibraryScanMessage;

            match msg {
                LibraryScanMessage::Progress { tracks, albums, .. } => {
                    self.scan_progress_tracks = tracks;
                    self.scan_progress_albums = albums;
                    self.status_message = Some(format!(
                        "Scanning: {} tracks, {} albums found...",
                        tracks, albums
                    ));
                }
                LibraryScanMessage::Complete { tracks, albums } => {
                    self.scan_in_progress = false;
                    self.needs_rescan = false;
                    self.scan_progress_tracks = tracks;
                    self.scan_progress_albums = albums;
                    self.status_message = Some(format!(
                        "Scan complete: {} tracks in {} albums",
                        tracks, albums
                    ));
                    log::info!(
                        "Library scan complete: {} tracks in {} albums",
                        tracks,
                        albums
                    );
                    completion = Some(Completion::Complete);
                }
                LibraryScanMessage::Error { message } => {
                    self.scan_in_progress = false;
                    self.status_message = Some(format!("Scan failed: {}", message));
                    log::error!("Library scan failed: {}", message);
                    completion = Some(Completion::Error);
                }
            }
        }

        // Drop the scanner borrow before running completion side effects that
        // need to mutate other fields of `self`.
        if completion.is_some() {
            self.library_scanner = None;
        }

        match completion {
            Some(Completion::Complete) => {
                // Reload library from database to get the new data
                if let Err(e) = self.library.load_from_database() {
                    log::error!("Failed to reload library after scan: {}", e);
                }
                self.rebuild_artist_tree();
                self.request_filter_update();

                // Start background scans for new tracks.
                // Bliss scan will auto-start when waveform completes to avoid
                // excessive memory usage from concurrent full-file decodings.
                if let Err(e) = self.start_replay_gain_scan() {
                    log::warn!("Failed to start replay gain scan: {}", e);
                }
                if let Err(e) = self.start_waveform_scan() {
                    log::warn!("Failed to start waveform scan: {}", e);
                }
                self.clear_pause_override_if_idle();
            }
            Some(Completion::Error) => {
                self.clear_pause_override_if_idle();
            }
            None => {}
        }
    }

    pub fn scan_library(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.scan_in_progress = true;
        self.scan_progress_tracks = 0;
        self.scan_progress_albums = 0;
        self.status_message = Some("Scanning library...".to_string());

        // Create shared progress state using atomics to avoid locking on every
        // progress tick.
        let progress_tracks = Arc::new(AtomicUsize::new(0));
        let progress_albums = Arc::new(AtomicUsize::new(0));
        let last_update_tracks = Arc::new(AtomicUsize::new(0));

        let progress_tracks_clone = Arc::clone(&progress_tracks);
        let progress_albums_clone = Arc::clone(&progress_albums);
        let last_update_clone = Arc::clone(&last_update_tracks);

        // Use progress callback to update shared progress
        let result = self.library.scan_with_progress(move |tracks, albums| {
            let last = last_update_clone.load(Ordering::Relaxed);
            let should_update = tracks.saturating_sub(last) >= 1000 || tracks == 0;

            if should_update {
                progress_tracks_clone.store(tracks, Ordering::Relaxed);
                progress_albums_clone.store(albums, Ordering::Relaxed);
                last_update_clone.store(tracks, Ordering::Relaxed);
                log::info!("Scan progress: {} tracks, {} albums found", tracks, albums);
            }
        });

        // Update app state with final progress
        self.scan_progress_tracks = progress_tracks.load(Ordering::Relaxed);
        self.scan_progress_albums = progress_albums.load(Ordering::Relaxed);

        self.scan_in_progress = false;
        self.needs_rescan = false;
        self.selected_album_index = 0;
        self.album_list_offset = 0;

        match &result {
            Ok(_) => {
                let album_count = self.library.albums.len();
                let track_count: usize = self.library.albums.iter().map(|a| a.tracks.len()).sum();
                self.status_message = Some(format!(
                    "Scan complete: {} tracks in {} albums",
                    track_count, album_count
                ));
                log::info!(
                    "Scan complete: {} tracks in {} albums",
                    track_count,
                    album_count
                );
            }
            Err(e) => {
                self.status_message = Some(format!("Scan failed: {}", e));
                log::error!("Scan failed: {}", e);
            }
        }

        self.rebuild_artist_tree();

        // Start background waveform scan for new tracks
        if result.is_ok()
            && let Err(e) = self.start_waveform_scan()
        {
            log::warn!("Failed to start waveform scan: {}", e);
        }

        result
    }
    /// Start background ReplayGain analysis for tracks without gain data
    pub fn start_replay_gain_scan(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.read_only {
            log::info!("Skipping replay gain scan (read-only mode)");
            return Ok(());
        }
        let msg = self.replay_gain_manager.start_scan()?;
        if self.replay_gain_manager.in_progress {
            self.scanner_pause_override = true;
        }
        self.status_message = Some(msg);
        Ok(())
    }

    pub fn start_force_replay_gain_scan(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.read_only {
            log::info!("Skipping force replay gain scan (read-only mode)");
            return Ok(());
        }
        let msg = self.replay_gain_manager.start_force_scan()?;
        if self.replay_gain_manager.in_progress {
            self.scanner_pause_override = true;
        }
        self.status_message = Some(msg);
        Ok(())
    }

    /// Check for ReplayGain scanner progress updates
    pub fn check_replay_gain_progress(&mut self) {
        if !self.replay_gain_manager.in_progress {
            return;
        }

        let just_completed = self.replay_gain_manager.update();

        if just_completed {
            self.status_message = Some(format!(
                "ReplayGain scan complete: {}/{} succeeded, {} failed",
                self.replay_gain_manager.succeeded,
                self.replay_gain_manager.total,
                self.replay_gain_manager.failed
            ));

            // Reload library so in-memory tracks get the new gain values
            if let Err(e) = self.library.load_from_database() {
                log::error!("Failed to reload library after ReplayGain scan: {}", e);
            }
            self.rebuild_artist_tree();
            self.request_filter_update();
            self.refresh_queue_metadata();
            self.clear_pause_override_if_idle();
        }
    }

    /// Clear the pause override once no user-initiated scans are running.
    fn clear_pause_override_if_idle(&mut self) {
        if !self.scan_in_progress
            && !self.replay_gain_manager.in_progress
            && !self.waveform_manager.in_progress
            && !self.bliss_manager.in_progress
        {
            self.scanner_pause_override = false;
        }
    }

    /// Start background waveform scanning for tracks without waveform data
    pub fn start_waveform_scan(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.read_only {
            log::info!("Skipping waveform scan (read-only mode)");
            return Ok(());
        }
        self.waveform_manager.start_scan()
    }

    pub fn start_force_waveform_scan(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.read_only {
            log::info!("Skipping force waveform scan (read-only mode)");
            return Ok(());
        }
        self.waveform_manager.start_force_scan()?;
        if self.waveform_manager.in_progress {
            self.scanner_pause_override = true;
        }
        self.status_message = Some("Force waveform rescan started...".to_string());
        Ok(())
    }

    /// Check progress of waveform scan
    pub fn check_waveform_progress(&mut self) {
        if !self.waveform_manager.in_progress {
            return;
        }
        let was_in_progress = self.waveform_manager.in_progress;
        self.waveform_manager.update();

        if was_in_progress && !self.waveform_manager.in_progress {
            // Reload library so in-memory tracks get waveform data
            if let Err(e) = self.library.load_from_database() {
                log::error!("Failed to reload library after waveform scan: {}", e);
            }
            self.rebuild_artist_tree();
            self.request_filter_update();
            self.refresh_queue_metadata();

            // Start bliss scan now that waveform is complete.
            // This serializes the two scans to avoid excessive memory usage
            // from concurrent full-file decodings.
            if let Err(e) = self.start_bliss_scan() {
                log::warn!("Failed to start bliss scan after waveform: {}", e);
            }

            self.clear_pause_override_if_idle();
        }
    }

    pub fn start_force_bliss_scan(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.read_only {
            log::info!("Skipping force bliss scan (read-only mode)");
            return Ok(());
        }
        let msg = self.bliss_manager.start_force_scan()?;
        if self.bliss_manager.in_progress {
            self.scanner_pause_override = true;
        }
        self.status_message = Some(msg);
        Ok(())
    }

    /// Start background bliss audio analysis for tracks without bliss data
    pub fn start_bliss_scan(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.read_only {
            log::info!("Skipping bliss scan (read-only mode)");
            return Ok(());
        }
        let msg = self.bliss_manager.start_scan()?;
        if self.bliss_manager.in_progress {
            self.scanner_pause_override = true;
        }
        self.status_message = Some(msg);
        Ok(())
    }

    /// Check progress of bliss scan
    pub fn check_bliss_progress(&mut self) {
        if !self.bliss_manager.in_progress {
            return;
        }
        let was_in_progress = self.bliss_manager.in_progress;
        self.bliss_manager.update();

        if was_in_progress && !self.bliss_manager.in_progress {
            log::info!(
                "Bliss scan complete: {}/{} succeeded, {} failed",
                self.bliss_manager.succeeded,
                self.bliss_manager.total,
                self.bliss_manager.failed
            );
            self.status_message = Some(format!(
                "Bliss scan complete: {}/{} succeeded, {} failed",
                self.bliss_manager.succeeded, self.bliss_manager.total, self.bliss_manager.failed
            ));
            self.clear_pause_override_if_idle();
        }
    }

    /// Refresh metadata (replay gain, waveform, etc.) on queued tracks
    /// from the freshly reloaded library.
    fn refresh_queue_metadata(&mut self) {
        for entry in &mut self.queue {
            let album = &mut entry.item.album;
            // Find the matching library album by id
            let lib_album = album
                .id
                .and_then(|id| self.library.albums.iter().find(|a| a.id == Some(id)));
            if let Some(lib_album) = lib_album {
                for track in &mut album.tracks {
                    if let Some(lib_track) = lib_album.tracks.iter().find(|t| t.path == track.path)
                    {
                        track.replay_gain = lib_track.replay_gain;
                        track.replay_peak = lib_track.replay_peak;
                        track.album_gain = lib_track.album_gain;
                        track.album_peak = lib_track.album_peak;
                        track.waveform = lib_track.waveform.clone();
                    }
                }
            }
        }
    }
}
