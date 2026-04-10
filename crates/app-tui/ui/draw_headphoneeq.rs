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
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(step_tab_border)
                .border_style(Style::default().fg(step_tab_border_color))
                .title("Headphone EQ"),
        )
        .select(s.step as usize)
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
        HeadphoneEqStep::SelectFile => {
            use sotf_audio_player::headphone_eq_types::HeadphoneMeasurementSource;

            let is_spinorama =
                s.measurement_source == HeadphoneMeasurementSource::Spinorama;

            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // source toggle
                    Constraint::Length(if is_spinorama { 8 } else { 3 }), // measurement/search
                    Constraint::Length(3), // target preset
                    Constraint::Length(3), // custom target path
                    Constraint::Min(1),    // help
                ])
                .split(content);

            // Row 0: Source toggle
            let source_style = if s.selected_field == 0 {
                Style::default().fg(app.theme.accent_primary)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };
            let source = Paragraph::new(s.measurement_source.label())
                .style(source_style)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Source (Left/Right to toggle)"),
                );
            f.render_widget(source, inner[0]);

            // Row 1: Measurement path or Spinorama search
            let meas_style = if s.selected_field == 1 {
                Style::default().fg(app.theme.accent_primary)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };

            if is_spinorama {
                // Spinorama mode: show search and headphone list
                let search_area = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Min(1)])
                    .split(inner[1]);

                let search_label = if s.editing_search {
                    if s.loading_headphones {
                        "Search (loading...)"
                    } else {
                        "Search (editing)"
                    }
                } else if s.loading_download {
                    "Search (downloading...)"
                } else {
                    "Search headphones (Enter to edit)"
                };
                let search_text = if s.search_query.is_empty() && !s.editing_search {
                    if let Some(ref name) = s.selected_headphone {
                        format!("Selected: {}", name)
                    } else {
                        "<type to search>".to_string()
                    }
                } else {
                    s.search_query.clone()
                };
                let search = Paragraph::new(search_text)
                    .style(meas_style)
                    .block(Block::default().borders(Borders::ALL).title(search_label));
                f.render_widget(search, search_area[0]);

                // Headphone list (show when editing search)
                if s.editing_search && !s.filtered_headphones.is_empty() {
                    let items: Vec<ratatui::widgets::ListItem> = s
                        .filtered_headphones
                        .iter()
                        .take(10)
                        .enumerate()
                        .map(|(i, name)| {
                            let style = if i == s.selected_headphone_idx {
                                Style::default()
                                    .fg(app.theme.bg_primary)
                                    .bg(app.theme.accent_primary)
                            } else {
                                Style::default().fg(app.theme.fg_primary)
                            };
                            ratatui::widgets::ListItem::new(name.as_str()).style(style)
                        })
                        .collect();
                    let list = ratatui::widgets::List::new(items).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(format!("{} matches", s.filtered_headphones.len())),
                    );
                    f.render_widget(list, search_area[1]);
                }

                // Show measurement path if downloaded
                if !s.measurement_path.is_empty() && !s.editing_search {
                    let path = Paragraph::new(format!("Downloaded: {}", s.measurement_path))
                        .style(Style::default().fg(app.theme.fg_secondary));
                    f.render_widget(path, search_area[1]);
                }
            } else {
                // File mode: measurement path input
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
                f.render_widget(meas, inner[1]);
            }

            // Row 2: Target preset
            let target_style = if s.selected_field == 2 {
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
            f.render_widget(target, inner[2]);

            // Row 3: Custom target path
            if s.target_preset == "custom" {
                let custom_style = if s.selected_field == 3 {
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
                f.render_widget(custom, inner[3]);
            }

            let help = Paragraph::new(
                " Up/Down=select field  Enter=edit  Left/Right=toggle/cycle  Tab=next step",
            )
            .style(Style::default().fg(app.theme.fg_secondary));
            f.render_widget(help, inner[4]);

            // Autocomplete overlay below the active input field
            let editing_field_rect = if s.editing_measurement {
                Some(inner[1])
            } else if s.editing_custom_target {
                Some(inner[3])
            } else {
                None
            };
            if let Some(field_rect) = editing_field_rect {
                let ac_h = autocomplete_dropdown_height(app);
                if ac_h > 0 {
                    let ac_area = Rect {
                        x: field_rect.x,
                        y: field_rect.y + field_rect.height,
                        width: field_rect.width,
                        height: ac_h
                            .min(area.height.saturating_sub(field_rect.y + field_rect.height)),
                    };
                    f.render_widget(Clear, ac_area);
                    render_autocomplete_dropdown(f, ac_area, app);
                }
            }
        }

        HeadphoneEqStep::Configure => {
            use sotf_audio_player::autoeq::{
                self, DetailLevel, EqWorkflow, PEQ_MODEL_OPTIONS, HEADPHONE_LOSS_OPTIONS,
            };

            let c = &s.config;
            let bool_str = |b: bool| if b { "[ON]" } else { "[OFF]" };
            let detail = s.detail_level;

            // Build rows based on detail level.
            // Field indices stay stable across all modes — hidden fields just aren't shown.
            let rows: Vec<(Option<usize>, &str, String)> = match detail {
                DetailLevel::Simple => vec![
                    (None, "── Preset ──", String::new()),
                    (Some(100), "Preset", {
                        autoeq::find_preset(EqWorkflow::Headphone, &s.selected_preset)
                            .map(|p| p.name)
                            .unwrap_or(&s.selected_preset)
                            .to_string()
                    }),
                ],
                DetailLevel::Intermediate => vec![
                    (None, "── Preset ──", String::new()),
                    (Some(100), "Preset", {
                        autoeq::find_preset(EqWorkflow::Headphone, &s.selected_preset)
                            .map(|p| p.name)
                            .unwrap_or(&s.selected_preset)
                            .to_string()
                    }),
                    (None, "── Filter Design ──", String::new()),
                    (Some(0), "Filters (n)", format!("{}", c.num_filters)),
                    (Some(7), "Filter Type", autoeq::label_for(PEQ_MODEL_OPTIONS, &c.peq_model).to_string()),
                    (Some(1), "Min Freq (Hz)", format!("{:.0}", c.min_freq)),
                    (Some(2), "Max Freq (Hz)", format!("{:.0}", c.max_freq)),
                    (None, "── Goal ──", String::new()),
                    (Some(18), "Loss Function", autoeq::label_for(HEADPHONE_LOSS_OPTIONS, &c.loss).to_string()),
                ],
                DetailLevel::Expert => vec![
                    (None, "── Preset ──", String::new()),
                    (Some(100), "Preset", {
                        autoeq::find_preset(EqWorkflow::Headphone, &s.selected_preset)
                            .map(|p| p.name)
                            .unwrap_or(&s.selected_preset)
                            .to_string()
                    }),
                    (None, "── Filters ──", String::new()),
                    (Some(0), "Filters (n)", format!("{}", c.num_filters)),
                    (Some(1), "Min Freq (Hz)", format!("{:.0}", c.min_freq)),
                    (Some(2), "Max Freq (Hz)", format!("{:.0}", c.max_freq)),
                    (Some(3), "Min dB", format!("{:.1}", c.min_db)),
                    (Some(4), "Max dB", format!("{:.1}", c.max_db)),
                    (Some(5), "Min Q", format!("{:.2}", c.min_q)),
                    (Some(6), "Max Q", format!("{:.2}", c.max_q)),
                    (Some(7), "Filter Type", autoeq::label_for(PEQ_MODEL_OPTIONS, &c.peq_model).to_string()),
                    (Some(18), "Loss Function", autoeq::label_for(HEADPHONE_LOSS_OPTIONS, &c.loss).to_string()),
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
                ],
            };

            // Detail mode label
            let mode_label = match detail {
                DetailLevel::Simple => "Simple",
                DetailLevel::Intermediate => "Customize",
                DetailLevel::Expert => "All Parameters",
            };

            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Speaker: ", Style::default().fg(app.theme.fg_secondary)),
                    Span::styled(
                        &s.measurement_path,
                        Style::default().fg(app.theme.accent_primary),
                    ),
                    Span::styled(
                        format!("  [{}]  Tab=cycle mode", mode_label),
                        Style::default().fg(app.theme.fg_secondary),
                    ),
                ]),
                Line::from(""),
            ];

            // Show preset description in Simple mode
            if detail == DetailLevel::Simple {
                if let Some(preset) = autoeq::find_preset(EqWorkflow::Headphone, &s.selected_preset) {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", preset.description),
                        Style::default().fg(app.theme.fg_secondary),
                    )));
                    lines.push(Line::from(""));
                }
            }

            for (idx, label, value) in &rows {
                let is_selected = idx.is_some_and(|i| i == s.config_selected_field);
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

            // Context-sensitive hint line
            lines.push(Line::from(""));
            let hint = if s.editing_value {
                " Type value, Enter=confirm  Esc=cancel"
            } else {
                match s.config_selected_field {
                    0 => " 5-7 for quick results, 10+ for surgical precision",
                    1 | 2 => " Narrow the range to the problem region for faster results",
                    7 => " Left/Right to cycle filter types",
                    18 => " Left/Right to cycle loss functions",
                    100 => " Left/Right to change preset",
                    _ => " Up/Down=navigate  Left/Right=adjust  Enter=edit  Tab=cycle mode",
                }
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

            let table_height = (s.filters.len() as u16 + 3).min(15);
            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),            // summary
                    Constraint::Min(8),               // freq response chart
                    Constraint::Length(table_height), // filter table
                ])
                .split(content);

            let summary = Paragraph::new(vec![Line::from(vec![
                Span::styled(
                    format!(" {} filters", s.filters.len()),
                    Style::default().fg(app.theme.accent_primary),
                ),
                Span::raw("  |  "),
                Span::styled(
                    format!(
                        "Loss: {:.4} → {:.4} (Δ {:.4})",
                        s.pre_loss,
                        s.post_loss,
                        s.pre_loss - s.post_loss
                    ),
                    Style::default().fg(app.theme.accent_success),
                ),
            ])])
            .block(Block::default().borders(Borders::ALL).title("Summary"));
            f.render_widget(summary, inner[0]);

            draw_freq_response_chart(
                f,
                inner[1],
                app,
                &s.curve_frequencies,
                &s.curve_input,
                &s.curve_corrected,
                &s.curve_filter_response,
            );

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
            f.render_widget(table, inner[2]);
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
                                " = save preset then apply   ",
                                Style::default().fg(app.theme.fg_secondary),
                            ),
                            Span::styled(
                                "n",
                                Style::default()
                                    .fg(app.theme.accent_error)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                " = apply without saving   ",
                                Style::default().fg(app.theme.fg_secondary),
                            ),
                            Span::styled(
                                "Esc",
                                Style::default()
                                    .fg(app.theme.fg_secondary)
                                    .add_modifier(Modifier::BOLD),
                            ),
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
