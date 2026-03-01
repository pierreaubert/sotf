mod utilities;
mod draw;
mod draw_album_list;
mod draw_configure;
mod draw_directory;
mod draw_file_explorer;
mod draw_graphs;
mod draw_headphoneeq;
mod draw_library_screen;
mod draw_loading;
mod draw_meters;
mod draw_plugins;
mod draw_progress;
mod draw_queue;
mod draw_roomeq;
mod draw_screen_boxes;
mod draw_search_box;
mod draw_spinorama;
mod draw_status_bar;
mod draw_title;
mod draw_transport;
mod draw_volume;

// Re-export the main draw entry point
pub use draw::draw;

// Re-export all draw functions so sibling submodules can use them via `use super::*`
pub(crate) use draw_album_list::*;
pub(crate) use draw_configure::*;
pub(crate) use draw_directory::*;
pub(crate) use draw_file_explorer::*;
pub(crate) use draw_graphs::*;
pub(crate) use draw_headphoneeq::*;
pub(crate) use draw_library_screen::*;
pub(crate) use draw_loading::*;
pub(crate) use draw_meters::*;
pub(crate) use draw_plugins::*;
pub(crate) use draw_progress::*;
pub(crate) use draw_queue::*;
pub(crate) use draw_roomeq::*;
pub(crate) use draw_screen_boxes::*;
pub(crate) use draw_search_box::*;
pub(crate) use draw_spinorama::*;
pub(crate) use draw_status_bar::*;
pub(crate) use draw_title::*;
pub(crate) use draw_transport::*;
pub(crate) use draw_volume::*;

// Common imports shared by all draw submodules via `use super::*`
pub(crate) use crate::app::{App, FocusedPane, InputMode, LibraryViewMode, MatrixEditMode, Screen, TreeItem};
pub(crate) use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Axis, Block, BorderType, Borders, Cell, Chart, Clear, Dataset, GraphType, List, ListItem,
        ListState, Paragraph, Row, Table, Wrap,
    },
};
pub(crate) use sotf_audio_player::{
    PluginSettings, PluginType, detect_matrix_preset, get_channel_label, linear_to_db_string,
};

// Re-export utility functions for submodules
pub(crate) use utilities::{
    clean_text, clean_track_name, format_channel_count, truncate_with_ellipsis, wrap_text,
};

/// Draw a standardized help box with keybindings for the given screen.
pub(crate) fn draw_help_box(f: &mut Frame, area: Rect, app: &App, screen: Screen) {
    let bindings = utilities::get_keybindings_for_screen(screen);
    let help_text = bindings
        .iter()
        .map(|(key, desc)| format!("{}={}", key, desc))
        .collect::<Vec<_>>()
        .join("  |  ");

    draw_help_box_with_text(f, area, app, &help_text);
}

/// Returns the screen area below the title bar (rows 3+), used for modals.
pub(crate) fn below_title_bar(f: &Frame) -> Rect {
    let area = f.area();
    let title_height = 3u16;
    Rect {
        x: area.x,
        y: area.y + title_height,
        width: area.width,
        height: area.height.saturating_sub(title_height),
    }
}

/// Draw a help box with custom text, inside a bordered block.
pub(crate) fn draw_help_box_with_text(f: &mut Frame, area: Rect, app: &App, text: &str) {
    let help = Paragraph::new(text.to_string())
        .style(Style::default().fg(app.theme.title_color))
        .alignment(Alignment::Left)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(app.theme.border_color))
                .title(" Help "),
        );
    f.render_widget(help, area);
}

// Minimum height threshold for showing both library and queue simultaneously
pub(crate) const DUAL_VIEW_HEIGHT_THRESHOLD: u16 = 40;

pub(crate) fn draw_help_modal(f: &mut Frame, app: &App) {
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
        .style(
            Style::default()
                .bg(app.theme.bg_primary)
                .fg(app.theme.fg_primary),
        )
        .title(format!(
            "Help - {} Screen (Press ESC or ? to close)",
            match app.current_screen {
                Screen::Loading => "Loading",
                Screen::Library => "Library",
                Screen::Queue => "Queue",
                Screen::Plugins => "Plugins",
                Screen::Devices => "Devices",
                Screen::Configure => "Configure",
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
        Span::styled("  L/Q/P/O/C", Style::default().fg(app.theme.accent_primary)),
        Span::raw("  Jump to Library/Queue/Plugins/Devices/Configure"),
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
                Screen::Loading => "LOADING",
                Screen::Library => "LIBRARY",
                Screen::Queue => "QUEUE",
                Screen::Plugins => "PLUGINS",
                Screen::Devices => "DEVICES",
                Screen::Configure => "CONFIGURE",
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
        Screen::Loading => vec![],
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
        Screen::Configure => vec![
            ("1", "Directories sub-screen"),
            ("2", "Recording sub-screen"),
            ("3", "Room EQ sub-screen"),
            ("4", "Headphone EQ sub-screen"),
            ("5", "Spinorama EQ sub-screen"),
            ("", ""),
            ("DIRECTORIES:", "(when on Directories sub-screen)"),
            ("↑/↓ or k/j", "Navigate directories"),
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

pub(crate) fn draw_error_modal(f: &mut Frame, app: &App) {
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
            .style(
                Style::default()
                    .bg(app.theme.bg_primary)
                    .fg(app.theme.fg_primary),
            )
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

pub(crate) fn draw_channel_conflict_modal(f: &mut Frame, app: &App) {
    let area = f.area();
    // Dynamic height: 2 (header) + 1 (blank) + conflicts + 1 (blank) + 3 (options) + 1 (blank) + 1 (help) + 3 (border)
    let conflict_lines = app.channel_conflicts.len().max(1);
    let content_height = 2 + 1 + conflict_lines + 1 + 3 + 1 + 1;
    let modal_width = 56u16.min(area.width.saturating_sub(4));
    let modal_height = (content_height as u16 + 3).min(area.height.saturating_sub(4));
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
        .style(
            Style::default()
                .bg(app.theme.bg_primary)
                .fg(app.theme.fg_primary),
        )
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
    let warn_style = Style::default().fg(Color::Yellow);

    let mut lines = vec![];

    lines.push(Line::from(Span::styled(
        format!(
            "This track has {} channels but these plugins",
            app.channel_conflict_track_channels
        ),
        text_style,
    )));
    lines.push(Line::from(Span::styled(
        "are incompatible:",
        text_style,
    )));
    lines.push(Line::from(""));

    for conflict in &app.channel_conflicts {
        lines.push(Line::from(Span::styled(
            format!(
                "  {} (requires {}ch, got {}ch)",
                conflict.plugin_type.name(),
                conflict.required_channels,
                conflict.actual_channels
            ),
            warn_style,
        )));
    }

    lines.push(Line::from(""));

    let options = [
        "Suspend incompatible and play",
        "Remove incompatible and play",
        "Cancel playback",
    ];

    for (i, option) in options.iter().enumerate() {
        let selected = i == app.channel_conflict_selection;
        let prefix = if selected { "▸ " } else { "  " };
        let style = if selected {
            highlight_style
        } else {
            text_style
        };
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
