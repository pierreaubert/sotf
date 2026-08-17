use super::FederationMode;
use super::federation_edit_state::FederationEditState;
use super::service_login_state::ServiceLoginState;
use sotf_audio_player::federation_config::{ConnectionStatus, FederationSourceEntry};
use std::collections::HashMap;

/// TUI state for the Federation Sources configuration screen.
#[derive(Debug, Clone)]
pub struct FederationTuiState {
    pub sources: Vec<FederationSourceEntry>,
    pub statuses: HashMap<String, ConnectionStatus>,
    pub selected_idx: usize,
    pub mode: FederationMode,
    pub edit: Option<FederationEditState>,
    /// In-progress Tidal/Spotify login, if any (see
    /// `events::conf_federation::poll_service_login`).
    pub login: Option<ServiceLoginState>,
}

impl Default for FederationTuiState {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            statuses: HashMap::new(),
            selected_idx: 0,
            mode: FederationMode::List,
            edit: None,
            login: None,
        }
    }
}
