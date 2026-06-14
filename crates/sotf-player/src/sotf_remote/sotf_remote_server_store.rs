use super::default::default_remote_server_store_version;
use super::misc::REMOTE_SERVER_STORE_VERSION;
use super::sotf_remote_server::SotfRemoteServer;
use crate::sotf_api_client::{SotfApiClientError, SotfApiResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SotfRemoteServerStore {
    #[serde(default = "default_remote_server_store_version")]
    pub version: u32,
    #[serde(default)]
    pub selected_server_id: Option<String>,
    #[serde(default)]
    pub servers: Vec<SotfRemoteServer>,
}

impl Default for SotfRemoteServerStore {
    fn default() -> Self {
        Self {
            version: REMOTE_SERVER_STORE_VERSION,
            selected_server_id: None,
            servers: Vec::new(),
        }
    }
}

impl SotfRemoteServerStore {
    pub fn load_from_path(path: impl AsRef<Path>) -> SotfApiResult<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(path)
            .map_err(|err| SotfApiClientError::InvalidConfig(err.to_string()))?;
        serde_json::from_str(&json).map_err(SotfApiClientError::Json)
    }

    pub fn save_to_path(&self, path: impl AsRef<Path>) -> SotfApiResult<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| SotfApiClientError::InvalidConfig(err.to_string()))?;
        }
        let json = serde_json::to_string_pretty(self).map_err(SotfApiClientError::Json)?;
        std::fs::write(path, json).map_err(|err| SotfApiClientError::InvalidConfig(err.to_string()))
    }

    pub fn upsert(&mut self, server: SotfRemoteServer) {
        if let Some(existing) = self
            .servers
            .iter_mut()
            .find(|existing| existing.id == server.id)
        {
            *existing = server;
        } else {
            self.servers.push(server);
        }
    }

    #[must_use]
    pub fn selected_server(&self) -> Option<&SotfRemoteServer> {
        let selected_id = self.selected_server_id.as_deref()?;
        self.servers.iter().find(|server| server.id == selected_id)
    }

    pub fn select(&mut self, server_id: impl Into<String>) -> bool {
        let server_id = server_id.into();
        let exists = self.servers.iter().any(|server| server.id == server_id);
        if exists {
            self.selected_server_id = Some(server_id);
        }
        exists
    }

    pub fn remove(&mut self, server_id: &str) -> Option<SotfRemoteServer> {
        let index = self
            .servers
            .iter()
            .position(|server| server.id == server_id)?;
        if self.selected_server_id.as_deref() == Some(server_id) {
            self.selected_server_id = None;
        }
        Some(self.servers.remove(index))
    }

    #[must_use]
    pub fn selected_token_secret_key(&self) -> Option<String> {
        self.selected_server()
            .map(SotfRemoteServer::token_secret_key)
    }
}
