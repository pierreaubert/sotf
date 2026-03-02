use super::PlayerCommand;
use crate::app::{App, FilePickerMode, FilePickerOrigin};
use crossterm::event::{KeyCode, KeyEvent};
use sotf_audio_player::PluginSettings;

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
            if let Some(plugin) = app.plugin_chain.get_plugin_mut(app.selected_plugin_index)
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
            app.recording.mic_calibration_path = path_str;
            app.recording.editing_mic_cal = false;
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
    }
    app.close_file_explorer();
}
