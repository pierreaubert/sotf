use super::*;

const MAX_VISIBLE_SUGGESTIONS: usize = 8;

/// Calculate the height needed for the autocomplete dropdown.
/// Returns 0 if menu is not active or no suggestions.
pub(crate) fn autocomplete_dropdown_height(app: &App) -> u16 {
    if !app.autocomplete.menu_active || app.autocomplete.suggestions.is_empty() {
        return 0;
    }
    // items + 2 for borders
    app.autocomplete.suggestions
        .len()
        .min(MAX_VISIBLE_SUGGESTIONS) as u16
        + 2
}

/// Render the autocomplete suggestions dropdown into the given area.
/// Shows a bordered list with the currently selected item highlighted.
pub(crate) fn render_autocomplete_dropdown(f: &mut Frame, area: Rect, app: &App) {
    if !app.autocomplete.menu_active || app.autocomplete.suggestions.is_empty() {
        return;
    }

    let total = app.autocomplete.suggestions.len();

    // Compute visible window (scroll so selected item is visible)
    let window_start = if app.autocomplete.index >= MAX_VISIBLE_SUGGESTIONS {
        app.autocomplete.index - MAX_VISIBLE_SUGGESTIONS + 1
    } else {
        0
    };
    let window_end = (window_start + MAX_VISIBLE_SUGGESTIONS).min(total);

    let items: Vec<ListItem> = app.autocomplete.suggestions[window_start..window_end]
        .iter()
        .enumerate()
        .map(|(i, suggestion)| {
            let absolute_idx = window_start + i;
            let style = if absolute_idx == app.autocomplete.index {
                Style::default()
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg_secondary)
            };
            // Show just the filename/last component for readability, full path if short
            ListItem::new(suggestion.as_str()).style(style)
        })
        .collect();

    let title = if total == 1 {
        "1 match".to_string()
    } else {
        format!("{}/{} matches", app.autocomplete.index + 1, total,)
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.fg_muted))
            .title(title),
    );

    f.render_widget(list, area);
}
