//! Headphone EQ optimization and management - GPUI frontend
//!
//! This module provides GPUI-specific UI interactions for headphone EQ optimization.
//! The actual optimization logic is in sotf_audio_player::autoeq::headphone.

use crate::app::types::PluginUpdateType;
use crate::ui::PlayerView;
use gpui::*;
use std::path::PathBuf;

// Re-export the result type from the common library
pub use sotf_audio_player::autoeq::HeadphoneOptimizationResult;

impl PlayerView {
    /// List saved EQ files
    pub fn list_saved_eq_files(&self) -> Vec<PathBuf> {
        if let Some(eq_dir) = sotf_audio_player::config::get_eq_dir() {
            if let Ok(entries) = std::fs::read_dir(&eq_dir) {
                return entries
                    .filter_map(|entry| entry.ok())
                    .filter(|entry| {
                        entry.path().extension().and_then(|s| s.to_str()) == Some("json")
                    })
                    .map(|entry| entry.path())
                    .collect();
            }
        }
        Vec::new()
    }

    /// Load EQ from file and apply to plugin chain
    pub fn load_headphone_eq(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match std::fs::read_to_string(&path) {
            Ok(json) => {
                // Parse the EQ file as array of biquad filters
                match serde_json::from_str::<Vec<autoeq_iir::Biquad>>(&json) {
                    Ok(biquads) => {
                        // TODO: Apply biquads to plugin chain
                        log::info!("Loaded {} biquad filters from {:?}", biquads.len(), path);
                        self.state.update(cx, |state, _cx| {
                            state.app.toast_message =
                                Some(crate::app::ToastMessage::success(format!(
                                    "Loaded {} filters from: {}",
                                    biquads.len(),
                                    path.display()
                                )));
                        });
                        cx.notify();
                    }
                    Err(e) => {
                        self.state.update(cx, |state, _cx| {
                            state.app.toast_message = Some(crate::app::ToastMessage::error(
                                format!("Failed to parse EQ file: {}", e),
                            ));
                        });
                        cx.notify();
                    }
                }
            }
            Err(e) => {
                self.state.update(cx, |state, _cx| {
                    state.app.toast_message = Some(crate::app::ToastMessage::error(format!(
                        "Failed to load EQ: {}",
                        e
                    )));
                });
                cx.notify();
            }
        }
    }

    /// Delete saved EQ file
    pub fn delete_headphone_eq(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match std::fs::remove_file(&path) {
            Ok(_) => {
                self.state.update(cx, |state, _cx| {
                    state.app.toast_message = Some(crate::app::ToastMessage::success(format!(
                        "Deleted EQ file: {}",
                        path.display()
                    )));
                });
                cx.notify();
            }
            Err(e) => {
                self.state.update(cx, |state, _cx| {
                    state.app.toast_message = Some(crate::app::ToastMessage::error(format!(
                        "Failed to delete EQ: {}",
                        e
                    )));
                });
                cx.notify();
            }
        }
    }

    /// Clear the headphone EQ from the playback chain
    pub fn clear_headphone_eq_from_playback(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Find and remove EQ plugins
            let plugins = state.app.plugin_chain.plugins();
            let eq_indices: Vec<_> = plugins
                .iter()
                .enumerate()
                .filter_map(|(i, p)| {
                    if matches!(p.plugin_type(), sotf_audio_player::PluginType::EQ) {
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect();

            // Remove in reverse order to maintain correct indices
            for idx in eq_indices.into_iter().rev() {
                state.app.plugin_chain.remove_plugin(idx);
            }

            state.app.pending_plugin_update = Some(PluginUpdateType::Structural);
            state.app.toast_message = Some(crate::app::ToastMessage::success(
                "Cleared EQ from playback",
            ));
        });
        cx.notify();
    }
}
