use super::*;

pub(crate) fn draw_album_list(f: &mut Frame, area: Rect, app: &App, is_focused: bool) {
    use ratatui::widgets::StatefulWidget;

    // Calculate channel count for truncation logic
    let num_channels = app
        .playback
        .loudness_info
        .as_ref()
        .map(|info| info.channel_peaks.len())
        .unwrap_or(2);

    match app.library_view.mode {
        LibraryViewMode::Flat => {
            let albums = &app.library_view.cached_filtered_albums;

            let items: Vec<ListItem> = albums
                .iter()
                .enumerate()
                .map(|(i, album)| {
                    // Clean and truncate to prevent overflow into meters column
                    // Truncation length adjusted based on right column width and play count display
                    // For stereo (12% right column): 90 chars is safe (reduced to make room for play count)
                    // For 5.1 (20% right column): 75 chars to be safe
                    let raw_content = album.display_name();
                    let cleaned = clean_text(&raw_content);
                    let max_len = if num_channels > 4 { 75 } else { 90 };
                    let content = truncate_with_ellipsis(&cleaned, max_len);

                    // Add favorite heart and play count to the display
                    let fav_prefix = if album.is_favorite { "\u{2665} " } else { "" };
                    let display_text = if album.play_count > 0 {
                        format!("{}{}  \u{1F3B5} {}", fav_prefix, content, album.play_count)
                    } else {
                        format!("{}{}", fav_prefix, content)
                    };

                    let style = if i == app.library_view.selected_album_index {
                        Style::default()
                            .fg(app.theme.fg_selected)
                            .bg(app.theme.bg_selected)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(app.theme.fg_primary)
                    };
                    ListItem::new(display_text).style(style)
                })
                .collect();

            let border_type = if is_focused {
                BorderType::Double
            } else {
                BorderType::Plain
            };

            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(border_type)
                        .title(format!(
                            "Albums ({}){}  'a' add, 't' tree, 'f' fav, 'F' filter",
                            albums.len(),
                            if app.library_view.show_favorites_only {
                                " [\u{2665} Favorites]"
                            } else {
                                ""
                            },
                        )),
                )
                .highlight_style(
                    Style::default()
                        .fg(app.theme.fg_selected)
                        .bg(app.theme.bg_selected)
                        .add_modifier(Modifier::BOLD),
                );

            let mut state = ListState::default();
            state.select(Some(app.library_view.selected_album_index));

            StatefulWidget::render(list, area, f.buffer_mut(), &mut state);
        }
        LibraryViewMode::TreeView => {
            let tree_items = app.get_tree_items();

            let items: Vec<ListItem> = tree_items
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let (content, style) = match item {
                        TreeItem::Artist { name, expanded } => {
                            let prefix = if *expanded { "▼ " } else { "▶ " };
                            // Clean artist name and truncate to prevent overflow
                            let cleaned_name = clean_text(name);
                            let truncated_name = truncate_with_ellipsis(&cleaned_name, 95);
                            let content = format!("{}{}", prefix, truncated_name);
                            let mut style = Style::default()
                                .fg(app.theme.accent_primary)
                                .add_modifier(Modifier::BOLD);
                            if i == app.library_view.selected_tree_index {
                                style = style.bg(app.theme.bg_highlight);
                            }
                            (content, style)
                        }
                        TreeItem::Album { index } => {
                            if let Some(album) = app.library.albums.get(*index) {
                                // Use display_name for consistency, clean and truncate
                                let raw_album = album.display_name();
                                let cleaned = clean_text(&raw_album);
                                let truncated = truncate_with_ellipsis(&cleaned, 80);

                                // Add play count if > 0
                                let content = if album.play_count > 0 {
                                    format!("  └─ {}  \u{1F3B5} {}", truncated, album.play_count)
                                } else {
                                    format!("  └─ {}", truncated)
                                };

                                let style = if i == app.library_view.selected_tree_index {
                                    Style::default()
                                        .fg(app.theme.fg_selected)
                                        .bg(app.theme.bg_selected)
                                        .add_modifier(Modifier::BOLD)
                                } else {
                                    Style::default().fg(app.theme.fg_primary)
                                };
                                (content, style)
                            } else {
                                (
                                    "  └─ <unknown>".to_string(),
                                    Style::default().fg(app.theme.fg_muted),
                                )
                            }
                        }
                    };
                    ListItem::new(content).style(style)
                })
                .collect();

            let border_type = if is_focused {
                BorderType::Double
            } else {
                BorderType::Plain
            };

            let list = List::new(items)
                .block(Block::default()
                    .borders(Borders::ALL)
                    .border_type(border_type)
                    .title(format!(
                        "Artists ({}) - 'h/l' to expand/collapse, 'a' to add, 't' to toggle view",
                        app.library_view.artist_tree.len()
                    )))
                .highlight_style(
                    Style::default()
                        .fg(app.theme.fg_selected)
                        .bg(app.theme.bg_selected)
                        .add_modifier(Modifier::BOLD),
                );

            let mut state = ListState::default();
            state.select(Some(app.library_view.selected_tree_index));

            StatefulWidget::render(list, area, f.buffer_mut(), &mut state);
        }
    }
}
