use super::super::*;

pub(crate) fn draw_room_eq_screen(f: &mut Frame, area: Rect, app: &App) {
    use ratatui::widgets::{Gauge, Tabs};
    use sotf_audio_player::room_eq_types::{OptimizationStatus, RoomEqStep};

    let s = &app.room_eq;

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

    // Step tabs
    let steps = RoomEqStep::all();
    let tab_titles: Vec<Line> = steps
        .iter()
        .map(|st| {
            let style = if *st == s.model.step {
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
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(step_tab_border)
                .border_style(Style::default().fg(step_tab_border_color))
                .title("Room EQ"),
        )
        .select(s.model.step.index())
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

    match s.model.step {
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
            } else if !s.model.channel_measurements.is_empty() {
                let status =
                    Paragraph::new(format!(" {} channels loaded", s.model.channel_measurements.len()))
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
            if !s.model.channel_measurements.is_empty() {
                let rows: Vec<Row> = s
                    .model
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

            let help_text = if s.editing_file_path {
                " Enter=confirm  F2=browse  Tab=autocomplete  Esc=cancel"
            } else {
                " Enter=browse for JSON  Tab=next step"
            };
            let help = Paragraph::new(help_text).style(Style::default().fg(app.theme.fg_secondary));
            f.render_widget(help, inner[3]);

            // Autocomplete overlay below the file path input
            if s.editing_file_path {
                let ac_h = autocomplete_dropdown_height(app);
                if ac_h > 0 {
                    let ac_area = Rect {
                        x: inner[0].x,
                        y: inner[0].y + inner[0].height,
                        width: inner[0].width,
                        height: ac_h.min(area.height.saturating_sub(inner[0].y + inner[0].height)),
                    };
                    f.render_widget(Clear, ac_area);
                    render_autocomplete_dropdown(f, ac_area, app);
                }
            }
        }

        RoomEqStep::Delay => {
            draw_delay_detection_step(f, content, app);
        }

        RoomEqStep::Process => {
            use sotf_audio_player::room_eq_types::RoomEqWizardMode;
            let mode = app.room_eq.model.wizard_mode;
            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // title
                    Constraint::Length(5), // simple card
                    Constraint::Length(5), // full card
                    Constraint::Length(1), // help
                ])
                .split(content);

            let title = Paragraph::new(" Choose your optimization workflow")
                .style(
                    Style::default()
                        .fg(app.theme.accent_primary)
                        .add_modifier(Modifier::BOLD),
                )
                .block(Block::default().borders(Borders::ALL).title("Process"));
            f.render_widget(title, inner[0]);

            let simple_style = if mode == RoomEqWizardMode::Simple {
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };
            let full_style = if mode == RoomEqWizardMode::Full {
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };
            let marker = |selected: bool| if selected { "▸ " } else { "  " };

            let simple = Paragraph::new(format!(
                "{}Simple Wizard\n  Guided presets for common setups.\n  Pick target, loss, and processing mode.",
                marker(mode == RoomEqWizardMode::Simple)
            ))
            .style(simple_style)
            .block(Block::default().borders(Borders::ALL));
            f.render_widget(simple, inner[1]);

            let full = Paragraph::new(format!(
                "{}Full Wizard\n  All parameters in Acoustic + Optimizer blocks.\n  Full control over every setting.",
                marker(mode == RoomEqWizardMode::Full)
            ))
            .style(full_style)
            .block(Block::default().borders(Borders::ALL));
            f.render_widget(full, inner[2]);

            f.render_widget(
                Paragraph::new(" 1=Simple  2=Full  Tab=next step")
                    .style(Style::default().fg(app.theme.fg_secondary)),
                inner[3],
            );
        }

        RoomEqStep::Configure => {
            let c = &s.model.optimizer_config;
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
                (Some(11), "BO Initial", format!("{}", c.bo_initial_samples)),
                (Some(12), "BO Batch", format!("{}", c.bo_batch_size)),
                (
                    Some(13),
                    "BO Std Stop",
                    format!("{:.3}", c.bo_posterior_std_threshold),
                ),
                (Some(14), "BO Acquisition", c.bo_acquisition.clone()),
                (Some(15), "BO qEHVI", bool_str(c.bo_ehvi).to_string()),
                (Some(16), "Refine", bool_str(c.refine).to_string()),
                (Some(17), "Local Algo", c.local_algo.clone()),
                (
                    Some(18),
                    "Psychoacoustic",
                    bool_str(c.psychoacoustic).to_string(),
                ),
                (
                    Some(19),
                    "Asymmetric Loss",
                    bool_str(c.asymmetric_loss).to_string(),
                ),
                (None, "── Mode ──", String::new()),
                (Some(20), "Mode", c.mode.as_str().to_string()),
                (
                    Some(21),
                    "Multi-Speaker",
                    c.multi_speaker_mode.as_str().to_string(),
                ),
                (None, "── Target Response ──", String::new()),
                (
                    Some(22),
                    "Target Response",
                    bool_str(c.target_response.enabled).to_string(),
                ),
                (
                    Some(23),
                    "Slope (dB/oct)",
                    format!("{:.1}", c.target_response.slope_db_per_octave),
                ),
                (None, "── Excursion ──", String::new()),
                (
                    Some(24),
                    "Excursion Prot.",
                    bool_str(c.excursion_protection.enabled).to_string(),
                ),
                (
                    Some(25),
                    "Manual F3 (Hz)",
                    format!("{:.0}", c.excursion_protection.manual_f3_hz),
                ),
                (None, "── Schroeder Split ──", String::new()),
                (
                    Some(26),
                    "Schroeder Split",
                    bool_str(c.schroeder_split.enabled).to_string(),
                ),
                (
                    Some(27),
                    "Schroeder Freq",
                    format!("{:.0}", c.schroeder_split.schroeder_freq),
                ),
                (None, "── Phase ──", String::new()),
                (
                    Some(28),
                    "Phase Alignment",
                    bool_str(c.phase_alignment.enabled).to_string(),
                ),
            ];

            let channels_info = if s.model.channel_measurements.is_empty() {
                "No data".to_string()
            } else {
                let names: Vec<&str> = s
                    .model
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
                let is_selected = idx.is_some_and(|i| i == s.selected_field);
                let is_editing = is_selected && s.editing_value;
                let display_value = if is_editing {
                    format!("{}▏", s.edit_buffer)
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

            // Add slope recommendation line
            if let Some((slope, rec_min, rec_max)) = s.model.compute_lr_slope() {
                lines.push(Line::from(Span::styled(
                    format!(
                        "  Slope: {:.2} dB/oct  |  Rec: [{:.2}, {:.2}] dB/oct",
                        slope, rec_min, rec_max
                    ),
                    Style::default().fg(app.theme.fg_secondary),
                )));
            }

            lines.push(Line::from(""));
            let hint = if s.editing_value {
                " Type value, Enter=confirm  Esc=cancel"
            } else {
                " Up/Down=navigate  Left/Right=adjust  Enter=edit value  Tab=next field"
            };
            lines.push(Line::from(Span::styled(
                hint,
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
                    Constraint::Length(3),      // status
                    Constraint::Length(3),      // progress bar
                    Constraint::Percentage(40), // loss chart or hint
                    Constraint::Percentage(40), // logs box
                    Constraint::Length(1),      // hint line
                ])
                .split(content);

            let (status_text, status_style) = match &s.model.optimization_status {
                OptimizationStatus::Idle => (
                    "Ready to optimize. Press Enter to start.".to_string(),
                    Style::default().fg(app.theme.fg_secondary),
                ),
                OptimizationStatus::Running => {
                    // Post-processing phases set iteration=0 and max_iterations=0
                    // with a descriptive status message. Show that instead of
                    // the frozen "iter 0/0 | loss: 0.0000".
                    if s.model.current_iteration == 0 && s.opt_max_iter == 0 {
                        if !s.model.status_message.is_empty() {
                            (
                                s.model.status_message.clone(),
                                Style::default().fg(app.theme.accent_primary),
                            )
                        } else if s.model.current_channel.as_ref().is_some_and(|n| !n.is_empty()) {
                            (
                                format!("{}...", s.model.current_channel.as_deref().unwrap_or("")),
                                Style::default().fg(app.theme.accent_primary),
                            )
                        } else {
                            (
                                "Starting optimization...".to_string(),
                                Style::default().fg(app.theme.accent_primary),
                            )
                        }
                    } else {
                        let speaker_info = if s.model.current_channel.as_ref().is_some_and(|n| !n.is_empty()) {
                            if s.opt_total_speakers() > 1 {
                                format!(
                                    " | {}/{} {}",
                                    s.opt_total_speakers().min(s.model.channel_results.len() + 1),
                                    s.opt_total_speakers(),
                                    s.model.current_channel.as_deref().unwrap_or("")
                                )
                            } else {
                                format!(" | {}", s.model.current_channel.as_deref().unwrap_or(""))
                            }
                        } else {
                            String::new()
                        };
                        (
                            format!(
                                "Optimizing... iter {}/{} | loss: {:.4}{}",
                                s.model.current_iteration, s.opt_max_iter, s.model.current_loss, speaker_info
                            ),
                            Style::default().fg(app.theme.accent_primary),
                        )
                    }
                }
                OptimizationStatus::Completed => (
                    format!("Completed! {} channel results", s.model.channel_results.len()),
                    Style::default().fg(app.theme.accent_success),
                ),
                OptimizationStatus::Failed => (
                    format!(
                        "Failed: {}",
                        s.model.error_message.as_deref().unwrap_or("unknown error")
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
            let pct = (s.model.overall_progress * 100.0) as u16;
            let gauge = Gauge::default()
                .block(Block::default().borders(Borders::ALL).title("Progress"))
                .gauge_style(Style::default().fg(app.theme.accent_primary))
                .percent(pct.min(100));
            f.render_widget(gauge, inner[1]);

            // Loss chart or hint
            if s.loss_history.len() >= 2 {
                let history: Vec<_> = s.loss_history.iter().map(|(i, l)| (*i, *l, None)).collect();
                draw_loss_chart(f, inner[2], app, &history);
            } else {
                let chart_hint = match &s.model.optimization_status {
                    OptimizationStatus::Idle => "Waiting for optimization...",
                    OptimizationStatus::Running => "Waiting for loss data...",
                    _ => "No loss data recorded",
                };
                let hint_para = Paragraph::new(chart_hint)
                    .style(Style::default().fg(app.theme.fg_secondary))
                    .block(Block::default().borders(Borders::ALL).title("Loss History"));
                f.render_widget(hint_para, inner[2]);
            }

            // Logs box
            let log_count = s.opt_log_lines.len();
            let log_title = format!("Logs ({} lines)", log_count);
            if log_count > 0 {
                // inner[3] height minus 2 for borders
                let visible_height = inner[3].height.saturating_sub(2) as usize;
                let scroll_offset = s
                    .opt_log_scroll
                    .min(log_count.saturating_sub(visible_height));
                // Calculate the start index from the bottom
                let end = log_count.saturating_sub(scroll_offset);
                let start = end.saturating_sub(visible_height);
                let visible_lines: Vec<Line> = s
                    .opt_log_lines
                    .iter()
                    .skip(start)
                    .take(end - start)
                    .map(|line| {
                        Line::from(Span::styled(
                            line.as_str(),
                            Style::default().fg(app.theme.fg_secondary),
                        ))
                    })
                    .collect();
                let log_para = Paragraph::new(visible_lines)
                    .block(Block::default().borders(Borders::ALL).title(log_title));
                f.render_widget(log_para, inner[3]);
            } else {
                let log_para = Paragraph::new("No log messages yet")
                    .style(Style::default().fg(app.theme.fg_secondary))
                    .block(Block::default().borders(Borders::ALL).title(log_title));
                f.render_widget(log_para, inner[3]);
            }

            // Hint line
            let hint = if s.opt_log_lines.is_empty() {
                match &s.model.optimization_status {
                    OptimizationStatus::Idle => " Enter=start  BackTab=configure",
                    OptimizationStatus::Running => " Optimization running...",
                    OptimizationStatus::Completed => {
                        " Enter=re-run  Tab=view results  BackTab=configure"
                    }
                    OptimizationStatus::Failed | OptimizationStatus::Cancelled => {
                        " Enter=retry  BackTab=configure"
                    }
                }
            } else {
                " j/k=scroll logs  Enter=start/re-run  BackTab=configure"
            };
            let hint_para = Paragraph::new(hint).style(Style::default().fg(app.theme.fg_secondary));
            f.render_widget(hint_para, inner[4]);
        }

        RoomEqStep::Review => {
            if s.model.channel_results.is_empty() {
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
                .model
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
            if let Some(ch) = s.model.channel_results.get(s.selected_channel) {
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
            // Apply badge tells the user up-front whether the result fits
            // the linear rack or needs graph routing — same heuristic the
            // GPUI app uses to swap the "Apply to Rack" / "Apply as Graph"
            // buttons.
            let apply_hint = match s.model.dsp_output.as_ref() {
                Some(out) => {
                    use sotf_audio_player::room_eq_types::DspChainOutputExt;
                    if out.is_rack_compatible() {
                        " a=Apply to Rack (linear EQ)"
                    } else {
                        " a=Apply as Graph (multi-driver / routed)"
                    }
                }
                None => " a=Apply (run optimizer first)",
            };

            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // export path
                    Constraint::Length(3), // export status
                    Constraint::Length(3), // apply status
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

            // Apply-to-chain status row
            if let Some(ref err) = s.apply_error {
                let err_para = Paragraph::new(err.as_str())
                    .style(Style::default().fg(app.theme.accent_error))
                    .block(Block::default().borders(Borders::ALL).title("Apply Error"));
                f.render_widget(err_para, inner[2]);
            } else if let Some(ref msg) = s.apply_status {
                let ok = Paragraph::new(format!(" {}", msg))
                    .style(Style::default().fg(app.theme.accent_success))
                    .block(Block::default().borders(Borders::ALL).title("Apply Status"));
                f.render_widget(ok, inner[2]);
            } else {
                let hint = Paragraph::new(apply_hint)
                    .style(Style::default().fg(app.theme.fg_secondary))
                    .block(Block::default().borders(Borders::ALL).title("Apply"));
                f.render_widget(hint, inner[2]);
            }

            let help = Paragraph::new(
                " Enter=edit/export  a=Apply to chain  Tab=back to load  BackTab=review",
            )
            .style(Style::default().fg(app.theme.fg_secondary));
            f.render_widget(help, inner[3]);

            // Autocomplete overlay below the export path input
            if s.editing_export_path {
                let ac_h = autocomplete_dropdown_height(app);
                if ac_h > 0 {
                    let ac_area = Rect {
                        x: inner[0].x,
                        y: inner[0].y + inner[0].height,
                        width: inner[0].width,
                        height: ac_h.min(area.height.saturating_sub(inner[0].y + inner[0].height)),
                    };
                    f.render_widget(Clear, ac_area);
                    render_autocomplete_dropdown(f, ac_area, app);
                }
            }
        }
    }
}

/// Render the "Delay" wizard step — simplified to just a read-only table
/// of per-channel alignment delays. The probe-running form has moved to
/// the Recording wizard's Probe step.
fn draw_delay_detection_step(f: &mut Frame, content: Rect, app: &App) {
    use sotf_audio_player::room_eq_types::DelayDetectionStatus;

    let s = &app.room_eq;
    let dd = &s.model.delay_detection;
    let has_results = dd.results.is_some() && matches!(dd.status, DelayDetectionStatus::Complete);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title
            Constraint::Min(5),    // results table or no-data message
            Constraint::Length(1), // help / hint
        ])
        .split(content);

    // --- Title ---
    let title = Paragraph::new(" Per-Channel Alignment Delays")
        .style(
            Style::default()
                .fg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL).title("Delay"));
    f.render_widget(title, inner[0]);

    // --- Results table or "no data" message ---
    if has_results {
        let results = dd.results.as_ref().unwrap();
        let live_alignment = dd.edited_alignment_delays_ms();
        let mut has_low_delay = false;

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
                let arrival = dd
                    .edited_arrival_ms
                    .get(i)
                    .copied()
                    .unwrap_or(ch.arrival_ms);
                let alignment = live_alignment
                    .get(i)
                    .copied()
                    .or_else(|| results.alignment_delays_ms.get(i).copied())
                    .unwrap_or(0.0);
                if alignment > 0.0 && alignment < 0.3 {
                    has_low_delay = true;
                }
                // Mark low-delay rows with ⚠
                let align_text = if alignment > 0.0 && alignment < 0.3 {
                    format!("{:.2} ⚠", alignment)
                } else {
                    format!("{:.2}", alignment)
                };
                Row::new(vec![
                    Cell::from(ch.channel_name.clone()),
                    Cell::from(format!("{:.2}", arrival)),
                    Cell::from(format!("{:+.1}", ch.gain_db)),
                    Cell::from(format!("{:+.1}", ch.snr_db)).style(Style::default().fg(snr_color)),
                    Cell::from(align_text),
                ])
            })
            .collect();

        let header = Row::new(vec![
            Cell::from("Channel"),
            Cell::from("Arrival ms"),
            Cell::from("Gain dB"),
            Cell::from("SNR dB"),
            Cell::from("Delay ms"),
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
                Constraint::Length(14),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Alignment Delays"),
        );
        f.render_widget(table, inner[1]);

        // Hint for low delays
        let help = if has_low_delay {
            " ⚠ Delays < 0.3 ms — consider using 0. Delays auto-feed into optimizer."
        } else {
            " Delays auto-feed into optimizer. j/k=row  e=edit  Tab=next step"
        };
        f.render_widget(
            Paragraph::new(help).style(Style::default().fg(app.theme.fg_secondary)),
            inner[2],
        );
    } else {
        let msg = Paragraph::new(
            " No delay data. Run the Probe step in the Recording wizard,\n \
             or load a file with probe results.",
        )
        .style(Style::default().fg(app.theme.fg_secondary))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Alignment Delays"),
        );
        f.render_widget(msg, inner[1]);

        f.render_widget(
            Paragraph::new(" Tab=next step").style(Style::default().fg(app.theme.fg_secondary)),
            inner[2],
        );
    }
}
