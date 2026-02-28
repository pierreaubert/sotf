use super::*;

pub(crate) fn draw_headphone_eq_screen(f: &mut Frame, area: Rect, app: &App) {
    use crate::app::HeadphoneEqStep;
    use ratatui::widgets::{Gauge, Tabs};
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    let s = &app.headphone_eq;

    // Layout: step tabs on top, content below
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(area);

    // Step tabs
    let steps = [
        HeadphoneEqStep::SelectFile,
        HeadphoneEqStep::Configure,
        HeadphoneEqStep::Optimize,
        HeadphoneEqStep::Results,
        HeadphoneEqStep::UpdatePlugin,
    ];
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
        .block(Block::default().borders(Borders::ALL).title("Headphone EQ"))
        .select(s.step as usize)
        .highlight_style(Style::default().fg(app.theme.accent_primary));
    f.render_widget(tabs, outer[0]);

    let content = outer[1];

    match s.step {
        HeadphoneEqStep::SelectFile => {
            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // measurement path
                    Constraint::Length(3), // target preset
                    Constraint::Length(3), // custom target path
                    Constraint::Min(1),    // help
                ])
                .split(content);

            let meas_style = if s.selected_field == 0 {
                Style::default().fg(app.theme.accent_primary)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };
            let meas_label = if s.editing_measurement {
                "Measurement CSV (editing)"
            } else {
                "Measurement CSV"
            };
            let meas = Paragraph::new(if s.measurement_path.is_empty() {
                "<type path or paste>".to_string()
            } else {
                s.measurement_path.clone()
            })
            .style(meas_style)
            .block(Block::default().borders(Borders::ALL).title(meas_label));
            f.render_widget(meas, inner[0]);

            let target_style = if s.selected_field == 1 {
                Style::default().fg(app.theme.accent_primary)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };
            let target = Paragraph::new(s.target_preset.clone())
                .style(target_style)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Target Preset (Left/Right to cycle)"),
                );
            f.render_widget(target, inner[1]);

            if s.target_preset == "custom" {
                let custom_style = if s.selected_field == 2 {
                    Style::default().fg(app.theme.accent_primary)
                } else {
                    Style::default().fg(app.theme.fg_primary)
                };
                let custom_label = if s.editing_custom_target {
                    "Custom Target (editing)"
                } else {
                    "Custom Target CSV"
                };
                let custom = Paragraph::new(if s.custom_target_path.is_empty() {
                    "<type path>".to_string()
                } else {
                    s.custom_target_path.clone()
                })
                .style(custom_style)
                .block(Block::default().borders(Borders::ALL).title(custom_label));
                f.render_widget(custom, inner[2]);
            }

            let help = Paragraph::new(
                " Up/Down=select field  Enter=edit text  Left/Right=cycle preset  Tab=next step",
            )
            .style(Style::default().fg(app.theme.fg_secondary));
            f.render_widget(help, inner[3]);
        }

        HeadphoneEqStep::Configure => {
            let c = &s.config;
            let bool_str = |b: bool| if b { "[ON]" } else { "[OFF]" };

            let rows: Vec<(Option<usize>, &str, String)> = vec![
                (None, "── Filters ──", String::new()),
                (Some(0), "Filters (n)", format!("{}", c.num_filters)),
                (Some(1), "Min Freq (Hz)", format!("{:.0}", c.min_freq)),
                (Some(2), "Max Freq (Hz)", format!("{:.0}", c.max_freq)),
                (Some(3), "Min dB", format!("{:.1}", c.min_db)),
                (Some(4), "Max dB", format!("{:.1}", c.max_db)),
                (Some(5), "Min Q", format!("{:.2}", c.min_q)),
                (Some(6), "Max Q", format!("{:.2}", c.max_q)),
                (Some(7), "PEQ Model", c.peq_model.clone()),
                (None, "── Optimization ──", String::new()),
                (Some(8), "Algorithm", c.algorithm.as_str().to_string()),
                (Some(9), "Max Iter", format!("{}", c.max_iter)),
                (Some(10), "Population", format!("{}", c.population)),
                (Some(11), "Strategy", c.strategy.clone()),
                (Some(12), "DE F (mutation)", format!("{:.2}", c.de_f)),
                (Some(13), "DE CR (crossover)", format!("{:.2}", c.de_cr)),
                (None, "── Refinement ──", String::new()),
                (Some(14), "Refine", bool_str(c.refine).to_string()),
                (Some(15), "Local Algo", c.local_algo.clone()),
                (None, "── Smoothing ──", String::new()),
                (Some(16), "Smooth", bool_str(c.smooth).to_string()),
                (Some(17), "Smooth N", format!("{}", c.smooth_n)),
            ];

            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Speaker: ", Style::default().fg(app.theme.fg_secondary)),
                    Span::styled(
                        &s.measurement_path,
                        Style::default().fg(app.theme.accent_primary),
                    ),
                ]),
                Line::from(""),
            ];

            for (idx, label, value) in &rows {
                let is_selected = idx.map_or(false, |i| i == s.config_selected_field);
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
                " Up/Down=navigate  -/+=adjust  Left/Right=step  Enter/Tab=optimize",
                Style::default().fg(app.theme.fg_secondary),
            )));

            let para = Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL).title("Configure"))
                .wrap(Wrap { trim: false });
            f.render_widget(para, content);
        }

        HeadphoneEqStep::Optimize => {
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
                    format!(
                        "Completed! Final loss: {:.4} | {} filters",
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

            // Loss chart or hint
            if s.loss_history.len() >= 2 {
                let history: Vec<_> = s.loss_history.iter().map(|(i, l)| (*i, *l, None)).collect();
                draw_loss_chart(f, inner[2], app, &history);
            } else {
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
                    .block(Block::default().borders(Borders::ALL).title("Loss"));
                f.render_widget(hint_para, inner[2]);
            }
        }

        HeadphoneEqStep::Results => {
            if s.filters.is_empty() {
                let placeholder =
                    Paragraph::new("No optimization results yet. Go to Optimize step first.")
                        .style(Style::default().fg(app.theme.fg_secondary))
                        .alignment(Alignment::Center)
                        .block(Block::default().borders(Borders::ALL).title("Results"));
                f.render_widget(placeholder, content);
                return;
            }

            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // summary
                    Constraint::Min(5),    // filter table
                ])
                .split(content);

            let summary = Paragraph::new(vec![Line::from(vec![
                Span::styled(
                    format!(" {} filters", s.filters.len()),
                    Style::default().fg(app.theme.accent_primary),
                ),
                Span::raw("  |  "),
                Span::styled(
                    format!("Pre: {:.2}", s.pre_loss),
                    Style::default().fg(app.theme.fg_secondary),
                ),
                Span::raw(" → "),
                Span::styled(
                    format!("Post: {:.2}", s.post_loss),
                    Style::default().fg(app.theme.accent_success),
                ),
            ])])
            .block(Block::default().borders(Borders::ALL).title("Summary"));
            f.render_widget(summary, inner[0]);

            // Filter table
            let header = Row::new(vec![
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

            let rows: Vec<Row> = s
                .filters
                .iter()
                .enumerate()
                .map(|(i, filt)| {
                    Row::new(vec![
                        Cell::from(format!("{}", i + 1)),
                        Cell::from(filt.filter_type.clone()),
                        Cell::from(format!("{:.1}", filt.freq)),
                        Cell::from(format!("{:.2}", filt.q)),
                        Cell::from(format!("{:+.1}", filt.db_gain)),
                    ])
                })
                .collect();

            let table = Table::new(
                rows,
                [
                    Constraint::Length(3),
                    Constraint::Length(12),
                    Constraint::Length(12),
                    Constraint::Length(8),
                    Constraint::Length(10),
                ],
            )
            .header(header)
            .block(Block::default().borders(Borders::ALL).title("Filters"));
            f.render_widget(table, inner[1]);
        }

        HeadphoneEqStep::UpdatePlugin => {
            use crate::app::SpinUpdateSubStep;
            let has_results = !s.filters.is_empty();
            let measurement = if s.measurement_path.is_empty() {
                "(none)"
            } else {
                &s.measurement_path
            };

            let mut lines = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("Measurement: ", Style::default().fg(app.theme.fg_secondary)),
                    Span::styled(
                        measurement,
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
                        " Enter=apply to rack  ←/BackTab=Results",
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
            f.render_widget(para, content);
        }
    }
}

