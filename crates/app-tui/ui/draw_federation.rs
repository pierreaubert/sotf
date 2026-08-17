use super::*;
use crate::app::{ADD_SOURCE_TYPE_IDX, FederationMode, SOURCE_TYPE_NAMES};
use sotf_audio_player::federation_config::ConnectionStatus;

pub(crate) fn draw_federation_screen(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.federation.state;

    // Check if we should show a panel below the list: an in-progress
    // service login takes precedence over the connection diagnostic.
    let has_login = state.login.is_some();
    let has_diagnostic = state
        .sources
        .get(state.selected_idx)
        .and_then(|s| state.statuses.get(&s.source_id))
        .is_some_and(|s| s.is_diagnostic());
    let has_panel = has_login || has_diagnostic;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if has_panel {
            vec![
                Constraint::Length(1), // Help
                Constraint::Min(5),    // Source list or edit form
                Constraint::Length(8), // Login / diagnostic panel
            ]
        } else {
            vec![
                Constraint::Length(1), // Help
                Constraint::Min(5),    // Source list or edit form
                Constraint::Length(0), // No panel
            ]
        })
        .split(area);

    let help_text = crate::tui_text!(
        app,
        match state.mode {
            FederationMode::List => {
                if cfg!(any(feature = "tidal", feature = "spotify")) {
                    " a=Add  e/Enter=Edit  d=Delete  t=Test+Scan  s=Scan  l=Login  L=Logout  Space=Toggle  Esc=Back"
                } else {
                    " a=Add  e/Enter=Edit  d=Delete  t=Test+Scan  s=Scan  Space=Toggle  Esc=Back"
                }
            }
            FederationMode::EditSource =>
                " Up/Down=Navigate  Enter=Edit field  s/Tab=Save  Esc=Cancel",
            FederationMode::AddSource => " Up/Down=Select type  Enter=Confirm  Esc=Cancel",
        }
    );
    let help = Paragraph::new(help_text).style(Style::default().fg(app.theme.fg_secondary));
    f.render_widget(help, chunks[0]);

    match state.mode {
        FederationMode::List => {
            draw_source_list(f, chunks[1], app);
            if has_login {
                draw_login_panel(f, chunks[2], app);
            } else if has_diagnostic {
                draw_diagnostic_panel(f, chunks[2], app);
            }
        }
        FederationMode::EditSource => draw_edit_form(f, chunks[1], app),
        FederationMode::AddSource => draw_add_source(f, chunks[1], app),
    }
}

fn draw_source_list(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.federation.state;

    if state.sources.is_empty() {
        let text = Paragraph::new(crate::tui_text!(
            app,
            " No sources configured. Press 'a' to add one."
        ))
        .style(Style::default().fg(app.theme.fg_secondary))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(crate::tui_text!(app, " Library Sources ")),
        );
        f.render_widget(text, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from(" "),
        Cell::from(crate::tui_text!(app, "Name")),
        Cell::from(crate::tui_text!(app, "Type")),
        Cell::from(crate::tui_text!(app, "Priority")),
        Cell::from(crate::tui_text!(app, "Status")),
        Cell::from(crate::tui_text!(app, "Enabled")),
    ])
    .style(
        Style::default()
            .fg(app.theme.accent_primary)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = state
        .sources
        .iter()
        .enumerate()
        .map(|(i, source)| {
            let is_selected = i == state.selected_idx;
            let status_source = state
                .statuses
                .get(&source.source_id)
                .map_or("untested", |s| s.label());
            let status_style = match status_source {
                "connected" => Style::default().fg(app.theme.accent_success),
                "error" => Style::default().fg(app.theme.accent_error),
                "testing..." => Style::default().fg(app.theme.accent_warning),
                _ => Style::default().fg(app.theme.fg_secondary),
            };
            let status = crate::tui_text!(app, status_source);
            let enabled_str = if source.is_enabled {
                crate::tui_text!(app, "yes")
            } else {
                crate::tui_text!(app, "no")
            };

            let style = if is_selected {
                Style::default()
                    .fg(app.theme.bg_primary)
                    .bg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };

            Row::new(vec![
                Cell::from(if is_selected { ">" } else { " " }),
                Cell::from(source.display_name.as_str()),
                Cell::from(crate::tui_text!(app, source.connection.type_name())),
                Cell::from(format!("{}", source.priority)),
                Cell::from(Span::styled(
                    status,
                    if is_selected { style } else { status_style },
                )),
                Cell::from(enabled_str),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Min(15),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(crate::tui_text!(
                app,
                format!(" Library Sources ({}) ", state.sources.len())
            )),
    );

    f.render_widget(table, area);
}

fn draw_edit_form(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.federation.state;
    let Some(edit) = &state.edit else {
        return;
    };

    let title = if edit.is_new {
        crate::tui_text!(
            app,
            format!(" New {} Source ", edit.source.connection.type_name())
        )
    } else {
        crate::tui_text!(app, format!(" Edit: {} ", edit.source.display_name))
    };

    let mut lines: Vec<Line> = Vec::new();
    for i in 0..edit.field_count() {
        let is_selected = i == edit.selected_field;
        let is_editing = is_selected && edit.editing_value;

        let label = crate::tui_text!(app, edit.field_label(i));
        let value = if is_editing {
            format!("{}|", edit.edit_buffer)
        } else {
            crate::i18n::TuiTranslations::for_language(app.ui.language)
                .dynamic_or_verbatim(&edit.field_value(i))
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

        lines.push(Line::from(Span::styled(
            format!("{arrow}{label:<20} {value}"),
            style,
        )));
    }

    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

fn draw_add_source(f: &mut Frame, area: Rect, app: &App) {
    let idx = ADD_SOURCE_TYPE_IDX.load(std::sync::atomic::Ordering::Relaxed);

    let items: Vec<ListItem> = SOURCE_TYPE_NAMES
        .iter()
        .enumerate()
        .map(|(i, (_, label))| {
            let is_selected = i == idx;
            let style = if is_selected {
                Style::default()
                    .fg(app.theme.bg_primary)
                    .bg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };
            ListItem::new(Line::from(Span::styled(
                format!("  {}", crate::tui_text!(app, *label)),
                style,
            )))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(idx));

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(crate::tui_text!(app, " Select source type ")),
        )
        .highlight_style(
            Style::default()
                .fg(app.theme.bg_primary)
                .bg(app.theme.accent_primary),
        );

    f.render_stateful_widget(list, area, &mut list_state);
}

fn draw_diagnostic_panel(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.federation.state;
    let source = match state.sources.get(state.selected_idx) {
        Some(s) => s,
        None => return,
    };
    let diag = match state.statuses.get(&source.source_id) {
        Some(ConnectionStatus::Diagnostic(d)) => d,
        _ => return,
    };

    use sotf_audio_player::federation_config::StepResult;

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        crate::tui_text!(
            app,
            format!("  Connection diagnostic: {}:{}", diag.host, diag.port)
        ),
        Style::default()
            .fg(app.theme.fg_secondary)
            .add_modifier(Modifier::BOLD),
    )));

    for (label, result) in diag.steps() {
        let (icon, color) = match result {
            StepResult::Ok(_) => (" OK  ", app.theme.accent_success),
            StepResult::Fail(_) => (" FAIL", app.theme.accent_error),
            StepResult::Skipped(_) => (" SKIP", app.theme.fg_secondary),
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {icon}"),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {label:<14}"),
                Style::default().fg(app.theme.fg_secondary),
            ),
            Span::styled(result.message(), Style::default().fg(color)),
        ]));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(crate::tui_text!(app, " Diagnostic "));
    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}

/// Panel shown while a Tidal/Spotify login is in progress: the device-code
/// prompt (Tidal) or the OAuth URL fallback (Spotify), plus a waiting hint.
/// Never renders tokens — only the public verification URL and user code.
fn draw_login_panel(f: &mut Frame, area: Rect, app: &App) {
    use crate::app::ServiceLoginStatus;

    let Some(login) = &app.federation.state.login else {
        return;
    };

    let mut lines: Vec<Line> = Vec::new();
    match &login.status {
        ServiceLoginStatus::Starting => {
            lines.push(Line::from(Span::styled(
                crate::tui_text!(app, "  Starting login..."),
                Style::default().fg(app.theme.fg_secondary),
            )));
        }
        ServiceLoginStatus::TidalDevicePrompt {
            verification_url,
            user_code,
            expires_in_secs,
            started,
        } => {
            let remaining = expires_in_secs.saturating_sub(started.elapsed().as_secs());
            lines.push(Line::from(Span::styled(
                crate::tui_text!(app, format!("  Visit: {verification_url}")),
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::styled(
                crate::tui_text!(
                    app,
                    format!("  Code: {user_code} (expires in {remaining}s)")
                ),
                Style::default()
                    .fg(app.theme.accent_warning)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::styled(
                crate::tui_text!(app, "  Waiting for authorization... (l = cancel)"),
                Style::default().fg(app.theme.fg_secondary),
            )));
        }
        ServiceLoginStatus::SpotifyOAuth { url, .. } => {
            lines.push(Line::from(Span::styled(
                crate::tui_text!(app, "  Complete the sign-in in your browser."),
                Style::default().fg(app.theme.fg_primary),
            )));
            lines.push(Line::from(Span::styled(
                crate::tui_text!(app, format!("  URL: {url}")),
                Style::default().fg(app.theme.fg_secondary),
            )));
            lines.push(Line::from(Span::styled(
                crate::tui_text!(app, "  Waiting for the browser callback... (l = cancel)"),
                Style::default().fg(app.theme.fg_secondary),
            )));
        }
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(crate::tui_text!(app, " Service Login "));
    let para = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}
