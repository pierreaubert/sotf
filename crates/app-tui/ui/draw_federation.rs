use super::*;
use crate::app::{FederationMode, ADD_SOURCE_TYPE_IDX, SOURCE_TYPE_NAMES};
use sotf_audio_player::federation_config::ConnectionStatus;

pub(crate) fn draw_federation_screen(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.federation_state;

    // Check if we should show the diagnostic panel below the list
    let has_diagnostic = state
        .sources
        .get(state.selected_idx)
        .and_then(|s| state.statuses.get(&s.source_id))
        .is_some_and(|s| s.is_diagnostic());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if has_diagnostic {
            vec![
                Constraint::Length(1),  // Help
                Constraint::Min(5),     // Source list or edit form
                Constraint::Length(8),  // Diagnostic panel
            ]
        } else {
            vec![
                Constraint::Length(1), // Help
                Constraint::Min(5),    // Source list or edit form
                Constraint::Length(0), // No diagnostic panel
            ]
        })
        .split(area);

    let help_text = match state.mode {
        FederationMode::List => {
            " a=Add  e/Enter=Edit  d=Delete  t=Test  s=Scan  Space=Toggle  Esc=Back"
        }
        FederationMode::EditSource => " Up/Down=Navigate  Enter=Edit field  s/Tab=Save  Esc=Cancel",
        FederationMode::AddSource => " Up/Down=Select type  Enter=Confirm  Esc=Cancel",
    };
    let help = Paragraph::new(help_text).style(Style::default().fg(app.theme.fg_secondary));
    f.render_widget(help, chunks[0]);

    match state.mode {
        FederationMode::List => {
            draw_source_list(f, chunks[1], app);
            if has_diagnostic {
                draw_diagnostic_panel(f, chunks[2], app);
            }
        }
        FederationMode::EditSource => draw_edit_form(f, chunks[1], app),
        FederationMode::AddSource => draw_add_source(f, chunks[1], app),
    }
}

fn draw_source_list(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.federation_state;

    if state.sources.is_empty() {
        let text = Paragraph::new(" No sources configured. Press 'a' to add one.")
            .style(Style::default().fg(app.theme.fg_secondary))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Library Sources "),
            );
        f.render_widget(text, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from(" "),
        Cell::from("Name"),
        Cell::from("Type"),
        Cell::from("Priority"),
        Cell::from("Status"),
        Cell::from("Enabled"),
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
            let status = state
                .statuses
                .get(&source.source_id)
                .map_or("untested", |s| s.label());
            let status_style = match status {
                "connected" => Style::default().fg(app.theme.accent_success),
                "error" => Style::default().fg(app.theme.accent_error),
                "testing..." => Style::default().fg(app.theme.accent_warning),
                _ => Style::default().fg(app.theme.fg_secondary),
            };
            let enabled_str = if source.is_enabled { "yes" } else { "no" };

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
                Cell::from(source.connection.type_name()),
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
            .title(format!(" Library Sources ({}) ", state.sources.len())),
    );

    f.render_widget(table, area);
}

fn draw_edit_form(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.federation_state;
    let Some(edit) = &state.edit else {
        return;
    };

    let title = if edit.is_new {
        format!(" New {} Source ", edit.source.connection.type_name())
    } else {
        format!(" Edit: {} ", edit.source.display_name)
    };

    let mut lines: Vec<Line> = Vec::new();
    for i in 0..edit.field_count() {
        let is_selected = i == edit.selected_field;
        let is_editing = is_selected && edit.editing_value;

        let label = edit.field_label(i);
        let value = if is_editing {
            format!("{}|", edit.edit_buffer)
        } else {
            edit.field_value(i)
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
            ListItem::new(Line::from(Span::styled(format!("  {label}"), style)))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(idx));

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Select source type "),
        )
        .highlight_style(
            Style::default()
                .fg(app.theme.bg_primary)
                .bg(app.theme.accent_primary),
        );

    f.render_stateful_widget(list, area, &mut list_state);
}

fn draw_diagnostic_panel(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.federation_state;
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
        format!("  Connection diagnostic: {}:{}", diag.host, diag.port),
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
        .title(" Diagnostic ");
    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}
