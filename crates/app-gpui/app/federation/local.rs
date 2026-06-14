use super::misc::pairing_qr_host;
use super::misc::run_sotf_api_request;
use super::misc::scan_federation_source_async;
use crate::app::App;
use crate::app::state::app::{FederationScanMessage, FederationScanProgress, FederationScanResult};
use crate::app::types::ToastMessage;
use sotf_audio_player::federation_config::{
    ConnectionStatus, FederationSourceEntry, SotfApiSettings, SourceConnectionConfig,
};
use std::net::IpAddr;

impl App {
    pub(super) fn save_federation_source_or_revert(
        &mut self,
        index: usize,
        previous: FederationSourceEntry,
        action: &str,
    ) -> bool {
        let result = self
            .library_state
            .library
            .get_database()
            .map(|db| db.save_federation_source(&self.federation.sources[index]))
            .unwrap_or(Ok(()));

        if let Err(e) = result {
            self.federation.sources[index] = previous;
            self.ui_state.toast_message = Some(ToastMessage::error(format!(
                "Failed to {action}; restored previous source settings: {e}"
            )));
            return false;
        }

        true
    }

    pub(super) fn persist_source_availability(
        &mut self,
        source_id: &str,
        available: bool,
        action: &str,
    ) -> bool {
        let result = self
            .library_state
            .library
            .get_database()
            .map(|db| db.set_source_availability(source_id, available))
            .unwrap_or(Ok(()));

        if let Err(e) = result {
            self.ui_state.toast_message = Some(ToastMessage::error(format!(
                "Failed to {action}; source availability was not saved: {e}"
            )));
            return false;
        }

        true
    }

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

        self.federation.sources.push(source);
        self.ui_state.toast_message = Some(ToastMessage::success("Source added."));
    }

    /// Remove a federation source by index and delete from database.
    pub fn remove_federation_source(&mut self, index: usize) {
        if index >= self.federation.sources.len() {
            return;
        }

        let source_id = self.federation.sources[index].source_id.clone();

        if let Some(db) = self.library_state.library.get_database()
            && let Err(e) = db.delete_federation_source(&source_id)
        {
            self.ui_state.toast_message =
                Some(ToastMessage::error(format!("Failed to delete source: {e}")));
            return;
        }

        self.federation.sources.remove(index);
        self.ui_state.toast_message = Some(ToastMessage::success("Source removed."));
    }

    /// Toggle the enabled state of a federation source by index.
    pub fn toggle_federation_source(&mut self, index: usize) {
        if index >= self.federation.sources.len() {
            return;
        }

        let previous = self.federation.sources[index].clone();
        let source = &mut self.federation.sources[index];
        source.is_enabled = !source.is_enabled;

        self.save_federation_source_or_revert(index, previous, "toggle federation source");
    }

    /// Update a field value on a federation source connection config.
    pub fn update_federation_source_field(
        &mut self,
        source_index: usize,
        field_index: usize,
        value: &str,
    ) {
        if source_index >= self.federation.sources.len() {
            return;
        }

        let previous = self.federation.sources[source_index].clone();
        let source = &mut self.federation.sources[source_index];
        source.connection.set_field_value(field_index, value);

        self.save_federation_source_or_revert(source_index, previous, "update federation source");
    }

    /// Update the display name of a federation source.
    pub fn update_federation_source_name(&mut self, index: usize, name: &str) {
        if index >= self.federation.sources.len() {
            return;
        }

        let previous = self.federation.sources[index].clone();
        self.federation.sources[index].display_name = name.to_string();

        self.save_federation_source_or_revert(index, previous, "rename federation source");
    }

    /// Toggle MPD server enabled state and persist.
    pub fn toggle_mpd_server(&mut self) {
        self.federation.server_config.mpd.enabled = !self.federation.server_config.mpd.enabled;
        self.save_server_config();
    }

    /// Toggle DLNA server enabled state and persist.
    pub fn toggle_dlna_server(&mut self) {
        self.federation.server_config.dlna.enabled = !self.federation.server_config.dlna.enabled;
        self.save_server_config();
    }

    /// Toggle the local SOTF API connection QR. When showing the QR, make sure
    /// the API is enabled and has a bearer token so the encoded payload is
    /// immediately usable by remote clients.
    pub fn toggle_sotf_api_connection_qr(&mut self) -> Result<(), String> {
        if self.ui_state.show_sotf_api_connection_qr {
            self.ui_state.show_sotf_api_connection_qr = false;
            return Ok(());
        }

        if sotf_audio_player::server::ensure_sotf_api_connection_config(
            &mut self.federation.server_config,
        ) {
            self.save_server_config();
        }
        self.ui_state.show_sotf_api_connection_qr = true;
        Ok(())
    }

    #[must_use]
    pub fn sotf_api_connection_qr_data(&self) -> Option<String> {
        sotf_audio_player::server::sotf_api_connection_qr_payload(
            &self.federation.server_config.api,
        )
        .ok()
    }

    /// Update an MPD server field and persist.
    pub fn update_mpd_field(&mut self, field: &str, value: &str) {
        match field {
            "bind_address" => self.federation.server_config.mpd.bind_address = value.to_string(),
            "port" => {
                if let Ok(p) = value.parse() {
                    self.federation.server_config.mpd.port = p;
                }
            }
            "tls_enabled" => self.federation.server_config.mpd.tls_enabled = value == "true",
            "auth_mode" => {
                use sotf_audio_player::federation_config::MpdAuthMode;
                self.federation.server_config.mpd.auth_mode = if value == "password" {
                    MpdAuthMode::Password
                } else {
                    MpdAuthMode::Certificate
                };
            }
            "password" => {
                self.federation.server_config.mpd.password = if value.is_empty() {
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
            "friendly_name" => self.federation.server_config.dlna.friendly_name = value.to_string(),
            "port" => {
                if let Ok(p) = value.parse() {
                    self.federation.server_config.dlna.port = p;
                }
            }
            _ => return,
        }
        self.save_server_config();
    }

    pub(super) fn save_server_config(&self) {
        if let Err(e) =
            sotf_audio_player::config::save_server_config(&self.federation.server_config)
        {
            log::warn!("Failed to save server config: {e}");
        }
    }

    pub(super) fn local_sotf_api_client(
        &self,
    ) -> Result<sotf_audio_player::sotf_api_client::SotfApiClient, String> {
        let api = &self.federation.server_config.api;
        if !api.enabled {
            return Err("SOTF API server is disabled".to_string());
        }
        let token = api
            .auth_token
            .as_deref()
            .filter(|token| !token.trim().is_empty())
            .ok_or_else(|| "SOTF API auth token is not configured".to_string())?;
        let base_url = local_sotf_api_base_url(api);
        sotf_audio_player::sotf_api_client::SotfApiClient::new(base_url, token)
            .map_err(|err| err.to_string())
    }

    pub(super) fn set_pairing_error(&mut self, message: String) {
        self.federation.pairing_error = Some(message.clone());
        self.ui_state.toast_message = Some(ToastMessage::error(message));
    }

    // -------------------------------------------------------------------------
    // Pairing & mTLS trust management
    // -------------------------------------------------------------------------

    /// Load the server TLS fingerprint and trusted clients from local cert store.
    pub fn refresh_pairing_state(&mut self) {
        let config_dir = match sotf_audio_player::config::get_app_config_dir() {
            Some(dir) => dir,
            None => {
                self.federation.pairing_error =
                    Some("Could not determine config directory".to_string());
                return;
            }
        };

        // Load server fingerprint
        match sotf_tls::CertStore::load_or_generate(&config_dir) {
            Ok(store) => {
                self.federation.server_fingerprint = Some(store.server_fingerprint());
            }
            Err(err) => {
                log::warn!("Failed to load server cert store: {err}");
                self.federation.pairing_error = Some(format!("Failed to load certificate: {err}"));
            }
        }

        // Load trusted clients
        match sotf_tls::TrustedClientStore::load(&config_dir) {
            Ok(store) => {
                self.federation.trusted_clients = store
                    .list()
                    .into_iter()
                    .map(|c| crate::app::state::app::TrustedClientInfo {
                        fingerprint: c.fingerprint.clone(),
                        name: c.name.clone(),
                        paired_at: c.paired_at.clone(),
                    })
                    .collect();
            }
            Err(err) => {
                log::warn!("Failed to load trusted clients: {err}");
                self.federation.pairing_error =
                    Some(format!("Failed to load trusted clients: {err}"));
            }
        }
    }

    /// Toggle pairing mode on/off.
    /// When enabled, generates a new nonce and loads the server fingerprint.
    pub fn toggle_pairing_mode(&mut self) {
        let client = match self.local_sotf_api_client() {
            Ok(client) => client,
            Err(err) => {
                self.set_pairing_error(err);
                return;
            }
        };

        let result = if self.federation.pairing_enabled {
            run_sotf_api_request(client.disable_pairing())
        } else {
            run_sotf_api_request(client.enable_pairing())
        };

        match result {
            Ok(response) => {
                self.federation.pairing_enabled = response.pairing_enabled;
                self.federation.pairing_nonce = response.nonce;
                self.federation.pairing_error = None;
                self.refresh_pairing_state();
                log::info!(
                    "[pairing] Pairing mode {}",
                    if self.federation.pairing_enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
            }
            Err(err) => self.set_pairing_error(format!("Failed to update pairing mode: {err}")),
        }
    }

    /// Revoke a trusted client by fingerprint.
    pub fn revoke_trusted_client(&mut self, fingerprint: &str) {
        let client = match self.local_sotf_api_client() {
            Ok(client) => client,
            Err(err) => {
                self.set_pairing_error(err);
                return;
            }
        };

        match run_sotf_api_request(client.revoke_trusted_client(fingerprint)) {
            Ok(_) => {
                log::info!("[pairing] Revoked client with fingerprint {fingerprint}");
                self.federation.pairing_error = None;
                self.refresh_pairing_state();
            }
            Err(err) => self.set_pairing_error(format!("Failed to revoke client: {err}")),
        }
    }

    /// Build the QR code data string for pairing.
    /// Format: `sotf://pair?host=<ip>&port=<port>&fingerprint=<fp>&nonce=<nonce>`
    #[must_use]
    pub fn pairing_qr_data(&self) -> Option<String> {
        let nonce = self.federation.pairing_nonce.as_ref()?;
        let fingerprint = self.federation.server_fingerprint.as_ref()?;
        let port = self.federation.server_config.api.port;
        let host = pairing_qr_host(&self.federation.server_config.api.bind_address)?;
        Some(format!(
            "sotf://pair?host={host}&port={port}&fingerprint={fingerprint}&nonce={nonce}"
        ))
    }

    /// Test connection to a federation source by index.
    /// Sets status to Testing synchronously and returns the source entry if valid.
    pub fn start_federation_source_test(
        &mut self,
        index: usize,
    ) -> Option<(String, FederationSourceEntry)> {
        if index >= self.federation.sources.len() {
            return None;
        }

        let source = self.federation.sources[index].clone();
        let source_id = source.source_id.clone();

        self.federation
            .source_statuses
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
            .federation
            .sources
            .iter_mut()
            .find(|s| s.source_id == source_id)
        {
            source.is_available = Some(available);
        }

        self.persist_source_availability(source_id, available, "save federation source status");

        self.federation
            .source_statuses
            .insert(source_id.to_string(), status);
    }

    /// Scan a federation source for content.
    /// Spawns a background thread that opens its own DB connection, fetches
    /// all albums/tracks from the provider, and merges them into the local database.
    /// Sends progress messages so the UI can display a live progress row.
    pub fn scan_federation_source(&mut self, index: usize) {
        if index >= self.federation.sources.len() {
            return;
        }

        if self.federation.scan_receiver.is_some() {
            self.ui_state.toast_message = Some(ToastMessage::warning(
                "A federation scan is already running.",
            ));
            return;
        }

        let source = self.federation.sources[index].clone();
        let display_name = source.display_name.clone();

        let (tx, rx) = std::sync::mpsc::channel();
        self.federation.scan_receiver = Some(rx);
        self.federation
            .scan_cancel
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.federation.scan_progress = Some(FederationScanProgress {
            source_name: display_name,
            albums_total: 0,
            albums_merged: 0,
            tracks_merged: 0,
        });

        let cancel = self.federation.scan_cancel.clone();

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
        self.federation
            .scan_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Poll for federation scan progress and completion. Call from the UI update loop.
    pub fn update_federation_scan(&mut self) {
        let rx = match &self.federation.scan_receiver {
            Some(rx) => rx,
            None => return,
        };

        // Drain all pending messages (progress updates arrive faster than frames)
        loop {
            match rx.try_recv() {
                Ok(FederationScanMessage::FetchedAlbums { total }) => {
                    if let Some(p) = &mut self.federation.scan_progress {
                        p.albums_total = total;
                    }
                }
                Ok(FederationScanMessage::Progress {
                    albums_merged,
                    tracks_merged,
                }) => {
                    if let Some(p) = &mut self.federation.scan_progress {
                        p.albums_merged = albums_merged;
                        p.tracks_merged = tracks_merged;
                    }
                }
                Ok(FederationScanMessage::Done(result)) => {
                    self.federation.scan_receiver = None;
                    self.federation.scan_progress = None;
                    self.handle_scan_result(result);
                    return;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.federation.scan_receiver = None;
                    self.federation.scan_progress = None;
                    return;
                }
            }
        }
    }

    pub(super) fn handle_scan_result(&mut self, result: FederationScanResult) {
        if let Some(ref err) = result.error {
            self.ui_state.toast_message = Some(ToastMessage::error(format!("Scan failed: {err}",)));
            if let Some(source) = self
                .federation
                .sources
                .iter_mut()
                .find(|s| s.source_id == result.source_id)
            {
                source.is_available = Some(false);
            }
            self.persist_source_availability(
                &result.source_id,
                false,
                "save failed federation scan status",
            );
        } else {
            self.ui_state.toast_message = Some(ToastMessage::success(format!(
                "Scan complete: {} albums, {} tracks merged.",
                result.albums, result.tracks
            )));
            if let Some(source) = self
                .federation
                .sources
                .iter_mut()
                .find(|s| s.source_id == result.source_id)
            {
                source.is_available = Some(true);
            }
            if self.persist_source_availability(
                &result.source_id,
                true,
                "save successful federation scan status",
            ) {
                let sync_result = self
                    .library_state
                    .library
                    .get_database()
                    .map(|db| db.update_federation_source_sync_time(&result.source_id))
                    .unwrap_or(Ok(()));
                if let Err(e) = sync_result {
                    self.ui_state.toast_message = Some(ToastMessage::error(format!(
                        "Scan completed, but failed to save source sync time: {e}"
                    )));
                }
            }
            if let Err(e) = self.load_library_from_database() {
                log::error!("Failed to reload library after federation scan: {e}");
            }
        }
        self.federation.source_statuses.insert(
            result.source_id.clone(),
            match result.error {
                Some(err) => ConnectionStatus::Error(err),
                None => ConnectionStatus::Connected { version: None },
            },
        );
    }

    /// Get the connection status for a federation source.
    pub fn get_federation_source_status(&self, source_id: &str) -> Option<&ConnectionStatus> {
        self.federation.source_statuses.get(source_id)
    }
}

fn local_sotf_api_base_url(settings: &SotfApiSettings) -> String {
    format!(
        "http://{}:{}",
        local_api_connect_host(&settings.bind_address),
        settings.port
    )
}

pub(super) fn local_api_connect_host(bind_address: &str) -> String {
    match bind_address.parse::<IpAddr>() {
        Ok(IpAddr::V4(addr)) if addr.is_unspecified() => "127.0.0.1".to_string(),
        Ok(IpAddr::V4(addr)) => addr.to_string(),
        Ok(IpAddr::V6(addr)) if addr.is_unspecified() => "[::1]".to_string(),
        Ok(IpAddr::V6(addr)) => format!("[{addr}]"),
        Err(_) => bind_address.to_string(),
    }
}
