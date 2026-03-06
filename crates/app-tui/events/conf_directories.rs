use super::PlayerCommand;
use crate::app::{App, FilePickerMode, FilePickerOrigin};
use crossterm::event::{KeyCode, KeyEvent};

pub(super) fn handle_directory_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    if app.editing_directory {
        return handle_directory_text_input(app, key);
    }

    const PAGE_SIZE: usize = 20;

    match key.code {
        KeyCode::Char('a') | KeyCode::F(2) => {
            app.editing_directory = true;
            app.directory_input.clear();
            None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.select_previous_directory();
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.select_next_directory();
            None
        }
        KeyCode::PageUp => {
            app.page_up_directories(PAGE_SIZE);
            None
        }
        KeyCode::PageDown => {
            app.page_down_directories(PAGE_SIZE);
            None
        }
        KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
            // Toggle directory expansion to show/hide subdirectories
            app.toggle_directory_expansion();
            None
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            app.remove_selected_directory();
            None
        }
        KeyCode::Char('s') => {
            app.start_library_scan();
            None
        }
        KeyCode::Char('m') => {
            // Maintenance: clean up database
            // The method handles all progress tracking and status messages
            let _ = app.clean_library_database();
            None
        }
        KeyCode::Char('r') => {
            // Start ReplayGain scan for tracks missing data
            if let Err(e) = app.start_replay_gain_scan() {
                app.status_message = Some(format!("Error starting ReplayGain scan: {}", e));
            }
            None
        }
        KeyCode::Char('R') => {
            // Force ReplayGain rescan of all tracks
            if let Err(e) = app.start_force_replay_gain_scan() {
                app.status_message = Some(format!("Error starting ReplayGain force scan: {}", e));
            }
            None
        }
        KeyCode::Char('b') => {
            // Start Bliss audio analysis scan
            if let Err(e) = app.start_bliss_scan() {
                app.status_message = Some(format!("Error starting Bliss scan: {}", e));
            }
            None
        }
        KeyCode::Char('B') => {
            // Force Bliss rescan of all tracks
            if let Err(e) = app.start_force_bliss_scan() {
                app.status_message = Some(format!("Error starting Bliss force scan: {}", e));
            }
            None
        }
        KeyCode::Char('w') => {
            // Start waveform scan for tracks missing data
            if let Err(e) = app.start_waveform_scan() {
                app.status_message = Some(format!("Error starting Waveform scan: {}", e));
            }
            None
        }
        KeyCode::Char('W') => {
            // Force waveform rescan of all tracks
            if let Err(e) = app.start_force_waveform_scan() {
                app.status_message = Some(format!("Error starting Waveform force scan: {}", e));
            }
            None
        }
        KeyCode::Char('S') => {
            // Force rescan all files (ignores modification time, preserves ReplayGain)
            app.start_force_library_scan();
            None
        }
        _ => None,
    }
}

fn handle_directory_text_input(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.editing_directory = false;
            app.directory_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Enter => {
            if !app.directory_input.is_empty() {
                let path = std::path::PathBuf::from(&app.directory_input);
                app.add_directory(path);
                app.directory_input.clear();
            }
            app.editing_directory = false;
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
            let start = app.directory_input.clone();
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
        KeyCode::Char(c) => {
            app.directory_input.push(c);
            app.refresh_autocomplete_inline(
                crate::app::app_autocomplete::get_directory_input,
                crate::app::app_autocomplete::AutocompleteKind::FilePath,
            );
            None
        }
        KeyCode::Backspace => {
            app.directory_input.pop();
            app.refresh_autocomplete_inline(
                crate::app::app_autocomplete::get_directory_input,
                crate::app::app_autocomplete::AutocompleteKind::FilePath,
            );
            None
        }
        _ => None,
    }
}
