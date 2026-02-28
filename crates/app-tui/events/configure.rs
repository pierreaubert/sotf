//! Configure screen event handlers
//!
//! Single entry point for the Configure screen. Manages the tab-bar navigation
//! and delegates to sub-screen handlers (Directories, Recording, RoomEQ,
//! HeadphoneEQ, SpinoramaEQ).

use crate::app::{App, ConfigureSubScreen, Screen};
use crossterm::event::{KeyCode, KeyEvent};
use super::PlayerCommand;

fn configure_sub_screen_prev(s: ConfigureSubScreen) -> ConfigureSubScreen {
    match s {
        ConfigureSubScreen::Directories  => ConfigureSubScreen::SpinoramaEq,
        ConfigureSubScreen::Recording    => ConfigureSubScreen::Directories,
        ConfigureSubScreen::RoomEq       => ConfigureSubScreen::Recording,
        ConfigureSubScreen::HeadphoneEq  => ConfigureSubScreen::RoomEq,
        ConfigureSubScreen::SpinoramaEq  => ConfigureSubScreen::HeadphoneEq,
    }
}

fn configure_sub_screen_next(s: ConfigureSubScreen) -> ConfigureSubScreen {
    match s {
        ConfigureSubScreen::Directories  => ConfigureSubScreen::Recording,
        ConfigureSubScreen::Recording    => ConfigureSubScreen::RoomEq,
        ConfigureSubScreen::RoomEq       => ConfigureSubScreen::HeadphoneEq,
        ConfigureSubScreen::HeadphoneEq  => ConfigureSubScreen::SpinoramaEq,
        ConfigureSubScreen::SpinoramaEq  => ConfigureSubScreen::Directories,
    }
}

pub fn handle_configure_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    // ── Esc handling ─────────────────────────────────────────────────────────
    if key.code == KeyCode::Esc {
        if app.configure_tab_focused {
            // At tab bar → leave Configure entirely
            app.current_screen = Screen::Library;
        } else if app.configure_sub_screen == ConfigureSubScreen::Directories {
            // Directories has no steps → go back to tab bar
            app.configure_tab_focused = true;
        } else {
            // Wizards handle their own Esc (step-back logic)
            return delegate_to_sub_handler(app, key);
        }
        return None;
    }

    // ── Tab-bar level ────────────────────────────────────────────────────────
    if app.configure_tab_focused {
        match key.code {
            KeyCode::Left | KeyCode::BackTab => {
                app.configure_sub_screen = configure_sub_screen_prev(app.configure_sub_screen);
            }
            KeyCode::Right | KeyCode::Tab => {
                app.configure_sub_screen = configure_sub_screen_next(app.configure_sub_screen);
            }
            KeyCode::Down | KeyCode::Enter => {
                app.configure_tab_focused = false;
            }
            KeyCode::Up => {
                app.current_screen = Screen::Library;
            }
            KeyCode::Char('1') => { app.configure_sub_screen = ConfigureSubScreen::Directories; }
            KeyCode::Char('2') => { app.configure_sub_screen = ConfigureSubScreen::Recording; }
            KeyCode::Char('3') => { app.configure_sub_screen = ConfigureSubScreen::RoomEq; }
            KeyCode::Char('4') => { app.configure_sub_screen = ConfigureSubScreen::HeadphoneEq; }
            KeyCode::Char('5') => { app.configure_sub_screen = ConfigureSubScreen::SpinoramaEq; }
            _ => {}
        }
        return None;
    }

    return delegate_to_sub_handler(app, key);
}

fn delegate_to_sub_handler(app: &mut App, key: KeyEvent) {
    match app.configure_sub_screen {
        ConfigureSubScreen::Directories => super::handle_directory_keys(app, key),
        ConfigureSubScreen::SpinoramaEq => super::spinorama::handle_spinorama_keys(app, key),
        ConfigureSubScreen::HeadphoneEq => super::headphone_eq::handle_headphone_eq_keys(app, key),
        ConfigureSubScreen::RoomEq      => super::room_eq::handle_room_eq_keys(app, key),
        ConfigureSubScreen::Recording   => super::recording::handle_recording_keys(app, key),
    }
}
