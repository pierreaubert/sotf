use super::*;

pub(crate) fn draw_directory_manager(f: &mut Frame, area: Rect, app: &App) {
    // Calculate constraints based on whether we have autocomplete suggestions
    let autocomplete_height =
        if app.input_mode == InputMode::AddDirectory && !app.autocomplete_suggestions.is_empty() {
            (app.autocomplete_suggestions.len().min(5) + 2) as u16 // Max 5 suggestions + borders
        } else {
            0
        };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),                   // Help box
            Constraint::Length(3),                   // Input box
            Constraint::Length(autocomplete_height), // Autocomplete suggestions
            Constraint::Min(0),                      // Directory list + status
        ])
        .split(area);

    // Help box with scan keybindings
    let help_text = "a/F2=Add dir | s/S=Scan | r/R=ReplayGain | b/B=Bliss | w/W=Waveform (uppercase=force)";
    draw_help_box_with_text(f, chunks[0], app, help_text);

    // Input box for adding directories
    let input_style = if app.input_mode == InputMode::AddDirectory {
        Style::default().fg(app.theme.title_color)
    } else {
        Style::default().fg(app.theme.fg_primary)
    };

    let input_text = if app.input_mode == InputMode::AddDirectory {
        format!("Path: {}█ (Tab to autocomplete)", app.directory_input)
    } else {
        "Path: (Press 'a' to add directory)".to_string()
    };

    let input_box = Paragraph::new(input_text).style(input_style).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Add Directory"),
    );

    f.render_widget(input_box, chunks[1]);

    // Show autocomplete suggestions if in add directory mode
    if app.input_mode == InputMode::AddDirectory && !app.autocomplete_suggestions.is_empty() {
        let suggestion_items: Vec<ListItem> = app
            .autocomplete_suggestions
            .iter()
            .take(5) // Show max 5 suggestions
            .enumerate()
            .map(|(i, suggestion)| {
                let style = if i == app.autocomplete_index {
                    Style::default()
                        .fg(app.theme.fg_selected)
                        .bg(app.theme.accent_primary)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.theme.fg_secondary)
                };
                ListItem::new(suggestion.as_str()).style(style)
            })
            .collect();

        let suggestions_list = List::new(suggestion_items).block(
            Block::default().borders(Borders::ALL).title(format!(
                "Suggestions ({}/{})",
                app.autocomplete_index + 1,
                app.autocomplete_suggestions.len()
            )),
        );

        f.render_widget(suggestions_list, chunks[2]);
    }

    // Split remaining area: directory list on top, status below
    let dir_status_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),      // Directory list
            Constraint::Length(6),   // Status (4 lines + 2 border)
        ])
        .split(chunks[3]);

    draw_directory_list(f, dir_status_chunks[0], app);
    draw_directory_status(f, dir_status_chunks[1], app);
}

pub(crate) fn draw_directory_list(f: &mut Frame, area: Rect, app: &App) {
    // Directory list with tree view
    let tree_items = app.get_directory_tree_items();

    let items: Vec<ListItem> = tree_items
        .iter()
        .enumerate()
        .map(|(i, (path, level, expanded))| {
            let indent = "  ".repeat(*level);
            let expand_indicator = if *level == 0 {
                if *expanded { "▼ " } else { "▶ " }
            } else {
                "└─ "
            };

            let path_str = if *level == 0 {
                path.display().to_string()
            } else {
                // For subdirectories, just show the name
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string())
            };

            // For all directories, add track/album count and last scan time
            // We need to find the DirectoryInfo corresponding to this path
            // Since we flattened the tree, we can't easily look it up by index in the original list
            // But we can search by path in the flattened list or just search the whole tree?
            // Actually, get_directory_tree_items returns (PathBuf, level, expanded)
            // It doesn't return the DirectoryInfo itself.
            // We should probably update get_directory_tree_items to return more info or look it up here.
            // Looking up by path in the recursive structure is expensive if we do it for every item.
            // But for a TUI it might be fine.

            // Helper to find directory info by path
            fn find_dir_info<'a>(
                directories: &'a [sotf_audio_player::DirectoryInfo],
                path: &std::path::Path,
            ) -> Option<&'a sotf_audio_player::DirectoryInfo> {
                for dir in directories {
                    if dir.path == path {
                        return Some(dir);
                    }
                    if let Some(found) = find_dir_info(&dir.subdirectories, path) {
                        return Some(found);
                    }
                }
                None
            }

            let info_str = if let Some(dir_info) = find_dir_info(&app.library.directories, path) {
                let track_count = dir_info.file_count;
                let album_count = dir_info.album_count;
                let last_scan = if let Some(time) = dir_info.last_scanned {
                    // Format as relative time (e.g., "2 days ago")
                    if let Ok(elapsed) = time.elapsed() {
                        let secs = elapsed.as_secs();
                        if secs < 60 {
                            "just now".to_string()
                        } else if secs < 3600 {
                            format!("{} min ago", secs / 60)
                        } else if secs < 86400 {
                            format!("{} hrs ago", secs / 3600)
                        } else {
                            format!("{} days ago", secs / 86400)
                        }
                    } else {
                        "never".to_string()
                    }
                } else {
                    "never".to_string()
                };

                if *level == 0 {
                    format!(
                        " [{} tracks, {} albums, {}]",
                        track_count, album_count, last_scan
                    )
                } else {
                    format!(" [{} tracks, {} albums]", track_count, album_count)
                }
            } else {
                String::new()
            };

            let content = format!("{}{}{}{}", indent, expand_indicator, path_str, info_str);

            let style = if i == app.selected_directory_index {
                Style::default()
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.bg_selected)
                    .add_modifier(Modifier::BOLD)
            } else if *level == 0 {
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let title = "Directories";

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .fg(app.theme.fg_selected)
                .bg(app.theme.bg_selected)
                .add_modifier(Modifier::BOLD),
        );

    // Use stateful widget for proper scrolling
    let mut state = ListState::default();
    state.select(Some(app.selected_directory_index));

    use ratatui::widgets::StatefulWidget;
    StatefulWidget::render(list, area, f.buffer_mut(), &mut state);
}

pub(crate) fn draw_directory_status(f: &mut Frame, area: Rect, app: &App) {
    let paused = app
        .scanner_pause_flag
        .load(std::sync::atomic::Ordering::Relaxed);

    let pause_tag = if paused { " [paused]" } else { "" };

    let label_style = Style::default().fg(app.theme.accent_primary);
    let idle_style = Style::default().fg(app.theme.fg_secondary);
    let progress_style = Style::default().fg(app.theme.accent_warning);
    let ok_style = Style::default().fg(app.theme.accent_success);
    let err_style = Style::default().fg(app.theme.accent_error);

    // Count library-level stats from tracks
    let total_tracks: usize = app.library.albums.iter().map(|a| a.tracks.len()).sum();
    let tracks_with_rg: usize = app
        .library
        .albums
        .iter()
        .flat_map(|a| &a.tracks)
        .filter(|t| t.replay_gain.is_some())
        .count();
    let tracks_with_waveform: usize = app
        .library
        .albums
        .iter()
        .flat_map(|a| &a.tracks)
        .filter(|t| t.waveform.is_some())
        .count();

    // Header
    //               "Scanner     Status              OK   Fail  Total"
    let mut lines: Vec<Line> = vec![Line::from(vec![
        Span::styled("Scanner     ", label_style),
        Span::styled("Status              ", idle_style),
        Span::styled("  OK", ok_style),
        Span::styled("  Fail", err_style),
        Span::styled("  Total", idle_style),
    ])];

    // Helper to format a scanner row with counts
    let scanner_line = |name: &str,
                        in_progress: bool,
                        status_text: String,
                        succeeded: usize,
                        failed: usize,
                        total: usize|
     -> Line {
        let status_span = if in_progress {
            Span::styled(format!("{:<20}", status_text), progress_style)
        } else {
            Span::styled(format!("{:<20}", status_text), idle_style)
        };
        Line::from(vec![
            Span::styled(format!("{:<12}", name), label_style),
            status_span,
            Span::styled(format!("{:>4}", succeeded), ok_style),
            Span::styled(format!("{:>6}", failed), if failed > 0 { err_style } else { idle_style }),
            Span::styled(format!("{:>7}", total), idle_style),
        ])
    };

    // ReplayGain
    let rg = &app.replay_gain_manager;
    if rg.in_progress {
        let rg_status = if rg.album_gain_phase == sotf_audio_player::AlbumGainPhase::Scanning {
            format!("album {}/{}{}", rg.album_gain_done, rg.album_gain_total, pause_tag)
        } else {
            let pct = if rg.total > 0 {
                rg.processed as f32 / rg.total as f32 * 100.0
            } else {
                0.0
            };
            format!("{}/{} ({:.0}%){}", rg.processed, rg.total, pct, pause_tag)
        };
        lines.push(scanner_line(
            "ReplayGain",
            true,
            rg_status,
            rg.succeeded,
            rg.failed,
            rg.total,
        ));
    } else {
        let missing = total_tracks.saturating_sub(tracks_with_rg);
        let status = format!("{}/{} tracks", tracks_with_rg, total_tracks);
        lines.push(scanner_line(
            "ReplayGain",
            false,
            status,
            tracks_with_rg,
            missing,
            total_tracks,
        ));
    }

    // Waveform
    let wf = &app.waveform_manager;
    if wf.in_progress {
        let pct = if wf.total > 0 {
            wf.processed as f32 / wf.total as f32 * 100.0
        } else {
            0.0
        };
        let wf_status = format!("{}/{} ({:.0}%){}", wf.processed, wf.total, pct, pause_tag);
        lines.push(scanner_line(
            "Waveform",
            true,
            wf_status,
            wf.succeeded,
            wf.failed,
            wf.total,
        ));
    } else {
        let missing = total_tracks.saturating_sub(tracks_with_waveform);
        let status = format!("{}/{} tracks", tracks_with_waveform, total_tracks);
        lines.push(scanner_line(
            "Waveform",
            false,
            status,
            tracks_with_waveform,
            missing,
            total_tracks,
        ));
    }

    // Bliss
    let bl = &app.bliss_manager;
    if bl.in_progress {
        let pct = if bl.total > 0 {
            bl.processed as f32 / bl.total as f32 * 100.0
        } else {
            0.0
        };
        let bl_status = format!("{}/{} ({:.0}%){}", bl.processed, bl.total, pct, pause_tag);
        lines.push(scanner_line(
            "Bliss",
            true,
            bl_status,
            bl.succeeded,
            bl.failed,
            bl.total,
        ));
    } else {
        // Bliss data not on Track struct — use last scan counts if available
        lines.push(scanner_line(
            "Bliss",
            false,
            "idle".to_string(),
            bl.succeeded,
            bl.failed,
            bl.total,
        ));
    }

    // Library scan (no success/failure breakdown)
    let album_count = app.library.albums.len();
    if app.scan_in_progress {
        lines.push(Line::from(vec![
            Span::styled(format!("{:<12}", "Library"), label_style),
            Span::styled(
                format!(
                    "{:<20}",
                    format!(
                        "{} tracks / {} albums{}",
                        app.scan_progress_tracks, app.scan_progress_albums, pause_tag
                    )
                ),
                progress_style,
            ),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled(format!("{:<12}", "Library"), label_style),
            Span::styled(
                format!("{:<20}", format!("{} tracks / {} albums", total_tracks, album_count)),
                idle_style,
            ),
        ]));
    }

    let status = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Status"));

    f.render_widget(status, area);
}

