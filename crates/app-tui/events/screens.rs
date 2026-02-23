//! Screen-specific key handlers for main screens

use super::PlayerCommand;
use crate::app::{App, InputMode, Screen};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Handle library screen key events
pub fn handle_library_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
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

/// Handle directory keys in configure screen
pub fn handle_directory_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
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
        KeyCode::Char('R') => {
            // Start ReplayGain scan for all tracks
            let _ = app.start_replay_gain_scan();
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

/// Handle queue screen key events
pub fn handle_queue_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
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

/// Handle plugins screen key events
pub fn handle_plugins_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
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

/// Handle devices screen key events
pub fn handle_devices_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
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

/// Handle normal mode key events (global navigation)
pub fn handle_normal_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
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
        KeyCode::Char('D') => {
            app.current_screen = Screen::Configure;
            app.configure_sub_screen = crate::app::ConfigureSubScreen::Directories;
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
        KeyCode::Char('C') if key.modifiers.contains(KeyModifiers::SHIFT) => {
            // Shift-C to clear all mutes and solos (only when meters pane focused)
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
            Screen::Library => handle_library_keys(app, key),
            Screen::Queue => handle_queue_keys(app, key),
            Screen::Plugins => handle_plugins_keys(app, key),
            Screen::Devices => handle_devices_keys(app, key),
            Screen::Configure => super::configure::handle_configure_keys(app, key),
        },
    }
}
