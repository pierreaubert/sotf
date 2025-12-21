use crate::app::{App, FocusedPane, InputMode, LibraryViewMode, Screen, TreeItem};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use sotf_audio_player::{PluginSettings, PluginType};

/// Format channel count as common surround notation (e.g., Mono, 2.0, 5.1, 7.1)
fn format_channel_count(n: u32) -> String {
    match n {
        1 => "Mono".to_string(),
        2 => "2.0".to_string(),
        4 => "4.0".to_string(),
        5 => "5.0".to_string(),
        6 => "5.1".to_string(),
        8 => "7.1".to_string(),
        10 => "7.1.2".to_string(),
        12 => "7.1.4".to_string(),
        14 => "9.1.4".to_string(),
        16 => "9.1.6".to_string(),
        _ => format!("{}ch", n),
    }
}

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
    let normalized: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();

    // Then split by whitespace and rejoin with single spaces (handles multiple spaces)
    // This also trims all leading and trailing whitespace
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
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
        assert_eq!(
            truncate_with_ellipsis("This is a very long track name", 15),
            "This is a ve..."
        );

        // Test truncation at edge
        assert_eq!(truncate_with_ellipsis("12345678", 5), "12...");

        // Test very short max_len
        assert_eq!(truncate_with_ellipsis("Test", 3), "...");

        // Test empty string
        assert_eq!(truncate_with_ellipsis("", 10), "");
    }
}

// Minimum height threshold for showing both library and queue simultaneously
const DUAL_VIEW_HEIGHT_THRESHOLD: u16 = 40;

pub fn draw(f: &mut Frame, app: &mut App) {
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

    // Calculate exact width needed for meters:
    // 2 (borders) + 1 (padding) + groups + 1 (padding)
    let mut meters_width = 2 + 1; // borders + left padding
    if !app.level_meter_groups.is_empty() {
        for group in &app.level_meter_groups {
            let num_channels_in_group = group.channels.len();
            // For stereo (2 channels), use 3 chars per meter + 2 chars spacing = 8 total
            // For other configs, use 1 char per channel with max(3) for controls
            let group_width = if num_channels_in_group == 2 {
                8 // 3 + 2 + 3 for stereo L-R layout
            } else {
                num_channels_in_group.max(3)
            };
            meters_width += group_width + 1; // group width + spacing
        }
    } else {
        meters_width += 1; // right padding even if no groups
    }

    // Use fixed width for right column to avoid extra space
    let right_col_width = meters_width.max(26) as u16; // Minimum 26 for LUFS/Volume boxes

    // Check window height for responsive layout
    let window_height = f.area().height;
    let use_three_columns = window_height < 40;

    let main_chunks = if use_three_columns {
        // When height < 40, use 3 columns: main, loudness+volume, level meters
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(0),                  // Main content (takes remaining space)
                Constraint::Length(26),              // Loudness + Volume column (fixed width)
                Constraint::Length(right_col_width), // Level meters column (exact width)
            ])
            .split(chunks[1])
    } else {
        // When height >= 40, use 2 columns: main, meters (with all components stacked)
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(0),                  // Main content (takes remaining space)
                Constraint::Length(right_col_width), // Right column (exact width)
            ])
            .split(chunks[1])
    };

    // Check if window is tall enough for dual view (library + queue)
    let show_dual_view = window_height >= DUAL_VIEW_HEIGHT_THRESHOLD
        && (app.current_screen == Screen::Library || app.current_screen == Screen::Queue);

    if show_dual_view {
        // Split main area vertically to show both library and queue
        let dual_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50), // Library
                Constraint::Percentage(50), // Queue
            ])
            .split(main_chunks[0]);

        // Draw both views with focus indication
        draw_library_screen(f, dual_chunks[0], app);
        draw_queue_screen(f, dual_chunks[1], app);
    } else {
        // Standard single-view mode
        match app.current_screen {
            Screen::Library => draw_library_screen(f, main_chunks[0], app),
            Screen::DirectoryManager => draw_directory_manager(f, main_chunks[0], app),
            Screen::Queue => draw_queue_screen(f, main_chunks[0], app),
            Screen::Plugins => draw_plugins_screen(f, main_chunks[0], app),
            Screen::Devices => draw_devices_screen(f, main_chunks[0], app),
        }
    }

    // Right column(s) with meters - layout depends on height
    if use_three_columns {
        // 3-column layout: loudness+volume in middle, level meters in right
        draw_loudness_and_volume_column(f, main_chunks[1], app);
        draw_level_meter_box(f, main_chunks[2], app);
    } else {
        // 2-column layout: all meters stacked vertically
        draw_meters_column(f, main_chunks[1], app);
    }

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
    } else if app.input_mode == InputMode::LoadApoFile {
        draw_load_apo_file_dialog(f, app);
    } else if app.input_mode == InputMode::LoadSofaFile {
        draw_load_sofa_file_dialog(f, app);
    }

    // Scan progress popup
    if app.scan_in_progress {
        draw_scan_progress_dialog(f, app);
    }

    // Maintenance progress dialog
    if app.maintenance_in_progress {
        draw_maintenance_progress_dialog(f, app);
    }

    // ReplayGain progress dialog
    if app.replay_gain_manager.in_progress {
        draw_replay_gain_progress_dialog(f, app);
    }

    // Help modal
    if app.input_mode == InputMode::ShowHelp {
        draw_help_modal(f, app);
    }

    // Error modal
    if app.input_mode == InputMode::ShowError {
        draw_error_modal(f, app);
    }
}

fn draw_title(f: &mut Frame, area: Rect, app: &App) {
    // Split title area into three parts: SOTF title, screen boxes, output device
    let title_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(10), // SOTF title
            Constraint::Min(0),     // Screen boxes (expandable)
            Constraint::Length(40), // Output device
        ])
        .split(area);

    // Draw "SOTF" on the left
    let sotf_title = Paragraph::new("SotF")
        .style(
            Style::default()
                .fg(app.theme.border_color)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Left)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(sotf_title, title_chunks[0]);

    // Draw screen indicator boxes in the middle
    draw_screen_boxes(f, title_chunks[1], app);

    // Device selector on the right
    let device_text = if let Some(device) = app.get_selected_output_device() {
        device.name.to_string()
    } else {
        "Default".to_string()
    };

    let device_widget = Paragraph::new(device_text)
        .style(Style::default().fg(app.theme.border_color))
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
        (Screen::Devices, "Output Devices"),
    ];

    // Create spans for each screen box
    let mut spans = vec![Span::raw(" ")]; // Leading space

    for (screen, label) in &screens {
        let is_active = *screen == app.current_screen;

        // Box with screen label
        let style = if is_active {
            Style::default()
                .fg(app.theme.border_color)
                .bg(app.theme.bg_primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(app.theme.fg_primary)
                .bg(app.theme.bg_primary)
        };

        if is_active {
            spans.push(Span::styled(label.to_string(), style));
            spans.push(Span::raw(" "));
        } else {
            spans.push(Span::raw("("));
            spans.push(Span::styled(
                label.chars().next().unwrap().to_string(),
                style,
            ));
            spans.push(Span::raw(")"));
            spans.push(Span::styled(label[1..].to_string(), style));
            spans.push(Span::raw(" "));
        }
    }

    let boxes = Paragraph::new(Line::from(spans))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

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
    let is_focused = app.current_screen == Screen::Library;
    draw_album_list(f, chunks[1], app, is_focused);
}

fn draw_search_box(f: &mut Frame, area: Rect, app: &App) {
    use crate::app::{ChannelFilter, LibrarySortOrder};

    let input_style = if app.input_mode == InputMode::Search {
        Style::default().fg(app.theme.title_color)
    } else {
        Style::default()
    };

    let search_text = if app.input_mode == InputMode::Search {
        format!("Search: {}█", app.search_query)
    } else {
        format!("Search: {}", app.search_query)
    };

    // Display current sort order (will be rendered in green)
    let sort_order_str = match app.library_sort_order {
        LibrarySortOrder::Year => "Year",
        LibrarySortOrder::Genre => "Genre",
        LibrarySortOrder::Artist => "Artist",
        LibrarySortOrder::Album => "Album",
        LibrarySortOrder::Tracks => "Tracks",
        LibrarySortOrder::Composer => "Composer",
        LibrarySortOrder::Popularity => "Popularity",
    };

    // Display current channel filter (will be rendered in green)
    let filter_str = match app.channel_filter {
        ChannelFilter::All => "All".to_string(),
        ChannelFilter::Mono => "Mono".to_string(),
        ChannelFilter::Stereo => "2.0".to_string(),
        ChannelFilter::Surround => "5.x".to_string(),
        ChannelFilter::Surround71 => "7.1".to_string(),
        ChannelFilter::SurroundPlus => "8+".to_string(),
        ChannelFilter::Mixed => "Mixed".to_string(),
        ChannelFilter::Specific(n) => format_channel_count(n),
    };

    // Get available channel counts for help text
    let available_counts = app.get_unique_channel_counts();
    let counts_str = if available_counts.is_empty() {
        String::new()
    } else {
        // Note: We'll show all available counts without brackets
        // The current filter is already shown in green in the title
        format!(
            " | Available: {}",
            available_counts
                .iter()
                .map(|&n| format_channel_count(n))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    // Build title with colored sorting and filtering
    let suffix = format!(" [c/5-9]{})", counts_str);
    let title_spans = vec![
        Span::raw("Search Albums ('/' search | Sort: "),
        Span::styled(sort_order_str, Style::default().fg(app.theme.border_color)),
        Span::raw(" [s/1-4] | Filter: "),
        Span::styled(&filter_str, Style::default().fg(app.theme.border_color)),
        Span::raw(suffix),
    ];
    let title = Line::from(title_spans);

    let search_box = Paragraph::new(search_text)
        .style(input_style)
        .block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(search_box, area);
}

fn draw_album_list(f: &mut Frame, area: Rect, app: &App, is_focused: bool) {
    use ratatui::widgets::StatefulWidget;

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
                    // Truncation length adjusted based on right column width and play count display
                    // For stereo (12% right column): 90 chars is safe (reduced to make room for play count)
                    // For 5.1 (20% right column): 75 chars to be safe
                    let raw_content = album.display_name();
                    let cleaned = clean_text(&raw_content);
                    let max_len = if num_channels > 4 { 75 } else { 90 };
                    let content = truncate_with_ellipsis(&cleaned, max_len);

                    // Add play count to the display if > 0
                    let display_text = if album.play_count > 0 {
                        format!("{}  \u{1F3B5} {}", content, album.play_count)
                    } else {
                        content
                    };

                    let style = if i == app.selected_album_index {
                        Style::default()
                            .fg(app.theme.fg_selected)
                            .bg(app.theme.bg_selected)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    ListItem::new(display_text).style(style)
                })
                .collect();

            let border_type = if is_focused {
                BorderType::Double
            } else {
                BorderType::Plain
            };

            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(border_type)
                        .title(format!(
                            "Albums ({}) - 'a' to add, 't' to toggle tree view",
                            albums.len()
                        )),
                )
                .highlight_style(
                    Style::default()
                        .fg(app.theme.fg_selected)
                        .bg(app.theme.bg_selected)
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
                                .fg(app.theme.accent_primary)
                                .add_modifier(Modifier::BOLD);
                            if i == app.selected_tree_index {
                                style = style.bg(app.theme.bg_highlight);
                            }
                            (content, style)
                        }
                        TreeItem::Album { index } => {
                            if let Some(album) = app.library.albums.get(*index) {
                                // Use display_name for consistency, clean and truncate
                                let raw_album = album.display_name();
                                let cleaned = clean_text(&raw_album);
                                let truncated = truncate_with_ellipsis(&cleaned, 80);

                                // Add play count if > 0
                                let content = if album.play_count > 0 {
                                    format!("  └─ {}  \u{1F3B5} {}", truncated, album.play_count)
                                } else {
                                    format!("  └─ {}", truncated)
                                };

                                let mut style = Style::default();
                                if i == app.selected_tree_index {
                                    style = Style::default()
                                        .fg(app.theme.fg_selected)
                                        .bg(app.theme.bg_selected)
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

            let border_type = if is_focused {
                BorderType::Double
            } else {
                BorderType::Plain
            };

            let list = List::new(items)
                .block(Block::default()
                    .borders(Borders::ALL)
                    .border_type(border_type)
                    .title(format!(
                        "Artists ({}) - 'h/l' to expand/collapse, 'a' to add, 't' to toggle view",
                        app.artist_tree.len()
                    )))
                .highlight_style(
                    Style::default()
                        .fg(app.theme.fg_selected)
                        .bg(app.theme.bg_selected)
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
        Style::default().fg(app.theme.title_color)
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
                        .fg(app.theme.fg_selected)
                        .bg(app.theme.accent_primary)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.theme.fg_secondary)
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

            // For all directories, add track/album count and last scan time
            // We need to find the DirectoryInfo corresponding to this path
            // Since we flattened the tree, we can't easily look it up by index in the original list
            // But we can search by path in the flattened list or just search the whole tree?
            // Actually, get_directory_tree_items returns (PathBuf, level, expanded)
            // It doesn't return the DirectoryInfo itself.
            // We should probably update get_directory_tree_items to return more info or look it up here.
            // Looking up by path in the recursive structure is expensive if we do it for every item.
            // But for a TUI it might be fine.

            // Helper to find directory info by path
            fn find_dir_info<'a>(
                directories: &'a [sotf_audio_player::DirectoryInfo],
                path: &std::path::Path,
            ) -> Option<&'a sotf_audio_player::DirectoryInfo> {
                for dir in directories {
                    if dir.path == path {
                        return Some(dir);
                    }
                    if let Some(found) = find_dir_info(&dir.subdirectories, path) {
                        return Some(found);
                    }
                }
                None
            }

            let info_str = if let Some(dir_info) = find_dir_info(&app.library.directories, path) {
                let track_count = dir_info.file_count;
                let album_count = dir_info.album_count;
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

                if *level == 0 {
                    format!(
                        " [{} tracks, {} albums, {}]",
                        track_count, album_count, last_scan
                    )
                } else {
                    format!(" [{} tracks, {} albums]", track_count, album_count)
                }
            } else {
                String::new()
            };

            let content = format!("{}{}{}{}", indent, expand_indicator, path_str, info_str);

            let style = if i == app.selected_directory_index {
                Style::default()
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.bg_selected)
                    .add_modifier(Modifier::BOLD)
            } else if *level == 0 {
                Style::default()
                    .fg(app.theme.accent_primary)
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
            "Directories - Enter/Right=expand, 'd'=remove, 's'=scan, 'R'=force scan, 'm'=maintain, 'r'=replaygain, 'a'=add".to_string()
        }
    } else {
        "Directories - Enter/Right=expand, 'd'=remove, 's'=scan, 'R'=force scan, 'm'=maintain, 'r'=replaygain, 'a'=add".to_string()
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .fg(app.theme.fg_selected)
                .bg(app.theme.bg_selected)
                .add_modifier(Modifier::BOLD),
        );

    // Use stateful widget for proper scrolling
    let mut state = ListState::default();
    state.select(Some(app.selected_directory_index));

    use ratatui::widgets::StatefulWidget;
    StatefulWidget::render(list, chunks[2], f.buffer_mut(), &mut state);
}

fn draw_queue_screen(f: &mut Frame, area: Rect, app: &mut App) {
    let is_focused = app.current_screen == Screen::Queue;

    // Check if we have album images to display
    let has_images = !app.album_images.is_empty();

    // Split the area horizontally if we have images
    let (queue_area, image_area) = if has_images {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60), // Queue list
                Constraint::Percentage(40), // Album art
            ])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

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
                .fg(app.theme.fg_selected)
                .bg(app.theme.bg_selected)
                .add_modifier(Modifier::BOLD);
        } else if is_current {
            style = style
                .fg(app.theme.playing_indicator)
                .add_modifier(Modifier::BOLD);
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
                        .fg(app.theme.current_track)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.theme.fg_muted)
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

    let border_type = if is_focused {
        BorderType::Double
    } else {
        BorderType::Plain
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(border_type)
            .title(title),
    );

    f.render_widget(list, queue_area);

    // Render album art if available
    if let Some(image_area) = image_area {
        draw_album_art(f, image_area, app);
    }
}

fn draw_album_art(f: &mut Frame, area: Rect, app: &mut App) {
    use ratatui_image::StatefulImage;

    // Create a border block
    let title = if app.album_images.is_empty() {
        "Album Art (none)".to_string()
    } else if app.album_images.len() > 1 {
        format!(
            "Album Art ({}/{}) - [] to cycle",
            app.selected_image_index + 1,
            app.album_images.len()
        )
    } else {
        "Album Art".to_string()
    };

    let block = Block::default().borders(Borders::ALL).title(title);
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    // Split area: image on top, ReplayGain info at bottom
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),   // Image area (takes most space)
            Constraint::Length(5), // ReplayGain info (fixed 5 lines)
        ])
        .split(inner_area);

    let image_area = chunks[0];
    let info_area = chunks[1];

    // Render the image if available
    // Clone the path to avoid borrow conflicts
    if let Some(image_path) = app.get_current_album_image().cloned() {
        if let Some(picker) = &mut app.image_picker {
            // Try to load and render the image
            if let Ok(img) = image::open(&image_path) {
                // Create protocol with the picker - new_resize_protocol returns StatefulProtocol
                let mut protocol = picker.new_resize_protocol(img);
                // Render using stateful widget
                let image = StatefulImage::new(None);
                f.render_stateful_widget(image, image_area, &mut protocol);
            } else {
                // Fallback if image loading fails
                let error_text = Paragraph::new("Failed to load image")
                    .style(Style::default().fg(ratatui::style::Color::Red));
                f.render_widget(error_text, image_area);
            }
        }
    } else {
        // No image available
        let no_image_text = Paragraph::new("No album art found");
        f.render_widget(no_image_text, image_area);
    }

    // Render ReplayGain info below the image
    draw_replay_gain_info(f, info_area, app);
}

fn draw_replay_gain_info(f: &mut Frame, area: Rect, app: &App) {
    // Get currently playing track
    let track_info = if let Some(queue_index) = app.current_queue_index {
        if let Some(queue_item) = app.queue.get(queue_index) {
            if let Some(track) = queue_item.album.tracks.get(queue_item.current_track_index) {
                Some((track, &queue_item.album))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let mut lines = Vec::new();

    if let Some((track, _album)) = track_info {
        // Track title
        if let Some(title) = &track.title {
            lines.push(Line::from(vec![
                Span::styled("Track: ", Style::default().fg(app.theme.title_color)),
                Span::raw(title),
            ]));
        }

        // Track ReplayGain
        if let Some(gain) = track.replay_gain {
            let peak_str = if let Some(peak) = track.replay_peak {
                format!(" (peak: {:.3})", peak)
            } else {
                String::new()
            };
            lines.push(Line::from(vec![
                Span::styled("Track RG: ", Style::default().fg(app.theme.title_color)),
                Span::styled(
                    format!("{:+.2} dB{}", gain, peak_str),
                    Style::default().fg(app.theme.accent_success),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("Track RG: ", Style::default().fg(app.theme.title_color)),
                Span::styled("not available", Style::default().fg(app.theme.fg_muted)),
            ]));
        }

        // Album ReplayGain
        if let Some(gain) = track.album_gain {
            let peak_str = if let Some(peak) = track.album_peak {
                format!(" (peak: {:.3})", peak)
            } else {
                String::new()
            };
            lines.push(Line::from(vec![
                Span::styled("Album RG: ", Style::default().fg(app.theme.title_color)),
                Span::styled(
                    format!("{:+.2} dB{}", gain, peak_str),
                    Style::default().fg(app.theme.accent_success),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("Album RG: ", Style::default().fg(app.theme.title_color)),
                Span::styled("not available", Style::default().fg(app.theme.fg_muted)),
            ]));
        }
    } else {
        lines.push(Line::from("No track playing"));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
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
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.bg_selected)
                    .add_modifier(Modifier::BOLD)
            } else if plugin.enabled {
                Style::default().fg(app.theme.accent_success)
            } else {
                Style::default().fg(app.theme.fg_muted)
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

fn draw_available_plugins(f: &mut Frame, area: Rect, app: &App) {
    let plugins = PluginType::all();
    let items: Vec<ListItem> = plugins
        .iter()
        .map(|plugin_type| {
            let content = format!("{}\n  {}", plugin_type.name(), plugin_type.description());
            ListItem::new(content).style(Style::default().fg(app.theme.accent_primary))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Available Plugins"),
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
        .style(Style::default().fg(app.theme.title_color))
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
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.bg_selected)
                    .add_modifier(Modifier::BOLD)
            } else if device.is_default {
                Style::default().fg(app.theme.accent_success)
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

fn draw_meters_column(f: &mut Frame, area: Rect, app: &mut App) {
    // Split the right column - LUFS, level meter, volume
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(15), // LUFS box (compact: 12 content lines + 2 borders + 1 padding)
            Constraint::Min(0),     // Level meter box (expandable)
            Constraint::Length(3),  // Volume box
        ])
        .split(area);

    // Draw LUFS info box
    draw_lufs_box(f, chunks[0], app);

    // Draw level meter box
    draw_level_meter_box(f, chunks[1], app);

    // Draw volume box
    draw_volume_box(f, chunks[2], app);
}

fn draw_loudness_and_volume_column(f: &mut Frame, area: Rect, app: &mut App) {
    // Split into loudness and volume (no level meters - they're in a separate column)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(15), // LUFS box (compact)
            Constraint::Min(0),     // Spacer (expandable)
            Constraint::Length(3),  // Volume box
        ])
        .split(area);

    // Draw LUFS info box
    draw_lufs_box(f, chunks[0], app);

    // Draw volume box
    draw_volume_box(f, chunks[2], app);
}

fn draw_lufs_box(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default().borders(Borders::ALL).title("Loudness");
    f.render_widget(block, area);

    // Inner area for content (excluding borders)
    let inner = area.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 1,
    });

    if inner.height < 3 {
        // Not enough space
        return;
    }

    if let Some(ref loudness) = app.loudness_info {
        let mut y_offset = 0;

        // ============================================================================
        // True Peak Section
        // ============================================================================

        if !loudness.true_peaks_dbtp.is_empty() && y_offset < inner.height {
            // Find max true peak for display
            let max_true_peak = loudness
                .true_peaks_dbtp
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);

            // Header: "True Peak      [XX.X]"
            let true_peak_label = if max_true_peak.is_finite() {
                format!("True Peak      [{:>4.1}]", max_true_peak)
            } else {
                "True Peak        [-∞]".to_string()
            };
            f.render_widget(
                Paragraph::new(true_peak_label).style(Style::default().fg(app.theme.title_color)),
                Rect {
                    x: inner.x,
                    y: inner.y + y_offset,
                    width: inner.width,
                    height: 1,
                },
            );
            y_offset += 1;

            // Render true peak bars for each channel (max 2 bars to save space)
            let num_peak_bars = loudness.true_peaks_dbtp.len().min(2);
            for ch_idx in 0..num_peak_bars {
                if y_offset >= inner.height {
                    break;
                }

                let true_peak_dbtp = loudness
                    .true_peaks_dbtp
                    .get(ch_idx)
                    .copied()
                    .unwrap_or(f64::NEG_INFINITY);

                // Map -60 dBTP to 0%, +6 dBTP to 100%
                let ratio = if true_peak_dbtp.is_finite() {
                    ((true_peak_dbtp + 60.0) / 66.0).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                // Choose color based on level: green → orange → red when >0
                let gauge_style = if true_peak_dbtp > 0.0 {
                    Style::default().fg(app.theme.accent_error) // Red - clipping
                } else if true_peak_dbtp > -1.0 {
                    Style::default().fg(app.theme.accent_warning) // Orange - near clipping
                } else {
                    Style::default().fg(app.theme.accent_success) // Green - safe
                };

                // Format label showing the dBTP value
                let label = if true_peak_dbtp.is_finite() {
                    format!("{:>5.1}", true_peak_dbtp)
                } else {
                    "  -∞".to_string()
                };

                use ratatui::widgets::Gauge;
                let gauge = Gauge::default()
                    .ratio(ratio)
                    .label(label)
                    .gauge_style(gauge_style)
                    .use_unicode(true);

                f.render_widget(
                    gauge,
                    Rect {
                        x: inner.x,
                        y: inner.y + y_offset,
                        width: inner.width,
                        height: 1,
                    },
                );
                y_offset += 1;
            }

            // Scale labels: "-60" at left, "0" at 60/66 position, "+6" at right
            if y_offset < inner.height {
                let width = inner.width as usize;
                // True peak scale: -60 dBTP to +6 dBTP (total range 66 dB)
                // Position of 0 dBTP: 60/66 ≈ 0.909
                let zero_pos = ((60.0 / 66.0) * width as f64) as usize;
                let max_pos = width.saturating_sub(2); // "+6" is 2 chars

                let mut scale = String::with_capacity(width);
                scale.push_str("-60");

                // Add spaces until zero position (accounting for "-60" = 3 chars)
                let spaces_before_zero = zero_pos.saturating_sub(3 + 1); // -1 for the "0" char
                if spaces_before_zero > 0 {
                    scale.push_str(&" ".repeat(spaces_before_zero));
                }
                scale.push('0');

                // Add spaces until "+6" position
                let current_len = scale.len();
                if max_pos > current_len {
                    scale.push_str(&" ".repeat(max_pos - current_len));
                }
                scale.push_str("+6");

                f.render_widget(
                    Paragraph::new(scale).style(Style::default().fg(app.theme.fg_muted)),
                    Rect {
                        x: inner.x,
                        y: inner.y + y_offset,
                        width: inner.width,
                        height: 1,
                    },
                );
                y_offset += 1;
            }
        }

        // ============================================================================
        // LUFS Section
        // ============================================================================

        if y_offset < inner.height {
            f.render_widget(
                Paragraph::new("LUFS").style(Style::default().fg(app.theme.title_color)),
                Rect {
                    x: inner.x,
                    y: inner.y + y_offset,
                    width: inner.width,
                    height: 1,
                },
            );
            y_offset += 1;
        }

        // Helper function to draw LUFS bar using Gauge widget
        let draw_lufs_bar = |f: &mut Frame, y: u16, label_char: &str, lufs: f64| {
            // Map -60 to 0 LUFS as 0% to 100%
            let ratio = if lufs.is_finite() {
                ((lufs + 60.0) / 60.0).clamp(0.0, 1.0)
            } else {
                0.0
            };

            // Choose color: green → orange → red based on level
            let gauge_style = if lufs > -1.0 {
                Style::default().fg(app.theme.accent_error) // Red - very loud
            } else if lufs > -10.0 {
                Style::default().fg(app.theme.accent_warning) // Orange - loud
            } else {
                Style::default().fg(app.theme.accent_success) // Green - normal
            };

            // Format label: "M -15.0"
            let value_str = if lufs.is_finite() {
                format!("{:>5.1}", lufs)
            } else {
                "  -∞".to_string()
            };
            let label = format!("{} {}", label_char, value_str);

            use ratatui::widgets::Gauge;
            let gauge = Gauge::default()
                .ratio(ratio)
                .label(label)
                .gauge_style(gauge_style)
                .use_unicode(true);

            f.render_widget(
                gauge,
                Rect {
                    x: inner.x,
                    y,
                    width: inner.width,
                    height: 1,
                },
            );
        };

        // M (Momentary)
        if y_offset < inner.height {
            draw_lufs_bar(f, inner.y + y_offset, "M", loudness.momentary_lufs);
            y_offset += 1;
        }

        // S (Short-term)
        if y_offset < inner.height {
            draw_lufs_bar(f, inner.y + y_offset, "S", loudness.shortterm_lufs);
            y_offset += 1;
        }

        // I (Integrated)
        if y_offset < inner.height {
            draw_lufs_bar(f, inner.y + y_offset, "I", loudness.integrated_lufs);
            y_offset += 1;
        }

        // Scale labels: "-60" at left, "0" at right
        if y_offset < inner.height {
            let width = inner.width as usize;
            let mut scale = String::with_capacity(width);
            scale.push_str("-60");

            // Add spaces until "0" at the right edge (0 is 1 char)
            let spaces = width.saturating_sub(4); // 3 for "-60", 1 for "0"
            scale.push_str(&" ".repeat(spaces));
            scale.push('0');

            f.render_widget(
                Paragraph::new(scale).style(Style::default().fg(app.theme.fg_muted)),
                Rect {
                    x: inner.x,
                    y: inner.y + y_offset,
                    width: inner.width,
                    height: 1,
                },
            );
            y_offset += 1;
        }

        // ============================================================================
        // Stereo Width Section (only for stereo)
        // ============================================================================

        if let Some(correlation) = loudness.correlation_lr {
            if y_offset < inner.height {
                f.render_widget(
                    Paragraph::new("Stereo width")
                        .style(Style::default().fg(app.theme.title_color)),
                    Rect {
                        x: inner.x,
                        y: inner.y + y_offset,
                        width: inner.width,
                        height: 1,
                    },
                );
                y_offset += 1;
            }

            if y_offset < inner.height {
                use ratatui::widgets::Gauge;

                // Correlation is typically between 0 and 1 for normal stereo content
                // 0 = uncorrelated (wide stereo), 1 = fully correlated (mono)
                // For "Stereo width" display, invert it so higher = wider
                let stereo_width = (1.0 - correlation).clamp(0.0, 1.0);
                let ratio = stereo_width;

                // Choose color based on stereo width
                let gauge_style = if stereo_width < 0.1 {
                    Style::default().fg(app.theme.accent_warning) // Too narrow (nearly mono)
                } else {
                    Style::default().fg(app.theme.accent_success) // Good stereo separation
                };

                let label = format!("{:>4.2}", stereo_width);

                let gauge = Gauge::default()
                    .ratio(ratio)
                    .label(label)
                    .gauge_style(gauge_style)
                    .use_unicode(true);

                f.render_widget(
                    gauge,
                    Rect {
                        x: inner.x,
                        y: inner.y + y_offset,
                        width: inner.width,
                        height: 1,
                    },
                );
                y_offset += 1;
            }

            // Scale labels: "0" at left, "1" at right
            if y_offset < inner.height {
                let width = inner.width as usize;
                let mut scale = String::with_capacity(width);
                scale.push('0');

                // Add spaces until "1" at the right edge
                let spaces = width.saturating_sub(2); // 1 for "0", 1 for "1"
                scale.push_str(&" ".repeat(spaces));
                scale.push('1');

                f.render_widget(
                    Paragraph::new(scale).style(Style::default().fg(app.theme.fg_muted)),
                    Rect {
                        x: inner.x,
                        y: inner.y + y_offset,
                        width: inner.width,
                        height: 1,
                    },
                );
            }
        }
    } else {
        // No loudness data
        f.render_widget(
            Paragraph::new("No audio playing")
                .style(Style::default().fg(app.theme.fg_muted))
                .alignment(Alignment::Center),
            Rect {
                x: inner.x,
                y: inner.y + inner.height / 2,
                width: inner.width,
                height: 1,
            },
        );
    }
}

fn draw_level_meter_box(f: &mut Frame, area: Rect, app: &mut App) {
    // Check for loudness info first
    let has_loudness = app.loudness_info.is_some();
    if !has_loudness {
        let paragraph = Paragraph::new("No audio")
            .block(Block::default().borders(Borders::ALL).title("Levels"))
            .alignment(Alignment::Center);
        f.render_widget(paragraph, area);
        return;
    }

    let num_channels = app
        .loudness_info
        .as_ref()
        .map(|l| l.channel_peaks.len())
        .unwrap_or(0);
    if num_channels == 0 {
        let paragraph = Paragraph::new("No channels")
            .block(Block::default().borders(Borders::ALL).title("Levels"));
        f.render_widget(paragraph, area);
        return;
    }

    // Update channel groups if not initialized or if channel count changed
    // Do this BEFORE borrowing loudness immutably
    if app.level_meter_groups.is_empty()
        || app
            .level_meter_groups
            .iter()
            .flat_map(|g| &g.channels)
            .count()
            != num_channels
    {
        app.update_level_meter_groups();
    }

    // Now borrow loudness immutably for the rest of the function
    let loudness = app.loudness_info.as_ref().unwrap();

    // Draw border with simple title
    let title_lines = vec![Line::from("Levels (help: ?)")];
    let title_height = 1;

    // Highlight border when focused
    let block = if app.focused_pane == FocusedPane::Meters {
        Block::default().borders(Borders::ALL).border_style(
            Style::default()
                .fg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Block::default().borders(Borders::ALL)
    };
    f.render_widget(block, area);

    // Render title lines at the top inside the border
    for (i, line) in title_lines.iter().enumerate() {
        f.render_widget(
            Paragraph::new(line.clone()),
            Rect {
                x: area.x + 1,
                y: area.y + 1 + i as u16,
                width: area.width.saturating_sub(2),
                height: 1,
            },
        );
    }

    // Create inner area for meters (after title lines and borders)
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1 + title_height,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2 + title_height),
    };

    // Calculate dimensions
    let max_name_lines = app
        .level_meter_groups
        .iter()
        .flat_map(|g| &g.channels)
        .map(|ch| ch.display_name.len())
        .max()
        .unwrap_or(1);

    // Reserve space for label/names and M/S/D controls (3 lines)
    // For stereo: 1 line for "L - R" label + 3 lines for controls
    // For multi-channel: max_name_lines + 3 lines for controls
    let meter_height = (inner.height as usize).saturating_sub(max_name_lines + 3);
    if meter_height == 0 {
        return;
    }

    // Reserve space for vertical scale legend on the left (4 chars: "-60 ")
    let scale_width = 4usize;
    let mut x_offset = scale_width; // Start after the scale
    let available_width = inner.width as usize;

    // Draw vertical scale legend on the left
    // Non-linear scale: -60 dB (0%), -40 dB (20%), -20 dB (50%), 0 dB (100%)
    let scale_markers = [
        (1.0, " 0 "), // 100% fill -> top
        (0.5, "-20"), // 50% fill
        (0.2, "-40"), // 20% fill
        (0.0, "-60"), // 0% fill -> bottom
    ];

    for (ratio, label) in scale_markers.iter() {
        // Convert fill ratio to row index from bottom
        let row_idx = (ratio * meter_height as f64).round() as usize;
        // Convert to y coordinate (inverted: top of screen = higher rows)
        let y = inner.y + (meter_height - 1).saturating_sub(row_idx.min(meter_height - 1)) as u16;

        f.render_widget(
            Paragraph::new(*label).style(Style::default().fg(app.theme.fg_muted)),
            Rect {
                x: inner.x,
                y,
                width: scale_width as u16,
                height: 1,
            },
        );
    }

    // Draw each group
    for (group_idx, group) in app.level_meter_groups.iter().enumerate() {
        let is_selected = group_idx == app.selected_level_meter_group;

        // Calculate width for this group
        let num_channels = group.channels.len();
        let is_stereo = num_channels == 2;
        let group_width = if is_stereo {
            8 // 3 + 2 + 3 for stereo
        } else {
            num_channels.max(3)
        };

        if is_stereo {
            // Special stereo rendering: 3-char wide meters with 2-char spacing
            // Center the group in available space
            let group_start_x = if available_width > group_width + x_offset {
                x_offset + (available_width - x_offset - group_width) / 2
            } else {
                x_offset
            };

            // Skip rendering if there's not enough space
            if group_start_x + group_width > available_width {
                continue;
            }

            // Draw L meter (3 chars wide)
            let l_channel = &group.channels[0];
            let l_peak = loudness
                .channel_peaks
                .get(l_channel.index)
                .copied()
                .unwrap_or(0.0);
            let l_peak_db = 20.0 * l_peak.max(0.0001).log10();

            // Linear dB scale: -60 dB to 0 dB
            let l_fill_ratio = ((l_peak_db + 60.0) / 60.0).clamp(0.0, 1.0);
            let l_filled_rows = (l_fill_ratio * meter_height as f64).round() as usize;

            // Draw R meter (3 chars wide)
            let r_channel = &group.channels[1];
            let r_peak = loudness
                .channel_peaks
                .get(r_channel.index)
                .copied()
                .unwrap_or(0.0);
            let r_peak_db = 20.0 * r_peak.max(0.0001).log10();

            // Linear dB scale: -60 dB to 0 dB
            let r_fill_ratio = ((r_peak_db + 60.0) / 60.0).clamp(0.0, 1.0);
            let r_filled_rows = (r_fill_ratio * meter_height as f64).round() as usize;

            // Draw both meters
            for row_idx in (0..meter_height).rev() {
                let y = inner.y + (meter_height - 1 - row_idx) as u16;
                let level_ratio = row_idx as f64 / meter_height as f64;
                let color = if level_ratio > 0.95 {
                    app.theme.accent_error
                } else if level_ratio > 0.90 {
                    app.theme.accent_warning
                } else {
                    app.theme.accent_success
                };

                // L meter (3 chars)
                let l_is_filled = row_idx < l_filled_rows;
                let l_bar = if l_is_filled {
                    "███"
                } else {
                    "░░░"
                };
                let l_style = if l_is_filled {
                    Style::default().fg(color)
                } else {
                    Style::default().fg(app.theme.fg_muted)
                };
                f.render_widget(
                    Paragraph::new(l_bar).style(l_style),
                    Rect {
                        x: inner.x + group_start_x as u16,
                        y,
                        width: 3,
                        height: 1,
                    },
                );

                // R meter (3 chars) - skip 2 chars for spacing
                let r_is_filled = row_idx < r_filled_rows;
                let r_bar = if r_is_filled {
                    "███"
                } else {
                    "░░░"
                };
                let r_style = if r_is_filled {
                    Style::default().fg(color)
                } else {
                    Style::default().fg(app.theme.fg_muted)
                };
                f.render_widget(
                    Paragraph::new(r_bar).style(r_style),
                    Rect {
                        x: inner.x + group_start_x as u16 + 5, // 3 (L) + 2 (spacing)
                        y,
                        width: 3,
                        height: 1,
                    },
                );
            }

            // Draw "L - R" label centered below meters
            let name_start_y = inner.y + meter_height as u16;
            let label = "L - R";
            let label_x = group_start_x + (group_width - label.len()) / 2;
            f.render_widget(
                Paragraph::new(label).style(Style::default().fg(app.theme.fg_primary)),
                Rect {
                    x: inner.x + label_x as u16,
                    y: name_start_y,
                    width: label.len() as u16,
                    height: 1,
                },
            );
        } else {
            // Original rendering for non-stereo: 1 char wide meters
            for (ch_idx, channel) in group.channels.iter().enumerate() {
                let ch_x_offset = x_offset + ch_idx;
                if ch_x_offset >= available_width {
                    break;
                }

                // Get the peak level for this channel
                let peak = loudness
                    .channel_peaks
                    .get(channel.index)
                    .copied()
                    .unwrap_or(0.0);
                let peak_db = 20.0 * peak.max(0.0001).log10();

                // Linear dB scale: -60 dB to 0 dB
                let fill_ratio = ((peak_db + 60.0) / 60.0).clamp(0.0, 1.0);
                let filled_rows = (fill_ratio * meter_height as f64).round() as usize;

                // Draw vertical meter (1 char wide)
                let meter_x = inner.x + ch_x_offset as u16;
                for row_idx in (0..meter_height).rev() {
                    let y = inner.y + (meter_height - 1 - row_idx) as u16;
                    let is_filled = row_idx < filled_rows;

                    let level_ratio = row_idx as f64 / meter_height as f64;
                    let color = if level_ratio > 0.95 {
                        app.theme.accent_error
                    } else if level_ratio > 0.90 {
                        app.theme.accent_warning
                    } else {
                        app.theme.accent_success
                    };

                    let bar = if is_filled { "█" } else { "░" };
                    let style = if is_filled {
                        Style::default().fg(color)
                    } else {
                        Style::default().fg(app.theme.fg_muted)
                    };

                    f.render_widget(
                        Paragraph::new(bar).style(style),
                        Rect {
                            x: meter_x,
                            y,
                            width: 1,
                            height: 1,
                        },
                    );
                }

                // Draw vertical channel name below meter
                let name_start_y = inner.y + meter_height as u16;
                for (line_idx, line) in channel.display_name.iter().enumerate() {
                    let y = name_start_y + line_idx as u16;
                    if y < inner.y + inner.height - 2 {
                        // -2 for M/S controls
                        f.render_widget(
                            Paragraph::new(line.as_str())
                                .style(Style::default().fg(app.theme.fg_primary)),
                            Rect {
                                x: meter_x,
                                y,
                                width: 1,
                                height: 1,
                            },
                        );
                    }
                }
            }
        }

        // Draw M/S/D controls for this group
        // Show controls if there's space, or always for stereo (centered layout)
        let show_controls =
            is_stereo || (app.level_meter_groups.len() > 1 && x_offset + 3 <= available_width);
        if show_controls {
            // For stereo, center controls under the meters
            let controls_x = if is_stereo {
                let group_start_x = if available_width > group_width + x_offset {
                    x_offset + (available_width - x_offset - group_width) / 2
                } else {
                    x_offset
                };
                // Bounds check - ensure controls fit
                let ctrl_offset = group_start_x + (group_width - 3) / 2;
                if ctrl_offset + 3 > available_width {
                    x_offset += group_width + 1;
                    continue;
                }
                inner.x + ctrl_offset as u16
            } else {
                if x_offset + 3 > available_width {
                    x_offset += group_width + 1;
                    continue;
                }
                inner.x + x_offset as u16
            };

            // Position controls below the label/channel names
            let controls_y = inner.y + meter_height as u16 + max_name_lines as u16;

            // Mute button
            let mute_style = if is_selected && app.level_meter_control_selection == 0 {
                Style::default()
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.bg_selected)
                    .add_modifier(Modifier::BOLD)
            } else if group.muted {
                Style::default().fg(app.theme.accent_error)
            } else {
                Style::default().fg(app.theme.fg_muted)
            };

            f.render_widget(
                Paragraph::new("[M]").style(mute_style),
                Rect {
                    x: controls_x,
                    y: controls_y,
                    width: 3,
                    height: 1,
                },
            );

            // Solo button
            let solo_style = if is_selected && app.level_meter_control_selection == 1 {
                Style::default()
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.bg_selected)
                    .add_modifier(Modifier::BOLD)
            } else if group.soloed {
                Style::default().fg(app.theme.accent_warning)
            } else {
                Style::default().fg(app.theme.fg_muted)
            };

            f.render_widget(
                Paragraph::new("[S]").style(solo_style),
                Rect {
                    x: controls_x,
                    y: controls_y + 1,
                    width: 3,
                    height: 1,
                },
            );

            // Dim button
            let dim_style = if is_selected && app.level_meter_control_selection == 2 {
                Style::default()
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.bg_selected)
                    .add_modifier(Modifier::BOLD)
            } else if group.dimmed {
                Style::default().fg(app.theme.accent_info)
            } else {
                Style::default().fg(app.theme.fg_muted)
            };

            f.render_widget(
                Paragraph::new("[D]").style(dim_style),
                Rect {
                    x: controls_x,
                    y: controls_y + 2,
                    width: 3,
                    height: 1,
                },
            );
        }

        // Advance by group width + 1 space between groups
        x_offset += group_width + 1;
    }
}

fn draw_volume_box(f: &mut Frame, area: Rect, app: &App) {
    let volume_pct = (app.volume * 100.0) as u32;
    let text = Line::from(vec![Span::styled(
        format!("{}%", volume_pct),
        Style::default()
            .fg(app.theme.accent_primary)
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
                    .fg(app.theme.title_color)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }

    if let Some(idx) = app.current_queue_index
        && let Some(item) = app.queue.get(idx)
        && let Some(track) = item.current_track()
    {
        let raw_track_name = track
            .title
            .as_deref()
            .unwrap_or_else(|| track.path.file_name().unwrap().to_str().unwrap());
        let cleaned_track_name = clean_track_name(raw_track_name);
        // Truncate to max 50 chars for status bar to leave room for other info
        let track_name = truncate_with_ellipsis(&cleaned_track_name, 50);
        status_spans.push(Span::styled(
            format!("Now: {}", track_name),
            Style::default().fg(app.theme.playing_indicator),
        ));
        status_spans.push(Span::raw(" | "));
    }

    if !app.plugin_chain.is_empty() {
        let plugin_status = if app.plugin_update_in_progress {
            format!("Plugins: {} [updating...] ", app.plugin_chain.len())
        } else {
            format!("Plugins: {} ", app.plugin_chain.len())
        };

        let plugin_color = if app.plugin_update_in_progress {
            app.theme.accent_warning
        } else {
            app.theme.accent_secondary
        };

        status_spans.push(Span::styled(
            plugin_status,
            Style::default().fg(plugin_color),
        ));
        status_spans.push(Span::raw("| "));
    }

    status_spans.push(Span::raw("Keys: "));
    status_spans.push(Span::styled(
        "TAB",
        Style::default().fg(app.theme.accent_primary),
    ));
    status_spans.push(Span::raw("=Next "));
    status_spans.push(Span::styled(
        "L",
        Style::default().fg(app.theme.accent_primary),
    ));
    status_spans.push(Span::raw("/"));
    status_spans.push(Span::styled(
        "D",
        Style::default().fg(app.theme.accent_primary),
    ));
    status_spans.push(Span::raw("/"));
    status_spans.push(Span::styled(
        "Q",
        Style::default().fg(app.theme.accent_primary),
    ));
    status_spans.push(Span::raw("/"));
    status_spans.push(Span::styled(
        "P",
        Style::default().fg(app.theme.accent_primary),
    ));
    status_spans.push(Span::raw("/"));
    status_spans.push(Span::styled(
        "O",
        Style::default().fg(app.theme.accent_primary),
    ));
    status_spans.push(Span::raw("=Screens "));
    status_spans.push(Span::styled(
        "+=/-_",
        Style::default().fg(app.theme.title_color),
    ));
    status_spans.push(Span::raw("=Volume "));
    status_spans.push(Span::styled(
        "ESC/%-Q",
        Style::default().fg(app.theme.accent_error),
    ));
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
            .style(Style::default().bg(app.theme.bg_primary))
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
            Span::styled("↑/↓", Style::default().fg(app.theme.accent_primary)),
            Span::styled(" to select parameter, ", Style::default()),
            Span::styled("←/→", Style::default().fg(app.theme.accent_primary)),
            Span::styled(" to adjust value", Style::default()),
        ]));
        lines.push(Line::from(""));

        let params = get_plugin_parameters(&plugin.settings, app.plugin_param_selection);
        for (i, param) in params.iter().enumerate() {
            let style = if i == app.plugin_param_selection {
                Style::default()
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.bg_selected)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            lines.push(Line::from(vec![
                Span::styled(format!("  {}: ", param.0), style),
                Span::styled(param.1.clone(), style.fg(app.theme.title_color)),
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
            enable_subharmonic_synth,
            subharmonic_gain,
            enable_hr_direct,
            hr_sharpen,
            safety_cap_db,
            decorrelation_mode,
            ..
        } => vec![
            ("Speaker Config".to_string(), speaker_config.clone()),
            (
                "Front Direct Gain".to_string(),
                format!("{:.2}x", gain_front_direct),
            ),
            (
                "Front Ambient Gain".to_string(),
                format!("{:.2}x", gain_front_ambient),
            ),
            (
                "Rear Ambient Gain".to_string(),
                format!("{:.2}x", gain_rear_ambient),
            ),
            ("LFE Cutoff".to_string(), format!("{:.0} Hz", lfe_cutoff_hz)),
            ("Stereo Width".to_string(), format!("{:.2}", stereo_width)),
            ("Bandpass".to_string(), format!("{:.0} Hz", bandpass_hz)),
            ("Height Gain".to_string(), format!("{:.2}x", height_gain)),
            ("LFE Gain".to_string(), format!("{:.2}x", lfe_gain)),
            (
                "Subharmonic Synth".to_string(),
                if *enable_subharmonic_synth {
                    "On".to_string()
                } else {
                    "Off".to_string()
                },
            ),
            (
                "Subharmonic Gain".to_string(),
                format!("{:.2}x", subharmonic_gain),
            ),
            (
                "HR Direct".to_string(),
                if *enable_hr_direct { "On" } else { "Off" }.to_string(),
            ),
            ("HR Sharpen".to_string(), format!("{:.2}", hr_sharpen)),
            ("Safety Cap".to_string(), format!("{:.1} dB", safety_cap_db)),
            (
                "Decorrelation".to_string(),
                match decorrelation_mode {
                    0 => "Velvet Noise".to_string(),
                    1 => "LFO Phase".to_string(),
                    _ => "Unknown".to_string(),
                },
            ),
        ],
        PluginSettings::Compressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            makeup_gain_db,
            mix,
            auto_makeup,
            link_channels,
            sidechain_hpf_hz,
        } => vec![
            ("Threshold".to_string(), format!("{:.1} dB", threshold_db)),
            ("Ratio".to_string(), format!("{:.1}:1", ratio)),
            ("Attack".to_string(), format!("{:.1} ms", attack_ms)),
            ("Release".to_string(), format!("{:.1} ms", release_ms)),
            ("Knee".to_string(), format!("{:.1} dB", knee_db)),
            (
                "Makeup Gain".to_string(),
                format!("{:.1} dB", makeup_gain_db),
            ),
            ("Mix".to_string(), format!("{:.2}", mix)),
            (
                "Auto Makeup".to_string(),
                if *auto_makeup { "On" } else { "Off" }.to_string(),
            ),
            (
                "Link Channels".to_string(),
                if *link_channels {
                    "Linked".to_string()
                } else {
                    "Unlinked".to_string()
                },
            ),
            (
                "Sidechain HPF".to_string(),
                format!("{:.0} Hz", sidechain_hpf_hz),
            ),
        ],
        PluginSettings::Limiter {
            threshold_db,
            release_ms,
            mix,
        } => vec![
            ("Threshold".to_string(), format!("{:.1} dB", threshold_db)),
            ("Release".to_string(), format!("{:.1} ms", release_ms)),
            ("Mix".to_string(), format!("{:.2}", mix)),
        ],
        PluginSettings::Gate {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            mix,
            link_channels,
            sidechain_hpf_hz,
        } => vec![
            ("Threshold".to_string(), format!("{:.1} dB", threshold_db)),
            ("Ratio".to_string(), format!("{:.1}:1", ratio)),
            ("Attack".to_string(), format!("{:.1} ms", attack_ms)),
            ("Release".to_string(), format!("{:.1} ms", release_ms)),
            ("Mix".to_string(), format!("{:.2}", mix)),
            (
                "Link Channels".to_string(),
                if *link_channels {
                    "Linked".to_string()
                } else {
                    "Unlinked".to_string()
                },
            ),
            (
                "Sidechain HPF".to_string(),
                if *sidechain_hpf_hz <= 0.0 {
                    "Off".to_string()
                } else {
                    format!("{:.0} Hz", sidechain_hpf_hz)
                },
            ),
        ],
        PluginSettings::LoudnessCompensation {
            low_freq,
            low_gain,
            high_freq,
            high_gain,
        } => vec![
            ("Low Freq".to_string(), format!("{:.0} Hz", low_freq)),
            ("Low Gain".to_string(), format!("{:.1} dB", low_gain)),
            ("High Freq".to_string(), format!("{:.0} Hz", high_freq)),
            ("High Gain".to_string(), format!("{:.1} dB", high_gain)),
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
        PluginSettings::Gain { gain_db } => {
            vec![("Gain".to_string(), format!("{:.1} dB", gain_db))]
        }
        PluginSettings::BinauralDecoder {
            sofa_file,
            input_channels,
            enable_optimization,
            externalization,
            near_field_strength,
        } => vec![
            (
                "SOFA File".to_string(),
                if sofa_file.is_empty() {
                    "[not set - press 'f' to load]".to_string()
                } else {
                    sofa_file.clone()
                },
            ),
            ("Input Channels".to_string(), format!("{}", input_channels)),
            (
                "Optimization".to_string(),
                if *enable_optimization {
                    "On".to_string()
                } else {
                    "Off".to_string()
                },
            ),
            (
                "Externalization".to_string(),
                format!("{:.2}", externalization),
            ),
            (
                "Near-Field".to_string(),
                format!("{:.2}", near_field_strength),
            ),
        ],
        PluginSettings::Convolution {
            ir_file,
            mix,
            gain_db,
        } => vec![
            (
                "IR File".to_string(),
                if ir_file.is_empty() {
                    "[not set]".to_string()
                } else {
                    ir_file.clone()
                },
            ),
            ("Mix (Dry/Wet)".to_string(), format!("{:.2}", mix)),
            ("Gain".to_string(), format!("{:.1} dB", gain_db)),
        ],
        PluginSettings::LoudnessMonitor => vec![(
            "Info".to_string(),
            "Real-time LUFS and peak meters".to_string(),
        )],
        PluginSettings::SpectrumAnalyzer {
            num_bins,
            min_freq,
            max_freq,
            smoothing,
        } => vec![
            ("Frequency Bins".to_string(), format!("{}", num_bins)),
            ("Min Frequency".to_string(), format!("{:.0} Hz", min_freq)),
            ("Max Frequency".to_string(), format!("{:.0} Hz", max_freq)),
            ("Smoothing".to_string(), format!("{:.2}", smoothing)),
        ],
        PluginSettings::ChannelMuteSolo {
            enabled,
            channel_states,
        } => {
            // Show basic info about mute/solo state
            let muted_count = channel_states.iter().filter(|s| s.muted).count();
            let soloed_count = channel_states.iter().filter(|s| s.soloed).count();
            vec![
                (
                    "Status".to_string(),
                    if *enabled {
                        "Active".to_string()
                    } else {
                        "Bypassed".to_string()
                    },
                ),
                (
                    "Total Channels".to_string(),
                    format!("{}", channel_states.len()),
                ),
                ("Muted Channels".to_string(), format!("{}", muted_count)),
                ("Soloed Channels".to_string(), format!("{}", soloed_count)),
            ]
        }
    }
}

fn draw_save_plugins_dialog(f: &mut Frame, app: &App) {
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
        .style(Style::default().bg(app.theme.bg_primary))
        .title("Save Plugin Preset");

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
            Line::from("Enter preset name (without .json extension):"),
            Line::from(vec![
                Span::styled("  Saved to: ", Style::default().fg(app.theme.fg_muted)),
                Span::styled(
                    "plugin_presets/",
                    Style::default()
                        .fg(app.theme.fg_muted)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("> ", Style::default().fg(app.theme.accent_primary)),
                Span::raw(&app.plugin_file_input),
                Span::styled("_", Style::default().fg(app.theme.accent_success)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Note: ", Style::default().fg(app.theme.title_color)),
                Span::raw(".json extension will be added automatically"),
            ]),
            Line::from("Press Enter to save, ESC to cancel"),
        ];

        let paragraph = Paragraph::new(lines)
            .block(Block::default())
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, inner);
    } else if app.available_plugin_presets.is_empty() {
        // No presets available - show instructions
        let lines = vec![
            Line::from("No existing presets found in plugin_presets directory"),
            Line::from(""),
            Line::from("Type a preset name to save"),
            Line::from(""),
            Line::from(vec![
                Span::styled("Note: ", Style::default().fg(app.theme.title_color)),
                Span::raw(".json extension will be added automatically"),
            ]),
            Line::from(""),
            Line::from("Press ESC to cancel"),
        ];

        let paragraph = Paragraph::new(lines)
            .block(Block::default())
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(app.theme.title_color));

        f.render_widget(paragraph, inner);
    } else {
        // Show preset list - user can select one to overwrite or type a new name
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Existing Presets ", Style::default()),
                Span::styled(
                    "(↑/↓ to select, Enter to overwrite, or type new name)",
                    Style::default().fg(app.theme.fg_muted),
                ),
            ]),
            Line::from(""),
        ];

        // Add each preset to the list
        for (i, preset) in app.available_plugin_presets.iter().enumerate() {
            let is_selected = i == app.selected_preset_index;
            let style = if is_selected {
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let marker = if is_selected { "► " } else { "  " };
            lines.push(Line::from(vec![
                Span::styled(marker, style),
                Span::styled(preset, style),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Hint: ", Style::default().fg(app.theme.title_color)),
            Span::raw("Select and press Enter to overwrite, or type to create new preset"),
        ]));

        let paragraph = Paragraph::new(lines)
            .block(Block::default())
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, inner);
    }
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
        .style(Style::default().bg(app.theme.bg_primary))
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
                Span::styled("> ", Style::default().fg(app.theme.accent_primary)),
                Span::raw(&app.plugin_file_input),
                Span::styled("_", Style::default().fg(app.theme.accent_success)),
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
            .style(Style::default().fg(app.theme.title_color));

        f.render_widget(paragraph, inner);
    } else {
        // Show preset list
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Available Presets ", Style::default()),
                Span::styled(
                    "(↑/↓ to select, Enter to load)",
                    Style::default().fg(app.theme.fg_muted),
                ),
            ]),
            Line::from(""),
        ];

        // Add preset items
        for (i, preset) in app.available_plugin_presets.iter().enumerate() {
            let is_selected = i == app.selected_preset_index;
            let style = if is_selected {
                Style::default()
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.bg_selected)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.accent_primary)
            };

            let marker = if is_selected { "► " } else { "  " };
            lines.push(Line::from(vec![
                Span::styled(marker, style),
                Span::styled(preset, style),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(
            "Or type a filename to load manually, ESC to cancel",
        ));

        let paragraph = Paragraph::new(lines)
            .block(Block::default())
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, inner);
    }
}

fn draw_load_apo_file_dialog(f: &mut Frame, app: &App) {
    // Create a centered dialog
    let area = f.area();
    let dialog_width = (area.width as f32 * 0.6) as u16;
    let dialog_height = 10;
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
        .style(Style::default().bg(app.theme.bg_primary))
        .title("Load APO EQ File");

    f.render_widget(block, dialog_area);

    // Inner area for text
    let inner = Rect {
        x: dialog_area.x + 1,
        y: dialog_area.y + 1,
        width: dialog_area.width.saturating_sub(2),
        height: dialog_area.height.saturating_sub(2),
    };

    let lines = vec![
        Line::from("Enter path to APO file:"),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(app.theme.accent_primary)),
            Span::raw(&app.apo_file_input),
            Span::styled("_", Style::default().fg(app.theme.accent_success)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Supported format:",
            Style::default().fg(app.theme.title_color),
        )]),
        Line::from("  Filter 1: ON PK Fc 100 Hz Gain -2.0 dB Q 1.41"),
        Line::from("  Filter 2: ON LSC Fc 105 Hz Gain 4.1 dB Q 0.71"),
        Line::from(""),
        Line::from("Press Enter to load, ESC to cancel"),
    ];

    let paragraph = Paragraph::new(lines)
        .block(Block::default())
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, inner);
}

fn draw_load_sofa_file_dialog(f: &mut Frame, app: &App) {
    // Create a centered dialog
    let area = f.area();
    let dialog_width = (area.width as f32 * 0.6) as u16;
    let dialog_height = 8;
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
        .style(Style::default().bg(app.theme.bg_primary))
        .title("Load SOFA HRTF File");

    f.render_widget(block, dialog_area);

    // Inner area for text
    let inner = Rect {
        x: dialog_area.x + 1,
        y: dialog_area.y + 1,
        width: dialog_area.width.saturating_sub(2),
        height: dialog_area.height.saturating_sub(2),
    };

    let lines = vec![
        Line::from("Enter path to SOFA file containing HRTFs:"),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(app.theme.accent_primary)),
            Span::raw(&app.sofa_file_input),
            Span::styled("_", Style::default().fg(app.theme.accent_success)),
        ]),
        Line::from(""),
        Line::from("SOFA format contains Head-Related Transfer Functions"),
        Line::from("Press Enter to set path, ESC to cancel"),
    ];

    let paragraph = Paragraph::new(lines)
        .block(Block::default())
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, inner);
}

fn draw_scan_progress_dialog(f: &mut Frame, app: &App) {
    // Create a centered dialog
    let area = f.area();
    let dialog_width = (area.width as f32 * 0.5) as u16;
    let dialog_height = 10;
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
        .border_type(BorderType::Double)
        .style(Style::default().bg(app.theme.bg_primary))
        .title("Scanning Library");

    f.render_widget(Clear, dialog_area);
    f.render_widget(block, dialog_area);

    // Inner area for text
    let inner = Rect {
        x: dialog_area.x + 2,
        y: dialog_area.y + 2,
        width: dialog_area.width.saturating_sub(4),
        height: dialog_area.height.saturating_sub(4),
    };

    let lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "Scanning directories for audio files...",
            Style::default().fg(app.theme.title_color),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Tracks found: "),
            Span::styled(
                format!("{}", app.scan_progress_tracks),
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("Albums found: "),
            Span::styled(
                format!("{}", app.scan_progress_albums),
                Style::default()
                    .fg(app.theme.accent_success)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Please wait...",
            Style::default()
                .fg(app.theme.fg_secondary)
                .add_modifier(Modifier::ITALIC),
        )]),
    ];

    let paragraph = Paragraph::new(lines)
        .block(Block::default())
        .alignment(Alignment::Left);

    f.render_widget(paragraph, inner);
}

fn draw_maintenance_progress_dialog(f: &mut Frame, app: &App) {
    // Create a centered dialog
    let area = f.area();
    let dialog_width = (area.width as f32 * 0.5) as u16;
    let dialog_height = 10;
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
        .border_type(BorderType::Double)
        .style(Style::default().bg(app.theme.bg_primary))
        .title("Database Maintenance");

    f.render_widget(Clear, dialog_area);
    f.render_widget(block, dialog_area);

    // Inner area for text
    let inner = Rect {
        x: dialog_area.x + 2,
        y: dialog_area.y + 2,
        width: dialog_area.width.saturating_sub(4),
        height: dialog_area.height.saturating_sub(4),
    };

    let progress_pct = if app.maintenance_progress_total > 0 {
        (app.maintenance_progress_checked as f32 / app.maintenance_progress_total as f32 * 100.0)
            as u32
    } else {
        0
    };

    let lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "Checking database for missing files...",
            Style::default().fg(app.theme.title_color),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Progress: "),
            Span::styled(
                format!(
                    "{} / {} ({}%)",
                    app.maintenance_progress_checked, app.maintenance_progress_total, progress_pct
                ),
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Please wait...",
            Style::default()
                .fg(app.theme.fg_secondary)
                .add_modifier(Modifier::ITALIC),
        )]),
    ];

    let paragraph = Paragraph::new(lines)
        .block(Block::default())
        .alignment(Alignment::Left);

    f.render_widget(paragraph, inner);
}

fn draw_replay_gain_progress_dialog(f: &mut Frame, app: &App) {
    // Create a centered dialog
    let area = f.area();
    let dialog_width = (area.width as f32 * 0.6) as u16;
    let dialog_height = 12;
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
        .border_type(BorderType::Double)
        .style(Style::default().bg(app.theme.bg_primary))
        .title("ReplayGain Analysis");

    f.render_widget(Clear, dialog_area);
    f.render_widget(block, dialog_area);

    // Inner area for text
    let inner = Rect {
        x: dialog_area.x + 2,
        y: dialog_area.y + 2,
        width: dialog_area.width.saturating_sub(4),
        height: dialog_area.height.saturating_sub(4),
    };

    let progress_pct = if app.replay_gain_manager.total > 0 {
        (app.replay_gain_manager.processed as f32 / app.replay_gain_manager.total as f32 * 100.0)
            as u32
    } else {
        0
    };

    let lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "Analyzing tracks for ReplayGain...",
            Style::default().fg(app.theme.title_color),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Progress: "),
            Span::styled(
                format!(
                    "{} / {} ({}%)",
                    app.replay_gain_manager.processed, app.replay_gain_manager.total, progress_pct
                ),
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Succeeded: "),
            Span::styled(
                format!("{}", app.replay_gain_manager.succeeded),
                Style::default()
                    .fg(app.theme.accent_success)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Failed: "),
            Span::styled(
                format!("{}", app.replay_gain_manager.failed),
                Style::default()
                    .fg(app.theme.accent_error)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Please wait...",
            Style::default()
                .fg(app.theme.fg_secondary)
                .add_modifier(Modifier::ITALIC),
        )]),
    ];

    let paragraph = Paragraph::new(lines)
        .block(Block::default())
        .alignment(Alignment::Left);

    f.render_widget(paragraph, inner);
}

fn draw_help_modal(f: &mut Frame, app: &App) {
    // Create a centered modal (80% width, 90% height)
    let area = f.area();
    let modal_width = (area.width as f32 * 0.8) as u16;
    let modal_height = (area.height as f32 * 0.9) as u16;
    let modal_x = (area.width - modal_width) / 2;
    let modal_y = (area.height - modal_height) / 2;

    let modal_area = Rect {
        x: modal_x,
        y: modal_y,
        width: modal_width,
        height: modal_height,
    };

    // Clear background
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .style(Style::default().bg(app.theme.bg_primary))
        .title(format!(
            "Help - {} Screen (Press ESC or ? to close)",
            match app.current_screen {
                Screen::Library => "Library",
                Screen::DirectoryManager => "Directories",
                Screen::Queue => "Queue",
                Screen::Plugins => "Plugins",
                Screen::Devices => "Devices",
            }
        ));

    f.render_widget(Clear, modal_area);
    f.render_widget(block, modal_area);

    // Inner area for content
    let inner = Rect {
        x: modal_area.x + 2,
        y: modal_area.y + 1,
        width: modal_area.width.saturating_sub(4),
        height: modal_area.height.saturating_sub(2),
    };

    // Get keybindings for current screen
    let keybindings = get_keybindings_for_screen(app.current_screen);

    // Build help text
    let mut lines = vec![];

    // Global keybindings
    lines.push(Line::from(vec![Span::styled(
        "GLOBAL KEYBINDINGS",
        Style::default()
            .fg(app.theme.title_color)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  TAB", Style::default().fg(app.theme.accent_primary)),
        Span::raw("  Cycle through screens and level meters pane"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  L/D/Q/P/O", Style::default().fg(app.theme.accent_primary)),
        Span::raw("  Jump to Library/Directories/Queue/Plugins/Devices"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Shift+M", Style::default().fg(app.theme.accent_primary)),
        Span::raw("  Focus level meters pane"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  +/=", Style::default().fg(app.theme.accent_primary)),
        Span::raw("  Increase volume"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  -/_", Style::default().fg(app.theme.accent_primary)),
        Span::raw("  Decrease volume"),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  Ctrl+Left/Right",
            Style::default().fg(app.theme.accent_primary),
        ),
        Span::raw("  Select output device"),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "LEVEL METERS (when Meters pane is focused)",
        Style::default()
            .fg(app.theme.border_color)
            .add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(vec![
        Span::styled(
            "  Left/Right",
            Style::default().fg(app.theme.accent_primary),
        ),
        Span::raw("  Navigate between channel groups"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Up/Down", Style::default().fg(app.theme.accent_primary)),
        Span::raw("  Select mute/solo control"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  m/s", Style::default().fg(app.theme.accent_primary)),
        Span::raw("  Toggle mute/solo on selected group"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  c", Style::default().fg(app.theme.accent_primary)),
        Span::raw("  Clear all mutes and solos"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  ESC", Style::default().fg(app.theme.accent_primary)),
        Span::raw("  Return to main pane"),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "LEVEL METERS (global shortcuts)",
        Style::default()
            .fg(app.theme.border_color)
            .add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(vec![
        Span::styled(
            "  Shift+Left/Right",
            Style::default().fg(app.theme.accent_primary),
        ),
        Span::raw("  Navigate level meter groups"),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  Shift+Up/Down",
            Style::default().fg(app.theme.accent_primary),
        ),
        Span::raw("  Select mute/solo control"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Shift+S", Style::default().fg(app.theme.accent_primary)),
        Span::raw("  Toggle solo on selected group"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Shift+C", Style::default().fg(app.theme.accent_primary)),
        Span::raw("  Clear all mutes and solos"),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ?", Style::default().fg(app.theme.accent_primary)),
        Span::raw("  Show this help"),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  Ctrl+Q/Cmd+Q",
            Style::default().fg(app.theme.accent_primary),
        ),
        Span::raw("  Quit (ESC quits from main pane)"),
    ]));
    lines.push(Line::from(""));

    // Screen-specific keybindings
    lines.push(Line::from(vec![Span::styled(
        format!(
            "{} KEYBINDINGS",
            match app.current_screen {
                Screen::Library => "LIBRARY",
                Screen::DirectoryManager => "DIRECTORIES",
                Screen::Queue => "QUEUE",
                Screen::Plugins => "PLUGINS",
                Screen::Devices => "DEVICES",
            }
        ),
        Style::default()
            .fg(app.theme.border_color)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )]));
    lines.push(Line::from(""));

    for (key, description) in keybindings {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {:<18}", key),
                Style::default().fg(app.theme.accent_primary),
            ),
            Span::raw(description),
        ]));
    }

    let paragraph = Paragraph::new(lines)
        .block(Block::default())
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, inner);
}

fn get_keybindings_for_screen(screen: Screen) -> Vec<(&'static str, &'static str)> {
    match screen {
        Screen::Library => vec![
            ("↑/↓ or k/j", "Navigate albums/artists"),
            ("PageUp/PageDown", "Jump by page"),
            ("/", "Search albums"),
            ("t", "Toggle tree view / flat view"),
            ("h/l or ←/→", "Collapse/expand artists in tree view"),
            ("s or 1/2/3/4", "Sort by Artist/Album/Title/Year"),
            ("c or 5/6/7/8/9", "Filter: All/Mono/Stereo/Multi/Mixed"),
            ("a or Enter", "Add album to queue"),
            ("q", "Go to queue screen"),
        ],
        Screen::DirectoryManager => vec![
            ("↑/↓ or k/j", "Navigate directories"),
            ("PageUp/PageDown", "Jump by page"),
            ("Enter/→/l", "Expand/collapse directory"),
            ("a", "Add directory"),
            ("d/Delete", "Remove selected directory"),
            ("s", "Scan library (incremental)"),
            ("R", "Force rescan ALL files (preserves ReplayGain)"),
            ("m", "Database maintenance (clean missing files)"),
            ("r", "Analyze ReplayGain for all tracks"),
        ],
        Screen::Queue => vec![
            ("↑/↓ or k/j", "Navigate queue items"),
            ("Enter", "Play selected album from start"),
            ("h/l or ←/→", "Expand/collapse album tracks"),
            ("p", "Play/resume from current position"),
            ("Space", "Pause/resume"),
            ("n or >", "Next track"),
            ("b or <", "Previous track"),
            ("d/Delete", "Remove from queue"),
            ("c", "Clear entire queue"),
        ],
        Screen::Plugins => vec![
            ("↑/↓ or k/j", "Navigate plugin chain"),
            ("a", "Add plugin (default)"),
            (
                "1-8",
                "Quick add: EQ/Upmixer/Compressor/Gate/Limiter/Loudness/Binaural/Convolution",
            ),
            ("9/0", "Quick add: LoudnessMonitor/SpectrumAnalyzer"),
            ("g", "Quick add: Gain"),
            ("m", "Quick add: Channel Mute/Solo"),
            ("e or Enter", "Edit selected plugin"),
            ("t", "Toggle plugin enabled/disabled"),
            ("d/Delete", "Remove plugin"),
            ("u/U or Shift+↑", "Move plugin up in chain"),
            ("n/N or Shift+↓", "Move plugin down in chain"),
            ("s", "Save plugin chain to file"),
            ("l", "Load plugin chain from file"),
            ("", ""),
            ("EDIT MODE:", "(when editing a plugin)"),
            ("↑/↓ or k/j", "Navigate parameters"),
            ("←/→ or h/l", "Adjust parameter value (small)"),
            ("[/]", "Adjust parameter value (large)"),
            ("a", "Load APO file (EQ plugins only)"),
            ("f", "Load SOFA file (Binaural Decoder only)"),
            ("ESC", "Exit edit mode"),
        ],
        Screen::Devices => vec![
            ("↑/↓ or k/j", "Navigate output devices"),
            ("Enter/Space", "Select output device"),
        ],
    }
}

/// Simple text wrapping function that breaks text into lines at word boundaries
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= max_width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    // Handle empty input
    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

fn draw_error_modal(f: &mut Frame, app: &App) {
    if let Some(error_msg) = &app.error_message {
        // Create a centered modal (60% width, auto height based on content)
        let area = f.area();
        let modal_width = (area.width as f32 * 0.6) as u16;

        // Calculate required height based on error message
        let max_text_width = modal_width.saturating_sub(6) as usize; // Account for borders and padding
        let wrapped_lines = wrap_text(error_msg, max_text_width);
        let content_height = wrapped_lines.len() + 6; // Message + title + instructions + padding
        let modal_height = content_height.min(area.height.saturating_sub(4) as usize) as u16;

        let modal_x = (area.width - modal_width) / 2;
        let modal_y = (area.height - modal_height) / 2;

        let modal_area = Rect {
            x: modal_x,
            y: modal_y,
            width: modal_width,
            height: modal_height,
        };

        // Clear background and draw border
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(Color::Red))
            .style(Style::default().bg(app.theme.bg_primary))
            .title(" Error ");

        f.render_widget(Clear, modal_area);
        f.render_widget(block, modal_area);

        // Inner area for content
        let inner = Rect {
            x: modal_area.x + 2,
            y: modal_area.y + 1,
            width: modal_area.width.saturating_sub(4),
            height: modal_area.height.saturating_sub(2),
        };

        // Build error message text
        let mut lines = vec![];

        // Add error icon and message
        lines.push(Line::from(vec![
            Span::styled(
                "✗ ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Audio Playback Error",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));

        // Add wrapped error message
        for line in wrapped_lines {
            lines.push(Line::from(Span::raw(line)));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(""));

        // Add instructions
        lines.push(Line::from(vec![
            Span::styled("Press ", Style::default()),
            Span::styled(
                "ESC",
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(", ", Style::default()),
            Span::styled(
                "Enter",
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(", or ", Style::default()),
            Span::styled(
                "Space",
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to close", Style::default()),
        ]));

        let paragraph = Paragraph::new(lines)
            .block(Block::default())
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, inner);
    }
}
