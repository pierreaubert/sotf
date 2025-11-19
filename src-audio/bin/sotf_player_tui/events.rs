use crate::app::{App, InputMode, Screen};
use crate::plugins::PluginType;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
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
        InputMode::Normal => handle_normal_mode(app, key),
    }
}

fn handle_normal_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
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

        // TAB to cycle through screens
        KeyCode::Tab => {
            app.current_screen = match app.current_screen {
                Screen::Library => Screen::DirectoryManager,
                Screen::DirectoryManager => Screen::Queue,
                Screen::Queue => Screen::Plugins,
                Screen::Plugins => Screen::Devices,
                Screen::Devices => Screen::Library,
            };
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

        // Global volume controls with Shift+Arrow keys
        KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.increase_volume();
            Some(PlayerCommand::SetVolume(app.volume))
        }
        KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.decrease_volume();
            Some(PlayerCommand::SetVolume(app.volume))
        }

        // Output device selection with Ctrl+Arrow keys
        KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.select_next_output_device();
            // TODO: Implement device switching
            None
        }
        KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.select_previous_output_device();
            // TODO: Implement device switching
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
    use crate::app::LibraryViewMode;

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
        KeyCode::Char('m') => {
            // Maintenance: clean up database
            match app.clean_library_database() {
                Ok(removed) => {
                    if removed > 0 {
                        app.status_message =
                            Some(format!("Cleaned {} missing tracks from database", removed));
                        log::info!("Database maintenance: removed {} missing tracks", removed);
                    } else {
                        app.status_message =
                            Some("Database is clean - no missing tracks found".to_string());
                        log::info!("Database maintenance: no missing tracks found");
                    }
                }
                Err(e) => {
                    app.status_message = Some(format!("Database maintenance failed: {}", e));
                    log::error!("Database maintenance failed: {}", e);
                }
            }
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
            if let Err(e) = app.scan_library() {
                log::error!("Failed to scan library: {}", e);
            }
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
        KeyCode::Char('+') | KeyCode::Char('=') => {
            app.increase_volume();
            Some(PlayerCommand::SetVolume(app.volume))
        }
        KeyCode::Char('-') => {
            app.decrease_volume();
            Some(PlayerCommand::SetVolume(app.volume))
        }
        _ => None,
    }
}

fn handle_search_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use crate::app::LibraryViewMode;

    const PAGE_SIZE: usize = 20;

    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
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
            Some(PlayerCommand::UpdatePlugins)
        }
        KeyCode::Char('t') => {
            // Toggle plugin enabled/disabled
            app.toggle_plugin(app.selected_plugin_index);
            Some(PlayerCommand::UpdatePlugins)
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            app.remove_plugin(app.selected_plugin_index);
            Some(PlayerCommand::UpdatePlugins)
        }
        KeyCode::Char('u') | KeyCode::Char('U') => {
            app.move_plugin_up(app.selected_plugin_index);
            Some(PlayerCommand::UpdatePlugins)
        }
        KeyCode::Char('n') | KeyCode::Char('N') => {
            app.move_plugin_down(app.selected_plugin_index);
            Some(PlayerCommand::UpdatePlugins)
        }
        KeyCode::Char('1') => {
            // Quick add EQ
            app.add_plugin(&PluginType::EQ);
            Some(PlayerCommand::UpdatePlugins)
        }
        KeyCode::Char('2') => {
            // Quick add Upmixer
            app.add_plugin(&PluginType::Upmixer);
            Some(PlayerCommand::UpdatePlugins)
        }
        KeyCode::Char('3') => {
            // Quick add Compressor
            app.add_plugin(&PluginType::Compressor);
            Some(PlayerCommand::UpdatePlugins)
        }
        KeyCode::Char('5') => {
            // Quick add Limiter
            app.add_plugin(&PluginType::Limiter);
            Some(PlayerCommand::UpdatePlugins)
        }
        KeyCode::Char('6') => {
            // Quick add Loudness Compensation
            app.add_plugin(&PluginType::LoudnessCompensation);
            Some(PlayerCommand::UpdatePlugins)
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
                app.needs_plugin_update = true;
                Some(PlayerCommand::UpdatePlugins)
            } else {
                None
            }
        }
        KeyCode::Right | KeyCode::Char('l') => {
            // Increase parameter value
            if app.adjust_selected_param(1.0) {
                app.needs_plugin_update = true;
                Some(PlayerCommand::UpdatePlugins)
            } else {
                None
            }
        }
        KeyCode::Char('[') => {
            // Large decrease
            if app.adjust_selected_param(-10.0) {
                app.needs_plugin_update = true;
                Some(PlayerCommand::UpdatePlugins)
            } else {
                None
            }
        }
        KeyCode::Char(']') => {
            // Large increase
            if app.adjust_selected_param(10.0) {
                app.needs_plugin_update = true;
                Some(PlayerCommand::UpdatePlugins)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn handle_save_plugins_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.plugin_file_input.clear();
            None
        }
        KeyCode::Enter => {
            app.save_plugin_chain();
            app.input_mode = InputMode::Normal;
            None
        }
        KeyCode::Char(c) => {
            app.plugin_file_input.push(c);
            None
        }
        KeyCode::Backspace => {
            app.plugin_file_input.pop();
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
            if app.needs_plugin_update {
                Some(PlayerCommand::UpdatePlugins)
            } else {
                None
            }
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
            None
        }
        KeyCode::Backspace => {
            app.plugin_file_input.pop();
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
            app.get_selected_output_device().map(|device| PlayerCommand::SetOutputDevice(device.name.clone()))
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
    UpdatePlugins,
    SetOutputDevice(String),
}
