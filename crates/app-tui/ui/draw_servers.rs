use super::*;
use crate::app::ServerSection;
use sotf_audio_player::federation_config::MpdAuthMode;
use sotf_audio_player::server::normalize_certificate_fingerprint;

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

    // Split content into three columns: API | MPD | DLNA
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(chunks[1]);

    draw_api_section(f, cols[0], app);
    draw_mpd_section(f, cols[1], app);
    draw_dlna_section(f, cols[2], app);
}

fn draw_api_section(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.server_state;
    let api = &state.config.api;
    let is_active = state.selected_section == ServerSection::Api;
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

    let token_summary = api
        .auth_token
        .as_deref()
        .filter(|token| !token.trim().is_empty())
        .map(|token| format!("{}...", token.chars().take(8).collect::<String>()))
        .unwrap_or_else(|| "(auto on enable)".to_string());

    let fields: Vec<(&str, String, bool)> = vec![
        (
            "Enabled",
            if api.enabled {
                "YES".to_string()
            } else {
                "no".to_string()
            },
            true,
        ),
        ("Bind Address", api.bind_address.clone(), false),
        ("Port", api.port.to_string(), false),
        ("Friendly Name", api.friendly_name.clone(), false),
        ("Auth Token", token_summary, false),
    ];

    let mut lines = render_field_lines(
        &fields,
        app,
        is_active,
        state.selected_field,
        state.editing_value,
        &state.edit_buffer,
    );

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!(
            "  URL: {}",
            sotf_audio_player::server::sotf_api_server_url_for_settings(api)
        ),
        Style::default()
            .fg(app.theme.accent_primary)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        "  Remote apps use this API port.",
        Style::default().fg(app.theme.fg_secondary),
    )));
    lines.push(Line::from(Span::styled(
        "  MPD clients use the MPD port.",
        Style::default().fg(app.theme.fg_secondary),
    )));

    let para = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(border_type)
                .border_style(Style::default().fg(border_color))
                .title(" SOTF API "),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

fn draw_mpd_section(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.server_state;
    let mpd = &state.config.mpd;
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
            if mpd.enabled {
                "YES".to_string()
            } else {
                "no".to_string()
            },
            true, // is toggle
        ),
        ("Bind Address", mpd.bind_address.clone(), false),
        ("Port", mpd.port.to_string(), false),
        (
            "TLS",
            if mpd.tls_enabled {
                "YES".to_string()
            } else {
                "no".to_string()
            },
            true,
        ),
        (
            "Auth Mode",
            match mpd.auth_mode {
                MpdAuthMode::Certificate => "Certificate".to_string(),
                MpdAuthMode::Password => "Password".to_string(),
            },
            true,
        ),
        (
            "Password",
            mpd.password
                .as_ref()
                .map_or_else(|| "(none)".to_string(), |p| "*".repeat(p.len().min(8))),
            false,
        ),
        (
            "Trusted Clients",
            trusted_fingerprints_summary(&mpd.trusted_client_fingerprints),
            false,
        ),
    ];

    let mut lines = render_field_lines(
        &fields,
        app,
        is_active,
        state.selected_field,
        state.editing_value,
        &state.edit_buffer,
    );

    let invalid_fingerprints = invalid_trusted_fingerprint_count(&mpd.trusted_client_fingerprints);
    if invalid_fingerprints > 0 {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  ! {invalid_fingerprints} trusted client fingerprint value(s) invalid."),
            Style::default()
                .fg(app.theme.accent_warning)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            "    Use client certificate SHA-256 fingerprints.",
            Style::default().fg(app.theme.fg_secondary),
        )));
    } else if mpd.enabled
        && mpd.tls_enabled
        && mpd.auth_mode == MpdAuthMode::Certificate
        && mpd.trusted_client_fingerprints.is_empty()
    {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ! Certificate auth needs at least one trusted client",
            Style::default()
                .fg(app.theme.accent_warning)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            "    fingerprint, or switch Auth Mode to Password.",
            Style::default().fg(app.theme.fg_secondary),
        )));
    } else if mpd.enabled && mpd.auth_mode == MpdAuthMode::Password && mpd.password.is_none() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ! Password auth needs a non-empty password.",
            Style::default()
                .fg(app.theme.accent_warning)
                .add_modifier(Modifier::BOLD),
        )));
    }

    if mpd.auth_mode == MpdAuthMode::Certificate {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Pairing clients can add trust automatically.",
            Style::default().fg(app.theme.fg_secondary),
        )));
        lines.push(Line::from(Span::styled(
            "  Manual fingerprints: comma-separated SHA-256 values.",
            Style::default().fg(app.theme.fg_secondary),
        )));
    }

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

fn trusted_fingerprints_summary(fingerprints: &[String]) -> String {
    match fingerprints {
        [] => "(none)".to_string(),
        [only] => abbreviate_fingerprint(only),
        [first, ..] => format!(
            "{} (+{} more)",
            abbreviate_fingerprint(first),
            fingerprints.len() - 1
        ),
    }
}

fn abbreviate_fingerprint(fingerprint: &str) -> String {
    if fingerprint.len() <= 18 {
        fingerprint.to_string()
    } else {
        format!("{}...", &fingerprint[..18])
    }
}

fn invalid_trusted_fingerprint_count(fingerprints: &[String]) -> usize {
    fingerprints
        .iter()
        .filter(|fingerprint| normalize_certificate_fingerprint(fingerprint).is_err())
        .count()
}

fn draw_dlna_section(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.server_state;
    let dlna = &state.config.dlna;
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
            if dlna.enabled {
                "YES".to_string()
            } else {
                "no".to_string()
            },
            true,
        ),
        ("Bind Address", dlna.bind_address.clone(), false),
        ("Friendly Name", dlna.friendly_name.clone(), false),
        ("Port", dlna.port.to_string(), false),
    ];

    let lines = render_field_lines(
        &fields,
        app,
        is_active,
        state.selected_field,
        state.editing_value,
        &state.edit_buffer,
    );

    let mut note_lines = lines;
    note_lines.push(Line::from(""));
    note_lines.push(Line::from(Span::styled(
        format!(
            "  URL: {}",
            sotf_audio_player::server::dlna_server_url_for_bind(&dlna.bind_address, dlna.port)
        ),
        Style::default()
            .fg(app.theme.accent_primary)
            .add_modifier(Modifier::BOLD),
    )));
    note_lines.push(Line::from(Span::styled(
        "  Bind 0.0.0.0 listens on all interfaces.",
        Style::default().fg(app.theme.fg_secondary),
    )));
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

            Line::from(Span::styled(format!("{arrow}{label:<18} {display}"), style))
        })
        .collect()
}
