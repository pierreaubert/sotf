use super::error::RemoteCacheRefreshError;
use super::remote_album_cache::RemoteAlbumCache;
use super::remote_refresh_requests::RemoteRefreshRequests;
use super::remote_server_probe_status::RemoteServerProbeStatus;
use super::types::RemoteAlbumQueueCommandResult;
use super::types::RemoteCacheRefreshResult;
use std::collections::HashMap;

/// Native SOTF remote-control server picker and discovery state.
#[derive(Debug, Default)]
pub struct RemoteState {
    pub server_store: sotf_audio_player::SotfRemoteServerStore,
    pub discovered_servers: Vec<sotf_audio_player::lan_discovery::DiscoveredSotfApiServer>,
    pub server_probe_statuses: HashMap<String, RemoteServerProbeStatus>,
    /// Monotonic marker for probe-status changes observed by the UI tick.
    pub server_probe_revision: u64,
    pub discovery_running: bool,
    pub discovery_error: Option<String>,
    pub manual_server_name: String,
    pub manual_api_base_url: String,
    pub manual_auth_token: String,
    pub server_probe_receiver: Option<std::sync::mpsc::Receiver<(String, RemoteServerProbeStatus)>>,
    pub discovery_receiver: Option<
        std::sync::mpsc::Receiver<
            Result<Vec<sotf_audio_player::lan_discovery::DiscoveredSotfApiServer>, String>,
        >,
    >,
    /// Receiver for live SSE events from the selected remote server.
    pub event_stream_receiver: Option<
        std::sync::mpsc::Receiver<
            Result<sotf_audio_player::sotf_api_client::SotfApiStreamEvent, String>,
        >,
    >,
    /// Receiver for quiet remote cache refresh jobs.
    pub cache_refresh_receiver: Option<
        std::sync::mpsc::Receiver<Result<RemoteCacheRefreshResult, RemoteCacheRefreshError>>,
    >,
    /// Receiver for remote album queue mutation jobs.
    pub album_queue_command_receiver:
        Option<std::sync::mpsc::Receiver<RemoteAlbumQueueCommandResult>>,
    /// Whether a quiet remote cache refresh job is currently running.
    pub cache_refresh_in_progress: bool,
    /// Requests covered by the currently running quiet remote cache refresh.
    pub cache_refresh_requests_in_progress: RemoteRefreshRequests,
    /// Consecutive quiet cache refresh failures for the selected remote.
    pub cache_refresh_failures: u8,
    /// Disable quiet cache refreshes after repeated network failures.
    pub cache_updates_disabled: bool,
    /// Last quiet cache refresh error, kept for diagnostics only.
    pub cache_last_error: Option<String>,
    /// In-memory bearer token cache keyed by server ID. Persisted credentials
    /// live in platform storage or the shared internal token store.
    pub server_tokens: HashMap<String, String>,
    /// Bounded in-memory cache for remote album metadata and artwork.
    /// This is a performance cache only, not a local library mirror.
    pub album_cache: RemoteAlbumCache,
    /// Latest remote state snapshot received from the selected server.
    pub current_state: Option<sotf_audio_player::sotf_api_client::SotfApiState>,
    /// Latest remote queue snapshot received from the selected server.
    pub current_queue: Option<sotf_audio_player::sotf_api_client::SotfApiQueue>,
    /// Visible remote album page, sourced from the server API.
    pub current_album_page: Option<sotf_audio_player::sotf_api_client::SotfApiAlbumList>,
    /// Server ID that produced the visible remote album page.
    pub current_album_page_server_id: Option<String>,
    /// Search query used to produce the visible remote album page.
    pub current_album_page_query: String,
    /// Monotonic marker for remote album-page changes observed by the UI tick.
    pub remote_album_page_revision: u64,
    /// Remote library identity currently associated with the local database.
    pub local_library_identity: Option<crate::config::RemoteLibraryIdentity>,
    /// Minimal refresh work requested by SSE events.
    pub refresh_requests: RemoteRefreshRequests,
}

impl RemoteState {
    pub const CACHE_REFRESH_FAILURE_DISABLE_THRESHOLD: u8 = 3;

    pub fn set_server_probe_status(
        &mut self,
        server_id: impl Into<String>,
        status: RemoteServerProbeStatus,
    ) {
        self.server_probe_statuses.insert(server_id.into(), status);
        self.server_probe_revision = self.server_probe_revision.wrapping_add(1);
    }

    pub fn remove_server_probe_status(&mut self, server_id: &str) {
        if self.server_probe_statuses.remove(server_id).is_some() {
            self.server_probe_revision = self.server_probe_revision.wrapping_add(1);
        }
    }

    pub fn merge_discovered_servers(
        &mut self,
        servers: Vec<sotf_audio_player::lan_discovery::DiscoveredSotfApiServer>,
    ) -> usize {
        let mut merged = 0;
        let had_selection = self.server_store.selected_server_id.is_some();
        let mut first_id = None;

        for discovered in &servers {
            match sotf_audio_player::SotfRemoteServer::from_discovered(discovered) {
                Ok(server) => {
                    first_id.get_or_insert_with(|| server.id.clone());
                    self.server_store.upsert(server);
                    merged += 1;
                }
                Err(err) => {
                    log::warn!("Ignoring invalid discovered SOTF server: {err}");
                }
            }
        }

        if !had_selection && let Some(id) = first_id {
            let _ = self.server_store.select(id);
        }

        self.discovered_servers = servers;
        self.discovery_error = None;
        merged
    }

    pub fn add_manual_server_record(
        &mut self,
        friendly_name: impl Into<String>,
        api_base_url: impl Into<String>,
    ) -> Result<String, String> {
        let server = sotf_audio_player::SotfRemoteServer::manual(friendly_name, api_base_url)
            .map_err(|err| err.to_string())?;
        let id = server.id.clone();
        self.server_store.upsert(server);
        let _ = self.server_store.select(id.clone());
        Ok(id)
    }

    pub fn set_manual_server_name(&mut self, name: impl Into<String>) {
        self.manual_server_name = name.into();
    }

    pub fn set_manual_api_base_url(&mut self, api_base_url: impl Into<String>) {
        self.manual_api_base_url = api_base_url.into();
    }

    pub fn set_manual_auth_token(&mut self, token: impl Into<String>) {
        self.manual_auth_token = token.into();
    }

    pub fn add_manual_server_from_inputs(&mut self) -> Result<String, String> {
        let name = self.manual_server_name.trim().to_string();
        let mut api_base_url = self.manual_api_base_url.trim().to_string();
        let auth_token = self.manual_auth_token.trim().to_string();
        if api_base_url.is_empty() {
            return Err("remote server URL must not be empty".to_string());
        }
        if auth_token.is_empty() {
            return Err("remote API token must not be empty".to_string());
        }
        if !api_base_url.starts_with("http://") && !api_base_url.starts_with("https://") {
            api_base_url = format!("http://{api_base_url}");
        }

        let id = self.add_manual_server_record(name, api_base_url)?;
        self.server_tokens.insert(id.clone(), auth_token);
        self.manual_server_name.clear();
        self.manual_api_base_url.clear();
        self.manual_auth_token.clear();
        Ok(id)
    }

    pub fn apply_remote_album_page(
        &mut self,
        server_id: impl Into<String>,
        page: sotf_audio_player::sotf_api_client::SotfApiAlbumList,
        query: impl Into<String>,
    ) {
        let server_id = server_id.into();
        self.album_cache
            .upsert_metadata_page(&server_id, page.library_version, &page.albums);
        self.current_album_page = Some(page);
        self.current_album_page_server_id = Some(server_id);
        self.current_album_page_query = query.into();
        self.remote_album_page_revision = self.remote_album_page_revision.wrapping_add(1);
        self.refresh_requests.visible_album_page = false;
    }

    pub fn update_local_library_identity(
        &mut self,
        identity: crate::config::RemoteLibraryIdentity,
    ) -> bool {
        if self.local_library_identity.as_ref() == Some(&identity) {
            return false;
        }

        self.album_cache.invalidate_all();
        self.clear_remote_album_page();
        self.local_library_identity = Some(identity);
        true
    }

    pub fn clear_remote_album_page(&mut self) {
        if self.current_album_page.is_some()
            || self.current_album_page_server_id.is_some()
            || !self.current_album_page_query.is_empty()
        {
            self.remote_album_page_revision = self.remote_album_page_revision.wrapping_add(1);
        }
        self.current_album_page = None;
        self.current_album_page_server_id = None;
        self.current_album_page_query.clear();
    }

    pub fn reset_remote_cache_updater(&mut self) {
        self.cache_refresh_receiver = None;
        self.cache_refresh_in_progress = false;
        self.cache_refresh_requests_in_progress = RemoteRefreshRequests::default();
        self.cache_refresh_failures = 0;
        self.cache_updates_disabled = false;
        self.cache_last_error = None;
    }

    pub fn record_remote_cache_refresh_success(&mut self) {
        self.cache_refresh_in_progress = false;
        self.cache_refresh_requests_in_progress = RemoteRefreshRequests::default();
        self.cache_refresh_receiver = None;
        self.cache_refresh_failures = 0;
        self.cache_last_error = None;
    }

    pub fn record_remote_cache_refresh_failure(&mut self, err: RemoteCacheRefreshError) {
        self.cache_refresh_in_progress = false;
        self.cache_refresh_requests_in_progress = RemoteRefreshRequests::default();
        self.cache_refresh_receiver = None;
        self.cache_last_error = Some(err.message);
        self.cache_refresh_failures = self.cache_refresh_failures.saturating_add(1);
        if self.cache_refresh_failures >= Self::CACHE_REFRESH_FAILURE_DISABLE_THRESHOLD {
            self.cache_updates_disabled = true;
            self.refresh_requests = RemoteRefreshRequests::default();
        } else {
            self.refresh_requests.merge(err.requests);
        }
    }
}
