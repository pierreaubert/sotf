use super::*;

pub(crate) fn draw_queue_screen(f: &mut Frame, area: Rect, app: &mut App) {
    let is_focused = app.current_screen == Screen::Queue;

    // Vertical split: help box on top, content in middle, transport at bottom
    let vchunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Help box
            Constraint::Min(0),    // Queue content
            Constraint::Length(3), // Transport bar
        ])
        .split(area);

    draw_help_box(f, vchunks[0], app, Screen::Queue);

    let content_area = vchunks[1];
    let transport_area = vchunks[2];

    // Split the area horizontally if we have album images (not available on Windows)
    #[cfg(not(target_os = "windows"))]
    let (queue_area, image_area) = {
        let has_images = !app.album_images.is_empty();
        if has_images {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(60), // Queue list
                    Constraint::Percentage(40), // Album art
                ])
                .split(content_area);
            (chunks[0], Some(chunks[1]))
        } else {
            (content_area, None)
        }
    };
    #[cfg(target_os = "windows")]
    let queue_area = content_area;

    let mut items: Vec<ListItem> = Vec::new();
    let mut selected_visual_index: Option<usize> = None;

    for (i, entry) in app.queue.iter().enumerate() {
        let is_current = app.current_queue_index == Some(i);
        let is_selected = i == app.selected_queue_index;
        let is_album_header_selected = is_selected && app.selected_queue_track_index.is_none();
        let is_expanded = entry.expanded;

        // Album header
        let expand_indicator = if is_expanded { "▼" } else { "▶" };
        let raw_display = entry.item.album.display_name();
        let cleaned_display = clean_text(&raw_display);
        let truncated_display = truncate_with_ellipsis(&cleaned_display, 90);
        let mut content = format!("{} {}", expand_indicator, truncated_display);

        if is_current {
            let track_info = format!(
                " [Track {}/{}]",
                entry.item.current_track_index + 1,
                entry.item.album.tracks.len()
            );
            content.push_str(&track_info);
        }

        let style = if is_album_header_selected {
            Style::default()
                .fg(app.theme.fg_selected)
                .bg(app.theme.bg_selected)
                .add_modifier(Modifier::BOLD)
        } else if is_current {
            Style::default()
                .fg(app.theme.playing_indicator)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.fg_primary)
        };

        if is_album_header_selected {
            selected_visual_index = Some(items.len());
        }
        items.push(ListItem::new(content).style(style));

        // Show individual tracks if expanded
        if is_expanded {
            for (track_idx, track) in entry.item.album.tracks.iter().enumerate() {
                let is_current_track = is_current && track_idx == entry.item.current_track_index;
                let is_track_selected =
                    is_selected && app.selected_queue_track_index == Some(track_idx);
                let raw_track_name = track
                    .title
                    .as_deref()
                    .unwrap_or_else(|| track.path.file_name().unwrap().to_str().unwrap());
                let cleaned_track_name = clean_track_name(raw_track_name);
                // Truncate to max 60 chars to prevent overflow into meters
                let track_name = truncate_with_ellipsis(&cleaned_track_name, 60);

                let duration_str = if let Some(duration) = track.duration_secs {
                    format!(" ({}:{:02})", duration / 60, duration % 60)
                } else {
                    String::new()
                };

                let track_content = if is_current_track {
                    if app.is_playing {
                        format!("  ▶ {}.{}{}", track_idx + 1, track_name, duration_str)
                    } else {
                        format!("  ⏸ {}.{}{}", track_idx + 1, track_name, duration_str)
                    }
                } else {
                    format!("    {}.{}{}", track_idx + 1, track_name, duration_str)
                };

                let track_style = if is_track_selected {
                    Style::default()
                        .fg(app.theme.fg_selected)
                        .bg(app.theme.bg_selected)
                        .add_modifier(Modifier::BOLD)
                } else if is_current_track {
                    Style::default()
                        .fg(app.theme.current_track)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.theme.fg_secondary)
                };

                if is_track_selected {
                    selected_visual_index = Some(items.len());
                }
                items.push(ListItem::new(track_content).style(track_style));
            }
        }
    }

    let title = if app.queue.is_empty() {
        "Queue (empty)".to_string()
    } else {
        format!("Queue ({})", app.queue.len())
    };

    let border_type = if is_focused {
        BorderType::Double
    } else {
        BorderType::Plain
    };

    let list = List::new(items)
        .style(Style::default().fg(app.theme.fg_primary))
        .highlight_style(Style::default()) // we handle highlighting manually per item
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(border_type)
                .title(title),
        );

    let mut list_state = ListState::default();
    list_state.select(selected_visual_index);
    f.render_stateful_widget(list, queue_area, &mut list_state);

    // Render album art if available (not on Windows)
    #[cfg(not(target_os = "windows"))]
    if let Some(image_area) = image_area {
        draw_album_art(f, image_area, app);
    }

    draw_transport(f, transport_area, app);
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn draw_album_art(f: &mut Frame, area: Rect, app: &mut App) {
    use ratatui_image::StatefulImage;

    // Create a border block
    let title = if app.album_images.is_empty() {
        "Album Art (none)".to_string()
    } else if app.album_images.len() > 1 {
        format!(
            "Album Art ({}/{}) - [] to cycle",
            app.selected_image_index + 1,
            app.album_images.len()
        )
    } else {
        "Album Art".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().fg(app.theme.fg_primary));
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    // Split area: image on top, ReplayGain info at bottom
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),   // Image area (takes most space)
            Constraint::Length(4), // ReplayGain info (fixed 4 lines)
        ])
        .split(inner_area);

    let image_area = chunks[0];
    let info_area = chunks[1];

    // Render the image if available
    if let Some(image_path) = app.get_current_album_image().cloned() {
        // Create the protocol once and cache it; reuse across renders so it can resize properly
        let needs_create = app
            .image_protocol_path
            .as_ref()
            .is_none_or(|p| *p != image_path);
        if needs_create && let Some(picker) = &mut app.image_picker {
            if let Ok(img) = image::open(&image_path) {
                app.image_protocol = Some(picker.new_resize_protocol(img));
                app.image_protocol_path = Some(image_path.clone());
            } else {
                app.image_protocol = None;
                app.image_protocol_path = None;
            }
        }

        if let Some(protocol) = &mut app.image_protocol {
            let image = StatefulImage::new();
            f.render_stateful_widget(image, image_area, protocol);
        } else {
            let error_text = Paragraph::new("Failed to load image")
                .style(Style::default().fg(ratatui::style::Color::Red));
            f.render_widget(error_text, image_area);
        }
    } else {
        let no_image_text =
            Paragraph::new("No album art found").style(Style::default().fg(app.theme.fg_muted));
        f.render_widget(no_image_text, image_area);
    }

    // Render ReplayGain info below the image
    draw_replay_gain_info(f, info_area, app);
}

pub(crate) fn draw_replay_gain_info(f: &mut Frame, area: Rect, app: &App) {
    // Get currently playing track
    let track_info = if let Some(queue_index) = app.current_queue_index {
        if let Some(entry) = app.queue.get(queue_index) {
            entry
                .item
                .album
                .tracks
                .get(entry.item.current_track_index)
                .map(|track| (track, &entry.item.album))
        } else {
            None
        }
    } else {
        None
    };

    let mut lines = Vec::new();

    if let Some((track, _album)) = track_info {
        // File format info: FLAC 24-bit/48kHz Stereo
        let format_str = if let (Some(bit_depth), Some(sample_rate), Some(channels)) =
            (track.bit_depth, track.sample_rate, track.channels)
        {
            let ext = track
                .path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_uppercase())
                .unwrap_or_else(|| "Unknown".to_string());
            let sr_khz = sample_rate as f64 / 1000.0;
            let ch_str = match channels {
                1 => "Mono",
                2 => "Stereo",
                6 => "5.1",
                8 => "7.1",
                _ => "Multi",
            };
            format!("{} {}-bit/{:.1}kHz {}", ext, bit_depth, sr_khz, ch_str)
        } else {
            "Unknown".to_string()
        };
        lines.push(Line::from(vec![
            Span::styled("Format: ", Style::default().fg(app.theme.title_color)),
            Span::raw(format_str),
        ]));

        // Track ReplayGain
        if let Some(gain) = track.replay_gain {
            let peak_str = if let Some(peak) = track.replay_peak {
                format!(" (peak: {:.3})", peak)
            } else {
                String::new()
            };
            lines.push(Line::from(vec![
                Span::styled("Track RG: ", Style::default().fg(app.theme.title_color)),
                Span::styled(
                    format!("{:+.2} dB{}", gain, peak_str),
                    Style::default().fg(app.theme.accent_success),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("Track RG: ", Style::default().fg(app.theme.title_color)),
                Span::styled("not available", Style::default().fg(app.theme.fg_muted)),
            ]));
        }

        // Album ReplayGain
        if let Some(gain) = track.album_gain {
            let peak_str = if let Some(peak) = track.album_peak {
                format!(" (peak: {:.3})", peak)
            } else {
                String::new()
            };
            lines.push(Line::from(vec![
                Span::styled("Album RG: ", Style::default().fg(app.theme.title_color)),
                Span::styled(
                    format!("{:+.2} dB{}", gain, peak_str),
                    Style::default().fg(app.theme.accent_success),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("Album RG: ", Style::default().fg(app.theme.title_color)),
                Span::styled("not available", Style::default().fg(app.theme.fg_muted)),
            ]));
        }
    } else {
        lines.push(Line::from("No track playing"));
    }

    let paragraph = Paragraph::new(lines)
        .style(Style::default().fg(app.theme.fg_primary))
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}
