//! Headphone EQ optimization and management

use crate::app::AppState;
use crate::ui::PlayerView;
use gpui::*;
use std::path::PathBuf;
use std::sync::Arc;

impl PlayerView {
    /// Open file dialog to select headphone measurement file
    pub fn browse_headphone_curve(&mut self, cx: &mut Context<Self>) {
        // Use rfd for native file dialog
        let file_dialog = rfd::FileDialog::new()
            .add_filter("CSV Files", &["csv"])
            .add_filter("All Files", &["*"])
            .set_title("Select Headphone Measurement File");

        if let Some(path) = file_dialog.pick_file() {
            let path_str = path.display().to_string();
            self.state.update(cx, |state, _cx| {
                state.app.headphone_curve_path = path_str;
            });
            cx.notify();
        }
    }

    /// Run headphone EQ optimization
    pub fn run_headphone_optimization(&mut self, cx: &mut Context<Self>) {
        let state = self.state.read(cx);

        // Validate inputs
        if state.app.headphone_curve_path.is_empty() {
            self.state.update(cx, |state, _cx| {
                state.app.toast_message = Some(crate::app::ToastMessage::error(
                    "Please select a headphone measurement file",
                ));
            });
            cx.notify();
            return;
        }

        // TODO: Implement optimization
        // For now, just show a toast that it's not yet implemented
        self.state.update(cx, |state, _cx| {
            state.app.toast_message = Some(crate::app::ToastMessage::info(
                "Headphone EQ optimization will be implemented soon",
            ));
        });
        cx.notify();
    }

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
                // Parse the EQ file (format TBD - could be array of biquad filters)
                // For now, just show success
                self.state.update(cx, |state, _cx| {
                    state.app.toast_message = Some(crate::app::ToastMessage::success(
                        format!("Loaded EQ from: {}", path.display()),
                    ));
                });
                cx.notify();
            }
            Err(e) => {
                self.state.update(cx, |state, _cx| {
                    state.app.toast_message = Some(crate::app::ToastMessage::error(
                        format!("Failed to load EQ: {}", e),
                    ));
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
                    state.app.toast_message = Some(crate::app::ToastMessage::success(
                        format!("Deleted EQ file: {}", path.display()),
                    ));
                });
                cx.notify();
            }
            Err(e) => {
                self.state.update(cx, |state, _cx| {
                    state.app.toast_message = Some(crate::app::ToastMessage::error(
                        format!("Failed to delete EQ: {}", e),
                    ));
                });
                cx.notify();
            }
        }
    }
}
