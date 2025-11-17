use crate::app::{App, InputMode, Screen};
use crate::plugins::PluginType;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
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

    // Main content based on current screen
    match app.current_screen {
        Screen::Library => draw_library_screen(f, chunks[1], app),
        Screen::DirectoryManager => draw_directory_manager(f, chunks[1], app),
        Screen::Queue => draw_queue_screen(f, chunks[1], app),
        Screen::Plugins => draw_plugins_screen(f, chunks[1], app),
    }

    // Status bar
    draw_status_bar(f, chunks[2], app);
}

fn draw_title(f: &mut Frame, area: Rect, app: &App) {
    let title_text = match app.current_screen {
        Screen::Library => "SOTF Music Player - Library",
        Screen::DirectoryManager => "SOTF Music Player - Directories",
        Screen::Queue => "SOTF Music Player - Queue",
        Screen::Plugins => "SOTF Music Player - Audio Plugins",
    };

    let title = Paragraph::new(title_text)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(title, area);
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
            .title(format!("Albums ({}) - Press 'a' to add to queue, 'q' to view queue", albums.len())),
    );

    f.render_widget(list, area);
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
        " | Press 'a' to add plugins from the right panel"
    } else {
        " | 't'=toggle, 'd'=remove, '↑/↓'=move, 'a'=add plugin"
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

fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let mut status_spans = vec![
        Span::raw(" "),
        Span::styled(
            format!("Vol: {:.0}%", app.volume * 100.0),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw(" | "),
    ];

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
    status_spans.push(Span::styled("L", Style::default().fg(Color::Cyan)));
    status_spans.push(Span::raw("=Library "));
    status_spans.push(Span::styled("D", Style::default().fg(Color::Cyan)));
    status_spans.push(Span::raw("=Directories "));
    status_spans.push(Span::styled("Q", Style::default().fg(Color::Cyan)));
    status_spans.push(Span::raw("=Queue "));
    status_spans.push(Span::styled("P", Style::default().fg(Color::Cyan)));
    status_spans.push(Span::raw("=Plugins "));
    status_spans.push(Span::styled("ESC", Style::default().fg(Color::Red)));
    status_spans.push(Span::raw("=Quit "));

    let status_text = Line::from(status_spans);

    let status = Paragraph::new(status_text)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: true });

    f.render_widget(status, area);
}
