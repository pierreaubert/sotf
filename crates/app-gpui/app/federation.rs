//! Federation sources and server configuration business logic.

use crate::app::types::ToastMessage;
use crate::app::App;
use sotf_audio_player::federation_config::{FederationSourceEntry, SourceConnectionConfig};

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
}
