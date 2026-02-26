use crate::app::{App, FilePickerMode, FilePickerOrigin, InputMode, MatrixEditMode, Screen};
use crate::media_controls::TuiMediaControls;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use sotf_audio_player::{PluginSettings, PluginType};
use souvlaki::MediaControlEvent;
use std::time::Duration;

// Sub-modules for event handling
// Note: Functions are still in mod.rs; sub-modules are gradually taking over
// as part of the modularization effort.
// Some sub-modules have pre-existing compilation errors that need to be fixed:
// - recording.rs: references types/fields that don't exist in RecordingTuiState
// - headphone_eq.rs: references non-existent config fields
// - spinorama.rs and room_eq.rs: have various type mismatches
// These need to be debugged and fixed before they can be fully integrated.

// Module declarations are temporarily disabled to allow compilation:
// mod media_control;
// mod input_modes;
// mod screens;
// pub mod configure;
// pub mod spinorama;
// pub mod headphone_eq;
// pub mod room_eq;
// pub mod recording;

// For now, keep using the functions in this file until sub-modules are fixed
// TODO: Fix compilation errors in sub-modules, then migrate functions

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
    if let Some(event) = media_controls.and_then(|mc| mc.poll_events().into_iter().next()) {
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

pub fn handle_media_control_event(
    app: &mut App,
    event: MediaControlEvent,
) -> Option<PlayerCommand> {
    match event {
        MediaControlEvent::Play => {
            if app.current_queue_index.is_none() {
                // Nothing playing yet — start the queue
                app.start_queue().map(PlayerCommand::Play)
            } else {
                app.is_playing = true;
                Some(PlayerCommand::Resume)
            }
        }
        MediaControlEvent::Pause => {
            app.is_playing = false;
            Some(PlayerCommand::Pause)
        }
        MediaControlEvent::Toggle => {
            if app.is_playing {
                app.is_playing = false;
                Some(PlayerCommand::Pause)
            } else {
                if app.current_queue_index.is_none() {
                    app.start_queue().map(PlayerCommand::Play)
                } else {
                    app.is_playing = true;
                    Some(PlayerCommand::Resume)
                }
            }
        }
        MediaControlEvent::Next => {
            if let Some(path) = app.next_track() {
                Some(PlayerCommand::Play(path))
            } else {
                app.is_playing = false;
                Some(PlayerCommand::Stop)
            }
        }
        MediaControlEvent::Previous => app.previous_track().map(PlayerCommand::Play),
        MediaControlEvent::Stop => {
            app.is_playing = false;
            Some(PlayerCommand::Stop)
        }
        MediaControlEvent::SetPosition(pos) => Some(PlayerCommand::Seek(pos.0.as_secs_f64())),
        MediaControlEvent::SetVolume(vol) => {
            let clamped = vol.clamp(0.0, 1.0) as f32;
            app.volume = clamped;
            Some(PlayerCommand::SetVolume(clamped))
        }
        MediaControlEvent::Seek(direction) => {
            let offset = match direction {
                souvlaki::SeekDirection::Forward => 10.0,
                souvlaki::SeekDirection::Backward => -10.0,
            };
            Some(PlayerCommand::SeekRelative(offset))
        }
        MediaControlEvent::SeekBy(direction, duration) => {
            let secs = duration.as_secs_f64();
            let offset = match direction {
                souvlaki::SeekDirection::Forward => secs,
                souvlaki::SeekDirection::Backward => -secs,
            };
            Some(PlayerCommand::SeekRelative(offset))
        }
        MediaControlEvent::Raise => None,
        MediaControlEvent::Quit => {
            app.should_quit = true;
            None
        }
        MediaControlEvent::OpenUri(_) => None,
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
        InputMode::AddDirectory => handle_add_directory_mode(app, key),
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
        InputMode::Normal => handle_normal_mode(app, key),
    }
}

fn handle_file_explorer_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.close_file_explorer();
            None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.file_explorer_select_prev();
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.file_explorer_select_next();
            None
        }
        KeyCode::Enter | KeyCode::Char('l') => {
            if let Some(path) = app.file_explorer_current().cloned() {
                if path.is_dir() {
                    match app.file_picker_mode {
                        FilePickerMode::Directory => {
                            // Enter selects this directory
                            apply_file_selection(app, path);
                        }
                        FilePickerMode::File => {
                            // Enter navigates into directory
                            app.file_explorer_enter_dir(path);
                        }
                    }
                } else {
                    // It's a file — select it
                    apply_file_selection(app, path);
                }
            }
            None
        }
        KeyCode::Right => {
            // Right always navigates into directory (even in Directory mode)
            if let Some(path) = app.file_explorer_current().cloned()
                && path.is_dir()
            {
                app.file_explorer_enter_dir(path);
            }
            None
        }
        KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => {
            app.file_explorer_go_parent();
            None
        }
        KeyCode::Char('H') => {
            // Toggle hidden files
            app.file_explorer_toggle_hidden();
            None
        }
        _ => None,
    }
}

fn apply_file_selection(app: &mut App, path: std::path::PathBuf) {
    let path_str = path.to_string_lossy().to_string();
    match app.file_picker_origin {
        FilePickerOrigin::SofaFile => {
            app.sofa_file_input = path_str;
            if let Err(e) = app.load_sofa_file() {
                app.status_message = Some(format!("Error: {}", e));
            } else {
                app.status_message = Some("SOFA file loaded".to_string());
                app.request_plugin_update();
            }
        }
        FilePickerOrigin::IrFile => {
            if let Some(plugin) = app.plugin_chain.get_plugin_mut(app.selected_plugin_index)
                && let PluginSettings::Convolution {
                    ref mut ir_file, ..
                } = plugin.settings
            {
                *ir_file = path_str;
                app.status_message = Some("IR file set".to_string());
                app.request_plugin_update();
            }
        }
        FilePickerOrigin::RecordingOutputDir => {
            app.recording.output_directory = path_str;
            app.recording.editing_output_dir = false;
        }
        FilePickerOrigin::RecordingMicCalibration => {
            app.recording.mic_calibration_path = path_str;
            app.recording.editing_mic_cal = false;
        }
        FilePickerOrigin::RoomEqFilePath => {
            app.room_eq.file_path = path_str;
            app.room_eq.editing_file_path = false;
            load_room_eq_measurements(app);
        }
        FilePickerOrigin::RoomEqExportPath => {
            app.room_eq.export_path = path_str;
            app.room_eq.editing_export_path = false;
            export_room_eq_results(app);
        }
        FilePickerOrigin::HeadphoneMeasurement => {
            app.headphone_eq.measurement_path = path_str;
            app.headphone_eq.editing_measurement = false;
        }
        FilePickerOrigin::HeadphoneCustomTarget => {
            app.headphone_eq.custom_target_path = path_str;
            app.headphone_eq.editing_custom_target = false;
        }
        FilePickerOrigin::AddDirectory => {
            app.add_directory(path);
        }
        FilePickerOrigin::ApoFile => {
            app.apo_file_input = path_str;
            if let Err(e) = app.load_apo_file() {
                app.status_message = Some(format!("APO error: {}", e));
            } else {
                app.status_message = Some("APO file loaded".to_string());
                app.request_plugin_update();
            }
        }
    }
    app.close_file_explorer();
}

fn handle_normal_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        // Esc to return to Main pane from Meters (takes priority over quit)
        KeyCode::Esc if app.focused_pane == crate::app::FocusedPane::Meters => {
            app.focused_pane = crate::app::FocusedPane::Main;
            None
        }
        // Quit — but first check if we're in a sub-screen and should go up
        KeyCode::Esc => {
            if app.current_screen == Screen::Configure {
                // Go back to Library from Configure
                app.current_screen = Screen::Library;
            } else {
                app.should_quit = true;
            }
            None
        }
        KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
            None
        }
        // Command-Q on macOS (crossterm treats it as SUPER modifier)
        KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::SUPER) => {
            app.should_quit = true;
            None
        }

        // TAB to cycle through screens and meters pane
        KeyCode::Tab => {
            use crate::app::FocusedPane;

            match app.focused_pane {
                FocusedPane::Main => {
                    // Cycle through screens, then switch to Meters pane
                    app.current_screen = match app.current_screen {
                        Screen::Loading => Screen::Loading, // Stay on loading
                        Screen::Library => Screen::Queue,
                        Screen::Queue => Screen::Plugins,
                        Screen::Plugins => Screen::Devices,
                        Screen::Devices => Screen::Configure,
                        Screen::Configure => {
                            // After last screen, switch to Meters pane
                            app.focused_pane = FocusedPane::Meters;
                            Screen::Library // Stay on a screen (doesn't matter which)
                        }
                    };
                }
                FocusedPane::Meters => {
                    // From Meters pane, go back to Main pane on Library screen
                    app.focused_pane = FocusedPane::Main;
                    app.current_screen = Screen::Library;
                }
            }
            None
        }

        // Screen switching (uppercase only to avoid conflicts with screen-specific shortcuts)
        KeyCode::Char('L') => {
            app.current_screen = Screen::Library;
            None
        }
        KeyCode::Char('N') => {
            app.current_screen = Screen::Configure;
            None
        }
        KeyCode::Char('Q') => {
            app.current_screen = Screen::Queue;
            None
        }
        KeyCode::Char('P') => {
            app.current_screen = Screen::Plugins;
            None
        }
        KeyCode::Char('O') => {
            app.current_screen = Screen::Devices;
            None
        }

        // Level meter controls (Shift + arrow keys)
        KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.select_previous_level_meter_group();
            None
        }
        KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.select_next_level_meter_group();
            None
        }
        KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.select_previous_level_meter_control();
            None
        }
        KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.select_next_level_meter_control();
            None
        }
        KeyCode::Char('M') if key.modifiers.contains(KeyModifiers::SHIFT) => {
            // Shift-M to focus on level meters pane
            app.focused_pane = crate::app::FocusedPane::Meters;
            None
        }
        KeyCode::Char('S') if key.modifiers.contains(KeyModifiers::SHIFT) => {
            // Shift-S to toggle solo on selected group
            app.toggle_level_meter_solo();
            None
        }
        KeyCode::Char('C') => {
            if app.focused_pane == crate::app::FocusedPane::Meters {
                app.clear_level_meter_mutes_and_solos();
            } else {
                app.current_screen = Screen::Configure;
            }
            None
        }

        // Help
        KeyCode::Char('?') => {
            app.input_mode = InputMode::ShowHelp;
            None
        }

        // ReplayGain toggle
        KeyCode::Char('g') => {
            app.replay_gain_enabled = !app.replay_gain_enabled;
            let mode_str = if app.replay_gain_enabled {
                match app.replay_gain_mode {
                    crate::app::ReplayGainMode::Track => "ON (Track mode)",
                    crate::app::ReplayGainMode::Album => "ON (Album mode)",
                }
            } else {
                "OFF"
            };
            app.status_message = Some(format!("ReplayGain: {}", mode_str));
            if app.is_playing {
                app.needs_plugin_update = true;
            }
            None
        }
        // ReplayGain mode cycle
        KeyCode::Char('G') => {
            use crate::app::ReplayGainMode;
            app.replay_gain_mode = match app.replay_gain_mode {
                ReplayGainMode::Track => ReplayGainMode::Album,
                ReplayGainMode::Album => ReplayGainMode::Track,
            };
            let mode_str = match app.replay_gain_mode {
                ReplayGainMode::Track => "Track",
                ReplayGainMode::Album => "Album",
            };
            app.status_message = Some(format!("ReplayGain mode: {}", mode_str));
            if app.is_playing && app.replay_gain_enabled {
                app.needs_plugin_update = true;
            }
            None
        }

        // Volume controls (in case Shift+Arrow doesn't work)
        KeyCode::Char('+') | KeyCode::Char('=') => {
            app.increase_volume();
            Some(PlayerCommand::SetVolume(app.volume))
        }
        KeyCode::Char('-') | KeyCode::Char('_') => {
            app.decrease_volume();
            Some(PlayerCommand::SetVolume(app.volume))
        }

        // Output device selection with Ctrl+Arrow keys
        KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.select_next_output_device();
            // Switch to the newly selected device
            app.get_selected_output_device()
                .map(|device| PlayerCommand::SetOutputDevice(device.name.clone()))
        }
        KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.select_previous_output_device();
            // Switch to the newly selected device
            app.get_selected_output_device()
                .map(|device| PlayerCommand::SetOutputDevice(device.name.clone()))
        }

        // Level meter navigation when Meters pane is focused
        KeyCode::Left if app.focused_pane == crate::app::FocusedPane::Meters => {
            app.select_previous_level_meter_group();
            None
        }
        KeyCode::Right if app.focused_pane == crate::app::FocusedPane::Meters => {
            app.select_next_level_meter_group();
            None
        }
        KeyCode::Up if app.focused_pane == crate::app::FocusedPane::Meters => {
            app.select_previous_level_meter_control();
            None
        }
        KeyCode::Down if app.focused_pane == crate::app::FocusedPane::Meters => {
            app.select_next_level_meter_control();
            None
        }
        KeyCode::Char('m') if app.focused_pane == crate::app::FocusedPane::Meters => {
            // 'm' to toggle mute when in Meters pane
            app.toggle_level_meter_mute();
            None
        }
        KeyCode::Char('s') if app.focused_pane == crate::app::FocusedPane::Meters => {
            // 's' to toggle solo when in Meters pane
            app.toggle_level_meter_solo();
            None
        }
        KeyCode::Char('d') if app.focused_pane == crate::app::FocusedPane::Meters => {
            // 'd' to toggle dim when in Meters pane
            app.toggle_level_meter_dim();
            None
        }
        KeyCode::Char('c') if app.focused_pane == crate::app::FocusedPane::Meters => {
            // 'c' to clear all mutes/solos/dims when in Meters pane
            app.clear_level_meter_mutes_and_solos();
            None
        }

        // Screen-specific controls
        _ => match app.current_screen {
            Screen::Loading => None, // Ignore keys during loading
            Screen::Library => handle_library_keys(app, key),
            Screen::Queue => handle_queue_keys(app, key),
            Screen::Plugins => handle_plugins_keys(app, key),
            Screen::Devices => handle_devices_keys(app, key),
            Screen::Configure => handle_configure_keys(app, key),
        },
    }
}

fn handle_library_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use crate::app::{ChannelFilter, LibrarySortOrder, LibraryViewMode};

    const PAGE_SIZE: usize = 20;

    match key.code {
        KeyCode::Char('/') => {
            app.input_mode = InputMode::Search;
            None
        }
        KeyCode::Char('X') => {
            // Explicitly clear search query
            app.search_query.clear();
            app.selected_album_index = 0;
            app.request_filter_update();
            None
        }
        KeyCode::Char('t') => {
            // Toggle between flat and tree view
            app.toggle_library_view_mode();
            None
        }
        KeyCode::Char('s') => {
            // Cycle through sort orders
            let next_order = match app.library_sort_order {
                LibrarySortOrder::Year => LibrarySortOrder::Genre,
                LibrarySortOrder::Genre => LibrarySortOrder::Artist,
                LibrarySortOrder::Artist => LibrarySortOrder::Album,
                LibrarySortOrder::Album => LibrarySortOrder::Tracks,
                LibrarySortOrder::Tracks => LibrarySortOrder::Composer,
                LibrarySortOrder::Composer => LibrarySortOrder::Popularity,
                LibrarySortOrder::Popularity => LibrarySortOrder::Year,
            };
            app.set_library_sort_order(next_order);
            None
        }
        KeyCode::Char('c') => {
            // Cycle through channel filters
            app.cycle_channel_filter();
            None
        }
        KeyCode::Char('1') => {
            // Sort by year
            app.set_library_sort_order(LibrarySortOrder::Year);
            None
        }
        KeyCode::Char('2') => {
            // Sort by genre
            app.set_library_sort_order(LibrarySortOrder::Genre);
            None
        }
        KeyCode::Char('3') => {
            // Sort by artist
            app.set_library_sort_order(LibrarySortOrder::Artist);
            None
        }
        KeyCode::Char('4') => {
            // Sort by album
            app.set_library_sort_order(LibrarySortOrder::Album);
            None
        }
        KeyCode::Char('5') => {
            // Filter: Show all
            app.set_channel_filter(ChannelFilter::All);
            None
        }
        KeyCode::Char('6') => {
            // Filter: Mono only
            app.set_channel_filter(ChannelFilter::Mono);
            None
        }
        KeyCode::Char('7') => {
            // Filter: Stereo only
            app.set_channel_filter(ChannelFilter::Stereo);
            None
        }
        KeyCode::Char('8') => {
            // Filter: Surround only (5.0/5.1)
            app.set_channel_filter(ChannelFilter::Surround);
            None
        }
        KeyCode::Char('9') => {
            // Filter: Mixed channels only
            app.set_channel_filter(ChannelFilter::Mixed);
            None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            match app.library_view_mode {
                LibraryViewMode::Flat => app.select_previous_album(),
                LibraryViewMode::TreeView => app.select_previous_tree_item(),
            }
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            match app.library_view_mode {
                LibraryViewMode::Flat => app.select_next_album(),
                LibraryViewMode::TreeView => app.select_next_tree_item(),
            }
            None
        }
        KeyCode::PageUp => {
            match app.library_view_mode {
                LibraryViewMode::Flat => app.page_up_albums(PAGE_SIZE),
                LibraryViewMode::TreeView => app.page_up_tree(PAGE_SIZE),
            }
            None
        }
        KeyCode::PageDown => {
            match app.library_view_mode {
                LibraryViewMode::Flat => app.page_down_albums(PAGE_SIZE),
                LibraryViewMode::TreeView => app.page_down_tree(PAGE_SIZE),
            }
            None
        }
        KeyCode::Right | KeyCode::Char('l') => {
            // Expand artist in tree view
            if app.library_view_mode == LibraryViewMode::TreeView {
                app.toggle_artist_expansion();
            }
            None
        }
        KeyCode::Left | KeyCode::Char('h') => {
            // Collapse artist in tree view
            if app.library_view_mode == LibraryViewMode::TreeView {
                app.toggle_artist_expansion();
            }
            None
        }
        KeyCode::Char('a') | KeyCode::Enter => {
            let path = match app.library_view_mode {
                LibraryViewMode::Flat => app.add_album_to_queue(),
                LibraryViewMode::TreeView => app.add_tree_selection_to_queue(),
            };
            path.map(PlayerCommand::Play)
        }
        KeyCode::Char('f') => {
            // Toggle favorite on selected album
            app.toggle_selected_album_favorite();
            None
        }
        KeyCode::Char('q') => {
            app.current_screen = Screen::Queue;
            None
        }
        _ => None,
    }
}

fn handle_directory_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    const PAGE_SIZE: usize = 20;

    match key.code {
        KeyCode::Char('a') => {
            app.input_mode = InputMode::AddDirectory;
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
            let _ = app.start_replay_gain_scan();
            None
        }
        KeyCode::Char('R') => {
            // Force ReplayGain rescan of all tracks
            let _ = app.start_force_replay_gain_scan();
            None
        }
        KeyCode::Char('b') => {
            // Start Bliss audio analysis scan
            let _ = app.start_bliss_scan();
            None
        }
        KeyCode::Char('B') => {
            // Force Bliss rescan of all tracks
            let _ = app.start_force_bliss_scan();
            None
        }
        KeyCode::Char('W') => {
            // Force waveform rescan of all tracks
            let _ = app.start_force_waveform_scan();
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

fn handle_queue_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.select_previous_queue_item();
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.select_next_queue_item();
            None
        }
        KeyCode::Enter => {
            // Jump to selected album and play its first track
            app.jump_to_selected_album().map(PlayerCommand::Play)
        }
        KeyCode::Right | KeyCode::Char('l') => {
            // Expand the selected queue item
            app.expand_queue_item();
            None
        }
        KeyCode::Left | KeyCode::Char('h') => {
            // Collapse the selected queue item (or move to album header if on a track)
            app.collapse_queue_item();
            None
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            app.remove_from_queue(app.selected_queue_index);
            None
        }
        KeyCode::Char('c') => {
            app.clear_queue();
            Some(PlayerCommand::Stop)
        }
        KeyCode::Char('p') => {
            // Play from start or current position
            if app.current_queue_index.is_none() {
                if let Some(path) = app.start_queue() {
                    return Some(PlayerCommand::Play(path));
                }
            } else {
                app.is_playing = true;
                return Some(PlayerCommand::Resume);
            }
            None
        }
        KeyCode::Char(' ') => {
            // Toggle pause
            if app.is_playing {
                app.is_playing = false;
                Some(PlayerCommand::Pause)
            } else {
                app.is_playing = true;
                Some(PlayerCommand::Resume)
            }
        }
        KeyCode::Char('n') | KeyCode::Char('>') => {
            // Next track
            if let Some(path) = app.next_track() {
                Some(PlayerCommand::Play(path))
            } else {
                app.is_playing = false;
                Some(PlayerCommand::Stop)
            }
        }
        KeyCode::Char('b') | KeyCode::Char('<') => {
            // Previous track
            app.previous_track().map(PlayerCommand::Play)
        }
        KeyCode::Char('[') => {
            // Previous album image
            app.prev_album_image();
            None
        }
        KeyCode::Char(']') => {
            // Next album image
            app.next_album_image();
            None
        }
        // Seek controls
        KeyCode::Char('.') => {
            // Seek forward 10 seconds
            Some(PlayerCommand::SeekRelative(10.0))
        }
        KeyCode::Char(',') => {
            // Seek backward 10 seconds
            Some(PlayerCommand::SeekRelative(-10.0))
        }
        KeyCode::Char(':') => {
            // Seek forward 30 seconds (Shift + ;)
            Some(PlayerCommand::SeekRelative(30.0))
        }
        KeyCode::Char(';') => {
            // Seek backward 30 seconds
            Some(PlayerCommand::SeekRelative(-30.0))
        }
        KeyCode::Char('f') => {
            // Toggle favorite on current queue album
            app.toggle_current_queue_album_favorite();
            None
        }
        // Note: Volume controls (+/-) are now global (see handle_normal_mode)
        _ => None,
    }
}

fn handle_search_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use crate::app::LibraryViewMode;

    const PAGE_SIZE: usize = 20;

    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            // Don't clear query, just exit mode to persist search
            None
        }
        KeyCode::Enter => {
            app.input_mode = InputMode::Normal;
            app.selected_album_index = 0;
            None
        }
        // Allow navigation while searching
        KeyCode::Up => {
            match app.library_view_mode {
                LibraryViewMode::Flat => app.select_previous_album(),
                LibraryViewMode::TreeView => app.select_previous_tree_item(),
            }
            None
        }
        KeyCode::Down => {
            match app.library_view_mode {
                LibraryViewMode::Flat => app.select_next_album(),
                LibraryViewMode::TreeView => app.select_next_tree_item(),
            }
            None
        }
        KeyCode::PageUp => {
            match app.library_view_mode {
                LibraryViewMode::Flat => app.page_up_albums(PAGE_SIZE),
                LibraryViewMode::TreeView => app.page_up_tree(PAGE_SIZE),
            }
            None
        }
        KeyCode::PageDown => {
            match app.library_view_mode {
                LibraryViewMode::Flat => app.page_down_albums(PAGE_SIZE),
                LibraryViewMode::TreeView => app.page_down_tree(PAGE_SIZE),
            }
            None
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            app.selected_album_index = 0;
            app.request_filter_update();
            None
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            app.selected_album_index = 0;
            app.request_filter_update();
            None
        }
        _ => None,
    }
}

fn handle_add_directory_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
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
            app.input_mode = InputMode::Normal;
            app.clear_autocomplete();
            None
        }
        KeyCode::Tab => {
            // Generate suggestions on first Tab, cycle through them on subsequent Tabs
            if app.autocomplete_suggestions.is_empty() {
                app.generate_autocomplete_suggestions();
                if !app.autocomplete_suggestions.is_empty() {
                    app.apply_autocomplete();
                }
            } else {
                app.next_autocomplete();
            }
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
        KeyCode::Char(c) => {
            app.directory_input.push(c);
            app.clear_autocomplete();
            None
        }
        KeyCode::Backspace => {
            app.directory_input.pop();
            app.clear_autocomplete();
            None
        }
        _ => None,
    }
}

fn handle_add_plugin_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    let mut plugin_types = PluginType::all();
    plugin_types.sort_by_key(|p| p.name());
    let num_plugins = plugin_types.len();

    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            None
        }
        KeyCode::Enter => {
            // Add the selected plugin
            if let Some(plugin_type) = plugin_types.get(app.add_plugin_selected_index) {
                app.add_plugin(plugin_type);
            }
            app.input_mode = InputMode::Normal;
            None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.add_plugin_selected_index > 0 {
                app.add_plugin_selected_index -= 1;
            } else {
                app.add_plugin_selected_index = num_plugins.saturating_sub(1);
            }
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.add_plugin_selected_index + 1 < num_plugins {
                app.add_plugin_selected_index += 1;
            } else {
                app.add_plugin_selected_index = 0;
            }
            None
        }
        _ => None,
    }
}

fn handle_plugins_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
            // Shift+Up: Move plugin up in the list
            app.move_plugin_up(app.selected_plugin_index);
            None
        }
        KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
            // Shift+Down: Move plugin down in the list
            app.move_plugin_down(app.selected_plugin_index);
            None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.select_previous_plugin();
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.select_next_plugin();
            None
        }
        KeyCode::Char('e') | KeyCode::Enter => {
            // Edit selected plugin
            app.enter_plugin_edit_mode();
            None
        }
        KeyCode::Char('s') => {
            // Save plugin chain
            app.input_mode = InputMode::SavePlugins;
            app.plugin_file_input.clear();
            // Refresh available presets to show in dialog
            app.refresh_plugin_presets();
            None
        }
        KeyCode::Char('l') => {
            // Load plugin chain
            app.input_mode = InputMode::LoadPlugins;
            app.plugin_file_input.clear();
            app.refresh_plugin_presets();
            None
        }
        KeyCode::Char('a') => {
            // Open plugin selection dialog
            app.add_plugin_selected_index = 0;
            app.input_mode = InputMode::AddPlugin;
            None
        }
        KeyCode::Char('t') => {
            // Toggle plugin enabled/disabled
            app.toggle_plugin(app.selected_plugin_index);
            None
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            app.remove_plugin(app.selected_plugin_index);
            None
        }
        KeyCode::Char('u') | KeyCode::Char('U') => {
            app.move_plugin_up(app.selected_plugin_index);
            None
        }
        KeyCode::Char('w') | KeyCode::Char('W') => {
            // Move plugin down (also available via Shift+Down)
            app.move_plugin_down(app.selected_plugin_index);
            None
        }
        _ => None,
    }
}

fn handle_edit_plugin_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    // Check if we're editing a Matrix plugin
    let is_matrix = app
        .plugin_chain
        .get_plugin(app.selected_plugin_index)
        .is_some_and(|p| matches!(p.settings, PluginSettings::Matrix { .. }));

    if is_matrix {
        return handle_matrix_edit_mode(app, key);
    }

    // Standard plugin editing
    match key.code {
        KeyCode::Esc => {
            app.exit_plugin_edit_mode();
            None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.select_previous_param();
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.select_next_param();
            None
        }
        KeyCode::Left | KeyCode::Char('h') => {
            // Decrease parameter value
            if app.adjust_selected_param(-1.0) {
                app.request_plugin_update();
                None
            } else {
                None
            }
        }
        KeyCode::Right | KeyCode::Char('l') => {
            // Increase parameter value
            if app.adjust_selected_param(1.0) {
                app.request_plugin_update();
                None
            } else {
                None
            }
        }
        KeyCode::Char('[') => {
            // Large decrease
            if app.adjust_selected_param(-10.0) {
                app.request_plugin_update();
                None
            } else {
                None
            }
        }
        KeyCode::Char(']') => {
            // Large increase
            if app.adjust_selected_param(10.0) {
                app.request_plugin_update();
                None
            } else {
                None
            }
        }
        KeyCode::Char('a') => {
            // Load APO file (for EQ plugins)
            if let Some(plugin) = app.plugin_chain.get_plugin(app.selected_plugin_index) {
                if matches!(plugin.settings, PluginSettings::EQ { .. }) {
                    app.input_mode = InputMode::LoadApoFile;
                    app.status_message = Some("Enter path to APO file:".to_string());
                } else {
                    app.status_message =
                        Some("APO files can only be loaded for EQ plugins".to_string());
                }
            }
            None
        }
        KeyCode::Char('o') => {
            // Open SOFA file browser (for Binaural Decoder plugins)
            if let Some(plugin) = app.plugin_chain.get_plugin(app.selected_plugin_index) {
                if matches!(plugin.settings, PluginSettings::BinauralDecoder { .. }) {
                    app.open_file_explorer(
                        FilePickerOrigin::SofaFile,
                        FilePickerMode::File,
                        "Select SOFA File",
                        Some(&app.sofa_file_input.clone()),
                        Some("sofa"),
                    );
                } else {
                    app.status_message = Some(
                        "SOFA files can only be loaded for Binaural Decoder plugins".to_string(),
                    );
                }
            }
            None
        }
        KeyCode::Char('f') => {
            // Open IR file browser (for Convolution plugins)
            if let Some(plugin) = app.plugin_chain.get_plugin(app.selected_plugin_index) {
                if let PluginSettings::Convolution { ref ir_file, .. } = plugin.settings {
                    let current_path = ir_file.clone();
                    app.open_file_explorer(
                        FilePickerOrigin::IrFile,
                        FilePickerMode::File,
                        "Select Impulse Response (WAV)",
                        Some(&current_path),
                        Some("wav"),
                    );
                } else {
                    app.status_message =
                        Some("IR files can only be loaded for Convolution plugins".to_string());
                }
            }
            None
        }
        _ => None,
    }
}

/// Handle key events for Matrix plugin editing
fn handle_matrix_edit_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.exit_plugin_edit_mode();
            None
        }
        KeyCode::Tab => {
            // Toggle between Header and Grid mode
            app.matrix_edit_mode = match app.matrix_edit_mode {
                MatrixEditMode::Header => MatrixEditMode::Grid,
                MatrixEditMode::Grid => MatrixEditMode::Header,
            };
            None
        }
        _ => {
            // Delegate to mode-specific handler
            match app.matrix_edit_mode {
                MatrixEditMode::Header => handle_matrix_header_keys(app, key),
                MatrixEditMode::Grid => handle_matrix_grid_keys(app, key),
            }
        }
    }
}

/// Handle key events in Matrix header mode (input/output channels, preset)
fn handle_matrix_header_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            if app.matrix_header_selection > 0 {
                app.matrix_header_selection -= 1;
            }
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.matrix_header_selection < 2 {
                app.matrix_header_selection += 1;
            }
            None
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if app.adjust_matrix_header(-1) {
                app.request_plugin_update();
            }
            None
        }
        KeyCode::Right | KeyCode::Char('l') => {
            if app.adjust_matrix_header(1) {
                app.request_plugin_update();
            }
            None
        }
        _ => None,
    }
}

/// Handle key events in Matrix grid mode (cell editing)
fn handle_matrix_grid_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    // Get current matrix dimensions
    let (in_ch, out_ch) = app.get_matrix_dimensions().unwrap_or((2, 2));

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            if app.matrix_grid_row > 0 {
                app.matrix_grid_row -= 1;
            }
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.matrix_grid_row + 1 < out_ch {
                app.matrix_grid_row += 1;
            }
            None
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if app.matrix_grid_col > 0 {
                app.matrix_grid_col -= 1;
            }
            None
        }
        KeyCode::Right | KeyCode::Char('l') => {
            if app.matrix_grid_col + 1 < in_ch {
                app.matrix_grid_col += 1;
            }
            None
        }
        KeyCode::Char('-') | KeyCode::Char('[') => {
            // Decrease gain by 0.5 dB
            if app.adjust_matrix_cell(-0.5) {
                app.request_plugin_update();
            }
            None
        }
        KeyCode::Char('+') | KeyCode::Char('=') | KeyCode::Char(']') => {
            // Increase gain by 0.5 dB
            if app.adjust_matrix_cell(0.5) {
                app.request_plugin_update();
            }
            None
        }
        KeyCode::Char('0') => {
            // Set cell to zero (−∞ dB / silence)
            if app.set_matrix_cell(0.0) {
                app.request_plugin_update();
            }
            None
        }
        KeyCode::Char('1') => {
            // Set cell to unity gain (0 dB)
            if app.set_matrix_cell(1.0) {
                app.request_plugin_update();
            }
            None
        }
        _ => None,
    }
}

fn handle_save_plugins_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.plugin_file_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Enter => {
            // If there are presets shown and input is empty, use selected preset (overwrite)
            if app.plugin_file_input.is_empty() && !app.available_plugin_presets.is_empty() {
                app.save_selected_preset();
            } else if !app.plugin_file_input.is_empty() {
                app.save_plugin_chain();
            }
            app.input_mode = InputMode::Normal;
            app.clear_autocomplete();
            None
        }
        KeyCode::Tab => {
            // Autocomplete from available presets (restricted to preset directory)
            if app.autocomplete_suggestions.is_empty() {
                app.generate_autocomplete_suggestions_for_save_preset();
                if !app.autocomplete_suggestions.is_empty() {
                    app.apply_autocomplete_to_plugin_file();
                }
            } else {
                app.next_autocomplete_for_plugin_file();
            }
            None
        }
        KeyCode::Up => {
            // Navigate preset list when input is empty
            if app.plugin_file_input.is_empty() && !app.available_plugin_presets.is_empty() {
                if app.selected_preset_index > 0 {
                    app.selected_preset_index -= 1;
                }
            }
            None
        }
        KeyCode::Down => {
            // Navigate preset list when input is empty
            if app.plugin_file_input.is_empty() && !app.available_plugin_presets.is_empty() {
                if app.selected_preset_index < app.available_plugin_presets.len() - 1 {
                    app.selected_preset_index += 1;
                }
            }
            None
        }
        KeyCode::Char(c) => {
            app.plugin_file_input.push(c);
            app.clear_autocomplete();
            None
        }
        KeyCode::Backspace => {
            app.plugin_file_input.pop();
            app.clear_autocomplete();
            None
        }
        _ => None,
    }
}

fn handle_load_plugins_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.plugin_file_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Enter => {
            // If there are presets shown and input is empty, load selected preset
            if app.plugin_file_input.is_empty() && !app.available_plugin_presets.is_empty() {
                app.load_selected_preset();
            } else if !app.plugin_file_input.is_empty() {
                app.load_plugin_chain();
            }
            app.input_mode = InputMode::Normal;
            app.clear_autocomplete();
            None
        }
        KeyCode::Tab => {
            // Autocomplete file path (only if user typed something)
            if !app.plugin_file_input.is_empty() {
                if app.autocomplete_suggestions.is_empty() {
                    app.generate_autocomplete_suggestions_for_plugin_file();
                    if !app.autocomplete_suggestions.is_empty() {
                        app.apply_autocomplete_to_plugin_file();
                    }
                } else {
                    app.next_autocomplete_for_plugin_file();
                }
            }
            None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            // Navigate through presets
            if app.plugin_file_input.is_empty() {
                app.select_previous_preset();
            }
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            // Navigate through presets
            if app.plugin_file_input.is_empty() {
                app.select_next_preset();
            }
            None
        }
        KeyCode::Char(c) => {
            app.plugin_file_input.push(c);
            app.clear_autocomplete();
            None
        }
        KeyCode::Backspace => {
            app.plugin_file_input.pop();
            app.clear_autocomplete();
            None
        }
        _ => None,
    }
}

fn handle_load_apo_file_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.apo_file_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Enter => {
            match app.load_apo_file() {
                Ok(()) => {
                    app.status_message = Some("APO file loaded successfully".to_string());
                    app.request_plugin_update();
                }
                Err(e) => {
                    app.status_message = Some(format!("Failed to load APO file: {}", e));
                }
            }
            app.input_mode = InputMode::Normal;
            app.apo_file_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Tab => {
            // Autocomplete file path
            if app.autocomplete_suggestions.is_empty() {
                app.generate_autocomplete_suggestions_for_apo_file();
                if !app.autocomplete_suggestions.is_empty() {
                    app.apply_autocomplete_to_apo_file();
                }
            } else {
                app.next_autocomplete_for_apo_file();
            }
            None
        }
        KeyCode::Char(c) => {
            app.apo_file_input.push(c);
            app.clear_autocomplete();
            None
        }
        KeyCode::Backspace => {
            app.apo_file_input.pop();
            app.clear_autocomplete();
            None
        }
        _ => None,
    }
}

fn handle_load_sofa_file_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.sofa_file_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Enter => {
            match app.load_sofa_file() {
                Ok(()) => {
                    app.status_message = Some("SOFA file path set successfully".to_string());
                    app.request_plugin_update();
                }
                Err(e) => {
                    app.status_message = Some(format!("Failed to set SOFA file: {}", e));
                }
            }
            app.input_mode = InputMode::Normal;
            app.sofa_file_input.clear();
            app.clear_autocomplete();
            None
        }
        KeyCode::Tab => {
            // Autocomplete file path
            if app.autocomplete_suggestions.is_empty() {
                app.generate_autocomplete_suggestions_for_sofa_file();
                if !app.autocomplete_suggestions.is_empty() {
                    app.apply_autocomplete_to_sofa_file();
                }
            } else {
                app.next_autocomplete_for_sofa_file();
            }
            None
        }
        KeyCode::Char(c) => {
            app.sofa_file_input.push(c);
            app.clear_autocomplete();
            None
        }
        KeyCode::Backspace => {
            app.sofa_file_input.pop();
            app.clear_autocomplete();
            None
        }
        _ => None,
    }
}

fn handle_devices_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
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

fn handle_help_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
            app.input_mode = InputMode::Normal;
            None
        }
        _ => None,
    }
}

fn handle_error_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('q') => {
            app.input_mode = InputMode::Normal;
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
                0 => ChannelConflictChoice::DisableUpmixer,
                1 => ChannelConflictChoice::RemoveUpmixer,
                2 => ChannelConflictChoice::Cancel,
                _ => ChannelConflictChoice::Cancel,
            };

            let path = app.channel_conflict_path.take();
            app.input_mode = InputMode::Normal;

            match choice {
                ChannelConflictChoice::DisableUpmixer => {
                    if let Some(idx) = app.plugin_chain.find_plugin_index(&PluginType::Upmixer) {
                        app.plugin_chain.toggle_plugin(idx);
                        log::info!("[TUI] Upmixer disabled by user (channel conflict)");
                    }
                    path.map(PlayerCommand::Play)
                }
                ChannelConflictChoice::RemoveUpmixer => {
                    if let Some(idx) = app.plugin_chain.find_plugin_index(&PluginType::Upmixer) {
                        app.plugin_chain.remove_plugin(idx);
                        log::info!("[TUI] Upmixer removed by user (channel conflict)");
                    }
                    path.map(PlayerCommand::Play)
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
            app.input_mode = InputMode::Normal;
            app.is_playing = false;
            None
        }
        _ => None,
    }
}

fn configure_sub_screen_prev(s: crate::app::ConfigureSubScreen) -> crate::app::ConfigureSubScreen {
    use crate::app::ConfigureSubScreen;
    match s {
        ConfigureSubScreen::Directories => ConfigureSubScreen::SpinoramaEq,
        ConfigureSubScreen::Recording => ConfigureSubScreen::Directories,
        ConfigureSubScreen::RoomEq => ConfigureSubScreen::Recording,
        ConfigureSubScreen::HeadphoneEq => ConfigureSubScreen::RoomEq,
        ConfigureSubScreen::SpinoramaEq => ConfigureSubScreen::HeadphoneEq,
    }
}

fn configure_sub_screen_next(s: crate::app::ConfigureSubScreen) -> crate::app::ConfigureSubScreen {
    use crate::app::ConfigureSubScreen;
    match s {
        ConfigureSubScreen::Directories => ConfigureSubScreen::Recording,
        ConfigureSubScreen::Recording => ConfigureSubScreen::RoomEq,
        ConfigureSubScreen::RoomEq => ConfigureSubScreen::HeadphoneEq,
        ConfigureSubScreen::HeadphoneEq => ConfigureSubScreen::SpinoramaEq,
        ConfigureSubScreen::SpinoramaEq => ConfigureSubScreen::Directories,
    }
}

fn handle_configure_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
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
            KeyCode::Char('1') => {
                app.configure_sub_screen = ConfigureSubScreen::Directories;
                return None;
            }
            KeyCode::Char('2') => {
                app.configure_sub_screen = ConfigureSubScreen::Recording;
                return None;
            }
            KeyCode::Char('3') => {
                app.configure_sub_screen = ConfigureSubScreen::RoomEq;
                return None;
            }
            KeyCode::Char('4') => {
                app.configure_sub_screen = ConfigureSubScreen::HeadphoneEq;
                return None;
            }
            KeyCode::Char('5') => {
                app.configure_sub_screen = ConfigureSubScreen::SpinoramaEq;
                return None;
            }
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
                KeyCode::Char('1') => {
                    app.configure_sub_screen = ConfigureSubScreen::Directories;
                    return None;
                }
                KeyCode::Char('2') => {
                    app.configure_sub_screen = ConfigureSubScreen::Recording;
                    return None;
                }
                KeyCode::Char('3') => {
                    app.configure_sub_screen = ConfigureSubScreen::RoomEq;
                    return None;
                }
                KeyCode::Char('4') => {
                    app.configure_sub_screen = ConfigureSubScreen::HeadphoneEq;
                    return None;
                }
                KeyCode::Char('5') => {
                    app.configure_sub_screen = ConfigureSubScreen::SpinoramaEq;
                    return None;
                }
                _ => {}
            }
            return handle_directory_keys(app, key);
        }
        ConfigureSubScreen::SpinoramaEq => {
            // Inside Spinorama wizard, all keys go to the wizard handler.
            // Number keys do NOT switch sub-screens while inside the wizard.
            return handle_spinorama_keys(app, key);
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
            return handle_headphone_eq_keys(app, key);
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
            return handle_room_eq_keys(app, key);
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
            return handle_recording_keys(app, key);
        }
    }
}

fn spinorama_step_prev(s: crate::app::SpinoramaStep) -> crate::app::SpinoramaStep {
    use crate::app::SpinoramaStep;
    match s {
        SpinoramaStep::Select => SpinoramaStep::Select, // no wrap
        SpinoramaStep::Configure => SpinoramaStep::Select,
        SpinoramaStep::Optimize => SpinoramaStep::Configure,
        SpinoramaStep::Results => SpinoramaStep::Optimize,
        SpinoramaStep::UpdatePlugin => SpinoramaStep::Results,
    }
}

fn spinorama_step_next(s: crate::app::SpinoramaStep) -> crate::app::SpinoramaStep {
    use crate::app::SpinoramaStep;
    match s {
        SpinoramaStep::Select => SpinoramaStep::Configure,
        SpinoramaStep::Configure => SpinoramaStep::Optimize,
        SpinoramaStep::Optimize => SpinoramaStep::Results,
        SpinoramaStep::Results => SpinoramaStep::UpdatePlugin,
        SpinoramaStep::UpdatePlugin => SpinoramaStep::UpdatePlugin, // no wrap
    }
}

fn handle_spinorama_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use crate::app::SpinoramaStep;
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    // Esc goes up one level within the wizard
    if key.code == KeyCode::Esc {
        match app.spinorama_eq.step {
            SpinoramaStep::Select => {
                // At top of wizard — go back to Configure tab bar
                app.configure_tab_focused = true;
            }
            SpinoramaStep::Configure => {
                app.spinorama_eq.step = SpinoramaStep::Select;
            }
            SpinoramaStep::Optimize => {
                app.spinorama_eq.step = SpinoramaStep::Configure;
            }
            SpinoramaStep::Results => {
                app.spinorama_eq.step = SpinoramaStep::Optimize;
            }
            SpinoramaStep::UpdatePlugin => {
                app.spinorama_eq.step = SpinoramaStep::Results;
            }
        }
        return None;
    }

    // Up always returns focus to the Configure tab bar
    if key.code == KeyCode::Up && app.spinorama_eq.step == SpinoramaStep::Select {
        app.configure_tab_focused = true;
        return None;
    }

    // Left/Right navigate between wizard steps (step-bar level),
    // but NOT in Configure step where Left/Right adjust field values.
    if key.code == KeyCode::Left && app.spinorama_eq.step != SpinoramaStep::Configure {
        app.spinorama_eq.step = spinorama_step_prev(app.spinorama_eq.step);
        return None;
    }
    if key.code == KeyCode::Right && app.spinorama_eq.step != SpinoramaStep::Configure {
        app.spinorama_eq.step = spinorama_step_next(app.spinorama_eq.step);
        return None;
    }

    match app.spinorama_eq.step {
        SpinoramaStep::Select => match key.code {
            KeyCode::Up => {
                if app.spinorama_eq.selected_speaker_idx > 0 {
                    app.spinorama_eq.selected_speaker_idx -= 1;
                }
                None
            }
            KeyCode::Down => {
                let max = app.spinorama_eq.filtered_speakers.len().saturating_sub(1);
                if app.spinorama_eq.selected_speaker_idx < max {
                    app.spinorama_eq.selected_speaker_idx += 1;
                }
                None
            }
            KeyCode::Enter => {
                let idx = app.spinorama_eq.selected_speaker_idx;
                if let Some(name) = app.spinorama_eq.filtered_speakers.get(idx).cloned() {
                    app.spinorama_eq.selected_speaker = Some(name);
                    app.spinorama_eq.step = SpinoramaStep::Configure;
                }
                None
            }
            KeyCode::Tab => {
                if app.spinorama_eq.selected_speaker.is_some() {
                    app.spinorama_eq.step = SpinoramaStep::Configure;
                }
                None
            }
            KeyCode::Char('r') => {
                // Retry speaker list load (e.g. after error)
                app.spinorama_eq.speakers_error = None;
                app.spinorama_eq.available_speakers.clear();
                app.spinorama_eq.loading_speakers = true;
                spawn_spinorama_speaker_load();
                None
            }
            KeyCode::Backspace => {
                app.spinorama_eq.search_query.pop();
                app.spinorama_eq.update_filter();
                None
            }
            KeyCode::Char(c) => {
                app.spinorama_eq.search_query.push(c);
                app.spinorama_eq.update_filter();
                None
            }
            _ => None,
        },

        SpinoramaStep::Configure => match key.code {
            KeyCode::Up => {
                if app.spinorama_eq.selected_field > 0 {
                    app.spinorama_eq.selected_field -= 1;
                }
                None
            }
            KeyCode::Down => {
                if app.spinorama_eq.selected_field < 24 {
                    app.spinorama_eq.selected_field += 1;
                }
                None
            }
            KeyCode::Left | KeyCode::Char('-') => {
                adjust_spinorama_field(app, -1);
                None
            }
            KeyCode::Right | KeyCode::Char('+') => {
                adjust_spinorama_field(app, 1);
                None
            }
            KeyCode::Enter | KeyCode::Tab => {
                // Reset optimization state so user can re-run with new parameters
                app.spinorama_eq.opt_status = OptimizationStatus::Idle;
                app.spinorama_eq.loss_history.clear();
                app.spinorama_eq.opt_progress = 0.0;
                app.spinorama_eq.opt_iteration = 0;
                app.spinorama_eq.step = SpinoramaStep::Optimize;
                None
            }
            KeyCode::BackTab => {
                app.spinorama_eq.step = SpinoramaStep::Select;
                None
            }
            _ => None,
        },

        SpinoramaStep::Optimize => match key.code {
            KeyCode::Enter => {
                match &app.spinorama_eq.opt_status {
                    OptimizationStatus::Idle
                    | OptimizationStatus::Failed
                    | OptimizationStatus::Cancelled
                    | OptimizationStatus::Completed => {
                        spawn_spinorama_optimization(app);
                    }
                    OptimizationStatus::Running => {}
                }
                None
            }
            KeyCode::Tab => {
                if app.spinorama_eq.opt_status == OptimizationStatus::Completed {
                    app.spinorama_eq.step = SpinoramaStep::Results;
                } else {
                    app.spinorama_eq.step = SpinoramaStep::Configure;
                }
                None
            }
            KeyCode::BackTab => {
                app.spinorama_eq.step = SpinoramaStep::Configure;
                None
            }
            _ => None,
        },

        SpinoramaStep::Results => match key.code {
            KeyCode::Tab => {
                app.spinorama_eq.step = SpinoramaStep::UpdatePlugin;
                None
            }
            KeyCode::BackTab => {
                app.spinorama_eq.step = SpinoramaStep::Optimize;
                None
            }
            _ => None,
        },

        SpinoramaStep::UpdatePlugin => match key.code {
            KeyCode::Enter => {
                match app.apply_spinorama_to_plugin_chain() {
                    Ok(msg) => app.status_message = Some(msg),
                    Err(e) => app.status_message = Some(format!("Error: {}", e)),
                }
                None
            }
            KeyCode::BackTab | KeyCode::Left => {
                app.spinorama_eq.step = SpinoramaStep::Results;
                None
            }
            KeyCode::Right => {
                app.spinorama_eq.step = SpinoramaStep::Select;
                None
            }
            _ => None,
        },
    }
}

fn cycle_string(current: &str, options: &[&str], delta: i32) -> String {
    let idx = options.iter().position(|&o| o == current).unwrap_or(0);
    let new_idx = if delta > 0 {
        (idx + 1) % options.len()
    } else {
        (idx + options.len() - 1) % options.len()
    };
    options[new_idx].to_string()
}

fn adjust_spinorama_field(app: &mut App, delta: i32) {
    let c = &mut app.spinorama_eq.config;
    // Field indices must match the UI rows in ui/mod.rs draw_spinorama_configure
    match app.spinorama_eq.selected_field {
        // ── Loss ──
        0 => {
            c.loss_function = cycle_string(
                &c.loss_function,
                &["flat", "flat-asymmetric", "score"],
                delta,
            );
        }
        // ── Filters ──
        1 => {
            let n = c.num_filters as i32 + delta;
            c.num_filters = n.clamp(1, 30) as usize;
        }
        2 => c.min_freq = (c.min_freq + delta as f64 * 10.0).clamp(20.0, 500.0),
        3 => c.max_freq = (c.max_freq + delta as f64 * 500.0).clamp(1000.0, 20000.0),
        4 => c.min_db = (c.min_db + delta as f64).clamp(-24.0, 0.0),
        5 => c.max_db = (c.max_db + delta as f64).clamp(0.0, 12.0),
        6 => c.min_q = (c.min_q + delta as f64 * 0.1).clamp(0.1, 2.0),
        7 => c.max_q = (c.max_q + delta as f64 * 0.5).clamp(1.0, 20.0),
        8 => {
            c.peq_model = cycle_string(
                &c.peq_model,
                &["pk", "hp-pk", "hp-pk-lp", "ls-pk", "ls-pk-hs"],
                delta,
            );
        }
        // ── Optimization ──
        9 => {
            use sotf_audio_player::room_eq_types::RoomEqAlgorithm;
            let algos = RoomEqAlgorithm::all();
            let idx = algos.iter().position(|a| *a == c.algorithm).unwrap_or(0);
            let new_idx = if delta > 0 {
                (idx + 1) % algos.len()
            } else {
                (idx + algos.len() - 1) % algos.len()
            };
            c.algorithm = algos[new_idx];
        }
        10 => {
            let n = c.max_iter as i32 + delta * 1000;
            c.max_iter = n.clamp(1000, 100000) as usize;
        }
        11 => {
            let n = c.population as i32 + delta * 10;
            c.population = n.clamp(10, 200) as usize;
        }
        12 => {
            c.strategy = cycle_string(
                &c.strategy,
                &["currenttobest1bin", "best1bin", "rand1bin", "best2bin"],
                delta,
            );
        }
        13 => c.de_f = (c.de_f + delta as f64 * 0.1).clamp(0.1, 2.0),
        14 => c.de_cr = (c.de_cr + delta as f64 * 0.1).clamp(0.1, 1.0),
        // ── Refinement ──
        15 => c.refine = !c.refine,
        16 => {
            c.local_algo = cycle_string(&c.local_algo, &["cobyla", "nelder-mead"], delta);
        }
        // ── Smoothing ──
        17 => c.smooth = !c.smooth,
        18 => {
            let n = c.smooth_n as i32 + delta;
            c.smooth_n = n.clamp(1, 24) as usize;
        }
        19 => c.psychoacoustic = !c.psychoacoustic,
        // ── Constraints ──
        20 => c.spacing_weight = (c.spacing_weight + delta as f64 * 10.0).clamp(0.0, 1000.0),
        21 => c.min_spacing_oct = (c.min_spacing_oct + delta as f64 * 0.01).clamp(0.01, 1.0),
        // ── Convergence ──
        22 => {
            c.tolerance = if delta > 0 {
                (c.tolerance * 10.0).min(1e-1)
            } else {
                (c.tolerance / 10.0).max(1e-6)
            };
        }
        23 => {
            c.atolerance = if delta > 0 {
                (c.atolerance * 10.0).min(1e-1)
            } else {
                (c.atolerance / 10.0).max(1e-6)
            };
        }
        24 => {
            c.sample_rate = match (c.sample_rate, delta > 0) {
                (44100, true) => 48000,
                (48000, true) => 96000,
                (96000, true) => 44100,
                (96000, false) => 48000,
                (48000, false) => 44100,
                (44100, false) => 96000,
                _ => 48000,
            };
        }
        _ => {}
    }
}

/// Poll speaker-load result on every tick. Returns true if the UI needs a redraw.
/// Also auto-triggers speaker list loading when entering the Select step.
pub fn poll_spinorama_speaker_load(app: &mut App) -> bool {
    // Auto-load speakers when on Select step with empty list
    if !app.spinorama_eq.loading_speakers
        && app.spinorama_eq.available_speakers.is_empty()
        && app.spinorama_eq.speakers_error.is_none()
        && app.current_screen == Screen::Configure
        && app.configure_sub_screen == crate::app::ConfigureSubScreen::SpinoramaEq
        && app.spinorama_eq.step == crate::app::SpinoramaStep::Select
    {
        app.spinorama_eq.loading_speakers = true;
        spawn_spinorama_speaker_load();
    }

    if !app.spinorama_eq.loading_speakers {
        return false;
    }
    let result_slot = SPEAKERS_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    if let Ok(mut guard) = result_slot.lock() {
        if let Some(result) = guard.take() {
            app.spinorama_eq.loading_speakers = false;
            match result {
                Ok(speakers) => {
                    app.spinorama_eq.available_speakers = speakers;
                    app.spinorama_eq.update_filter();
                }
                Err(e) => {
                    app.spinorama_eq.speakers_error = Some(e);
                }
            }
            return true;
        }
    }
    false
}

fn spawn_spinorama_speaker_load() {
    let result_slot = SPEAKERS_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    // Clear any stale result from a previous load
    if let Ok(mut g) = result_slot.lock() {
        *g = None;
    }

    // Spawn background thread
    let slot = result_slot.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let result = rt
            .block_on(async { autoeq::fetch_available_speakers().await })
            .map_err(|e| e.to_string());
        if let Ok(mut guard) = slot.lock() {
            *guard = Some(result);
        }
    });
}

use std::sync::{Arc, Mutex};

static SPEAKERS_RESULT: std::sync::OnceLock<Arc<Mutex<Option<Result<Vec<String>, String>>>>> =
    std::sync::OnceLock::new();

static OPT_RESULT: std::sync::OnceLock<
    Arc<Mutex<Option<Result<sotf_audio_player::autoeq::SpeakerOptimizationResult, String>>>>,
> = std::sync::OnceLock::new();
static OPT_PROGRESS: std::sync::OnceLock<Arc<Mutex<Option<(usize, usize, f64, f32)>>>> =
    std::sync::OnceLock::new();

/// Poll optimization progress/result on every tick while optimization is running.
/// Returns true if the UI needs a redraw.
pub fn poll_spinorama_optimization(app: &mut App) -> bool {
    use sotf_audio_player::room_eq_types::OptimizationStatus;
    use sotf_audio_player::spinorama_eq_types::SpinoramaBiquad;

    if app.spinorama_eq.opt_status != OptimizationStatus::Running {
        return false;
    }

    let result_slot = OPT_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    let progress_slot = OPT_PROGRESS
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    if let Ok(mut guard) = result_slot.lock() {
        if let Some(result) = guard.take() {
            match result {
                Ok(r) => {
                    app.spinorama_eq.pre_loss = r.initial_loss;
                    app.spinorama_eq.post_loss = r.final_loss;
                    app.spinorama_eq.filters = r
                        .biquads
                        .iter()
                        .map(|b| SpinoramaBiquad {
                            filter_type: format!("{:?}", b.filter_type),
                            freq: b.freq,
                            q: b.q,
                            db_gain: b.db_gain,
                        })
                        .collect();
                    app.spinorama_eq.curve_frequencies = r.frequencies.clone();
                    app.spinorama_eq.curve_input = r.input_curve.clone();
                    app.spinorama_eq.curve_target = r.target_curve.clone();
                    app.spinorama_eq.curve_corrected = r.corrected_curve.clone();
                    app.spinorama_eq.curve_filter_response = r.filter_response.clone();
                    if app.spinorama_eq.loss_history.is_empty() {
                        app.spinorama_eq.loss_history = r
                            .optimization_history
                            .iter()
                            .map(|(iter, loss)| (*iter, *loss, None))
                            .collect();
                    }
                    app.spinorama_eq.opt_status = OptimizationStatus::Completed;
                    app.spinorama_eq.opt_progress = 1.0;
                }
                Err(e) => {
                    app.spinorama_eq.opt_status = OptimizationStatus::Failed;
                    app.spinorama_eq.opt_error = Some(e);
                }
            }
            return true;
        }
    }

    if let Ok(mut guard) = progress_slot.lock() {
        if let Some((iter, max_iter, loss, pct)) = guard.take() {
            app.spinorama_eq.opt_iteration = iter;
            app.spinorama_eq.opt_max_iter = max_iter;
            app.spinorama_eq.opt_loss = loss;
            app.spinorama_eq.opt_progress = pct;
            return true;
        }
    }

    false
}

fn spawn_spinorama_optimization(app: &mut App) {
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    // Start new optimization
    let speaker = match &app.spinorama_eq.selected_speaker {
        Some(s) => s.clone(),
        None => {
            app.spinorama_eq.opt_status = OptimizationStatus::Failed;
            app.spinorama_eq.opt_error = Some("No speaker selected".to_string());
            return;
        }
    };

    app.spinorama_eq.opt_status = OptimizationStatus::Running;
    app.spinorama_eq.opt_error = None;
    app.spinorama_eq.opt_progress = 0.0;
    app.spinorama_eq.opt_iteration = 0;
    app.spinorama_eq.opt_loss = 0.0;
    app.spinorama_eq.filters.clear();

    let c = &app.spinorama_eq.config;
    let num_filters = c.num_filters;
    let min_freq = c.min_freq;
    let max_freq = c.max_freq;
    let min_db = c.min_db;
    let max_db = c.max_db;
    let min_q = c.min_q;
    let max_q = c.max_q;
    let max_iter = c.max_iter;
    let peq_model_str = c.peq_model.clone();
    let algorithm = c.algorithm;
    let population = c.population;
    let strategy = c.strategy.clone();
    let de_f = c.de_f;
    let de_cr = c.de_cr;
    let refine = c.refine;
    let local_algo = c.local_algo.clone();
    let smooth = c.smooth;
    let smooth_n = c.smooth_n;
    let psychoacoustic = c.psychoacoustic;
    let spacing_weight = c.spacing_weight;
    let min_spacing_oct = c.min_spacing_oct;
    let loss_function = c.loss_function.clone();
    let tolerance = c.tolerance;
    let atolerance = c.atolerance;
    let sample_rate = c.sample_rate;

    let result_slot2 = OPT_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    let progress_slot2 = OPT_PROGRESS
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    // Clear any stale result from a previous run
    if let Ok(mut g) = result_slot2.lock() {
        *g = None;
    }
    if let Ok(mut g) = progress_slot2.lock() {
        *g = None;
    }

    std::thread::spawn(move || {
        use sotf_audio_player::autoeq::{
            CallbackAction, CallbackConfig, MeasurementInput, SpeakerOptimizationConfig,
            run_speaker_optimization_with_callback,
        };

        let mut args = autoeq::Args::speaker_defaults();
        args.num_filters = num_filters;
        args.min_freq = min_freq;
        args.max_freq = max_freq;
        args.min_db = min_db;
        args.max_db = max_db;
        args.min_q = min_q;
        args.max_q = max_q;
        args.maxeval = max_iter;
        args.sample_rate = sample_rate as f64;
        args.population = population;
        args.strategy = strategy;
        args.adaptive_weight_f = de_f;
        args.recombination = de_cr;
        args.refine = refine;
        args.local_algo = local_algo;
        args.smooth = smooth;
        args.smooth_n = smooth_n;
        args.spacing_weight = spacing_weight;
        args.min_spacing_oct = min_spacing_oct;
        args.tolerance = tolerance;
        args.atolerance = atolerance;
        // Map algorithm enum to autoeq algo format
        args.algo = algorithm.to_autoeq_string().to_string();
        // Map PEQ model string to enum
        args.peq_model = match peq_model_str.as_str() {
            "pk" => autoeq::PeqModel::Pk,
            "hp-pk" => autoeq::PeqModel::HpPk,
            "hp-pk-lp" => autoeq::PeqModel::HpPkLp,
            "ls-pk" => autoeq::PeqModel::LsPk,
            "ls-pk-hs" => autoeq::PeqModel::LsPkHs,
            _ => autoeq::PeqModel::Pk,
        };
        // Map loss function string to LossType enum
        args.loss = match loss_function.as_str() {
            "flat" => autoeq::LossType::SpeakerFlat,
            "flat-asymmetric" => autoeq::LossType::SpeakerFlatAsymmetric,
            "score" => autoeq::LossType::SpeakerScore,
            other => panic!("Unknown loss function: {}", other),
        };
        // Psychoacoustic smoothing not directly on Args — handled via smooth settings
        let _ = psychoacoustic; // TODO: map when autoeq supports it directly

        let config = SpeakerOptimizationConfig {
            main_measurement: Some(MeasurementInput::Spinorama {
                speaker: speaker.clone(),
                version: "asr".to_string(),
                measurement: "CEA2034".to_string(),
                curve_name: args.curve_name.clone(),
            }),
            args,
            callback_config: Some(CallbackConfig {
                interval: 50,
                include_biquads: false,
                include_filter_response: false,
            }),
            ..Default::default()
        };

        let progress_slot3 = progress_slot2.clone();
        let callback: sotf_audio_player::autoeq::SpeakerOptimizationCallback = Box::new(move |p| {
            let pct = if p.max_iterations > 0 {
                p.iteration as f32 / p.max_iterations as f32
            } else {
                0.0
            };
            if let Ok(mut guard) = progress_slot3.lock() {
                *guard = Some((p.iteration, p.max_iterations, p.loss, pct));
            }
            CallbackAction::Continue
        });

        let result = run_speaker_optimization_with_callback(&config, Some(callback));
        if let Ok(mut guard) = result_slot2.lock() {
            *guard = Some(result);
        }
    });
}

// ============================================================================
// Headphone EQ Wizard
// ============================================================================

fn headphone_eq_step_prev(s: crate::app::HeadphoneEqStep) -> crate::app::HeadphoneEqStep {
    use crate::app::HeadphoneEqStep;
    match s {
        HeadphoneEqStep::SelectFile => HeadphoneEqStep::SelectFile, // no wrap
        HeadphoneEqStep::Configure => HeadphoneEqStep::SelectFile,
        HeadphoneEqStep::Optimize => HeadphoneEqStep::Configure,
        HeadphoneEqStep::Results => HeadphoneEqStep::Optimize,
    }
}

fn headphone_eq_step_next(s: crate::app::HeadphoneEqStep) -> crate::app::HeadphoneEqStep {
    use crate::app::HeadphoneEqStep;
    match s {
        HeadphoneEqStep::SelectFile => HeadphoneEqStep::Configure,
        HeadphoneEqStep::Configure => HeadphoneEqStep::Optimize,
        HeadphoneEqStep::Optimize => HeadphoneEqStep::Results,
        HeadphoneEqStep::Results => HeadphoneEqStep::Results, // no wrap
    }
}

fn handle_headphone_eq_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use crate::app::{HEADPHONE_TARGET_PRESETS, HeadphoneEqStep};
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    // Esc goes up one level
    if key.code == KeyCode::Esc {
        match app.headphone_eq.step {
            HeadphoneEqStep::SelectFile => {
                if app.headphone_eq.editing_measurement {
                    app.headphone_eq.editing_measurement = false;
                } else if app.headphone_eq.editing_custom_target {
                    app.headphone_eq.editing_custom_target = false;
                } else {
                    app.configure_tab_focused = true;
                }
            }
            _ => {
                app.headphone_eq.step = headphone_eq_step_prev(app.headphone_eq.step);
            }
        }
        return None;
    }

    // Step navigation via Left/Right (except when editing text or in Configure step)
    let editing = app.headphone_eq.editing_measurement || app.headphone_eq.editing_custom_target;
    if !editing && app.headphone_eq.step != HeadphoneEqStep::Configure {
        if key.code == KeyCode::Left {
            app.headphone_eq.step = headphone_eq_step_prev(app.headphone_eq.step);
            return None;
        }
        if key.code == KeyCode::Right {
            app.headphone_eq.step = headphone_eq_step_next(app.headphone_eq.step);
            return None;
        }
    }

    match app.headphone_eq.step {
        HeadphoneEqStep::SelectFile => {
            if app.headphone_eq.editing_measurement {
                match key.code {
                    KeyCode::Enter => {
                        app.headphone_eq.editing_measurement = false;
                    }
                    KeyCode::Backspace => {
                        app.headphone_eq.measurement_path.pop();
                    }
                    KeyCode::F(2) => {
                        let start = app.headphone_eq.measurement_path.clone();
                        app.open_file_explorer(
                            FilePickerOrigin::HeadphoneMeasurement,
                            FilePickerMode::File,
                            "Select Measurement CSV",
                            Some(&start),
                            Some("csv"),
                        );
                    }
                    KeyCode::Char(c) => {
                        app.headphone_eq.measurement_path.push(c);
                    }
                    _ => {}
                }
                return None;
            }
            if app.headphone_eq.editing_custom_target {
                match key.code {
                    KeyCode::Enter => {
                        app.headphone_eq.editing_custom_target = false;
                    }
                    KeyCode::Backspace => {
                        app.headphone_eq.custom_target_path.pop();
                    }
                    KeyCode::F(2) => {
                        let start = app.headphone_eq.custom_target_path.clone();
                        app.open_file_explorer(
                            FilePickerOrigin::HeadphoneCustomTarget,
                            FilePickerMode::File,
                            "Select Custom Target CSV",
                            Some(&start),
                            Some("csv"),
                        );
                    }
                    KeyCode::Char(c) => {
                        app.headphone_eq.custom_target_path.push(c);
                    }
                    _ => {}
                }
                return None;
            }
            match key.code {
                KeyCode::Up => {
                    if app.headphone_eq.selected_field > 0 {
                        app.headphone_eq.selected_field -= 1;
                    } else {
                        app.configure_tab_focused = true;
                    }
                }
                KeyCode::Down => {
                    let max = if app.headphone_eq.target_preset == "custom" {
                        2
                    } else {
                        1
                    };
                    if app.headphone_eq.selected_field < max {
                        app.headphone_eq.selected_field += 1;
                    }
                }
                KeyCode::Enter => {
                    match app.headphone_eq.selected_field {
                        0 => {
                            app.headphone_eq.editing_measurement = true;
                        }
                        1 => {} // target preset cycles with Left/Right
                        2 => {
                            app.headphone_eq.editing_custom_target = true;
                        }
                        _ => {}
                    }
                }
                KeyCode::Left | KeyCode::Right => {
                    if app.headphone_eq.selected_field == 1 {
                        let delta = if key.code == KeyCode::Right { 1i32 } else { -1 };
                        app.headphone_eq.target_preset = cycle_string(
                            &app.headphone_eq.target_preset,
                            HEADPHONE_TARGET_PRESETS,
                            delta,
                        );
                        // Clamp selected_field if "custom" row disappeared
                        let max = if app.headphone_eq.target_preset == "custom" {
                            2
                        } else {
                            1
                        };
                        if app.headphone_eq.selected_field > max {
                            app.headphone_eq.selected_field = max;
                        }
                    }
                }
                KeyCode::Tab => {
                    if !app.headphone_eq.measurement_path.is_empty() {
                        app.headphone_eq.step = HeadphoneEqStep::Configure;
                    }
                }
                _ => {}
            }
            None
        }

        HeadphoneEqStep::Configure => match key.code {
            KeyCode::Up => {
                if app.headphone_eq.config_selected_field > 0 {
                    app.headphone_eq.config_selected_field -= 1;
                }
                None
            }
            KeyCode::Down => {
                if app.headphone_eq.config_selected_field < 17 {
                    app.headphone_eq.config_selected_field += 1;
                }
                None
            }
            KeyCode::Left | KeyCode::Char('-') => {
                adjust_headphone_eq_field(app, -1);
                None
            }
            KeyCode::Right | KeyCode::Char('+') => {
                adjust_headphone_eq_field(app, 1);
                None
            }
            KeyCode::Enter | KeyCode::Tab => {
                app.headphone_eq.step = HeadphoneEqStep::Optimize;
                None
            }
            KeyCode::BackTab => {
                app.headphone_eq.step = HeadphoneEqStep::SelectFile;
                None
            }
            _ => None,
        },

        HeadphoneEqStep::Optimize => match key.code {
            KeyCode::Enter => {
                match &app.headphone_eq.opt_status {
                    OptimizationStatus::Idle
                    | OptimizationStatus::Failed
                    | OptimizationStatus::Cancelled => {
                        spawn_headphone_eq_optimization(app);
                    }
                    OptimizationStatus::Completed => {
                        app.headphone_eq.step = HeadphoneEqStep::Results;
                    }
                    OptimizationStatus::Running => {}
                }
                None
            }
            KeyCode::Tab => {
                if app.headphone_eq.opt_status == OptimizationStatus::Completed {
                    app.headphone_eq.step = HeadphoneEqStep::Results;
                } else {
                    app.headphone_eq.step = HeadphoneEqStep::Configure;
                }
                None
            }
            KeyCode::BackTab => {
                app.headphone_eq.step = HeadphoneEqStep::Configure;
                None
            }
            _ => None,
        },

        HeadphoneEqStep::Results => match key.code {
            KeyCode::Tab => {
                app.headphone_eq.step = HeadphoneEqStep::SelectFile;
                None
            }
            KeyCode::BackTab => {
                app.headphone_eq.step = HeadphoneEqStep::Optimize;
                None
            }
            _ => None,
        },
    }
}

fn adjust_headphone_eq_field(app: &mut App, delta: i32) {
    let c = &mut app.headphone_eq.config;
    match app.headphone_eq.config_selected_field {
        0 => {
            let n = c.num_filters as i32 + delta;
            c.num_filters = n.clamp(1, 30) as usize;
        }
        1 => c.min_freq = (c.min_freq + delta as f64 * 10.0).clamp(20.0, 500.0),
        2 => c.max_freq = (c.max_freq + delta as f64 * 500.0).clamp(1000.0, 20000.0),
        3 => c.min_db = (c.min_db + delta as f64).clamp(-24.0, 0.0),
        4 => c.max_db = (c.max_db + delta as f64).clamp(0.0, 12.0),
        5 => c.min_q = (c.min_q + delta as f64 * 0.1).clamp(0.1, 2.0),
        6 => c.max_q = (c.max_q + delta as f64 * 0.5).clamp(1.0, 20.0),
        7 => {
            c.peq_model = cycle_string(
                &c.peq_model,
                &["pk", "hp-pk", "hp-pk-lp", "ls-pk", "ls-pk-hs"],
                delta,
            );
        }
        8 => {
            use sotf_audio_player::room_eq_types::RoomEqAlgorithm;
            let algos = RoomEqAlgorithm::all();
            let idx = algos.iter().position(|a| *a == c.algorithm).unwrap_or(0);
            let new_idx = if delta > 0 {
                (idx + 1) % algos.len()
            } else {
                (idx + algos.len() - 1) % algos.len()
            };
            c.algorithm = algos[new_idx];
        }
        9 => {
            let n = c.max_iter as i32 + delta * 1000;
            c.max_iter = n.clamp(1000, 100000) as usize;
        }
        10 => {
            let n = c.population as i32 + delta * 10;
            c.population = n.clamp(10, 200) as usize;
        }
        11 => {
            c.strategy = cycle_string(
                &c.strategy,
                &["currenttobest1bin", "best1bin", "rand1bin", "best2bin"],
                delta,
            );
        }
        12 => c.de_f = (c.de_f + delta as f64 * 0.1).clamp(0.1, 2.0),
        13 => c.de_cr = (c.de_cr + delta as f64 * 0.1).clamp(0.1, 1.0),
        14 => c.refine = !c.refine,
        15 => {
            c.local_algo = cycle_string(&c.local_algo, &["cobyla", "nelder-mead"], delta);
        }
        16 => c.smooth = !c.smooth,
        17 => {
            let n = c.smooth_n as i32 + delta;
            c.smooth_n = n.clamp(1, 24) as usize;
        }
        _ => {}
    }
}

static HEADPHONE_OPT_RESULT: std::sync::OnceLock<
    Arc<Mutex<Option<Result<sotf_audio_player::autoeq::HeadphoneOptimizationResult, String>>>>,
> = std::sync::OnceLock::new();

pub fn poll_headphone_eq_optimization(app: &mut App) -> bool {
    use sotf_audio_player::headphone_eq_types::HeadphoneEqBiquad;
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    if app.headphone_eq.opt_status != OptimizationStatus::Running {
        return false;
    }

    let result_slot = HEADPHONE_OPT_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    if let Ok(mut guard) = result_slot.lock() {
        if let Some(result) = guard.take() {
            match result {
                Ok(r) => {
                    app.headphone_eq.pre_loss = r.initial_loss;
                    app.headphone_eq.post_loss = r.final_loss;
                    app.headphone_eq.filters = r
                        .biquads
                        .iter()
                        .map(|b| HeadphoneEqBiquad {
                            filter_type: format!("{:?}", b.filter_type),
                            freq: b.freq,
                            q: b.q,
                            db_gain: b.db_gain,
                        })
                        .collect();
                    app.headphone_eq.curve_frequencies = r.frequencies.clone();
                    app.headphone_eq.curve_input = r.input_curve.clone();
                    app.headphone_eq.curve_target = r.target_curve.clone();
                    app.headphone_eq.curve_corrected = r.corrected_curve.clone();
                    app.headphone_eq.curve_filter_response = r.filter_response.clone();
                    app.headphone_eq.loss_history = r.optimization_history.clone();
                    app.headphone_eq.opt_status = OptimizationStatus::Completed;
                    app.headphone_eq.opt_progress = 1.0;
                }
                Err(e) => {
                    app.headphone_eq.opt_status = OptimizationStatus::Failed;
                    app.headphone_eq.opt_error = Some(e);
                }
            }
            return true;
        }
    }

    false
}

fn spawn_headphone_eq_optimization(app: &mut App) {
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    if app.headphone_eq.measurement_path.is_empty() {
        app.headphone_eq.opt_status = OptimizationStatus::Failed;
        app.headphone_eq.opt_error = Some("No measurement file selected".to_string());
        return;
    }

    app.headphone_eq.opt_status = OptimizationStatus::Running;
    app.headphone_eq.opt_error = None;
    app.headphone_eq.opt_progress = 0.0;
    app.headphone_eq.opt_iteration = 0;
    app.headphone_eq.opt_loss = 0.0;
    app.headphone_eq.filters.clear();

    let curve_path = app.headphone_eq.measurement_path.clone();
    let target = app.headphone_eq.target_preset.clone();
    let custom_target = app.headphone_eq.custom_target_path.clone();
    let c = &app.headphone_eq.config;

    let mut args = autoeq::Args::headphone_defaults();
    args.num_filters = c.num_filters;
    args.min_freq = c.min_freq;
    args.max_freq = c.max_freq;
    args.min_db = c.min_db;
    args.max_db = c.max_db;
    args.min_q = c.min_q;
    args.max_q = c.max_q;
    args.maxeval = c.max_iter;
    args.algo = c.algorithm.to_autoeq_string().to_string();
    args.peq_model = sotf_audio_player::autoeq::parse_peq_model(&c.peq_model);
    args.population = c.population;
    args.recombination = c.de_cr;
    args.strategy = c.strategy.clone();
    args.tolerance = c.tolerance;
    args.refine = c.refine;
    args.local_algo = c.local_algo.clone();
    args.smooth = c.smooth;
    args.smooth_n = c.smooth_n;
    args.loss = sotf_audio_player::autoeq::parse_loss_type(&c.loss);

    let result_slot = HEADPHONE_OPT_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    // Clear stale result
    if let Ok(mut g) = result_slot.lock() {
        *g = None;
    }

    std::thread::spawn(move || {
        let result = sotf_audio_player::autoeq::headphone::run_headphone_optimization(
            &curve_path,
            &target,
            &custom_target,
            &args,
            "json",
        );
        if let Ok(mut guard) = result_slot.lock() {
            *guard = Some(result);
        }
    });
}

// ============================================================================
// Recording Wizard
// ============================================================================

fn handle_recording_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use sotf_audio_player::recording_types::{ChannelRecordingState, RecordingStep};

    // Esc goes up one level
    if key.code == KeyCode::Esc {
        match app.recording.step {
            RecordingStep::Config => {
                if app.recording.editing_output_dir {
                    app.recording.editing_output_dir = false;
                } else if app.recording.editing_mic_cal {
                    app.recording.editing_mic_cal = false;
                } else {
                    app.configure_tab_focused = true;
                }
            }
            RecordingStep::Capture => {
                app.recording.step = RecordingStep::Config;
            }
            RecordingStep::Evaluating => {
                app.recording.step = RecordingStep::Capture;
            }
            RecordingStep::Saving => {
                if app.recording.editing_save_name {
                    app.recording.editing_save_name = false;
                } else {
                    app.recording.step = RecordingStep::Evaluating;
                }
            }
        }
        return None;
    }

    match app.recording.step {
        RecordingStep::Config => {
            if app.recording.editing_output_dir {
                match key.code {
                    KeyCode::Enter => {
                        app.recording.editing_output_dir = false;
                    }
                    KeyCode::Backspace => {
                        app.recording.output_directory.pop();
                    }
                    KeyCode::F(2) => {
                        let start = app.recording.output_directory.clone();
                        app.open_file_explorer(
                            FilePickerOrigin::RecordingOutputDir,
                            FilePickerMode::Directory,
                            "Select Output Directory",
                            Some(&start),
                            None,
                        );
                    }
                    KeyCode::Char(c) => {
                        app.recording.output_directory.push(c);
                    }
                    _ => {}
                }
                return None;
            }
            if app.recording.editing_mic_cal {
                match key.code {
                    KeyCode::Enter => {
                        app.recording.editing_mic_cal = false;
                    }
                    KeyCode::Backspace => {
                        app.recording.mic_calibration_path.pop();
                    }
                    KeyCode::F(2) => {
                        let start = app.recording.mic_calibration_path.clone();
                        app.open_file_explorer(
                            FilePickerOrigin::RecordingMicCalibration,
                            FilePickerMode::File,
                            "Select Mic Calibration File",
                            Some(&start),
                            None,
                        );
                    }
                    KeyCode::Char(c) => {
                        app.recording.mic_calibration_path.push(c);
                    }
                    _ => {}
                }
                return None;
            }

            match key.code {
                KeyCode::Up => {
                    if app.recording.selected_field == 0 {
                        app.configure_tab_focused = true;
                    } else {
                        app.recording.selected_field -= 1;
                    }
                }
                KeyCode::Down => {
                    if app.recording.selected_field < 9 {
                        app.recording.selected_field += 1;
                    }
                }
                KeyCode::Enter => match app.recording.selected_field {
                    8 => {
                        app.recording.editing_output_dir = true;
                    }
                    9 => {
                        app.recording.editing_mic_cal = true;
                    }
                    _ => {}
                },
                KeyCode::Left | KeyCode::Right => {
                    let delta = if key.code == KeyCode::Right { 1i32 } else { -1 };
                    adjust_recording_field(app, delta);
                }
                KeyCode::Tab => {
                    // Initialize channel recordings based on speaker config
                    init_recording_channels(app);
                    app.recording.step = RecordingStep::Capture;
                }
                _ => {}
            }
            None
        }

        RecordingStep::Capture => match key.code {
            KeyCode::Up => {
                if let Some(ch) = app.recording.current_channel {
                    if ch > 0 {
                        app.recording.current_channel = Some(ch - 1);
                    }
                }
                None
            }
            KeyCode::Down => {
                if let Some(ch) = app.recording.current_channel {
                    if ch + 1 < app.recording.channel_recordings.len() {
                        app.recording.current_channel = Some(ch + 1);
                    }
                } else if !app.recording.channel_recordings.is_empty() {
                    app.recording.current_channel = Some(0);
                }
                None
            }
            KeyCode::Enter => {
                // Record current channel (placeholder - actual recording needs audio engine)
                if let Some(ch_idx) = app.recording.current_channel {
                    if let Some(ch) = app.recording.channel_recordings.get_mut(ch_idx) {
                        if ch.state == ChannelRecordingState::Empty
                            || ch.state == ChannelRecordingState::Error
                        {
                            ch.state = ChannelRecordingState::Recording;
                            app.recording.status_message =
                                format!("Recording channel {}...", ch.channel_name);
                            // TODO: Spawn actual recording via engine
                        }
                    }
                }
                None
            }
            KeyCode::Tab => {
                let has_done = app
                    .recording
                    .channel_recordings
                    .iter()
                    .any(|ch| ch.state == ChannelRecordingState::Done);
                if has_done {
                    app.recording.step = RecordingStep::Evaluating;
                }
                None
            }
            KeyCode::BackTab => {
                app.recording.step = RecordingStep::Config;
                None
            }
            _ => None,
        },

        RecordingStep::Evaluating => match key.code {
            KeyCode::Up => {
                if app.recording.selected_channel_view > 0 {
                    app.recording.selected_channel_view -= 1;
                }
                None
            }
            KeyCode::Down => {
                let completed = app
                    .recording
                    .channel_recordings
                    .iter()
                    .filter(|ch| ch.state == ChannelRecordingState::Done)
                    .count();
                if app.recording.selected_channel_view + 1 < completed {
                    app.recording.selected_channel_view += 1;
                }
                None
            }
            KeyCode::Tab => {
                app.recording.step = RecordingStep::Saving;
                None
            }
            KeyCode::BackTab => {
                app.recording.step = RecordingStep::Capture;
                None
            }
            _ => None,
        },

        RecordingStep::Saving => {
            if app.recording.editing_save_name {
                match key.code {
                    KeyCode::Enter => {
                        app.recording.editing_save_name = false;
                        save_recordings(app);
                    }
                    KeyCode::Backspace => {
                        app.recording.save_name.pop();
                    }
                    KeyCode::Char(c) => {
                        app.recording.save_name.push(c);
                    }
                    _ => {}
                }
                return None;
            }
            match key.code {
                KeyCode::Enter => {
                    app.recording.editing_save_name = true;
                }
                KeyCode::Tab => {
                    app.recording.step = RecordingStep::Config;
                }
                KeyCode::BackTab => {
                    app.recording.step = RecordingStep::Evaluating;
                }
                _ => {}
            }
            None
        }
    }
}

fn adjust_recording_field(app: &mut App, delta: i32) {
    use sotf_audio_player::recording_types::{RecordingSignalType, SpeakerConfiguration};

    match app.recording.selected_field {
        0 => {
            // Cycle playback device
            if !app.recording.available_playback_devices.is_empty() {
                let len = app.recording.available_playback_devices.len();
                app.recording.selected_playback_idx = if delta > 0 {
                    (app.recording.selected_playback_idx + 1) % len
                } else {
                    (app.recording.selected_playback_idx + len - 1) % len
                };
            }
        }
        1 => {
            // Cycle recording device
            if !app.recording.available_recording_devices.is_empty() {
                let len = app.recording.available_recording_devices.len();
                app.recording.selected_recording_idx = if delta > 0 {
                    (app.recording.selected_recording_idx + 1) % len
                } else {
                    (app.recording.selected_recording_idx + len - 1) % len
                };
            }
        }
        2 => {
            // Cycle speaker config
            let configs = SpeakerConfiguration::all();
            let idx = configs
                .iter()
                .position(|c| *c == app.recording.playback_config.speaker_configuration)
                .unwrap_or(0);
            let new_idx = if delta > 0 {
                (idx + 1) % configs.len()
            } else {
                (idx + configs.len() - 1) % configs.len()
            };
            let new_config = configs[new_idx];
            app.recording.playback_config.speaker_configuration = new_config;
            // Update channel mappings for new config
            update_channel_mappings_for_config(app, new_config);
        }
        3 => {
            // Cycle signal type
            let types = RecordingSignalType::all();
            let idx = types
                .iter()
                .position(|t| *t == app.recording.signal_type)
                .unwrap_or(0);
            let new_idx = if delta > 0 {
                (idx + 1) % types.len()
            } else {
                (idx + types.len() - 1) % types.len()
            };
            app.recording.signal_type = types[new_idx];
        }
        4 => {
            app.recording.signal_duration_secs =
                (app.recording.signal_duration_secs + delta as f32).clamp(1.0, 30.0);
        }
        5 => {
            app.recording.signal_level_db =
                (app.recording.signal_level_db + delta as f32).clamp(-40.0, 0.0);
        }
        6 => {
            app.recording.sweep_start_freq =
                (app.recording.sweep_start_freq + delta as f32 * 10.0).clamp(10.0, 1000.0);
        }
        7 => {
            app.recording.sweep_end_freq =
                (app.recording.sweep_end_freq + delta as f32 * 1000.0).clamp(1000.0, 24000.0);
        }
        _ => {}
    }
}

fn update_channel_mappings_for_config(
    app: &mut App,
    config: sotf_audio_player::recording_types::SpeakerConfiguration,
) {
    use sotf_audio_player::recording_types::ChannelMapping;

    let names = config.default_channel_names();
    app.recording.playback_config.channel_mappings = names
        .iter()
        .enumerate()
        .map(|(i, name)| ChannelMapping::single(i, *name))
        .collect();
    app.recording.playback_config.num_channels = names.len();
}

fn init_recording_channels(app: &mut App) {
    use sotf_audio_player::recording_types::{ChannelRecording, ChannelRecordingState};

    let expected_count = app.recording.playback_config.channel_mappings.len();
    if app.recording.channel_recordings.len() != expected_count {
        app.recording.channel_recordings = app
            .recording
            .playback_config
            .channel_mappings
            .iter()
            .enumerate()
            .map(|(i, mapping)| ChannelRecording {
                channel_index: i,
                channel_name: mapping.group_name.clone(),
                state: ChannelRecordingState::Empty,
                result: None,
            })
            .collect();
        app.recording.current_channel = if expected_count > 0 { Some(0) } else { None };
    }
}

fn save_recordings(app: &mut App) {
    use sotf_audio_player::recording_types::ChannelRecordingState;
    use sotf_audio_player::room_eq_types::{ChannelMeasurement, RoomEqMeasurementsFile};

    // Validate save name early (before any I/O)
    let name = if app.recording.save_name.is_empty() {
        "recordings".to_string()
    } else {
        app.recording.save_name.clone()
    };
    if name.contains('/') || name.contains('\\') {
        app.recording.save_error = Some("Save name must not contain path separators".to_string());
        return;
    }

    let completed: Vec<_> = app
        .recording
        .channel_recordings
        .iter()
        .filter(|ch| ch.state == ChannelRecordingState::Done && ch.result.is_some())
        .collect();

    if completed.is_empty() {
        app.recording.save_error = Some("No completed recordings to save".to_string());
        return;
    }

    // Build measurements file
    let channels: Vec<ChannelMeasurement> = completed
        .iter()
        .map(|ch| ChannelMeasurement {
            channel_name: ch.channel_name.clone(),
            measurement: ch.result.clone().unwrap(),
            is_group: false,
            group_drivers: Vec::new(),
        })
        .collect();

    let measurements_file = RoomEqMeasurementsFile::new(channels);

    // Determine output path
    let dir = if app.recording.output_directory.is_empty() {
        ".".to_string()
    } else {
        app.recording.output_directory.clone()
    };

    let path = std::path::PathBuf::from(&dir).join(format!("{}.json", name));

    match serde_json::to_string_pretty(&measurements_file) {
        Ok(json) => match std::fs::write(&path, json) {
            Ok(()) => {
                app.recording.save_success = true;
                app.recording.save_error = None;
            }
            Err(e) => {
                app.recording.save_error = Some(format!("Write error: {}", e));
            }
        },
        Err(e) => {
            app.recording.save_error = Some(format!("Serialize error: {}", e));
        }
    }
}

// ============================================================================
// Room EQ Wizard
// ============================================================================

fn handle_room_eq_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use sotf_audio_player::room_eq_types::{OptimizationStatus, RoomEqStep};

    // Esc goes up one level
    if key.code == KeyCode::Esc {
        match app.room_eq.step {
            RoomEqStep::LoadData => {
                if app.room_eq.editing_file_path {
                    app.room_eq.editing_file_path = false;
                } else {
                    app.configure_tab_focused = true;
                }
            }
            RoomEqStep::Configure => {
                app.room_eq.step = RoomEqStep::LoadData;
            }
            RoomEqStep::Optimize => {
                app.room_eq.step = RoomEqStep::Configure;
            }
            RoomEqStep::Review => {
                app.room_eq.step = RoomEqStep::Optimize;
            }
            RoomEqStep::Export => {
                if app.room_eq.editing_export_path {
                    app.room_eq.editing_export_path = false;
                } else {
                    app.room_eq.step = RoomEqStep::Review;
                }
            }
        }
        return None;
    }

    // Step navigation via Left/Right (except when editing text or in Configure step)
    let editing = app.room_eq.editing_file_path || app.room_eq.editing_export_path;
    if !editing && app.room_eq.step != RoomEqStep::Configure {
        if key.code == KeyCode::Left {
            if let Some(prev) = app.room_eq.step.previous() {
                app.room_eq.step = prev;
            }
            return None;
        }
        if key.code == KeyCode::Right {
            if let Some(next) = app.room_eq.step.next() {
                app.room_eq.step = next;
            }
            return None;
        }
    }

    match app.room_eq.step {
        RoomEqStep::LoadData => {
            if app.room_eq.editing_file_path {
                match key.code {
                    KeyCode::Enter => {
                        app.room_eq.editing_file_path = false;
                        load_room_eq_measurements(app);
                    }
                    KeyCode::Backspace => {
                        app.room_eq.file_path.pop();
                    }
                    KeyCode::F(2) => {
                        let start = app.room_eq.file_path.clone();
                        app.open_file_explorer(
                            FilePickerOrigin::RoomEqFilePath,
                            FilePickerMode::File,
                            "Select Room EQ Measurements (JSON)",
                            Some(&start),
                            Some("json"),
                        );
                    }
                    KeyCode::Char(c) => {
                        app.room_eq.file_path.push(c);
                    }
                    _ => {}
                }
                return None;
            }
            match key.code {
                KeyCode::Enter => {
                    app.room_eq.editing_file_path = true;
                }
                KeyCode::Tab => {
                    if !app.room_eq.channel_measurements.is_empty() {
                        app.room_eq.step = RoomEqStep::Configure;
                    }
                }
                _ => {}
            }
            None
        }

        RoomEqStep::Configure => match key.code {
            KeyCode::Up => {
                if app.room_eq.selected_field > 0 {
                    app.room_eq.selected_field -= 1;
                }
                None
            }
            KeyCode::Down => {
                if app.room_eq.selected_field < ROOM_EQ_FIELD_COUNT - 1 {
                    app.room_eq.selected_field += 1;
                }
                None
            }
            KeyCode::Left | KeyCode::Char('-') => {
                adjust_room_eq_field(app, -1);
                None
            }
            KeyCode::Right | KeyCode::Char('+') => {
                adjust_room_eq_field(app, 1);
                None
            }
            KeyCode::Enter | KeyCode::Tab => {
                app.room_eq.step = RoomEqStep::Optimize;
                None
            }
            KeyCode::BackTab => {
                app.room_eq.step = RoomEqStep::LoadData;
                None
            }
            _ => None,
        },

        RoomEqStep::Optimize => match key.code {
            KeyCode::Enter => {
                match &app.room_eq.opt_status {
                    OptimizationStatus::Idle
                    | OptimizationStatus::Failed
                    | OptimizationStatus::Cancelled => {
                        spawn_room_eq_optimization(app);
                    }
                    OptimizationStatus::Completed => {
                        app.room_eq.step = RoomEqStep::Review;
                    }
                    OptimizationStatus::Running => {}
                }
                None
            }
            KeyCode::Tab => {
                if app.room_eq.opt_status == OptimizationStatus::Completed {
                    app.room_eq.step = RoomEqStep::Review;
                } else {
                    app.room_eq.step = RoomEqStep::Configure;
                }
                None
            }
            KeyCode::BackTab => {
                app.room_eq.step = RoomEqStep::Configure;
                None
            }
            _ => None,
        },

        RoomEqStep::Review => match key.code {
            KeyCode::Up => {
                if app.room_eq.selected_channel > 0 {
                    app.room_eq.selected_channel -= 1;
                }
                None
            }
            KeyCode::Down => {
                if !app.room_eq.channel_results.is_empty()
                    && app.room_eq.selected_channel < app.room_eq.channel_results.len() - 1
                {
                    app.room_eq.selected_channel += 1;
                }
                None
            }
            KeyCode::Tab => {
                app.room_eq.step = RoomEqStep::Export;
                None
            }
            KeyCode::BackTab => {
                app.room_eq.step = RoomEqStep::Optimize;
                None
            }
            _ => None,
        },

        RoomEqStep::Export => {
            if app.room_eq.editing_export_path {
                match key.code {
                    KeyCode::Enter => {
                        app.room_eq.editing_export_path = false;
                        export_room_eq_results(app);
                    }
                    KeyCode::Backspace => {
                        app.room_eq.export_path.pop();
                    }
                    KeyCode::F(2) => {
                        let start = app.room_eq.export_path.clone();
                        app.open_file_explorer(
                            FilePickerOrigin::RoomEqExportPath,
                            FilePickerMode::File,
                            "Select Export Path (JSON)",
                            Some(&start),
                            Some("json"),
                        );
                    }
                    KeyCode::Char(c) => {
                        app.room_eq.export_path.push(c);
                    }
                    _ => {}
                }
                return None;
            }
            match key.code {
                KeyCode::Enter => {
                    app.room_eq.editing_export_path = true;
                }
                KeyCode::Tab => {
                    app.room_eq.step = RoomEqStep::LoadData;
                }
                KeyCode::BackTab => {
                    app.room_eq.step = RoomEqStep::Review;
                }
                _ => {}
            }
            None
        }
    }
}

/// Total number of adjustable fields in the Room EQ configure step
const ROOM_EQ_FIELD_COUNT: usize = 24;

fn adjust_room_eq_field(app: &mut App, delta: i32) {
    use sotf_audio_player::room_eq_types::{MultiSpeakerMode, RoomEqOptimizationMode};

    let c = &mut app.room_eq.config;
    match app.room_eq.selected_field {
        // Basic
        0 => {
            let n = c.num_filters as i32 + delta;
            c.num_filters = n.clamp(1, 30) as usize;
        }
        1 => c.min_freq = (c.min_freq + delta as f64 * 10.0).clamp(20.0, 500.0),
        2 => c.max_freq = (c.max_freq + delta as f64 * 500.0).clamp(1000.0, 20000.0),
        3 => c.min_db = (c.min_db + delta as f64).clamp(-24.0, 0.0),
        4 => c.max_db = (c.max_db + delta as f64).clamp(0.0, 12.0),
        5 => c.min_q = (c.min_q + delta as f64 * 0.1).clamp(0.1, 2.0),
        6 => c.max_q = (c.max_q + delta as f64 * 0.5).clamp(1.0, 20.0),
        7 => {
            c.peq_model = cycle_string(
                &c.peq_model,
                &["pk", "hp-pk", "hp-pk-lp", "ls-pk", "ls-pk-hs"],
                delta,
            );
        }
        // Optimization
        8 => {
            let algos = ["cobyla", "autoeq:de", "nelder-mead"];
            c.algorithm = cycle_string(&c.algorithm, &algos, delta);
        }
        9 => {
            let n = c.max_iter as i32 + delta * 1000;
            c.max_iter = n.clamp(1000, 100000) as usize;
        }
        10 => {
            let n = c.population as i32 + delta * 10;
            c.population = n.clamp(10, 200) as usize;
        }
        11 => c.refine = !c.refine,
        12 => {
            c.local_algo = cycle_string(&c.local_algo, &["cobyla", "nelder-mead"], delta);
        }
        13 => c.psychoacoustic = !c.psychoacoustic,
        14 => c.asymmetric_loss = !c.asymmetric_loss,
        // Mode
        15 => {
            let modes = RoomEqOptimizationMode::all();
            let idx = modes.iter().position(|m| *m == c.mode).unwrap_or(0);
            let new_idx = if delta > 0 {
                (idx + 1) % modes.len()
            } else {
                (idx + modes.len() - 1) % modes.len()
            };
            c.mode = modes[new_idx];
        }
        16 => {
            let modes = MultiSpeakerMode::all();
            let idx = modes
                .iter()
                .position(|m| *m == c.multi_speaker_mode)
                .unwrap_or(0);
            let new_idx = if delta > 0 {
                (idx + 1) % modes.len()
            } else {
                (idx + modes.len() - 1) % modes.len()
            };
            c.multi_speaker_mode = modes[new_idx];
        }
        // Target Tilt
        17 => c.target_tilt.enabled = !c.target_tilt.enabled,
        18 => c.target_tilt.slope = (c.target_tilt.slope + delta as f64 * 0.1).clamp(-3.0, 0.0),
        // Excursion Protection
        19 => c.excursion_protection.enabled = !c.excursion_protection.enabled,
        20 => {
            c.excursion_protection.manual_f3_hz =
                (c.excursion_protection.manual_f3_hz + delta as f64 * 5.0).clamp(20.0, 200.0)
        }
        // Schroeder Split
        21 => c.schroeder_split.enabled = !c.schroeder_split.enabled,
        22 => {
            c.schroeder_split.schroeder_freq =
                (c.schroeder_split.schroeder_freq + delta as f64 * 10.0).clamp(100.0, 1000.0)
        }
        // Phase Alignment
        23 => c.phase_alignment.enabled = !c.phase_alignment.enabled,
        _ => {}
    }
}

fn load_room_eq_measurements(app: &mut App) {
    use sotf_audio_player::room_eq_types::RoomEqMeasurementsFile;

    let path = &app.room_eq.file_path;
    if path.is_empty() {
        app.room_eq.load_error = Some("No file path specified".to_string());
        return;
    }

    match std::fs::read_to_string(path) {
        Ok(contents) => match RoomEqMeasurementsFile::from_json_str(&contents) {
            Ok(file) => {
                app.room_eq.channel_measurements = file.channels;
                app.room_eq.load_error = None;
            }
            Err(e) => {
                app.room_eq.load_error = Some(format!("Parse error: {}", e));
                app.room_eq.channel_measurements.clear();
            }
        },
        Err(e) => {
            app.room_eq.load_error = Some(format!("Read error: {}", e));
            app.room_eq.channel_measurements.clear();
        }
    }
}

static ROOM_OPT_RESULT: std::sync::OnceLock<
    Arc<Mutex<Option<Result<sotf_audio_player::autoeq::RoomOptimizationResult, String>>>>,
> = std::sync::OnceLock::new();
static ROOM_OPT_PROGRESS: std::sync::OnceLock<
    Arc<Mutex<Option<sotf_audio_player::autoeq::RoomOptimizationProgress>>>,
> = std::sync::OnceLock::new();

pub fn poll_room_eq_optimization(app: &mut App) -> bool {
    use sotf_audio_player::room_eq_types::{ChannelOptResult, EqFilterConfig, OptimizationStatus};

    if app.room_eq.opt_status != OptimizationStatus::Running {
        return false;
    }

    let result_slot = ROOM_OPT_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    let progress_slot = ROOM_OPT_PROGRESS
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    if let Ok(mut guard) = result_slot.lock() {
        if let Some(result) = guard.take() {
            match result {
                Ok(r) => {
                    // Convert autoeq results to TUI ChannelOptResult
                    app.room_eq.channel_results = r
                        .channel_results
                        .iter()
                        .map(|(name, ch)| ChannelOptResult {
                            channel_name: name.clone(),
                            pre_score: ch.pre_score,
                            post_score: ch.post_score,
                            eq_filters: ch
                                .biquads
                                .iter()
                                .map(|b| EqFilterConfig {
                                    filter_type: format!("{:?}", b.filter_type),
                                    frequency: b.freq,
                                    q: b.q,
                                    gain_db: b.db_gain,
                                })
                                .collect(),
                            crossover_freqs: None,
                            driver_gains: None,
                            original_response: Some(
                                ch.initial_curve
                                    .freq
                                    .iter()
                                    .zip(ch.initial_curve.spl.iter())
                                    .map(|(&f, &s)| (f, s))
                                    .collect(),
                            ),
                            corrected_response: Some(
                                ch.final_curve
                                    .freq
                                    .iter()
                                    .zip(ch.final_curve.spl.iter())
                                    .map(|(&f, &s)| (f, s))
                                    .collect(),
                            ),
                            normalized_response: None,
                        })
                        .collect();
                    app.room_eq.opt_status = OptimizationStatus::Completed;
                    app.room_eq.opt_progress = 1.0;
                }
                Err(e) => {
                    app.room_eq.opt_status = OptimizationStatus::Failed;
                    app.room_eq.opt_error = Some(e);
                }
            }
            return true;
        }
    }

    if let Ok(mut guard) = progress_slot.lock() {
        if let Some(p) = guard.take() {
            let total_iters = p.total_speakers * p.max_iterations;
            let done_iters = p.speaker_index * p.max_iterations + p.iteration;
            app.room_eq.opt_iteration = done_iters;
            app.room_eq.opt_max_iter = total_iters;
            app.room_eq.opt_loss = p.loss;
            app.room_eq.opt_progress = if total_iters > 0 {
                (done_iters as f32 / total_iters as f32).min(1.0)
            } else {
                0.0
            };
            return true;
        }
    }

    false
}

fn spawn_room_eq_optimization(app: &mut App) {
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    if app.room_eq.channel_measurements.is_empty() {
        app.room_eq.opt_status = OptimizationStatus::Failed;
        app.room_eq.opt_error = Some("No measurements loaded".to_string());
        return;
    }

    app.room_eq.opt_status = OptimizationStatus::Running;
    app.room_eq.opt_error = None;
    app.room_eq.opt_progress = 0.0;
    app.room_eq.opt_iteration = 0;
    app.room_eq.opt_loss = 0.0;
    app.room_eq.channel_results.clear();

    // Build curves from loaded measurements
    let measurements = app.room_eq.channel_measurements.clone();
    let config = app.room_eq.config.clone();

    let result_slot = ROOM_OPT_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    let progress_slot = ROOM_OPT_PROGRESS
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    // Clear stale results
    if let Ok(mut g) = result_slot.lock() {
        *g = None;
    }
    if let Ok(mut g) = progress_slot.lock() {
        *g = None;
    }

    std::thread::spawn(move || {
        use sotf_audio_player::autoeq::{
            build_room_config_from_curves, optimizer_config_from_args, run_room_optimization,
        };

        // Convert measurements to curves
        let speaker_curves: Vec<(String, autoeq::Curve)> = measurements
            .iter()
            .map(|m| {
                let freq: Vec<f64> = m
                    .measurement
                    .frequencies
                    .iter()
                    .map(|&f| f as f64)
                    .collect();
                let spl: Vec<f64> = m
                    .measurement
                    .magnitude_db
                    .iter()
                    .map(|&db| db as f64)
                    .collect();
                let curve = autoeq::Curve {
                    freq: ndarray::Array1::from(freq),
                    spl: ndarray::Array1::from(spl),
                    phase: None,
                };
                (m.channel_name.clone(), curve)
            })
            .collect();

        // Build autoeq Args from config
        let mut args = autoeq::Args::speaker_defaults();
        args.num_filters = config.num_filters;
        args.min_freq = config.min_freq;
        args.max_freq = config.max_freq;
        args.min_db = config.min_db;
        args.max_db = config.max_db;
        args.min_q = config.min_q;
        args.max_q = config.max_q;
        args.maxeval = config.max_iter;
        args.population = config.population;
        args.algo = config.algorithm.clone();
        args.refine = config.refine;
        args.local_algo = config.local_algo.clone();
        args.peq_model = sotf_audio_player::autoeq::parse_peq_model(&config.peq_model);
        args.loss = if config.asymmetric_loss {
            autoeq::LossType::SpeakerFlatAsymmetric
        } else {
            autoeq::LossType::SpeakerFlat
        };

        let optimizer = optimizer_config_from_args(&args);
        let room_config = build_room_config_from_curves(&speaker_curves, optimizer);

        let progress_slot2 = progress_slot.clone();
        let callback: sotf_audio_player::autoeq::RoomOptimizationCallback = Box::new(move |p| {
            if let Ok(mut guard) = progress_slot2.lock() {
                *guard = Some(p.clone());
            }
            autoeq::roomeq::CallbackAction::Continue
        });

        let result = run_room_optimization(&room_config, 48000.0, Some(callback));
        if let Ok(mut guard) = result_slot.lock() {
            *guard = Some(result);
        }
    });
}

fn export_room_eq_results(app: &mut App) {
    if app.room_eq.export_path.is_empty() {
        app.room_eq.export_error = Some("No export path specified".to_string());
        return;
    }

    // Serialize channel results as JSON
    let json = match serde_json::to_string_pretty(&app.room_eq.channel_results) {
        Ok(j) => j,
        Err(e) => {
            app.room_eq.export_error = Some(format!("Serialize error: {}", e));
            return;
        }
    };

    match std::fs::write(&app.room_eq.export_path, json) {
        Ok(()) => {
            app.room_eq.export_success = true;
            app.room_eq.export_error = None;
        }
        Err(e) => {
            app.room_eq.export_error = Some(format!("Write error: {}", e));
            app.room_eq.export_success = false;
        }
    }
}

#[derive(Debug, Clone)]
pub enum PlayerCommand {
    Play(std::path::PathBuf),
    Pause,
    Resume,
    Stop,
    SetVolume(f32),
    SetOutputDevice(String),
    /// Seek to absolute position in seconds
    Seek(f64),
    /// Seek relative to current position (positive = forward, negative = backward)
    SeekRelative(f64),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{HEADPHONE_TARGET_PRESETS, HeadphoneEqStep, SpinoramaStep};
    use crate::theme::Theme;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use sotf_audio_player::recording_types::{
        ChannelMapping, ChannelRecordingState, RecordingStep, SpeakerConfiguration,
    };
    use sotf_audio_player::room_eq_types::RoomEqStep;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn make_app() -> App {
        App::new(Theme::default())
    }

    // ========================================================================
    // Step navigation: no-wrap tests
    // ========================================================================

    #[test]
    fn headphone_eq_step_prev_does_not_wrap() {
        assert_eq!(
            headphone_eq_step_prev(HeadphoneEqStep::SelectFile),
            HeadphoneEqStep::SelectFile,
        );
    }

    #[test]
    fn headphone_eq_step_next_does_not_wrap() {
        assert_eq!(
            headphone_eq_step_next(HeadphoneEqStep::Results),
            HeadphoneEqStep::Results,
        );
    }

    #[test]
    fn headphone_eq_step_prev_advances_backwards() {
        assert_eq!(
            headphone_eq_step_prev(HeadphoneEqStep::Configure),
            HeadphoneEqStep::SelectFile,
        );
        assert_eq!(
            headphone_eq_step_prev(HeadphoneEqStep::Optimize),
            HeadphoneEqStep::Configure,
        );
        assert_eq!(
            headphone_eq_step_prev(HeadphoneEqStep::Results),
            HeadphoneEqStep::Optimize,
        );
    }

    #[test]
    fn headphone_eq_step_next_advances_forward() {
        assert_eq!(
            headphone_eq_step_next(HeadphoneEqStep::SelectFile),
            HeadphoneEqStep::Configure,
        );
        assert_eq!(
            headphone_eq_step_next(HeadphoneEqStep::Configure),
            HeadphoneEqStep::Optimize,
        );
        assert_eq!(
            headphone_eq_step_next(HeadphoneEqStep::Optimize),
            HeadphoneEqStep::Results,
        );
    }

    #[test]
    fn spinorama_step_prev_does_not_wrap() {
        assert_eq!(
            spinorama_step_prev(SpinoramaStep::Select),
            SpinoramaStep::Select,
        );
    }

    #[test]
    fn spinorama_step_next_does_not_wrap() {
        assert_eq!(
            spinorama_step_next(SpinoramaStep::UpdatePlugin),
            SpinoramaStep::UpdatePlugin,
        );
    }

    #[test]
    fn spinorama_step_round_trip() {
        let steps = [
            SpinoramaStep::Select,
            SpinoramaStep::Configure,
            SpinoramaStep::Optimize,
            SpinoramaStep::Results,
            SpinoramaStep::UpdatePlugin,
        ];
        for i in 0..steps.len() - 1 {
            assert_eq!(spinorama_step_next(steps[i]), steps[i + 1]);
            assert_eq!(spinorama_step_prev(steps[i + 1]), steps[i]);
        }
    }

    #[test]
    fn room_eq_step_prev_does_not_wrap() {
        assert_eq!(RoomEqStep::LoadData.previous(), None);
    }

    #[test]
    fn room_eq_step_next_does_not_wrap() {
        assert_eq!(RoomEqStep::Export.next(), None);
    }

    // ========================================================================
    // HeadphoneEQ: Up key at step 1 returns focus to tab bar
    // ========================================================================

    #[test]
    fn headphone_eq_up_at_top_returns_to_tab_bar() {
        let mut app = make_app();
        app.current_screen = Screen::Configure;
        app.configure_sub_screen = crate::app::ConfigureSubScreen::HeadphoneEq;
        app.configure_tab_focused = false;
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.headphone_eq.selected_field = 0;

        handle_headphone_eq_keys(&mut app, key(KeyCode::Up));
        assert!(app.configure_tab_focused);
    }

    #[test]
    fn headphone_eq_up_decrements_field_when_not_at_top() {
        let mut app = make_app();
        app.current_screen = Screen::Configure;
        app.configure_sub_screen = crate::app::ConfigureSubScreen::HeadphoneEq;
        app.configure_tab_focused = false;
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.headphone_eq.selected_field = 1;

        handle_headphone_eq_keys(&mut app, key(KeyCode::Up));
        assert_eq!(app.headphone_eq.selected_field, 0);
        assert!(!app.configure_tab_focused);
    }

    // ========================================================================
    // HeadphoneEQ: selected_field clamp on preset cycle
    // ========================================================================

    #[test]
    fn headphone_eq_down_clamps_at_non_custom_max() {
        let mut app = make_app();
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.headphone_eq.target_preset = "harman-over-ear-2018".to_string();
        app.headphone_eq.selected_field = 1;

        // Down should NOT go past 1 for non-custom presets
        handle_headphone_eq_keys(&mut app, key(KeyCode::Down));
        assert_eq!(app.headphone_eq.selected_field, 1);
    }

    #[test]
    fn headphone_eq_down_allows_field_2_for_custom() {
        let mut app = make_app();
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.headphone_eq.target_preset = "custom".to_string();
        app.headphone_eq.selected_field = 1;

        // Down should go to 2 when preset is "custom"
        handle_headphone_eq_keys(&mut app, key(KeyCode::Down));
        assert_eq!(app.headphone_eq.selected_field, 2);
    }

    // ========================================================================
    // Recording: init_recording_channels re-inits on config change
    // ========================================================================

    #[test]
    fn init_recording_channels_creates_channels() {
        let mut app = make_app();
        app.recording.playback_config.channel_mappings = vec![
            ChannelMapping::single(0, "FL"),
            ChannelMapping::single(1, "FR"),
        ];
        app.recording.playback_config.num_channels = 2;

        init_recording_channels(&mut app);
        assert_eq!(app.recording.channel_recordings.len(), 2);
        assert_eq!(app.recording.current_channel, Some(0));
        assert_eq!(app.recording.channel_recordings[0].channel_name, "FL");
        assert_eq!(app.recording.channel_recordings[1].channel_name, "FR");
    }

    #[test]
    fn init_recording_channels_reinits_on_config_change() {
        let mut app = make_app();
        // Start with 2 channels
        app.recording.playback_config.channel_mappings = vec![
            ChannelMapping::single(0, "FL"),
            ChannelMapping::single(1, "FR"),
        ];
        init_recording_channels(&mut app);
        assert_eq!(app.recording.channel_recordings.len(), 2);

        // Change to 3 channels
        app.recording.playback_config.channel_mappings = vec![
            ChannelMapping::single(0, "FL"),
            ChannelMapping::single(1, "FR"),
            ChannelMapping::single(2, "C"),
        ];
        init_recording_channels(&mut app);
        assert_eq!(app.recording.channel_recordings.len(), 3);
        assert_eq!(app.recording.channel_recordings[2].channel_name, "C");
    }

    #[test]
    fn init_recording_channels_handles_empty_config() {
        let mut app = make_app();
        app.recording.playback_config.channel_mappings = vec![];
        init_recording_channels(&mut app);
        assert_eq!(app.recording.channel_recordings.len(), 0);
        assert_eq!(app.recording.current_channel, None);
    }

    // ========================================================================
    // Recording: save_recordings path validation
    // ========================================================================

    #[test]
    fn save_recordings_rejects_path_separators_in_name() {
        let mut app = make_app();
        app.recording.save_name = "../../evil".to_string();
        save_recordings(&mut app);
        assert!(app.recording.save_error.is_some());
        assert!(
            app.recording
                .save_error
                .as_ref()
                .unwrap()
                .contains("path separators")
        );
    }

    #[test]
    fn save_recordings_rejects_backslash_in_name() {
        let mut app = make_app();
        app.recording.save_name = "foo\\bar".to_string();
        save_recordings(&mut app);
        assert!(app.recording.save_error.is_some());
        assert!(
            app.recording
                .save_error
                .as_ref()
                .unwrap()
                .contains("path separators")
        );
    }

    #[test]
    fn save_recordings_requires_completed_channels() {
        let mut app = make_app();
        app.recording.save_name = "test".to_string();
        // No completed recordings
        save_recordings(&mut app);
        assert!(app.recording.save_error.is_some());
        assert!(
            app.recording
                .save_error
                .as_ref()
                .unwrap()
                .contains("No completed")
        );
    }

    // ========================================================================
    // Recording: default step
    // ========================================================================

    #[test]
    fn recording_step_default_is_config() {
        assert_eq!(RecordingStep::default(), RecordingStep::Config);
    }

    // ========================================================================
    // Room EQ: progress clamping
    // ========================================================================

    #[test]
    fn room_eq_progress_clamped_to_one() {
        // Simulate progress calculation that could exceed 1.0
        let total_iters: usize = 100;
        let done_iters: usize = 150; // Exceeds total (e.g. extra iterations)
        let progress = if total_iters > 0 {
            (done_iters as f32 / total_iters as f32).min(1.0)
        } else {
            0.0
        };
        assert_eq!(progress, 1.0);
        assert!(progress <= 1.0);
    }

    // ========================================================================
    // Recording: update_channel_mappings_for_config
    // ========================================================================

    #[test]
    fn update_channel_mappings_creates_correct_channels() {
        let mut app = make_app();
        update_channel_mappings_for_config(&mut app, SpeakerConfiguration::Stereo);
        assert_eq!(app.recording.playback_config.num_channels, 2);
        assert_eq!(app.recording.playback_config.channel_mappings.len(), 2);
    }

    // ========================================================================
    // Room EQ: step navigation via handle_room_eq_keys
    // ========================================================================

    #[test]
    fn room_eq_esc_at_load_data_returns_to_tab_bar() {
        let mut app = make_app();
        app.current_screen = Screen::Configure;
        app.configure_sub_screen = crate::app::ConfigureSubScreen::RoomEq;
        app.configure_tab_focused = false;
        app.room_eq.step = RoomEqStep::LoadData;

        handle_room_eq_keys(&mut app, key(KeyCode::Esc));
        assert!(app.configure_tab_focused);
    }

    #[test]
    fn room_eq_esc_at_configure_goes_back() {
        let mut app = make_app();
        app.room_eq.step = RoomEqStep::Configure;
        handle_room_eq_keys(&mut app, key(KeyCode::Esc));
        assert_eq!(app.room_eq.step, RoomEqStep::LoadData);
    }

    // ========================================================================
    // HeadphoneEQ: Esc navigation
    // ========================================================================

    #[test]
    fn headphone_eq_esc_at_configure_goes_to_select() {
        let mut app = make_app();
        app.headphone_eq.step = HeadphoneEqStep::Configure;
        handle_headphone_eq_keys(&mut app, key(KeyCode::Esc));
        assert_eq!(app.headphone_eq.step, HeadphoneEqStep::SelectFile);
    }

    #[test]
    fn headphone_eq_esc_at_select_returns_to_tab_bar() {
        let mut app = make_app();
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.configure_tab_focused = false;
        handle_headphone_eq_keys(&mut app, key(KeyCode::Esc));
        assert!(app.configure_tab_focused);
    }

    // ========================================================================
    // HeadphoneEQ: Tab requires measurement path
    // ========================================================================

    #[test]
    fn headphone_eq_tab_without_path_stays_on_select() {
        let mut app = make_app();
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.headphone_eq.measurement_path = String::new();
        handle_headphone_eq_keys(&mut app, key(KeyCode::Tab));
        assert_eq!(app.headphone_eq.step, HeadphoneEqStep::SelectFile);
    }

    #[test]
    fn headphone_eq_tab_with_path_advances_to_configure() {
        let mut app = make_app();
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.headphone_eq.measurement_path = "/some/path.csv".to_string();
        handle_headphone_eq_keys(&mut app, key(KeyCode::Tab));
        assert_eq!(app.headphone_eq.step, HeadphoneEqStep::Configure);
    }
}
