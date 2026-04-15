//! Federation sources and server configuration business logic.

use crate::app::App;
use crate::app::state::app::{FederationScanMessage, FederationScanProgress, FederationScanResult};
use crate::app::types::ToastMessage;
use sotf_audio_player::federation_config::{
    ConnectionStatus, FederationSourceEntry, SourceConnectionConfig,
};

impl App {
    /// Add a new federation source of the given type and persist to database.
    pub fn add_federation_source(&mut self, type_name: &str) {
        let source_id = format!("{}_{}", type_name, chrono::Utc::now().timestamp_millis());
        let display_name = format!("New {} source", type_name);
        let source = FederationSourceEntry {
            source_id,
            display_name,
            priority: 0,
            is_enabled: false,
            connection: SourceConnectionConfig::default_for_type(type_name),
            is_available: None,
        };

        if let Some(db) = self.library_state.library.get_database()
            && let Err(e) = db.save_federation_source(&source)
        {
            self.ui_state.toast_message =
                Some(ToastMessage::error(format!("Failed to save source: {e}")));
            return;
        }

        self.federation_sources.push(source);
        self.ui_state.toast_message = Some(ToastMessage::success("Source added."));
    }

    /// Remove a federation source by index and delete from database.
    pub fn remove_federation_source(&mut self, index: usize) {
        if index >= self.federation_sources.len() {
            return;
        }

        let source_id = self.federation_sources[index].source_id.clone();

        if let Some(db) = self.library_state.library.get_database()
            && let Err(e) = db.delete_federation_source(&source_id)
        {
            self.ui_state.toast_message =
                Some(ToastMessage::error(format!("Failed to delete source: {e}")));
            return;
        }

        self.federation_sources.remove(index);
        self.ui_state.toast_message = Some(ToastMessage::success("Source removed."));
    }

    /// Toggle the enabled state of a federation source by index.
    pub fn toggle_federation_source(&mut self, index: usize) {
        if index >= self.federation_sources.len() {
            return;
        }

        let source = &mut self.federation_sources[index];
        source.is_enabled = !source.is_enabled;

        if let Some(db) = self.library_state.library.get_database() {
            let _ = db.save_federation_source(source);
        }
    }

    /// Update a field value on a federation source connection config.
    pub fn update_federation_source_field(
        &mut self,
        source_index: usize,
        field_index: usize,
        value: &str,
    ) {
        if source_index >= self.federation_sources.len() {
            return;
        }

        let source = &mut self.federation_sources[source_index];
        source.connection.set_field_value(field_index, value);

        if let Some(db) = self.library_state.library.get_database() {
            let _ = db.save_federation_source(source);
        }
    }

    /// Update the display name of a federation source.
    pub fn update_federation_source_name(&mut self, index: usize, name: &str) {
        if index >= self.federation_sources.len() {
            return;
        }

        self.federation_sources[index].display_name = name.to_string();

        if let Some(db) = self.library_state.library.get_database() {
            let _ = db.save_federation_source(&self.federation_sources[index]);
        }
    }

    /// Toggle MPD server enabled state and persist.
    pub fn toggle_mpd_server(&mut self) {
        self.server_config.mpd.enabled = !self.server_config.mpd.enabled;
        self.save_server_config();
    }

    /// Toggle DLNA server enabled state and persist.
    pub fn toggle_dlna_server(&mut self) {
        self.server_config.dlna.enabled = !self.server_config.dlna.enabled;
        self.save_server_config();
    }

    /// Update an MPD server field and persist.
    pub fn update_mpd_field(&mut self, field: &str, value: &str) {
        match field {
            "bind_address" => self.server_config.mpd.bind_address = value.to_string(),
            "port" => {
                if let Ok(p) = value.parse() {
                    self.server_config.mpd.port = p;
                }
            }
            "tls_enabled" => self.server_config.mpd.tls_enabled = value == "true",
            "auth_mode" => {
                use sotf_audio_player::federation_config::MpdAuthMode;
                self.server_config.mpd.auth_mode = if value == "password" {
                    MpdAuthMode::Password
                } else {
                    MpdAuthMode::Certificate
                };
            }
            "password" => {
                self.server_config.mpd.password = if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
            }
            _ => return,
        }
        self.save_server_config();
    }

    /// Update a DLNA server field and persist.
    pub fn update_dlna_field(&mut self, field: &str, value: &str) {
        match field {
            "friendly_name" => self.server_config.dlna.friendly_name = value.to_string(),
            "port" => {
                if let Ok(p) = value.parse() {
                    self.server_config.dlna.port = p;
                }
            }
            _ => return,
        }
        self.save_server_config();
    }

    fn save_server_config(&self) {
        if let Err(e) = sotf_audio_player::config::save_server_config(&self.server_config) {
            log::warn!("Failed to save server config: {e}");
        }
    }

    /// Test connection to a federation source by index.
    /// Sets status to Testing synchronously and returns the source entry if valid.
    pub fn start_federation_source_test(
        &mut self,
        index: usize,
    ) -> Option<(String, FederationSourceEntry)> {
        if index >= self.federation_sources.len() {
            return None;
        }

        let source = self.federation_sources[index].clone();
        let source_id = source.source_id.clone();

        self.federation_source_statuses
            .insert(source_id.clone(), ConnectionStatus::Testing);

        Some((source_id, source))
    }

    /// Update federation source connection status (called after async test completes).
    /// Also persists the availability state to the database.
    pub fn set_federation_source_status(&mut self, source_id: &str, status: ConnectionStatus) {
        let available = match &status {
            ConnectionStatus::Connected { .. } => true,
            ConnectionStatus::Diagnostic(d) => d.is_success(),
            _ => false,
        };

        // Update in-memory source
        if let Some(source) = self
            .federation_sources
            .iter_mut()
            .find(|s| s.source_id == source_id)
        {
            source.is_available = Some(available);
        }

        // Persist to database
        if let Some(db) = self.library_state.library.get_database() {
            let _ = db.set_source_availability(source_id, available);
        }

        self.federation_source_statuses
            .insert(source_id.to_string(), status);
    }

    /// Scan a federation source for content.
    /// Spawns a background thread that opens its own DB connection, fetches
    /// all albums/tracks from the provider, and merges them into the local database.
    /// Sends progress messages so the UI can display a live progress row.
    pub fn scan_federation_source(&mut self, index: usize) {
        if index >= self.federation_sources.len() {
            return;
        }

        if self.federation_scan_receiver.is_some() {
            self.ui_state.toast_message = Some(ToastMessage::warning(
                "A federation scan is already running.",
            ));
            return;
        }

        let source = self.federation_sources[index].clone();
        let display_name = source.display_name.clone();

        let (tx, rx) = std::sync::mpsc::channel();
        self.federation_scan_receiver = Some(rx);
        self.federation_scan_cancel
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.federation_scan_progress = Some(FederationScanProgress {
            source_name: display_name,
            albums_total: 0,
            albums_merged: 0,
            tracks_merged: 0,
        });

        let cancel = self.federation_scan_cancel.clone();

        std::thread::Builder::new()
            .name("federation-scan".into())
            .spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
                rt.block_on(scan_federation_source_async(&source, &tx, &cancel));
            })
            .expect("spawn federation scan thread");
    }

    /// Cancel the running federation scan.
    pub fn cancel_federation_scan(&mut self) {
        self.federation_scan_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Poll for federation scan progress and completion. Call from the UI update loop.
    pub fn update_federation_scan(&mut self) {
        let rx = match &self.federation_scan_receiver {
            Some(rx) => rx,
            None => return,
        };

        // Drain all pending messages (progress updates arrive faster than frames)
        loop {
            match rx.try_recv() {
                Ok(FederationScanMessage::FetchedAlbums { total }) => {
                    if let Some(p) = &mut self.federation_scan_progress {
                        p.albums_total = total;
                    }
                }
                Ok(FederationScanMessage::Progress {
                    albums_merged,
                    tracks_merged,
                }) => {
                    if let Some(p) = &mut self.federation_scan_progress {
                        p.albums_merged = albums_merged;
                        p.tracks_merged = tracks_merged;
                    }
                }
                Ok(FederationScanMessage::Done(result)) => {
                    self.federation_scan_receiver = None;
                    self.federation_scan_progress = None;
                    self.handle_scan_result(result);
                    return;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.federation_scan_receiver = None;
                    self.federation_scan_progress = None;
                    return;
                }
            }
        }
    }

    fn handle_scan_result(&mut self, result: FederationScanResult) {
        if let Some(ref err) = result.error {
            self.ui_state.toast_message = Some(ToastMessage::error(format!("Scan failed: {err}",)));
            if let Some(source) = self
                .federation_sources
                .iter_mut()
                .find(|s| s.source_id == result.source_id)
            {
                source.is_available = Some(false);
            }
            if let Some(db) = self.library_state.library.get_database() {
                let _ = db.set_source_availability(&result.source_id, false);
            }
        } else {
            self.ui_state.toast_message = Some(ToastMessage::success(format!(
                "Scan complete: {} albums, {} tracks merged.",
                result.albums, result.tracks
            )));
            if let Some(source) = self
                .federation_sources
                .iter_mut()
                .find(|s| s.source_id == result.source_id)
            {
                source.is_available = Some(true);
            }
            if let Some(db) = self.library_state.library.get_database() {
                let _ = db.set_source_availability(&result.source_id, true);
                let _ = db.update_federation_source_sync_time(&result.source_id);
            }
            if let Err(e) = self.load_library_from_database() {
                log::error!("Failed to reload library after federation scan: {e}");
            }
        }
        self.federation_source_statuses.insert(
            result.source_id.clone(),
            match result.error {
                Some(err) => ConnectionStatus::Error(err),
                None => ConnectionStatus::Connected { version: None },
            },
        );
    }

    /// Get the connection status for a federation source.
    pub fn get_federation_source_status(&self, source_id: &str) -> Option<&ConnectionStatus> {
        self.federation_source_statuses.get(source_id)
    }
}

/// Run a structured diagnostic test against a federation source.
/// Delegates to the shared implementation in sotf-player.
pub fn test_federation_connection(source: &FederationSourceEntry) -> ConnectionStatus {
    sotf_audio_player::federation_scan::run_connection_diagnostic(source)
}

/// Scan a federation source using the shared pipeline.
/// Sends progress messages via `tx`. Checks `cancel` flag between albums.
async fn scan_federation_source_async(
    source: &FederationSourceEntry,
    tx: &std::sync::mpsc::Sender<FederationScanMessage>,
    cancel: &std::sync::atomic::AtomicBool,
) {
    use sotf_audio_player::federation_scan;

    let source_id = source.source_id.clone();

    // Phase 1: fetch albums from the provider
    let albums = match federation_scan::fetch_source_albums(source).await {
        Ok(albums) => albums,
        Err(result) => {
            let _ = tx.send(FederationScanMessage::Done(result));
            return;
        }
    };

    let _ = tx.send(FederationScanMessage::FetchedAlbums {
        total: albums.len(),
    });

    // Phase 2: merge into local DB with progress reporting
    let tx_progress = tx.clone();
    let progress_cb: federation_scan::ScanProgressFn = Box::new(move |a, t| {
        let _ = tx_progress.send(FederationScanMessage::Progress {
            albums_merged: a,
            tracks_merged: t,
        });
    });

    let result =
        federation_scan::merge_albums_to_db(&source_id, &albums, cancel, Some(&progress_cb));

    let _ = tx.send(FederationScanMessage::Done(result));
}
