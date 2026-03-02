use super::PlayerCommand;
use crate::app::{App, FilePickerMode, FilePickerOrigin, InputMode, MatrixEditMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use sotf_audio_player::{PluginSettings, PluginType};

pub(super) fn handle_add_plugin_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    let mut plugin_types = PluginType::all();
    plugin_types.sort_by_key(|p| p.name());
    let num_plugins = plugin_types.len();

    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            None
        }
        KeyCode::Enter => {
            // Add the selected plugin
            if let Some(plugin_type) = plugin_types.get(app.add_plugin_selected_index) {
                app.add_plugin(plugin_type);
            }
            app.input_mode = InputMode::Normal;
            None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.add_plugin_selected_index > 0 {
                app.add_plugin_selected_index -= 1;
            } else {
                app.add_plugin_selected_index = num_plugins.saturating_sub(1);
            }
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.add_plugin_selected_index + 1 < num_plugins {
                app.add_plugin_selected_index += 1;
            } else {
                app.add_plugin_selected_index = 0;
            }
            None
        }
        _ => None,
    }
}

pub(super) fn handle_plugins_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
            // Shift+Up: Move plugin up in the list
            app.move_plugin_up(app.selected_plugin_index);
            None
        }
        KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
            // Shift+Down: Move plugin down in the list
            app.move_plugin_down(app.selected_plugin_index);
            None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.select_previous_plugin();
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.select_next_plugin();
            None
        }
        KeyCode::Char('e') | KeyCode::Enter => {
            // Edit selected plugin
            app.enter_plugin_edit_mode();
            None
        }
        KeyCode::Char('s') => {
            // Save plugin chain
            app.input_mode = InputMode::SavePlugins;
            app.plugin_file_input.clear();
            // Refresh available presets to show in dialog
            app.refresh_plugin_presets();
            None
        }
        KeyCode::Char('l') => {
            // Load plugin chain
            app.input_mode = InputMode::LoadPlugins;
            app.plugin_file_input.clear();
            app.refresh_plugin_presets();
            None
        }
        KeyCode::Char('a') => {
            // Open plugin selection dialog
            app.add_plugin_selected_index = 0;
            app.input_mode = InputMode::AddPlugin;
            None
        }
        KeyCode::Char('t') => {
            // Toggle plugin enabled/disabled
            app.toggle_plugin(app.selected_plugin_index);
            None
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            app.remove_plugin(app.selected_plugin_index);
            None
        }
        KeyCode::Char('u') | KeyCode::Char('U') => {
            app.move_plugin_up(app.selected_plugin_index);
            None
        }
        KeyCode::Char('w') | KeyCode::Char('W') => {
            // Move plugin down (also available via Shift+Down)
            app.move_plugin_down(app.selected_plugin_index);
            None
        }
        _ => None,
    }
}

pub(super) fn handle_edit_plugin_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    // Check if we're editing a Matrix plugin
    let is_matrix = app
        .plugin_chain
        .get_plugin(app.selected_plugin_index)
        .is_some_and(|p| matches!(p.settings, PluginSettings::Matrix { .. }));

    if is_matrix {
        return handle_matrix_edit_mode(app, key);
    }

    // Standard plugin editing
    match key.code {
        KeyCode::Esc => {
            app.exit_plugin_edit_mode();
            None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.select_previous_param();
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.select_next_param();
            None
        }
        KeyCode::Left | KeyCode::Char('h') => {
            // Decrease parameter value
            if app.adjust_selected_param(-1.0) {
                app.request_plugin_update();
                None
            } else {
                None
            }
        }
        KeyCode::Right | KeyCode::Char('l') => {
            // Increase parameter value
            if app.adjust_selected_param(1.0) {
                app.request_plugin_update();
                None
            } else {
                None
            }
        }
        KeyCode::Char('[') => {
            // Large decrease
            if app.adjust_selected_param(-10.0) {
                app.request_plugin_update();
                None
            } else {
                None
            }
        }
        KeyCode::Char(']') => {
            // Large increase
            if app.adjust_selected_param(10.0) {
                app.request_plugin_update();
                None
            } else {
                None
            }
        }
        KeyCode::Char('a') => {
            // Load APO file (for EQ plugins)
            if let Some(plugin) = app.plugin_chain.get_plugin(app.selected_plugin_index) {
                if matches!(plugin.settings, PluginSettings::EQ { .. }) {
                    app.input_mode = InputMode::LoadApoFile;
                    app.status_message = Some("Enter path to APO file:".to_string());
                } else {
                    app.status_message =
                        Some("APO files can only be loaded for EQ plugins".to_string());
                }
            }
            None
        }
        KeyCode::Char('o') => {
            // Open SOFA file browser (for Binaural Decoder plugins)
            if let Some(plugin) = app.plugin_chain.get_plugin(app.selected_plugin_index) {
                if matches!(plugin.settings, PluginSettings::BinauralDecoder { .. }) {
                    app.open_file_explorer(
                        FilePickerOrigin::SofaFile,
                        FilePickerMode::File,
                        "Select SOFA File",
                        Some(&app.sofa_file_input.clone()),
                        Some("sofa"),
                    );
                } else {
                    app.status_message = Some(
                        "SOFA files can only be loaded for Binaural Decoder plugins".to_string(),
                    );
                }
            }
            None
        }
        KeyCode::Char('f') => {
            // Open IR file browser (for Convolution plugins)
            if let Some(plugin) = app.plugin_chain.get_plugin(app.selected_plugin_index) {
                if let PluginSettings::Convolution { ref ir_file, .. } = plugin.settings {
                    let current_path = ir_file.clone();
                    app.open_file_explorer(
                        FilePickerOrigin::IrFile,
                        FilePickerMode::File,
                        "Select Impulse Response (WAV)",
                        Some(&current_path),
                        Some("wav"),
                    );
                } else {
                    app.status_message =
                        Some("IR files can only be loaded for Convolution plugins".to_string());
                }
            }
            None
        }
        _ => None,
    }
}

/// Handle key events for Matrix plugin editing
fn handle_matrix_edit_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.exit_plugin_edit_mode();
            None
        }
        KeyCode::Tab => {
            // Toggle between Header and Grid mode
            app.matrix_edit_mode = match app.matrix_edit_mode {
                MatrixEditMode::Header => MatrixEditMode::Grid,
                MatrixEditMode::Grid => MatrixEditMode::Header,
            };
            None
        }
        _ => {
            // Delegate to mode-specific handler
            match app.matrix_edit_mode {
                MatrixEditMode::Header => handle_matrix_header_keys(app, key),
                MatrixEditMode::Grid => handle_matrix_grid_keys(app, key),
            }
        }
    }
}

/// Handle key events in Matrix header mode (input/output channels, preset)
fn handle_matrix_header_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            if app.matrix_header_selection > 0 {
                app.matrix_header_selection -= 1;
            }
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.matrix_header_selection < 2 {
                app.matrix_header_selection += 1;
            }
            None
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if app.adjust_matrix_header(-1) {
                app.request_plugin_update();
            }
            None
        }
        KeyCode::Right | KeyCode::Char('l') => {
            if app.adjust_matrix_header(1) {
                app.request_plugin_update();
            }
            None
        }
        _ => None,
    }
}

/// Handle key events in Matrix grid mode (cell editing)
fn handle_matrix_grid_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    // Get current matrix dimensions
    let (in_ch, out_ch) = app.get_matrix_dimensions().unwrap_or((2, 2));

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            if app.matrix_grid_row > 0 {
                app.matrix_grid_row -= 1;
            }
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.matrix_grid_row + 1 < out_ch {
                app.matrix_grid_row += 1;
            }
            None
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if app.matrix_grid_col > 0 {
                app.matrix_grid_col -= 1;
            }
            None
        }
        KeyCode::Right | KeyCode::Char('l') => {
            if app.matrix_grid_col + 1 < in_ch {
                app.matrix_grid_col += 1;
            }
            None
        }
        KeyCode::Char('-') | KeyCode::Char('[') => {
            // Decrease gain by 0.5 dB
            if app.adjust_matrix_cell(-0.5) {
                app.request_plugin_update();
            }
            None
        }
        KeyCode::Char('+') | KeyCode::Char('=') | KeyCode::Char(']') => {
            // Increase gain by 0.5 dB
            if app.adjust_matrix_cell(0.5) {
                app.request_plugin_update();
            }
            None
        }
        KeyCode::Char('0') => {
            // Set cell to zero (−∞ dB / silence)
            if app.set_matrix_cell(0.0) {
                app.request_plugin_update();
            }
            None
        }
        KeyCode::Char('1') => {
            // Set cell to unity gain (0 dB)
            if app.set_matrix_cell(1.0) {
                app.request_plugin_update();
            }
            None
        }
        _ => None,
    }
}

pub(super) fn handle_save_plugins_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.plugin_file_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Enter => {
            // If there are presets shown and input is empty, use selected preset (overwrite)
            if app.plugin_file_input.is_empty() && !app.available_plugin_presets.is_empty() {
                app.save_selected_preset();
            } else if !app.plugin_file_input.is_empty() {
                app.save_plugin_chain();
            }
            app.input_mode = InputMode::Normal;
            app.clear_autocomplete();
            None
        }
        KeyCode::Tab => {
            // Autocomplete from available presets (restricted to preset directory)
            if app.autocomplete_suggestions.is_empty() {
                app.generate_autocomplete_suggestions_for_save_preset();
                if !app.autocomplete_suggestions.is_empty() {
                    app.apply_autocomplete_to_plugin_file();
                }
            } else {
                app.next_autocomplete_for_plugin_file();
            }
            None
        }
        KeyCode::Up => {
            // Navigate preset list when input is empty
            if app.plugin_file_input.is_empty() && !app.available_plugin_presets.is_empty() {
                if app.selected_preset_index > 0 {
                    app.selected_preset_index -= 1;
                }
            }
            None
        }
        KeyCode::Down => {
            // Navigate preset list when input is empty
            if app.plugin_file_input.is_empty() && !app.available_plugin_presets.is_empty() {
                if app.selected_preset_index < app.available_plugin_presets.len() - 1 {
                    app.selected_preset_index += 1;
                }
            }
            None
        }
        KeyCode::Char(c) => {
            app.plugin_file_input.push(c);
            app.clear_autocomplete();
            None
        }
        KeyCode::Backspace => {
            app.plugin_file_input.pop();
            app.clear_autocomplete();
            None
        }
        _ => None,
    }
}

pub(super) fn handle_load_plugins_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.plugin_file_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Enter => {
            // If there are presets shown and input is empty, load selected preset
            if app.plugin_file_input.is_empty() && !app.available_plugin_presets.is_empty() {
                app.load_selected_preset();
            } else if !app.plugin_file_input.is_empty() {
                app.load_plugin_chain();
            }
            app.input_mode = InputMode::Normal;
            app.clear_autocomplete();
            None
        }
        KeyCode::Tab => {
            // Autocomplete file path (only if user typed something)
            if !app.plugin_file_input.is_empty() {
                if app.autocomplete_suggestions.is_empty() {
                    app.generate_autocomplete_suggestions_for_plugin_file();
                    if !app.autocomplete_suggestions.is_empty() {
                        app.apply_autocomplete_to_plugin_file();
                    }
                } else {
                    app.next_autocomplete_for_plugin_file();
                }
            }
            None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            // Navigate through presets
            if app.plugin_file_input.is_empty() {
                app.select_previous_preset();
            }
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            // Navigate through presets
            if app.plugin_file_input.is_empty() {
                app.select_next_preset();
            }
            None
        }
        KeyCode::Char(c) => {
            app.plugin_file_input.push(c);
            app.clear_autocomplete();
            None
        }
        KeyCode::Backspace => {
            app.plugin_file_input.pop();
            app.clear_autocomplete();
            None
        }
        _ => None,
    }
}

pub(super) fn handle_load_apo_file_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.apo_file_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Enter => {
            match app.load_apo_file() {
                Ok(()) => {
                    app.status_message = Some("APO file loaded successfully".to_string());
                    app.request_plugin_update();
                }
                Err(e) => {
                    app.status_message = Some(format!("Failed to load APO file: {}", e));
                }
            }
            app.input_mode = InputMode::Normal;
            app.apo_file_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Tab => {
            // Autocomplete file path
            if app.autocomplete_suggestions.is_empty() {
                app.generate_autocomplete_suggestions_for_apo_file();
                if !app.autocomplete_suggestions.is_empty() {
                    app.apply_autocomplete_to_apo_file();
                }
            } else {
                app.next_autocomplete_for_apo_file();
            }
            None
        }
        KeyCode::Char(c) => {
            app.apo_file_input.push(c);
            app.clear_autocomplete();
            None
        }
        KeyCode::Backspace => {
            app.apo_file_input.pop();
            app.clear_autocomplete();
            None
        }
        _ => None,
    }
}

pub(super) fn handle_load_sofa_file_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.sofa_file_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Enter => {
            match app.load_sofa_file() {
                Ok(()) => {
                    app.status_message = Some("SOFA file path set successfully".to_string());
                    app.request_plugin_update();
                }
                Err(e) => {
                    app.status_message = Some(format!("Failed to set SOFA file: {}", e));
                }
            }
            app.input_mode = InputMode::Normal;
            app.sofa_file_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Tab => {
            // Autocomplete file path
            if app.autocomplete_suggestions.is_empty() {
                app.generate_autocomplete_suggestions_for_sofa_file();
                if !app.autocomplete_suggestions.is_empty() {
                    app.apply_autocomplete_to_sofa_file();
                }
            } else {
                app.next_autocomplete_for_sofa_file();
            }
            None
        }
        KeyCode::Char(c) => {
            app.sofa_file_input.push(c);
            app.clear_autocomplete();
            None
        }
        KeyCode::Backspace => {
            app.sofa_file_input.pop();
            app.clear_autocomplete();
            None
        }
        _ => None,
    }
}
