use super::*;

pub(crate) fn draw_screen_boxes(f: &mut Frame, area: Rect, app: &App) {
    // Define screens with their labels
    let screens = [
        (Screen::Library, "Library"),
        (Screen::Queue, "Queue"),
        (Screen::Plugins, "Plugins"),
        (Screen::Devices, "Output Devices"),
        (Screen::Configure, "Configure"),
    ];

    // Create spans for each screen box
    let mut spans = vec![Span::raw(" ")]; // Leading space

    for (screen, label) in &screens {
        let is_active = *screen == app.current_screen;

        // Box with screen label
        let style = if is_active {
            Style::default()
                .fg(app.theme.border_color)
                .bg(app.theme.bg_primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(app.theme.fg_primary)
                .bg(app.theme.bg_primary)
        };

        if is_active {
            spans.push(Span::styled(label.to_string(), style));
            spans.push(Span::raw(" "));
        } else {
            spans.push(Span::raw("("));
            spans.push(Span::styled(
                label.chars().next().unwrap().to_string(),
                style,
            ));
            spans.push(Span::raw(")"));
            spans.push(Span::styled(label[1..].to_string(), style));
            spans.push(Span::raw(" "));
        }
    }

    let boxes = Paragraph::new(Line::from(spans))
        .style(Style::default().fg(app.theme.fg_primary))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(boxes, area);
}
