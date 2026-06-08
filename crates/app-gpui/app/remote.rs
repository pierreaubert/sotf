//! Native SOTF remote server picker and discovery logic.

use std::sync::mpsc;
use std::time::Duration;

use crate::app::App;
use crate::app::state::app::{
    RemoteCacheRefreshError, RemoteCacheRefreshResult, RemoteRefreshRequests,
    RemoteServerProbeStatus,
};
use crate::app::types::ToastMessage;

const DEFAULT_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(2);
const DEFAULT_REMOTE_ALBUM_PAGE_LIMIT: usize = 50;

impl App {
    pub fn start_remote_server_discovery(&mut self) {
        self.start_remote_server_discovery_with_timeout(DEFAULT_DISCOVERY_TIMEOUT);
    }

    pub fn start_remote_server_discovery_with_timeout(&mut self, timeout: Duration) {
        if self.remote.discovery_running {
            return;
        }

        let (tx, rx) = mpsc::channel();
        self.remote.discovery_receiver = Some(rx);
        self.remote.discovery_running = true;
        self.remote.discovery_error = None;

        std::thread::Builder::new()
            .name("sotf-remote-discovery".into())
            .spawn(move || {
                let result = tokio::runtime::Runtime::new()
                    .map_err(|err| format!("Failed to start discovery runtime: {err}"))
                    .and_then(|rt| {
                        rt.block_on(sotf_audio_player::lan_discovery::discover_sotf_api_servers(
                            timeout,
                        ))
                    });
                let _ = tx.send(result);
            })
            .expect("spawn SOTF remote discovery thread");
    }

    pub fn update_remote_server_discovery(&mut self) {
        let result = match self.remote.discovery_receiver.as_ref() {
            Some(rx) => match rx.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Err("SOTF remote discovery worker disconnected".to_string())
                }
            },
            None => return,
        };

        self.remote.discovery_receiver = None;
        self.remote.discovery_running = false;

        match result {
            Ok(servers) => {
                let merged = self.remote.merge_discovered_servers(servers);
                if merged > 0 {
                    if self.save_remote_server_store("save discovered SOTF servers") {
                        self.ui_state.toast_message = Some(ToastMessage::success(format!(
                            "Found {merged} SOTF server(s)."
                        )));
                    }
                } else {
                    self.ui_state.toast_message = Some(ToastMessage::info(
                        "No SOTF servers found on the local network.",
                    ));
                }
            }
            Err(err) => {
                self.remote.discovery_error = Some(err.clone());
                self.ui_state.toast_message = Some(ToastMessage::warning(format!(
                    "SOTF discovery failed: {err}"
                )));
            }
        }
    }

    pub fn start_remote_server_probe(&mut self, server_id: &str) -> bool {
        if self.remote.server_probe_receiver.is_some() {
            self.ui_state.toast_message = Some(ToastMessage::warning(
                "A SOTF server test is already running.",
            ));
            return false;
        }

        let Some(server) = self
            .remote
            .server_store
            .servers
            .iter()
            .find(|server| server.id == server_id)
            .cloned()
        else {
            return false;
        };

        let server_id = server.id.clone();
        let api_base_url = server.api_base_url.clone();
        let (tx, rx) = mpsc::channel();
        self.remote
            .server_probe_statuses
            .insert(server_id.clone(), RemoteServerProbeStatus::Testing);
        self.remote.server_probe_receiver = Some(rx);

        std::thread::Builder::new()
            .name("sotf-remote-probe".into())
            .spawn(move || {
                let status = probe_remote_server_public(&api_base_url);
                let _ = tx.send((server_id, status));
            })
            .expect("spawn SOTF remote probe thread");

        true
    }

    pub fn update_remote_server_probe(&mut self) {
        let Some(rx) = self.remote.server_probe_receiver.as_ref() else {
            return;
        };

        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => (
                String::new(),
                RemoteServerProbeStatus::Failed("SOTF server test worker disconnected".to_string()),
            ),
        };

        self.remote.server_probe_receiver = None;
        let (server_id, status) = result;
        if !server_id.is_empty() {
            self.remote
                .server_probe_statuses
                .insert(server_id, status.clone());
        }

        match status {
            RemoteServerProbeStatus::Reachable { friendly_name, .. } => {
                self.ui_state.toast_message = Some(ToastMessage::success(format!(
                    "{friendly_name} is reachable."
                )));
            }
            RemoteServerProbeStatus::Failed(err) => {
                self.ui_state.toast_message = Some(ToastMessage::warning(format!(
                    "SOTF server test failed: {err}"
                )));
            }
            RemoteServerProbeStatus::Testing => {}
        }
    }

    pub fn add_manual_remote_server(
        &mut self,
        friendly_name: impl Into<String>,
        api_base_url: impl Into<String>,
    ) -> Result<String, String> {
        let id = self
            .remote
            .add_manual_server_record(friendly_name, api_base_url)?;
        if !self.save_remote_server_store("save manual SOTF server") {
            return Err("failed to save remote server store".to_string());
        }
        self.ui_state.toast_message = Some(ToastMessage::success("SOTF server saved."));
        Ok(id)
    }

    pub fn add_manual_remote_server_from_inputs(&mut self) -> Result<String, String> {
        let id = self.remote.add_manual_server_from_inputs()?;
        if !self.save_remote_server_store("save manual SOTF server") {
            return Err("failed to save remote server store".to_string());
        }
        let token_persisted = self.save_cached_remote_server_token(&id);
        let _ = self.select_remote_server(&id);
        self.ui_state.toast_message = if token_persisted {
            Some(ToastMessage::success("SOTF server saved."))
        } else {
            Some(ToastMessage::warning(
                "SOTF server saved, but the API token could not be stored in Keychain.",
            ))
        };
        Ok(id)
    }

    pub fn update_manual_remote_server_name(&mut self, name: impl Into<String>) {
        self.remote.set_manual_server_name(name);
    }

    pub fn update_manual_remote_server_url(&mut self, api_base_url: impl Into<String>) {
        self.remote.set_manual_api_base_url(api_base_url);
    }

    pub fn update_manual_remote_server_token(&mut self, token: impl Into<String>) {
        self.remote.set_manual_auth_token(token);
    }

    pub fn select_remote_server(&mut self, server_id: &str) -> bool {
        if !self.remote.server_store.select(server_id) {
            return false;
        }
        // Drop any existing event stream and start a new one for the selected server
        self.remote.event_stream_receiver = None;
        self.remote.album_cache.invalidate_server(server_id);
        self.remote.current_state = None;
        self.remote.current_queue = None;
        self.remote.clear_remote_album_page();
        self.remote.reset_remote_cache_updater();
        self.remote.refresh_requests = RemoteRefreshRequests {
            state: true,
            queue: true,
            visible_album_page: true,
        };
        self.load_persisted_remote_server_token(server_id);
        if self
            .remote
            .server_tokens
            .get(server_id)
            .is_some_and(|token| !token.trim().is_empty())
        {
            self.start_remote_event_stream();
        } else {
            self.ui_state.toast_message = Some(ToastMessage::warning(
                "SOTF API token required. Enter the token from the server settings.",
            ));
        }
        self.save_remote_server_store("save selected SOTF server")
    }

    pub fn remove_remote_server(&mut self, server_id: &str) -> bool {
        let removed = self.remote.server_store.remove(server_id);
        if removed.is_none() {
            return false;
        }
        if let Some(server) = removed.as_ref() {
            delete_persisted_remote_server_token(server);
        }
        self.remote.server_probe_statuses.remove(server_id);
        self.remote.server_tokens.remove(server_id);
        self.remote.event_stream_receiver = None;
        self.remote.album_cache.invalidate_server(server_id);
        self.remote.clear_remote_album_page();
        self.remote.reset_remote_cache_updater();
        self.save_remote_server_store("remove SOTF server")
    }

    /// Cache a bearer token for a remote server (in-memory only).
    pub fn set_remote_server_token(
        &mut self,
        server_id: impl Into<String>,
        token: impl Into<String>,
    ) {
        let server_id = server_id.into();
        let token = token.into();
        let token = token.trim();
        if token.is_empty() {
            self.remote.server_tokens.remove(&server_id);
        } else {
            self.remote
                .server_tokens
                .insert(server_id, token.to_string());
        }
        self.remote.reset_remote_cache_updater();
    }

    /// Retrieve the cached bearer token for a remote server, if any.
    #[must_use]
    pub fn get_remote_server_token(&self, server_id: &str) -> Option<&str> {
        self.remote.server_tokens.get(server_id).map(String::as_str)
    }

    /// Remove the cached bearer token for a remote server.
    pub fn clear_remote_server_token(&mut self, server_id: &str) {
        if let Some(server) = self
            .remote
            .server_store
            .servers
            .iter()
            .find(|server| server.id == server_id)
        {
            delete_persisted_remote_server_token(server);
        }
        self.remote.server_tokens.remove(server_id);
    }

    /// Start consuming the live SSE event stream from the selected remote server.
    pub fn start_remote_event_stream(&mut self) {
        let Some(server) = self.remote.server_store.selected_server().cloned() else {
            return;
        };
        let token = match self.remote.server_tokens.get(&server.id) {
            Some(token) => token.clone(),
            None => {
                log::warn!(
                    "No bearer token cached for remote server {}; event stream not started",
                    server.id
                );
                return;
            }
        };
        let api_base_url = server.api_base_url.clone();

        let (tx, rx) = std::sync::mpsc::channel();
        self.remote.event_stream_receiver = Some(rx);

        std::thread::Builder::new()
            .name("sotf-remote-events".into())
            .spawn(move || {
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(err) => {
                        let _ =
                            tx.send(Err(format!("Failed to start event stream runtime: {err}")));
                        return;
                    }
                };
                rt.block_on(async {
                    let client = match sotf_audio_player::sotf_api_client::SotfApiClient::new(
                        &api_base_url,
                        &token,
                    ) {
                        Ok(client) => client,
                        Err(err) => {
                            let _ = tx.send(Err(format!("Event stream client error: {err}")));
                            return;
                        }
                    };
                    let mut stream = match client.events_stream().await {
                        Ok(stream) => stream,
                        Err(err) => {
                            let _ = tx.send(Err(format!("Event stream open error: {err}")));
                            return;
                        }
                    };
                    while let Some(result) = stream.recv().await {
                        let mapped = match result {
                            Ok(event) => Ok(event),
                            Err(err) => Err(format!("Event stream error: {err}")),
                        };
                        if tx.send(mapped).is_err() {
                            break;
                        }
                    }
                });
            })
            .expect("spawn SOTF remote event stream thread");
    }

    /// Poll the remote event stream for new server events and dispatch them into app state.
    pub fn update_remote_event_stream(&mut self) {
        let result = match self.remote.event_stream_receiver.as_ref() {
            Some(rx) => match rx.try_recv() {
                Ok(result) => result,
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.remote.event_stream_receiver = None;
                    return;
                }
            },
            None => return,
        };

        match result {
            Ok(stream_event) => {
                log::debug!("[remote] SSE event received: {stream_event:?}");
                match stream_event {
                    sotf_audio_player::sotf_api_client::SotfApiStreamEvent::State(state) => {
                        log::debug!(
                            "[remote] State refresh: playback={} volume={} albums={}",
                            state.playback.state,
                            state.playback.volume,
                            state.library.albums
                        );
                        self.remote.current_state = Some(state);
                    }
                    sotf_audio_player::sotf_api_client::SotfApiStreamEvent::Server(event) => {
                        match event {
                            sotf_audio_player::sotf_server_event::SotfServerEvent::PlaybackChanged => {
                                self.remote.refresh_requests.state = true;
                            }
                            sotf_audio_player::sotf_server_event::SotfServerEvent::QueueChanged {
                                playlist_version,
                            } => {
                                let current_version = self
                                    .remote
                                    .current_state
                                    .as_ref()
                                    .map(|state| state.playback.playlist_version);
                                if current_version != Some(playlist_version) {
                                    self.remote.refresh_requests.queue = true;
                                }
                            }
                            sotf_audio_player::sotf_server_event::SotfServerEvent::VolumeChanged {
                                volume,
                            } => {
                                if let Some(state) = self.remote.current_state.as_mut() {
                                    state.playback.volume = volume;
                                } else {
                                    self.remote.refresh_requests.state = true;
                                }
                            }
                            sotf_audio_player::sotf_server_event::SotfServerEvent::LibraryChanged {
                                library_version: _,
                            } => {
                                if let Some(server_id) =
                                    self.remote.server_store.selected_server_id.as_deref()
                                {
                                    self.remote.album_cache.invalidate_server(server_id);
                                } else {
                                    self.remote.album_cache.invalidate_all();
                                }
                                self.remote.clear_remote_album_page();
                                self.remote.refresh_requests.visible_album_page = true;
                            }
                            sotf_audio_player::sotf_server_event::SotfServerEvent::StreamMetadataChanged {
                                title,
                                artist,
                            } => {
                                log::info!(
                                    "[remote] Stream metadata changed: artist={artist:?} title={title:?}"
                                );
                            }
                            sotf_audio_player::sotf_server_event::SotfServerEvent::ScannerProgress {
                                done,
                                total,
                            } => {
                                log::info!("[remote] Scanner progress: {done}/{total}");
                            }
                            sotf_audio_player::sotf_server_event::SotfServerEvent::Error { message } => {
                                log::warn!("[remote] Server error: {message}");
                                self.ui_state.toast_message =
                                    Some(ToastMessage::warning(format!("Remote server: {message}")));
                            }
                        }
                    }
                }
            }
            Err(err) => {
                log::warn!("[remote] Event stream error: {err}");
                self.ui_state.toast_message =
                    Some(ToastMessage::warning(format!("Remote events: {err}")));
                self.remote.event_stream_receiver = None;
            }
        }
    }

    /// Poll and launch quiet remote cache refreshes requested by SSE events.
    ///
    /// This never emits user-visible toasts: it is a performance cache, and
    /// unstable network conditions simply disable it until the remote is reset.
    pub fn update_remote_cache_refresh(&mut self) {
        if let Some(rx) = self.remote.cache_refresh_receiver.as_ref() {
            match rx.try_recv() {
                Ok(Ok(result)) => {
                    self.apply_remote_cache_refresh_result(result);
                    self.remote.record_remote_cache_refresh_success();
                }
                Ok(Err(err)) => {
                    log::warn!("[remote] Quiet cache refresh failed: {}", err.message);
                    self.remote.record_remote_cache_refresh_failure(err);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.remote
                        .record_remote_cache_refresh_failure(RemoteCacheRefreshError {
                            requests: RemoteRefreshRequests::default(),
                            message: "remote cache refresh worker disconnected".to_string(),
                        });
                }
            }
        }

        if self.remote.cache_updates_disabled
            || self.remote.cache_refresh_in_progress
            || self.remote.refresh_requests.is_empty()
        {
            return;
        }

        let Some(server) = self.remote.server_store.selected_server().cloned() else {
            return;
        };
        let Some(token) = self.remote.server_tokens.get(&server.id).cloned() else {
            return;
        };

        let requests = self.remote.refresh_requests;
        self.remote.refresh_requests = RemoteRefreshRequests::default();
        let page_request = if requests.visible_album_page {
            let (offset, limit) = self
                .remote
                .current_album_page
                .as_ref()
                .map(|page| (page.offset, page.limit.max(1)))
                .unwrap_or((0, DEFAULT_REMOTE_ALBUM_PAGE_LIMIT));
            Some((offset, limit.min(self.remote.album_cache.max_albums())))
        } else {
            None
        };

        let (tx, rx) = std::sync::mpsc::channel();
        self.remote.cache_refresh_receiver = Some(rx);
        self.remote.cache_refresh_in_progress = true;

        std::thread::Builder::new()
            .name("sotf-remote-cache".into())
            .spawn(move || {
                let result = refresh_remote_cache_worker(
                    server.id,
                    server.api_base_url,
                    token,
                    requests,
                    page_request,
                );
                let _ = tx.send(result);
            })
            .expect("spawn SOTF remote cache refresh thread");
    }

    fn apply_remote_cache_refresh_result(&mut self, result: RemoteCacheRefreshResult) {
        let selected_server = self.remote.server_store.selected_server_id.as_deref();
        if selected_server != Some(result.server_id.as_str()) {
            return;
        }

        if let Some(state) = result.state {
            self.remote.current_state = Some(state);
        }
        if let Some(queue) = result.queue {
            self.remote.current_queue = Some(queue);
        }

        let artwork_version = result
            .album_page
            .as_ref()
            .map(|page| page.library_version)
            .or_else(|| {
                self.remote
                    .current_album_page
                    .as_ref()
                    .map(|page| page.library_version)
            });

        if let Some(page) = result.album_page {
            self.remote
                .apply_remote_album_page(result.server_id.clone(), page);
        }

        if let Some(library_version) = artwork_version {
            for (album_id, bytes) in result.artwork {
                self.remote.album_cache.upsert_artwork(
                    &result.server_id,
                    library_version,
                    &album_id,
                    bytes,
                );
            }
        }
    }

    fn save_remote_server_store(&mut self, action: &str) -> bool {
        match sotf_audio_player::config::save_remote_server_store(&self.remote.server_store) {
            Ok(()) => true,
            Err(err) => {
                self.ui_state.toast_message =
                    Some(ToastMessage::error(format!("Failed to {action}: {err}")));
                false
            }
        }
    }

    /// Load platform-persisted tokens for all saved remote servers.
    pub fn load_persisted_remote_server_tokens(&mut self) {
        let servers = self.remote.server_store.servers.clone();
        for server in servers {
            if let Some(token) = load_persisted_remote_server_token(&server) {
                self.remote.server_tokens.insert(server.id, token);
            }
        }
    }

    fn load_persisted_remote_server_token(&mut self, server_id: &str) -> bool {
        if self
            .remote
            .server_tokens
            .get(server_id)
            .is_some_and(|token| !token.trim().is_empty())
        {
            return true;
        }

        let Some(server) = self
            .remote
            .server_store
            .servers
            .iter()
            .find(|server| server.id == server_id)
            .cloned()
        else {
            return false;
        };
        let Some(token) = load_persisted_remote_server_token(&server) else {
            return false;
        };
        self.remote.server_tokens.insert(server.id, token);
        true
    }

    fn save_cached_remote_server_token(&self, server_id: &str) -> bool {
        let Some(server) = self
            .remote
            .server_store
            .servers
            .iter()
            .find(|server| server.id == server_id)
        else {
            return false;
        };
        let Some(token) = self.remote.server_tokens.get(server_id) else {
            return false;
        };
        save_persisted_remote_server_token(server, token)
    }
}

#[cfg(target_os = "ios")]
unsafe extern "C" {
    fn sotf_ios_keychain_save(key: *const std::ffi::c_char, token: *const std::ffi::c_char)
    -> bool;
    fn sotf_ios_keychain_load(key: *const std::ffi::c_char) -> *const std::ffi::c_char;
    fn sotf_ios_keychain_delete(key: *const std::ffi::c_char) -> bool;
}

#[cfg(target_os = "ios")]
fn cstring_for_keychain(value: &str, field: &str) -> Option<std::ffi::CString> {
    match std::ffi::CString::new(value) {
        Ok(value) => Some(value),
        Err(_) => {
            log::warn!("[iOS] interior NUL in remote {field}; refusing Keychain operation");
            None
        }
    }
}

#[cfg(target_os = "ios")]
fn save_persisted_remote_server_token(
    server: &sotf_audio_player::SotfRemoteServer,
    token: &str,
) -> bool {
    let Some(key) = cstring_for_keychain(&server.token_secret_key(), "token key") else {
        return false;
    };
    let Some(token) = cstring_for_keychain(token, "token") else {
        return false;
    };
    // SAFETY: `key` and `token` are valid NUL-terminated strings for the
    // duration of the call. The Swift bridge copies the token into Keychain.
    unsafe { sotf_ios_keychain_save(key.as_ptr(), token.as_ptr()) }
}

#[cfg(target_os = "macos")]
fn save_persisted_remote_server_token(
    server: &sotf_audio_player::SotfRemoteServer,
    token: &str,
) -> bool {
    macos_keychain::save_token(&server.token_secret_key(), token)
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn save_persisted_remote_server_token(
    server: &sotf_audio_player::SotfRemoteServer,
    token: &str,
) -> bool {
    match sotf_audio_player::config::save_remote_server_token(&server.token_secret_key(), token) {
        Ok(()) => true,
        Err(err) => {
            log::warn!("Failed to save remote server token to internal store: {err}");
            false
        }
    }
}

#[cfg(not(any(
    target_os = "ios",
    target_os = "macos",
    target_os = "linux",
    target_os = "windows"
)))]
fn save_persisted_remote_server_token(
    _server: &sotf_audio_player::SotfRemoteServer,
    _token: &str,
) -> bool {
    false
}

#[cfg(target_os = "ios")]
fn load_persisted_remote_server_token(
    server: &sotf_audio_player::SotfRemoteServer,
) -> Option<String> {
    let key = cstring_for_keychain(&server.token_secret_key(), "token key")?;
    // SAFETY: `key` is a valid NUL-terminated string for the duration of the
    // call. The Swift bridge returns either NULL or a pointer to a static
    // UTF-8 buffer that remains valid until the next load call.
    let token = unsafe { sotf_ios_keychain_load(key.as_ptr()) };
    if token.is_null() {
        return None;
    }
    // SAFETY: non-null pointer returned by the Swift bridge points at a
    // NUL-terminated UTF-8 string.
    let token = unsafe { std::ffi::CStr::from_ptr(token) }.to_str().ok()?;
    let token = token.trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

#[cfg(target_os = "macos")]
fn load_persisted_remote_server_token(
    server: &sotf_audio_player::SotfRemoteServer,
) -> Option<String> {
    macos_keychain::load_token(&server.token_secret_key())
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn load_persisted_remote_server_token(
    server: &sotf_audio_player::SotfRemoteServer,
) -> Option<String> {
    match sotf_audio_player::config::load_remote_server_token(&server.token_secret_key()) {
        Ok(token) => token,
        Err(err) => {
            log::warn!("Failed to load remote server token from internal store: {err}");
            None
        }
    }
}

#[cfg(not(any(
    target_os = "ios",
    target_os = "macos",
    target_os = "linux",
    target_os = "windows"
)))]
fn load_persisted_remote_server_token(
    _server: &sotf_audio_player::SotfRemoteServer,
) -> Option<String> {
    None
}

#[cfg(target_os = "ios")]
fn delete_persisted_remote_server_token(server: &sotf_audio_player::SotfRemoteServer) -> bool {
    let Some(key) = cstring_for_keychain(&server.token_secret_key(), "token key") else {
        return false;
    };
    // SAFETY: `key` is a valid NUL-terminated string for the duration of the
    // call. The Swift bridge does not retain the pointer.
    unsafe { sotf_ios_keychain_delete(key.as_ptr()) }
}

#[cfg(target_os = "macos")]
fn delete_persisted_remote_server_token(server: &sotf_audio_player::SotfRemoteServer) -> bool {
    macos_keychain::delete_token(&server.token_secret_key())
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn delete_persisted_remote_server_token(server: &sotf_audio_player::SotfRemoteServer) -> bool {
    match sotf_audio_player::config::delete_remote_server_token(&server.token_secret_key()) {
        Ok(()) => true,
        Err(err) => {
            log::warn!("Failed to delete remote server token from internal store: {err}");
            false
        }
    }
}

#[cfg(not(any(
    target_os = "ios",
    target_os = "macos",
    target_os = "linux",
    target_os = "windows"
)))]
fn delete_persisted_remote_server_token(_server: &sotf_audio_player::SotfRemoteServer) -> bool {
    false
}

#[cfg(target_os = "macos")]
mod macos_keychain {
    use core_foundation::base::{CFType, CFTypeRef, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::data::CFData;
    use core_foundation::dictionary::CFMutableDictionary;
    use core_foundation::string::CFString;
    use core_foundation_sys::base::OSStatus;
    use core_foundation_sys::dictionary::CFDictionaryRef;
    use core_foundation_sys::string::CFStringRef;

    const ERR_SEC_SUCCESS: OSStatus = 0;
    const ERR_SEC_USER_CANCELED: OSStatus = -128;
    const ERR_SEC_ITEM_NOT_FOUND: OSStatus = -25300;

    pub fn save_token(key: &str, token: &str) -> bool {
        match save_token_result(key, token) {
            Ok(()) => true,
            Err(err) => {
                log::warn!("Failed to save remote server token to macOS Keychain: {err}");
                false
            }
        }
    }

    pub fn load_token(key: &str) -> Option<String> {
        match load_token_result(key) {
            Ok(token) => token,
            Err(err) => {
                log::warn!("Failed to load remote server token from macOS Keychain: {err}");
                None
            }
        }
    }

    pub fn delete_token(key: &str) -> bool {
        match delete_token_result(key) {
            Ok(()) => true,
            Err(err) => {
                log::warn!("Failed to delete remote server token from macOS Keychain: {err}");
                false
            }
        }
    }

    fn save_token_result(key: &str, token: &str) -> Result<(), String> {
        let service = CFString::from(sotf_audio_player::config::APP_BUNDLE_ID);
        let account = CFString::from(key);
        let token = CFData::from_buffer(token.trim().as_bytes());

        // SAFETY: Security.framework is called with CoreFoundation objects that
        // stay alive for the duration of the call. SecItem* retains/copies data.
        unsafe {
            let query = keychain_query(&service, &account);
            let mut attrs = CFMutableDictionary::with_capacity(1);
            attrs.set(kSecValueData as *const _, token.as_CFTypeRef());

            let mut status =
                SecItemUpdate(query.as_concrete_TypeRef(), attrs.as_concrete_TypeRef());
            if status == ERR_SEC_ITEM_NOT_FOUND {
                let mut add_attrs = keychain_query(&service, &account);
                add_attrs.set(kSecValueData as *const _, token.as_CFTypeRef());
                status = SecItemAdd(add_attrs.as_concrete_TypeRef(), std::ptr::null_mut());
            }

            if status == ERR_SEC_SUCCESS {
                Ok(())
            } else {
                Err(format!("SecItem save failed: {status}"))
            }
        }
    }

    fn load_token_result(key: &str) -> Result<Option<String>, String> {
        let service = CFString::from(sotf_audio_player::config::APP_BUNDLE_ID);
        let account = CFString::from(key);

        // SAFETY: Security.framework writes either NULL or a retained CFData
        // object into `result`; wrap_under_create_rule takes ownership.
        unsafe {
            let mut query = keychain_query(&service, &account);
            query.set(
                kSecReturnData as *const _,
                CFBoolean::true_value().as_CFTypeRef(),
            );

            let mut result = CFTypeRef::from(std::ptr::null());
            let status = SecItemCopyMatching(query.as_concrete_TypeRef(), &mut result);
            match status {
                ERR_SEC_SUCCESS => {}
                ERR_SEC_ITEM_NOT_FOUND | ERR_SEC_USER_CANCELED => return Ok(None),
                _ => return Err(format!("SecItem load failed: {status}")),
            }

            let data = CFType::wrap_under_create_rule(result)
                .downcast::<CFData>()
                .ok_or_else(|| "Keychain item data was not CFData".to_string())?;
            let token = std::str::from_utf8(data.bytes())
                .map_err(|err| format!("Keychain token was not UTF-8: {err}"))?
                .trim()
                .to_string();
            if token.is_empty() {
                Ok(None)
            } else {
                Ok(Some(token))
            }
        }
    }

    fn delete_token_result(key: &str) -> Result<(), String> {
        let service = CFString::from(sotf_audio_player::config::APP_BUNDLE_ID);
        let account = CFString::from(key);

        // SAFETY: Security.framework is called with CoreFoundation objects that
        // stay alive for the duration of the call.
        unsafe {
            let query = keychain_query(&service, &account);
            match SecItemDelete(query.as_concrete_TypeRef()) {
                ERR_SEC_SUCCESS | ERR_SEC_ITEM_NOT_FOUND => Ok(()),
                status => Err(format!("SecItem delete failed: {status}")),
            }
        }
    }

    fn keychain_query(
        service: &CFString,
        account: &CFString,
    ) -> CFMutableDictionary<*const std::ffi::c_void, *const std::ffi::c_void> {
        let mut query = CFMutableDictionary::with_capacity(3);
        // SAFETY: Security.framework exports these immutable CoreFoundation
        // string constants for process-wide use.
        unsafe {
            query.set(kSecClass as *const _, kSecClassGenericPassword as *const _);
            query.set(kSecAttrService as *const _, service.as_CFTypeRef());
            query.set(kSecAttrAccount as *const _, account.as_CFTypeRef());
        }
        query
    }

    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        static kSecClass: CFStringRef;
        static kSecClassGenericPassword: CFStringRef;
        static kSecAttrService: CFStringRef;
        static kSecAttrAccount: CFStringRef;
        static kSecValueData: CFStringRef;
        static kSecReturnData: CFStringRef;

        fn SecItemAdd(attributes: CFDictionaryRef, result: *mut CFTypeRef) -> OSStatus;
        fn SecItemUpdate(query: CFDictionaryRef, attributes: CFDictionaryRef) -> OSStatus;
        fn SecItemDelete(query: CFDictionaryRef) -> OSStatus;
        fn SecItemCopyMatching(query: CFDictionaryRef, result: *mut CFTypeRef) -> OSStatus;
    }
}

fn probe_remote_server_public(api_base_url: &str) -> RemoteServerProbeStatus {
    let result = tokio::runtime::Runtime::new()
        .map_err(|err| format!("Failed to start probe runtime: {err}"))
        .and_then(|rt| {
            rt.block_on(async move {
                let client = sotf_audio_player::sotf_api_client::SotfApiClient::new(
                    api_base_url,
                    "public-probe",
                )
                .map_err(|err| err.to_string())?;
                let health = client.health().await.map_err(|err| err.to_string())?;
                if !health.ok {
                    return Err("health endpoint reported not-ok".to_string());
                }
                let discovery = client.discovery().await.map_err(|err| err.to_string())?;
                let capabilities = client.capabilities().await.map_err(|err| err.to_string())?;
                Ok(RemoteServerProbeStatus::Reachable {
                    friendly_name: discovery.friendly_name,
                    version: discovery.version,
                    auth_required: discovery.auth_required,
                    api_version: capabilities.api_version,
                    media_range: capabilities.features.media_range,
                    events: capabilities.features.events,
                })
            })
        });

    match result {
        Ok(status) => status,
        Err(err) => RemoteServerProbeStatus::Failed(err),
    }
}

fn refresh_remote_cache_worker(
    server_id: String,
    api_base_url: String,
    token: String,
    requests: RemoteRefreshRequests,
    page_request: Option<(usize, usize)>,
) -> Result<RemoteCacheRefreshResult, RemoteCacheRefreshError> {
    let worker_result = tokio::runtime::Runtime::new()
        .map_err(|err| format!("Failed to start remote cache runtime: {err}"))
        .and_then(|rt| {
            rt.block_on(async move {
                let client =
                    sotf_audio_player::sotf_api_client::SotfApiClient::new(&api_base_url, &token)
                        .map_err(|err| err.to_string())?;
                let state = if requests.state {
                    Some(client.state().await.map_err(|err| err.to_string())?)
                } else {
                    None
                };
                let queue = if requests.queue {
                    Some(client.queue().await.map_err(|err| err.to_string())?)
                } else {
                    None
                };
                let album_page = if let Some((offset, limit)) = page_request {
                    Some(
                        client
                            .library_albums_page(offset, limit, None, Some("artist_title"))
                            .await
                            .map_err(|err| err.to_string())?,
                    )
                } else {
                    None
                };

                let mut artwork = Vec::new();
                if let Some(page) = album_page.as_ref() {
                    for album in &page.albums {
                        match client.album_artwork(&album.id).await {
                            Ok(bytes) => artwork.push((album.id.clone(), bytes)),
                            Err(err) => {
                                log::debug!(
                                    "[remote] Album artwork skipped for {}: {}",
                                    album.id,
                                    err
                                );
                            }
                        }
                    }
                }

                Ok(RemoteCacheRefreshResult {
                    server_id,
                    state,
                    queue,
                    album_page,
                    artwork,
                })
            })
        });

    worker_result.map_err(|message| RemoteCacheRefreshError { requests, message })
}
