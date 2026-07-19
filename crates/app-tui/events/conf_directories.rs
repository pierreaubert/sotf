use super::PlayerCommand;
use crate::app::{App, FilePickerMode, FilePickerOrigin};
use crate::ui::keybinding_catalog::{DirectoryCommand, TuiCommand, TuiKeyContext, resolve_command};
use crossterm::event::{KeyCode, KeyEvent};

pub(super) fn handle_directory_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    if app.library_view.editing_directory {
        return handle_directory_text_input(app, key);
    }

    if let Some(command) = resolve_command(TuiKeyContext::Directories, key) {
        let TuiCommand::Directory(command) = command else {
            unreachable!("non-directory command in Directories context: {command:?}");
        };
        return handle_documented_command(app, key, command);
    }

    handle_undocumented_command(app, key)
}

fn handle_documented_command(
    app: &mut App,
    key: KeyEvent,
    command: DirectoryCommand,
) -> Option<PlayerCommand> {
    match command {
        DirectoryCommand::Navigate => {
            if matches!(key.code, KeyCode::Up | KeyCode::Char('k')) {
                app.select_previous_directory();
            } else {
                app.select_next_directory();
            }
            None
        }
        DirectoryCommand::Add => {
            app.library_view.editing_directory = true;
            app.library_view.directory_input.clear();
            None
        }
        DirectoryCommand::Remove => {
            app.remove_selected_directory();
            None
        }
        DirectoryCommand::Scan => {
            app.start_library_scan();
            None
        }
        DirectoryCommand::ForceScan => {
            app.start_force_library_scan();
            None
        }
        DirectoryCommand::Maintenance => {
            let _ = app.clean_library_database();
            None
        }
        DirectoryCommand::ReplayGain => {
            if let Err(error) = app.start_replay_gain_scan() {
                app.ui.status_message = Some(format!("Error starting ReplayGain scan: {error}"));
            }
            None
        }
    }
}

fn handle_undocumented_command(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    const PAGE_SIZE: usize = 20;

    match key.code {
        KeyCode::PageUp => {
            app.page_up_directories(PAGE_SIZE);
            None
        }
        KeyCode::PageDown => {
            app.page_down_directories(PAGE_SIZE);
            None
        }
        KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
            app.toggle_directory_expansion();
            None
        }
        KeyCode::Char('R') => {
            if let Err(error) = app.start_force_replay_gain_scan() {
                app.ui.status_message =
                    Some(format!("Error starting ReplayGain force scan: {error}"));
            }
            None
        }
        KeyCode::Char('b') => {
            if let Err(error) = app.start_bliss_scan() {
                app.ui.status_message = Some(format!("Error starting Bliss scan: {error}"));
            }
            None
        }
        KeyCode::Char('B') => {
            if let Err(error) = app.start_force_bliss_scan() {
                app.ui.status_message = Some(format!("Error starting Bliss force scan: {error}"));
            }
            None
        }
        KeyCode::Char('w') => {
            if let Err(error) = app.start_waveform_scan() {
                app.ui.status_message = Some(format!("Error starting Waveform scan: {error}"));
            }
            None
        }
        KeyCode::Char('W') => {
            if let Err(error) = app.start_force_waveform_scan() {
                app.ui.status_message =
                    Some(format!("Error starting Waveform force scan: {error}"));
            }
            None
        }
        _ => None,
    }
}

fn handle_directory_text_input(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.library_view.editing_directory = false;
            app.library_view.directory_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Enter => {
            if !app.library_view.directory_input.is_empty() {
                let path = std::path::PathBuf::from(&app.library_view.directory_input);
                app.add_directory(path);
                app.library_view.directory_input.clear();
            }
            app.library_view.editing_directory = false;
            app.clear_autocomplete();
            None
        }
        KeyCode::Tab => {
            app.zsh_tab_complete(
                crate::app::app_autocomplete::get_directory_input,
                crate::app::app_autocomplete::set_directory_input,
                crate::app::app_autocomplete::AutocompleteKind::FilePath,
            );
            None
        }
        KeyCode::BackTab => {
            app.zsh_backtab_complete(crate::app::app_autocomplete::set_directory_input);
            None
        }
        KeyCode::F(2) => {
            let start = app.library_view.directory_input.clone();
            app.open_file_explorer(
                FilePickerOrigin::AddDirectory,
                FilePickerMode::Directory,
                "Select Music Directory",
                Some(&start),
                None,
            );
            None
        }
        KeyCode::Down => {
            app.autocomplete_down(crate::app::app_autocomplete::set_directory_input);
            None
        }
        KeyCode::Up => {
            app.autocomplete_up(crate::app::app_autocomplete::set_directory_input);
            None
        }
        KeyCode::Char(character) => {
            app.library_view.directory_input.push(character);
            app.refresh_autocomplete_inline(
                crate::app::app_autocomplete::get_directory_input,
                crate::app::app_autocomplete::AutocompleteKind::FilePath,
            );
            None
        }
        KeyCode::Backspace => {
            app.library_view.directory_input.pop();
            app.refresh_autocomplete_inline(
                crate::app::app_autocomplete::get_directory_input,
                crate::app::app_autocomplete::AutocompleteKind::FilePath,
            );
            None
        }
        _ => None,
    }
}
