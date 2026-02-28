use super::*;

pub(crate) fn draw_scan_progress_dialog(f: &mut Frame, app: &App) {
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
        .style(
            Style::default()
                .bg(app.theme.bg_primary)
                .fg(app.theme.fg_primary),
        )
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

pub(crate) fn draw_maintenance_progress_dialog(f: &mut Frame, app: &App) {
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
        .style(
            Style::default()
                .bg(app.theme.bg_primary)
                .fg(app.theme.fg_primary),
        )
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

pub(crate) fn draw_replay_gain_progress_dialog(f: &mut Frame, app: &App) {
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
        .style(
            Style::default()
                .bg(app.theme.bg_primary)
                .fg(app.theme.fg_primary),
        )
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

