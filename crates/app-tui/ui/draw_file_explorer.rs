use super::*;

pub(crate) fn draw_file_explorer_modal(f: &mut Frame, app: &App) {
    let area = f.area();
    let modal_area = centered_modal_rect(area, 80, 80, 20, 6);

    let title = format!(" {} ", app.file_explorer.picker_title);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .style(
            Style::default()
                .bg(app.theme.bg_primary)
                .fg(app.theme.fg_primary),
        )
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
    let dir_text = format!("Dir: {}", app.file_explorer.dir.display());
    f.render_widget(
        Paragraph::new(dir_text).style(Style::default().fg(app.theme.accent_primary)),
        chunks[0],
    );

    // File list
    let items: Vec<ListItem> = app
        .file_explorer
        .items
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let is_selected = i == app.file_explorer.selected;
            let is_dir = path.is_dir();
            let icon = if is_dir { "/" } else { " " };
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string());

            let content = format!(" {}{}", icon, name);
            let style = if is_selected {
                Style::default()
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.bg_selected)
                    .add_modifier(Modifier::BOLD)
            } else if is_dir {
                Style::default().fg(app.theme.accent_primary)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items);
    let mut state = ListState::default();
    state.select(Some(app.file_explorer.selected));

    use ratatui::widgets::StatefulWidget;
    StatefulWidget::render(list, chunks[1], f.buffer_mut(), &mut state);

    // Help text
    let help_text =
        "Enter:Select | j/k:Navigate | l/Enter:Open dir | h:Parent | H:Hidden | Esc:Cancel";
    f.render_widget(
        Paragraph::new(help_text)
            .style(Style::default().fg(app.theme.fg_secondary))
            .alignment(Alignment::Center),
        chunks[2],
    );
}
