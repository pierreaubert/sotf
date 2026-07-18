use super::*;

pub(crate) fn draw_title(f: &mut Frame, area: Rect, app: &App) {
    // Split title area into three parts: SOTF title, screen boxes, output device
    let ouput_width = if f.area().width > 100 { 40 } else { 20 };

    let title_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(6),           // SOTF title
            Constraint::Min(0),              // Screen boxes (expandable)
            Constraint::Length(ouput_width), // Output device
        ])
        .split(area);

    // Draw "SOTF" on the left
    let sotf_title = Paragraph::new("SotF")
        .style(
            Style::default()
                .fg(app.theme.border_color)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Left)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(app.theme.fg_primary)),
        );

    f.render_widget(sotf_title, title_chunks[0]);

    // Draw screen indicator boxes in the middle
    draw_screen_boxes(f, title_chunks[1], app);

    // Device selector on the right
    let device_text = if let Some(device) = app.get_selected_output_device() {
        device.name.to_string()
    } else {
        crate::tui_text!(app, "Default")
    };

    let device_widget = Paragraph::new(device_text)
        .style(Style::default().fg(app.theme.border_color))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(crate::tui_text!(app, "Output Device"))
                .style(Style::default().fg(app.theme.fg_primary)),
        );

    f.render_widget(device_widget, title_chunks[2]);
}
