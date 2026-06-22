use super::super::app_impl::App;
use super::super::types::{
    ArtistNode, CastDeviceInfo, ChannelFilter, LibrarySortOrder, LibraryViewMode, QueueEntry,
    QueueItem, TreeItem,
};
use sotf_audio::decoder::AudioSource;
use sotf_audio::devices::AudioDevice;
use sotf_audio_player::{Album, QueuePlaybackEffect};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

impl App {
    /// Load library from database if available
    pub fn load_library_from_database(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.library.load_from_database()?;
        self.rebuild_artist_tree();
        // Update last scan times for directories from database
        self.update_directory_scan_times();
        Ok(())
    }

    /// Update directory scan times from database
    pub fn update_directory_scan_times(&mut self) {
        self.library.update_directory_scan_times();
    }

    /// Load all audio devices (output + recording) in a single cpal enumeration.
    /// Previously this called get_audio_devices() twice (once for output, once for recording).
    pub fn load_all_audio_devices(&mut self) {
        let t0 = std::time::Instant::now();

        let Ok(devices_map) = sotf_audio::devices::get_audio_devices() else {
            log::warn!("[startup] Failed to enumerate audio devices");
            return;
        };

        log::info!(
            "[startup] Audio device enumeration: {:.1}ms",
            t0.elapsed().as_secs_f64() * 1000.0
        );

        // Output devices
        if let Some(output_devices) = devices_map.get("output") {
            self.audio_devices.outputs = output_devices.clone();
            if !self.audio_devices.outputs.is_empty() {
                self.audio_devices.selected_output_index = self
                    .audio_devices
                    .outputs
                    .iter()
                    .position(|d| d.is_default)
                    .unwrap_or(0);
                self.audio_devices.current_output_name = self.audio_devices.outputs
                    [self.audio_devices.selected_output_index]
                    .name
                    .clone()
                    .into();
            }

            // Recording playback devices (reuse same output list)
            self.recording.available_playback_devices = output_devices
                .iter()
                .map(|d| (d.device_id.clone().unwrap_or_default(), d.name.clone()))
                .collect();
            if let Some(default_idx) = output_devices.iter().position(|d| d.is_default) {
                self.recording.selected_playback_idx = default_idx;
            }
        }

        // Recording input devices
        if let Some(input_devices) = devices_map.get("input") {
            self.recording.available_recording_devices = input_devices
                .iter()
                .map(|d| (d.device_id.clone().unwrap_or_default(), d.name.clone()))
                .collect();
            if let Some(default_idx) = input_devices.iter().position(|d| d.is_default) {
                self.recording.selected_recording_idx = default_idx;
            }
        }

        // Populate device_name/device_id fields from selected devices
        if !self.recording.available_playback_devices.is_empty() {
            let (id, name) = self.recording.available_playback_devices
                [self.recording.selected_playback_idx]
                .clone();
            self.recording.model.playback_config.device_name = name;
            self.recording.model.playback_config.device_id = id;
        }
        if !self.recording.available_recording_devices.is_empty() {
            let (id, name) = self.recording.available_recording_devices
                [self.recording.selected_recording_idx]
                .clone();
            self.recording.model.recording_config.device_name = name;
            self.recording.model.recording_config.device_id = id;
        }
    }

    pub fn load_output_devices(&mut self) {
        // Load available output devices
        if let Ok(devices_map) = sotf_audio::devices::get_audio_devices()
            && let Some(output_devices) = devices_map.get("output")
        {
            self.audio_devices.outputs = output_devices.clone();
            if !self.audio_devices.outputs.is_empty() {
                self.audio_devices.selected_output_index = self
                    .audio_devices
                    .outputs
                    .iter()
                    .position(|d| d.is_default)
                    .unwrap_or(0);
                self.audio_devices.current_output_name = self.audio_devices.outputs
                    [self.audio_devices.selected_output_index]
                    .name
                    .clone()
                    .into();
            }
        }
    }

    /// Spawn a background mDNS scan for AirPlay + Chromecast receivers.
    /// Idempotent: returns immediately if a scan is already in flight.
    pub fn start_cast_discovery(&mut self) {
        if self.audio_devices.cast_discovery_running {
            return;
        }
        self.audio_devices.cast_discovery_running = true;

        let (tx, rx) = std::sync::mpsc::channel();
        self.audio_devices.cast_discovery_receiver = Some(rx);

        std::thread::Builder::new()
            .name("tui-cast-discovery".into())
            .spawn(move || {
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(e) => {
                        log::warn!("Cast discovery: failed to create tokio runtime: {e}");
                        let _ = tx.send(Vec::new());
                        return;
                    }
                };
                let devices = rt.block_on(async {
                    let timeout = std::time::Duration::from_secs(3);
                    match sotf_cast::CastDiscovery::discover_all(timeout).await {
                        Ok(devices) => devices
                            .into_iter()
                            .map(|d| CastDeviceInfo {
                                name: d.name.clone(),
                                device_type: match d.device_type {
                                    sotf_cast::CastDeviceType::AirPlay => "AirPlay".to_string(),
                                    sotf_cast::CastDeviceType::Chromecast => {
                                        "Chromecast".to_string()
                                    }
                                },
                                address: d.address.to_string(),
                                port: d.port,
                            })
                            .collect(),
                        Err(e) => {
                            log::warn!("Cast discovery failed: {e}");
                            Vec::new()
                        }
                    }
                });
                let _ = tx.send(devices);
            })
            .expect("spawn cast discovery thread");
    }

    /// Drain the cast-discovery channel; returns true if state changed and the
    /// devices screen should redraw.
    pub fn poll_cast_discovery(&mut self) -> bool {
        let rx = match &self.audio_devices.cast_discovery_receiver {
            Some(rx) => rx,
            None => return false,
        };
        match rx.try_recv() {
            Ok(devices) => {
                log::info!("Cast discovery found {} device(s)", devices.len());
                self.audio_devices.cast = devices;
                self.audio_devices.cast_discovery_running = false;
                self.audio_devices.cast_discovery_receiver = None;
                true
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.audio_devices.cast_discovery_running = false;
                self.audio_devices.cast_discovery_receiver = None;
                true
            }
        }
    }

    /// Reload local output devices and kick off a cast-device rescan.
    pub fn reload_all_devices(&mut self) {
        self.load_output_devices();
        self.start_cast_discovery();
    }

    pub fn load_recording_devices(&mut self) {
        if let Ok(devices_map) = sotf_audio::devices::get_audio_devices() {
            if let Some(output_devices) = devices_map.get("output") {
                self.recording.available_playback_devices = output_devices
                    .iter()
                    .map(|d| (d.device_id.clone().unwrap_or_default(), d.name.clone()))
                    .collect();
                if let Some(default_idx) = output_devices.iter().position(|d| d.is_default) {
                    self.recording.selected_playback_idx = default_idx;
                }
            }
            if let Some(input_devices) = devices_map.get("input") {
                self.recording.available_recording_devices = input_devices
                    .iter()
                    .map(|d| (d.device_id.clone().unwrap_or_default(), d.name.clone()))
                    .collect();
                if let Some(default_idx) = input_devices.iter().position(|d| d.is_default) {
                    self.recording.selected_recording_idx = default_idx;
                }
            }
        }
        // Populate device_name/device_id fields from selected devices
        if !self.recording.available_playback_devices.is_empty() {
            let (id, name) = self.recording.available_playback_devices
                [self.recording.selected_playback_idx]
                .clone();
            self.recording.model.playback_config.device_name = name;
            self.recording.model.playback_config.device_id = id;
        }
        if !self.recording.available_recording_devices.is_empty() {
            let (id, name) = self.recording.available_recording_devices
                [self.recording.selected_recording_idx]
                .clone();
            self.recording.model.recording_config.device_name = name;
            self.recording.model.recording_config.device_id = id;
        }
    }

    pub fn select_next_output_device(&mut self) {
        if !self.audio_devices.outputs.is_empty() {
            self.audio_devices.selected_output_index =
                (self.audio_devices.selected_output_index + 1) % self.audio_devices.outputs.len();
        }
    }

    pub fn select_previous_output_device(&mut self) {
        if !self.audio_devices.outputs.is_empty() {
            if self.audio_devices.selected_output_index == 0 {
                self.audio_devices.selected_output_index = self.audio_devices.outputs.len() - 1;
            } else {
                self.audio_devices.selected_output_index -= 1;
            }
        }
    }

    pub fn get_selected_output_device(&self) -> Option<&AudioDevice> {
        self.audio_devices
            .outputs
            .get(self.audio_devices.selected_output_index)
    }

    /// Get the maximum output channels supported by the selected device
    pub fn get_device_max_channels(&self) -> Option<usize> {
        self.get_selected_output_device().and_then(|device| {
            device
                .supported_configs
                .iter()
                .map(|config| config.channels as usize)
                .max()
                .or_else(|| {
                    device
                        .default_config
                        .as_ref()
                        .map(|config| config.channels as usize)
                })
        })
    }

    /// Get current device sample rate or fallback to 48kHz
    pub fn get_current_sample_rate(&self) -> f64 {
        self.get_selected_output_device()
            .and_then(|device| device.default_config.as_ref())
            .map(|config| config.sample_rate as f64)
            .unwrap_or(48000.0)
    }

    /// Get the target sample rate for a track, accounting for device capabilities
    pub fn get_target_sample_rate(&self, track_sample_rate: u32) -> f64 {
        sotf_audio::select_output_sample_rate(
            track_sample_rate,
            self.audio_devices.current_output_name.as_deref(),
        ) as f64
    }

    /// Get filtered albums, using cache if available
    pub fn filtered_albums(&mut self) -> &[Album] {
        if self.library_view.needs_filter_update {
            self.library_view.cached_filtered_albums = self.library.get_filtered_albums(
                &self.library_view.search_query,
                self.library_view.sort_order,
                self.library_view.channel_filter,
                self.library_view.show_favorites_only,
            );
            self.library_view.needs_filter_update = false;
        }
        &self.library_view.cached_filtered_albums
    }

    /// Mark filtered albums cache as dirty
    pub fn request_filter_update(&mut self) {
        self.library_view.needs_filter_update = true;
    }

    pub fn add_album_to_queue(
        &mut self,
    ) -> Result<Option<sotf_audio::decoder::AudioSource>, String> {
        // Use a local copy to avoid borrow issues while mutating queue
        let index = self.library_view.selected_album_index;
        let album = match self.filtered_albums().get(index) {
            Some(a) => a.clone(),
            None => return Ok(None),
        };

        // Validate at least one track has a playable source. Remote federation
        // tracks use URL sources and intentionally do not have local paths.
        if !album.tracks.is_empty()
            && !album
                .tracks
                .iter()
                .any(|t| !matches!(t.audio_source(), AudioSource::File(_)) || t.path.exists())
        {
            return Err(format!(
                "None of the files for \"{}\" exist on disk",
                album.title,
            ));
        }

        let artist = album.artist();
        let title = &album.title;
        let already_queued = self
            .queue
            .iter()
            .any(|e| e.item.album.artist() == artist && e.item.album.title == *title);
        if already_queued {
            return Ok(None);
        }

        self.queue.push(QueueEntry::new(QueueItem::new(album)));
        Ok(None)
    }

    pub fn remove_from_queue(&mut self, index: usize) -> QueuePlaybackEffect {
        if index >= self.queue.len() {
            return QueuePlaybackEffect::None;
        }

        let was_playing = self.playback.is_playing;
        let mut effect = QueuePlaybackEffect::None;
        self.queue.remove(index);

        // Adjust current queue index if needed
        if let Some(current_idx) = self.playback.current_queue_index {
            if current_idx == index {
                // We deleted the currently playing album
                if self.queue.is_empty() {
                    self.playback.current_queue_index = None;
                    self.playback.is_playing = false;
                    effect = QueuePlaybackEffect::Stop;
                } else if index < self.queue.len() {
                    // There are albums after the deleted one, stay at same index
                    // (items have shifted down, so index now points to the next album)
                    self.playback.current_queue_index = Some(index);
                    // Reset to first track of the new album at this position
                    if let Some(entry) = self.queue.get_mut(index) {
                        entry.item.current_track_index = 0;
                    }
                    if was_playing && let Some(source) = self.current_track_source() {
                        effect = QueuePlaybackEffect::Reload(source);
                    }
                } else if index > 0 {
                    // Deleted last album, move to previous album
                    self.playback.current_queue_index = Some(index - 1);
                    if was_playing && let Some(source) = self.current_track_source() {
                        effect = QueuePlaybackEffect::Reload(source);
                    }
                } else {
                    self.playback.current_queue_index = None;
                    self.playback.is_playing = false;
                    effect = QueuePlaybackEffect::Stop;
                }
            } else if current_idx > index {
                // Deleted an album before the current one, adjust index
                self.playback.current_queue_index = Some(current_idx - 1);
            }
        }
        if self.queue_view.selected_index >= self.queue.len() && self.queue_view.selected_index > 0
        {
            self.queue_view.selected_index = self.queue.len() - 1;
        }
        self.queue_view.selected_track_index = None;
        effect
    }

    pub fn clear_queue(&mut self) {
        self.queue.clear();
        self.playback.current_queue_index = None;
        self.queue_view.selected_index = 0;
        self.queue_view.selected_track_index = None;
        self.playback.is_playing = false;
    }

    pub fn toggle_queue_item_expansion(&mut self) {
        if let Some(entry) = self.queue.get_mut(self.queue_view.selected_index) {
            entry.expanded = !entry.expanded;
            if !entry.expanded {
                self.queue_view.selected_track_index = None;
            }
        }
    }

    pub fn expand_queue_item(&mut self) {
        if let Some(entry) = self.queue.get_mut(self.queue_view.selected_index) {
            entry.expanded = true;
        }
    }

    pub fn collapse_queue_item(&mut self) {
        if self.queue_view.selected_track_index.is_some() {
            // On a track: move back to album header
            self.queue_view.selected_track_index = None;
        } else if let Some(entry) = self.queue.get_mut(self.queue_view.selected_index) {
            entry.expanded = false;
        }
    }

    pub fn select_next_album(&mut self) {
        let count = self.filtered_albums().len();
        if count > 0 {
            self.library_view.selected_album_index =
                (self.library_view.selected_album_index + 1) % count;
        }
    }

    pub fn select_previous_album(&mut self) {
        let count = self.filtered_albums().len();
        if count > 0 {
            if self.library_view.selected_album_index == 0 {
                self.library_view.selected_album_index = count - 1;
            } else {
                self.library_view.selected_album_index -= 1;
            }
        }
    }

    pub fn page_down_albums(&mut self, page_size: usize) {
        let count = self.filtered_albums().len();
        if count > 0 {
            self.library_view.selected_album_index =
                (self.library_view.selected_album_index + page_size).min(count - 1);
        }
    }

    pub fn page_up_albums(&mut self, page_size: usize) {
        let count = self.filtered_albums().len();
        if count > 0 {
            self.library_view.selected_album_index = self
                .library_view
                .selected_album_index
                .saturating_sub(page_size);
        }
    }

    pub fn page_down_tree(&mut self, page_size: usize) {
        if self.library_view.mode != LibraryViewMode::TreeView {
            return;
        }

        let tree_items = self.get_tree_items();
        if !tree_items.is_empty() {
            self.library_view.selected_tree_index =
                (self.library_view.selected_tree_index + page_size).min(tree_items.len() - 1);
        }
    }

    pub fn page_up_tree(&mut self, page_size: usize) {
        if self.library_view.mode != LibraryViewMode::TreeView {
            return;
        }

        let tree_items = self.get_tree_items();
        if !tree_items.is_empty() {
            self.library_view.selected_tree_index = self
                .library_view
                .selected_tree_index
                .saturating_sub(page_size);
        }
    }

    pub fn select_next_directory(&mut self) {
        let tree_items = self.get_directory_tree_items();
        if !tree_items.is_empty() {
            self.library_view.selected_directory_index =
                (self.library_view.selected_directory_index + 1) % tree_items.len();
        }
    }

    pub fn select_previous_directory(&mut self) {
        let tree_items = self.get_directory_tree_items();
        if !tree_items.is_empty() {
            if self.library_view.selected_directory_index == 0 {
                self.library_view.selected_directory_index = tree_items.len() - 1;
            } else {
                self.library_view.selected_directory_index -= 1;
            }
        }
    }

    pub fn page_down_directories(&mut self, page_size: usize) {
        let tree_items = self.get_directory_tree_items();
        if !tree_items.is_empty() {
            self.library_view.selected_directory_index =
                (self.library_view.selected_directory_index + page_size).min(tree_items.len() - 1);
        }
    }

    pub fn page_up_directories(&mut self, page_size: usize) {
        let tree_items = self.get_directory_tree_items();
        if !tree_items.is_empty() {
            self.library_view.selected_directory_index = self
                .library_view
                .selected_directory_index
                .saturating_sub(page_size);
        }
    }

    pub fn select_next_queue_item(&mut self) {
        if self.queue.is_empty() {
            return;
        }

        let entry = &self.queue[self.queue_view.selected_index];
        if entry.expanded {
            match self.queue_view.selected_track_index {
                None => {
                    // On album header of expanded album → move to first track
                    self.queue_view.selected_track_index = Some(0);
                }
                Some(ti) if ti + 1 < entry.item.album.tracks.len() => {
                    // Move to next track within album
                    self.queue_view.selected_track_index = Some(ti + 1);
                }
                Some(_) => {
                    // Past last track → move to next album header
                    self.queue_view.selected_track_index = None;
                    self.queue_view.selected_index =
                        (self.queue_view.selected_index + 1) % self.queue.len();
                }
            }
        } else {
            // Collapsed album → move to next album
            self.queue_view.selected_track_index = None;
            self.queue_view.selected_index =
                (self.queue_view.selected_index + 1) % self.queue.len();
        }
    }

    pub fn select_previous_queue_item(&mut self) {
        if self.queue.is_empty() {
            return;
        }

        match self.queue_view.selected_track_index {
            Some(0) => {
                // First track → move back to album header
                self.queue_view.selected_track_index = None;
            }
            Some(ti) => {
                // Move to previous track
                self.queue_view.selected_track_index = Some(ti - 1);
            }
            None => {
                // On album header → move to previous album
                if self.queue_view.selected_index == 0 {
                    self.queue_view.selected_index = self.queue.len() - 1;
                } else {
                    self.queue_view.selected_index -= 1;
                }
                // If the previous album is expanded, land on its last track
                let prev = &self.queue[self.queue_view.selected_index];
                if prev.expanded && !prev.item.album.tracks.is_empty() {
                    self.queue_view.selected_track_index = Some(prev.item.album.tracks.len() - 1);
                } else {
                    self.queue_view.selected_track_index = None;
                }
            }
        }
    }

    pub fn add_directory(&mut self, path: PathBuf) {
        match self.library.add_directory(path) {
            Ok(needs_scan) => {
                if needs_scan {
                    self.scan.needs_rescan = true;
                    self.ui.status_message =
                        Some("Directory added. Press 's' to scan.".to_string());
                } else {
                    self.ui.status_message = Some("Directory already exists.".to_string());
                }
            }
            Err(msg) => {
                self.ui.status_message = Some(msg);
            }
        }
    }

    /// Add a directory without triggering rescan (for startup initialization)
    pub fn add_directory_quiet(&mut self, path: PathBuf) {
        let _ = self.library.add_directory(path);
    }

    pub fn remove_selected_directory(&mut self) {
        if self
            .library
            .remove_directory(self.library_view.selected_directory_index)
            .is_some()
        {
            if self.library_view.selected_directory_index >= self.library.directories.len()
                && self.library_view.selected_directory_index > 0
            {
                self.library_view.selected_directory_index = self.library.directories.len() - 1;
            }
            // Reload library from database (tracks already removed by remove_directory)
            if let Err(e) = self.load_library_from_database() {
                log::warn!("Failed to reload library after directory removal: {}", e);
            }
        }
    }

    pub fn toggle_directory_expansion(&mut self) {
        let tree_items = self.get_directory_tree_items();
        if let Some((path, _, _)) = tree_items.get(self.library_view.selected_directory_index) {
            self.library.toggle_directory_expanded(path);
        }
    }

    /// Get flattened directory tree for display
    pub fn get_directory_tree_items(&self) -> Vec<(PathBuf, usize, bool)> {
        self.library.get_directory_tree_items()
    }

    pub fn clean_library_database(&mut self) -> Result<usize, Box<dyn std::error::Error>> {
        self.scan.maintenance_in_progress = true;
        self.scan.maintenance_progress_checked = 0;
        self.scan.maintenance_progress_total = 0;
        self.ui.status_message = Some("Starting database maintenance...".to_string());

        // Create shared progress state
        let progress_checked = Arc::new(Mutex::new(0usize));
        let progress_total = Arc::new(Mutex::new(0usize));

        let progress_checked_clone = Arc::clone(&progress_checked);
        let progress_total_clone = Arc::clone(&progress_total);

        // Use progress callback to update shared progress
        let result = self
            .library
            .clean_database_with_progress(move |checked, total| {
                if let Ok(mut pc) = progress_checked_clone.lock() {
                    *pc = checked;
                }
                if let Ok(mut pt) = progress_total_clone.lock() {
                    *pt = total;
                }
            });

        // Update app state with final progress
        if let Ok(pc) = progress_checked.lock() {
            self.scan.maintenance_progress_checked = *pc;
        }
        if let Ok(pt) = progress_total.lock() {
            self.scan.maintenance_progress_total = *pt;
        }

        self.scan.maintenance_in_progress = false;

        match &result {
            Ok(removed) => {
                if *removed > 0 {
                    self.ui.status_message =
                        Some(format!("Cleaned {} missing tracks from database", removed));
                    log::info!("Database maintenance: removed {} missing tracks", removed);
                } else {
                    self.ui.status_message =
                        Some("Database is clean - no missing tracks found".to_string());
                    log::info!("Database maintenance: no missing tracks found");
                }
            }
            Err(e) => {
                self.ui.status_message = Some(format!("Database maintenance failed: {}", e));
                log::error!("Database maintenance failed: {}", e);
            }
        }

        self.rebuild_artist_tree();

        result
    }

    /// Save current app state to config file
    pub fn save_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config = sotf_audio_player::config::AppConfig {
            version: 1,
            output_device: self.audio_devices.current_output_name.clone(),
            queue: self
                .queue
                .iter()
                .map(|entry| (entry.item.album.artist(), entry.item.album.title.clone()))
                .collect(),
            queue_index: self.playback.current_queue_index,
            track_index: self
                .playback
                .current_queue_index
                .and_then(|idx| self.queue.get(idx))
                .map(|entry| entry.item.current_track_index)
                .unwrap_or(0),
            plugin_preset: self.plugin_rack.last_loaded_preset.clone(),
        };

        sotf_audio_player::config::save_app_config(&config)?;
        log::info!("Saved app configuration");
        Ok(())
    }

    /// Load app state from config file and restore it
    pub fn load_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let config = sotf_audio_player::config::load_app_config()?;

        // Restore output device
        if let Some(device_name) = &config.output_device {
            self.audio_devices.current_output_name = Some(device_name.clone());
            // Find the device index
            if let Some(idx) = self
                .audio_devices
                .outputs
                .iter()
                .position(|d| d.name == *device_name)
            {
                self.audio_devices.selected_output_index = idx;
            }
        }

        // Restore queue - need to find albums by artist/title
        for (artist, title) in config.queue {
            if let Some(album) = self
                .library
                .albums
                .iter()
                .find(|a| a.artist() == artist && a.title == title)
                .cloned()
            {
                self.queue.push(QueueEntry::new(QueueItem::new(album)));
            }
        }

        // Restore queue position
        if let Some(queue_idx) = config.queue_index
            && queue_idx < self.queue.len()
        {
            self.playback.current_queue_index = Some(queue_idx);
            // Restore track position within album
            if let Some(entry) = self.queue.get_mut(queue_idx)
                && config.track_index < entry.item.album.tracks.len()
            {
                entry.item.current_track_index = config.track_index;
            }
        }

        // Restore plugin preset
        if let Some(preset_name) = &config.plugin_preset {
            // Use the plugin chain's own load method
            if let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() {
                match self
                    .plugin_rack
                    .graph
                    .load_from_file(&presets_dir, preset_name)
                {
                    Ok(warnings) => {
                        // Update BinauralDecoder input channels after loading
                        self.plugin_rack.graph.update_channel_dependent_plugins();

                        self.plugin_rack.last_loaded_preset = Some(preset_name.clone());
                        self.request_plugin_update();
                        if warnings.is_empty() {
                            log::info!("Restored plugin preset: {}", preset_name);
                        } else {
                            log::warn!(
                                "Restored preset '{}' with {} skipped plugin(s)",
                                preset_name,
                                warnings.len()
                            );
                            for w in &warnings {
                                log::warn!("  {}", w);
                            }
                            self.ui.status_message = Some(format!(
                                "Preset '{}': {} plugin(s) skipped",
                                preset_name,
                                warnings.len()
                            ));
                        }
                    }
                    Err(e) => {
                        log::warn!("Could not restore preset '{}': {}", preset_name, e);
                    }
                }
            }
        }

        log::info!(
            "Loaded app configuration: {} items in queue, device: {:?}, preset: {:?}",
            self.queue.len(),
            self.audio_devices.current_output_name,
            self.plugin_rack.last_loaded_preset
        );
        Ok(())
    }

    /// Build the artist tree from the current album list
    pub fn rebuild_artist_tree(&mut self) {
        use std::collections::HashMap;

        let mut artist_map: HashMap<String, Vec<usize>> = HashMap::new();

        // Group albums by artist
        for (idx, album) in self.library.albums.iter().enumerate() {
            artist_map.entry(album.artist()).or_default().push(idx);
        }

        // Create artist nodes
        let mut artists: Vec<_> = artist_map.into_iter().collect();
        artists.sort_by(|a, b| a.0.cmp(&b.0));

        self.library_view.artist_tree = artists
            .into_iter()
            .map(|(artist, album_indices)| ArtistNode {
                artist,
                album_indices,
                expanded: false,
            })
            .collect();

        self.library_view.selected_tree_index = 0;
    }

    /// Toggle tree view mode
    pub fn toggle_library_view_mode(&mut self) {
        self.library_view.mode = match self.library_view.mode {
            LibraryViewMode::Flat => LibraryViewMode::TreeView,
            LibraryViewMode::TreeView => LibraryViewMode::Flat,
        };
        self.library_view.selected_tree_index = 0;
    }

    /// Set library sort order
    pub fn set_library_sort_order(&mut self, order: LibrarySortOrder) {
        self.library_view.sort_order = order;
        // Reset selection to top when changing sort order
        self.library_view.selected_album_index = 0;
        self.library_view.selected_tree_index = 0;
        // Mark cache as dirty
        self.request_filter_update();
        // Rebuild tree view if active (as sort order affects tree structure)
        if self.library_view.mode == LibraryViewMode::TreeView {
            self.rebuild_artist_tree();
        }
    }

    /// Set channel filter
    pub fn set_channel_filter(&mut self, filter: ChannelFilter) {
        self.library_view.channel_filter = filter;
        // Reset selection to top when changing filter
        self.library_view.selected_album_index = 0;
        self.library_view.selected_tree_index = 0;
        // Mark cache as dirty
        self.request_filter_update();
        // Rebuild tree view if active
        if self.library_view.mode == LibraryViewMode::TreeView {
            self.rebuild_artist_tree();
        }
    }

    /// Get unique channel counts present in the library
    pub fn get_unique_channel_counts(&self) -> Vec<u32> {
        use std::collections::HashSet;

        let mut channel_counts = HashSet::new();

        for album in &self.library.albums {
            if let Some(count) = album.uniform_channel_count() {
                channel_counts.insert(count);
            }
        }

        let mut counts: Vec<u32> = channel_counts.into_iter().collect();
        counts.sort();
        counts
    }

    /// Cycle to next channel filter
    pub fn cycle_channel_filter(&mut self) {
        // Get available channel counts in library (excluding mono and stereo since they have their own filters)
        let specific_counts: Vec<u32> = self
            .get_unique_channel_counts()
            .into_iter()
            .filter(|&count| count > 2)
            .collect();

        self.library_view.channel_filter = match self.library_view.channel_filter {
            ChannelFilter::All => ChannelFilter::Mono,
            ChannelFilter::Mono => ChannelFilter::Stereo,
            ChannelFilter::Stereo => ChannelFilter::Surround,
            ChannelFilter::Surround => ChannelFilter::Surround71,
            ChannelFilter::Surround71 => ChannelFilter::SurroundPlus,
            ChannelFilter::SurroundPlus => ChannelFilter::Mixed,
            ChannelFilter::Mixed => {
                // Cycle to first specific count if available, otherwise back to All
                if let Some(&first_count) = specific_counts.first() {
                    ChannelFilter::Specific(first_count)
                } else {
                    ChannelFilter::All
                }
            }
            ChannelFilter::Specific(current) => {
                // Find next specific count in the list
                if let Some(pos) = specific_counts.iter().position(|&c| c == current) {
                    if pos + 1 < specific_counts.len() {
                        ChannelFilter::Specific(specific_counts[pos + 1])
                    } else {
                        ChannelFilter::All
                    }
                } else {
                    ChannelFilter::All
                }
            }
        };
        // Reset selection
        self.library_view.selected_album_index = 0;
        self.library_view.selected_tree_index = 0;
        // Mark cache as dirty
        self.request_filter_update();
        // Rebuild tree view if active
        if self.library_view.mode == LibraryViewMode::TreeView {
            self.rebuild_artist_tree();
        }
    }

    /// Toggle expansion of the currently selected artist node
    pub fn toggle_artist_expansion(&mut self) {
        if self.library_view.mode != LibraryViewMode::TreeView {
            return;
        }

        // Get the filtered tree items to find which artist we're on
        let tree_items = self.get_tree_items();
        if let Some(TreeItem::Artist { name, .. }) =
            tree_items.get(self.library_view.selected_tree_index)
        {
            // Find this artist in the tree and toggle expansion
            for artist_node in &mut self.library_view.artist_tree {
                if artist_node.artist == *name {
                    artist_node.expanded = !artist_node.expanded;
                    // Note: This doesn't change the set of albums, just visibility in tree
                    // so we don't necessarily need request_filter_update() here
                    // but we do need to rebuild the tree items display
                    return;
                }
            }
        }
    }

    /// Get the set of album indices that pass the current search and channel filters
    pub(in super::super) fn filtered_album_indices(&self) -> std::collections::HashSet<usize> {
        use sotf_audio_player::AlbumChannelType;
        use std::collections::HashSet;

        let mut indices: HashSet<usize> = if self.library_view.search_query.is_empty() {
            (0..self.library.albums.len()).collect()
        } else {
            // Get filtered albums and find their indices in the library
            let filtered = self.library.search_albums(&self.library_view.search_query);
            self.library
                .albums
                .iter()
                .enumerate()
                .filter(|(_, album)| filtered.iter().any(|a| std::ptr::eq(*a, *album)))
                .map(|(idx, _)| idx)
                .collect()
        };

        // Apply channel filter
        indices.retain(|&idx| {
            if let Some(album) = self.library.albums.get(idx) {
                match self.library_view.channel_filter {
                    ChannelFilter::All => true,
                    ChannelFilter::Mono => album.uniform_channel_count() == Some(1),
                    ChannelFilter::Stereo => album.uniform_channel_count() == Some(2),
                    ChannelFilter::Surround => {
                        matches!(album.uniform_channel_count(), Some(5) | Some(6))
                    }
                    ChannelFilter::Surround71 => album.uniform_channel_count() == Some(8),
                    ChannelFilter::SurroundPlus => {
                        album.uniform_channel_count().is_some_and(|ch| ch > 8)
                    }
                    ChannelFilter::Mixed => {
                        matches!(album.channel_type(), Some(AlbumChannelType::Mixed))
                    }
                    ChannelFilter::Specific(n) => album.uniform_channel_count() == Some(n),
                }
            } else {
                false
            }
        });

        indices
    }
}
