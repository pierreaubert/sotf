use super::super::*;

pub(crate) fn draw_spinorama_eq_screen(f: &mut Frame, area: Rect, app: &App) {
    let i18n = crate::i18n::TuiTranslations::for_language(app.ui.language);
    use crate::app::SpinoramaStep;
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    let s = &app.spinorama_eq;

    // Layout: step header (3) + content (rest)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
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
        spans.push(Span::styled(
            format!(" {} ", i18n.dynamic(step.label().to_string())),
            style,
        ));
        spans.push(Span::raw(" "));
    }
    let header = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(step_tab_border)
            .border_style(Style::default().fg(step_tab_border_color))
            .title(i18n.ui("Spinorama EQ")),
    );
    f.render_widget(header, chunks[0]);

    // Content wrapper with focus-aware border
    let content_wrapper = Block::default()
        .borders(Borders::ALL)
        .border_type(content_border)
        .border_style(Style::default().fg(content_border_color));
    f.render_widget(content_wrapper, chunks[1]);
    let content_area = Rect {
        x: chunks[1].x + 1,
        y: chunks[1].y + 1,
        width: chunks[1].width.saturating_sub(2),
        height: chunks[1].height.saturating_sub(2),
    };

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
                .split(content_area);

            // Search box
            let search_title = if s.model.loading_speakers {
                i18n.dynamic("Search Speaker (loading...)".to_string())
            } else if let Some(ref e) = s.speakers_error {
                i18n.dynamic(format!("Error: {}", e))
            } else {
                i18n.dynamic("Search Speaker (type to filter, Enter to select)".to_string())
            };
            let search = Paragraph::new(s.model.speaker_search.as_str())
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
                .model
                .speaker_suggestions
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

            let list_title = i18n.dynamic(
                if s.model.speaker_suggestions.is_empty() && !s.model.loading_speakers {
                    "Speakers (press 'r' to load from spinorama.org)".to_string()
                } else {
                    format!(
                        "Speakers ({}/{})",
                        s.model.speaker_suggestions.len(),
                        s.model.available_speakers.len()
                    )
                },
            );
            let list =
                List::new(items).block(Block::default().borders(Borders::ALL).title(list_title));
            f.render_widget(list, inner[1]);

            // Hint bar
            let hint = i18n.dynamic(if let Some(ref sel) = s.model.selected_speaker {
                format!(" Selected: {}  |  ←/→=step  Enter=confirm", sel)
            } else {
                " ←/→=step  ↑/↓=navigate  Enter=select  r=load speakers".to_string()
            });
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
                .split(content_area);

            let speaker_name = s
                .model
                .selected_speaker
                .as_deref()
                .map(str::to_owned)
                .unwrap_or_else(|| i18n.dynamic("(no speaker selected)".to_string()));

            let bool_str = |b: bool| if b { "[ON]" } else { "[OFF]" };

            // Each entry is either a Section header (None index) or a field (Some index)
            // Fields are numbered 0..24 for selected_field navigation
            let c = &s.model.optimizer_config;
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
                    Span::styled(
                        i18n.ui("Speaker: "),
                        Style::default().fg(app.theme.fg_secondary),
                    ),
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
                            format!("  {}", i18n.dynamic((*label).to_string())),
                            section_style,
                        )));
                    }
                    Some(i) => {
                        let is_selected = *i == s.selected_field;
                        let is_editing = is_selected && s.editing_value;
                        let display_value = if is_editing {
                            format!("{}▏", s.edit_buffer)
                        } else {
                            value.clone()
                        };
                        let label_style = if is_editing {
                            Style::default()
                                .fg(app.theme.fg_selected)
                                .add_modifier(Modifier::BOLD)
                        } else if is_selected {
                            Style::default()
                                .fg(app.theme.accent_primary)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(app.theme.fg_secondary)
                        };
                        let value_style = if is_editing || is_selected {
                            Style::default()
                                .fg(app.theme.fg_selected)
                                .bg(app.theme.bg_selected)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(app.theme.fg_primary)
                        };
                        let prefix = if is_editing {
                            "✎ "
                        } else if is_selected {
                            "► "
                        } else {
                            "  "
                        };
                        lines.push(Line::from(vec![
                            Span::raw(prefix),
                            Span::styled(
                                format!("{:<20}", i18n.dynamic((*label).to_string())),
                                label_style,
                            ),
                            Span::styled(format!(" {}", display_value), value_style),
                        ]));
                    }
                }
            }

            let para = Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(i18n.ui("Configuration")),
                )
                .scroll((s.selected_field.saturating_sub(10) as u16, 0));
            f.render_widget(para, cfg_layout[0]);

            let hint = i18n.dynamic(
                if s.editing_value {
                    " Type value, Enter=confirm  Esc=cancel"
                } else {
                    " ↑/↓=select field  Left/Right=adjust  Enter=edit value  Tab=next field"
                }
                .to_string(),
            );
            let hint_widget = Paragraph::new(hint)
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
                .split(content_area);

            // Status
            let unknown_error = i18n.dynamic("unknown error".to_string());
            let (status_text, status_style) = match &s.model.optimization_status {
                OptimizationStatus::Idle => (
                    "Press Enter to start optimization".to_string(),
                    Style::default().fg(app.theme.fg_secondary),
                ),
                OptimizationStatus::Running => (
                    format!(
                        "Running... iter {}/{} | loss: {:.6}",
                        s.model.current_iteration, s.opt_max_iter, s.model.current_loss
                    ),
                    Style::default().fg(app.theme.accent_primary),
                ),
                OptimizationStatus::Completed => (
                    format!(
                        "Completed! Final loss: {:.6}  |  {} filters found",
                        s.model.post_loss,
                        s.model.filters.len()
                    ),
                    Style::default().fg(app.theme.accent_success),
                ),
                OptimizationStatus::Failed => (
                    format!(
                        "Failed: {}",
                        s.model
                            .error_message
                            .as_deref()
                            .unwrap_or(unknown_error.as_str())
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
                Line::from(Span::styled(i18n.dynamic(status_text), status_style)),
            ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(i18n.ui("Status")),
            );
            f.render_widget(status_para, inner[0]);

            // Progress bar
            let progress_pct = (s.model.progress * 100.0) as u16;
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
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(i18n.ui("Progress")),
                );
            f.render_widget(progress_para, inner[1]);

            // Loss history chart (if data available), else hint
            if s.model.progress_history.len() >= 2 {
                draw_loss_chart(f, inner[2], app, &s.model.progress_history);
            } else {
                let hint = match &s.model.optimization_status {
                    OptimizationStatus::Idle => " Enter=start  Tab=back to configure",
                    OptimizationStatus::Running => " Optimization running...",
                    OptimizationStatus::Completed => " Enter=re-run  Tab=view results",
                    OptimizationStatus::Failed | OptimizationStatus::Cancelled => {
                        " Enter=retry  Tab=back to configure"
                    }
                };
                let hint_para = Paragraph::new(i18n.dynamic(hint.to_string()))
                    .style(Style::default().fg(app.theme.fg_secondary))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(i18n.ui("Loss History")),
                    );
                f.render_widget(hint_para, inner[2]);
            }
        }

        SpinoramaStep::Results => {
            if s.model.filters.is_empty() {
                let msg = Paragraph::new(
                    i18n.ui("No results yet. Go to Optimize step and run optimization."),
                )
                .style(Style::default().fg(app.theme.fg_secondary))
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(i18n.ui("Results")),
                );
                f.render_widget(msg, content_area);
            } else {
                // Vertical split: summary + chart on top, filter table on bottom
                let table_height = (s.model.filters.len() as u16 + 3).min(15); // rows + header + borders, capped
                let rows_layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),            // summary
                        Constraint::Min(8),               // freq response chart
                        Constraint::Length(table_height), // filter table
                    ])
                    .split(content_area);

                let initial_score = s
                    .model
                    .progress_history
                    .iter()
                    .find_map(|(_, _, score)| *score);
                let final_score = s
                    .model
                    .progress_history
                    .iter()
                    .rev()
                    .find_map(|(_, _, score)| *score);
                let score_part = match (initial_score, final_score) {
                    (Some(init), Some(fin)) => i18n.dynamic(format!(
                        "  |  Score: {:.2} → {:.2} (Δ {:+.2})",
                        init,
                        fin,
                        fin - init,
                    )),
                    _ => String::new(),
                };
                let summary = format!(
                    " {} filters  |  Loss: {:.4} → {:.4} (Δ {:.4}){}",
                    s.model.filters.len(),
                    s.model.pre_loss,
                    s.model.post_loss,
                    s.model.pre_loss - s.model.post_loss,
                    score_part,
                );
                let summary_para = Paragraph::new(i18n.dynamic(summary))
                    .style(Style::default().fg(app.theme.accent_success))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(i18n.ui("Summary")),
                    );
                f.render_widget(summary_para, rows_layout[0]);

                draw_freq_response_chart(
                    f,
                    rows_layout[1],
                    app,
                    &s.model.curve_frequencies,
                    &s.model.curve_input,
                    &s.model.curve_corrected,
                    &s.model.curve_filter_response,
                );

                // Bottom: filter table
                let header_cells = ["#", "Type", "Freq", "Q", "dB"].iter().map(|h| {
                    Cell::from(if *h == "#" {
                        (*h).to_string()
                    } else {
                        i18n.dynamic((*h).to_string())
                    })
                    .style(
                        Style::default()
                            .fg(app.theme.accent_primary)
                            .add_modifier(Modifier::BOLD),
                    )
                });
                let header_row = Row::new(header_cells).height(1).bottom_margin(0);

                let rows: Vec<Row> = s
                    .model
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
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(i18n.ui("PEQ Filters")),
                );
                f.render_widget(table, rows_layout[2]);
            }
        }

        SpinoramaStep::UpdatePlugin => {
            use crate::app::SpinUpdateSubStep;
            let has_results = !s.model.filters.is_empty();
            let speaker = s
                .model
                .selected_speaker
                .as_deref()
                .map(str::to_owned)
                .unwrap_or_else(|| i18n.dynamic("(none)".to_string()));

            let mut lines = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        i18n.ui("Speaker: "),
                        Style::default().fg(app.theme.fg_secondary),
                    ),
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
                            i18n.dynamic(format!(
                                "  {} PEQ filters ready to apply",
                                s.model.filters.len()
                            )),
                            Style::default().fg(app.theme.accent_success),
                        )]));
                        lines.push(Line::from(""));
                        lines.push(Line::from(vec![Span::styled(
                            i18n.dynamic(
                                "  Press Enter to apply filters to the EQ plugin in the rack."
                                    .to_string(),
                            ),
                            Style::default().fg(app.theme.fg_primary),
                        )]));
                        lines.push(Line::from(vec![Span::styled(
                            i18n.dynamic(
                                "  If no EQ plugin exists it will be added automatically."
                                    .to_string(),
                            ),
                            Style::default().fg(app.theme.fg_secondary),
                        )]));
                    } else {
                        lines.push(Line::from(vec![Span::styled(
                            i18n.dynamic(
                                "  No optimization results yet. Run optimization first."
                                    .to_string(),
                            ),
                            Style::default().fg(app.theme.accent_error),
                        )]));
                    }

                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![Span::styled(
                        i18n.dynamic(
                            " Enter=apply to rack  →=Select  ←/BackTab=Results".to_string(),
                        ),
                        Style::default().fg(app.theme.fg_secondary),
                    )]));
                }
                SpinUpdateSubStep::ConfirmOverwrite => {
                    if let Some((slot, count)) = s.update_existing_eq_info {
                        lines.push(Line::from(vec![Span::styled(
                            i18n.dynamic(format!(
                                "  Existing EQ in slot {} has {} filter(s).",
                                slot, count
                            )),
                            Style::default().fg(app.theme.accent_warning),
                        )]));
                        lines.push(Line::from(""));
                        lines.push(Line::from(vec![Span::styled(
                            i18n.dynamic("  Save current preset before overwriting?".to_string()),
                            Style::default()
                                .fg(app.theme.fg_primary)
                                .add_modifier(Modifier::BOLD),
                        )]));
                        lines.push(Line::from(""));
                        lines.push(Line::from(vec![
                            Span::styled(
                                "  y",
                                Style::default()
                                    .fg(app.theme.accent_success)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                i18n.dynamic(" = save preset then apply   ".to_string()),
                                Style::default().fg(app.theme.fg_secondary),
                            ),
                            Span::styled(
                                "n",
                                Style::default()
                                    .fg(app.theme.accent_error)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                i18n.dynamic(" = apply without saving   ".to_string()),
                                Style::default().fg(app.theme.fg_secondary),
                            ),
                            Span::styled(
                                "Esc",
                                Style::default()
                                    .fg(app.theme.fg_secondary)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                i18n.ui(" = cancel"),
                                Style::default().fg(app.theme.fg_secondary),
                            ),
                        ]));
                    }
                }
            }

            let para = Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(i18n.ui("Update Plugin")),
            );
            f.render_widget(para, content_area);
        }
    }
}
