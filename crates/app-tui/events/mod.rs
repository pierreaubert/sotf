use crate::app::{App, InputMode, Screen};
use crate::media_controls::TuiMediaControls;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use souvlaki::MediaControlEvent;
use std::time::Duration;

pub mod devices;
pub mod file_explorer;
pub mod level_meters;
pub mod library;
pub mod media_control;
pub mod playlists;
pub mod plugins;
pub mod queue;
pub mod search;

pub mod conf;
pub mod conf_directories;
pub mod conf_federation;
pub mod conf_headphoneeq;
pub mod conf_recordings;
pub mod conf_roomeq;
pub mod conf_servers;
pub mod conf_spinoramaeq;

pub use conf_federation::{poll_federation_scan, poll_federation_test};
pub use conf_headphoneeq::{
    poll_headphone_download, poll_headphone_eq_optimization, poll_headphone_list_load,
};
pub use conf_recordings::{poll_probe_capture, poll_recording};
pub use conf_roomeq::{poll_delay_detection, poll_room_eq_optimization};
pub use conf_spinoramaeq::{poll_spinorama_optimization, poll_spinorama_speaker_load};
pub use media_control::handle_media_control_event;

/// Cycle through a list of string options, wrapping around.
pub(super) fn cycle_string(current: &str, options: &[&str], delta: i32) -> String {
    let idx = options.iter().position(|&o| o == current).unwrap_or(0);
    let new_idx = if delta > 0 {
        (idx + 1) % options.len()
    } else {
        (idx + options.len() - 1) % options.len()
    };
    options[new_idx].to_string()
}

use devices::handle_devices_keys;
use file_explorer::handle_file_explorer_mode;
use library::handle_library_keys;
use plugins::{
    handle_add_plugin_mode, handle_edit_plugin_mode, handle_load_apo_file_mode,
    handle_load_plugins_mode, handle_load_sofa_file_mode, handle_plugins_keys,
    handle_save_plugins_mode,
};
use queue::handle_queue_keys;
use search::handle_search_mode;

pub enum AppEvent {
    Tick,
    Key(KeyEvent),
    Resize,
    MediaControl(MediaControlEvent),
}

pub fn handle_events(
    timeout: Duration,
    media_controls: Option<&TuiMediaControls>,
) -> std::io::Result<Option<AppEvent>> {
    // Check media control events first (non-blocking)
    if let Some(event) = media_controls.and_then(|mc| mc.poll_event()) {
        return Ok(Some(AppEvent::MediaControl(event)));
    }

    if event::poll(timeout)? {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => Ok(Some(AppEvent::Key(key))),
            Event::Resize(_, _) => Ok(Some(AppEvent::Resize)),
            _ => Ok(None),
        }
    } else {
        Ok(Some(AppEvent::Tick))
    }
}

pub fn handle_key_event(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    // Ctrl+C always quits, regardless of input mode
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.should_quit = true;
        return None;
    }

    match app.input_mode {
        InputMode::Search => handle_search_mode(app, key),
        InputMode::AddPlugin => handle_add_plugin_mode(app, key),
        InputMode::EditPlugin => handle_edit_plugin_mode(app, key),
        InputMode::SavePlugins => handle_save_plugins_mode(app, key),
        InputMode::LoadPlugins => handle_load_plugins_mode(app, key),
        InputMode::LoadApoFile => handle_load_apo_file_mode(app, key),
        InputMode::LoadSofaFile => handle_load_sofa_file_mode(app, key),
        InputMode::FileExplorer => handle_file_explorer_mode(app, key),
        InputMode::ShowHelp => handle_help_mode(app, key),
        InputMode::ShowError => handle_error_mode(app, key),
        InputMode::ChannelConflict => handle_channel_conflict_mode(app, key),
        InputMode::LevelMeters => level_meters::handle_level_meters_keys(app, key),
        InputMode::Configure
        | InputMode::ConfigureDirectories
        | InputMode::ConfigureRecording
        | InputMode::ConfigureRoomEq
        | InputMode::ConfigureHeadphoneEq
        | InputMode::ConfigureSpinoramaEq
        | InputMode::ConfigureFederationSources
        | InputMode::ConfigureServers => conf::handle_configure_mode(app, key),
        InputMode::Normal => handle_normal_mode(app, key),
    }
}

/// Shared keys available across Normal and Configure modes.
///
/// Returns `Some(cmd)` when the key is handled (cmd may be `None`),
/// or `None` when the key is not a shared key.
fn handle_shared_keys(app: &mut App, key: KeyEvent) -> Option<Option<PlayerCommand>> {
    match key.code {
        // Quit
        KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
            Some(None)
        }
        KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::SUPER) => {
            app.should_quit = true;
            Some(None)
        }

        // Screen switching
        KeyCode::Char('L') => {
            app.current_screen = Screen::Library;
            app.input_mode = InputMode::Normal;
            Some(None)
        }
        KeyCode::Char('C') => {
            app.current_screen = Screen::Configure;
            app.input_mode = InputMode::Configure;
            Some(None)
        }
        KeyCode::Char('Q') => {
            app.current_screen = Screen::Queue;
            app.input_mode = InputMode::Normal;
            Some(None)
        }
        KeyCode::Char('P') => {
            app.current_screen = Screen::Plugins;
            app.input_mode = InputMode::Normal;
            Some(None)
        }
        KeyCode::Char('O') => {
            app.current_screen = Screen::Devices;
            app.input_mode = InputMode::Normal;
            Some(None)
        }
        KeyCode::Char('Y') => {
            app.current_screen = Screen::Playlists;
            app.input_mode = InputMode::Normal;
            Some(None)
        }
        KeyCode::Char('N') => {
            app.current_screen = Screen::Configure;
            app.input_mode = InputMode::Configure;
            Some(None)
        }

        // Help
        KeyCode::Char('?') => {
            app.enter_overlay_mode(InputMode::ShowHelp);
            Some(None)
        }

        // Volume controls (not in configure sub-screens where +/- adjust fields)
        KeyCode::Char('+') | KeyCode::Char('=') if !app.input_mode.is_configure_sub_screen() => {
            app.increase_volume();
            Some(Some(PlayerCommand::SetVolume(app.volume)))
        }
        KeyCode::Char('-') | KeyCode::Char('_') if !app.input_mode.is_configure_sub_screen() => {
            app.decrease_volume();
            Some(Some(PlayerCommand::SetVolume(app.volume)))
        }

        // Mute toggle
        KeyCode::Char('u') if !app.input_mode.is_configure_sub_screen() => {
            Some(Some(PlayerCommand::ToggleMute))
        }

        _ => None,
    }
}

/// Handle keys in Normal mode (no special sub-mode active).
fn handle_normal_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    // The Playlists screen has its own sub-state machine. When the user is
    // typing a playlist name (Create / Rename), characters must reach the
    // text input handler — bypass Tab/Esc/shared-shortcut routing so capital
    // letters like `C` go into the name instead of switching to Configure.
    use crate::app::PlaylistMode;
    if app.current_screen == Screen::Playlists
        && matches!(app.playlist_mode, PlaylistMode::Create | PlaylistMode::Rename)
    {
        return playlists::handle_playlists_keys(app, key);
    }

    match key.code {
        KeyCode::Esc => {
            app.should_quit = true;
            None
        }
        // TAB to cycle through screens
        KeyCode::Tab => {
            app.current_screen = match app.current_screen {
                Screen::Loading => Screen::Loading,
                Screen::Library => Screen::Queue,
                Screen::Queue => Screen::Playlists,
                Screen::Playlists => Screen::Plugins,
                Screen::Plugins => Screen::Devices,
                Screen::Devices => Screen::Configure,
                Screen::Configure => Screen::Library,
            };
            if app.current_screen == Screen::Configure {
                app.input_mode = InputMode::Configure;
            } else {
                app.input_mode = InputMode::Normal;
            }
            None
        }
        _ => {
            // Try shared keys
            if let Some(cmd) = handle_shared_keys(app, key) {
                return cmd;
            }
            // Dispatch based on current screen
            match app.current_screen {
                Screen::Loading => None,
                Screen::Library => handle_library_keys(app, key),
                Screen::Queue => handle_queue_keys(app, key),
                Screen::Playlists => playlists::handle_playlists_keys(app, key),
                Screen::Plugins => handle_plugins_keys(app, key),
                Screen::Devices => handle_devices_keys(app, key),
                Screen::Configure => conf::handle_tab_bar_keys(app, key),
            }
        }
    }
}

fn handle_help_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
            app.exit_overlay_mode();
            None
        }
        _ => None,
    }
}

fn handle_error_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('q') => {
            app.exit_overlay_mode();
            app.error_message = None;
            None
        }
        _ => None,
    }
}

fn handle_channel_conflict_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use crate::app::ChannelConflictChoice;

    const NUM_OPTIONS: usize = 3;

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            if app.channel_conflict_selection > 0 {
                app.channel_conflict_selection -= 1;
            }
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.channel_conflict_selection < NUM_OPTIONS - 1 {
                app.channel_conflict_selection += 1;
            }
            None
        }
        KeyCode::Enter => {
            let choice = match app.channel_conflict_selection {
                0 => ChannelConflictChoice::SuspendIncompatible,
                1 => ChannelConflictChoice::RemoveIncompatible,
                2 => ChannelConflictChoice::Cancel,
                _ => ChannelConflictChoice::Cancel,
            };

            let path = app.channel_conflict_path.take();
            let conflicts = std::mem::take(&mut app.channel_conflicts);
            app.exit_overlay_mode();

            match choice {
                ChannelConflictChoice::SuspendIncompatible => {
                    let indices: Vec<usize> = conflicts.iter().map(|c| c.index).collect();
                    app.plugin_graph.suspend_plugins(&indices);
                    app.plugin_graph.update_channel_dependent_plugins();
                    log::info!(
                        "[TUI] Suspended {} incompatible plugin(s) (channel conflict)",
                        indices.len()
                    );
                    path.map(PlayerCommand::PlayResolved)
                }
                ChannelConflictChoice::RemoveIncompatible => {
                    // Remove in reverse order to keep indices valid
                    let mut indices: Vec<usize> = conflicts.iter().map(|c| c.index).collect();
                    indices.sort_unstable_by(|a, b| b.cmp(a));
                    for idx in &indices {
                        app.plugin_graph.remove_plugin_by_index(*idx).ok();
                    }
                    log::info!(
                        "[TUI] Removed {} incompatible plugin(s) (channel conflict)",
                        indices.len()
                    );
                    path.map(PlayerCommand::PlayResolved)
                }
                ChannelConflictChoice::Cancel => {
                    log::info!("[TUI] Playback cancelled by user (channel conflict)");
                    app.is_playing = false;
                    None
                }
            }
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.channel_conflict_path = None;
            app.channel_conflicts.clear();
            app.exit_overlay_mode();
            app.is_playing = false;
            None
        }
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub enum PlayerCommand {
    Play(sotf_audio::decoder::AudioSource),
    /// Play after channel conflict was already resolved (skip conflict re-check)
    PlayResolved(sotf_audio::decoder::AudioSource),
    Pause,
    Resume,
    Stop,
    SetVolume(f32),
    SetOutputDevice(String),
    /// Seek to absolute position in seconds
    Seek(f64),
    /// Seek relative to current position (positive = forward, negative = backward)
    SeekRelative(f64),
    /// Toggle mute on/off
    ToggleMute,
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::theme::Theme;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    pub(crate) fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    pub(crate) fn make_app() -> App {
        App::new(Theme::default(), false)
    }
}

#[cfg(test)]
#[path = "../tests/test_events_navigation.rs"]
mod test_events_navigation;
#[cfg(test)]
#[path = "../tests/test_events_scenario.rs"]
mod test_events_scenario;
