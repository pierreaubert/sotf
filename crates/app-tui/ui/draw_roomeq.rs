use super::*;

pub(crate) fn draw_room_eq_screen(f: &mut Frame, area: Rect, app: &App) {
    use ratatui::widgets::{Gauge, Tabs};
    use sotf_audio_player::room_eq_types::{OptimizationStatus, RoomEqStep};

    let s = &app.room_eq;

    // Layout: step tabs on top, content below
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(area);

    // Step tabs
    let steps = RoomEqStep::all();
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
    let tabs = Tabs::new(tab_titles)
        .block(Block::default().borders(Borders::ALL).title("Room EQ"))
        .select(s.step.index())
        .highlight_style(Style::default().fg(app.theme.accent_primary));
    f.render_widget(tabs, outer[0]);

    let content = outer[1];

    match s.step {
        RoomEqStep::LoadData => {
            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // file path
                    Constraint::Length(3), // status/error
                    Constraint::Min(3),    // loaded channels
                    Constraint::Length(1), // help
                ])
                .split(content);

            let path_label = if s.editing_file_path {
                "Measurements JSON (editing)"
            } else {
                "Measurements JSON"
            };
            let path_style = Style::default().fg(app.theme.accent_primary);
            let path = Paragraph::new(if s.file_path.is_empty() {
                "<type path to recordings.json>".to_string()
            } else {
                s.file_path.clone()
            })
            .style(path_style)
            .block(Block::default().borders(Borders::ALL).title(path_label));
            f.render_widget(path, inner[0]);

            // Status/error
            if let Some(ref err) = s.load_error {
                let err_para = Paragraph::new(err.as_str())
                    .style(Style::default().fg(app.theme.accent_error))
                    .block(Block::default().borders(Borders::ALL).title("Error"));
                f.render_widget(err_para, inner[1]);
            } else if !s.channel_measurements.is_empty() {
                let status =
                    Paragraph::new(format!(" {} channels loaded", s.channel_measurements.len()))
                        .style(Style::default().fg(app.theme.accent_success))
                        .block(Block::default().borders(Borders::ALL).title("Status"));
                f.render_widget(status, inner[1]);
            } else {
                let status = Paragraph::new(" No data loaded")
                    .style(Style::default().fg(app.theme.fg_secondary))
                    .block(Block::default().borders(Borders::ALL).title("Status"));
                f.render_widget(status, inner[1]);
            }

            // Loaded channels list
            if !s.channel_measurements.is_empty() {
                let rows: Vec<Row> = s
                    .channel_measurements
                    .iter()
                    .map(|m| {
                        Row::new(vec![
                            Cell::from(m.channel_name.clone()),
                            Cell::from(format!("{} pts", m.measurement.frequencies.len())),
                            Cell::from(if m.is_group { "Group" } else { "Single" }),
                        ])
                    })
                    .collect();

                let header = Row::new(vec![
                    Cell::from("Channel"),
                    Cell::from("Points"),
                    Cell::from("Type"),
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
                        Constraint::Length(10),
                        Constraint::Length(8),
                    ],
                )
                .header(header)
                .block(Block::default().borders(Borders::ALL).title("Channels"));
                f.render_widget(table, inner[2]);
            }

            let help = Paragraph::new(" Enter=edit path  Tab=next step (after loading)")
                .style(Style::default().fg(app.theme.fg_secondary));
            f.render_widget(help, inner[3]);
        }

        RoomEqStep::Configure => {
            let c = &s.config;
            let bool_str = |b: bool| if b { "[ON]" } else { "[OFF]" };

            let rows: Vec<(Option<usize>, &str, String)> = vec![
                (None, "── Basic ──", String::new()),
                (Some(0), "Filters (n)", format!("{}", c.num_filters)),
                (Some(1), "Min Freq (Hz)", format!("{:.0}", c.min_freq)),
                (Some(2), "Max Freq (Hz)", format!("{:.0}", c.max_freq)),
                (Some(3), "Min dB", format!("{:.1}", c.min_db)),
                (Some(4), "Max dB", format!("{:.1}", c.max_db)),
                (Some(5), "Min Q", format!("{:.2}", c.min_q)),
                (Some(6), "Max Q", format!("{:.2}", c.max_q)),
                (Some(7), "PEQ Model", c.peq_model.clone()),
                (None, "── Optimization ──", String::new()),
                (Some(8), "Algorithm", c.algorithm.clone()),
                (Some(9), "Max Iter", format!("{}", c.max_iter)),
                (Some(10), "Population", format!("{}", c.population)),
                (Some(11), "Refine", bool_str(c.refine).to_string()),
                (Some(12), "Local Algo", c.local_algo.clone()),
                (
                    Some(13),
                    "Psychoacoustic",
                    bool_str(c.psychoacoustic).to_string(),
                ),
                (
                    Some(14),
                    "Asymmetric Loss",
                    bool_str(c.asymmetric_loss).to_string(),
                ),
                (None, "── Mode ──", String::new()),
                (Some(15), "Mode", c.mode.as_str().to_string()),
                (
                    Some(16),
                    "Multi-Speaker",
                    c.multi_speaker_mode.as_str().to_string(),
                ),
                (None, "── Target Tilt ──", String::new()),
                (
                    Some(17),
                    "Target Tilt",
                    bool_str(c.target_tilt.enabled).to_string(),
                ),
                (
                    Some(18),
                    "Slope (dB/oct)",
                    format!("{:.1}", c.target_tilt.slope),
                ),
                (None, "── Excursion ──", String::new()),
                (
                    Some(19),
                    "Excursion Prot.",
                    bool_str(c.excursion_protection.enabled).to_string(),
                ),
                (
                    Some(20),
                    "Manual F3 (Hz)",
                    format!("{:.0}", c.excursion_protection.manual_f3_hz),
                ),
                (None, "── Schroeder Split ──", String::new()),
                (
                    Some(21),
                    "Schroeder Split",
                    bool_str(c.schroeder_split.enabled).to_string(),
                ),
                (
                    Some(22),
                    "Schroeder Freq",
                    format!("{:.0}", c.schroeder_split.schroeder_freq),
                ),
                (None, "── Phase ──", String::new()),
                (
                    Some(23),
                    "Phase Alignment",
                    bool_str(c.phase_alignment.enabled).to_string(),
                ),
            ];

            let channels_info = if s.channel_measurements.is_empty() {
                "No data".to_string()
            } else {
                let names: Vec<&str> = s
                    .channel_measurements
                    .iter()
                    .map(|m| m.channel_name.as_str())
                    .collect();
                names.join(", ")
            };

            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Channels: ", Style::default().fg(app.theme.fg_secondary)),
                    Span::styled(channels_info, Style::default().fg(app.theme.accent_primary)),
                ]),
                Line::from(""),
            ];

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
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " Up/Down=navigate  Left/Right=adjust  Enter/Tab=optimize",
                Style::default().fg(app.theme.fg_secondary),
            )));

            let para = Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL).title("Configure"))
                .wrap(Wrap { trim: false });
            f.render_widget(para, content);
        }

        RoomEqStep::Optimize => {
            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(5), // status
                    Constraint::Length(3), // progress bar
                    Constraint::Min(3),    // loss chart or hint
                ])
                .split(content);

            let (status_text, status_style) = match &s.opt_status {
                OptimizationStatus::Idle => (
                    "Ready to optimize. Press Enter to start.".to_string(),
                    Style::default().fg(app.theme.fg_secondary),
                ),
                OptimizationStatus::Running => (
                    format!(
                        "Optimizing... iter {}/{} | loss: {:.4}",
                        s.opt_iteration, s.opt_max_iter, s.opt_loss
                    ),
                    Style::default().fg(app.theme.accent_primary),
                ),
                OptimizationStatus::Completed => (
                    format!("Completed! {} channel results", s.channel_results.len()),
                    Style::default().fg(app.theme.accent_success),
                ),
                OptimizationStatus::Failed => (
                    format!(
                        "Failed: {}",
                        s.opt_error.as_deref().unwrap_or("unknown error")
                    ),
                    Style::default().fg(app.theme.accent_error),
                ),
                OptimizationStatus::Cancelled => (
                    "Cancelled".to_string(),
                    Style::default().fg(app.theme.accent_error),
                ),
            };

            let status_para =
                Paragraph::new(vec![Line::from(Span::styled(status_text, status_style))])
                    .block(Block::default().borders(Borders::ALL).title("Optimization"));
            f.render_widget(status_para, inner[0]);

            // Progress bar
            let pct = (s.opt_progress * 100.0) as u16;
            let gauge = Gauge::default()
                .block(Block::default().borders(Borders::ALL).title("Progress"))
                .gauge_style(Style::default().fg(app.theme.accent_primary))
                .percent(pct.min(100));
            f.render_widget(gauge, inner[1]);

            // Hint
            let hint = match &s.opt_status {
                OptimizationStatus::Idle => " Enter=start  BackTab=back to configure",
                OptimizationStatus::Running => " Optimization running...",
                OptimizationStatus::Completed => " Enter or Tab=view results",
                OptimizationStatus::Failed | OptimizationStatus::Cancelled => {
                    " Enter=retry  BackTab=back to configure"
                }
            };
            let hint_para = Paragraph::new(hint)
                .style(Style::default().fg(app.theme.fg_secondary))
                .block(Block::default().borders(Borders::ALL).title("Info"));
            f.render_widget(hint_para, inner[2]);
        }

        RoomEqStep::Review => {
            if s.channel_results.is_empty() {
                let placeholder =
                    Paragraph::new("No optimization results yet. Go to Optimize step first.")
                        .style(Style::default().fg(app.theme.fg_secondary))
                        .alignment(Alignment::Center)
                        .block(Block::default().borders(Borders::ALL).title("Review"));
                f.render_widget(placeholder, content);
                return;
            }

            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(5), // channel summary table
                    Constraint::Min(5),    // selected channel filters
                ])
                .split(content);

            // Channel summary
            let header = Row::new(vec![
                Cell::from("Channel"),
                Cell::from("Pre Score"),
                Cell::from("Post Score"),
                Cell::from("Filters"),
            ])
            .style(
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            );

            let rows: Vec<Row> = s
                .channel_results
                .iter()
                .enumerate()
                .map(|(i, ch)| {
                    let style = if i == s.selected_channel {
                        Style::default()
                            .fg(app.theme.accent_primary)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(app.theme.fg_primary)
                    };
                    Row::new(vec![
                        Cell::from(ch.channel_name.clone()),
                        Cell::from(format!("{:.2}", ch.pre_score)),
                        Cell::from(format!("{:.2}", ch.post_score)),
                        Cell::from(format!("{}", ch.eq_filters.len())),
                    ])
                    .style(style)
                })
                .collect();

            let ch_table = Table::new(
                rows,
                [
                    Constraint::Length(12),
                    Constraint::Length(12),
                    Constraint::Length(12),
                    Constraint::Length(8),
                ],
            )
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Channels (Up/Down to select)"),
            );
            f.render_widget(ch_table, inner[0]);

            // Selected channel filters
            if let Some(ch) = s.channel_results.get(s.selected_channel) {
                let filt_header = Row::new(vec![
                    Cell::from("#"),
                    Cell::from("Type"),
                    Cell::from("Freq (Hz)"),
                    Cell::from("Q"),
                    Cell::from("Gain (dB)"),
                ])
                .style(
                    Style::default()
                        .fg(app.theme.accent_primary)
                        .add_modifier(Modifier::BOLD),
                );

                let filt_rows: Vec<Row> = ch
                    .eq_filters
                    .iter()
                    .enumerate()
                    .map(|(i, filt)| {
                        Row::new(vec![
                            Cell::from(format!("{}", i + 1)),
                            Cell::from(filt.filter_type.clone()),
                            Cell::from(format!("{:.1}", filt.frequency)),
                            Cell::from(format!("{:.2}", filt.q)),
                            Cell::from(format!("{:+.1}", filt.gain_db)),
                        ])
                    })
                    .collect();

                let filt_table = Table::new(
                    filt_rows,
                    [
                        Constraint::Length(3),
                        Constraint::Length(12),
                        Constraint::Length(12),
                        Constraint::Length(8),
                        Constraint::Length(10),
                    ],
                )
                .header(filt_header)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!("Filters: {}", ch.channel_name)),
                );
                f.render_widget(filt_table, inner[1]);
            }
        }

        RoomEqStep::Export => {
            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // export path
                    Constraint::Length(3), // status
                    Constraint::Min(1),    // help
                ])
                .split(content);

            let path_label = if s.editing_export_path {
                "Export Path (editing)"
            } else {
                "Export Path"
            };
            let path_style = Style::default().fg(app.theme.accent_primary);
            let path = Paragraph::new(if s.export_path.is_empty() {
                "<type path for JSON export>".to_string()
            } else {
                s.export_path.clone()
            })
            .style(path_style)
            .block(Block::default().borders(Borders::ALL).title(path_label));
            f.render_widget(path, inner[0]);

            // Status
            if let Some(ref err) = s.export_error {
                let err_para = Paragraph::new(err.as_str())
                    .style(Style::default().fg(app.theme.accent_error))
                    .block(Block::default().borders(Borders::ALL).title("Error"));
                f.render_widget(err_para, inner[1]);
            } else if s.export_success {
                let ok = Paragraph::new(" Export successful!")
                    .style(Style::default().fg(app.theme.accent_success))
                    .block(Block::default().borders(Borders::ALL).title("Status"));
                f.render_widget(ok, inner[1]);
            } else {
                let hint = Paragraph::new(" Enter=edit path, type path and Enter to export")
                    .style(Style::default().fg(app.theme.fg_secondary))
                    .block(Block::default().borders(Borders::ALL).title("Status"));
                f.render_widget(hint, inner[1]);
            }

            let help = Paragraph::new(" Enter=edit/export  Tab=back to load  BackTab=review")
                .style(Style::default().fg(app.theme.fg_secondary));
            f.render_widget(help, inner[2]);
        }
    }
}

