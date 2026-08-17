use crate::app::{App, InputMode, Screen};
use crate::media_controls::TuiMediaControls;
use crate::ui::keybinding_catalog::{SharedCommand, TuiCommand, TuiKeyContext, resolve_command};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use sotf_media_controls::MediaControlEvent;
use std::time::Duration;

pub mod devices;
pub mod ear_training;
pub mod file_explorer;
pub mod level_meters;
pub mod library;
pub mod media_control;
pub mod metadata;
pub mod playlists;
pub mod plugins;
pub mod queue;
pub mod search;
pub mod tools;

pub mod conf;
pub mod conf_directories;
pub mod conf_federation;
pub mod conf_headphoneeq;
pub mod conf_recordings;
pub mod conf_roomeq;
pub mod conf_servers;
pub mod conf_spinoramaeq;

pub use conf_federation::{poll_federation_scan, poll_federation_test, poll_service_login};
pub use conf_headphoneeq::{
    poll_headphone_download, poll_headphone_eq_optimization, poll_headphone_list_load,
};
pub use conf_recordings::{
    poll_bass_anchor_capture, poll_probe_capture, poll_recording, poll_save_recordings,
    poll_spl_calibration_capture,
};
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
use ear_training::handle_ear_training_keys;
use file_explorer::handle_file_explorer_mode;
use library::handle_library_keys;
use metadata::handle_metadata_editor_mode;
use plugins::{
    handle_add_plugin_mode, handle_edit_plugin_mode, handle_load_apo_file_mode,
    handle_load_plugins_mode, handle_load_sofa_file_mode, handle_plugins_keys,
    handle_save_plugins_mode,
};
use queue::handle_queue_keys;
use search::handle_search_mode;
use tools::handle_tools_keys;

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
            Event::Mouse(_) => Ok(None), // Mouse support is explicitly deferred for this release.
            _ => Ok(None),
        }
    } else {
        Ok(Some(AppEvent::Tick))
    }
}

pub fn handle_key_event(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    if let Some(command) = resolve_command(TuiKeyContext::Always, key) {
        let TuiCommand::Shared(command) = command else {
            unreachable!("non-shared command in Always context: {command:?}");
        };
        return dispatch_shared_command(app, key, command);
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
        InputMode::MetadataEditor => handle_metadata_editor_mode(app, key),
        InputMode::LevelMeters => level_meters::handle_level_meters_keys(app, key),
        InputMode::Configure
        | InputMode::ConfigureDirectories
        | InputMode::ConfigureRecording
        | InputMode::ConfigureRoomEq
        | InputMode::ConfigureHeadphoneEq
        | InputMode::ConfigureSpinoramaEq
        | InputMode::ConfigureFederationSources
        | InputMode::ConfigureServers
        | InputMode::ConfigureMetadataServices => conf::handle_configure_mode(app, key),
        InputMode::Normal => handle_normal_mode(app, key),
    }
}

#[cfg(test)]
mod p1_event_policy_tests {
    #[test]
    fn mouse_support_is_explicitly_deferred() {
        let source = include_str!("mod.rs");
        let production_source = source
            .split("#[cfg(test)]")
            .next()
            .expect("source before tests");
        assert!(production_source.contains("Event::Mouse(_) => Ok(None)"));
        assert!(production_source.contains("Mouse support is explicitly deferred"));
        assert!(!production_source.contains("EnableMouseCapture"));
    }
}

/// Shared keys available across Normal and Configure modes.
///
/// Returns `Some(cmd)` when the key is handled (cmd may be `None`),
/// or `None` when the key is not a shared key.
fn handle_shared_keys(app: &mut App, key: KeyEvent) -> Option<Option<PlayerCommand>> {
    // Screen-specific bindings take precedence over root conveniences. This
    // makes Plugins `u` and Shift+Up/Down reachable instead of being consumed
    // as mute or meter-control commands.
    if app.current_screen == Screen::Plugins
        && app.input_mode == InputMode::Normal
        && resolve_command(TuiKeyContext::PluginList, key).is_some()
    {
        return None;
    }

    if !app.input_mode.is_configure_sub_screen()
        && let Some(command) = resolve_command(TuiKeyContext::GlobalMeters, key)
    {
        let TuiCommand::Shared(command) = command else {
            unreachable!("non-shared command in GlobalMeters context: {command:?}");
        };
        return Some(dispatch_shared_command(app, key, command));
    }

    if !app.input_mode.is_configure_sub_screen()
        && let Some(command) = resolve_command(TuiKeyContext::NormalRoot, key)
    {
        let TuiCommand::Shared(command) = command else {
            unreachable!("non-shared command in NormalRoot context: {command:?}");
        };
        return Some(dispatch_shared_command(app, key, command));
    }

    if let Some(command) = resolve_command(TuiKeyContext::SharedRoot, key) {
        let TuiCommand::Shared(command) = command else {
            unreachable!("non-shared command in SharedRoot context: {command:?}");
        };
        return Some(dispatch_shared_command(app, key, command));
    }

    None
}

pub(super) fn dispatch_shared_command(
    app: &mut App,
    key: KeyEvent,
    command: SharedCommand,
) -> Option<PlayerCommand> {
    match command {
        SharedCommand::Quit => {
            app.leave_ear_training();
            app.leave_ab_testing();
            app.should_quit = true;
            None
        }
        SharedCommand::ExitApplication => {
            match app.current_screen {
                Screen::EarTraining | Screen::AbTesting => app.switch_screen(Screen::Tools),
                Screen::Tools => app.switch_screen(Screen::Library),
                _ => app.should_quit = true,
            }
            None
        }
        SharedCommand::CycleScreens => {
            let screen = match app.current_screen {
                Screen::Loading => Screen::Loading,
                Screen::Library => Screen::Queue,
                Screen::Queue => Screen::Playlists,
                Screen::Playlists => Screen::Plugins,
                Screen::Plugins => Screen::Devices,
                Screen::Devices => Screen::Tools,
                Screen::Tools | Screen::EarTraining | Screen::AbTesting => Screen::Configure,
                Screen::Configure => Screen::Library,
            };
            app.switch_screen(screen);
            app.input_mode = if screen == Screen::Configure {
                InputMode::Configure
            } else {
                InputMode::Normal
            };
            None
        }
        SharedCommand::SwitchScreen => {
            let (screen, mode) = match key.code {
                KeyCode::Char('L') => (Screen::Library, InputMode::Normal),
                KeyCode::Char('Q') => (Screen::Queue, InputMode::Normal),
                KeyCode::Char('P') => (Screen::Plugins, InputMode::Normal),
                KeyCode::Char('O') => (Screen::Devices, InputMode::Normal),
                KeyCode::Char('Y') => (Screen::Playlists, InputMode::Normal),
                KeyCode::Char('T') => (Screen::Tools, InputMode::Normal),
                KeyCode::Char('C') | KeyCode::Char('N') => {
                    (Screen::Configure, InputMode::Configure)
                }
                _ => unreachable!("non-screen chord resolved as SwitchScreen: {key:?}"),
            };
            app.switch_screen(screen);
            app.input_mode = mode;
            None
        }
        SharedCommand::FocusLevelMeters => {
            app.input_mode = InputMode::LevelMeters;
            None
        }
        SharedCommand::CycleLanguage => {
            let language = app.ui.language.next();
            app.ui.language = language;
            app.ui.status_message = Some(
                crate::i18n::TuiTranslations::for_language(language)
                    .language_changed
                    .to_string(),
            );
            app.ui.needs_redraw = true;
            None
        }
        SharedCommand::AdjustVolume => {
            if matches!(key.code, KeyCode::Char('+') | KeyCode::Char('=')) {
                app.increase_volume();
            } else {
                app.decrease_volume();
            }
            Some(PlayerCommand::SetVolume(app.playback.volume))
        }
        SharedCommand::SelectOutputDevice => {
            if key.code == KeyCode::Right {
                app.select_next_output_device();
            } else {
                app.select_previous_output_device();
            }
            app.get_selected_output_device()
                .map(|device| PlayerCommand::SetOutputDevice(device.name.clone()))
        }
        SharedCommand::NavigateMeterGroup | SharedCommand::FocusedMeterGroup => {
            if key.code == KeyCode::Right {
                app.select_next_level_meter_group();
            } else {
                app.select_previous_level_meter_group();
            }
            None
        }
        SharedCommand::NavigateMeterControl | SharedCommand::FocusedMeterControl => {
            if key.code == KeyCode::Down {
                app.select_next_level_meter_control();
            } else {
                app.select_previous_level_meter_control();
            }
            None
        }
        SharedCommand::ToggleMeterSolo | SharedCommand::FocusedMeterSolo => {
            app.toggle_level_meter_solo();
            None
        }
        SharedCommand::ToggleMute => Some(PlayerCommand::ToggleMute),
        SharedCommand::ToggleReplayGain => {
            app.playback.replay_gain_enabled = !app.playback.replay_gain_enabled;
            let mode = if app.playback.replay_gain_enabled {
                match app.playback.replay_gain_mode {
                    crate::app::ReplayGainMode::Track => crate::tui_text!(app, "ON (Track mode)"),
                    crate::app::ReplayGainMode::Album => crate::tui_text!(app, "ON (Album mode)"),
                }
            } else {
                crate::tui_text!(app, "OFF")
            };
            app.ui.status_message = Some(format!("ReplayGain: {mode}"));
            if app.playback.is_playing {
                app.plugin_rack.needs_update = true;
            }
            None
        }
        SharedCommand::CycleReplayGainMode => {
            use crate::app::ReplayGainMode;
            app.playback.replay_gain_mode = match app.playback.replay_gain_mode {
                ReplayGainMode::Track => ReplayGainMode::Album,
                ReplayGainMode::Album => ReplayGainMode::Track,
            };
            let mode = match app.playback.replay_gain_mode {
                ReplayGainMode::Track => crate::tui_text!(app, "Track"),
                ReplayGainMode::Album => crate::tui_text!(app, "Album"),
            };
            app.ui.status_message = Some(format!("ReplayGain mode: {mode}"));
            if app.playback.is_playing && app.playback.replay_gain_enabled {
                app.plugin_rack.needs_update = true;
            }
            None
        }
        SharedCommand::ShowHelp => {
            app.enter_overlay_mode(InputMode::ShowHelp);
            None
        }
        SharedCommand::FocusedMeterMute => {
            app.toggle_level_meter_mute();
            None
        }
        SharedCommand::FocusedMeterDim => {
            app.toggle_level_meter_dim();
            None
        }
        SharedCommand::FocusedMeterClear => {
            app.clear_level_meter_mutes_and_solos();
            None
        }
        SharedCommand::ExitLevelMeters => {
            app.input_mode = InputMode::Normal;
            if key.code == KeyCode::Tab {
                app.current_screen = Screen::Library;
            }
            None
        }
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
        && matches!(
            app.playlists.mode,
            PlaylistMode::Create | PlaylistMode::Rename
        )
    {
        return playlists::handle_playlists_keys(app, key);
    }

    if let Some(command) = handle_shared_keys(app, key) {
        return command;
    }

    match app.current_screen {
        Screen::Loading => None,
        Screen::Library => handle_library_keys(app, key),
        Screen::Queue => handle_queue_keys(app, key),
        Screen::Playlists => playlists::handle_playlists_keys(app, key),
        Screen::Plugins => handle_plugins_keys(app, key),
        Screen::Devices => handle_devices_keys(app, key),
        Screen::Tools => handle_tools_keys(app, key),
        Screen::EarTraining => handle_ear_training_keys(app, key),
        Screen::AbTesting => tools::handle_ab_testing_keys(app, key),
        Screen::Configure => conf::handle_tab_bar_keys(app, key),
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
            app.ui.error_message = None;
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
            if app.modal.channel_conflict_selection > 0 {
                app.modal.channel_conflict_selection -= 1;
            }
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.modal.channel_conflict_selection < NUM_OPTIONS - 1 {
                app.modal.channel_conflict_selection += 1;
            }
            None
        }
        KeyCode::Enter => {
            // Project rule: crash hard on unknown values. The Up/Down
            // handler clamps to [0, NUM_OPTIONS), but if any future
            // code path pokes this field out of range we want to know.
            let choice = match app.modal.channel_conflict_selection {
                0 => ChannelConflictChoice::SuspendIncompatible,
                1 => ChannelConflictChoice::RemoveIncompatible,
                2 => ChannelConflictChoice::Cancel,
                n => unreachable!("channel_conflict_selection out of range: {}", n),
            };

            let path = app.modal.channel_conflict_path.take();
            let conflicts = std::mem::take(&mut app.modal.channel_conflicts);
            app.exit_overlay_mode();

            match choice {
                ChannelConflictChoice::SuspendIncompatible => {
                    let indices: Vec<usize> = conflicts.iter().map(|c| c.index).collect();
                    app.plugin_rack.graph.suspend_plugins(&indices);
                    app.plugin_rack.graph.update_channel_dependent_plugins();
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
                        app.plugin_rack.graph.remove_plugin_by_index(*idx).ok();
                    }
                    log::info!(
                        "[TUI] Removed {} incompatible plugin(s) (channel conflict)",
                        indices.len()
                    );
                    path.map(PlayerCommand::PlayResolved)
                }
                ChannelConflictChoice::Cancel => {
                    log::info!("[TUI] Playback cancelled by user (channel conflict)");
                    app.playback.is_playing = false;
                    None
                }
            }
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.modal.channel_conflict_path = None;
            app.modal.channel_conflicts.clear();
            app.exit_overlay_mode();
            app.playback.is_playing = false;
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
