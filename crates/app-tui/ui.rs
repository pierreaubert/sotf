use crate::app::{App, FocusedPane, InputMode, LibraryViewMode, MatrixEditMode, Screen, TreeItem};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table,
        Wrap,
    },
};
use sotf_audio_player::{
    PluginSettings, PluginType, detect_matrix_preset, get_channel_label, linear_to_db_string,
};

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

/// Get a human-readable name for a path config JSON (for A/B Compare plugin)
fn path_config_to_display_name(config: &str) -> String {
    if config.is_empty() || config == r#"{"type":"None"}"# {
        "None (passthrough)".to_string()
    } else if config.contains(r#""plugin_type":"EQ""#) {
        "EQ".to_string()
    } else if config.contains(r#""plugin_type":"gain""#) {
        "Gain".to_string()
    } else if config.contains(r#""plugin_type":"compressor""#) {
        "Compressor".to_string()
    } else if config.contains(r#""plugin_type":"limiter""#) {
        "Limiter".to_string()
    } else if config.contains(r#""plugin_type":"gate""#) {
        "Gate".to_string()
    } else if config.contains(r#""plugin_type":"expander""#) {
        "Expander".to_string()
    } else if config.contains(r#""plugin_type":"denoiser""#) {
        "Denoiser".to_string()
    } else if config.contains(r#""plugin_type":"loudness_compensation""#) {
        "Loudness Comp".to_string()
    } else if config.contains(r#""type":"Rack""#) {
        "Rack (chain)".to_string()
    } else if config.contains(r#""type":"Graph""#) {
        "Graph".to_string()
    } else {
        "Custom".to_string()
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
    // Paint the entire frame with theme colors so all widgets inherit them
    let bg_block = Block::default().style(
        Style::default()
            .bg(app.theme.bg_primary)
            .fg(app.theme.fg_primary),
    );
    f.render_widget(bg_block, f.area());

    // Ensure filtered albums cache is updated
    app.filtered_albums();

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
    } else if matches!(
        app.input_mode,
        InputMode::BrowseSofaFile | InputMode::BrowseIrFile
    ) {
        draw_file_browser_modal(f, app);
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

    // Channel conflict modal
    if app.input_mode == InputMode::ChannelConflict {
        draw_channel_conflict_modal(f, app);
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
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(app.theme.fg_primary)),
        );

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
        .style(Style::default().fg(app.theme.fg_primary))
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
        Style::default().fg(app.theme.fg_primary)
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
    let base_title_style = Style::default().fg(app.theme.fg_secondary);
    let suffix = format!(" [c/5-9]{})", counts_str);
    let title_spans = vec![
        Span::styled("Search Albums ('/' search | Sort: ", base_title_style),
        Span::styled(sort_order_str, Style::default().fg(app.theme.border_color)),
        Span::styled(" [s/1-4] | Filter: ", base_title_style),
        Span::styled(&filter_str, Style::default().fg(app.theme.border_color)),
        Span::styled(suffix, base_title_style),
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
            let albums = &app.cached_filtered_albums;

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
                        Style::default().fg(app.theme.fg_primary)
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

                                let style = if i == app.selected_tree_index {
                                    Style::default()
                                        .fg(app.theme.fg_selected)
                                        .bg(app.theme.bg_selected)
                                        .add_modifier(Modifier::BOLD)
                                } else {
                                    Style::default().fg(app.theme.fg_primary)
                                };
                                (content, style)
                            } else {
                                ("  └─ <unknown>".to_string(), Style::default().fg(app.theme.fg_muted))
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
        Style::default().fg(app.theme.fg_primary)
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
                Style::default().fg(app.theme.fg_primary)
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
    let mut selected_visual_index: Option<usize> = None;

    for (i, entry) in app.queue.iter().enumerate() {
        let is_current = app.current_queue_index == Some(i);
        let is_selected = i == app.selected_queue_index;
        let is_album_header_selected = is_selected && app.selected_queue_track_index.is_none();
        let is_expanded = entry.expanded;

        // Album header
        let expand_indicator = if is_expanded { "▼" } else { "▶" };
        let raw_display = entry.item.album.display_name();
        let cleaned_display = clean_text(&raw_display);
        let truncated_display = truncate_with_ellipsis(&cleaned_display, 90);
        let mut content = format!("{} {}", expand_indicator, truncated_display);

        if is_current {
            let track_info = format!(
                " [Track {}/{}]",
                entry.item.current_track_index + 1,
                entry.item.album.tracks.len()
            );
            content.push_str(&track_info);
        }

        let style = if is_album_header_selected {
            Style::default()
                .fg(app.theme.fg_selected)
                .bg(app.theme.bg_selected)
                .add_modifier(Modifier::BOLD)
        } else if is_current {
            Style::default()
                .fg(app.theme.playing_indicator)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.fg_primary)
        };

        if is_album_header_selected {
            selected_visual_index = Some(items.len());
        }
        items.push(ListItem::new(content).style(style));

        // Show individual tracks if expanded
        if is_expanded {
            for (track_idx, track) in entry.item.album.tracks.iter().enumerate() {
                let is_current_track = is_current && track_idx == entry.item.current_track_index;
                let is_track_selected =
                    is_selected && app.selected_queue_track_index == Some(track_idx);
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

                let track_style = if is_track_selected {
                    Style::default()
                        .fg(app.theme.fg_selected)
                        .bg(app.theme.bg_selected)
                        .add_modifier(Modifier::BOLD)
                } else if is_current_track {
                    Style::default()
                        .fg(app.theme.current_track)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.theme.fg_secondary)
                };

                if is_track_selected {
                    selected_visual_index = Some(items.len());
                }
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

    let list = List::new(items)
        .style(Style::default().fg(app.theme.fg_primary))
        .highlight_style(Style::default()) // we handle highlighting manually per item
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(border_type)
                .title(title),
        );

    let mut list_state = ListState::default();
    list_state.select(selected_visual_index);
    f.render_stateful_widget(list, queue_area, &mut list_state);

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

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().fg(app.theme.fg_primary));
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
                let image = StatefulImage::new();
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
        let no_image_text = Paragraph::new("No album art found")
            .style(Style::default().fg(app.theme.fg_muted));
        f.render_widget(no_image_text, image_area);
    }

    // Render ReplayGain info below the image
    draw_replay_gain_info(f, info_area, app);
}

fn draw_replay_gain_info(f: &mut Frame, area: Rect, app: &App) {
    // Get currently playing track
    let track_info = if let Some(queue_index) = app.current_queue_index {
        if let Some(entry) = app.queue.get(queue_index) {
            if let Some(track) = entry.item.album.tracks.get(entry.item.current_track_index) {
                Some((track, &entry.item.album))
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

    // ReplayGain correction status
    let applied_gain = app.plugin_chain.replay_gain_db();
    let mode_str = match app.replay_gain_mode {
        crate::app::ReplayGainMode::Track => "Track",
        crate::app::ReplayGainMode::Album => "Album",
    };
    let status_spans = if app.replay_gain_enabled {
        if let Some(db) = applied_gain {
            vec![
                Span::styled("RG: ", Style::default().fg(app.theme.title_color)),
                Span::styled(
                    format!("{:+.2} dB ({})", db, mode_str),
                    Style::default().fg(app.theme.accent_success),
                ),
            ]
        } else {
            vec![
                Span::styled("RG: ", Style::default().fg(app.theme.title_color)),
                Span::styled(
                    format!("ON ({}) - no data", mode_str),
                    Style::default().fg(app.theme.fg_muted),
                ),
            ]
        }
    } else {
        vec![
            Span::styled("RG: ", Style::default().fg(app.theme.title_color)),
            Span::styled("OFF", Style::default().fg(app.theme.fg_muted)),
        ]
    };
    lines.push(Line::from(status_spans));

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

    let paragraph = Paragraph::new(lines)
        .style(Style::default().fg(app.theme.fg_primary))
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn draw_plugins_screen(f: &mut Frame, area: Rect, app: &App) {
    // Split vertically: command bar on top, plugin panels below
    let vchunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Command bar
            Constraint::Min(0),   // Plugin panels
        ])
        .split(area);

    // Draw command bar
    draw_plugin_command_bar(f, vchunks[0], app);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30), // Plugin chain
            Constraint::Percentage(70), // Available plugins
        ])
        .split(vchunks[1]);

    // Plugin chain list
    draw_plugin_chain(f, chunks[0], app);

    // Available plugins list
    draw_available_plugins(f, chunks[1], app);
}

fn draw_plugin_command_bar(f: &mut Frame, area: Rect, app: &App) {
    let help_text = if app.input_mode == InputMode::AddPlugin {
        " ↑/↓=navigate  Enter=add  Esc=cancel"
    } else if app.plugin_chain.is_empty() {
        " 'a'=add plugins  's'=save  'l'=load"
    } else {
        " 'e'=edit  't'=toggle  'd'=remove  '↑/↓'=move  'a'=add  's'=save  'l'=load"
    };

    let bar = Paragraph::new(Line::from(vec![
        Span::styled(
            "Commands:",
            Style::default()
                .fg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(help_text, Style::default().fg(app.theme.fg_secondary)),
    ]))
    .alignment(ratatui::layout::Alignment::Center);

    f.render_widget(bar, area);
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

            let style = if plugin.enabled {
                Style::default().fg(app.theme.accent_success)
            } else {
                Style::default().fg(app.theme.fg_muted)
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let title = if app.plugin_chain.is_empty() {
        "0 plugins".to_string()
    } else {
        format!(
            "{} plugins ({} ch)",
            app.plugin_chain.len(),
            app.plugin_chain.output_channels()
        )
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title),
        )
        .highlight_style(
            Style::default()
                .fg(app.theme.fg_selected)
                .bg(app.theme.bg_selected)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    if !app.plugin_chain.is_empty() {
        state.select(Some(app.selected_plugin_index));
    }

    use ratatui::widgets::StatefulWidget;
    StatefulWidget::render(list, area, f.buffer_mut(), &mut state);
}

fn draw_available_plugins(f: &mut Frame, area: Rect, app: &App) {
    let mut plugins = PluginType::all();
    plugins.sort_by_key(|p| p.name());
    let is_selecting = app.input_mode == InputMode::AddPlugin;

    let items: Vec<ListItem> = plugins
        .iter()
        .map(|plugin_type| {
            let content = format!("{} - {}", plugin_type.name(), plugin_type.description());
            ListItem::new(content).style(Style::default().fg(app.theme.accent_primary))
        })
        .collect();

    let title = if is_selecting {
        "▶ Select Plugin (↑/↓, Enter=add, Esc=cancel)"
    } else {
        "Available Plugins"
    };

    let border_style = if is_selecting {
        Style::default().fg(app.theme.accent_primary)
    } else {
        Style::default().fg(app.theme.fg_secondary)
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title),
        )
        .highlight_style(
            Style::default()
                .fg(app.theme.fg_selected)
                .bg(app.theme.bg_selected)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    if is_selecting {
        state.select(Some(app.add_plugin_selected_index));
    }

    use ratatui::widgets::StatefulWidget;
    StatefulWidget::render(list, area, f.buffer_mut(), &mut state);
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
                Style::default().fg(app.theme.fg_primary)
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
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Loudness")
        .style(Style::default().fg(app.theme.fg_primary));
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
            .style(Style::default().fg(app.theme.fg_muted))
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
            .style(Style::default().fg(app.theme.fg_muted))
            .block(Block::default().borders(Borders::ALL).title("Levels"));
        f.render_widget(paragraph, area);
        return;
    }

    // Update channel groups if needed (method handles caching internally)
    // Do this BEFORE borrowing loudness immutably
    app.update_level_meter_groups();

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
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(app.theme.fg_primary))
    };
    f.render_widget(block, area);

    // Render title lines at the top inside the border
    for (i, line) in title_lines.iter().enumerate() {
        f.render_widget(
            Paragraph::new(line.clone())
                .style(Style::default().fg(app.theme.fg_primary)),
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

    // Scale legend width (3 chars: "-60", "-20", " 0 ") + 2-char gap to meters
    let scale_text_width = 3usize;
    let scale_gap = 2usize;
    let scale_width = scale_text_width + scale_gap; // 5 total before meters
    let available_width = inner.width as usize;

    // Check if we should show right-side scale (only for single stereo group with enough space)
    let is_single_stereo_group = app.level_meter_groups.len() == 1
        && app.level_meter_groups[0].channels.len() == 2;
    let right_scale_width = if is_single_stereo_group && available_width >= scale_width * 2 + 8 {
        scale_width
    } else {
        0
    };

    // Calculate total width for multi-group layout
    let total_groups_width: usize = app
        .level_meter_groups
        .iter()
        .map(|g| g.channels.len().max(3))
        .sum::<usize>()
        + app.level_meter_groups.len().saturating_sub(1); // 1-char gaps between groups

    // Right-align: legend + gap + meters flush against the right edge
    // Right-align: meters flush against the right edge
    // For stereo, the stereo branch handles its own centering
    let mut x_offset = if is_single_stereo_group {
        scale_width // stereo branch handles its own centering
    } else {
        available_width.saturating_sub(total_groups_width)
    };

    // Position the dB scale legend just before the meters (2 chars gap)
    let scale_x = if is_single_stereo_group {
        0 // stereo: legend at left edge
    } else {
        x_offset.saturating_sub(scale_width)
    };

    // Non-linear scale: -60 dB (0%), -40 dB (20%), -20 dB (50%), 0 dB (100%)
    let scale_markers = [
        (1.0, " 0 "), // 100% fill -> top
        (0.5, "-20"), // 50% fill
        (0.2, "-40"), // 20% fill
        (0.0, "-60"), // 0% fill -> bottom
    ];

    // Draw vertical scale legend on the left
    for (ratio, label) in scale_markers.iter() {
        let row_idx = (ratio * meter_height as f64).round() as usize;
        let y = inner.y + (meter_height - 1).saturating_sub(row_idx.min(meter_height - 1)) as u16;

        f.render_widget(
            Paragraph::new(*label).style(Style::default().fg(app.theme.fg_muted)),
            Rect {
                x: inner.x + scale_x as u16,
                y,
                width: scale_text_width as u16,
                height: 1,
            },
        );
    }

    // Draw vertical scale legend on the right (if applicable, stereo only)
    if right_scale_width > 0 {
        let right_x = inner.x + inner.width - scale_text_width as u16;
        for (ratio, label) in scale_markers.iter() {
            let row_idx = (ratio * meter_height as f64).round() as usize;
            let y =
                inner.y + (meter_height - 1).saturating_sub(row_idx.min(meter_height - 1)) as u16;

            f.render_widget(
                Paragraph::new(*label).style(Style::default().fg(app.theme.fg_muted)),
                Rect {
                    x: right_x,
                    y,
                    width: scale_text_width as u16,
                    height: 1,
                },
            );
        }
    }

    // Draw each group
    for (group_idx, group) in app.level_meter_groups.iter().enumerate() {
        let is_selected = group_idx == app.selected_level_meter_group;

        // Calculate width for this group
        let num_channels = group.channels.len();
        let is_stereo = is_single_stereo_group;
        let group_width = if is_stereo {
            8 // 3 + 2 + 3 for stereo
        } else {
            num_channels.max(3)
        };

        if is_stereo {
            // Special stereo rendering: 3-char wide meters with 2-char spacing
            // Center the group in the meter area (between left and right scales)
            let meter_area_start = scale_width;
            let meter_area_end = available_width.saturating_sub(right_scale_width);
            let meter_area_width = meter_area_end.saturating_sub(meter_area_start);
            let group_start_x = if meter_area_width > group_width {
                meter_area_start + (meter_area_width - group_width) / 2
            } else {
                meter_area_start
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
            // Center [M][S][D] (3 chars) under the group
            let controls_x = if is_stereo {
                let meter_area_start = scale_width;
                let meter_area_end = available_width.saturating_sub(right_scale_width);
                let meter_area_width = meter_area_end.saturating_sub(meter_area_start);
                let group_start_x = if meter_area_width > group_width {
                    meter_area_start + (meter_area_width - group_width) / 2
                } else {
                    meter_area_start
                };
                // Center [M][S][D] under the 8-char stereo group
                let ctrl_offset = group_start_x + (group_width - 3) / 2;
                if ctrl_offset + 3 > available_width {
                    x_offset += group_width + 1;
                    continue;
                }
                inner.x + ctrl_offset as u16
            } else {
                // Center [M][S][D] (3 chars) under the group_width
                let ctrl_offset = x_offset + group_width.saturating_sub(3) / 2;
                if ctrl_offset + 3 > available_width {
                    x_offset += group_width + 1;
                    continue;
                }
                inner.x + ctrl_offset as u16
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
    let key_style = Style::default().fg(app.theme.title_color);
    let volume_style = Style::default()
        .fg(app.theme.accent_primary)
        .add_modifier(Modifier::BOLD);
    let text = Line::from(vec![
        Span::styled("[-_] ", key_style),
        Span::styled(format!("{}%", volume_pct), volume_style),
        Span::styled(" [=+]", key_style),
    ]);

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
        && let Some(entry) = app.queue.get(idx)
        && let Some(track) = entry.item.current_track()
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
        "ESC/%-Q",
        Style::default().fg(app.theme.accent_error),
    ));
    status_spans.push(Span::raw("=Quit "));

    let status_text = Line::from(status_spans);

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(app.theme.fg_primary))
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: true });

    f.render_widget(status, area);
}

fn draw_plugin_editor_modal(f: &mut Frame, app: &App) {
    if let Some(plugin) = app.get_editing_plugin() {
        // Check if we're editing a Matrix plugin - use specialized editor
        if matches!(plugin.settings, PluginSettings::Matrix { .. }) {
            draw_matrix_editor_modal(f, app);
            return;
        }

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

        // Clear the background
        f.render_widget(Clear, modal_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .style(
                Style::default()
                    .bg(app.theme.bg_primary)
                    .fg(app.theme.fg_primary),
            )
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

        let base_style = Style::default()
            .bg(app.theme.bg_primary)
            .fg(app.theme.fg_primary);

        // Build parameter list
        let mut lines = Vec::new();
        lines.push(Line::from(vec![
            Span::styled("Use ", base_style),
            Span::styled("↑/↓", base_style.fg(app.theme.accent_primary)),
            Span::styled(" to select parameter, ", base_style),
            Span::styled("←/→", base_style.fg(app.theme.accent_primary)),
            Span::styled(" to adjust value", base_style),
        ]));
        lines.push(Line::from(Span::styled("", base_style)));

        let entries = get_plugin_parameters(&plugin.settings, app.plugin_param_selection);

        // Compute the max label width for right-alignment (only from Param entries)
        let max_label_width = entries
            .iter()
            .filter_map(|e| match e {
                ParamDisplayEntry::Param(name, _) => Some(name.len()),
                ParamDisplayEntry::Separator(_) => None,
            })
            .max()
            .unwrap_or(0);

        let inner_width = inner.width as usize;
        let mut param_index = 0usize;
        let mut selected_line_index = 0usize; // display line of selected param
        for entry in &entries {
            match entry {
                ParamDisplayEntry::Separator(title) => {
                    // Render as a separator line: ── Title ──────
                    let prefix = "\u{2500}\u{2500} ";
                    let suffix_char = '\u{2500}';
                    let label = format!("{}{} ", prefix, title);
                    let remaining = inner_width.saturating_sub(label.len());
                    let separator_line = format!(
                        "{}{}",
                        label,
                        std::iter::repeat(suffix_char).take(remaining).collect::<String>()
                    );
                    lines.push(Line::from(Span::styled(
                        separator_line,
                        base_style.fg(app.theme.fg_secondary),
                    )));
                }
                ParamDisplayEntry::Param(name, value) => {
                    if param_index == app.plugin_param_selection {
                        selected_line_index = lines.len();
                    }
                    let style = if param_index == app.plugin_param_selection {
                        Style::default()
                            .fg(app.theme.fg_selected)
                            .bg(app.theme.bg_selected)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        base_style
                    };

                    // Right-align the label by padding on the left
                    let padded_name =
                        format!("{:>width$}", name, width = max_label_width + 1);

                    lines.push(Line::from(vec![
                        Span::styled(format!("{} ", padded_name), style),
                        Span::styled(value.to_string(), style.fg(app.theme.title_color)),
                    ]));
                    param_index += 1;
                }
            }
        }

        // Auto-scroll to keep selected parameter visible
        let visible_height = inner.height as usize;
        let scroll_offset = if selected_line_index >= visible_height {
            (selected_line_index - visible_height + 2) as u16
        } else {
            0
        };

        let paragraph = Paragraph::new(lines)
            .style(Style::default().fg(app.theme.fg_primary))
            .block(Block::default())
            .style(base_style)
            .scroll((scroll_offset, 0));

        f.render_widget(paragraph, inner);
    }
}

/// Specialized matrix editor modal with visual grid display
fn draw_matrix_editor_modal(f: &mut Frame, app: &App) {
    let Some(plugin) = app.get_editing_plugin() else {
        return;
    };

    let PluginSettings::Matrix {
        input_channels,
        output_channels,
        matrix,
        ..
    } = &plugin.settings
    else {
        return;
    };

    // Create a centered modal (70% width, 85% height)
    let area = f.area();
    let modal_width = (area.width as f32 * 0.7).min(80.0) as u16;
    let modal_height = (area.height as f32 * 0.85).min(35.0) as u16;
    let modal_x = (area.width - modal_width) / 2;
    let modal_y = (area.height - modal_height) / 2;

    let modal_area = Rect {
        x: modal_x,
        y: modal_y,
        width: modal_width,
        height: modal_height,
    };

    // Clear background
    f.render_widget(Clear, modal_area);

    let preset_name = detect_matrix_preset(*input_channels, *output_channels, matrix);

    // Outer block
    let block = Block::default()
        .borders(Borders::ALL)
        .style(
            Style::default()
                .bg(app.theme.bg_primary)
                .fg(app.theme.fg_primary),
        )
        .title(format!(" Matrix Mixer - {} (ESC to close) ", preset_name));
    f.render_widget(block, modal_area);

    // Inner area
    let inner = Rect {
        x: modal_area.x + 1,
        y: modal_area.y + 1,
        width: modal_area.width.saturating_sub(2),
        height: modal_area.height.saturating_sub(2),
    };

    // Split into header (5 lines) and grid sections
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Header section
            Constraint::Min(3),    // Grid section
            Constraint::Length(2), // Help line
        ])
        .split(inner);

    // === Header Section ===
    draw_matrix_header(
        f,
        app,
        chunks[0],
        *input_channels,
        *output_channels,
        preset_name,
    );

    // === Grid Section ===
    draw_matrix_grid(f, app, chunks[1], *input_channels, *output_channels, matrix);

    // === Help Line ===
    let help_text = match app.matrix_edit_mode {
        MatrixEditMode::Header => "↑↓: Select | ←→: Adjust | Tab: Grid Mode | Esc: Exit",
        MatrixEditMode::Grid => {
            "↑↓←→: Navigate | -/+: Adjust ±0.5dB | 0: Zero | 1: Unity | Tab: Header Mode | Esc: Exit"
        }
    };
    let help = Paragraph::new(help_text)
        .style(Style::default().fg(app.theme.fg_secondary))
        .alignment(Alignment::Center);
    f.render_widget(help, chunks[2]);
}

/// Draw the header section of matrix editor (input/output channels, preset)
fn draw_matrix_header(
    f: &mut Frame,
    app: &App,
    area: Rect,
    input_channels: usize,
    output_channels: usize,
    preset_name: &str,
) {
    let in_header = app.matrix_edit_mode == MatrixEditMode::Header;

    let mut lines = Vec::new();

    let base_style = Style::default().fg(app.theme.fg_primary);

    // Input channels line
    let input_style = if in_header && app.matrix_header_selection == 0 {
        Style::default()
            .fg(app.theme.fg_selected)
            .bg(app.theme.bg_selected)
            .add_modifier(Modifier::BOLD)
    } else {
        base_style
    };
    lines.push(Line::from(vec![
        Span::styled("  Input Channels:  ", base_style),
        Span::styled(
            format!("[{}]", input_channels),
            input_style.fg(app.theme.accent_primary),
        ),
        Span::styled(
            format!("  ({})", format_channel_config(input_channels)),
            Style::default().fg(app.theme.fg_secondary),
        ),
    ]));

    // Output channels line
    let output_style = if in_header && app.matrix_header_selection == 1 {
        Style::default()
            .fg(app.theme.fg_selected)
            .bg(app.theme.bg_selected)
            .add_modifier(Modifier::BOLD)
    } else {
        base_style
    };
    lines.push(Line::from(vec![
        Span::styled("  Output Channels: ", base_style),
        Span::styled(
            format!("[{}]", output_channels),
            output_style.fg(app.theme.accent_primary),
        ),
        Span::styled(
            format!("  ({})", format_channel_config(output_channels)),
            Style::default().fg(app.theme.fg_secondary),
        ),
    ]));

    // Preset line
    let preset_style = if in_header && app.matrix_header_selection == 2 {
        Style::default()
            .fg(app.theme.fg_selected)
            .bg(app.theme.bg_selected)
            .add_modifier(Modifier::BOLD)
    } else {
        base_style
    };
    lines.push(Line::from(vec![
        Span::styled("  Preset:          ", base_style),
        Span::styled(
            format!("[{}]", preset_name),
            preset_style.fg(app.theme.title_color),
        ),
    ]));

    lines.push(Line::from(""));

    let paragraph = Paragraph::new(lines)
        .style(Style::default().fg(app.theme.fg_primary));
    f.render_widget(paragraph, area);
}

/// Format channel count to a common config name
fn format_channel_config(channels: usize) -> &'static str {
    match channels {
        1 => "Mono",
        2 => "Stereo",
        3 => "2.1 / LCR",
        4 => "Quad",
        5 => "5.0",
        6 => "5.1",
        8 => "7.1",
        10 => "7.1.2",
        12 => "7.1.4",
        _ => "Custom",
    }
}

/// Draw the matrix grid with channel labels and dB values
fn draw_matrix_grid(
    f: &mut Frame,
    app: &App,
    area: Rect,
    input_channels: usize,
    output_channels: usize,
    matrix: &[f32],
) {
    let in_grid = app.matrix_edit_mode == MatrixEditMode::Grid;

    // Calculate column widths: first column for row labels, then one per input
    let label_width = 5u16; // "Out" label column
    let cell_width = 7u16; // Each gain cell (e.g., "-12.5" or "-∞")

    // Build header row (empty corner + input channel labels)
    let mut header_cells =
        vec![Cell::from("Out\\In").style(Style::default().fg(app.theme.fg_secondary))];
    for inp in 0..input_channels {
        let label = get_channel_label(inp, input_channels);
        header_cells.push(
            Cell::from(label).style(
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
        );
    }
    let header = Row::new(header_cells).height(1);

    // Build data rows
    let mut rows = Vec::new();
    for out in 0..output_channels {
        let mut cells = Vec::new();

        // Row label (output channel)
        let row_label = get_channel_label(out, output_channels);
        cells.push(
            Cell::from(row_label).style(
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
        );

        // Gain cells
        for inp in 0..input_channels {
            let gain = matrix
                .get(out * input_channels + inp)
                .copied()
                .unwrap_or(0.0);
            let db_str = linear_to_db_string(gain);

            let is_selected = in_grid && out == app.matrix_grid_row && inp == app.matrix_grid_col;
            let style = if is_selected {
                Style::default()
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.bg_selected)
                    .add_modifier(Modifier::BOLD)
            } else if gain > 0.999 && gain < 1.001 {
                // Unity gain - highlight
                Style::default().fg(app.theme.title_color)
            } else if gain < 0.001 {
                // Silent - dim
                Style::default().fg(app.theme.fg_secondary)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };

            cells.push(Cell::from(db_str).style(style));
        }

        rows.push(Row::new(cells).height(1));
    }

    // Column widths
    let mut widths = vec![Constraint::Length(label_width)];
    for _ in 0..input_channels {
        widths.push(Constraint::Length(cell_width));
    }

    let table = Table::new(rows, widths).header(header).block(
        Block::default()
            .borders(Borders::TOP)
            .title(" Matrix Grid "),
    );

    f.render_widget(table, area);
}

/// Entry in the plugin parameter display list.
/// Can be a selectable parameter or a non-selectable section separator.
enum ParamDisplayEntry {
    /// A selectable parameter with name and formatted value
    Param(String, String),
    /// A section separator line (not selectable)
    Separator(String),
}

/// Get the parameters for a plugin as display entries.
/// Returns a mix of selectable parameters and non-selectable separators.
/// Get the parameters for a plugin as display entries.
/// Returns a mix of selectable parameters and non-selectable separators.
fn get_plugin_parameters(settings: &PluginSettings, _selected: usize) -> Vec<ParamDisplayEntry> {
    use ParamDisplayEntry::{Param, Separator};
    use crate::app::TuiEditablePlugin;

    let descriptors = settings.get_descriptors();
    let mut entries = Vec::with_capacity(descriptors.len() + 5);
    let mut last_group = String::new();

    for (i, desc) in descriptors.iter().enumerate() {
        // Add separator if group changes
        if !desc.group.is_empty() && desc.group != last_group {
            entries.push(Separator(desc.group.clone()));
            last_group = desc.group.clone();
        }

        let value = settings.get_value_as_string(i);
        let display_value = if desc.unit.is_empty() {
            value
        } else {
            format!("{} {}", value, desc.unit)
        };

        entries.push(Param(desc.name.clone(), display_value));
    }

    entries
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
        .style(Style::default().bg(app.theme.bg_primary).fg(app.theme.fg_primary))
        .title("Save Plugin Preset");

    f.render_widget(Clear, dialog_area);
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
            .style(Style::default().fg(app.theme.fg_primary))
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
            .style(Style::default().fg(app.theme.title_color))
            .block(Block::default())
            .wrap(Wrap { trim: false });

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
            .style(Style::default().fg(app.theme.fg_primary))
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
        .style(Style::default().bg(app.theme.bg_primary).fg(app.theme.fg_primary))
        .title("Load Plugin Preset");

    f.render_widget(Clear, dialog_area);
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
            .style(Style::default().fg(app.theme.fg_primary))
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
            .style(Style::default().fg(app.theme.title_color))
            .block(Block::default())
            .wrap(Wrap { trim: false });

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
            .style(Style::default().fg(app.theme.fg_primary))
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
        .style(Style::default().bg(app.theme.bg_primary).fg(app.theme.fg_primary))
        .title("Load APO EQ File");

    f.render_widget(Clear, dialog_area);
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
        .style(Style::default().fg(app.theme.fg_primary))
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
        .style(Style::default().bg(app.theme.bg_primary).fg(app.theme.fg_primary))
        .title("Load SOFA HRTF File");

    f.render_widget(Clear, dialog_area);
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
        .style(Style::default().fg(app.theme.fg_primary))
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
        .style(Style::default().bg(app.theme.bg_primary).fg(app.theme.fg_primary))
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
        .style(Style::default().fg(app.theme.fg_primary))
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
        .style(Style::default().bg(app.theme.bg_primary).fg(app.theme.fg_primary))
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
        .style(Style::default().fg(app.theme.fg_primary))
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
        .style(Style::default().bg(app.theme.bg_primary).fg(app.theme.fg_primary))
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
        .style(Style::default().fg(app.theme.fg_primary))
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
        .style(Style::default().bg(app.theme.bg_primary).fg(app.theme.fg_primary))
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
        .style(Style::default().fg(app.theme.fg_primary))
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
            ("a", "Add plugin (opens selection dialog)"),
            ("e or Enter", "Edit selected plugin"),
            ("t", "Toggle plugin enabled/disabled"),
            ("d/Delete", "Remove plugin"),
            ("u/U or Shift+↑", "Move plugin up in chain"),
            ("w/W or Shift+↓", "Move plugin down in chain"),
            ("s", "Save plugin chain to file"),
            ("l", "Load plugin chain from file"),
            ("", ""),
            ("ADD PLUGIN:", "(↑/↓ navigate, Enter select, Esc cancel)"),
            ("", ""),
            ("EDIT MODE:", "(when editing a plugin)"),
            ("↑/↓ or k/j", "Navigate parameters"),
            ("←/→ or h/l", "Adjust parameter value (small)"),
            ("[/]", "Adjust parameter value (large)"),
            ("a", "Load APO file (EQ plugins only)"),
            ("o", "Load SOFA file (Binaural only)"),
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
            .style(Style::default().bg(app.theme.bg_primary).fg(app.theme.fg_primary))
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
        let text_style = Style::default().fg(app.theme.fg_primary);
        for line in wrapped_lines {
            lines.push(Line::from(Span::styled(line, text_style)));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(""));

        // Add instructions
        lines.push(Line::from(vec![
            Span::styled("Press ", text_style),
            Span::styled(
                "ESC",
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(", ", text_style),
            Span::styled(
                "Enter",
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(", or ", text_style),
            Span::styled(
                "Space",
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to close", text_style),
        ]));

        let paragraph = Paragraph::new(lines)
            .style(Style::default().fg(app.theme.fg_primary))
            .block(Block::default())
            .style(text_style)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, inner);
    }
}

fn draw_channel_conflict_modal(f: &mut Frame, app: &App) {
    let area = f.area();
    let modal_width = 56u16.min(area.width.saturating_sub(4));
    let modal_height = 14u16.min(area.height.saturating_sub(4));
    let modal_x = (area.width - modal_width) / 2;
    let modal_y = (area.height - modal_height) / 2;

    let modal_area = Rect {
        x: modal_x,
        y: modal_y,
        width: modal_width,
        height: modal_height,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(app.theme.bg_primary).fg(app.theme.fg_primary))
        .title(" Channel Conflict ");

    f.render_widget(Clear, modal_area);
    f.render_widget(block, modal_area);

    let inner = Rect {
        x: modal_area.x + 2,
        y: modal_area.y + 2,
        width: modal_area.width.saturating_sub(4),
        height: modal_area.height.saturating_sub(3),
    };

    let text_style = Style::default().fg(app.theme.fg_primary);
    let highlight_style = Style::default()
        .fg(app.theme.accent_primary)
        .add_modifier(Modifier::BOLD);

    let mut lines = vec![];

    lines.push(Line::from(Span::styled(
        format!(
            "This track has {} channels but the upmixer",
            app.channel_conflict_track_channels
        ),
        text_style,
    )));
    lines.push(Line::from(Span::styled(
        "only supports stereo (2ch) input.",
        text_style,
    )));
    lines.push(Line::from(""));

    let options = [
        "Disable upmixer and play",
        "Remove upmixer and play",
        "Cancel playback",
    ];

    for (i, option) in options.iter().enumerate() {
        let selected = i == app.channel_conflict_selection;
        let prefix = if selected { "▸ " } else { "  " };
        let style = if selected { highlight_style } else { text_style };
        lines.push(Line::from(Span::styled(format!("{prefix}{option}"), style)));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Use ", text_style),
        Span::styled("↑↓", highlight_style),
        Span::styled(" to select, ", text_style),
        Span::styled("Enter", highlight_style),
        Span::styled(" to confirm, ", text_style),
        Span::styled("Esc", highlight_style),
        Span::styled(" to cancel", text_style),
    ]));

    let paragraph = Paragraph::new(lines)
        .style(text_style)
        .block(Block::default());

    f.render_widget(paragraph, inner);
}

fn draw_file_browser_modal(f: &mut Frame, app: &App) {
    let area = f.area();
    let modal_width = (area.width as f32 * 0.8) as u16;
    let modal_height = (area.height as f32 * 0.8) as u16;
    let modal_x = (area.width - modal_width) / 2;
    let modal_y = (area.height - modal_height) / 2;

    let modal_area = Rect {
        x: modal_x,
        y: modal_y,
        width: modal_width,
        height: modal_height,
    };

    let title = match app.input_mode {
        InputMode::BrowseSofaFile => " Select SOFA File ",
        InputMode::BrowseIrFile => " Select Impulse Response (WAV) ",
        _ => " File Browser ",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .style(Style::default().bg(app.theme.bg_primary).fg(app.theme.fg_primary))
        .title(title);

    f.render_widget(Clear, modal_area);
    f.render_widget(block, modal_area);

    let inner = modal_area.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 1,
    });

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Current directory
            Constraint::Min(0),    // File list
            Constraint::Length(1), // Help text
        ])
        .split(inner);

    // Current directory
    let dir_text = format!("Dir: {}", app.current_browser_dir.display());
    f.render_widget(
        Paragraph::new(dir_text).style(Style::default().fg(app.theme.accent_primary)),
        chunks[0],
    );

    // File list
    let items: Vec<ListItem> = app
        .file_browser_items
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let is_selected = i == app.selected_file_index;
            let icon = if path.is_dir() { "📁" } else { "📄" };
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string());

            let content = format!(" {} {}", icon, name);
            let style = if is_selected {
                Style::default()
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.bg_selected)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items);
    let mut state = ListState::default();
    state.select(Some(app.selected_file_index));

    use ratatui::widgets::StatefulWidget;
    StatefulWidget::render(list, chunks[1], f.buffer_mut(), &mut state);

    // Help text
    let help_text = "↑/↓: Navigate | Enter/→: Select/Open | ←/Back: Up | Esc: Cancel";
    f.render_widget(
        Paragraph::new(help_text)
            .style(Style::default().fg(app.theme.fg_secondary))
            .alignment(Alignment::Center),
        chunks[2],
    );
}
