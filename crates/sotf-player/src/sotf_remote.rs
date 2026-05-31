//! Native SOTF remote connection state for mobile and desktop clients.

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::lan_discovery::DiscoveredSotfApiServer;
use crate::sotf_api_client::{
    SotfApiAlbumList, SotfApiCapabilities, SotfApiClient, SotfApiClientError,
    SotfApiCommandResponse, SotfApiQueue, SotfApiQueueEditResponse, SotfApiResult, SotfApiState,
    normalized_api_base_url,
};

const REMOTE_SERVER_STORE_VERSION: u32 = 1;

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

/// Non-secret metadata that can be persisted in config.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SotfRemoteServer {
    pub id: String,
    pub friendly_name: String,
    pub api_base_url: String,
    pub origin_url: String,
    pub host_name: Option<String>,
    pub address: Option<String>,
    pub port: u16,
    pub protocol: String,
    pub api_path: String,
    pub auth: String,
}

impl SotfRemoteServer {
    pub fn from_discovered(server: &DiscoveredSotfApiServer) -> SotfApiResult<Self> {
        let api_base_url = normalized_api_base_url(&server.api_base_url)?;
        Ok(Self {
            id: remote_server_id(&api_base_url),
            friendly_name: server.friendly_name.clone(),
            api_base_url,
            origin_url: server.origin_url.clone(),
            host_name: Some(server.host_name.clone()),
            address: Some(server.address.to_string()),
            port: server.port,
            protocol: server.protocol.clone(),
            api_path: server.api_path.clone(),
            auth: server.auth.clone(),
        })
    }

    pub fn manual(
        friendly_name: impl Into<String>,
        base_url: impl Into<String>,
    ) -> SotfApiResult<Self> {
        let api_base_url = normalized_api_base_url(base_url.into())?;
        let friendly_name = friendly_name.into();
        let url = reqwest::Url::parse(&api_base_url)
            .map_err(|err| SotfApiClientError::InvalidConfig(err.to_string()))?;
        let protocol = url.scheme().to_string();
        let host_name = url.host_str().map(ToString::to_string);
        let port = url.port_or_known_default().unwrap_or(0);
        let mut origin = url.clone();
        origin.set_path("");
        origin.set_query(None);
        origin.set_fragment(None);
        let origin_url = origin.as_str().trim_end_matches('/').to_string();
        Ok(Self {
            id: remote_server_id(&api_base_url),
            friendly_name: if friendly_name.trim().is_empty() {
                "SOTF Player".to_string()
            } else {
                friendly_name.trim().to_string()
            },
            api_base_url,
            origin_url,
            host_name,
            address: None,
            port,
            protocol,
            api_path: "/api/v1".to_string(),
            auth: "bearer".to_string(),
        })
    }

    pub fn connect(&self, token: &SotfRemoteAuthToken) -> SotfApiResult<SotfRemoteConnection> {
        let client = SotfApiClient::new(&self.api_base_url, token.as_str())?;
        Ok(SotfRemoteConnection {
            server: self.clone(),
            client,
        })
    }

    #[must_use]
    pub fn token_secret_key(&self) -> String {
        format!(
            "{}.remote.{}.bearer-token",
            crate::config::APP_BUNDLE_ID,
            self.id.strip_prefix("sotf:").unwrap_or(&self.id)
        )
    }

    /// Build a `sotf://pair` URL for QR-code pairing.
    #[must_use]
    pub fn pairing_url(&self, server_fingerprint: &str, nonce: &str) -> String {
        let host = self
            .host_name
            .as_deref()
            .or(self.address.as_deref())
            .unwrap_or("localhost");
        format!(
            "sotf://pair?host={host}&port={}&fingerprint={server_fingerprint}&nonce={nonce}",
            self.port
        )
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SotfRemoteAuthToken(String);

impl SotfRemoteAuthToken {
    pub fn new(token: impl Into<String>) -> SotfApiResult<Self> {
        let token = token.into();
        let token = token.trim();
        if token.is_empty() {
            return Err(SotfApiClientError::InvalidConfig(
                "remote auth token must not be empty".to_string(),
            ));
        }
        Ok(Self(token.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SotfRemoteAuthToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SotfRemoteAuthToken")
            .field(&redacted_token(&self.0))
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct SotfRemoteConnection {
    pub server: SotfRemoteServer,
    pub client: SotfApiClient,
}

impl SotfRemoteConnection {
    pub async fn validate(&self) -> SotfApiResult<SotfRemoteConnectionInfo> {
        let health = self.client.health().await?;
        let discovery = self.client.discovery().await?;
        let capabilities = self.client.capabilities().await?;
        let state = self.client.state().await?;
        Ok(SotfRemoteConnectionInfo {
            health,
            discovery,
            capabilities,
            state,
        })
    }

    pub async fn refresh(&self) -> SotfApiResult<SotfRemoteSnapshot> {
        let state = self.client.state().await?;
        let queue = self.client.queue().await?;
        let library = self.client.library_albums().await?;
        Ok(SotfRemoteSnapshot {
            state,
            queue,
            library,
        })
    }

    pub async fn transport(
        &self,
        command: SotfRemoteTransportCommand,
    ) -> SotfApiResult<SotfApiCommandResponse> {
        match command {
            SotfRemoteTransportCommand::Play => self.client.play().await,
            SotfRemoteTransportCommand::Pause => self.client.pause().await,
            SotfRemoteTransportCommand::Resume => self.client.resume().await,
            SotfRemoteTransportCommand::Stop => self.client.stop().await,
            SotfRemoteTransportCommand::Next => self.client.next().await,
            SotfRemoteTransportCommand::Previous => self.client.previous().await,
        }
    }

    pub async fn seek(&self, position_secs: f64) -> SotfApiResult<SotfApiCommandResponse> {
        self.client.seek(position_secs).await
    }

    pub async fn set_volume(&self, volume: u8) -> SotfApiResult<SotfApiCommandResponse> {
        self.client.set_volume(volume).await
    }

    pub async fn add_album_to_queue(
        &self,
        album_id: impl Into<String>,
        play_now: bool,
    ) -> SotfApiResult<SotfApiQueueEditResponse> {
        self.client.queue_add_album(album_id, play_now).await
    }

    pub async fn clear_queue(&self) -> SotfApiResult<SotfApiQueueEditResponse> {
        self.client.queue_clear().await
    }

    pub async fn delete_queue_item(&self, index: usize) -> SotfApiResult<SotfApiQueueEditResponse> {
        self.client.queue_delete(index).await
    }

    pub async fn jump_to_queue_item(
        &self,
        index: usize,
    ) -> SotfApiResult<SotfApiQueueEditResponse> {
        self.client.queue_jump(index).await
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SotfRemoteTransportCommand {
    Play,
    Pause,
    Resume,
    Stop,
    Next,
    Previous,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SotfRemoteConnectionInfo {
    pub health: crate::sotf_api_client::SotfApiHealth,
    pub discovery: crate::sotf_api_client::SotfApiDiscoveryInfo,
    pub capabilities: SotfApiCapabilities,
    pub state: SotfApiState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SotfRemoteSnapshot {
    pub state: SotfApiState,
    pub queue: SotfApiQueue,
    pub library: SotfApiAlbumList,
}

fn remote_server_id(api_base_url: &str) -> String {
    let digest = Sha256::digest(api_base_url.as_bytes());
    let mut out = String::from("sotf:");
    for byte in digest.iter().take(8) {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn redacted_token(token: &str) -> String {
    if token.is_empty() {
        "<empty>".to_string()
    } else {
        format!("<redacted:{}>", token.len())
    }
}

fn default_remote_server_store_version() -> u32 {
    REMOTE_SERVER_STORE_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::net::Ipv4Addr;

    fn discovered_server() -> DiscoveredSotfApiServer {
        DiscoveredSotfApiServer {
            instance_name: "Listening Room._sotf._tcp.local".to_string(),
            friendly_name: "Listening Room".to_string(),
            host_name: "listening-room.local".to_string(),
            address: Ipv4Addr::new(192, 168, 1, 23),
            port: 8732,
            protocol: "http".to_string(),
            api_path: "/api/v1".to_string(),
            auth: "bearer".to_string(),
            origin_url: "http://192.168.1.23:8732".to_string(),
            api_base_url: "http://192.168.1.23:8732/api/v1".to_string(),
            txt_records: BTreeMap::new(),
        }
    }

    #[test]
    fn discovered_server_maps_to_non_secret_remote_record() {
        let server = SotfRemoteServer::from_discovered(&discovered_server()).unwrap();
        assert_eq!(server.friendly_name, "Listening Room");
        assert_eq!(server.address.as_deref(), Some("192.168.1.23"));
        assert_eq!(server.api_base_url, "http://192.168.1.23:8732/api/v1");
        assert_eq!(server.auth, "bearer");
        assert!(!format!("{server:?}").contains("secret"));
    }

    #[test]
    fn manual_server_normalizes_base_url_and_fills_defaults() {
        let server = SotfRemoteServer::manual("  ", "http://host.local:8732/").unwrap();
        assert_eq!(server.friendly_name, "SOTF Player");
        assert_eq!(server.api_base_url, "http://host.local:8732/api/v1");
        assert_eq!(server.origin_url, "http://host.local:8732");
        assert_eq!(server.host_name.as_deref(), Some("host.local"));
        assert_eq!(server.port, 8732);
    }

    #[test]
    fn server_store_upserts_selects_and_removes_records() {
        let mut store = SotfRemoteServerStore::default();
        assert_eq!(store.version, REMOTE_SERVER_STORE_VERSION);
        let mut first = SotfRemoteServer::from_discovered(&discovered_server()).unwrap();
        let id = first.id.clone();
        store.upsert(first.clone());
        assert!(store.select(&id));
        assert_eq!(
            store.selected_server().unwrap().friendly_name,
            "Listening Room"
        );

        first.friendly_name = "Updated Room".to_string();
        store.upsert(first);
        assert_eq!(store.servers.len(), 1);
        assert_eq!(
            store.selected_server().unwrap().friendly_name,
            "Updated Room"
        );

        let removed = store.remove(&id).unwrap();
        assert_eq!(removed.id, id);
        assert!(store.selected_server().is_none());
    }

    #[test]
    fn server_store_serializes_without_token_material() {
        let mut store = SotfRemoteServerStore::default();
        store.upsert(SotfRemoteServer::from_discovered(&discovered_server()).unwrap());
        let json = serde_json::to_string(&store).unwrap();
        assert!(json.contains("Listening Room"));
        assert!(!json.contains("auth_token"));
        assert!(!json.contains("secret"));
    }

    #[test]
    fn server_store_deserializes_missing_version_as_current() {
        let store: SotfRemoteServerStore = serde_json::from_str(r#"{"servers":[]}"#).unwrap();
        assert_eq!(store.version, REMOTE_SERVER_STORE_VERSION);
        assert!(store.servers.is_empty());
    }

    #[test]
    fn server_store_round_trips_to_json_file_without_token_material() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("remote_servers.json");
        let mut store = SotfRemoteServerStore::default();
        let server = SotfRemoteServer::from_discovered(&discovered_server()).unwrap();
        let key = server.token_secret_key();
        store.selected_server_id = Some(server.id.clone());
        store.upsert(server);

        store.save_to_path(&path).unwrap();
        let json = std::fs::read_to_string(&path).unwrap();
        assert!(!json.contains("remote_servers.json"));
        assert!(json.contains("Listening Room"));
        assert!(!json.contains("very-secret-token"));
        assert!(!json.contains("auth_token"));
        assert!(!json.contains(&key));

        let loaded = SotfRemoteServerStore::load_from_path(&path).unwrap();
        assert_eq!(loaded, store);
        assert_eq!(
            loaded.selected_token_secret_key().as_deref(),
            Some(key.as_str())
        );
    }

    #[test]
    fn auth_token_debug_is_redacted() {
        let token = SotfRemoteAuthToken::new("very-secret-token").unwrap();
        assert_eq!(token.as_str(), "very-secret-token");
        let debug = format!("{token:?}");
        assert!(debug.contains("<redacted:17>"));
        assert!(!debug.contains("very-secret-token"));
        assert!(SotfRemoteAuthToken::new("  ").is_err());
    }

    #[test]
    fn connection_debug_redacts_client_token() {
        let server = SotfRemoteServer::from_discovered(&discovered_server()).unwrap();
        let token = SotfRemoteAuthToken::new("very-secret-token").unwrap();
        let connection = server.connect(&token).unwrap();
        let debug = format!("{connection:?}");
        assert!(debug.contains("SotfApiClient"));
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("very-secret-token"));
    }

    #[test]
    fn pairing_url_uses_host_name_and_port() {
        let server = SotfRemoteServer::from_discovered(&discovered_server()).unwrap();
        let url = server.pairing_url("FP:00", "NONCE1");
        assert!(url.starts_with("sotf://pair?"));
        assert!(url.contains("host=listening-room.local"));
        assert!(url.contains("port=8732"));
        assert!(url.contains("fingerprint=FP:00"));
        assert!(url.contains("nonce=NONCE1"));
    }

    #[test]
    fn pairing_url_falls_back_to_address() {
        let mut server = SotfRemoteServer::manual("Desk", "http://desk.local:8732").unwrap();
        server.host_name = None;
        server.address = Some("192.168.1.5".to_string());
        let url = server.pairing_url("FP:00", "NONCE1");
        assert!(url.contains("host=192.168.1.5"));
    }
}
