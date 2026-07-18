use super::*;

pub(crate) fn draw_screen_boxes(f: &mut Frame, area: Rect, app: &App) {
    let text = crate::i18n::TuiTranslations::for_language(app.ui.language);
    // Define screens with their labels
    // Keep hotkeys explicit: localized labels cannot encode a shortcut by
    // capitalizing an English character in the middle of the word.
    let screens = [
        (Screen::Library, 'L'),
        (Screen::Queue, 'Q'),
        (Screen::Playlists, 'Y'),
        (Screen::Plugins, 'P'),
        (Screen::Devices, 'O'),
        (Screen::Configure, 'C'),
    ];

    // Create spans for each screen box
    let mut spans = vec![Span::raw(" ")]; // Leading space

    for (screen, hotkey) in &screens {
        let label = text.screen_name(*screen);
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
            spans.push(Span::styled(hotkey.to_string(), style));
            spans.push(Span::raw(") "));
            spans.push(Span::styled(label.to_string(), style));
            spans.push(Span::raw(" "));
        }
    }

    let boxes = Paragraph::new(Line::from(spans))
        .style(Style::default().fg(app.theme.fg_primary))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(boxes, area);
}
