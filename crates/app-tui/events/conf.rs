//! Configure screen event handlers
//!
//! `handle_configure_mode` is the entry point for all Configure input modes
//! (tab bar + sub-screens).  It handles Esc/Tab/BackTab routing, delegates
//! to shared keys, then dispatches to the appropriate sub-handler.
//!
//! `handle_tab_bar_keys` handles tab-bar navigation when InputMode::Configure.

use super::PlayerCommand;
use crate::app::{App, ConfigureSubScreen, InputMode, Screen};
use crossterm::event::{KeyCode, KeyEvent};

fn configure_sub_screen_prev(s: ConfigureSubScreen) -> ConfigureSubScreen {
    match s {
        ConfigureSubScreen::Directories => ConfigureSubScreen::SpinoramaEq,
        ConfigureSubScreen::Recording => ConfigureSubScreen::Directories,
        ConfigureSubScreen::RoomEq => ConfigureSubScreen::Recording,
        ConfigureSubScreen::HeadphoneEq => ConfigureSubScreen::RoomEq,
        ConfigureSubScreen::SpinoramaEq => ConfigureSubScreen::HeadphoneEq,
    }
}

fn configure_sub_screen_next(s: ConfigureSubScreen) -> ConfigureSubScreen {
    match s {
        ConfigureSubScreen::Directories => ConfigureSubScreen::Recording,
        ConfigureSubScreen::Recording => ConfigureSubScreen::RoomEq,
        ConfigureSubScreen::RoomEq => ConfigureSubScreen::HeadphoneEq,
        ConfigureSubScreen::HeadphoneEq => ConfigureSubScreen::SpinoramaEq,
        ConfigureSubScreen::SpinoramaEq => ConfigureSubScreen::Directories,
    }
}

/// Entry point for all Configure input modes (called from handle_key_event).
pub(super) fn handle_configure_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    // Esc / Tab / BackTab have configure-wide routing
    match key.code {
        KeyCode::Esc => {
            return match app.input_mode {
                InputMode::Configure => {
                    // At tab bar → leave Configure entirely
                    app.current_screen = Screen::Library;
                    app.input_mode = InputMode::Normal;
                    None
                }
                InputMode::ConfigureDirectories if app.editing_directory => {
                    // Editing text → let directory handler cancel editing
                    super::conf_directories::handle_directory_keys(app, key)
                }
                InputMode::ConfigureDirectories => {
                    // Directories has no steps → go back to tab bar
                    app.input_mode = InputMode::Configure;
                    None
                }
                // Wizards handle their own Esc (step-back logic)
                InputMode::ConfigureRecording => {
                    super::conf_recordings::handle_recording_keys(app, key)
                }
                InputMode::ConfigureRoomEq => super::conf_roomeq::handle_room_eq_keys(app, key),
                InputMode::ConfigureHeadphoneEq => {
                    super::conf_headphoneeq::handle_headphone_eq_keys(app, key)
                }
                InputMode::ConfigureSpinoramaEq => {
                    super::conf_spinoramaeq::handle_spinorama_keys(app, key)
                }
                _ => None,
            };
        }
        KeyCode::Tab | KeyCode::BackTab => {
            return match app.input_mode {
                InputMode::Configure => handle_tab_bar_keys(app, key),
                InputMode::ConfigureDirectories if app.editing_directory => {
                    super::conf_directories::handle_directory_keys(app, key)
                }
                InputMode::ConfigureRecording => {
                    super::conf_recordings::handle_recording_keys(app, key)
                }
                InputMode::ConfigureRoomEq => super::conf_roomeq::handle_room_eq_keys(app, key),
                InputMode::ConfigureHeadphoneEq => {
                    super::conf_headphoneeq::handle_headphone_eq_keys(app, key)
                }
                InputMode::ConfigureSpinoramaEq => {
                    super::conf_spinoramaeq::handle_spinorama_keys(app, key)
                }
                _ => None,
            };
        }
        _ => {}
    }

    // Try shared keys
    if let Some(cmd) = super::handle_shared_keys(app, key) {
        return cmd;
    }

    // Dispatch remaining keys to the appropriate sub-handler
    match app.input_mode {
        InputMode::Configure => handle_tab_bar_keys(app, key),
        InputMode::ConfigureDirectories => super::conf_directories::handle_directory_keys(app, key),
        InputMode::ConfigureRecording => super::conf_recordings::handle_recording_keys(app, key),
        InputMode::ConfigureRoomEq => super::conf_roomeq::handle_room_eq_keys(app, key),
        InputMode::ConfigureHeadphoneEq => {
            super::conf_headphoneeq::handle_headphone_eq_keys(app, key)
        }
        InputMode::ConfigureSpinoramaEq => super::conf_spinoramaeq::handle_spinorama_keys(app, key),
        _ => None,
    }
}

/// Handle keys when InputMode::Configure (tab bar is focused).
pub(super) fn handle_tab_bar_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    fn enter_sub_screen(app: &mut App, sub: ConfigureSubScreen) {
        app.configure_sub_screen = sub;
        app.input_mode = InputMode::from_configure_sub_screen(sub);
        if sub == ConfigureSubScreen::RoomEq
            && app.room_eq.step == sotf_audio_player::room_eq_types::RoomEqStep::LoadData
        {
            super::conf_roomeq::auto_open_load_data(app);
        }
    }

    match key.code {
        KeyCode::Left | KeyCode::Up => {
            app.configure_sub_screen = configure_sub_screen_prev(app.configure_sub_screen);
        }
        KeyCode::Right | KeyCode::Down | KeyCode::Tab => {
            app.configure_sub_screen = configure_sub_screen_next(app.configure_sub_screen);
        }
        KeyCode::BackTab => {
            app.configure_sub_screen = configure_sub_screen_prev(app.configure_sub_screen);
        }
        KeyCode::Enter => {
            enter_sub_screen(app, app.configure_sub_screen);
        }
        KeyCode::Char('1') => enter_sub_screen(app, ConfigureSubScreen::Directories),
        KeyCode::Char('2') => enter_sub_screen(app, ConfigureSubScreen::Recording),
        KeyCode::Char('3') => enter_sub_screen(app, ConfigureSubScreen::RoomEq),
        KeyCode::Char('4') => enter_sub_screen(app, ConfigureSubScreen::HeadphoneEq),
        KeyCode::Char('5') => enter_sub_screen(app, ConfigureSubScreen::SpinoramaEq),
        _ => {}
    }
    None
}
