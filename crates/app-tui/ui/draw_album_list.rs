use super::*;

fn album_list_inner_width(area: Rect) -> usize {
    area.width.saturating_sub(2) as usize
}

fn play_count_suffix_width(play_count: usize) -> usize {
    if play_count > 0 {
        format!("  \u{1F3B5} {play_count}").chars().count()
    } else {
        0
    }
}

pub(crate) fn flat_album_name_width(
    area: Rect,
    num_channels: usize,
    is_favorite: bool,
    play_count: usize,
) -> usize {
    let favorite_prefix = if is_favorite { 2 } else { 0 };
    let high_channel_reserve = if num_channels > 4 { 8 } else { 0 };
    album_list_inner_width(area)
        .saturating_sub(
            favorite_prefix + play_count_suffix_width(play_count) + high_channel_reserve,
        )
        .max(1)
}

pub(crate) fn tree_artist_name_width(area: Rect) -> usize {
    album_list_inner_width(area).saturating_sub(2).max(1)
}

pub(crate) fn tree_album_name_width(area: Rect, play_count: usize) -> usize {
    album_list_inner_width(area)
        .saturating_sub(4 + play_count_suffix_width(play_count))
        .max(1)
}

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
                    let raw_content = album.display_name();
                    let cleaned = clean_text(&raw_content);
                    let max_len = flat_album_name_width(
                        area,
                        num_channels,
                        album.is_favorite,
                        album.play_count,
                    );
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
                            let truncated_name =
                                truncate_with_ellipsis(&cleaned_name, tree_artist_name_width(area));
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
                                let truncated = truncate_with_ellipsis(
                                    &cleaned,
                                    tree_album_name_width(area, album.play_count),
                                );

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

#[cfg(test)]
mod tests {
    use super::{flat_album_name_width, tree_album_name_width, tree_artist_name_width};
    use ratatui::layout::Rect;

    #[test]
    fn album_list_truncation_tracks_terminal_width() {
        let narrow = Rect::new(0, 0, 24, 10);
        let wide = Rect::new(0, 0, 120, 10);

        assert!(
            flat_album_name_width(narrow, 2, false, 0) < flat_album_name_width(wide, 2, false, 0)
        );
        assert!(tree_artist_name_width(narrow) < tree_artist_name_width(wide));
        assert!(tree_album_name_width(narrow, 12) < tree_album_name_width(wide, 12));
    }

    #[test]
    fn album_list_truncation_reserves_badges_and_high_channel_meter_space() {
        let area = Rect::new(0, 0, 80, 10);
        assert!(
            flat_album_name_width(area, 6, true, 99) < flat_album_name_width(area, 2, false, 0)
        );
        assert_eq!(
            flat_album_name_width(Rect::new(0, 0, 1, 1), 8, true, 999),
            1
        );
    }
}
