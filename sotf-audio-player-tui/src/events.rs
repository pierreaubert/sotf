use crate::app::{App, InputMode, Screen};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use sotf_audio_player::PluginType;
use std::time::Duration;

pub enum AppEvent {
    Tick,
    Key(KeyEvent),
    Resize,
}

pub fn handle_events(timeout: Duration) -> std::io::Result<Option<AppEvent>> {
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
    match app.input_mode {
        InputMode::Search => handle_search_mode(app, key),
        InputMode::AddDirectory => handle_add_directory_mode(app, key),
        InputMode::EditPlugin => handle_edit_plugin_mode(app, key),
        InputMode::SavePlugins => handle_save_plugins_mode(app, key),
        InputMode::LoadPlugins => handle_load_plugins_mode(app, key),
        InputMode::LoadApoFile => handle_load_apo_file_mode(app, key),
        InputMode::LoadSofaFile => handle_load_sofa_file_mode(app, key),
        InputMode::ShowHelp => handle_help_mode(app, key),
        InputMode::ShowError => handle_error_mode(app, key),
        InputMode::Normal => handle_normal_mode(app, key),
    }
}

fn handle_normal_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        // Esc to return to Main pane from Meters (takes priority over quit)
        KeyCode::Esc if app.focused_pane == crate::app::FocusedPane::Meters => {
            app.focused_pane = crate::app::FocusedPane::Main;
            None
        }
        // Quit
        KeyCode::Esc => {
            app.should_quit = true;
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
                        Screen::Library => Screen::DirectoryManager,
                        Screen::DirectoryManager => Screen::Queue,
                        Screen::Queue => Screen::Plugins,
                        Screen::Plugins => Screen::Devices,
                        Screen::Devices => {
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
            app.current_screen = Screen::DirectoryManager;
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
            // Shift-C to clear all mutes and solos
            app.clear_level_meter_mutes_and_solos();
            None
        }

        // Help
        KeyCode::Char('?') => {
            app.input_mode = InputMode::ShowHelp;
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
        KeyCode::Char('c') if app.focused_pane == crate::app::FocusedPane::Meters => {
            // 'c' to clear all mutes/solos when in Meters pane
            app.clear_level_meter_mutes_and_solos();
            None
        }

        // Screen-specific controls
        _ => match app.current_screen {
            Screen::Library => handle_library_keys(app, key),
            Screen::DirectoryManager => handle_directory_keys(app, key),
            Screen::Queue => handle_queue_keys(app, key),
            Screen::Plugins => handle_plugins_keys(app, key),
            Screen::Devices => handle_devices_keys(app, key),
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
        KeyCode::Char('t') => {
            // Toggle between flat and tree view
            app.toggle_library_view_mode();
            None
        }
        KeyCode::Char('s') => {
            // Cycle through sort orders
            let next_order = match app.library_sort_order {
                LibrarySortOrder::Artist => LibrarySortOrder::Album,
                LibrarySortOrder::Album => LibrarySortOrder::Title,
                LibrarySortOrder::Title => LibrarySortOrder::Year,
                LibrarySortOrder::Year => LibrarySortOrder::Popularity,
                LibrarySortOrder::Popularity => LibrarySortOrder::Artist,
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
            // Sort by artist
            app.set_library_sort_order(LibrarySortOrder::Artist);
            None
        }
        KeyCode::Char('2') => {
            // Sort by album
            app.set_library_sort_order(LibrarySortOrder::Album);
            None
        }
        KeyCode::Char('3') => {
            // Sort by title
            app.set_library_sort_order(LibrarySortOrder::Title);
            None
        }
        KeyCode::Char('4') => {
            // Sort by year
            app.set_library_sort_order(LibrarySortOrder::Year);
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
            // Filter: Multichannel only
            app.set_channel_filter(ChannelFilter::Multichannel);
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
            // Start ReplayGain scan for all tracks
            let _ = app.start_replay_gain_scan();
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
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Left | KeyCode::Char('h') => {
            // Toggle expansion of the selected queue item
            app.toggle_queue_item_expansion();
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
            app.search_query.clear();
            app.selected_album_index = 0;
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
            None
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            app.selected_album_index = 0;
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
            // Add a plugin - cycle through available types for simplicity
            // In a more complex UI, this could open a selection dialog
            let plugin_types = PluginType::all();
            // For now, add EQ by default, user can modify this behavior
            if let Some(first_type) = plugin_types.first() {
                app.add_plugin(first_type);
            }
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
        KeyCode::Char('n') | KeyCode::Char('N') => {
            app.move_plugin_down(app.selected_plugin_index);
            None
        }
        KeyCode::Char('1') => {
            // Quick add EQ
            app.add_plugin(&PluginType::EQ);
            None
        }
        KeyCode::Char('2') => {
            // Quick add Upmixer
            app.add_plugin(&PluginType::Upmixer);
            None
        }
        KeyCode::Char('3') => {
            // Quick add Compressor
            app.add_plugin(&PluginType::Compressor);
            None
        }
        KeyCode::Char('4') => {
            // Quick add Gate
            app.add_plugin(&PluginType::Gate);
            None
        }
        KeyCode::Char('5') => {
            // Quick add Limiter
            app.add_plugin(&PluginType::Limiter);
            None
        }
        KeyCode::Char('6') => {
            // Quick add Loudness Compensation
            app.add_plugin(&PluginType::LoudnessCompensation);
            None
        }
        KeyCode::Char('7') => {
            // Quick add Binaural Decoder
            app.add_plugin(&PluginType::BinauralDecoder);
            None
        }
        KeyCode::Char('8') => {
            // Quick add Convolution
            app.add_plugin(&PluginType::Convolution);
            None
        }
        KeyCode::Char('9') => {
            // Quick add Loudness Monitor
            app.add_plugin(&PluginType::LoudnessMonitor);
            None
        }
        KeyCode::Char('0') => {
            // Quick add Spectrum Analyzer
            app.add_plugin(&PluginType::SpectrumAnalyzer);
            None
        }
        KeyCode::Char('g') => {
            // Quick add Gain
            app.add_plugin(&PluginType::Gain);
            None
        }
        KeyCode::Char('m') => {
            // Quick add Channel Mute/Solo
            app.add_plugin(&PluginType::ChannelMuteSolo);
            None
        }
        _ => None,
    }
}

fn handle_edit_plugin_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
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
            use sotf_audio_player::PluginSettings;
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
        KeyCode::Char('f') => {
            // Load SOFA file (for Binaural Decoder plugins)
            use sotf_audio_player::PluginSettings;
            if let Some(plugin) = app.plugin_chain.get_plugin(app.selected_plugin_index) {
                if matches!(plugin.settings, PluginSettings::BinauralDecoder { .. }) {
                    app.input_mode = InputMode::LoadSofaFile;
                    app.status_message = Some("Enter path to SOFA file:".to_string());
                } else {
                    app.status_message = Some(
                        "SOFA files can only be loaded for Binaural Decoder plugins".to_string(),
                    );
                }
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

#[derive(Debug, Clone)]
pub enum PlayerCommand {
    Play(std::path::PathBuf),
    Pause,
    Resume,
    Stop,
    SetVolume(f32),
    SetOutputDevice(String),
}
