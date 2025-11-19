use crate::app::{App, InputMode, Screen};
use crate::plugins::{PluginSettings, PluginType};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

/// Clean up track/song titles by:
/// - Trimming ALL leading/trailing whitespace
/// - Replacing multiple consecutive spaces with a single space
/// - Removing tabs, newlines, and other control characters
fn clean_track_name(name: &str) -> String {
    clean_text(name)
}

/// Clean up any text field (artist, album, track) by:
/// - Trimming ALL leading/trailing whitespace
/// - Replacing multiple consecutive spaces with a single space
/// - Removing tabs, newlines, and other control characters
fn clean_text(text: &str) -> String {
    // First, replace all control characters (tabs, newlines, etc.) with spaces
    let normalized: String = text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();

    // Then split by whitespace and rejoin with single spaces (handles multiple spaces)
    // This also trims all leading and trailing whitespace
    normalized
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Truncate a string to a maximum length, adding "..." if truncated
fn truncate_with_ellipsis(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_text() {
        // Test leading/trailing whitespace
        assert_eq!(clean_text("  Text  "), "Text");

        // Test multiple spaces
        assert_eq!(clean_text("Text    Name"), "Text Name");

        // Test tabs
        assert_eq!(clean_text("Text\tName"), "Text Name");

        // Test newlines
        assert_eq!(clean_text("Text\nName"), "Text Name");

        // Test combination
        assert_eq!(clean_text("  \t Text   Name\n  "), "Text Name");

        // Test normal string (no change needed)
        assert_eq!(clean_text("Text Name"), "Text Name");

        // Test empty string
        assert_eq!(clean_text(""), "");

        // Test only whitespace
        assert_eq!(clean_text("   \t\n  "), "");
    }

    #[test]
    fn test_clean_track_name() {
        // Verify clean_track_name wraps clean_text correctly
        assert_eq!(clean_track_name("  Track Name  "), "Track Name");
        assert_eq!(clean_track_name("Track\tName"), "Track Name");
    }

    #[test]
    fn test_truncate_with_ellipsis() {
        // Test no truncation needed
        assert_eq!(truncate_with_ellipsis("Short", 10), "Short");

        // Test exact length
        assert_eq!(truncate_with_ellipsis("Exact", 5), "Exact");

        // Test truncation
        assert_eq!(truncate_with_ellipsis("This is a very long track name", 15), "This is a ve...");

        // Test truncation at edge
        assert_eq!(truncate_with_ellipsis("12345678", 5), "12...");

        // Test very short max_len
        assert_eq!(truncate_with_ellipsis("Test", 3), "...");

        // Test empty string
        assert_eq!(truncate_with_ellipsis("", 10), "");
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title bar
            Constraint::Min(0),    // Main content
            Constraint::Length(3), // Status bar
        ])
        .split(f.area());

    // Title bar
    draw_title(f, chunks[0], app);

    // Split main content into left (main) and right (meters) columns
    // Adjust right column width based on number of channels
    let num_channels = app
        .loudness_info
        .as_ref()
        .map(|info| info.channel_peaks.len())
        .unwrap_or(2);

    let right_col_pct = if num_channels > 4 {
        20 // For 5.1 and above, use wider right column
    } else {
        12 // For stereo/quad, use standard width
    };

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(100 - right_col_pct), // Main content
            Constraint::Percentage(right_col_pct),        // Right column (LUFS, level meter, volume)
        ])
        .split(chunks[1]);

    // Main content based on current screen
    match app.current_screen {
        Screen::Library => draw_library_screen(f, main_chunks[0], app),
        Screen::DirectoryManager => draw_directory_manager(f, main_chunks[0], app),
        Screen::Queue => draw_queue_screen(f, main_chunks[0], app),
        Screen::Plugins => draw_plugins_screen(f, main_chunks[0], app),
        Screen::Devices => draw_devices_screen(f, main_chunks[0], app),
    }

    // Right column with meters
    draw_meters_column(f, main_chunks[1], app);

    // Status bar
    draw_status_bar(f, chunks[2], app);

    // Plugin parameter editor modal (if in edit mode)
    if app.input_mode == InputMode::EditPlugin {
        draw_plugin_editor_modal(f, app);
    }

    // Save/Load plugin input dialog
    if app.input_mode == InputMode::SavePlugins {
        draw_save_plugins_dialog(f, app);
    } else if app.input_mode == InputMode::LoadPlugins {
        draw_load_plugins_dialog(f, app);
    }
}

fn draw_title(f: &mut Frame, area: Rect, app: &App) {
    // Split title area into three parts: SOTF title, screen boxes, output device
    let title_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(6),      // SOTF title
            Constraint::Min(0),         // Screen boxes (expandable)
            Constraint::Length(30),     // Output device
        ])
        .split(area);

    // Draw "SOTF" on the left
    let sotf_title = Paragraph::new("SotF")
        .style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Left)
        .block(Block::default().borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM));

    f.render_widget(sotf_title, title_chunks[0]);

    // Draw screen indicator boxes in the middle
    draw_screen_boxes(f, title_chunks[1], app);

    // Device selector on the right
    let device_text = if let Some(device) = app.get_selected_output_device() {
        format!("Out: {}", device.name)
    } else {
        "Out: Default".to_string()
    };

    let device_widget = Paragraph::new(device_text)
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Output Device"),
        );

    f.render_widget(device_widget, title_chunks[2]);
}

fn draw_screen_boxes(f: &mut Frame, area: Rect, app: &App) {
    // Define screens with their labels
    let screens = [
        (Screen::Library, "Library"),
        (Screen::DirectoryManager, "Directories"),
        (Screen::Queue, "Queue"),
        (Screen::Plugins, "Plugins"),
        (Screen::Devices, "Devices"),
    ];

    // Create spans for each screen box
    let mut spans = vec![Span::raw(" ")]; // Leading space

    for (screen, label) in &screens {
        let is_active = *screen == app.current_screen;

        // Box with screen label
        let style = if is_active {
            Style::default()
                .fg(Color::Green)
                .bg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::White)
                .bg(Color::Black)
        };

        spans.push(Span::raw("[ "));
        spans.push(Span::styled(label.to_string(), style));
        spans.push(Span::raw(" ]"));
        spans.push(Span::raw(" "));
    }

    let boxes = Paragraph::new(Line::from(spans))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::TOP | Borders::BOTTOM));

    f.render_widget(boxes, area);
}

fn draw_library_screen(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search box
            Constraint::Min(0),    // Album list
        ])
        .split(area);

    // Search box
    draw_search_box(f, chunks[0], app);

    // Album list
    draw_album_list(f, chunks[1], app);
}

fn draw_search_box(f: &mut Frame, area: Rect, app: &App) {
    let input_style = if app.input_mode == InputMode::Search {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let search_text = if app.input_mode == InputMode::Search {
        format!("Search: {}█", app.search_query)
    } else {
        format!("Search: {}", app.search_query)
    };

    let search_box = Paragraph::new(search_text).style(input_style).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Search Albums (Press '/' to search, ESC to exit search)"),
    );

    f.render_widget(search_box, area);
}

fn draw_album_list(f: &mut Frame, area: Rect, app: &App) {
    use crate::app::{LibraryViewMode, TreeItem};
    use ratatui::widgets::{ListState, StatefulWidget};

    // Calculate channel count for truncation logic
    let num_channels = app
        .loudness_info
        .as_ref()
        .map(|info| info.channel_peaks.len())
        .unwrap_or(2);

    match app.library_view_mode {
        LibraryViewMode::Flat => {
            let albums = app.filtered_albums();

            let items: Vec<ListItem> = albums
                .iter()
                .enumerate()
                .map(|(i, album)| {
                    // Clean and truncate to prevent overflow into meters column
                    // Truncation length adjusted based on right column width
                    // For stereo (12% right column): 100 chars is safe
                    // For 5.1 (20% right column): 85 chars to be safe
                    let raw_content = album.display_name();
                    let cleaned = clean_text(&raw_content);
                    let max_len = if num_channels > 4 { 85 } else { 100 };
                    let content = truncate_with_ellipsis(&cleaned, max_len);

                    let style = if i == app.selected_album_index {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    ListItem::new(content).style(style)
                })
                .collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(format!(
                    "Albums ({}) - 'a' to add, 't' to toggle tree view, 'm' for maintenance",
                    albums.len()
                )))
                .highlight_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::White)
                        .add_modifier(Modifier::BOLD),
                );

            let mut state = ListState::default();
            state.select(Some(app.selected_album_index));

            StatefulWidget::render(list, area, f.buffer_mut(), &mut state);
        }
        LibraryViewMode::TreeView => {
            let tree_items = app.get_tree_items();

            let items: Vec<ListItem> = tree_items
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let (content, style) = match item {
                        TreeItem::Artist { name, expanded } => {
                            let prefix = if *expanded { "▼ " } else { "▶ " };
                            // Clean artist name and truncate to prevent overflow
                            let cleaned_name = clean_text(name);
                            let truncated_name = truncate_with_ellipsis(&cleaned_name, 95);
                            let content = format!("{}{}", prefix, truncated_name);
                            let mut style = Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD);
                            if i == app.selected_tree_index {
                                style = style.bg(Color::DarkGray);
                            }
                            (content, style)
                        }
                        TreeItem::Album { index } => {
                            if let Some(album) = app.library.albums.get(*index) {
                                // Use display_name for consistency, clean and truncate
                                let raw_album = album.display_name();
                                let cleaned = clean_text(&raw_album);
                                let truncated = truncate_with_ellipsis(&cleaned, 90);
                                let content = format!("  └─ {}", truncated);
                                let mut style = Style::default();
                                if i == app.selected_tree_index {
                                    style = Style::default()
                                        .fg(Color::Black)
                                        .bg(Color::White)
                                        .add_modifier(Modifier::BOLD);
                                }
                                (content, style)
                            } else {
                                ("  └─ <unknown>".to_string(), Style::default())
                            }
                        }
                    };
                    ListItem::new(content).style(style)
                })
                .collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(format!(
                    "Artists ({}) - 'h/l' to expand/collapse, 'a' to add, 't' to toggle view",
                    app.artist_tree.len()
                )))
                .highlight_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::White)
                        .add_modifier(Modifier::BOLD),
                );

            let mut state = ListState::default();
            state.select(Some(app.selected_tree_index));

            StatefulWidget::render(list, area, f.buffer_mut(), &mut state);
        }
    }
}

fn draw_directory_manager(f: &mut Frame, area: Rect, app: &App) {
    // Calculate constraints based on whether we have autocomplete suggestions
    let autocomplete_height =
        if app.input_mode == InputMode::AddDirectory && !app.autocomplete_suggestions.is_empty() {
            (app.autocomplete_suggestions.len().min(5) + 2) as u16 // Max 5 suggestions + borders
        } else {
            0
        };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),                   // Input box
            Constraint::Length(autocomplete_height), // Autocomplete suggestions
            Constraint::Min(0),                      // Directory list
        ])
        .split(area);

    // Input box for adding directories
    let input_style = if app.input_mode == InputMode::AddDirectory {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let input_text = if app.input_mode == InputMode::AddDirectory {
        format!("Path: {}█ (Tab to autocomplete)", app.directory_input)
    } else {
        "Path: (Press 'a' to add directory)".to_string()
    };

    let input_box = Paragraph::new(input_text).style(input_style).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Add Directory"),
    );

    f.render_widget(input_box, chunks[0]);

    // Show autocomplete suggestions if in add directory mode
    if app.input_mode == InputMode::AddDirectory && !app.autocomplete_suggestions.is_empty() {
        let suggestion_items: Vec<ListItem> = app
            .autocomplete_suggestions
            .iter()
            .take(5) // Show max 5 suggestions
            .enumerate()
            .map(|(i, suggestion)| {
                let style = if i == app.autocomplete_index {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                };
                ListItem::new(suggestion.as_str()).style(style)
            })
            .collect();

        let suggestions_list = List::new(suggestion_items).block(
            Block::default().borders(Borders::ALL).title(format!(
                "Suggestions ({}/{})",
                app.autocomplete_index + 1,
                app.autocomplete_suggestions.len()
            )),
        );

        f.render_widget(suggestions_list, chunks[1]);
    }

    // Directory list with tree view
    let tree_items = app.get_directory_tree_items();

    let items: Vec<ListItem> = tree_items
        .iter()
        .enumerate()
        .map(|(i, (path, level, expanded))| {
            let indent = "  ".repeat(*level);
            let expand_indicator = if *level == 0 {
                if *expanded { "▼ " } else { "▶ " }
            } else {
                "└─ "
            };

            let path_str = if *level == 0 {
                path.display().to_string()
            } else {
                // For subdirectories, just show the name
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string())
            };

            // For main directories (level 0), add track count and last scan time
            let info_str = if *level == 0 {
                // Find the directory info
                if let Some(dir_info) = app.library.directories.iter().find(|d| d.path == *path) {
                    let track_count = dir_info.file_count;
                    let last_scan = if let Some(time) = dir_info.last_scanned {
                        // Format as relative time (e.g., "2 days ago")
                        if let Ok(elapsed) = time.elapsed() {
                            let secs = elapsed.as_secs();
                            if secs < 60 {
                                "just now".to_string()
                            } else if secs < 3600 {
                                format!("{} min ago", secs / 60)
                            } else if secs < 86400 {
                                format!("{} hrs ago", secs / 3600)
                            } else {
                                format!("{} days ago", secs / 86400)
                            }
                        } else {
                            "never".to_string()
                        }
                    } else {
                        "never".to_string()
                    };
                    format!(" [{} tracks, {}]", track_count, last_scan)
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            let content = format!("{}{}{}{}", indent, expand_indicator, path_str, info_str);

            let style = if i == app.selected_directory_index {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if *level == 0 {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(content).style(style)
        })
        .collect();

    // Update title to show scan progress if scanning
    let title = if app.scan_in_progress {
        format!(
            "Directories - Scanning: {}T/{}A",
            app.scan_progress_tracks, app.scan_progress_albums
        )
    } else if let Some(msg) = &app.status_message {
        // Show scan results in title if available
        if msg.contains("Scan complete") {
            format!("Directories - {}", msg)
        } else {
            "Directories - Enter/Right=expand, 'd'=remove, 's'=scan, 'a'=add".to_string()
        }
    } else {
        "Directories - Enter/Right=expand, 'd'=remove, 's'=scan, 'a'=add".to_string()
    };

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(list, chunks[2]);
}

fn draw_queue_screen(f: &mut Frame, area: Rect, app: &App) {
    let mut items: Vec<ListItem> = Vec::new();

    for (i, item) in app.queue.iter().enumerate() {
        let is_current = app.current_queue_index == Some(i);
        let is_selected = i == app.selected_queue_index;
        let is_expanded = app.expanded_queue_items.get(i).copied().unwrap_or(false);

        // Album header
        let expand_indicator = if is_expanded { "▼" } else { "▶" };
        let raw_display = item.album.display_name();
        let cleaned_display = clean_text(&raw_display);
        let truncated_display = truncate_with_ellipsis(&cleaned_display, 90);
        let mut content = format!("{} {}", expand_indicator, truncated_display);

        if is_current {
            let track_info = format!(
                " [Track {}/{}]",
                item.current_track_index + 1,
                item.album.tracks.len()
            );
            content.push_str(&track_info);
        }

        let mut style = Style::default();
        if is_selected {
            style = style
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD);
        } else if is_current {
            style = style.fg(Color::Green).add_modifier(Modifier::BOLD);
        }

        items.push(ListItem::new(content).style(style));

        // Show individual tracks if expanded
        if is_expanded {
            for (track_idx, track) in item.album.tracks.iter().enumerate() {
                let is_current_track = is_current && track_idx == item.current_track_index;
                let raw_track_name = track
                    .title
                    .as_deref()
                    .unwrap_or_else(|| track.path.file_name().unwrap().to_str().unwrap());
                let cleaned_track_name = clean_track_name(raw_track_name);
                // Truncate to max 60 chars to prevent overflow into meters
                let track_name = truncate_with_ellipsis(&cleaned_track_name, 60);

                let duration_str = if let Some(duration) = track.duration_secs {
                    format!(" ({}:{:02})", duration / 60, duration % 60)
                } else {
                    String::new()
                };

                let track_content = if is_current_track {
                    if app.is_playing {
                        format!("  ▶ {}.{}{}", track_idx + 1, track_name, duration_str)
                    } else {
                        format!("  ⏸ {}.{}{}", track_idx + 1, track_name, duration_str)
                    }
                } else {
                    format!("    {}.{}{}", track_idx + 1, track_name, duration_str)
                };

                let track_style = if is_current_track {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                items.push(ListItem::new(track_content).style(track_style));
            }
        }
    }

    let title = if app.queue.is_empty() {
        "Queue (empty) - Add albums from library".to_string()
    } else {
        format!(
            "Queue ({}) - Enter: play album, 'p' play, SPACE pause, 'n' next, 'b' prev, 'l'/'h' expand, 'd' remove",
            app.queue.len()
        )
    };

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(list, area);
}

fn draw_plugins_screen(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(70), // Plugin list
            Constraint::Percentage(30), // Available plugins
        ])
        .split(area);

    // Plugin chain list
    draw_plugin_chain(f, chunks[0], app);

    // Available plugins list
    draw_available_plugins(f, chunks[1], app);
}

fn draw_plugin_chain(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .plugin_chain
        .plugins()
        .iter()
        .enumerate()
        .map(|(i, plugin)| {
            let enabled_marker = if plugin.enabled { "●" } else { "○" };
            let content = format!(
                "{} {} - {}",
                enabled_marker,
                i + 1,
                plugin.plugin_type().name()
            );

            let style = if i == app.selected_plugin_index {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if plugin.enabled {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let title = if app.plugin_chain.is_empty() {
        "Plugin Chain (empty)".to_string()
    } else {
        format!(
            "Plugin Chain ({}) - Output: {}ch",
            app.plugin_chain.len(),
            app.plugin_chain.output_channels()
        )
    };

    let help_text = if app.plugin_chain.is_empty() {
        " | Press 'a' to add plugins | 's'=save, 'l'=load"
    } else {
        " | 'e'=edit, 't'=toggle, 'd'=remove, '↑/↓'=move, 'a'=add, 's'=save, 'l'=load"
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("{}{}", title, help_text)),
    );

    f.render_widget(list, area);
}

fn draw_available_plugins(f: &mut Frame, area: Rect, _app: &App) {
    let plugins = PluginType::all();
    let items: Vec<ListItem> = plugins
        .iter()
        .map(|plugin_type| {
            let content = format!("{}\n  {}", plugin_type.name(), plugin_type.description());
            ListItem::new(content).style(Style::default().fg(Color::Cyan))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Available Plugins - Press 'a' to add"),
    );

    f.render_widget(list, area);
}

fn draw_devices_screen(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Info box
            Constraint::Min(0),    // Device list
        ])
        .split(area);

    // Info box
    let info_text = "Select output device with ↑/↓, press Enter or Space to apply";
    let info = Paragraph::new(info_text)
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("Help"));

    f.render_widget(info, chunks[0]);

    // Device list
    let items: Vec<ListItem> = app
        .output_devices
        .iter()
        .enumerate()
        .map(|(i, device)| {
            let default_marker = if device.is_default { " [DEFAULT]" } else { "" };
            let current_marker = if i == app.selected_output_device_index {
                "► "
            } else {
                "  "
            };

            // Show device name and some config info
            let config_info = if let Some(ref config) = device.default_config {
                format!(" ({}ch, {}Hz)", config.channels, config.sample_rate)
            } else {
                String::new()
            };

            let content = format!(
                "{}{}{}{}",
                current_marker, device.name, default_marker, config_info
            );

            let style = if i == app.selected_output_device_index {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if device.is_default {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let title = if app.output_devices.is_empty() {
        "Output Devices (none found)".to_string()
    } else {
        format!(
            "Output Devices ({}) - Use ↑/↓ to select, Enter to apply",
            app.output_devices.len()
        )
    };

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(list, chunks[1]);
}

fn draw_meters_column(f: &mut Frame, area: Rect, app: &App) {
    // Split the right column - LUFS, level meter, volume
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // LUFS box
            Constraint::Min(0),    // Level meter box (expandable)
            Constraint::Length(3), // Volume box
        ])
        .split(area);

    // Draw LUFS info box
    draw_lufs_box(f, chunks[0], app);

    // Draw level meter box
    draw_level_meter_box(f, chunks[1], app);

    // Draw volume box
    draw_volume_box(f, chunks[2], app);
}

fn draw_lufs_box(f: &mut Frame, area: Rect, app: &App) {
    let text = if let Some(ref loudness) = app.loudness_info {
        let momentary = if loudness.momentary_lufs.is_finite() {
            format!("{:>6.1}", loudness.momentary_lufs)
        } else {
            " -∞".to_string()
        };
        let shortterm = if loudness.shortterm_lufs.is_finite() {
            format!("{:>6.1}", loudness.shortterm_lufs)
        } else {
            " -∞".to_string()
        };
        let peak_db = 20.0 * loudness.peak.max(0.0001).log10();

        vec![
            Line::from(vec![
                Span::raw("M: "),
                Span::styled(
                    format!("{} LUFS", momentary),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                Span::raw("S: "),
                Span::styled(
                    format!("{} LUFS", shortterm),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                Span::raw("Pk: "),
                Span::styled(
                    format!("{:>5.1} dBFS", peak_db),
                    Style::default().fg(Color::Red),
                ),
            ]),
        ]
    } else {
        vec![
            Line::from("M:   -∞ LUFS"),
            Line::from("S:   -∞ LUFS"),
            Line::from("Pk:  -∞ dBFS"),
        ]
    };

    let paragraph =
        Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Loudness"));

    f.render_widget(paragraph, area);
}

fn draw_level_meter_box(f: &mut Frame, area: Rect, app: &App) {
    if let Some(ref loudness) = app.loudness_info {
        let num_channels = loudness.channel_peaks.len();
        if num_channels == 0 {
            let paragraph = Paragraph::new("No channels")
                .block(Block::default().borders(Borders::ALL).title("Levels"));
            f.render_widget(paragraph, area);
            return;
        }

        // Create inner area for meters (without borders)
        let inner = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };

        // Draw border
        let block = Block::default().borders(Borders::ALL).title("Levels");
        f.render_widget(block, area);

        // Reserve space for legend on the left (4 characters: "-60 ")
        let legend_width = 4;
        let meters_start_x = inner.x + legend_width;
        let meters_width = inner.width.saturating_sub(legend_width);

        // Calculate meter dimensions
        let meter_height = inner.height as usize;
        let channel_width = (meters_width as usize) / num_channels.max(1);

        // Draw each channel as a vertical meter
        for (ch_idx, &peak) in loudness.channel_peaks.iter().enumerate() {
            let peak_db = 20.0 * peak.max(0.0001).log10();

            // Non-linear meter scaling for better resolution at higher levels:
            // [-20, 0]   -> 50% of meter (top half)
            // [-40, -20] -> 30% of meter
            // [-60, -40] -> 20% of meter (bottom)
            let fill_ratio = if peak_db >= -20.0 {
                // Top 50%: -20 to 0 dB
                0.5 + ((peak_db + 20.0) / 40.0)
            } else if peak_db >= -40.0 {
                // Middle 30%: -40 to -20 dB
                0.2 + ((peak_db + 40.0) / 20.0) * 0.3
            } else {
                // Bottom 20%: -60 to -40 dB
                ((peak_db + 60.0) / 20.0) * 0.2
            };
            let fill_ratio = fill_ratio.clamp(0.0, 1.0);
            let filled_rows = (fill_ratio * meter_height as f64).round() as usize;

            let ch_x = meters_start_x + (ch_idx * channel_width) as u16;
            let ch_width = channel_width
                .min((meters_width as usize - ch_idx * channel_width).min(channel_width))
                as u16;

            // Build the entire meter as a single multi-line widget to ensure proper clearing
            let mut meter_lines = Vec::new();

            // Draw vertical meter from top to bottom (reversed iteration for display)
            for row_idx in (0..meter_height).rev() {
                // Determine if this row should be filled
                let is_filled = row_idx < filled_rows;

                // Color based on level (top = red, middle = yellow, bottom = green)
                let level_ratio = row_idx as f64 / meter_height as f64;
                let color = if level_ratio > 0.95 {
                    Color::Red
                } else if level_ratio > 0.90 {
                    Color::Yellow
                } else {
                    Color::Green
                };

                if is_filled {
                    // Draw filled bar
                    let bar = "█".repeat(ch_width.saturating_sub(1) as usize);
                    meter_lines.push(Line::from(Span::styled(bar, Style::default().fg(color))));
                } else {
                    // Draw empty bar
                    let bar = "░".repeat(ch_width.saturating_sub(1) as usize);
                    meter_lines.push(Line::from(Span::styled(
                        bar,
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }

            // Render the entire meter column as a single widget
            f.render_widget(
                Paragraph::new(meter_lines),
                Rect {
                    x: ch_x,
                    y: inner.y,
                    width: ch_width.saturating_sub(1),
                    height: meter_height as u16,
                },
            );

            // Draw channel label at bottom
            let label = format!("{}", ch_idx + 1);
            f.render_widget(
                Paragraph::new(label)
                    .style(Style::default().fg(Color::Cyan))
                    .alignment(Alignment::Center),
                Rect {
                    x: ch_x,
                    y: inner.y + inner.height,
                    width: ch_width.saturating_sub(1),
                    height: 1,
                },
            );
        }

        // Draw dB scale legend on the left
        if legend_width > 0 && meter_height > 0 {
            // Helper function to convert dB to non-linear fill ratio
            // Same scale as meter bars: [-20,0] = 50%, [-40,-20] = 30%, [-60,-40] = 20%
            let db_to_ratio = |db: i32| -> f64 {
                let db = db as f64;
                if db >= -20.0 {
                    0.5 + ((db + 20.0) / 40.0)
                } else if db >= -40.0 {
                    0.2 + ((db + 40.0) / 20.0) * 0.3
                } else {
                    ((db + 60.0) / 20.0) * 0.2
                }
            };

            // Draw scale marks - show 0dB at top with "dB", then just numbers
            let db_marks = [
                (0, true),
                (-5, false),
                (-10, false),
                (-20, false),
                (-30, false),
                (-40, false),
                (-50, false),
                (-60, false),
            ];

            for &(db, show_db_suffix) in &db_marks {
                // Calculate Y position using non-linear scale
                let ratio = db_to_ratio(db);
                let y_pos =
                    inner.y + inner.height - 1 - (ratio * meter_height as f64).round() as u16;

                // Only draw if within bounds
                if y_pos >= inner.y && y_pos < inner.y + inner.height {
                    let label = if show_db_suffix {
                        format!("{:>3}dB", db)
                    } else if db == 0 {
                        "   0".to_string()
                    } else {
                        format!("{:>4}", db)
                    };

                    let color = if db >= -6 {
                        Color::Red
                    } else if db >= -20 {
                        Color::Yellow
                    } else {
                        Color::DarkGray
                    };

                    f.render_widget(
                        Paragraph::new(label).style(Style::default().fg(color)),
                        Rect {
                            x: inner.x,
                            y: y_pos,
                            width: legend_width,
                            height: 1,
                        },
                    );
                }
            }
        }
    } else {
        let paragraph = Paragraph::new("No audio")
            .block(Block::default().borders(Borders::ALL).title("Levels"))
            .alignment(Alignment::Center);
        f.render_widget(paragraph, area);
    }
}

fn draw_volume_box(f: &mut Frame, area: Rect, app: &App) {
    let volume_pct = (app.volume * 100.0) as u32;
    let text = Line::from(vec![Span::styled(
        format!("{}%", volume_pct),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )]);

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Volume"))
        .alignment(Alignment::Center);

    f.render_widget(paragraph, area);
}

fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let mut status_spans = vec![Span::raw(" ")];

    // Show status message if available
    // Filter out scan-related messages unless we're on the Directory screen
    if let Some(msg) = &app.status_message {
        let is_scan_message = msg.contains("Scanning")
            || msg.contains("Scan complete")
            || msg.contains("Scan failed");
        let should_show = !is_scan_message || app.current_screen == Screen::DirectoryManager;

        if should_show {
            // Truncate message to prevent overflow (leave room for other info)
            let max_msg_len = (area.width as usize).saturating_sub(80);
            let truncated_msg = if msg.len() > max_msg_len {
                format!("{}...", &msg[..max_msg_len.saturating_sub(3)])
            } else {
                msg.clone()
            };

            status_spans.push(Span::styled(
                format!("{} | ", truncated_msg),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }

    if let Some(idx) = app.current_queue_index
        && let Some(item) = app.queue.get(idx)
            && let Some(track) = item.current_track() {
                let raw_track_name = track
                    .title
                    .as_deref()
                    .unwrap_or_else(|| track.path.file_name().unwrap().to_str().unwrap());
                let cleaned_track_name = clean_track_name(raw_track_name);
                // Truncate to max 50 chars for status bar to leave room for other info
                let track_name = truncate_with_ellipsis(&cleaned_track_name, 50);
                status_spans.push(Span::styled(
                    format!("Now: {}", track_name),
                    Style::default().fg(Color::Green),
                ));
                status_spans.push(Span::raw(" | "));
            }

    if !app.plugin_chain.is_empty() {
        status_spans.push(Span::styled(
            format!("Plugins: {} ", app.plugin_chain.len()),
            Style::default().fg(Color::Magenta),
        ));
        status_spans.push(Span::raw("| "));
    }

    status_spans.push(Span::raw("Keys: "));
    status_spans.push(Span::styled("TAB", Style::default().fg(Color::Cyan)));
    status_spans.push(Span::raw("=Next "));
    status_spans.push(Span::styled("L", Style::default().fg(Color::Cyan)));
    status_spans.push(Span::raw("/"));
    status_spans.push(Span::styled("D", Style::default().fg(Color::Cyan)));
    status_spans.push(Span::raw("/"));
    status_spans.push(Span::styled("Q", Style::default().fg(Color::Cyan)));
    status_spans.push(Span::raw("/"));
    status_spans.push(Span::styled("P", Style::default().fg(Color::Cyan)));
    status_spans.push(Span::raw("/"));
    status_spans.push(Span::styled("O", Style::default().fg(Color::Cyan)));
    status_spans.push(Span::raw("=Screens "));
    status_spans.push(Span::styled(
        "Shift+↑/↓",
        Style::default().fg(Color::Yellow),
    ));
    status_spans.push(Span::raw("=Volume "));
    status_spans.push(Span::styled("ESC", Style::default().fg(Color::Red)));
    status_spans.push(Span::raw("=Quit "));

    let status_text = Line::from(status_spans);

    let status = Paragraph::new(status_text)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: true });

    f.render_widget(status, area);
}

fn draw_plugin_editor_modal(f: &mut Frame, app: &App) {
    if let Some(plugin) = app.get_editing_plugin() {
        // Create a centered modal (60% width, 80% height)
        let area = f.area();
        let modal_width = (area.width as f32 * 0.6) as u16;
        let modal_height = (area.height as f32 * 0.8) as u16;
        let modal_x = (area.width - modal_width) / 2;
        let modal_y = (area.height - modal_height) / 2;

        let modal_area = Rect {
            x: modal_x,
            y: modal_y,
            width: modal_width,
            height: modal_height,
        };

        // Clear the background with a block
        let block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black))
            .title(format!(
                "Edit {} Plugin (ESC to close)",
                plugin.plugin_type().name()
            ));

        f.render_widget(block, modal_area);

        // Inner area for parameters
        let inner = Rect {
            x: modal_area.x + 1,
            y: modal_area.y + 1,
            width: modal_area.width.saturating_sub(2),
            height: modal_area.height.saturating_sub(2),
        };

        // Build parameter list
        let mut lines = Vec::new();
        lines.push(Line::from(vec![
            Span::styled("Use ", Style::default()),
            Span::styled("↑/↓", Style::default().fg(Color::Cyan)),
            Span::styled(" to select parameter, ", Style::default()),
            Span::styled("←/→", Style::default().fg(Color::Cyan)),
            Span::styled(" to adjust value", Style::default()),
        ]));
        lines.push(Line::from(""));

        let params = get_plugin_parameters(&plugin.settings, app.plugin_param_selection);
        for (i, param) in params.iter().enumerate() {
            let style = if i == app.plugin_param_selection {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            lines.push(Line::from(vec![
                Span::styled(format!("  {}: ", param.0), style),
                Span::styled(param.1.clone(), style.fg(Color::Yellow)),
            ]));
        }

        let paragraph = Paragraph::new(lines)
            .block(Block::default())
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, inner);
    }
}

/// Get the parameters for a plugin as (name, value) pairs
fn get_plugin_parameters(settings: &PluginSettings, _selected: usize) -> Vec<(String, String)> {
    match settings {
        PluginSettings::Upmixer {
            speaker_config,
            gain_front_direct,
            gain_front_ambient,
            gain_rear_ambient,
            lfe_cutoff_hz,
            stereo_width,
            bandpass_hz,
            height_gain,
            lfe_gain,
        } => vec![
            ("Speaker Config".to_string(), speaker_config.clone()),
            ("Front Direct Gain".to_string(), format!("{:.2}x", gain_front_direct)),
            ("Front Ambient Gain".to_string(), format!("{:.2}x", gain_front_ambient)),
            ("Rear Ambient Gain".to_string(), format!("{:.2}x", gain_rear_ambient)),
            ("LFE Cutoff".to_string(), format!("{:.0} Hz", lfe_cutoff_hz)),
            ("Stereo Width".to_string(), format!("{:.2}", stereo_width)),
            ("Bandpass".to_string(), format!("{:.0} Hz", bandpass_hz)),
            ("Height Gain".to_string(), format!("{:.2}x", height_gain)),
            ("LFE Gain".to_string(), format!("{:.2}x", lfe_gain)),
        ],
        PluginSettings::Compressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
        } => vec![
            ("Threshold".to_string(), format!("{:.1} dB", threshold_db)),
            ("Ratio".to_string(), format!("{:.1}:1", ratio)),
            ("Attack".to_string(), format!("{:.1} ms", attack_ms)),
            ("Release".to_string(), format!("{:.1} ms", release_ms)),
            ("Knee".to_string(), format!("{:.1} dB", knee_db)),
        ],
        PluginSettings::Limiter {
            threshold_db,
            release_ms,
        } => vec![
            ("Threshold".to_string(), format!("{:.1} dB", threshold_db)),
            ("Release".to_string(), format!("{:.1} ms", release_ms)),
        ],
        PluginSettings::Gate {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
        } => vec![
            ("Threshold".to_string(), format!("{:.1} dB", threshold_db)),
            ("Ratio".to_string(), format!("{:.1}:1", ratio)),
            ("Attack".to_string(), format!("{:.1} ms", attack_ms)),
            ("Release".to_string(), format!("{:.1} ms", release_ms)),
        ],
        PluginSettings::LoudnessCompensation {
            target_lufs,
            min_gain_db,
            max_gain_db,
        } => vec![
            (
                "Target LUFS".to_string(),
                format!("{:.1} LUFS", target_lufs),
            ),
            ("Min Gain".to_string(), format!("{:.1} dB", min_gain_db)),
            ("Max Gain".to_string(), format!("{:.1} dB", max_gain_db)),
        ],
        PluginSettings::EQ { filters } => {
            let mut params = Vec::new();
            for (i, filter) in filters.iter().enumerate() {
                params.push((
                    format!("Filter {} Frequency", i + 1),
                    format!("{:.0} Hz", filter.frequency),
                ));
                params.push((format!("Filter {} Q", i + 1), format!("{:.2}", filter.q)));
                params.push((
                    format!("Filter {} Gain", i + 1),
                    format!("{:.1} dB", filter.gain_db),
                ));
                params.push((
                    format!("Filter {} Type", i + 1),
                    format!("{:?}", filter.filter_type),
                ));
            }
            params
        }
    }
}

fn draw_save_plugins_dialog(f: &mut Frame, app: &App) {
    // Create a centered dialog (60% width, larger to show info)
    let area = f.area();
    let dialog_width = (area.width as f32 * 0.6) as u16;
    let dialog_height = 9;
    let dialog_x = (area.width - dialog_width) / 2;
    let dialog_y = (area.height - dialog_height) / 2;

    let dialog_area = Rect {
        x: dialog_x,
        y: dialog_y,
        width: dialog_width,
        height: dialog_height,
    };

    // Clear background
    let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black))
        .title("Save Plugin Preset");

    f.render_widget(block, dialog_area);

    // Inner area for text
    let inner = Rect {
        x: dialog_area.x + 1,
        y: dialog_area.y + 1,
        width: dialog_area.width.saturating_sub(2),
        height: dialog_area.height.saturating_sub(2),
    };

    let lines = vec![
        Line::from("Enter preset name (without .json extension):"),
        Line::from(vec![
            Span::styled("  Saved to: ", Style::default().fg(Color::DarkGray)),
            Span::styled("plugin_presets/", Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Cyan)),
            Span::raw(&app.plugin_file_input),
            Span::styled("_", Style::default().fg(Color::Green)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Note: ", Style::default().fg(Color::Yellow)),
            Span::raw(".json extension will be added automatically"),
        ]),
        Line::from("Press Enter to save, ESC to cancel"),
    ];

    let paragraph = Paragraph::new(lines)
        .block(Block::default())
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, inner);
}

fn draw_load_plugins_dialog(f: &mut Frame, app: &App) {
    // Create a larger centered dialog for preset list (70% width, 60% height)
    let area = f.area();
    let dialog_width = (area.width as f32 * 0.7) as u16;
    let dialog_height = (area.height as f32 * 0.6) as u16;
    let dialog_x = (area.width - dialog_width) / 2;
    let dialog_y = (area.height - dialog_height) / 2;

    let dialog_area = Rect {
        x: dialog_x,
        y: dialog_y,
        width: dialog_width,
        height: dialog_height,
    };

    // Clear background
    let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black))
        .title("Load Plugin Preset");

    f.render_widget(block, dialog_area);

    // Inner area for content
    let inner = Rect {
        x: dialog_area.x + 1,
        y: dialog_area.y + 1,
        width: dialog_area.width.saturating_sub(2),
        height: dialog_area.height.saturating_sub(2),
    };

    // If user is typing, show filename input; otherwise show preset list
    if !app.plugin_file_input.is_empty() {
        // Manual filename entry mode
        let lines = vec![
            Line::from("Enter filename (without .json extension):"),
            Line::from(""),
            Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Cyan)),
                Span::raw(&app.plugin_file_input),
                Span::styled("_", Style::default().fg(Color::Green)),
            ]),
            Line::from(""),
            Line::from("Press Enter to load, ESC to cancel"),
        ];

        let paragraph = Paragraph::new(lines)
            .block(Block::default())
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, inner);
    } else if app.available_plugin_presets.is_empty() {
        // No presets available
        let lines = vec![
            Line::from("No presets found in plugin_presets directory"),
            Line::from(""),
            Line::from("You can:"),
            Line::from("  • Type a filename to load a preset"),
            Line::from("  • Press ESC to cancel"),
            Line::from("  • Save your first preset with 's' from the Plugins screen"),
        ];

        let paragraph = Paragraph::new(lines)
            .block(Block::default())
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(Color::Yellow));

        f.render_widget(paragraph, inner);
    } else {
        // Show preset list
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Available Presets ", Style::default()),
                Span::styled("(↑/↓ to select, Enter to load)", Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(""),
        ];

        // Add preset items
        for (i, preset) in app.available_plugin_presets.iter().enumerate() {
            let is_selected = i == app.selected_preset_index;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };

            let marker = if is_selected { "► " } else { "  " };
            lines.push(Line::from(vec![
                Span::styled(marker, style),
                Span::styled(preset, style),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from("Or type a filename to load manually, ESC to cancel"));

        let paragraph = Paragraph::new(lines)
            .block(Block::default())
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, inner);
    }
}
