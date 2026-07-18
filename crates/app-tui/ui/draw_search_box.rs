use super::*;

pub(crate) fn draw_search_box(f: &mut Frame, area: Rect, app: &App) {
    use crate::app::{ChannelFilter, LibrarySortOrder};

    let input_style = if app.input_mode == InputMode::Search {
        Style::default().fg(app.theme.title_color)
    } else {
        Style::default().fg(app.theme.fg_primary)
    };

    let search_text = if app.input_mode == InputMode::Search {
        crate::tui_text!(app, format!("Search: {}█", app.library_view.search_query))
    } else {
        crate::tui_text!(app, format!("Search: {}", app.library_view.search_query))
    };

    // Display current sort order (will be rendered in green)
    let sort_order_str = match app.library_view.sort_order {
        LibrarySortOrder::Year => crate::tui_text!(app, "Year"),
        LibrarySortOrder::Genre => crate::tui_text!(app, "Genre"),
        LibrarySortOrder::Artist => crate::tui_text!(app, "Artist"),
        LibrarySortOrder::Album => crate::tui_text!(app, "Album"),
        LibrarySortOrder::Tracks => crate::tui_text!(app, "Tracks"),
        LibrarySortOrder::Composer => crate::tui_text!(app, "Composer"),
        LibrarySortOrder::Popularity => crate::tui_text!(app, "Popularity"),
    };

    // Display current channel filter (will be rendered in green)
    let filter_str = match app.library_view.channel_filter {
        ChannelFilter::All => crate::tui_text!(app, "All"),
        ChannelFilter::Mono => crate::tui_text!(app, "Mono"),
        ChannelFilter::Stereo => "2.0".to_string(),
        ChannelFilter::Surround => "5.x".to_string(),
        ChannelFilter::Surround71 => "7.1".to_string(),
        ChannelFilter::SurroundPlus => "8+".to_string(),
        ChannelFilter::Mixed => crate::tui_text!(app, "Mixed"),
        ChannelFilter::Specific(n) => format_channel_count(n),
    };

    // Get available channel counts for help text
    let available_counts = app.get_unique_channel_counts();
    let counts_str = if available_counts.is_empty() {
        String::new()
    } else {
        // Note: We'll show all available counts without brackets
        // The current filter is already shown in green in the title
        crate::tui_text!(
            app,
            format!(
                " | Available: {}",
                available_counts
                    .iter()
                    .map(|&n| format_channel_count(n))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        )
    };

    // Build title with colored sorting and filtering
    let base_title_style = Style::default().fg(app.theme.fg_secondary);
    let title_spans = vec![
        Span::styled(
            crate::tui_text!(app, "Search Albums | Sort: "),
            base_title_style,
        ),
        Span::styled(sort_order_str, Style::default().fg(app.theme.border_color)),
        Span::styled(crate::tui_text!(app, " | Filter: "), base_title_style),
        Span::styled(&filter_str, Style::default().fg(app.theme.border_color)),
        Span::styled(counts_str, base_title_style),
    ];
    let title = Line::from(title_spans);

    let search_box = Paragraph::new(search_text)
        .style(input_style)
        .block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(search_box, area);
}
