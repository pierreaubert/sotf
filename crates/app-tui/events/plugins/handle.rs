use super::super::PlayerCommand;
use super::misc::open_file_path_param;
use crate::app::{App, FilePickerMode, FilePickerOrigin, InputMode, MatrixEditMode};
use crate::ui::keybinding_catalog::{
    AddPluginCommand, PluginEditCommand, PluginListCommand, TuiCommand, TuiKeyContext,
    resolve_command,
};
use crossterm::event::{KeyCode, KeyEvent};
use sotf_audio_player::{PluginSettings, PluginType};

pub(in super::super) fn handle_add_plugin_mode(
    app: &mut App,
    key: KeyEvent,
) -> Option<PlayerCommand> {
    // Flatten the canonical category list into a selection order. Categories
    // are presented in `draw_available_plugins`; selection always lands on a
    // plugin (headers are non-selectable).
    let plugin_types: Vec<PluginType> = sotf_audio_player::plugin_categories::CATEGORIES
        .iter()
        .flat_map(|c| c.plugins.iter().cloned())
        .collect();
    let num_plugins = plugin_types.len();

    let command = match resolve_command(TuiKeyContext::AddPlugin, key) {
        Some(TuiCommand::AddPlugin(command)) => command,
        Some(command) => unreachable!("non-add-plugin command in AddPlugin context: {command:?}"),
        None => return None,
    };

    match command {
        AddPluginCommand::Cancel => {
            app.input_mode = InputMode::Normal;
            None
        }
        AddPluginCommand::Select => {
            if let Some(plugin_type) = plugin_types.get(app.plugin_rack.add_selected_index) {
                app.add_plugin(plugin_type);
            }
            app.input_mode = InputMode::Normal;
            None
        }
        AddPluginCommand::Navigate => {
            if matches!(key.code, KeyCode::Up | KeyCode::Char('k')) {
                if app.plugin_rack.add_selected_index > 0 {
                    app.plugin_rack.add_selected_index -= 1;
                } else {
                    app.plugin_rack.add_selected_index = num_plugins.saturating_sub(1);
                }
            } else {
                if app.plugin_rack.add_selected_index + 1 < num_plugins {
                    app.plugin_rack.add_selected_index += 1;
                } else {
                    app.plugin_rack.add_selected_index = 0;
                }
            }
            None
        }
    }
}

pub(in super::super) fn handle_plugins_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    let command = match resolve_command(TuiKeyContext::PluginList, key) {
        Some(TuiCommand::PluginList(command)) => command,
        Some(command) => unreachable!("non-plugin command in PluginList context: {command:?}"),
        None => return None,
    };

    match command {
        PluginListCommand::Navigate => {
            if matches!(key.code, KeyCode::Up | KeyCode::Char('k')) {
                app.select_previous_plugin();
            } else {
                app.select_next_plugin();
            }
            None
        }
        PluginListCommand::Add => {
            app.plugin_rack.add_selected_index = 0;
            app.input_mode = InputMode::AddPlugin;
            None
        }
        PluginListCommand::Edit => {
            app.enter_plugin_edit_mode();
            None
        }
        PluginListCommand::Toggle => {
            app.toggle_plugin(app.plugin_rack.selected_index);
            None
        }
        PluginListCommand::Remove => {
            app.remove_plugin(app.plugin_rack.selected_index);
            None
        }
        PluginListCommand::MoveUp => {
            app.move_plugin_up(app.plugin_rack.selected_index);
            None
        }
        PluginListCommand::MoveDown => {
            app.move_plugin_down(app.plugin_rack.selected_index);
            None
        }
        PluginListCommand::Save => {
            app.input_mode = InputMode::SavePlugins;
            app.plugin_rack.file_input.clear();
            app.refresh_plugin_presets();
            None
        }
        PluginListCommand::Load => {
            app.input_mode = InputMode::LoadPlugins;
            app.plugin_rack.file_input.clear();
            app.refresh_plugin_presets();
            None
        }
    }
}

pub(in super::super) fn handle_edit_plugin_mode(
    app: &mut App,
    key: KeyEvent,
) -> Option<PlayerCommand> {
    // Check if we're editing a Matrix plugin
    let is_matrix = app
        .plugin_rack
        .graph
        .get_plugin(app.plugin_rack.selected_index)
        .is_some_and(|p| matches!(p.settings, PluginSettings::Matrix { .. }));

    if is_matrix {
        return handle_matrix_edit_mode(app, key);
    }

    if let Some(command) = resolve_command(TuiKeyContext::PluginEdit, key) {
        let TuiCommand::PluginEdit(command) = command else {
            unreachable!("non-edit command in PluginEdit context: {command:?}");
        };
        return handle_documented_plugin_edit_command(app, key, command);
    }

    // Uncommon plugin-specific file actions that intentionally remain outside
    // the common editor help catalog.
    match key.code {
        KeyCode::Enter | KeyCode::Char('e') => {
            // Open file explorer for FilePath parameters
            if let Some(plugin) = app
                .plugin_rack
                .graph
                .get_plugin(app.plugin_rack.selected_index)
                && let Some(spec) = plugin
                    .settings
                    .param_specs()
                    .get(app.plugin_rack.param_selection)
                && matches!(
                    spec.param_type,
                    sotf_audio_player::param_specs::ParamType::FilePath
                )
            {
                open_file_path_param(app, spec.engine_key);
            }
            None
        }
        KeyCode::Char('f') => {
            // Open IR file browser (for Convolution plugins)
            if let Some(plugin) = app
                .plugin_rack
                .graph
                .get_plugin(app.plugin_rack.selected_index)
            {
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
                    app.ui.status_message =
                        Some("IR files can only be loaded for Convolution plugins".to_string());
                }
            }
            None
        }
        KeyCode::Char('A') => {
            // Load Path A config from preset file (for A/B Compare plugins)
            if let Some(plugin) = app
                .plugin_rack
                .graph
                .get_plugin(app.plugin_rack.selected_index)
                && matches!(plugin.settings, PluginSettings::ABCompare { .. })
            {
                let start = sotf_audio_player::config::get_plugin_presets_dir()
                    .map(|d| d.to_string_lossy().to_string());
                app.open_file_explorer(
                    FilePickerOrigin::ABConfigA,
                    FilePickerMode::File,
                    "Select Path A Config (JSON)",
                    start.as_deref(),
                    Some("json"),
                );
            }
            None
        }
        KeyCode::Char('B') => {
            // Load Path B config from preset file (for A/B Compare plugins)
            if let Some(plugin) = app
                .plugin_rack
                .graph
                .get_plugin(app.plugin_rack.selected_index)
                && matches!(plugin.settings, PluginSettings::ABCompare { .. })
            {
                let start = sotf_audio_player::config::get_plugin_presets_dir()
                    .map(|d| d.to_string_lossy().to_string());
                app.open_file_explorer(
                    FilePickerOrigin::ABConfigB,
                    FilePickerMode::File,
                    "Select Path B Config (JSON)",
                    start.as_deref(),
                    Some("json"),
                );
            }
            None
        }
        _ => None,
    }
}

fn handle_documented_plugin_edit_command(
    app: &mut App,
    key: KeyEvent,
    command: PluginEditCommand,
) -> Option<PlayerCommand> {
    match command {
        PluginEditCommand::Exit => {
            app.exit_plugin_edit_mode();
            None
        }
        PluginEditCommand::NavigateParameter => {
            if matches!(key.code, KeyCode::Up | KeyCode::Char('k')) {
                app.select_previous_param();
            } else {
                app.select_next_param();
            }
            None
        }
        PluginEditCommand::AdjustSmall => {
            let delta = if matches!(key.code, KeyCode::Left | KeyCode::Char('h')) {
                -1.0
            } else {
                1.0
            };
            if app.adjust_selected_param(delta) {
                app.request_plugin_update();
            }
            None
        }
        PluginEditCommand::AdjustLarge => {
            let delta = if key.code == KeyCode::Char('[') {
                -10.0
            } else {
                10.0
            };
            if app.adjust_selected_param(delta) {
                app.request_plugin_update();
            }
            None
        }
        PluginEditCommand::LoadApo => {
            if let Some(plugin) = app
                .plugin_rack
                .graph
                .get_plugin(app.plugin_rack.selected_index)
            {
                if matches!(plugin.settings, PluginSettings::EQ { .. }) {
                    app.input_mode = InputMode::LoadApoFile;
                    app.ui.status_message = Some("Enter path to APO file:".to_string());
                } else {
                    app.ui.status_message =
                        Some("APO files can only be loaded for EQ plugins".to_string());
                }
            }
            None
        }
        PluginEditCommand::LoadSofa => {
            if let Some(plugin) = app
                .plugin_rack
                .graph
                .get_plugin(app.plugin_rack.selected_index)
            {
                if matches!(plugin.settings, PluginSettings::BinauralDecoder { .. }) {
                    app.open_file_explorer(
                        FilePickerOrigin::SofaFile,
                        FilePickerMode::File,
                        "Select SOFA File",
                        Some(&app.plugin_rack.sofa_input.clone()),
                        Some("sofa"),
                    );
                } else {
                    app.ui.status_message = Some(
                        "SOFA files can only be loaded for Binaural Decoder plugins".to_string(),
                    );
                }
            }
            None
        }
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
            app.matrix.edit_mode = match app.matrix.edit_mode {
                MatrixEditMode::Header => MatrixEditMode::Grid,
                MatrixEditMode::Grid => MatrixEditMode::Header,
            };
            None
        }
        _ => {
            // Delegate to mode-specific handler
            match app.matrix.edit_mode {
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
            if app.matrix.header_selection > 0 {
                app.matrix.header_selection -= 1;
            }
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.matrix.header_selection < 2 {
                app.matrix.header_selection += 1;
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
            if app.matrix.grid_row > 0 {
                app.matrix.grid_row -= 1;
            }
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.matrix.grid_row + 1 < out_ch {
                app.matrix.grid_row += 1;
            }
            None
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if app.matrix.grid_col > 0 {
                app.matrix.grid_col -= 1;
            }
            None
        }
        KeyCode::Right | KeyCode::Char('l') => {
            if app.matrix.grid_col + 1 < in_ch {
                app.matrix.grid_col += 1;
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

pub(in super::super) fn handle_save_plugins_mode(
    app: &mut App,
    key: KeyEvent,
) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.plugin_rack.file_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Enter => {
            // If there are presets shown and input is empty, use selected preset (overwrite)
            if app.plugin_rack.file_input.is_empty()
                && !app.plugin_rack.available_presets.is_empty()
            {
                app.save_selected_preset();
            } else if !app.plugin_rack.file_input.is_empty() {
                app.save_plugins();
            }
            app.input_mode = InputMode::Normal;
            app.clear_autocomplete();
            None
        }
        KeyCode::Tab => {
            app.zsh_tab_complete(
                crate::app::app_autocomplete::get_plugin_file_input,
                crate::app::app_autocomplete::set_plugin_file_input,
                crate::app::app_autocomplete::AutocompleteKind::PresetName,
            );
            None
        }
        KeyCode::BackTab => {
            app.zsh_backtab_complete(crate::app::app_autocomplete::set_plugin_file_input);
            None
        }
        KeyCode::Up => {
            if !app.autocomplete_up(crate::app::app_autocomplete::set_plugin_file_input) {
                // Navigate preset list when input is empty
                if app.plugin_rack.file_input.is_empty()
                    && !app.plugin_rack.available_presets.is_empty()
                    && app.plugin_rack.selected_preset_index > 0
                {
                    app.plugin_rack.selected_preset_index -= 1;
                }
            }
            None
        }
        KeyCode::Down => {
            if !app.autocomplete_down(crate::app::app_autocomplete::set_plugin_file_input) {
                // Navigate preset list when input is empty
                if app.plugin_rack.file_input.is_empty()
                    && !app.plugin_rack.available_presets.is_empty()
                    && app.plugin_rack.selected_preset_index
                        < app.plugin_rack.available_presets.len() - 1
                {
                    app.plugin_rack.selected_preset_index += 1;
                }
            }
            None
        }
        KeyCode::Char(c) => {
            app.plugin_rack.file_input.push(c);
            app.refresh_autocomplete_inline(
                crate::app::app_autocomplete::get_plugin_file_input,
                crate::app::app_autocomplete::AutocompleteKind::PresetName,
            );
            None
        }
        KeyCode::Backspace => {
            app.plugin_rack.file_input.pop();
            app.refresh_autocomplete_inline(
                crate::app::app_autocomplete::get_plugin_file_input,
                crate::app::app_autocomplete::AutocompleteKind::PresetName,
            );
            None
        }
        _ => None,
    }
}

pub(in super::super) fn handle_load_plugins_mode(
    app: &mut App,
    key: KeyEvent,
) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.plugin_rack.file_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Enter => {
            // If there are presets shown and input is empty, load selected preset
            if app.plugin_rack.file_input.is_empty()
                && !app.plugin_rack.available_presets.is_empty()
            {
                app.load_selected_preset();
            } else if !app.plugin_rack.file_input.is_empty() {
                app.load_plugins();
            }
            app.input_mode = InputMode::Normal;
            app.clear_autocomplete();
            None
        }
        KeyCode::Tab => {
            if !app.plugin_rack.file_input.is_empty() {
                app.zsh_tab_complete(
                    crate::app::app_autocomplete::get_plugin_file_input,
                    crate::app::app_autocomplete::set_plugin_file_input,
                    crate::app::app_autocomplete::AutocompleteKind::FilePath,
                );
            }
            None
        }
        KeyCode::BackTab => {
            app.zsh_backtab_complete(crate::app::app_autocomplete::set_plugin_file_input);
            None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if !app.autocomplete_up(crate::app::app_autocomplete::set_plugin_file_input) {
                // Navigate through presets
                if app.plugin_rack.file_input.is_empty() {
                    app.select_previous_preset();
                }
            }
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.autocomplete_down(crate::app::app_autocomplete::set_plugin_file_input) {
                // Navigate through presets
                if app.plugin_rack.file_input.is_empty() {
                    app.select_next_preset();
                }
            }
            None
        }
        KeyCode::Char(c) => {
            app.plugin_rack.file_input.push(c);
            app.refresh_autocomplete_inline(
                crate::app::app_autocomplete::get_plugin_file_input,
                crate::app::app_autocomplete::AutocompleteKind::FilePath,
            );
            None
        }
        KeyCode::Backspace => {
            app.plugin_rack.file_input.pop();
            app.refresh_autocomplete_inline(
                crate::app::app_autocomplete::get_plugin_file_input,
                crate::app::app_autocomplete::AutocompleteKind::FilePath,
            );
            None
        }
        _ => None,
    }
}

pub(in super::super) fn handle_load_apo_file_mode(
    app: &mut App,
    key: KeyEvent,
) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.plugin_rack.apo_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Enter => {
            match app.load_apo_file() {
                Ok(()) => {
                    app.ui.status_message = Some("APO file loaded successfully".to_string());
                    app.request_plugin_update();
                }
                Err(e) => {
                    app.ui.status_message = Some(format!("Failed to load APO file: {}", e));
                }
            }
            app.input_mode = InputMode::Normal;
            app.plugin_rack.apo_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Tab => {
            app.zsh_tab_complete(
                crate::app::app_autocomplete::get_apo_file_input,
                crate::app::app_autocomplete::set_apo_file_input,
                crate::app::app_autocomplete::AutocompleteKind::FilePath,
            );
            None
        }
        KeyCode::BackTab => {
            app.zsh_backtab_complete(crate::app::app_autocomplete::set_apo_file_input);
            None
        }
        KeyCode::Down => {
            app.autocomplete_down(crate::app::app_autocomplete::set_apo_file_input);
            None
        }
        KeyCode::Up => {
            app.autocomplete_up(crate::app::app_autocomplete::set_apo_file_input);
            None
        }
        KeyCode::Char(c) => {
            app.plugin_rack.apo_input.push(c);
            app.refresh_autocomplete_inline(
                crate::app::app_autocomplete::get_apo_file_input,
                crate::app::app_autocomplete::AutocompleteKind::FilePath,
            );
            None
        }
        KeyCode::Backspace => {
            app.plugin_rack.apo_input.pop();
            app.refresh_autocomplete_inline(
                crate::app::app_autocomplete::get_apo_file_input,
                crate::app::app_autocomplete::AutocompleteKind::FilePath,
            );
            None
        }
        _ => None,
    }
}

pub(in super::super) fn handle_load_sofa_file_mode(
    app: &mut App,
    key: KeyEvent,
) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.plugin_rack.sofa_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Enter => {
            match app.load_sofa_file() {
                Ok(()) => {
                    app.ui.status_message = Some("SOFA file path set successfully".to_string());
                    app.request_plugin_update();
                }
                Err(e) => {
                    app.ui.status_message = Some(format!("Failed to set SOFA file: {}", e));
                }
            }
            app.input_mode = InputMode::Normal;
            app.plugin_rack.sofa_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Tab => {
            app.zsh_tab_complete(
                crate::app::app_autocomplete::get_sofa_file_input,
                crate::app::app_autocomplete::set_sofa_file_input,
                crate::app::app_autocomplete::AutocompleteKind::FilePath,
            );
            None
        }
        KeyCode::BackTab => {
            app.zsh_backtab_complete(crate::app::app_autocomplete::set_sofa_file_input);
            None
        }
        KeyCode::Down => {
            app.autocomplete_down(crate::app::app_autocomplete::set_sofa_file_input);
            None
        }
        KeyCode::Up => {
            app.autocomplete_up(crate::app::app_autocomplete::set_sofa_file_input);
            None
        }
        KeyCode::Char(c) => {
            app.plugin_rack.sofa_input.push(c);
            app.refresh_autocomplete_inline(
                crate::app::app_autocomplete::get_sofa_file_input,
                crate::app::app_autocomplete::AutocompleteKind::FilePath,
            );
            None
        }
        KeyCode::Backspace => {
            app.plugin_rack.sofa_input.pop();
            app.refresh_autocomplete_inline(
                crate::app::app_autocomplete::get_sofa_file_input,
                crate::app::app_autocomplete::AutocompleteKind::FilePath,
            );
            None
        }
        _ => None,
    }
}
