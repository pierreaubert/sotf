//! Level-meter focus-mode event handling.

use super::PlayerCommand;
use crate::app::App;
use crate::ui::keybinding_catalog::{TuiCommand, TuiKeyContext, resolve_command};
use crossterm::event::KeyEvent;

pub(super) fn handle_level_meters_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    if let Some(command) = resolve_command(TuiKeyContext::LevelMeters, key) {
        let TuiCommand::Shared(command) = command else {
            unreachable!("non-shared command in LevelMeters context: {command:?}");
        };
        return super::dispatch_shared_command(app, key, command);
    }

    // Modifier-based root controls remain available while the meter pane is
    // focused; all other unrecognized keys are absorbed.
    super::handle_shared_keys(app, key).unwrap_or(None)
}
