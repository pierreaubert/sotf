use super::*;

pub(crate) fn draw_playlists_screen(f: &mut Frame, area: Rect, app: &App) {
    // PlaylistMode is available via `use super::*`

    // Top help line above the two-column body, mirroring the layout of the
    // Output Devices screen so the keybindings are discoverable without
    // having to read the bottom border.
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Help box
            Constraint::Min(0),    // Body (playlist list + tracks)
        ])
        .split(area);

    let help_text = match app.playlist_mode {
        PlaylistMode::List => {
            "↑↓=Navigate  Enter=Open  n=New  r=Rename  d=Delete  p=Play  i=Import  e=Export  Esc=Back"
        }
        PlaylistMode::Tracks => {
            "↑↓=Navigate  Enter=Play track  p=Play all  x=Remove  K/J=Move up/down  Esc=Back to list"
        }
        PlaylistMode::Create => "Type playlist name  Enter=Create  Esc=Cancel",
        PlaylistMode::Rename => "Type new name  Enter=Save  Esc=Cancel",
        PlaylistMode::ConfirmDelete => "y=Confirm delete  n/Esc=Cancel",
    };
    draw_help_box_with_text(f, outer[0], app, help_text);

    // Split body into two columns: playlist list (left) and tracks (right)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(outer[1]);

    draw_playlist_list(f, chunks[0], app);
    draw_playlist_tracks(f, chunks[1], app);

    // Overlay for text input modes
    match app.playlist_mode {
        PlaylistMode::Create => draw_input_popup(f, area, "New Playlist", &app.playlist_name_input),
        PlaylistMode::Rename => {
            draw_input_popup(f, area, "Rename Playlist", &app.playlist_name_input)
        }
        PlaylistMode::ConfirmDelete => {
            let name = app
                .playlist_controller
                .playlists()
                .get(app.playlist_controller.selected_playlist_index)
                .map(|p| p.name.as_str())
                .unwrap_or("?");
            draw_confirm_popup(f, area, &format!("Delete '{}'? (y/n)", name));
        }
        _ => {}
    }
}

fn draw_playlist_list(f: &mut Frame, area: Rect, app: &App) {
    let playlists = app.playlist_controller.playlists();
    let is_list_focused = app.playlist_mode == PlaylistMode::List;

    let items: Vec<ListItem> = playlists
        .iter()
        .enumerate()
        .map(|(i, playlist)| {
            let track_count = app.playlist_controller.playlist_track_count(i);
            let content = if track_count > 0 {
                format!("{} ({})", playlist.name, track_count)
            } else {
                playlist.name.clone()
            };

            let style = if i == app.playlist_controller.selected_playlist_index {
                Style::default()
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.bg_selected)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };
            ListItem::new(content).style(style)
        })
        .collect();

    let border_type = if is_list_focused {
        BorderType::Double
    } else {
        BorderType::Plain
    };

    let help = if is_list_focused {
        "n:new r:rename d:del Enter:open p:play i:import e:export"
    } else {
        ""
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(border_type)
            .title(format!("Playlists ({})", playlists.len()))
            .title_bottom(help),
    );

    f.render_widget(list, area);
}

fn draw_playlist_tracks(f: &mut Frame, area: Rect, app: &App) {
    let is_tracks_focused = app.playlist_mode == PlaylistMode::Tracks;

    let (title, items) = if let Some(playlist) = app.playlist_controller.active_playlist() {
        let resolved = app.playlist_controller.resolve_tracks(&app.library);

        let items: Vec<ListItem> = playlist
            .entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let display = if let Some(Some(track)) = resolved.get(i) {
                    let artist = track.artist.as_deref().unwrap_or("?");
                    let title = track.title.as_deref().unwrap_or("?");
                    let dur = track
                        .duration_secs
                        .map(|d| format!(" [{}:{:02}]", d / 60, d % 60))
                        .unwrap_or_default();
                    format!("{}. {} - {}{}", i + 1, artist, title, dur)
                } else {
                    format!(
                        "{}. {}",
                        i + 1,
                        entry
                            .track_path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                    )
                };

                let style = if i == app.playlist_controller.selected_track_index {
                    Style::default()
                        .fg(app.theme.fg_selected)
                        .bg(app.theme.bg_selected)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.theme.fg_primary)
                };
                ListItem::new(display).style(style)
            })
            .collect();

        (
            format!("{} ({} tracks)", playlist.name, playlist.entries.len()),
            items,
        )
    } else {
        ("Tracks (select a playlist)".to_string(), Vec::new())
    };

    let border_type = if is_tracks_focused {
        BorderType::Double
    } else {
        BorderType::Plain
    };

    let help = if is_tracks_focused {
        "x:remove K/J:move Esc:back p:play"
    } else {
        ""
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(border_type)
            .title(title)
            .title_bottom(help),
    );

    f.render_widget(list, area);
}

fn draw_input_popup(f: &mut Frame, area: Rect, title: &str, input: &str) {
    let popup_area = centered_rect(50, 3, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .title(title);
    let text = Paragraph::new(format!("{}_", input)).block(block);
    f.render_widget(Clear, popup_area);
    f.render_widget(text, popup_area);
}

fn draw_confirm_popup(f: &mut Frame, area: Rect, message: &str) {
    let popup_area = centered_rect(50, 3, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .title("Confirm");
    let text = Paragraph::new(message).block(block);
    f.render_widget(Clear, popup_area);
    f.render_widget(text, popup_area);
}

/// Create a centered rect with the given percentage size.
fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((r.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
