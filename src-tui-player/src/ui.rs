use crate::app::{App, InputMode, Screen};
use crate::plugins::{PluginSettings, PluginType};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

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
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(85), // Main content
            Constraint::Percentage(15), // Right column (LUFS, level meter, volume)
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
    let title_text = match app.current_screen {
        Screen::Library => "SOTF Music Player - Library",
        Screen::DirectoryManager => "SOTF Music Player - Directories",
        Screen::Queue => "SOTF Music Player - Queue",
        Screen::Plugins => "SOTF Music Player - Audio Plugins",
        Screen::Devices => "SOTF Music Player - Output Devices",
    };

    // Split title area into left (title) and right (device selector)
    let title_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(70), // Title
            Constraint::Percentage(30), // Device selector
        ])
        .split(area);

    let title = Paragraph::new(title_text)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM));

    f.render_widget(title, title_chunks[0]);

    // Device selector
    let device_text = if let Some(device) = app.get_selected_output_device() {
        format!("Out: {}", device.name)
    } else {
        "Out: Default".to_string()
    };

    let device_widget = Paragraph::new(device_text)
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("Output Device"));

    f.render_widget(device_widget, title_chunks[1]);
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

    let search_box = Paragraph::new(search_text)
        .style(input_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Search Albums (Press '/' to search, ESC to exit search)"),
        );

    f.render_widget(search_box, area);
}

fn draw_album_list(f: &mut Frame, area: Rect, app: &App) {
    use crate::app::{LibraryViewMode, TreeItem};

    match app.library_view_mode {
        LibraryViewMode::Flat => {
            let albums = app.filtered_albums();

            let items: Vec<ListItem> = albums
                .iter()
                .enumerate()
                .map(|(i, album)| {
                    let content = format!("{}", album.display_name());
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

            let list = List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(
                        "Albums ({}) - 'a' to add, 't' to toggle tree view",
                        albums.len()
                    )),
            );

            f.render_widget(list, area);
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
                            let content = format!("{}{}", prefix, name);
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
                                let content = format!("  └─ {}", album.title);
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

            let list = List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(
                        "Artists ({}) - 'h/l' to expand/collapse, 'a' to add, 't' to toggle view",
                        app.artist_tree.len()
                    )),
            );

            f.render_widget(list, area);
        }
    }
}

fn draw_directory_manager(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input box
            Constraint::Min(0),    // Directory list
        ])
        .split(area);

    // Input box for adding directories
    let input_style = if app.input_mode == InputMode::AddDirectory {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let input_text = if app.input_mode == InputMode::AddDirectory {
        format!("Path: {}█", app.directory_input)
    } else {
        "Path: (Press 'a' to add directory)".to_string()
    };

    let input_box = Paragraph::new(input_text)
        .style(input_style)
        .block(Block::default().borders(Borders::ALL).title("Add Directory"));

    f.render_widget(input_box, chunks[0]);

    // Directory list
    let items: Vec<ListItem> = app
        .library
        .directories
        .iter()
        .enumerate()
        .map(|(i, dir)| {
            let content = dir.display().to_string();
            let style = if i == app.selected_directory_index {
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

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Directories - Press 'd' to remove, 's' to scan"),
    );

    f.render_widget(list, chunks[1]);
}

fn draw_queue_screen(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .queue
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_current = app.current_queue_index == Some(i);
            let is_selected = i == app.selected_queue_index;

            let mut content = format!("{}", item.album.display_name());
            if is_current {
                let track_info = format!(
                    " [Track {}/{}]",
                    item.current_track_index + 1,
                    item.album.tracks.len()
                );
                content.push_str(&track_info);
                if app.is_playing {
                    content = format!("▶ {}", content);
                } else {
                    content = format!("⏸ {}", content);
                }
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

            ListItem::new(content).style(style)
        })
        .collect();

    let title = if app.queue.is_empty() {
        "Queue (empty) - Add albums from library".to_string()
    } else {
        format!(
            "Queue ({}) - Press 'p' to play, SPACE to pause, 'n' for next, 'd' to remove",
            app.queue.len()
        )
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title));

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
                format!(
                    " ({}ch, {}Hz)",
                    config.channels, config.sample_rate
                )
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
        format!("Output Devices ({}) - Use ↑/↓ to select, Enter to apply", app.output_devices.len())
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(list, chunks[1]);
}

fn draw_meters_column(f: &mut Frame, area: Rect, app: &App) {
    // Split the right column into 3 boxes
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),  // LUFS box
            Constraint::Min(0),     // Level meter box (expandable)
            Constraint::Length(5),  // Volume box
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
                Span::styled(format!("{} LUFS", momentary), Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::raw("S: "),
                Span::styled(format!("{} LUFS", shortterm), Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::raw("Pk: "),
                Span::styled(format!("{:>5.1} dBFS", peak_db), Style::default().fg(Color::Red)),
            ]),
        ]
    } else {
        vec![
            Line::from("M:   -∞ LUFS"),
            Line::from("S:   -∞ LUFS"),
            Line::from("Pk:  -∞ dBFS"),
        ]
    };

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Loudness"));

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

        // Calculate meter dimensions
        let meter_height = inner.height as usize;
        let channel_width = (inner.width as usize) / num_channels.max(1);

        // Draw each channel as a vertical meter
        for (ch_idx, &peak) in loudness.channel_peaks.iter().enumerate() {
            let peak_db = 20.0 * peak.max(0.0001).log10();

            // Calculate meter fill (0 dB = full, -60 dB = empty)
            let fill_ratio = ((peak_db + 60.0) / 60.0).clamp(0.0, 1.0);
            let filled_rows = (fill_ratio * meter_height as f64).round() as usize;

            let ch_x = inner.x + (ch_idx * channel_width) as u16;
            let ch_width = channel_width.min((inner.width as usize - ch_idx * channel_width).min(channel_width)) as u16;

            // Draw vertical meter from bottom to top
            for row_idx in 0..meter_height {
                let y = inner.y + inner.height - 1 - row_idx as u16;

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
                    let span = Span::styled(bar, Style::default().fg(color));
                    f.render_widget(
                        Paragraph::new(Line::from(span)),
                        Rect {
                            x: ch_x,
                            y,
                            width: ch_width.saturating_sub(1),
                            height: 1,
                        },
                    );
                } else {
                    // Draw empty bar
                    let bar = "░".repeat(ch_width.saturating_sub(1) as usize);
                    let span = Span::styled(bar, Style::default().fg(Color::DarkGray));
                    f.render_widget(
                        Paragraph::new(Line::from(span)),
                        Rect {
                            x: ch_x,
                            y,
                            width: ch_width.saturating_sub(1),
                            height: 1,
                        },
                    );
                }
            }

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
    } else {
        let paragraph = Paragraph::new("No audio")
            .block(Block::default().borders(Borders::ALL).title("Levels"))
            .alignment(Alignment::Center);
        f.render_widget(paragraph, area);
    }
}

fn draw_volume_box(f: &mut Frame, area: Rect, app: &App) {
    let volume_pct = (app.volume * 100.0) as u32;
    let text = Line::from(vec![
        Span::styled(
            format!("{}%", volume_pct),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
    ]);

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Volume"))
        .alignment(Alignment::Center);

    f.render_widget(paragraph, area);
}

fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let mut status_spans = vec![Span::raw(" ")];

    // Show status message if available
    if let Some(msg) = &app.status_message {
        status_spans.push(Span::styled(
            format!("{} | ", msg),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(idx) = app.current_queue_index {
        if let Some(item) = app.queue.get(idx) {
            if let Some(track) = item.current_track() {
                let track_name = track
                    .title
                    .as_deref()
                    .unwrap_or_else(|| track.path.file_name().unwrap().to_str().unwrap());
                status_spans.push(Span::styled(
                    format!("Now: {}", track_name),
                    Style::default().fg(Color::Green),
                ));
                status_spans.push(Span::raw(" | "));
            }
        }
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
    status_spans.push(Span::styled("Shift+↑/↓", Style::default().fg(Color::Yellow)));
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
            .title(format!("Edit {} Plugin (ESC to close)", plugin.plugin_type().name()));

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
            center_level_db,
            lfe_level_db,
            surround_delay_ms,
        } => vec![
            ("Center Level".to_string(), format!("{:.1} dB", center_level_db)),
            ("LFE Level".to_string(), format!("{:.1} dB", lfe_level_db)),
            ("Surround Delay".to_string(), format!("{:.1} ms", surround_delay_ms)),
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
            ("Target LUFS".to_string(), format!("{:.1} LUFS", target_lufs)),
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
                params.push((
                    format!("Filter {} Q", i + 1),
                    format!("{:.2}", filter.q),
                ));
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
    // Create a centered dialog (50% width, 20% height)
    let area = f.area();
    let dialog_width = (area.width as f32 * 0.5) as u16;
    let dialog_height = 7;
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
        .title("Save Plugin Chain");

    f.render_widget(block, dialog_area);

    // Inner area for text
    let inner = Rect {
        x: dialog_area.x + 1,
        y: dialog_area.y + 1,
        width: dialog_area.width.saturating_sub(2),
        height: dialog_area.height.saturating_sub(2),
    };

    let mut lines = Vec::new();
    lines.push(Line::from("Enter filename (e.g., plugins.json):"));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("> ", Style::default().fg(Color::Cyan)),
        Span::raw(&app.plugin_file_input),
        Span::styled("_", Style::default().fg(Color::Green)),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from("Press Enter to save, ESC to cancel"));

    let paragraph = Paragraph::new(lines)
        .block(Block::default())
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, inner);
}

fn draw_load_plugins_dialog(f: &mut Frame, app: &App) {
    // Create a centered dialog (50% width, 20% height)
    let area = f.area();
    let dialog_width = (area.width as f32 * 0.5) as u16;
    let dialog_height = 7;
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
        .title("Load Plugin Chain");

    f.render_widget(block, dialog_area);

    // Inner area for text
    let inner = Rect {
        x: dialog_area.x + 1,
        y: dialog_area.y + 1,
        width: dialog_area.width.saturating_sub(2),
        height: dialog_area.height.saturating_sub(2),
    };

    let mut lines = Vec::new();
    lines.push(Line::from("Enter filename (e.g., plugins.json):"));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("> ", Style::default().fg(Color::Cyan)),
        Span::raw(&app.plugin_file_input),
        Span::styled("_", Style::default().fg(Color::Green)),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from("Press Enter to load, ESC to cancel"));

    let paragraph = Paragraph::new(lines)
        .block(Block::default())
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, inner);
}
