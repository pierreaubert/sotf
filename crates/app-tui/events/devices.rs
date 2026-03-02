use super::PlayerCommand;
use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent};

pub(super) fn handle_devices_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.select_previous_output_device();
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.select_next_output_device();
            None
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            // Apply device change
            app.get_selected_output_device()
                .map(|device| PlayerCommand::SetOutputDevice(device.name.clone()))
        }
        _ => None,
    }
}
