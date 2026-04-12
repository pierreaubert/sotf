use super::*;

pub(crate) fn draw_devices_screen(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Help box
            Constraint::Min(0),    // Device list
        ])
        .split(area);

    draw_help_box_with_text(f, chunks[0], app, "↑↓=Navigate  Enter=Select  Esc=Back");

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
                    if is_selected {
                        selected_style
                    } else {
                        normal_style
                    },
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

    draw_help_box_with_text(f, chunks[0], app, "↑↓=Navigate  Enter=Open  Esc=Back");

    let options: &[(ConfigureSubScreen, &str, &str)] = &[
        (
            ConfigureSubScreen::Directories,
            "1",
            "Directories        – Music library folders",
        ),
        (
            ConfigureSubScreen::Recording,
            "2",
            "Recording          – Measure impulse responses",
        ),
        (
            ConfigureSubScreen::RoomEq,
            "3",
            "Room EQ            – Optimize room correction filters",
        ),
        (
            ConfigureSubScreen::HeadphoneEq,
            "4",
            "Headphone EQ       – Target-curve EQ for headphones",
        ),
        (
            ConfigureSubScreen::SpinoramaEq,
            "5",
            "Spinorama EQ       – Speaker EQ from spinorama data",
        ),
        (
            ConfigureSubScreen::FederationSources,
            "6",
            "Library Sources    – Remote libraries (Subsonic, MPD, DLNA, Peer)",
        ),
        (
            ConfigureSubScreen::Servers,
            "7",
            "Servers            – MPD and DLNA server settings",
        ),
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

    // Use Double border when the configure menu is focused
    let menu_border = if app.input_mode == InputMode::Configure {
        BorderType::Double
    } else {
        BorderType::Rounded
    };
    let menu_border_color = if app.input_mode == InputMode::Configure {
        app.theme.accent_primary
    } else {
        app.theme.border_color
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(menu_border)
                .border_style(Style::default().fg(menu_border_color))
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
        ConfigureSubScreen::Recording => " Recording ",
        ConfigureSubScreen::RoomEq => " Room EQ ",
        ConfigureSubScreen::HeadphoneEq => " Headphone EQ ",
        ConfigureSubScreen::SpinoramaEq => " Spinorama EQ ",
        ConfigureSubScreen::FederationSources => " Library Sources ",
        ConfigureSubScreen::Servers => " Servers ",
    };

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(
            Style::default()
                .fg(app.theme.accent_primary)
                .bg(app.theme.bg_primary),
        )
        .style(
            Style::default()
                .bg(app.theme.bg_primary)
                .fg(app.theme.fg_primary),
        )
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
        ConfigureSubScreen::Recording => draw_recording_screen(f, inner, app),
        ConfigureSubScreen::RoomEq => draw_room_eq_screen(f, inner, app),
        ConfigureSubScreen::HeadphoneEq => draw_headphone_eq_screen(f, inner, app),
        ConfigureSubScreen::SpinoramaEq => draw_spinorama_eq_screen(f, inner, app),
        ConfigureSubScreen::FederationSources => draw_federation_screen(f, inner, app),
        ConfigureSubScreen::Servers => draw_servers_screen(f, inner, app),
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

    // Border types: Double for focused area, Plain for unfocused
    let step_tab_border = if s.step_tab_focused {
        BorderType::Double
    } else {
        BorderType::Plain
    };
    let content_border = if s.step_tab_focused {
        BorderType::Plain
    } else {
        BorderType::Double
    };
    let step_tab_border_color = if s.step_tab_focused {
        app.theme.accent_primary
    } else {
        app.theme.border_color
    };
    let content_border_color = if s.step_tab_focused {
        app.theme.border_color
    } else {
        app.theme.accent_primary
    };

    // Step tabs — built from `RecordingStep::all()` so new variants
    // (e.g. `Probe`) show up automatically. The Room EQ wizard had a
    // bug where a hand-rolled `Vec<WizardStep>` list silently dropped
    // newly-added variants; we don't repeat the pattern here.
    let steps = RecordingStep::all();
    let tab_titles: Vec<Line> = steps
        .iter()
        .map(|st| {
            let style = if *st == s.step {
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg_secondary)
            };
            Line::from(Span::styled(st.label(), style))
        })
        .collect();
    let step_idx = steps.iter().position(|st| *st == s.step).unwrap_or(0);
    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(step_tab_border)
                .border_style(Style::default().fg(step_tab_border_color))
                .title("Recording"),
        )
        .select(step_idx)
        .highlight_style(Style::default().fg(app.theme.accent_primary));
    f.render_widget(tabs, outer[0]);

    // Content wrapper with focus-aware border
    let content_wrapper = Block::default()
        .borders(Borders::ALL)
        .border_type(content_border)
        .border_style(Style::default().fg(content_border_color));
    f.render_widget(content_wrapper, outer[1]);
    let content = Rect {
        x: outer[1].x + 1,
        y: outer[1].y + 1,
        width: outer[1].width.saturating_sub(2),
        height: outer[1].height.saturating_sub(2),
    };

    match s.step {
        RecordingStep::Config => {
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
                let is_selected = idx.is_some_and(|i| i == s.selected_field);
                let is_editing_numerical = is_selected && s.editing_value;
                let is_editing_path = is_selected
                    && match idx {
                        Some(8) => s.editing_output_dir,
                        Some(9) => s.editing_mic_cal,
                        _ => false,
                    };
                let is_editing = is_editing_numerical || is_editing_path;
                let display_value = if is_editing_numerical {
                    format!("{}▏", s.edit_buffer)
                } else if is_editing_path {
                    let path_val = match idx {
                        Some(8) => &s.output_directory,
                        Some(9) => &s.mic_calibration_path,
                        _ => value,
                    };
                    format!("{}▏", path_val)
                } else {
                    value.clone()
                };
                let style = if is_editing {
                    Style::default()
                        .fg(app.theme.fg_selected)
                        .bg(app.theme.bg_selected)
                        .add_modifier(Modifier::BOLD)
                } else if is_selected {
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
                let arrow = if is_editing {
                    "✎ "
                } else if is_selected {
                    "> "
                } else {
                    "  "
                };
                lines.push(Line::from(Span::styled(
                    format!("{}{:<22} {}", arrow, label, display_value),
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
            let hint = if s.editing_value {
                " Type value, Enter=confirm  Esc=cancel"
            } else if s.editing_output_dir || s.editing_mic_cal {
                " Type path, Tab=complete  Enter=confirm  F2=browse  Esc=cancel"
            } else {
                " Up/Down=navigate  Left/Right=adjust  Enter=edit value/path  Tab=next field"
            };
            lines.push(Line::from(Span::styled(
                hint,
                Style::default().fg(app.theme.fg_secondary),
            )));

            let para = Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL).title("Configure"))
                .wrap(Wrap { trim: false });
            f.render_widget(para, content);

            // Autocomplete overlay for output_dir / mic_cal path fields
            if s.editing_output_dir || s.editing_mic_cal {
                let ac_h = autocomplete_dropdown_height(app);
                if ac_h > 0 {
                    // Position below the config block
                    let ac_y = content.y + content.height;
                    let available = area.height.saturating_sub(ac_y);
                    if available > 2 {
                        let ac_area = Rect {
                            x: content.x,
                            y: ac_y,
                            width: content.width,
                            height: ac_h.min(available),
                        };
                        f.render_widget(Clear, ac_area);
                        render_autocomplete_dropdown(f, ac_area, app);
                    }
                }
            }
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

        RecordingStep::Probe => {
            draw_recording_probe_step(f, content, app);
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
            if let Some(ch) = completed.get(s.selected_channel_view)
                && let Some(ref result) = ch.result
            {
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
                    let positive: Vec<f32> = rt60.iter().copied().filter(|v| *v > 0.0).collect();
                    if !positive.is_empty() {
                        let avg_rt60 = positive.iter().sum::<f32>() / positive.len() as f32;
                        details.push(Line::from(format!(" Avg RT60: {:.0} ms", avg_rt60)));
                    }
                }
                let detail_para = Paragraph::new(details)
                    .block(Block::default().borders(Borders::ALL).title("Details"));
                f.render_widget(detail_para, inner[1]);
            }

            let help = Paragraph::new(" Up/Down=select channel  Tab=save  BackTab=capture")
                .style(Style::default().fg(app.theme.fg_secondary));
            f.render_widget(help, inner[2]);
        }

        RecordingStep::Saving => {
            draw_recording_saving_step(f, content, app);
        }
    }
}

/// Save-step renderer for the Recording wizard.
///
/// Lays out five boxes vertically: session name, room dimensions,
/// setup description, per-channel speakers, and a status/help strip.
/// The currently-focused field (`selected_save_field`) is highlighted
/// with the accent color; when `editing_save_value` is set, the field
/// shows a `>` marker and echoes `edit_buffer`.
///
/// Selected-field layout:
///   0      Session name
///   1..=3  Room width / depth / height
///   4      Unit toggle (Metric / Imperial)
///   5      Setup description
///   6..    Per-channel speaker entries (one index per `channel_recordings`)
fn draw_recording_saving_step(f: &mut Frame, content: Rect, app: &App) {
    use sotf_audio_player::recording_types::{ChannelRecordingState, RoomDimensionUnit};

    let s = &app.recording;
    let channel_count = s.channel_recordings.len();
    // Results table (speakers per channel) grows with the channel
    // count; 1 row per channel + 2 lines borders + 1 line dropdown
    // overlay when editing.
    let speakers_rows = channel_count.max(1) as u16 + 2;
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),             // session name
            Constraint::Length(5),             // room dimensions
            Constraint::Length(3),             // setup description
            Constraint::Length(speakers_rows), // speakers per channel
            Constraint::Length(3),             // status
            Constraint::Length(1),             // help
        ])
        .split(content);

    let focused = |idx: usize| -> Style {
        if s.selected_save_field == idx {
            Style::default()
                .fg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.fg_primary)
        }
    };
    let is_editing_field = |idx: usize| s.editing_save_value && s.selected_save_field == idx;
    let field_text = |idx: usize, value: String, placeholder: &str| -> String {
        if is_editing_field(idx) {
            format!("> {}_", s.edit_buffer)
        } else if value.is_empty() {
            format!("<{}>", placeholder)
        } else {
            value
        }
    };

    // --- Session Name ------------------------------------------------
    let name_title = if is_editing_field(0) {
        "Session Name (editing)"
    } else {
        "Session Name"
    };
    let name_para = Paragraph::new(field_text(0, s.save_name.clone(), "type session name"))
        .style(focused(0))
        .block(Block::default().borders(Borders::ALL).title(name_title));
    f.render_widget(name_para, inner[0]);

    // --- Room Dimensions ---------------------------------------------
    // Single line with four "cells" separated by spaces. Each cell is
    // rendered via a sub-paragraph so the accent-bold style only lands
    // on the focused one.
    let room_block = Block::default()
        .borders(Borders::ALL)
        .title(format!("Room Dimensions ({})", s.save_room_unit.label()));
    let room_inner = room_block.inner(inner[1]);
    f.render_widget(room_block, inner[1]);

    let cells = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(room_inner);
    let dim_str = |idx: usize, v: f64| {
        field_text(
            idx,
            if v > 0.0 {
                format!("{:.2}", v)
            } else {
                String::new()
            },
            "0.00",
        )
    };
    f.render_widget(
        Paragraph::new(format!(" W: {}", dim_str(1, s.save_room_width))).style(focused(1)),
        cells[0],
    );
    f.render_widget(
        Paragraph::new(format!(" D: {}", dim_str(2, s.save_room_depth))).style(focused(2)),
        cells[1],
    );
    f.render_widget(
        Paragraph::new(format!(" H: {}", dim_str(3, s.save_room_height))).style(focused(3)),
        cells[2],
    );
    let unit_marker = match s.save_room_unit {
        RoomDimensionUnit::Metric => "[Metric]",
        RoomDimensionUnit::Imperial => "[Imperial]",
    };
    f.render_widget(
        Paragraph::new(format!(" {}", unit_marker)).style(focused(4)),
        cells[3],
    );

    // --- Setup Description -------------------------------------------
    let desc_title = if is_editing_field(5) {
        "Setup Description (editing)"
    } else {
        "Setup Description"
    };
    let desc_para = Paragraph::new(field_text(
        5,
        s.setup_description.clone(),
        "describe treatment, seating, equipment",
    ))
    .style(focused(5))
    .block(Block::default().borders(Borders::ALL).title(desc_title));
    f.render_widget(desc_para, inner[2]);

    // --- Speakers per Channel ----------------------------------------
    let catalog = &app.spinorama_eq.available_speakers;
    let spk_title = if catalog.is_empty() {
        "Speakers per Channel  (catalog loading…)"
    } else {
        "Speakers per Channel"
    };
    let spk_block = Block::default().borders(Borders::ALL).title(spk_title);
    let spk_inner = spk_block.inner(inner[3]);
    f.render_widget(spk_block, inner[3]);
    if channel_count == 0 {
        f.render_widget(
            Paragraph::new(" No channels yet — record some first.")
                .style(Style::default().fg(app.theme.fg_secondary)),
            spk_inner,
        );
    } else {
        let rows: Vec<Row> = s
            .channel_recordings
            .iter()
            .enumerate()
            .map(|(i, rec)| {
                let field_idx = 6 + i;
                let current = s.channel_speakers.get(i).cloned().unwrap_or_default();
                let cell_value = if is_editing_field(field_idx) {
                    format!("> {}_", s.edit_buffer)
                } else if current.is_empty() {
                    "<empty>".to_string()
                } else {
                    current
                };
                let row_style = if s.selected_save_field == field_idx {
                    Style::default()
                        .fg(app.theme.accent_primary)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                Row::new(vec![
                    Cell::from(format!(" {}", rec.channel_name.clone())),
                    Cell::from(cell_value),
                ])
                .style(row_style)
            })
            .collect();
        let table = Table::new(rows, [Constraint::Length(8), Constraint::Percentage(90)]);
        f.render_widget(table, spk_inner);
    }

    // --- Status / suggestions ----------------------------------------
    // When editing a channel-speaker field, repurpose the status box
    // to show pipe-separated autocomplete matches from the spinorama
    // catalog. The user types freely in the input; this line is a
    // visual hint — they still commit with Enter.
    let editing_speaker = s.editing_save_value
        && s.selected_save_field >= 6
        && s.selected_save_field < 6 + channel_count;
    if editing_speaker {
        let q = s.edit_buffer.to_lowercase();
        let matches: Vec<String> = catalog
            .iter()
            .filter(|name| !q.is_empty() && name.to_lowercase().contains(&q))
            .take(5)
            .cloned()
            .collect();
        let hint = if matches.is_empty() && catalog.is_empty() {
            " Loading catalog…".to_string()
        } else if matches.is_empty() {
            " No matches — free-form text is saved as-is".to_string()
        } else {
            format!(" ▸ {}", matches.join(" | "))
        };
        let suggestions = Paragraph::new(hint)
            .style(Style::default().fg(app.theme.accent_primary))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Suggestions (spinorama.org)"),
            );
        f.render_widget(suggestions, inner[4]);
    } else if let Some(ref err) = s.save_error {
        let err_para = Paragraph::new(err.as_str())
            .style(Style::default().fg(app.theme.accent_error))
            .block(Block::default().borders(Borders::ALL).title("Error"));
        f.render_widget(err_para, inner[4]);
    } else if s.save_success {
        let ok = Paragraph::new(" Recordings saved successfully!")
            .style(Style::default().fg(app.theme.accent_success))
            .block(Block::default().borders(Borders::ALL).title("Status"));
        f.render_widget(ok, inner[4]);
    } else {
        let completed = s
            .channel_recordings
            .iter()
            .filter(|ch| ch.state == ChannelRecordingState::Done)
            .count();
        let status = Paragraph::new(format!(
            " {} channels ready. Output: {}",
            completed,
            if s.output_directory.is_empty() {
                "<default>"
            } else {
                &s.output_directory
            }
        ))
        .style(Style::default().fg(app.theme.fg_secondary))
        .block(Block::default().borders(Borders::ALL).title("Status"));
        f.render_widget(status, inner[4]);
    }

    // --- Help --------------------------------------------------------
    let help_text = if s.editing_save_value {
        " Type value | Enter=confirm | Esc=cancel"
    } else {
        " Tab=next field  ↑↓=nav  Enter=edit  u=unit  Ctrl+S=save"
    };
    f.render_widget(
        Paragraph::new(help_text).style(Style::default().fg(app.theme.fg_secondary)),
        inner[5],
    );
}

/// Probe-step renderer for the Recording wizard.
///
/// Three panes: probe/silence/mic form, status/progress banner, and a
/// per-channel results table populated from
/// `ProbeCaptureState.results` after a successful capture. Mirrors the
/// Room EQ Delay Detection step layout so the two feel consistent —
/// differences are:
///   - channel list is seeded from `channel_recordings` (Capture step
///     already ran) rather than loaded measurements.
///   - on success, also shows the persisted WAV path under "Results".
fn draw_recording_probe_step(f: &mut Frame, content: Rect, app: &App) {
    use sotf_audio_player::recording_types::ProbeCaptureStatus;
    use sotf_audio_player::room_eq_types::estimate_probe_sequence_ms;

    let s = &app.recording;
    let pc = &s.probe_capture;
    let channel_count = s.channel_recordings.len();

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // form
            Constraint::Length(3), // status
            Constraint::Min(5),    // results
            Constraint::Length(1), // help
        ])
        .split(content);

    let focused = |idx: usize| -> Style {
        if s.probe_selected_field == idx {
            Style::default()
                .fg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.fg_primary)
        }
    };
    let is_editing = |idx: usize| s.probe_editing_value && s.probe_selected_field == idx;
    let field_text = |idx: usize, val: String, placeholder: &str| -> String {
        if is_editing(idx) {
            format!("> {}_", s.edit_buffer)
        } else if val.is_empty() {
            format!("<{}>", placeholder)
        } else {
            val
        }
    };

    // --- Form ----------------------------------------------------------
    let form_rows = vec![
        Row::new(vec![
            Cell::from("Probe duration (ms)").style(focused(0)),
            Cell::from(field_text(
                0,
                format!("{:.0}", pc.probe_duration_ms),
                "1000",
            ))
            .style(focused(0)),
        ]),
        Row::new(vec![
            Cell::from("Silence gap (ms)").style(focused(1)),
            Cell::from(field_text(
                1,
                format!("{:.0}", pc.silence_duration_ms),
                "500",
            ))
            .style(focused(1)),
        ]),
        Row::new(vec![
            Cell::from("Mic input channel").style(focused(2)),
            Cell::from(field_text(2, format!("{}", pc.input_channel), "0")).style(focused(2)),
        ]),
        Row::new(vec![
            Cell::from("[ Run Probe ]").style(focused(3)),
            Cell::from(match pc.status {
                ProbeCaptureStatus::Running { .. } => "running...",
                ProbeCaptureStatus::Complete => "done",
                ProbeCaptureStatus::Failed(_) => "failed",
                ProbeCaptureStatus::Idle => "press r or Enter",
            })
            .style(focused(3)),
        ]),
    ];
    let form = Table::new(
        form_rows,
        [Constraint::Length(24), Constraint::Percentage(60)],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Delay Probe Capture"),
    );
    f.render_widget(form, inner[0]);

    // --- Status banner -------------------------------------------------
    let estimated_total =
        estimate_probe_sequence_ms(channel_count, pc.probe_duration_ms, pc.silence_duration_ms);
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let (status_text, status_color) = match &pc.status {
        ProbeCaptureStatus::Idle => (
            "Idle — press `r` to capture tone-burst delays".to_string(),
            app.theme.fg_secondary,
        ),
        ProbeCaptureStatus::Running { .. } => {
            let pct = pc
                .status
                .progress(estimated_total, now_ms)
                .map(|p| format!("{:.0}%", p * 100.0))
                .unwrap_or_else(|| "…".to_string());
            (format!("Running... {}", pct), app.theme.accent_primary)
        }
        ProbeCaptureStatus::Complete => {
            let n = pc.results.as_ref().map(|r| r.channels.len()).unwrap_or(0);
            (
                format!("Complete — detected {} channel(s)", n),
                app.theme.accent_success,
            )
        }
        ProbeCaptureStatus::Failed(msg) => (format!("Failed: {}", msg), app.theme.accent_error),
    };
    let status = Paragraph::new(status_text)
        .style(Style::default().fg(status_color))
        .block(Block::default().borders(Borders::ALL).title("Status"));
    f.render_widget(status, inner[1]);

    // --- Results table -------------------------------------------------
    if let Some(results) = pc.results.as_ref() {
        let rows: Vec<Row> = results
            .channels
            .iter()
            .enumerate()
            .map(|(i, ch)| {
                let snr_color = if ch.snr_db > 10.0 {
                    app.theme.accent_success
                } else if ch.snr_db > 0.0 {
                    app.theme.accent_primary
                } else {
                    app.theme.accent_error
                };
                let align = results.alignment_delays_ms.get(i).copied().unwrap_or(0.0);
                Row::new(vec![
                    Cell::from(ch.channel_name.clone()),
                    Cell::from(format!("{:.2}", ch.arrival_ms)),
                    Cell::from(format!("{:+.1}", ch.gain_db)),
                    Cell::from(format!("{:+.1}", ch.snr_db)).style(Style::default().fg(snr_color)),
                    Cell::from(format!("{:.2}", align)),
                ])
            })
            .collect();
        let header = Row::new(vec![
            Cell::from("Channel"),
            Cell::from("Arrival ms"),
            Cell::from("Gain dB"),
            Cell::from("SNR dB"),
            Cell::from("Align ms"),
        ])
        .style(
            Style::default()
                .fg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        );
        let table = Table::new(
            rows,
            [
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(12),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("Results"));
        f.render_widget(table, inner[2]);
    } else {
        let empty = Paragraph::new(" No probe captured yet — press `r` to run")
            .style(Style::default().fg(app.theme.fg_secondary))
            .block(Block::default().borders(Borders::ALL).title("Results"));
        f.render_widget(empty, inner[2]);
    }

    // --- Help ----------------------------------------------------------
    let help = if s.probe_editing_value {
        " Type value | Enter=confirm | Esc=cancel"
    } else {
        " Tab=next field  ←→=adjust  r=run  Tab=evaluate"
    };
    f.render_widget(
        Paragraph::new(help).style(Style::default().fg(app.theme.fg_secondary)),
        inner[3],
    );
}
