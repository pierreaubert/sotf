use super::*;

pub(crate) fn draw_devices_screen(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Help box
            Constraint::Min(0),    // Device list
        ])
        .split(area);

    draw_help_box_with_text(
        f,
        chunks[0],
        app,
        "↑↓=Navigate  Enter=Select  Esc=Back",
    );

    // Device list
    let items: Vec<ListItem> = app
        .output_devices
        .iter()
        .enumerate()
        .map(|(i, device)| {
            let is_selected = i == app.selected_output_device_index;

            let default_tag = if device.is_default { " [DEFAULT]" } else { "" };
            let config_info = if let Some(ref config) = device.default_config {
                format!(" ({}ch, {}Hz)", config.channels, config.sample_rate)
            } else {
                String::new()
            };

            let selected_style = Style::default()
                .fg(app.theme.bg_primary)
                .bg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD);
            let normal_style = Style::default().fg(app.theme.fg_primary);

            let line = Line::from(vec![
                Span::styled(
                    format!(" {} ", if is_selected { "►" } else { " " }),
                    if is_selected {
                        selected_style
                    } else {
                        Style::default().fg(app.theme.accent_primary)
                    },
                ),
                Span::styled(
                    device.name.clone(),
                    if is_selected { selected_style } else { normal_style },
                ),
                Span::styled(
                    default_tag.to_string(),
                    if is_selected {
                        selected_style
                    } else {
                        Style::default().fg(app.theme.accent_success)
                    },
                ),
                Span::styled(
                    config_info,
                    if is_selected {
                        selected_style
                    } else {
                        Style::default().fg(app.theme.fg_secondary)
                    },
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let title = if app.output_devices.is_empty() {
        " Output Devices (none found) ".to_string()
    } else {
        format!(" Output Devices ({}) ", app.output_devices.len())
    };

    let mut list_state = ListState::default();
    list_state.select(Some(app.selected_output_device_index));

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(app.theme.border_color))
                .title(title),
        )
        .highlight_style(
            Style::default()
                .fg(app.theme.bg_primary)
                .bg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(list, chunks[1], &mut list_state);
}

pub(crate) fn draw_configure_screen(f: &mut Frame, area: Rect, app: &App) {
    use crate::app::ConfigureSubScreen;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Help box (with border)
            Constraint::Min(0),    // Select menu
        ])
        .split(area);

    draw_help_box_with_text(
        f,
        chunks[0],
        app,
        "↑↓=Navigate  Enter=Open  Esc=Back",
    );

    let options: &[(ConfigureSubScreen, &str, &str)] = &[
        (ConfigureSubScreen::Directories,  "1", "Directories   – Music library folders"),
        (ConfigureSubScreen::Recording,    "2", "Recording     – Measure impulse responses"),
        (ConfigureSubScreen::RoomEq,       "3", "Room EQ       – Optimize room correction filters"),
        (ConfigureSubScreen::HeadphoneEq,  "4", "Headphone EQ  – Target-curve EQ for headphones"),
        (ConfigureSubScreen::SpinoramaEq,  "5", "Spinorama EQ  – Speaker EQ from spinorama data"),
    ];

    let items: Vec<ListItem> = options
        .iter()
        .map(|(sub, key, label)| {
            let is_selected = *sub == app.configure_sub_screen;
            let style = if is_selected {
                Style::default()
                    .fg(app.theme.bg_primary)
                    .bg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };
            let line = Line::from(vec![
                Span::styled(
                    format!(" [{}] ", key),
                    if is_selected {
                        style.add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(app.theme.accent_primary)
                    },
                ),
                Span::styled(label.to_string(), style),
            ]);
            ListItem::new(line)
        })
        .collect();

    let selected_idx = options
        .iter()
        .position(|(sub, _, _)| *sub == app.configure_sub_screen)
        .unwrap_or(0);

    let mut list_state = ListState::default();
    list_state.select(Some(selected_idx));

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(app.theme.border_color))
                .title(" Configure – select a workflow "),
        )
        .highlight_style(
            Style::default()
                .fg(app.theme.bg_primary)
                .bg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(list, chunks[1], &mut list_state);
}

/// Draw the full-screen configure modal (covers everything below the 3-row title bar).
pub(crate) fn draw_configure_modal(f: &mut Frame, app: &App) {
    use crate::app::ConfigureSubScreen;

    let area = below_title_bar(f);

    // Clear the area so the modal paints over everything underneath
    f.render_widget(Clear, area);

    let title = match app.configure_sub_screen {
        ConfigureSubScreen::Directories => " Directories ",
        ConfigureSubScreen::Recording   => " Recording ",
        ConfigureSubScreen::RoomEq      => " Room EQ ",
        ConfigureSubScreen::HeadphoneEq => " Headphone EQ ",
        ConfigureSubScreen::SpinoramaEq => " Spinorama EQ ",
    };

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(
            Style::default()
                .fg(app.theme.accent_primary)
                .bg(app.theme.bg_primary),
        )
        .style(Style::default().bg(app.theme.bg_primary).fg(app.theme.fg_primary))
        .title(format!("{} (Esc to close)", title));

    f.render_widget(outer, area);

    // Inner content area (inside border)
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    match app.configure_sub_screen {
        ConfigureSubScreen::Directories => draw_directory_manager(f, inner, app),
        ConfigureSubScreen::Recording   => draw_recording_screen(f, inner, app),
        ConfigureSubScreen::RoomEq      => draw_room_eq_screen(f, inner, app),
        ConfigureSubScreen::HeadphoneEq => draw_headphone_eq_screen(f, inner, app),
        ConfigureSubScreen::SpinoramaEq => draw_spinorama_eq_screen(f, inner, app),
    }
}

pub(crate) fn draw_recording_screen(f: &mut Frame, area: Rect, app: &App) {
    use ratatui::widgets::Tabs;
    use sotf_audio_player::recording_types::{
        ChannelRecording, ChannelRecordingState, RecordingStep,
    };

    let s = &app.recording;

    // Layout: step tabs on top, content below
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(area);

    // Step tabs
    let steps = [
        RecordingStep::Config,
        RecordingStep::Capture,
        RecordingStep::Evaluating,
        RecordingStep::Saving,
    ];
    let step_labels = ["Config", "Capture", "Evaluate", "Save"];
    let tab_titles: Vec<Line> = steps
        .iter()
        .zip(step_labels.iter())
        .map(|(st, label)| {
            let style = if *st == s.step {
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg_secondary)
            };
            Line::from(Span::styled(*label, style))
        })
        .collect();
    let step_idx = steps.iter().position(|st| *st == s.step).unwrap_or(0);
    let tabs = Tabs::new(tab_titles)
        .block(Block::default().borders(Borders::ALL).title("Recording"))
        .select(step_idx)
        .highlight_style(Style::default().fg(app.theme.accent_primary));
    f.render_widget(tabs, outer[0]);

    let content = outer[1];

    match s.step {
        RecordingStep::Config => {
            let bool_str = |b: bool| if b { "[ON]" } else { "[OFF]" };

            let playback_name = if s.available_playback_devices.is_empty() {
                "(no devices)".to_string()
            } else if let Some(d) = s.available_playback_devices.get(s.selected_playback_idx) {
                d.1.clone()
            } else {
                "(select)".to_string()
            };

            let recording_name = if s.available_recording_devices.is_empty() {
                "(no devices)".to_string()
            } else if let Some(d) = s.available_recording_devices.get(s.selected_recording_idx) {
                d.1.clone()
            } else {
                "(select)".to_string()
            };

            let rows: Vec<(Option<usize>, &str, String)> = vec![
                (None, "── Devices ──", String::new()),
                (Some(0), "Playback Device", playback_name),
                (Some(1), "Recording Device", recording_name),
                (
                    Some(2),
                    "Speaker Config",
                    s.playback_config.speaker_configuration.as_str().to_string(),
                ),
                (None, "── Signal ──", String::new()),
                (Some(3), "Signal Type", s.signal_type.as_str().to_string()),
                (
                    Some(4),
                    "Duration (s)",
                    format!("{:.1}", s.signal_duration_secs),
                ),
                (Some(5), "Level (dB)", format!("{:.1}", s.signal_level_db)),
                (
                    Some(6),
                    "Sweep Start (Hz)",
                    format!("{:.0}", s.sweep_start_freq),
                ),
                (
                    Some(7),
                    "Sweep End (Hz)",
                    format!("{:.0}", s.sweep_end_freq),
                ),
                (None, "── Paths ──", String::new()),
                (
                    Some(8),
                    "Output Directory",
                    if s.output_directory.is_empty() {
                        "<not set>".to_string()
                    } else {
                        s.output_directory.clone()
                    },
                ),
                (
                    Some(9),
                    "Mic Calibration",
                    if s.mic_calibration_path.is_empty() {
                        "<none>".to_string()
                    } else {
                        s.mic_calibration_path.clone()
                    },
                ),
            ];

            let mut lines: Vec<Line> = Vec::new();
            for (idx, label, value) in &rows {
                let is_selected = idx.map_or(false, |i| i == s.selected_field);
                let style = if is_selected {
                    Style::default()
                        .fg(app.theme.accent_primary)
                        .add_modifier(Modifier::BOLD)
                } else if idx.is_none() {
                    Style::default()
                        .fg(app.theme.fg_secondary)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.theme.fg_primary)
                };
                let arrow = if is_selected { "> " } else { "  " };
                lines.push(Line::from(Span::styled(
                    format!("{}{:<22} {}", arrow, label, value),
                    style,
                )));
            }

            // Channel mapping display
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Channels:",
                Style::default()
                    .fg(app.theme.fg_secondary)
                    .add_modifier(Modifier::BOLD),
            )));
            for mapping in &s.playback_config.channel_mappings {
                lines.push(Line::from(Span::styled(
                    format!(
                        "    {} → ch {}",
                        mapping.group_name,
                        mapping
                            .interface_channels
                            .iter()
                            .map(|c| c.to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    ),
                    Style::default().fg(app.theme.fg_primary),
                )));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " Up/Down=navigate  Left/Right=adjust  Enter=edit path  Tab=capture",
                Style::default().fg(app.theme.fg_secondary),
            )));

            let para = Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL).title("Configure"))
                .wrap(Wrap { trim: false });
            f.render_widget(para, content);
        }

        RecordingStep::Capture => {
            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // status
                    Constraint::Min(5),    // channel list
                    Constraint::Length(1), // help
                ])
                .split(content);

            // Status
            let status_text = if s.status_message.is_empty() {
                "Ready to record. Select a channel and press Enter.".to_string()
            } else {
                s.status_message.clone()
            };
            let status = Paragraph::new(status_text)
                .style(Style::default().fg(app.theme.accent_primary))
                .block(Block::default().borders(Borders::ALL).title("Status"));
            f.render_widget(status, inner[0]);

            // Channel list
            let header = Row::new(vec![
                Cell::from("#"),
                Cell::from("Channel"),
                Cell::from("State"),
            ])
            .style(
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            );

            let rows: Vec<Row> = s
                .channel_recordings
                .iter()
                .enumerate()
                .map(|(i, ch)| {
                    let is_current = s.current_channel == Some(i);
                    let state_str = match ch.state {
                        ChannelRecordingState::Empty => "[ ]",
                        ChannelRecordingState::Recording => "[REC]",
                        ChannelRecordingState::Done => "[OK]",
                        ChannelRecordingState::Error => "[ERR]",
                    };
                    let style = if is_current {
                        Style::default()
                            .fg(app.theme.accent_primary)
                            .add_modifier(Modifier::BOLD)
                    } else if ch.state == ChannelRecordingState::Done {
                        Style::default().fg(app.theme.accent_success)
                    } else {
                        Style::default().fg(app.theme.fg_primary)
                    };
                    Row::new(vec![
                        Cell::from(format!("{}", i + 1)),
                        Cell::from(ch.channel_name.clone()),
                        Cell::from(state_str),
                    ])
                    .style(style)
                })
                .collect();

            let ch_table = Table::new(
                rows,
                [
                    Constraint::Length(3),
                    Constraint::Length(12),
                    Constraint::Length(8),
                ],
            )
            .header(header)
            .block(Block::default().borders(Borders::ALL).title("Channels"));
            f.render_widget(ch_table, inner[1]);

            let help =
                Paragraph::new(" Up/Down=select  Enter=record  A=auto-record all  Tab=evaluate")
                    .style(Style::default().fg(app.theme.fg_secondary));
            f.render_widget(help, inner[2]);
        }

        RecordingStep::Evaluating => {
            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(5), // channel summary
                    Constraint::Min(3),    // selected channel details
                    Constraint::Length(1), // help
                ])
                .split(content);

            // Channel summary
            let completed: Vec<&ChannelRecording> = s
                .channel_recordings
                .iter()
                .filter(|ch| ch.state == ChannelRecordingState::Done)
                .collect();

            if completed.is_empty() {
                let placeholder =
                    Paragraph::new("No recordings completed yet. Go to Capture step.")
                        .style(Style::default().fg(app.theme.fg_secondary))
                        .alignment(Alignment::Center)
                        .block(Block::default().borders(Borders::ALL).title("Evaluate"));
                f.render_widget(placeholder, content);
                return;
            }

            let header = Row::new(vec![
                Cell::from("Channel"),
                Cell::from("Points"),
                Cell::from("Status"),
            ])
            .style(
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            );

            let rows: Vec<Row> = completed
                .iter()
                .enumerate()
                .map(|(i, ch)| {
                    let pts = ch
                        .result
                        .as_ref()
                        .map(|r| format!("{}", r.frequencies.len()))
                        .unwrap_or_else(|| "-".to_string());
                    let style = if i == s.selected_channel_view {
                        Style::default()
                            .fg(app.theme.accent_primary)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(app.theme.fg_primary)
                    };
                    Row::new(vec![
                        Cell::from(ch.channel_name.clone()),
                        Cell::from(pts),
                        Cell::from("OK"),
                    ])
                    .style(style)
                })
                .collect();

            let table = Table::new(
                rows,
                [
                    Constraint::Length(12),
                    Constraint::Length(10),
                    Constraint::Length(8),
                ],
            )
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Recorded Channels"),
            );
            f.render_widget(table, inner[0]);

            // Selected channel details
            if let Some(ch) = completed.get(s.selected_channel_view) {
                if let Some(ref result) = ch.result {
                    let mut details = vec![
                        Line::from(Span::styled(
                            format!(" Channel: {}", ch.channel_name),
                            Style::default().fg(app.theme.accent_primary),
                        )),
                        Line::from(format!(" Frequency points: {}", result.frequencies.len())),
                    ];
                    if let Some(ref thd) = result.thd_percent {
                        let avg_thd = thd.iter().copied().sum::<f32>() / thd.len().max(1) as f32;
                        details.push(Line::from(format!(" Avg THD: {:.2}%", avg_thd)));
                    }
                    if let Some(ref rt60) = result.rt60_ms {
                        let positive: Vec<f32> =
                            rt60.iter().copied().filter(|v| *v > 0.0).collect();
                        if !positive.is_empty() {
                            let avg_rt60 = positive.iter().sum::<f32>() / positive.len() as f32;
                            details.push(Line::from(format!(" Avg RT60: {:.0} ms", avg_rt60)));
                        }
                    }
                    let detail_para = Paragraph::new(details)
                        .block(Block::default().borders(Borders::ALL).title("Details"));
                    f.render_widget(detail_para, inner[1]);
                }
            }

            let help = Paragraph::new(" Up/Down=select channel  Tab=save  BackTab=capture")
                .style(Style::default().fg(app.theme.fg_secondary));
            f.render_widget(help, inner[2]);
        }

        RecordingStep::Saving => {
            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // save name
                    Constraint::Length(3), // status
                    Constraint::Min(1),    // help
                ])
                .split(content);

            let name_label = if s.editing_save_name {
                "Session Name (editing)"
            } else {
                "Session Name"
            };
            let name_style = Style::default().fg(app.theme.accent_primary);
            let name_para = Paragraph::new(if s.save_name.is_empty() {
                "<type session name>".to_string()
            } else {
                s.save_name.clone()
            })
            .style(name_style)
            .block(Block::default().borders(Borders::ALL).title(name_label));
            f.render_widget(name_para, inner[0]);

            // Status
            if let Some(ref err) = s.save_error {
                let err_para = Paragraph::new(err.as_str())
                    .style(Style::default().fg(app.theme.accent_error))
                    .block(Block::default().borders(Borders::ALL).title("Error"));
                f.render_widget(err_para, inner[1]);
            } else if s.save_success {
                let ok = Paragraph::new(" Recordings saved successfully!")
                    .style(Style::default().fg(app.theme.accent_success))
                    .block(Block::default().borders(Borders::ALL).title("Status"));
                f.render_widget(ok, inner[1]);
            } else {
                let completed = s
                    .channel_recordings
                    .iter()
                    .filter(|ch| ch.state == ChannelRecordingState::Done)
                    .count();
                let status = Paragraph::new(format!(
                    " {} channels ready to save. Output: {}",
                    completed,
                    if s.output_directory.is_empty() {
                        "<default>"
                    } else {
                        &s.output_directory
                    }
                ))
                .style(Style::default().fg(app.theme.fg_secondary))
                .block(Block::default().borders(Borders::ALL).title("Status"));
                f.render_widget(status, inner[1]);
            }

            let help = Paragraph::new(" Enter=edit name/save  Tab=config  BackTab=evaluate")
                .style(Style::default().fg(app.theme.fg_secondary));
            f.render_widget(help, inner[2]);
        }
    }
}

