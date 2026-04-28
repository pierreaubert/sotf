use super::PlayerCommand;
use crate::app::{App, FilePickerMode, FilePickerOrigin};
use crossterm::event::{KeyCode, KeyEvent};
use sotf_audio_player::{PluginSettings, preset_file_to_path_config_json};

pub(super) fn handle_file_explorer_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.close_file_explorer();
            None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.file_explorer_select_prev();
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.file_explorer_select_next();
            None
        }
        KeyCode::Enter | KeyCode::Char('l') => {
            if let Some(path) = app.file_explorer_current().cloned() {
                if path.is_dir() {
                    match app.file_picker_mode {
                        FilePickerMode::Directory => {
                            // Enter selects this directory
                            apply_file_selection(app, path);
                        }
                        FilePickerMode::File => {
                            // Enter navigates into directory
                            app.file_explorer_enter_dir(path);
                        }
                    }
                } else {
                    // It's a file — select it
                    apply_file_selection(app, path);
                }
            }
            None
        }
        KeyCode::Right => {
            // Right always navigates into directory (even in Directory mode)
            if let Some(path) = app.file_explorer_current().cloned()
                && path.is_dir()
            {
                app.file_explorer_enter_dir(path);
            }
            None
        }
        KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => {
            app.file_explorer_go_parent();
            None
        }
        KeyCode::Char('H') => {
            // Toggle hidden files
            app.file_explorer_toggle_hidden();
            None
        }
        _ => None,
    }
}

fn apply_file_selection(app: &mut App, path: std::path::PathBuf) {
    let path_str = path.to_string_lossy().to_string();
    match app.file_picker_origin {
        FilePickerOrigin::SofaFile => {
            app.sofa_file_input = path_str;
            if let Err(e) = app.load_sofa_file() {
                app.status_message = Some(format!("Error: {}", e));
            } else {
                app.status_message = Some("SOFA file loaded".to_string());
                app.request_plugin_update();
            }
        }
        FilePickerOrigin::IrFile => {
            if let Some(plugin) = app.plugin_graph.get_plugin_mut(app.selected_plugin_index)
                && let PluginSettings::Convolution {
                    ref mut ir_file, ..
                } = plugin.settings
            {
                *ir_file = path_str;
                app.status_message = Some("IR file set".to_string());
                app.request_plugin_update();
            }
        }
        FilePickerOrigin::RecordingOutputDir => {
            app.recording.output_directory = path_str;
            app.recording.editing_output_dir = false;
        }
        FilePickerOrigin::RecordingMicCalibration => {
            // Write into the channel slot the user was editing.
            app.recording.set_active_mic_cal_path(path_str);
            app.recording.editing_mic_cal_channel = None;
        }
        FilePickerOrigin::RoomEqFilePath => {
            app.room_eq.file_path = path_str;
            app.room_eq.editing_file_path = false;
            super::conf_roomeq::load_room_eq_measurements(app);
        }
        FilePickerOrigin::RoomEqExportPath => {
            app.room_eq.export_path = path_str;
            app.room_eq.editing_export_path = false;
            super::conf_roomeq::export_room_eq_results(app);
        }
        FilePickerOrigin::HeadphoneMeasurement => {
            app.headphone_eq.measurement_path = path_str;
            app.headphone_eq.editing_measurement = false;
        }
        FilePickerOrigin::HeadphoneCustomTarget => {
            app.headphone_eq.custom_target_path = path_str;
            app.headphone_eq.editing_custom_target = false;
        }
        FilePickerOrigin::AddDirectory => {
            app.add_directory(path);
        }
        FilePickerOrigin::ApoFile => {
            app.apo_file_input = path_str;
            if let Err(e) = app.load_apo_file() {
                app.status_message = Some(format!("APO error: {}", e));
            } else {
                app.status_message = Some("APO file loaded".to_string());
                app.request_plugin_update();
            }
        }
        FilePickerOrigin::ABConfigA | FilePickerOrigin::ABConfigB => {
            let is_path_a = app.file_picker_origin == FilePickerOrigin::ABConfigA;
            match std::fs::read_to_string(&path) {
                Ok(json_content) => {
                    let sample_rate = app.get_current_sample_rate();
                    match preset_file_to_path_config_json(&json_content, sample_rate) {
                        Ok(path_config_json) => {
                            if let Some(plugin) =
                                app.plugin_graph.get_plugin_mut(app.selected_plugin_index)
                                && let PluginSettings::ABCompare {
                                    ref mut path_a_config,
                                    ref mut path_b_config,
                                    ref mut path_a_file,
                                    ref mut path_b_file,
                                    ..
                                } = plugin.settings
                            {
                                if is_path_a {
                                    *path_a_config = path_config_json;
                                    *path_a_file = path_str;
                                } else {
                                    *path_b_config = path_config_json;
                                    *path_b_file = path_str;
                                }
                            }
                            let filename = path.file_name().unwrap_or_default().to_string_lossy();
                            app.status_message = Some(format!("Config loaded from {}", filename));
                            app.request_plugin_update();
                        }
                        Err(e) => {
                            app.status_message = Some(format!("Invalid preset: {}", e));
                        }
                    }
                }
                Err(e) => {
                    app.status_message = Some(format!("Failed to read config: {}", e));
                }
            }
        }
        FilePickerOrigin::PlaylistImport => {
            if let Some(db) = app.library.get_database() {
                match app.playlist_controller.import_playlist(db, &path) {
                    Ok(()) => {
                        let name = path.file_stem().unwrap_or_default().to_string_lossy();
                        app.status_message = Some(format!("Imported playlist '{}'", name));
                        app.playlist_mode = crate::app::PlaylistMode::Tracks;
                    }
                    Err(e) => app.status_message = Some(format!("Import error: {}", e)),
                }
            }
        }
        FilePickerOrigin::PlaylistExport => {
            // Build the output file path: directory + playlist_name.m3u8
            let export_path = if path.is_dir() {
                let playlist_name = app
                    .playlist_controller
                    .active_playlist()
                    .map(|p| p.name.as_str())
                    .unwrap_or("playlist");
                // Sanitize name for filename (replace non-alphanumeric with _)
                let safe_name: String = playlist_name
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == '-' || c == ' ' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();
                path.join(format!("{}.m3u8", safe_name.trim()))
            } else {
                path.clone()
            };
            match app
                .playlist_controller
                .export_playlist(&app.library, &export_path)
            {
                Ok(()) => {
                    let name = export_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy();
                    app.status_message = Some(format!("Exported to '{}'", name));
                }
                Err(e) => app.status_message = Some(format!("Export error: {}", e)),
            }
        }
    }
    app.close_file_explorer();
}
