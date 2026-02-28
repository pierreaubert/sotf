use super::*;

pub(crate) fn draw_spinorama_eq_screen(f: &mut Frame, area: Rect, app: &App) {
    use crate::app::SpinoramaStep;
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    let s = &app.spinorama_eq;

    // Layout: step header (3) + content (rest)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Step header tabs
    let steps = [
        SpinoramaStep::Select,
        SpinoramaStep::Configure,
        SpinoramaStep::Optimize,
        SpinoramaStep::Results,
        SpinoramaStep::UpdatePlugin,
    ];
    let mut spans = vec![Span::raw(" ")];
    for step in &steps {
        let is_active = *step == s.step;
        let style = if is_active {
            Style::default()
                .fg(app.theme.bg_primary)
                .bg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(app.theme.fg_secondary)
                .bg(app.theme.bg_secondary)
        };
        spans.push(Span::styled(format!(" {} ", step.label()), style));
        spans.push(Span::raw(" "));
    }
    let header = Paragraph::new(Line::from(spans))
        .block(Block::default().borders(Borders::ALL).title("Spinorama EQ"));
    f.render_widget(header, chunks[0]);

    // Content per step
    match s.step {
        SpinoramaStep::Select => {
            // Split: search box (3) + list (rest) + hint (3)
            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(3),
                ])
                .split(chunks[1]);

            // Search box
            let search_title = if s.loading_speakers {
                "Search Speaker (loading...)"
            } else if let Some(ref e) = s.speakers_error {
                &format!("Error: {}", e)
            } else {
                "Search Speaker (type to filter, Enter to select)"
            };
            let search = Paragraph::new(s.search_query.as_str())
                .style(Style::default().fg(app.theme.fg_primary))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(search_title)
                        .border_style(Style::default().fg(app.theme.accent_primary)),
                );
            f.render_widget(search, inner[0]);

            // Speaker list
            let items: Vec<ListItem> = s
                .filtered_speakers
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let style = if i == s.selected_speaker_idx {
                        Style::default()
                            .fg(app.theme.fg_selected)
                            .bg(app.theme.bg_selected)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(app.theme.fg_primary)
                    };
                    let prefix = if i == s.selected_speaker_idx {
                        "► "
                    } else {
                        "  "
                    };
                    ListItem::new(format!("{}{}", prefix, name)).style(style)
                })
                .collect();

            let list_title = if s.filtered_speakers.is_empty() && !s.loading_speakers {
                "Speakers (press 'r' to load from spinorama.org)".to_string()
            } else {
                format!(
                    "Speakers ({}/{})",
                    s.filtered_speakers.len(),
                    s.available_speakers.len()
                )
            };
            let list =
                List::new(items).block(Block::default().borders(Borders::ALL).title(list_title));
            f.render_widget(list, inner[1]);

            // Hint bar
            let hint = if let Some(ref sel) = s.selected_speaker {
                format!(" Selected: {}  |  ←/→=step  Enter=confirm", sel)
            } else {
                " ←/→=step  ↑/↓=navigate  Enter=select  r=load speakers".to_string()
            };
            let hint_widget = Paragraph::new(hint)
                .style(Style::default().fg(app.theme.fg_secondary))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(hint_widget, inner[2]);
        }

        SpinoramaStep::Configure => {
            // Split: scrollable config (rest) + hint (3)
            let cfg_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(3)])
                .split(chunks[1]);

            let speaker_name = s
                .selected_speaker
                .as_deref()
                .unwrap_or("(no speaker selected)");

            let bool_str = |b: bool| if b { "[ON]" } else { "[OFF]" };

            // Each entry is either a Section header (None index) or a field (Some index)
            // Fields are numbered 0..24 for selected_field navigation
            let c = &s.config;
            let rows: Vec<(Option<usize>, &str, String)> = vec![
                (None, "── Loss ──", String::new()),
                (Some(0), "Loss Function", c.loss_function.clone()),
                (None, "── Filters ──", String::new()),
                (Some(1), "Filters (n)", format!("{}", c.num_filters)),
                (Some(2), "Min Freq (Hz)", format!("{:.0}", c.min_freq)),
                (Some(3), "Max Freq (Hz)", format!("{:.0}", c.max_freq)),
                (Some(4), "Min dB", format!("{:.1}", c.min_db)),
                (Some(5), "Max dB", format!("{:.1}", c.max_db)),
                (Some(6), "Min Q", format!("{:.2}", c.min_q)),
                (Some(7), "Max Q", format!("{:.2}", c.max_q)),
                (Some(8), "PEQ Model", c.peq_model.clone()),
                (None, "── Optimization ──", String::new()),
                (Some(9), "Algorithm", c.algorithm.as_str().to_string()),
                (Some(10), "Max Iter", format!("{}", c.max_iter)),
                (Some(11), "Population", format!("{}", c.population)),
                (Some(12), "Strategy", c.strategy.clone()),
                (Some(13), "DE F (mutation)", format!("{:.2}", c.de_f)),
                (Some(14), "DE CR (crossover)", format!("{:.2}", c.de_cr)),
                (None, "── Refinement ──", String::new()),
                (Some(15), "Refine", bool_str(c.refine).to_string()),
                (Some(16), "Local Algo", c.local_algo.clone()),
                (None, "── Smoothing ──", String::new()),
                (Some(17), "Smooth", bool_str(c.smooth).to_string()),
                (Some(18), "Smooth N", format!("{}", c.smooth_n)),
                (
                    Some(19),
                    "Psychoacoustic",
                    bool_str(c.psychoacoustic).to_string(),
                ),
                (None, "── Constraints ──", String::new()),
                (
                    Some(20),
                    "Spacing Weight",
                    format!("{:.1}", c.spacing_weight),
                ),
                (
                    Some(21),
                    "Min Spacing (oct)",
                    format!("{:.2}", c.min_spacing_oct),
                ),
                (None, "── Convergence ──", String::new()),
                (Some(22), "Tolerance", format!("{:.0e}", c.tolerance)),
                (Some(23), "Abs Tolerance", format!("{:.0e}", c.atolerance)),
                (Some(24), "Sample Rate", format!("{}", c.sample_rate)),
            ];

            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Speaker: ", Style::default().fg(app.theme.fg_secondary)),
                    Span::styled(
                        speaker_name,
                        Style::default()
                            .fg(app.theme.accent_primary)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(""),
            ];

            let section_style = Style::default()
                .fg(app.theme.fg_secondary)
                .add_modifier(Modifier::DIM);

            for (idx, label, value) in &rows {
                match idx {
                    None => {
                        lines.push(Line::from(Span::styled(
                            format!("  {}", label),
                            section_style,
                        )));
                    }
                    Some(i) => {
                        let is_selected = *i == s.selected_field;
                        let label_style = if is_selected {
                            Style::default()
                                .fg(app.theme.accent_primary)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(app.theme.fg_secondary)
                        };
                        let value_style = if is_selected {
                            Style::default()
                                .fg(app.theme.fg_selected)
                                .bg(app.theme.bg_selected)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(app.theme.fg_primary)
                        };
                        let prefix = if is_selected { "► " } else { "  " };
                        lines.push(Line::from(vec![
                            Span::raw(prefix),
                            Span::styled(format!("{:<20}", label), label_style),
                            Span::styled(format!(" {}", value), value_style),
                        ]));
                    }
                }
            }

            let para = Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Configuration"),
                )
                .scroll((s.selected_field.saturating_sub(10) as u16, 0));
            f.render_widget(para, cfg_layout[0]);

            let hint_widget = Paragraph::new(" ←/→=step  ↑/↓=select field  -/+=adjust  Enter=optimize")
                .style(Style::default().fg(app.theme.fg_secondary))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(hint_widget, cfg_layout[1]);
        }

        SpinoramaStep::Optimize => {
            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(4),
                    Constraint::Length(3),
                    Constraint::Min(0),
                ])
                .split(chunks[1]);

            // Status
            let (status_text, status_style) = match &s.opt_status {
                OptimizationStatus::Idle => (
                    "Press Enter to start optimization".to_string(),
                    Style::default().fg(app.theme.fg_secondary),
                ),
                OptimizationStatus::Running => (
                    format!(
                        "Running... iter {}/{} | loss: {:.6}",
                        s.opt_iteration, s.opt_max_iter, s.opt_loss
                    ),
                    Style::default().fg(app.theme.accent_primary),
                ),
                OptimizationStatus::Completed => (
                    format!(
                        "Completed! Final loss: {:.6}  |  {} filters found",
                        s.post_loss,
                        s.filters.len()
                    ),
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

            let status_para = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(status_text, status_style)),
            ])
            .block(Block::default().borders(Borders::ALL).title("Status"));
            f.render_widget(status_para, inner[0]);

            // Progress bar
            let progress_pct = (s.opt_progress * 100.0) as u16;
            let bar_width = inner[1].width.saturating_sub(4) as usize;
            let filled = (bar_width * progress_pct as usize / 100).min(bar_width);
            let bar = format!(
                "[{}{}] {}%",
                "█".repeat(filled),
                "░".repeat(bar_width.saturating_sub(filled)),
                progress_pct
            );
            let progress_para = Paragraph::new(bar)
                .style(Style::default().fg(app.theme.accent_primary))
                .block(Block::default().borders(Borders::ALL).title("Progress"));
            f.render_widget(progress_para, inner[1]);

            // Loss history chart (if data available), else hint
            if s.loss_history.len() >= 2 {
                draw_loss_chart(f, inner[2], app, &s.loss_history);
            } else {
                let hint = match &s.opt_status {
                    OptimizationStatus::Idle => " Enter=start  Tab=back to configure",
                    OptimizationStatus::Running => " Optimization running...",
                    OptimizationStatus::Completed => " Enter=re-run  Tab=view results",
                    OptimizationStatus::Failed | OptimizationStatus::Cancelled => {
                        " Enter=retry  Tab=back to configure"
                    }
                };
                let hint_para = Paragraph::new(hint)
                    .style(Style::default().fg(app.theme.fg_secondary))
                    .block(Block::default().borders(Borders::ALL).title("Loss History"));
                f.render_widget(hint_para, inner[2]);
            }
        }

        SpinoramaStep::Results => {
            if s.filters.is_empty() {
                let msg =
                    Paragraph::new("No results yet. Go to Optimize step and run optimization.")
                        .style(Style::default().fg(app.theme.fg_secondary))
                        .alignment(Alignment::Center)
                        .block(Block::default().borders(Borders::ALL).title("Results"));
                f.render_widget(msg, chunks[1]);
            } else {
                // Vertical split: summary + chart on top, filter table on bottom
                let table_height = (s.filters.len() as u16 + 3).min(15); // rows + header + borders, capped
                let rows_layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),           // summary
                        Constraint::Min(8),              // freq response chart
                        Constraint::Length(table_height), // filter table
                    ])
                    .split(chunks[1]);

                let initial_score = s.loss_history.iter().find_map(|(_, _, score)| *score);
                let final_score = s.loss_history.iter().rev().find_map(|(_, _, score)| *score);
                let score_part = match (initial_score, final_score) {
                    (Some(init), Some(fin)) => format!(
                        "  |  Score: {:.2} → {:.2} (Δ {:+.2})",
                        init, fin, fin - init,
                    ),
                    _ => String::new(),
                };
                let summary = format!(
                    " {} filters  |  Loss: {:.4} → {:.4} (Δ {:.4}){}",
                    s.filters.len(),
                    s.pre_loss,
                    s.post_loss,
                    s.pre_loss - s.post_loss,
                    score_part,
                );
                let summary_para = Paragraph::new(summary)
                    .style(Style::default().fg(app.theme.accent_success))
                    .block(Block::default().borders(Borders::ALL).title("Summary"));
                f.render_widget(summary_para, rows_layout[0]);

                draw_freq_response_chart(f, rows_layout[1], app, s);

                // Bottom: filter table
                let header_cells = ["#", "Type", "Freq", "Q", "dB"].iter().map(|h| {
                    Cell::from(*h).style(
                        Style::default()
                            .fg(app.theme.accent_primary)
                            .add_modifier(Modifier::BOLD),
                    )
                });
                let header_row = Row::new(header_cells).height(1).bottom_margin(0);

                let rows: Vec<Row> = s
                    .filters
                    .iter()
                    .enumerate()
                    .map(|(i, filt)| {
                        let cells = vec![
                            Cell::from(format!("{}", i + 1)),
                            Cell::from(filt.filter_type.clone()),
                            Cell::from(format!("{:.0}", filt.freq)),
                            Cell::from(format!("{:.2}", filt.q)),
                            Cell::from(format!("{:+.1}", filt.db_gain)),
                        ];
                        Row::new(cells)
                    })
                    .collect();

                let table = Table::new(
                    rows,
                    [
                        Constraint::Length(3),
                        Constraint::Length(9),
                        Constraint::Length(6),
                        Constraint::Length(6),
                        Constraint::Length(6),
                    ],
                )
                .header(header_row)
                .block(Block::default().borders(Borders::ALL).title("PEQ Filters"));
                f.render_widget(table, rows_layout[2]);
            }
        }

        SpinoramaStep::UpdatePlugin => {
            use crate::app::SpinUpdateSubStep;
            let has_results = !s.filters.is_empty();
            let speaker = s.selected_speaker.as_deref().unwrap_or("(none)");

            let mut lines = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("Speaker: ", Style::default().fg(app.theme.fg_secondary)),
                    Span::styled(
                        speaker,
                        Style::default()
                            .fg(app.theme.accent_primary)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(""),
            ];

            match s.update_substep {
                SpinUpdateSubStep::Ready => {
                    if has_results {
                        lines.push(Line::from(vec![Span::styled(
                            format!("  {} PEQ filters ready to apply", s.filters.len()),
                            Style::default().fg(app.theme.accent_success),
                        )]));
                        lines.push(Line::from(""));
                        lines.push(Line::from(vec![Span::styled(
                            "  Press Enter to apply filters to the EQ plugin in the rack.",
                            Style::default().fg(app.theme.fg_primary),
                        )]));
                        lines.push(Line::from(vec![Span::styled(
                            "  If no EQ plugin exists it will be added automatically.",
                            Style::default().fg(app.theme.fg_secondary),
                        )]));
                    } else {
                        lines.push(Line::from(vec![Span::styled(
                            "  No optimization results yet. Run optimization first.",
                            Style::default().fg(app.theme.accent_error),
                        )]));
                    }

                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![Span::styled(
                        " Enter=apply to rack  →=Select  ←/BackTab=Results",
                        Style::default().fg(app.theme.fg_secondary),
                    )]));
                }
                SpinUpdateSubStep::ConfirmOverwrite => {
                    if let Some((slot, count)) = s.update_existing_eq_info {
                        lines.push(Line::from(vec![Span::styled(
                            format!("  Existing EQ in slot {} has {} filter(s).", slot, count),
                            Style::default().fg(app.theme.accent_warning),
                        )]));
                        lines.push(Line::from(""));
                        lines.push(Line::from(vec![Span::styled(
                            "  Save current preset before overwriting?",
                            Style::default().fg(app.theme.fg_primary).add_modifier(Modifier::BOLD),
                        )]));
                        lines.push(Line::from(""));
                        lines.push(Line::from(vec![
                            Span::styled("  y", Style::default().fg(app.theme.accent_success).add_modifier(Modifier::BOLD)),
                            Span::styled(" = save preset then apply   ", Style::default().fg(app.theme.fg_secondary)),
                            Span::styled("n", Style::default().fg(app.theme.accent_error).add_modifier(Modifier::BOLD)),
                            Span::styled(" = apply without saving   ", Style::default().fg(app.theme.fg_secondary)),
                            Span::styled("Esc", Style::default().fg(app.theme.fg_secondary).add_modifier(Modifier::BOLD)),
                            Span::styled(" = cancel", Style::default().fg(app.theme.fg_secondary)),
                        ]));
                    }
                }
            }

            let para = Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Update Plugin"),
            );
            f.render_widget(para, chunks[1]);
        }
    }
}

