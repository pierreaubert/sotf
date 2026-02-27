//! Configure wizard event handlers
//!
//! Delegates to sub-screens (Spinorama, HeadphoneEQ, RoomEQ, Recording) and manages
//! the tab-bar navigation for the Configure screen.

use crate::app::{App, InputMode, Screen};
use crossterm::event::{KeyCode, KeyEvent};
use super::PlayerCommand;
use super::screens::handle_directory_keys;

fn configure_sub_screen_prev(s: crate::app::ConfigureSubScreen) -> crate::app::ConfigureSubScreen {
    use crate::app::ConfigureSubScreen;
    match s {
        ConfigureSubScreen::Directories  => ConfigureSubScreen::SpinoramaEq,
        ConfigureSubScreen::Recording    => ConfigureSubScreen::Directories,
        ConfigureSubScreen::RoomEq       => ConfigureSubScreen::Recording,
        ConfigureSubScreen::HeadphoneEq  => ConfigureSubScreen::RoomEq,
        ConfigureSubScreen::SpinoramaEq  => ConfigureSubScreen::HeadphoneEq,
    }
}

fn configure_sub_screen_next(s: crate::app::ConfigureSubScreen) -> crate::app::ConfigureSubScreen {
    use crate::app::ConfigureSubScreen;
    match s {
        ConfigureSubScreen::Directories  => ConfigureSubScreen::Recording,
        ConfigureSubScreen::Recording    => ConfigureSubScreen::RoomEq,
        ConfigureSubScreen::RoomEq       => ConfigureSubScreen::HeadphoneEq,
        ConfigureSubScreen::HeadphoneEq  => ConfigureSubScreen::SpinoramaEq,
        ConfigureSubScreen::SpinoramaEq  => ConfigureSubScreen::Directories,
    }
}

pub fn handle_configure_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use crate::app::ConfigureSubScreen;

    // Esc always exits Configure → Library, and resets focus to tab bar.
    if key.code == KeyCode::Esc {
        app.configure_tab_focused = true;
        app.current_screen = Screen::Library;
        return None;
    }

    // ── Tab-bar level (arrows cycle tabs, Down enters sub-screen) ──────────
    if app.configure_tab_focused {
        match key.code {
            KeyCode::Left => {
                app.configure_sub_screen = configure_sub_screen_prev(app.configure_sub_screen);
                return None;
            }
            KeyCode::Right => {
                app.configure_sub_screen = configure_sub_screen_next(app.configure_sub_screen);
                return None;
            }
            KeyCode::Down | KeyCode::Enter => {
                app.configure_tab_focused = false;
                return None;
            }
            KeyCode::Up => {
                app.current_screen = Screen::Library;
                return None;
            }
            // Number keys still jump directly to a tab
            KeyCode::Char('1') => { app.configure_sub_screen = ConfigureSubScreen::Directories;  return None; }
            KeyCode::Char('2') => { app.configure_sub_screen = ConfigureSubScreen::Recording;    return None; }
            KeyCode::Char('3') => { app.configure_sub_screen = ConfigureSubScreen::RoomEq;       return None; }
            KeyCode::Char('4') => { app.configure_sub_screen = ConfigureSubScreen::HeadphoneEq;  return None; }
            KeyCode::Char('5') => { app.configure_sub_screen = ConfigureSubScreen::SpinoramaEq;  return None; }
            _ => return None,
        }
    }

    // ── Inside a sub-screen ─────────────────────────────────────────────────
    // Up at the top of any sub-screen returns focus to the tab bar.
    if key.code == KeyCode::Up
        && app.configure_sub_screen != ConfigureSubScreen::SpinoramaEq
        && app.configure_sub_screen != ConfigureSubScreen::HeadphoneEq
        && app.configure_sub_screen != ConfigureSubScreen::RoomEq
        && app.configure_sub_screen != ConfigureSubScreen::Recording
    {
        app.configure_tab_focused = true;
        return None;
    }

    // Sub-screens get priority: delegate first, before number-key tab switching.
    // This prevents e.g. '1' in the Spinorama Select step from switching to Directories.
    match app.configure_sub_screen {
        ConfigureSubScreen::Directories => {
            // Number keys 1-5 still switch sub-screens from Directories
            match key.code {
                KeyCode::Char('1') => { app.configure_sub_screen = ConfigureSubScreen::Directories; return None; }
                KeyCode::Char('2') => { app.configure_sub_screen = ConfigureSubScreen::Recording; return None; }
                KeyCode::Char('3') => { app.configure_sub_screen = ConfigureSubScreen::RoomEq; return None; }
                KeyCode::Char('4') => { app.configure_sub_screen = ConfigureSubScreen::HeadphoneEq; return None; }
                KeyCode::Char('5') => { app.configure_sub_screen = ConfigureSubScreen::SpinoramaEq; return None; }
                _ => {}
            }
            return handle_directory_keys(app, key);
        }
        ConfigureSubScreen::SpinoramaEq => {
            // Inside Spinorama wizard, all keys go to the wizard handler.
            // Number keys do NOT switch sub-screens while inside the wizard.
            return super::spinorama::handle_spinorama_keys(app, key);
        }
        ConfigureSubScreen::HeadphoneEq => {
            // Number keys 1-5 switch sub-screens, BackTab returns to tab bar
            match key.code {
                KeyCode::Char('1') => { app.configure_sub_screen = ConfigureSubScreen::Directories; return None; }
                KeyCode::Char('2') => { app.configure_sub_screen = ConfigureSubScreen::Recording; return None; }
                KeyCode::Char('3') => { app.configure_sub_screen = ConfigureSubScreen::RoomEq; return None; }
                KeyCode::Char('4') => { app.configure_sub_screen = ConfigureSubScreen::HeadphoneEq; return None; }
                KeyCode::Char('5') => { app.configure_sub_screen = ConfigureSubScreen::SpinoramaEq; return None; }
                KeyCode::BackTab => { app.configure_tab_focused = true; return None; }
                _ => {}
            }
            return super::headphone_eq::handle_headphone_eq_keys(app, key);
        }
        ConfigureSubScreen::RoomEq => {
            // Number keys 1-5 switch sub-screens, BackTab returns to tab bar
            match key.code {
                KeyCode::Char('1') => { app.configure_sub_screen = ConfigureSubScreen::Directories; return None; }
                KeyCode::Char('2') => { app.configure_sub_screen = ConfigureSubScreen::Recording; return None; }
                KeyCode::Char('3') => { app.configure_sub_screen = ConfigureSubScreen::RoomEq; return None; }
                KeyCode::Char('4') => { app.configure_sub_screen = ConfigureSubScreen::HeadphoneEq; return None; }
                KeyCode::Char('5') => { app.configure_sub_screen = ConfigureSubScreen::SpinoramaEq; return None; }
                KeyCode::BackTab => { app.configure_tab_focused = true; return None; }
                _ => {}
            }
            return super::room_eq::handle_room_eq_keys(app, key);
        }
        ConfigureSubScreen::Recording => {
            // Number keys 1-5 switch sub-screens, BackTab returns to tab bar
            match key.code {
                KeyCode::Char('1') => { app.configure_sub_screen = ConfigureSubScreen::Directories; return None; }
                KeyCode::Char('2') => { app.configure_sub_screen = ConfigureSubScreen::Recording; return None; }
                KeyCode::Char('3') => { app.configure_sub_screen = ConfigureSubScreen::RoomEq; return None; }
                KeyCode::Char('4') => { app.configure_sub_screen = ConfigureSubScreen::HeadphoneEq; return None; }
                KeyCode::Char('5') => { app.configure_sub_screen = ConfigureSubScreen::SpinoramaEq; return None; }
                KeyCode::BackTab => { app.configure_tab_focused = true; return None; }
                _ => {}
            }
            return super::recording::handle_recording_keys(app, key);
        }
    }
}
