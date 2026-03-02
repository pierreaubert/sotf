use super::*;

pub(crate) fn draw_volume_box(f: &mut Frame, area: Rect, app: &App) {
    let volume_pct = (app.volume * 100.0) as u32;
    let key_style = Style::default().fg(app.theme.title_color);
    let volume_style = Style::default()
        .fg(app.theme.accent_primary)
        .add_modifier(Modifier::BOLD);
    let text = Line::from(vec![
        Span::styled("[-_] ", key_style),
        Span::styled(format!("{}%", volume_pct), volume_style),
        Span::styled(" [=+]", key_style),
    ]);

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Volume"))
        .alignment(Alignment::Center);

    f.render_widget(paragraph, area);
}
