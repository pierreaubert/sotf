use super::*;
use crate::app::ServerSection;

pub(crate) fn draw_servers_screen(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.server_state;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Help
            Constraint::Min(5),    // Content
        ])
        .split(area);

    let help_text = if state.editing_value {
        " Type value, Enter=confirm  Esc=cancel"
    } else {
        " Tab=Switch section  Up/Down=Navigate  Enter/Space=Toggle/Edit  Esc=Back"
    };
    let help = Paragraph::new(help_text).style(Style::default().fg(app.theme.fg_secondary));
    f.render_widget(help, chunks[0]);

    // Split content into two columns: MPD | DLNA
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    draw_mpd_section(f, cols[0], app);
    draw_dlna_section(f, cols[1], app);
}

fn draw_mpd_section(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.server_state;
    let is_active = state.selected_section == ServerSection::Mpd;
    let border_type = if is_active {
        BorderType::Double
    } else {
        BorderType::Rounded
    };
    let border_color = if is_active {
        app.theme.accent_primary
    } else {
        app.theme.border_color
    };

    let fields: Vec<(&str, String, bool)> = vec![
        (
            "Enabled",
            if state.config.mpd.enabled {
                "YES".to_string()
            } else {
                "no".to_string()
            },
            true, // is toggle
        ),
        (
            "Bind Address",
            state.config.mpd.bind_address.clone(),
            false,
        ),
        ("Port", state.config.mpd.port.to_string(), false),
        (
            "TLS",
            if state.config.mpd.tls_enabled {
                "YES".to_string()
            } else {
                "no".to_string()
            },
            true,
        ),
        (
            "Password",
            state
                .config
                .mpd
                .password
                .as_ref()
                .map_or_else(|| "(none)".to_string(), |p| "*".repeat(p.len().min(8))),
            false,
        ),
    ];

    let mut lines = render_field_lines(&fields, app, is_active, state.selected_field, state.editing_value, &state.edit_buffer);

    // TLS fingerprint
    if let Some(ref fp) = state.tls_fingerprint {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  Fingerprint: {}", &fp[..fp.len().min(23)]),
            Style::default().fg(app.theme.fg_secondary),
        )));
    }

    let para = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(border_type)
                .border_style(Style::default().fg(border_color))
                .title(" MPD Server "),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

fn draw_dlna_section(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.server_state;
    let is_active = state.selected_section == ServerSection::Dlna;
    let border_type = if is_active {
        BorderType::Double
    } else {
        BorderType::Rounded
    };
    let border_color = if is_active {
        app.theme.accent_primary
    } else {
        app.theme.border_color
    };

    let fields: Vec<(&str, String, bool)> = vec![
        (
            "Enabled",
            if state.config.dlna.enabled {
                "YES".to_string()
            } else {
                "no".to_string()
            },
            true,
        ),
        (
            "Friendly Name",
            state.config.dlna.friendly_name.clone(),
            false,
        ),
        ("Port", state.config.dlna.port.to_string(), false),
    ];

    let lines = render_field_lines(&fields, app, is_active, state.selected_field, state.editing_value, &state.edit_buffer);

    let mut note_lines = lines;
    note_lines.push(Line::from(""));
    note_lines.push(Line::from(Span::styled(
        "  (DLNA uses plain HTTP for",
        Style::default().fg(app.theme.fg_secondary),
    )));
    note_lines.push(Line::from(Span::styled(
        "   device compatibility)",
        Style::default().fg(app.theme.fg_secondary),
    )));

    let para = Paragraph::new(note_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(border_type)
                .border_style(Style::default().fg(border_color))
                .title(" DLNA Server "),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

fn render_field_lines<'a>(
    fields: &[(&str, String, bool)],
    app: &App,
    is_active_section: bool,
    selected_field: usize,
    editing_value: bool,
    edit_buffer: &str,
) -> Vec<Line<'a>> {
    fields
        .iter()
        .enumerate()
        .map(|(i, (label, value, _is_toggle))| {
            let is_selected = is_active_section && i == selected_field;
            let is_editing = is_selected && editing_value;

            let display = if is_editing {
                format!("{edit_buffer}|")
            } else {
                value.clone()
            };

            let style = if is_editing {
                Style::default()
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.bg_selected)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };

            let arrow = if is_editing {
                "* "
            } else if is_selected {
                "> "
            } else {
                "  "
            };

            Line::from(Span::styled(
                format!("{arrow}{label:<18} {display}"),
                style,
            ))
        })
        .collect()
}
