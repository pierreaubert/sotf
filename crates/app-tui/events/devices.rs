use super::PlayerCommand;
use crate::app::App;
use crate::ui::keybinding_catalog::{DeviceCommand, TuiCommand, TuiKeyContext, resolve_command};
use crossterm::event::{KeyCode, KeyEvent};

pub(super) fn handle_devices_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    let command = match resolve_command(TuiKeyContext::Devices, key) {
        Some(TuiCommand::Device(command)) => command,
        Some(command) => unreachable!("non-device command in Devices context: {command:?}"),
        None => return None,
    };

    match command {
        DeviceCommand::Navigate => {
            if matches!(key.code, KeyCode::Up | KeyCode::Char('k')) {
                app.select_previous_output_device();
            } else {
                app.select_next_output_device();
            }
            None
        }
        DeviceCommand::Select => app
            .get_selected_output_device()
            .map(|device| PlayerCommand::SetOutputDevice(device.name.clone())),
        DeviceCommand::Rescan => {
            app.reload_all_devices();
            app.ui.status_message = Some("Rescanning audio + cast devices…".to_string());
            app.ui.needs_redraw = true;
            None
        }
    }
}
