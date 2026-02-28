use super::*;

pub(crate) fn draw_library_screen(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Help box
            Constraint::Length(3), // Search box
            Constraint::Min(0),    // Album list
        ])
        .split(area);

    draw_help_box(f, chunks[0], app, Screen::Library);
    draw_search_box(f, chunks[1], app);

    let is_focused = app.current_screen == Screen::Library;
    draw_album_list(f, chunks[2], app, is_focused);
}

