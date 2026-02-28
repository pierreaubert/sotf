use super::*;

pub(crate) fn draw_loading_screen(f: &mut Frame, app: &App) {
    let area = f.area();

    // Vertical layout: center the content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Length(1), // Title
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Loading text
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Progress bar
            Constraint::Percentage(40),
        ])
        .split(area);

    // App title centered
    let title = Paragraph::new("SOTF Player")
        .alignment(Alignment::Center)
        .style(
            Style::default()
                .fg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(title, chunks[1]);

    // "Loading..." text centered
    let loading_text = Paragraph::new("Loading...")
        .alignment(Alignment::Center)
        .style(Style::default().fg(app.theme.fg_secondary));
    f.render_widget(loading_text, chunks[3]);

    // Bouncing progress bar
    let bar_area = chunks[5];
    // Center the bar horizontally (max 40 chars wide)
    let bar_total = (bar_area.width as usize).min(40);
    if bar_total < 4 {
        return;
    }
    let bar_x = bar_area.x + (bar_area.width.saturating_sub(bar_total as u16)) / 2;
    let bar_rect = Rect {
        x: bar_x,
        y: bar_area.y,
        width: bar_total as u16,
        height: 1,
    };

    let segment_len = bar_total / 5; // Moving segment is 1/5 of total width
    let travel = bar_total.saturating_sub(segment_len);
    let cycle = if travel == 0 { 1 } else { travel * 2 };
    let tick = (app.loading_tick as usize) % cycle;
    let pos = if tick < travel { tick } else { cycle - tick };

    let mut bar_str = String::with_capacity(bar_total);
    for i in 0..bar_total {
        if i >= pos && i < pos + segment_len {
            bar_str.push('\u{2588}'); // █ Full block
        } else {
            bar_str.push('\u{2591}'); // ░ Light shade
        }
    }

    let bar = Paragraph::new(bar_str).style(Style::default().fg(app.theme.accent_primary));
    f.render_widget(bar, bar_rect);
}

