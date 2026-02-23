use crate::app::{App, InputMode, MatrixEditMode, Screen};
use crate::media_controls::TuiMediaControls;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use souvlaki::MediaControlEvent;
use sotf_audio_player::{PluginSettings, PluginType};
use std::time::Duration;

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
        MediaControlEvent::SetPosition(pos) => {
            Some(PlayerCommand::Seek(pos.0.as_secs_f64()))
        }
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
        InputMode::BrowseSofaFile => handle_file_browser_mode(app, key, true),
        InputMode::BrowseIrFile => handle_file_browser_mode(app, key, false),
        InputMode::ShowHelp => handle_help_mode(app, key),
        InputMode::ShowError => handle_error_mode(app, key),
        InputMode::ChannelConflict => handle_channel_conflict_mode(app, key),
        InputMode::Normal => handle_normal_mode(app, key),
    }
}

fn handle_file_browser_mode(app: &mut App, key: KeyEvent, is_sofa: bool) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::EditPlugin;
            None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.select_previous_file();
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.select_next_file();
            None
        }
        KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
            if let Some(path) = app.navigate_file_browser() {
                let path_str = path.to_string_lossy().to_string();
                if is_sofa {
                    app.sofa_file_input = path_str;
                    if let Err(e) = app.load_sofa_file() {
                        app.status_message = Some(format!("Error: {}", e));
                    } else {
                        app.status_message = Some("SOFA file loaded".to_string());
                        app.request_plugin_update();
                    }
                } else {
                    // Load IR for Convolution
                    if let Some(plugin) = app.plugin_chain.get_plugin_mut(app.selected_plugin_index)
                    {
                        if let PluginSettings::Convolution {
                            ref mut ir_file, ..
                        } = plugin.settings
                        {
                            *ir_file = path_str;
                            app.status_message = Some("IR file set".to_string());
                            app.request_plugin_update();
                        }
                    }
                }
                app.input_mode = InputMode::EditPlugin;
            }
            None
        }
        KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => {
            if let Some(parent) = app.current_browser_dir.parent() {
                app.current_browser_dir = parent.to_path_buf();
                app.refresh_file_browser();
            }
            None
        }
        _ => None,
    }
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
                    app.input_mode = InputMode::BrowseSofaFile;
                    app.file_browser_extension = Some("sofa".to_string());
                    app.refresh_file_browser();
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
                if matches!(plugin.settings, PluginSettings::Convolution { .. }) {
                    app.input_mode = InputMode::BrowseIrFile;
                    app.file_browser_extension = Some("wav".to_string()); // Common IR extension
                    app.refresh_file_browser();
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
    if key.code == KeyCode::Up && app.configure_sub_screen != ConfigureSubScreen::SpinoramaEq {
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
            return handle_spinorama_keys(app, key);
        }
        _ => {
            // Other sub-screens: number keys switch tabs
            match key.code {
                KeyCode::Char('1') => { app.configure_sub_screen = ConfigureSubScreen::Directories; }
                KeyCode::Char('2') => { app.configure_sub_screen = ConfigureSubScreen::Recording; }
                KeyCode::Char('3') => { app.configure_sub_screen = ConfigureSubScreen::RoomEq; }
                KeyCode::Char('4') => { app.configure_sub_screen = ConfigureSubScreen::HeadphoneEq; }
                KeyCode::Char('5') => { app.configure_sub_screen = ConfigureSubScreen::SpinoramaEq; }
                _ => {}
            }
            None
        }
    }
}

fn spinorama_step_prev(s: crate::app::SpinoramaStep) -> crate::app::SpinoramaStep {
    use crate::app::SpinoramaStep;
    match s {
        SpinoramaStep::Select       => SpinoramaStep::UpdatePlugin,
        SpinoramaStep::Configure    => SpinoramaStep::Select,
        SpinoramaStep::Optimize     => SpinoramaStep::Configure,
        SpinoramaStep::Results      => SpinoramaStep::Optimize,
        SpinoramaStep::UpdatePlugin => SpinoramaStep::Results,
    }
}

fn spinorama_step_next(s: crate::app::SpinoramaStep) -> crate::app::SpinoramaStep {
    use crate::app::SpinoramaStep;
    match s {
        SpinoramaStep::Select       => SpinoramaStep::Configure,
        SpinoramaStep::Configure    => SpinoramaStep::Optimize,
        SpinoramaStep::Optimize     => SpinoramaStep::Results,
        SpinoramaStep::Results      => SpinoramaStep::UpdatePlugin,
        SpinoramaStep::UpdatePlugin => SpinoramaStep::Select,
    }
}

fn handle_spinorama_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use crate::app::{SpinoramaOptStatus, SpinoramaStep};

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
                // Trigger speaker list load
                app.spinorama_eq.loading_speakers = true;
                app.spinorama_eq.speakers_error = None;
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
                    SpinoramaOptStatus::Idle | SpinoramaOptStatus::Failed(_) => {
                        spawn_spinorama_optimization(app);
                    }
                    SpinoramaOptStatus::Completed => {
                        app.spinorama_eq.step = SpinoramaStep::Results;
                    }
                    SpinoramaOptStatus::Running => {}
                }
                None
            }
            KeyCode::Tab => {
                if app.spinorama_eq.opt_status == SpinoramaOptStatus::Completed {
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
            KeyCode::Tab => {
                app.spinorama_eq.step = SpinoramaStep::Select;
                None
            }
            KeyCode::BackTab => {
                app.spinorama_eq.step = SpinoramaStep::Results;
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
    let s = &mut app.spinorama_eq;
    match s.selected_field {
        // ── Filters ──
        0 => {
            let n = s.num_filters as i32 + delta;
            s.num_filters = n.clamp(1, 30) as usize;
        }
        1 => s.min_freq = (s.min_freq + delta as f64 * 10.0).clamp(20.0, 500.0),
        2 => s.max_freq = (s.max_freq + delta as f64 * 500.0).clamp(1000.0, 20000.0),
        3 => s.min_db = (s.min_db + delta as f64).clamp(-24.0, 0.0),
        4 => s.max_db = (s.max_db + delta as f64).clamp(0.0, 12.0),
        5 => s.min_q = (s.min_q + delta as f64 * 0.1).clamp(0.1, 2.0),
        6 => s.max_q = (s.max_q + delta as f64 * 0.5).clamp(1.0, 20.0),
        7 => {
            s.peq_model =
                cycle_string(&s.peq_model, &["pk", "hp-pk", "hp-pk-lp", "ls-pk", "ls-pk-hs"], delta);
        }
        // ── Optimization ──
        8 => {
            s.algorithm = cycle_string(&s.algorithm, &["de", "cobyla", "nelder-mead"], delta);
        }
        9 => {
            let n = s.max_iter as i32 + delta * 1000;
            s.max_iter = n.clamp(1000, 100000) as usize;
        }
        10 => {
            let n = s.population as i32 + delta * 10;
            s.population = n.clamp(10, 200) as usize;
        }
        11 => {
            s.strategy = cycle_string(
                &s.strategy,
                &["currenttobest1bin", "best1bin", "rand1bin", "best2bin"],
                delta,
            );
        }
        12 => s.de_f = (s.de_f + delta as f64 * 0.1).clamp(0.1, 2.0),
        13 => s.de_cr = (s.de_cr + delta as f64 * 0.1).clamp(0.1, 1.0),
        // ── Refinement ──
        14 => s.refine = !s.refine,
        15 => {
            s.local_algo = cycle_string(&s.local_algo, &["cobyla", "nelder-mead"], delta);
        }
        // ── Smoothing ──
        16 => s.smooth = !s.smooth,
        17 => {
            let n = s.smooth_n as i32 + delta;
            s.smooth_n = n.clamp(1, 24) as usize;
        }
        18 => s.psychoacoustic = !s.psychoacoustic,
        // ── Constraints ──
        19 => s.spacing_weight = (s.spacing_weight + delta as f64 * 10.0).clamp(0.0, 1000.0),
        20 => s.min_spacing_oct = (s.min_spacing_oct + delta as f64 * 0.01).clamp(0.01, 1.0),
        21 => s.asymmetric_loss = !s.asymmetric_loss,
        // ── Convergence ──
        22 => {
            s.tolerance = if delta > 0 {
                (s.tolerance * 10.0).min(1e-1)
            } else {
                (s.tolerance / 10.0).max(1e-6)
            };
        }
        23 => {
            s.atolerance = if delta > 0 {
                (s.atolerance * 10.0).min(1e-1)
            } else {
                (s.atolerance / 10.0).max(1e-6)
            };
        }
        24 => {
            s.sample_rate = match (s.sample_rate, delta > 0) {
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
pub fn poll_spinorama_speaker_load(app: &mut App) -> bool {
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
    if let Ok(mut g) = result_slot.lock() { *g = None; }

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
    Arc<
        Mutex<
            Option<
                Result<sotf_audio_player::autoeq::SpeakerOptimizationResult, String>,
            >,
        >,
    >,
> = std::sync::OnceLock::new();
static OPT_PROGRESS: std::sync::OnceLock<Arc<Mutex<Option<(usize, usize, f64, f32)>>>> =
    std::sync::OnceLock::new();

/// Poll optimization progress/result on every tick while optimization is running.
/// Returns true if the UI needs a redraw.
pub fn poll_spinorama_optimization(app: &mut App) -> bool {
    use crate::app::{SpinoramaFilter, SpinoramaOptStatus};

    if app.spinorama_eq.opt_status != SpinoramaOptStatus::Running {
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
                        .map(|b| SpinoramaFilter {
                            filter_type: format!("{:?}", b.filter_type),
                            freq: b.freq,
                            q: b.q,
                            gain_db: b.db_gain,
                        })
                        .collect();
                    app.spinorama_eq.curve_frequencies = r.frequencies.clone();
                    app.spinorama_eq.curve_input = r.input_curve.clone();
                    app.spinorama_eq.curve_target = r.target_curve.clone();
                    app.spinorama_eq.curve_corrected = r.corrected_curve.clone();
                    app.spinorama_eq.curve_filter_response = r.filter_response.clone();
                    app.spinorama_eq.loss_history = r.optimization_history.clone();
                    app.spinorama_eq.opt_status = SpinoramaOptStatus::Completed;
                    app.spinorama_eq.opt_progress = 1.0;
                }
                Err(e) => {
                    app.spinorama_eq.opt_status = SpinoramaOptStatus::Failed(e);
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
    use crate::app::SpinoramaOptStatus;

    // Start new optimization
    let speaker = match &app.spinorama_eq.selected_speaker {
        Some(s) => s.clone(),
        None => {
            app.spinorama_eq.opt_status =
                SpinoramaOptStatus::Failed("No speaker selected".to_string());
            return;
        }
    };

    app.spinorama_eq.opt_status = SpinoramaOptStatus::Running;
    app.spinorama_eq.opt_progress = 0.0;
    app.spinorama_eq.opt_iteration = 0;
    app.spinorama_eq.opt_loss = 0.0;
    app.spinorama_eq.filters.clear();

    let s = &app.spinorama_eq;
    let num_filters = s.num_filters;
    let min_freq = s.min_freq;
    let max_freq = s.max_freq;
    let min_db = s.min_db;
    let max_db = s.max_db;
    let min_q = s.min_q;
    let max_q = s.max_q;
    let max_iter = s.max_iter;
    let peq_model_str = s.peq_model.clone();
    let algorithm = s.algorithm.clone();
    let population = s.population;
    let strategy = s.strategy.clone();
    let de_f = s.de_f;
    let de_cr = s.de_cr;
    let refine = s.refine;
    let local_algo = s.local_algo.clone();
    let smooth = s.smooth;
    let smooth_n = s.smooth_n;
    let psychoacoustic = s.psychoacoustic;
    let spacing_weight = s.spacing_weight;
    let min_spacing_oct = s.min_spacing_oct;
    let asymmetric_loss = s.asymmetric_loss;
    let tolerance = s.tolerance;
    let atolerance = s.atolerance;
    let sample_rate = s.sample_rate;

    let result_slot2 = OPT_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    let progress_slot2 = OPT_PROGRESS
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    // Clear any stale result from a previous run
    if let Ok(mut g) = result_slot2.lock() { *g = None; }
    if let Ok(mut g) = progress_slot2.lock() { *g = None; }

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
        // Map algorithm string to autoeq algo format
        args.algo = match algorithm.as_str() {
            "de" => "autoeq:de".to_string(),
            "cobyla" => "nlopt:cobyla".to_string(),
            "nelder-mead" => "nlopt:neldermead".to_string(),
            other => other.to_string(),
        };
        // Map PEQ model string to enum
        args.peq_model = match peq_model_str.as_str() {
            "pk" => autoeq::PeqModel::Pk,
            "hp-pk" => autoeq::PeqModel::HpPk,
            "hp-pk-lp" => autoeq::PeqModel::HpPkLp,
            "ls-pk" => autoeq::PeqModel::LsPk,
            "ls-pk-hs" => autoeq::PeqModel::LsPkHs,
            _ => autoeq::PeqModel::Pk,
        };
        // Map loss type based on asymmetric_loss flag
        args.loss = if asymmetric_loss {
            autoeq::LossType::SpeakerFlatAsymmetric
        } else {
            autoeq::LossType::SpeakerFlat
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
        let callback: sotf_audio_player::autoeq::SpeakerOptimizationCallback =
            Box::new(move |p| {
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
