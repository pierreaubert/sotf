use super::misc::{centered_modal_rect, get_keybindings_for_screen};
pub(crate) use super::utilities::wrap_text;
pub(crate) use crate::app::{App, MetadataEditorState, Screen};
pub(crate) use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

/// Draw a standardized help box with keybindings for the given screen.
pub(crate) fn draw_help_box(f: &mut Frame, area: Rect, app: &App, screen: Screen) {
    let bindings = super::utilities::get_keybindings_for_screen(screen);
    let help_text = bindings
        .iter()
        .map(|(key, desc)| format!("{}={}", key, desc))
        .collect::<Vec<_>>()
        .join("  |  ");

    draw_help_box_with_text(f, area, app, &help_text);
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

pub(crate) fn draw_help_modal(f: &mut Frame, app: &App) {
    let area = f.area();
    let modal_area = centered_modal_rect(area, 80, 90, 20, 6);

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
                Screen::Playlists => "Playlists",
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
                Screen::Playlists => "PLAYLISTS",
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

pub(crate) fn draw_metadata_editor_modal(f: &mut Frame, app: &App) {
    let Some(editor) = &app.modal.metadata_editor else {
        return;
    };

    let area = f.area();
    let modal_area = centered_modal_rect(area, 82, 82, 60, 20);

    f.render_widget(Clear, modal_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .style(
            Style::default()
                .bg(app.theme.bg_primary)
                .fg(app.theme.fg_primary),
        )
        .title(" Edit Metadata ");
    f.render_widget(block, modal_area);

    let inner = Rect {
        x: modal_area.x + 2,
        y: modal_area.y + 1,
        width: modal_area.width.saturating_sub(4),
        height: modal_area.height.saturating_sub(2),
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(10),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(2),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("Target: ", Style::default().fg(app.theme.accent_primary)),
                Span::raw(editor.target_label.clone()),
            ]),
            Line::from(""),
        ]),
        chunks[0],
    );

    let mut field_lines = Vec::new();
    for index in 0..MetadataEditorState::FIELD_COUNT {
        let selected = index == editor.selected_field;
        let marker = if selected { ">" } else { " " };
        let value = if selected && editor.editing {
            editor.edit_buffer.as_str()
        } else {
            editor.field_value(index)
        };
        let style = if selected {
            Style::default()
                .fg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.fg_primary)
        };
        field_lines.push(Line::from(vec![
            Span::styled(marker, style),
            Span::raw(" "),
            Span::styled(
                format!("{:<13}", MetadataEditorState::field_label(index)),
                Style::default().fg(app.theme.title_color),
            ),
            Span::styled(value.to_string(), style),
        ]));
    }
    f.render_widget(
        Paragraph::new(field_lines).wrap(Wrap { trim: true }),
        chunks[1],
    );

    let preview_lines = if let Some(preview) = &editor.preview {
        let mut lines = vec![Line::from(vec![
            Span::styled("Preview: ", Style::default().fg(app.theme.accent_primary)),
            Span::raw(format!(
                "{} file(s), {} unsupported, sidecar {}",
                preview.affected_files.len(),
                preview.unsupported_writes.len(),
                if preview.sidecar_path.is_some() {
                    "yes"
                } else {
                    "no"
                }
            )),
        ])];
        for file in preview.unsupported_writes.iter().take(2) {
            lines.push(Line::from(vec![
                Span::styled("Warning: ", Style::default().fg(Color::Yellow)),
                Span::raw(format!(
                    "{}: {}",
                    file.path.display(),
                    file.reason.as_deref().unwrap_or("unsupported")
                )),
            ]));
        }
        lines
    } else {
        vec![Line::from("Preview: press p before saving")]
    };
    let mut preview_lines = preview_lines;
    if let Some(error) = &editor.error {
        preview_lines.push(Line::from(vec![
            Span::styled("Error: ", Style::default().fg(Color::Red)),
            Span::raw(error.clone()),
        ]));
    }
    f.render_widget(
        Paragraph::new(preview_lines).wrap(Wrap { trim: true }),
        chunks[2],
    );

    let mut mb_lines = vec![Line::from(vec![
        Span::styled(
            "MusicBrainz: ",
            Style::default().fg(app.theme.accent_primary),
        ),
        Span::raw(editor.search_query.clone()),
    ])];
    if let Some(error) = &editor.search_error {
        mb_lines.push(Line::from(vec![
            Span::styled("Search error: ", Style::default().fg(Color::Red)),
            Span::raw(error.clone()),
        ]));
    }
    for (idx, candidate) in editor.search_results.iter().take(3).enumerate() {
        let selected = idx == editor.selected_result;
        mb_lines.push(Line::from(vec![
            Span::styled(
                if selected { "> " } else { "  " },
                Style::default().fg(app.theme.accent_primary),
            ),
            Span::raw(format!(
                "{}  {} - {} ({})",
                candidate.score,
                candidate
                    .album_title
                    .as_deref()
                    .or(candidate.title.as_deref())
                    .unwrap_or("Untitled"),
                candidate
                    .album_artist
                    .as_deref()
                    .or(candidate.artist.as_deref())
                    .unwrap_or("Unknown"),
                candidate
                    .year
                    .map(|year| year.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            )),
        ]));
    }
    f.render_widget(
        Paragraph::new(mb_lines).wrap(Wrap { trim: true }),
        chunks[3],
    );

    let help = if editor.editing {
        " Type value | Enter=confirm | Esc=cancel"
    } else {
        " ↑↓ field | Enter edit | p preview | s save | b MusicBrainz | i import | ←→ candidate | Esc close"
    };
    f.render_widget(
        Paragraph::new(help)
            .style(Style::default().fg(app.theme.title_color))
            .alignment(Alignment::Center),
        chunks[4],
    );
}

pub(crate) fn draw_error_modal(f: &mut Frame, app: &App) {
    if let Some(error_msg) = &app.ui.error_message {
        // Create a centered modal (60% width, auto height based on content)
        let area = f.area();
        let modal_width = (area.width.saturating_mul(60) / 100).max(1);

        // Calculate required height based on error message
        let max_text_width = modal_width.saturating_sub(6) as usize; // Account for borders and padding
        let wrapped_lines = wrap_text(error_msg, max_text_width);
        let content_height = wrapped_lines.len() + 6; // Message + title + instructions + padding
        let modal_area = centered_modal_rect(area, 60, 100, 20, content_height as u16);

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
    let conflict_lines = app.modal.channel_conflicts.len().max(1);
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
            app.modal.channel_conflict_track_channels
        ),
        text_style,
    )));
    lines.push(Line::from(Span::styled("are incompatible:", text_style)));
    lines.push(Line::from(""));

    for conflict in &app.modal.channel_conflicts {
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
        let selected = i == app.modal.channel_conflict_selection;
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
