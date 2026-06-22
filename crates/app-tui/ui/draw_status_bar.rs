use super::*;

pub(crate) fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let mut status_spans = vec![Span::raw(" ")];

    if app.read_only {
        status_spans.push(Span::styled(
            "[READ-ONLY] ",
            Style::default()
                .fg(app.theme.accent_warning)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Show status message if available
    // Filter out scan-related messages unless we're on the Directory screen
    if let Some(msg) = &app.ui.status_message {
        let is_scan_message = msg.contains("Scanning")
            || msg.contains("Scan complete")
            || msg.contains("Scan failed");
        let should_show = !is_scan_message || app.current_screen == Screen::Configure;

        if should_show {
            // Truncate message to prevent overflow (leave room for other info)
            let max_msg_len = (area.width as usize).saturating_sub(80);
            let truncated_msg = if msg.len() > max_msg_len {
                format!("{}...", &msg[..max_msg_len.saturating_sub(3)])
            } else {
                msg.clone()
            };

            status_spans.push(Span::styled(
                format!("{} | ", truncated_msg),
                Style::default()
                    .fg(app.theme.title_color)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }

    if let Some(idx) = app.playback.current_queue_index
        && let Some(entry) = app.queue.get(idx)
        && let Some(track) = entry.item.current_track()
    {
        let raw_track_name = track
            .title
            .as_deref()
            .unwrap_or_else(|| track.path.file_name().unwrap().to_str().unwrap());
        let cleaned_track_name = clean_track_name(raw_track_name);
        // Truncate to max 50 chars for status bar to leave room for other info
        let track_name = truncate_with_ellipsis(&cleaned_track_name, 50);
        status_spans.push(Span::styled(
            format!("Now: {}", track_name),
            Style::default().fg(app.theme.playing_indicator),
        ));
        status_spans.push(Span::raw(" | "));
    }

    if !app.plugin_rack.graph.is_empty() {
        let plugin_status = if app.plugin_rack.update_in_progress {
            format!("Plugins: {} [updating...] ", app.plugin_rack.graph.len())
        } else {
            format!("Plugins: {} ", app.plugin_rack.graph.len())
        };

        let plugin_color = if app.plugin_rack.update_in_progress {
            app.theme.accent_warning
        } else {
            app.theme.accent_secondary
        };

        status_spans.push(Span::styled(
            plugin_status,
            Style::default().fg(plugin_color),
        ));
        status_spans.push(Span::raw("| "));
    }

    // Show background scanner progress
    {
        let mut scanner_parts: Vec<String> = Vec::new();
        if app.scan.waveform_manager.in_progress {
            scanner_parts.push(format!(
                "Waveform {}/{}/{}",
                app.scan.waveform_manager.succeeded,
                app.scan.waveform_manager.failed,
                app.scan.waveform_manager.total
            ));
        }
        if app.scan.replay_gain_manager.in_progress {
            if app.scan.replay_gain_manager.album_gain_phase
                == sotf_audio_player::AlbumGainPhase::Scanning
            {
                scanner_parts.push(format!(
                    "AlbumGain {}/{}",
                    app.scan.replay_gain_manager.album_gain_done,
                    app.scan.replay_gain_manager.album_gain_total,
                ));
            } else {
                scanner_parts.push(format!(
                    "ReplayGain {}/{}/{}",
                    app.scan.replay_gain_manager.succeeded,
                    app.scan.replay_gain_manager.failed,
                    app.scan.replay_gain_manager.total
                ));
            }
        }
        if app.scan.bliss_manager.in_progress {
            scanner_parts.push(format!(
                "Bliss {}/{}/{}",
                app.scan.bliss_manager.succeeded,
                app.scan.bliss_manager.failed,
                app.scan.bliss_manager.total
            ));
        }
        if app.scan.in_progress {
            scanner_parts.push(format!("Library {}", app.scan.progress_tracks));
        }
        if !scanner_parts.is_empty() {
            let paused = app
                .scan
                .pause_flag
                .load(std::sync::atomic::Ordering::Relaxed);
            let label = if paused {
                format!("[paused] {} ", scanner_parts.join(", "))
            } else {
                format!("{} ", scanner_parts.join(", "))
            };
            status_spans.push(Span::styled(
                label,
                Style::default().fg(app.theme.accent_warning),
            ));
            status_spans.push(Span::raw("| "));
        }
    }

    status_spans.push(Span::styled(
        "?",
        Style::default().fg(app.theme.accent_primary),
    ));
    status_spans.push(Span::raw("=Help"));

    let status_text = Line::from(status_spans);

    let status = Paragraph::new(status_text).style(Style::default().fg(app.theme.fg_primary));

    f.render_widget(status, area);
}
