//! LevelMeters mode event handlers
//!
//! When InputMode::LevelMeters, arrow keys navigate groups/controls and
//! m/s/d/c toggle mute/solo/dim/clear.  Unhandled keys fall through to
//! the shared key handler (screen switching, volume, help, etc.).

use super::PlayerCommand;
use crate::app::{App, InputMode, Screen};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(super) fn handle_level_meters_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        // Esc → return to Normal
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            None
        }
        // Tab → return to Normal on Library screen
        KeyCode::Tab => {
            app.input_mode = InputMode::Normal;
            app.current_screen = Screen::Library;
            None
        }
        // C → clear mutes/solos (overrides shared C→Configure)
        KeyCode::Char('C') => {
            app.clear_level_meter_mutes_and_solos();
            None
        }

        // Arrow navigation (plain — Shift/Ctrl arrows fall through to shared keys)
        KeyCode::Left
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::SHIFT) =>
        {
            app.select_previous_level_meter_group();
            None
        }
        KeyCode::Right
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::SHIFT) =>
        {
            app.select_next_level_meter_group();
            None
        }
        KeyCode::Up if !key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.select_previous_level_meter_control();
            None
        }
        KeyCode::Down if !key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.select_next_level_meter_control();
            None
        }

        // mute / solo / dim / clear
        KeyCode::Char('m') => {
            app.toggle_level_meter_mute();
            None
        }
        KeyCode::Char('s') => {
            app.toggle_level_meter_solo();
            None
        }
        KeyCode::Char('d') => {
            app.toggle_level_meter_dim();
            None
        }
        KeyCode::Char('c') => {
            app.clear_level_meter_mutes_and_solos();
            None
        }

        // Fallthrough: shared keys, then absorb
        _ => super::handle_shared_keys(app, key).unwrap_or(None),
    }
}
