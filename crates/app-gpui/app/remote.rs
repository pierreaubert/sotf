//! Native SOTF remote server picker and discovery logic.

use std::sync::mpsc;
use std::time::Duration;

use crate::app::App;
use crate::app::state::app::RemoteServerProbeStatus;
use crate::app::types::ToastMessage;

const DEFAULT_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(2);

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
        self.ui_state.toast_message = Some(ToastMessage::success("SOTF server saved."));
        Ok(id)
    }

    pub fn update_manual_remote_server_name(&mut self, name: impl Into<String>) {
        self.remote.set_manual_server_name(name);
    }

    pub fn update_manual_remote_server_url(&mut self, api_base_url: impl Into<String>) {
        self.remote.set_manual_api_base_url(api_base_url);
    }

    pub fn select_remote_server(&mut self, server_id: &str) -> bool {
        if !self.remote.server_store.select(server_id) {
            return false;
        }
        // Drop any existing event stream and start a new one for the selected server
        self.remote.event_stream_receiver = None;
        self.start_remote_event_stream();
        self.save_remote_server_store("save selected SOTF server")
    }

    pub fn remove_remote_server(&mut self, server_id: &str) -> bool {
        let removed = self.remote.server_store.remove(server_id);
        if removed.is_none() {
            return false;
        }
        self.remote.server_probe_statuses.remove(server_id);
        self.remote.server_tokens.remove(server_id);
        self.remote.event_stream_receiver = None;
        self.save_remote_server_store("remove SOTF server")
    }

    /// Cache a bearer token for a remote server (in-memory only).
    pub fn set_remote_server_token(
        &mut self,
        server_id: impl Into<String>,
        token: impl Into<String>,
    ) {
        self.remote
            .server_tokens
            .insert(server_id.into(), token.into());
    }

    /// Retrieve the cached bearer token for a remote server, if any.
    #[must_use]
    pub fn get_remote_server_token(&self, server_id: &str) -> Option<&str> {
        self.remote.server_tokens.get(server_id).map(String::as_str)
    }

    /// Remove the cached bearer token for a remote server.
    pub fn clear_remote_server_token(&mut self, server_id: &str) {
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
                    }
                    sotf_audio_player::sotf_api_client::SotfApiStreamEvent::Server(event) => {
                        match event {
                            sotf_audio_player::sotf_server_event::SotfServerEvent::PlaybackChanged
                            | sotf_audio_player::sotf_server_event::SotfServerEvent::QueueChanged { .. }
                            | sotf_audio_player::sotf_server_event::SotfServerEvent::VolumeChanged { .. } => {
                                // TODO: trigger remote state refresh or incremental UI update
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
