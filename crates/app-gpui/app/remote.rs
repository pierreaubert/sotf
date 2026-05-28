//! Native SOTF remote server picker and discovery logic.

use std::sync::mpsc;
use std::time::Duration;

use crate::app::App;
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
        self.save_remote_server_store("save selected SOTF server")
    }

    pub fn remove_remote_server(&mut self, server_id: &str) -> bool {
        let removed = self.remote.server_store.remove(server_id);
        if removed.is_none() {
            return false;
        }
        self.save_remote_server_store("remove SOTF server")
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
