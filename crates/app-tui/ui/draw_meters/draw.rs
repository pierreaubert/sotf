use super::super::*;
use super::meter_label_buf::MeterLabelBuf;
use std::fmt::Write as _;

pub(crate) fn draw_meters_column(f: &mut Frame, area: Rect, app: &mut App) {
    // Split the right column - LUFS, level meter, volume
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(15), // LUFS box (compact: 12 content lines + 2 borders + 1 padding)
            Constraint::Min(0),     // Level meter box (expandable)
            Constraint::Length(3),  // Volume box
        ])
        .split(area);

    // Draw LUFS info box
    draw_lufs_box(f, chunks[0], app);

    // Draw level meter box
    draw_level_meter_box(f, chunks[1], app);

    // Draw volume box
    draw_volume_box(f, chunks[2], app);
}

pub(crate) fn draw_loudness_and_volume_column(f: &mut Frame, area: Rect, app: &mut App) {
    // Split into loudness and volume (no level meters - they're in a separate column)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(15), // LUFS box (compact)
            Constraint::Min(0),     // Spacer (expandable)
            Constraint::Length(3),  // Volume box
        ])
        .split(area);

    // Draw LUFS info box
    draw_lufs_box(f, chunks[0], app);

    // Draw volume box
    draw_volume_box(f, chunks[2], app);
}

fn lufs_maximum_summary_rows(label: &str, width: u16) -> u16 {
    // Reserve enough room for a plausible low LUFS value such as "-120.0".
    // Labels stack with the value when the translated line would be clipped.
    let maximum_value_width = 6;
    if label.chars().count() + 1 + maximum_value_width <= usize::from(width) {
        1
    } else {
        2
    }
}

fn draw_lufs_maximum_summary(
    f: &mut Frame,
    label_buf: &mut MeterLabelBuf,
    label: &str,
    maximum: Option<f64>,
    area: Rect,
    color: Color,
) -> u16 {
    let rows = lufs_maximum_summary_rows(label, area.width);
    if rows == 2 {
        f.render_widget(
            Paragraph::new(label).style(Style::default().fg(color)),
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            },
        );
    }

    label_buf.len = 0;
    if rows == 1 {
        let _ = write!(label_buf, "{label} ");
    }
    match maximum {
        Some(value) if value.is_finite() => {
            let _ = write!(label_buf, "{value:.1}");
        }
        _ => {
            let _ = write!(label_buf, "—");
        }
    }
    f.render_widget(
        Paragraph::new(label_buf.as_str()).style(Style::default().fg(color)),
        Rect {
            x: area.x,
            y: area.y + rows - 1,
            width: area.width,
            height: 1,
        },
    );
    rows
}

pub(crate) fn draw_lufs_box(f: &mut Frame, area: Rect, app: &App) {
    let i18n = crate::i18n::TuiTranslations::for_language(app.ui.language);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(i18n.ui("Loudness"))
        .style(Style::default().fg(app.theme.fg_primary));
    f.render_widget(block, area);

    // Inner area for content (excluding borders)
    let inner = area.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 1,
    });

    if inner.height < 3 {
        // Not enough space
        return;
    }

    if let Some(ref loudness) = app.playback.loudness_info {
        let mut y_offset = 0;
        // Reused stack buffer for all short numeric gauge labels in this box.
        let mut label_buf = MeterLabelBuf::new();

        // ============================================================================
        // True Peak Section
        // ============================================================================

        let unavailable = match loudness.maximum_true_peak_dbtp {
            Some(value) => !value.is_finite(),
            None => !loudness.true_peak_is_compliant,
        };
        let unavailable_label = i18n.ui("Max TP");
        let unavailable_text = i18n.ui("Unavailable");
        let unavailable_line_width =
            unavailable_label.chars().count() + 2 + unavailable_text.chars().count();
        let max_tp_rows: u16 = if unavailable && usize::from(inner.width) < unavailable_line_width {
            2
        } else {
            1
        };
        let maximum_momentary_label = i18n.ui("Max momentary");
        let maximum_shortterm_label = i18n.ui("Max short-term");
        let maximum_momentary_rows =
            lufs_maximum_summary_rows(maximum_momentary_label, inner.width);
        let maximum_shortterm_rows =
            lufs_maximum_summary_rows(maximum_shortterm_label, inner.width);
        let required_lufs_rows = 1 + maximum_momentary_rows + maximum_shortterm_rows + 3;
        let optional_meter_rows = usize::from(
            inner
                .height
                .saturating_sub(max_tp_rows.saturating_add(required_lufs_rows)),
        );
        let num_peak_bars = loudness.true_peaks_dbtp.len().min(2);
        let visible_peak_bars = num_peak_bars.min(optional_meter_rows);
        let optional_after_peaks = optional_meter_rows.saturating_sub(visible_peak_bars);
        let show_true_peak_scale = optional_after_peaks >= 1;
        let show_lufs_scale = optional_after_peaks >= 2;

        if y_offset < inner.height {
            // Stack the heading and status whenever their localized combined
            // width exceeds the available inner width, so narrow terminals do
            // not clip the unavailable state.
            if unavailable && usize::from(inner.width) < unavailable_line_width {
                for text in [unavailable_label, unavailable_text] {
                    if y_offset >= inner.height {
                        break;
                    }
                    f.render_widget(
                        Paragraph::new(text).style(Style::default().fg(app.theme.title_color)),
                        Rect {
                            x: inner.x,
                            y: inner.y + y_offset,
                            width: inner.width,
                            height: 1,
                        },
                    );
                    y_offset += 1;
                }
            } else {
                label_buf.len = 0;
                match loudness.maximum_true_peak_dbtp {
                    Some(value) if value.is_finite() => {
                        let _ =
                            write!(&mut label_buf, "{}: {:>4.1} dBTP", i18n.ui("Max TP"), value);
                    }
                    Some(_) => {
                        let _ = write!(
                            &mut label_buf,
                            "{}: {}",
                            i18n.ui("Max TP"),
                            i18n.ui("Unavailable")
                        );
                    }
                    None if loudness.true_peak_is_compliant => {
                        let _ = write!(&mut label_buf, "{}: — dBTP", i18n.ui("Max TP"));
                    }
                    None => {
                        let _ = write!(
                            &mut label_buf,
                            "{}: {}",
                            i18n.ui("Max TP"),
                            i18n.ui("Unavailable")
                        );
                    }
                }
                f.render_widget(
                    Paragraph::new(label_buf.as_str())
                        .style(Style::default().fg(app.theme.title_color)),
                    Rect {
                        x: inner.x,
                        y: inner.y + y_offset,
                        width: inner.width,
                        height: 1,
                    },
                );
                y_offset += 1;
            }

            // Render true peak bars for each channel (max 2 bars to save space)
            for ch_idx in 0..visible_peak_bars {
                if y_offset >= inner.height {
                    break;
                }

                let true_peak_dbtp = loudness
                    .true_peaks_dbtp
                    .get(ch_idx)
                    .copied()
                    .unwrap_or(f64::NEG_INFINITY);

                // Map -60 dBTP to 0%, +6 dBTP to 100%
                let ratio = if true_peak_dbtp.is_finite() {
                    ((true_peak_dbtp + 60.0) / 66.0).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                // Choose color based on level: green → orange → red when >0
                // bg sets the label text color on the filled portion (fg/bg are swapped for labels)
                let gauge_style = if true_peak_dbtp > 0.0 {
                    Style::default().fg(app.theme.accent_error).bg(Color::White)
                } else if true_peak_dbtp > -1.0 {
                    Style::default()
                        .fg(app.theme.accent_warning)
                        .bg(Color::Black)
                } else {
                    Style::default()
                        .fg(app.theme.accent_success)
                        .bg(Color::Black)
                };

                // Format label showing the dBTP value into the reused stack buffer.
                label_buf.len = 0;
                if true_peak_dbtp.is_finite() {
                    let _ = write!(&mut label_buf, "{:>5.1}", true_peak_dbtp);
                } else {
                    let _ = write!(&mut label_buf, "  -∞");
                }

                use ratatui::widgets::Gauge;
                let gauge = Gauge::default()
                    .ratio(ratio)
                    .label(label_buf.as_str())
                    .gauge_style(gauge_style)
                    .use_unicode(true);

                f.render_widget(
                    gauge,
                    Rect {
                        x: inner.x,
                        y: inner.y + y_offset,
                        width: inner.width,
                        height: 1,
                    },
                );
                y_offset += 1;
            }

            // Scale labels: "-60" at left, "0" at 60/66 position, "+6" at right.
            // Rendered as separate static spans instead of building a whitespace string.
            if show_true_peak_scale && y_offset < inner.height {
                let width = inner.width as usize;
                // True peak scale: -60 dBTP to +6 dBTP (total range 66 dB)
                // Position of 0 dBTP: 60/66 ≈ 0.909
                let zero_pos = ((60.0 / 66.0) * width as f64) as u16;
                let max_pos = width.saturating_sub(2).min(inner.width as usize) as u16; // "+6" is 2 chars

                let scale_style = Style::default().fg(app.theme.fg_muted);
                f.render_widget(
                    Paragraph::new("-60").style(scale_style),
                    Rect {
                        x: inner.x,
                        y: inner.y + y_offset,
                        width: 3,
                        height: 1,
                    },
                );
                if zero_pos > 0 && zero_pos < inner.width {
                    f.render_widget(
                        Paragraph::new("0").style(scale_style),
                        Rect {
                            x: inner.x + zero_pos,
                            y: inner.y + y_offset,
                            width: 1,
                            height: 1,
                        },
                    );
                }
                if max_pos + 1 < inner.width {
                    f.render_widget(
                        Paragraph::new("+6").style(scale_style),
                        Rect {
                            x: inner.x + max_pos,
                            y: inner.y + y_offset,
                            width: 2,
                            height: 1,
                        },
                    );
                }
                y_offset += 1;
            }
        }

        // ============================================================================
        // LUFS Section
        // ============================================================================

        if y_offset < inner.height {
            f.render_widget(
                Paragraph::new("LUFS").style(Style::default().fg(app.theme.title_color)),
                Rect {
                    x: inner.x,
                    y: inner.y + y_offset,
                    width: inner.width,
                    height: 1,
                },
            );
            y_offset += 1;
        }

        // Helper function to draw LUFS bar using Gauge widget.
        // Borrows the shared stack label buffer to avoid per-bar `format!` calls.
        let draw_lufs_bar =
            |f: &mut Frame, label_buf: &mut MeterLabelBuf, y: u16, label_char: &str, lufs: f64| {
                // Map -60 to 0 LUFS as 0% to 100%
                let ratio = if lufs.is_finite() {
                    ((lufs + 60.0) / 60.0).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                // Choose color: green → orange → red based on level
                // bg sets the label text color on the filled portion (fg/bg are swapped for labels)
                let gauge_style = if lufs > -1.0 {
                    Style::default().fg(app.theme.accent_error).bg(Color::White)
                } else if lufs > -10.0 {
                    Style::default()
                        .fg(app.theme.accent_warning)
                        .bg(Color::Black)
                } else {
                    Style::default()
                        .fg(app.theme.accent_success)
                        .bg(Color::Black)
                };

                // Format label: "M -15.0" into the reused stack buffer.
                label_buf.len = 0;
                let _ = write!(label_buf, "{} ", label_char);
                if lufs.is_finite() {
                    let _ = write!(label_buf, "{:>5.1}", lufs);
                } else {
                    let _ = write!(label_buf, "  -∞");
                }

                use ratatui::widgets::Gauge;
                let gauge = Gauge::default()
                    .ratio(ratio)
                    .label(label_buf.as_str())
                    .gauge_style(gauge_style)
                    .use_unicode(true);

                f.render_widget(
                    gauge,
                    Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: 1,
                    },
                );
            };

        // Maximum Momentary and Short-term summaries are independent of the
        // current-window validity flags. The finite latches remain meaningful
        // across publication/cache transitions that invalidate current values.
        for (label, maximum) in [
            (maximum_momentary_label, loudness.maximum_momentary_lufs),
            (maximum_shortterm_label, loudness.maximum_shortterm_lufs),
        ] {
            let rows = lufs_maximum_summary_rows(label, inner.width);
            if y_offset.saturating_add(rows) <= inner.height {
                let drawn_rows = draw_lufs_maximum_summary(
                    f,
                    &mut label_buf,
                    label,
                    maximum,
                    Rect {
                        x: inner.x,
                        y: inner.y + y_offset,
                        width: inner.width,
                        height: inner.height - y_offset,
                    },
                    app.theme.title_color,
                );
                y_offset = y_offset.saturating_add(drawn_rows);
            }
        }

        // M (Momentary)
        if y_offset < inner.height {
            draw_lufs_bar(
                f,
                &mut label_buf,
                inner.y + y_offset,
                "M",
                loudness.momentary_lufs,
            );
            y_offset += 1;
        }

        // S (Short-term)
        if y_offset < inner.height {
            draw_lufs_bar(
                f,
                &mut label_buf,
                inner.y + y_offset,
                "S",
                loudness.shortterm_lufs,
            );
            y_offset += 1;
        }

        // I (Integrated)
        if y_offset < inner.height {
            draw_lufs_bar(
                f,
                &mut label_buf,
                inner.y + y_offset,
                "I",
                loudness.integrated_lufs,
            );
            y_offset += 1;
        }

        // Scale labels: "-60" at left, "0" at right
        if show_lufs_scale && y_offset < inner.height {
            let scale_style = Style::default().fg(app.theme.fg_muted);
            f.render_widget(
                Paragraph::new("-60").style(scale_style),
                Rect {
                    x: inner.x,
                    y: inner.y + y_offset,
                    width: 3,
                    height: 1,
                },
            );
            if inner.width >= 2 {
                f.render_widget(
                    Paragraph::new("0").style(scale_style),
                    Rect {
                        x: inner.x + inner.width - 1,
                        y: inner.y + y_offset,
                        width: 1,
                        height: 1,
                    },
                );
            }
            y_offset += 1;
        }

        // ============================================================================
        // Stereo Width Section (only for stereo)
        // ============================================================================

        if let Some(correlation) = loudness.correlation_lr {
            if y_offset < inner.height {
                f.render_widget(
                    Paragraph::new(i18n.ui("Stereo width"))
                        .style(Style::default().fg(app.theme.title_color)),
                    Rect {
                        x: inner.x,
                        y: inner.y + y_offset,
                        width: inner.width,
                        height: 1,
                    },
                );
                y_offset += 1;
            }

            if y_offset < inner.height {
                use ratatui::widgets::Gauge;

                // Correlation is typically between 0 and 1 for normal stereo content
                // 0 = uncorrelated (wide stereo), 1 = fully correlated (mono)
                // For "Stereo width" display, invert it so higher = wider
                let stereo_width = (1.0 - correlation).clamp(0.0, 1.0);
                let ratio = stereo_width;

                // Choose color based on stereo width
                // bg sets the label text color on the filled portion (fg/bg are swapped for labels)
                let gauge_style = if stereo_width < 0.1 {
                    Style::default()
                        .fg(app.theme.accent_warning)
                        .bg(Color::Black)
                } else {
                    Style::default()
                        .fg(app.theme.accent_success)
                        .bg(Color::Black)
                };

                label_buf.len = 0;
                let _ = write!(&mut label_buf, "{:>4.2}", stereo_width);

                let gauge = Gauge::default()
                    .ratio(ratio)
                    .label(label_buf.as_str())
                    .gauge_style(gauge_style)
                    .use_unicode(true);

                f.render_widget(
                    gauge,
                    Rect {
                        x: inner.x,
                        y: inner.y + y_offset,
                        width: inner.width,
                        height: 1,
                    },
                );
                y_offset += 1;
            }

            // Scale labels: "0" at left, "1" at right
            if y_offset < inner.height {
                let scale_style = Style::default().fg(app.theme.fg_muted);
                f.render_widget(
                    Paragraph::new("0").style(scale_style),
                    Rect {
                        x: inner.x,
                        y: inner.y + y_offset,
                        width: 1,
                        height: 1,
                    },
                );
                if inner.width >= 2 {
                    f.render_widget(
                        Paragraph::new("1").style(scale_style),
                        Rect {
                            x: inner.x + inner.width - 1,
                            y: inner.y + y_offset,
                            width: 1,
                            height: 1,
                        },
                    );
                }
                y_offset += 1;
            }
        }

        // LRA is secondary to the current M/S/I values and their maxima.
        // Draw its entire numeric/status block only when it fits the remaining
        // inner area, so a short terminal never overwrites the box border.
        let range = loudness.loudness_range.as_ref();
        let finite_valid_range = range
            .filter(|data| data.status == sotf_plugins::analyzer::LoudnessRangeStatus::Valid)
            .and_then(|data| {
                data.range_lu
                    .filter(|value| value.is_finite() && *value >= 0.0)
            });
        let show_unstable =
            finite_valid_range.is_some() && range.is_some_and(|data| !data.is_stable);
        label_buf.len = 0;
        if let Some(value) = finite_valid_range {
            let _ = write!(&mut label_buf, "LRA: {value:.1} LU");
        } else {
            let _ = write!(&mut label_buf, "LRA: {}", i18n.ui("Unavailable"));
        }
        let lra_text = label_buf.as_str();
        let unstable_text = i18n.ui("Not stable");
        let lra_rows = if show_unstable { 2 } else { 1 };
        let lra_fits_width = lra_text.chars().count() <= usize::from(inner.width)
            && (!show_unstable || unstable_text.chars().count() <= usize::from(inner.width));
        if lra_fits_width && y_offset.saturating_add(lra_rows) <= inner.height {
            f.render_widget(
                Paragraph::new(lra_text).style(Style::default().fg(app.theme.title_color)),
                Rect {
                    x: inner.x,
                    y: inner.y + y_offset,
                    width: inner.width,
                    height: 1,
                },
            );
            if show_unstable {
                f.render_widget(
                    Paragraph::new(unstable_text).style(Style::default().fg(app.theme.title_color)),
                    Rect {
                        x: inner.x,
                        y: inner.y + y_offset + 1,
                        width: inner.width,
                        height: 1,
                    },
                );
            }
        }
    } else {
        // No loudness data
        f.render_widget(
            Paragraph::new(i18n.ui("No audio playing"))
                .style(Style::default().fg(app.theme.fg_muted))
                .alignment(Alignment::Center),
            Rect {
                x: inner.x,
                y: inner.y + inner.height / 2,
                width: inner.width,
                height: 1,
            },
        );
    }
}

pub(crate) fn draw_level_meter_box(f: &mut Frame, area: Rect, app: &mut App) {
    let i18n = crate::i18n::TuiTranslations::for_language(app.ui.language);
    // Check for loudness info first
    let has_loudness = app.playback.loudness_info.is_some();
    if !has_loudness {
        let paragraph = Paragraph::new(i18n.ui("No audio"))
            .style(Style::default().fg(app.theme.fg_muted))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(i18n.ui("Levels")),
            )
            .alignment(Alignment::Center);
        f.render_widget(paragraph, area);
        return;
    }

    let num_channels = app
        .playback
        .loudness_info
        .as_ref()
        .map(|l| l.channel_peaks.len())
        .unwrap_or(0);
    if num_channels == 0 {
        let paragraph = Paragraph::new(i18n.ui("No channels"))
            .style(Style::default().fg(app.theme.fg_muted))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(i18n.ui("Levels")),
            );
        f.render_widget(paragraph, area);
        return;
    }

    // Update channel groups if needed (method handles caching internally)
    // Do this BEFORE borrowing loudness immutably
    app.update_level_meter_groups();

    // Now borrow loudness immutably for the rest of the function
    let loudness = app.playback.loudness_info.as_ref().unwrap();

    // Draw border with simple title
    let control_status = if let Some(error) = app.plugin_rack.loudness_control_error.as_ref() {
        error.clone()
    } else if let Some(pending) = app.plugin_rack.pending_loudness_control {
        if pending.runtime_instance_id != loudness.integrated_control_instance_id {
            i18n.ui("Monitor changed; request not confirmed")
                .to_string()
        } else if loudness.integrated_control_request_id > pending.request_id {
            i18n.ui("Request superseded").to_string()
        } else if loudness.integrated_control_request_id == pending.request_id {
            if loudness.integrated_measurement_running {
                i18n.ui("Integrated/LRA running").to_string()
            } else {
                i18n.ui("Integrated/LRA paused").to_string()
            }
        } else {
            format!(
                "{} · {}",
                i18n.ui(pending.operation.help_label()),
                i18n.ui("Waiting for meter update")
            )
        }
    } else if loudness.integrated_measurement_running {
        i18n.ui("Integrated/LRA running").to_string()
    } else {
        i18n.ui("Integrated/LRA paused").to_string()
    };
    let title_lines = [Line::from(format!(
        "{} · {control_status}",
        i18n.ui("Levels (help: ?)")
    ))];
    let title_height = 1;

    // Highlight border when focused
    let block = if app.input_mode == InputMode::LevelMeters {
        Block::default().borders(Borders::ALL).border_style(
            Style::default()
                .fg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(app.theme.fg_primary))
    };
    f.render_widget(block, area);

    // Render title lines at the top inside the border
    for (i, line) in title_lines.iter().enumerate() {
        f.render_widget(
            Paragraph::new(line.clone()).style(Style::default().fg(app.theme.fg_primary)),
            Rect {
                x: area.x + 1,
                y: area.y + 1 + i as u16,
                width: area.width.saturating_sub(2),
                height: 1,
            },
        );
    }

    // Create inner area for meters (after title lines and borders)
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1 + title_height,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2 + title_height),
    };

    // Calculate dimensions
    let max_name_lines = app
        .level_meters
        .groups
        .iter()
        .flat_map(|g| &g.channels)
        .map(|ch| ch.display_name.len())
        .max()
        .unwrap_or(1);

    // Reserve space for label/names and M/S/D controls (3 lines)
    // For stereo: 1 line for "L - R" label + 3 lines for controls
    // For multi-channel: max_name_lines + 3 lines for controls
    let meter_height = (inner.height as usize).saturating_sub(max_name_lines + 3);
    if meter_height == 0 {
        return;
    }

    // Scale legend width (3 chars: "-60", "-20", " 0 ") + 2-char gap to meters
    let scale_text_width = 3usize;
    let scale_gap = 2usize;
    let scale_width = scale_text_width + scale_gap; // 5 total before meters
    let available_width = inner.width as usize;

    // Check if we should show right-side scale (only for single stereo group with enough space)
    let is_single_stereo_group =
        app.level_meters.groups.len() == 1 && app.level_meters.groups[0].channels.len() == 2;
    let right_scale_width = if is_single_stereo_group && available_width >= scale_width * 2 + 8 {
        scale_width
    } else {
        0
    };

    // Calculate total width for multi-group layout
    let num_group_gaps = app.level_meters.groups.len().saturating_sub(1);

    // First try with padded group widths (min 3 for [M][S][D] controls)
    let padded_groups_width: usize = app
        .level_meters
        .groups
        .iter()
        .map(|g| g.channels.len().max(3))
        .sum::<usize>()
        + num_group_gaps;

    // Available space for meters (after the dB scale legend)
    let meters_area = available_width.saturating_sub(scale_width);

    // If padded widths don't fit, use actual channel counts (no min 3 padding)
    let compact_groups_width: usize = app
        .level_meters
        .groups
        .iter()
        .map(|g| g.channels.len())
        .sum::<usize>()
        + num_group_gaps;

    let (total_groups_width, use_compact) = if padded_groups_width <= meters_area {
        (padded_groups_width, false)
    } else {
        (compact_groups_width, true)
    };

    // Right-align within the meters area (after scale), clamped so we don't go before scale
    let mut x_offset = if is_single_stereo_group {
        scale_width // stereo branch handles its own centering
    } else {
        let right_aligned = scale_width + meters_area.saturating_sub(total_groups_width);
        right_aligned.max(scale_width)
    };

    // Position the dB scale legend just before the meters (2 chars gap)
    let scale_x = if is_single_stereo_group {
        0 // stereo: legend at left edge
    } else {
        x_offset.saturating_sub(scale_width)
    };

    // Non-linear scale: -60 dB (0%), -40 dB (20%), -20 dB (50%), 0 dB (100%)
    let scale_markers = [
        (1.0, " 0 "), // 100% fill -> top
        (0.5, "-20"), // 50% fill
        (0.2, "-40"), // 20% fill
        (0.0, "-60"), // 0% fill -> bottom
    ];

    // Draw vertical scale legend on the left
    for (ratio, label) in scale_markers.iter() {
        let row_idx = (ratio * meter_height as f64).round() as usize;
        let y = inner.y + (meter_height - 1).saturating_sub(row_idx.min(meter_height - 1)) as u16;

        f.render_widget(
            Paragraph::new(*label).style(Style::default().fg(app.theme.fg_muted)),
            Rect {
                x: inner.x + scale_x as u16,
                y,
                width: scale_text_width as u16,
                height: 1,
            },
        );
    }

    // Draw vertical scale legend on the right (if applicable, stereo only)
    if right_scale_width > 0 {
        let right_x = inner.x + inner.width - scale_text_width as u16;
        for (ratio, label) in scale_markers.iter() {
            let row_idx = (ratio * meter_height as f64).round() as usize;
            let y =
                inner.y + (meter_height - 1).saturating_sub(row_idx.min(meter_height - 1)) as u16;

            f.render_widget(
                Paragraph::new(*label).style(Style::default().fg(app.theme.fg_muted)),
                Rect {
                    x: right_x,
                    y,
                    width: scale_text_width as u16,
                    height: 1,
                },
            );
        }
    }

    // Draw each group
    for (group_idx, group) in app.level_meters.groups.iter().enumerate() {
        let is_selected = group_idx == app.level_meters.selected_group;

        // Calculate width for this group
        let num_channels = group.channels.len();
        let is_stereo = is_single_stereo_group;
        let group_width = if is_stereo {
            8 // 3 + 2 + 3 for stereo
        } else if use_compact {
            num_channels
        } else {
            num_channels.max(3)
        };

        if is_stereo {
            // Special stereo rendering: 3-char wide meters with 2-char spacing
            // Center the group in the meter area (between left and right scales)
            let meter_area_start = scale_width;
            let meter_area_end = available_width.saturating_sub(right_scale_width);
            let meter_area_width = meter_area_end.saturating_sub(meter_area_start);
            let group_start_x = if meter_area_width > group_width {
                meter_area_start + (meter_area_width - group_width) / 2
            } else {
                meter_area_start
            };

            // Skip rendering if there's not enough space
            if group_start_x + group_width > available_width {
                continue;
            }

            // Draw L meter (3 chars wide)
            let l_channel = &group.channels[0];
            let l_peak = loudness
                .channel_peaks
                .get(l_channel.index)
                .copied()
                .unwrap_or(0.0);
            let l_peak_db = 20.0 * l_peak.max(0.0001).log10();

            // Linear dB scale: -60 dB to 0 dB
            let l_fill_ratio = ((l_peak_db + 60.0) / 60.0).clamp(0.0, 1.0);
            let l_filled_rows = (l_fill_ratio * meter_height as f64).round() as usize;

            // Draw R meter (3 chars wide)
            let r_channel = &group.channels[1];
            let r_peak = loudness
                .channel_peaks
                .get(r_channel.index)
                .copied()
                .unwrap_or(0.0);
            let r_peak_db = 20.0 * r_peak.max(0.0001).log10();

            // Linear dB scale: -60 dB to 0 dB
            let r_fill_ratio = ((r_peak_db + 60.0) / 60.0).clamp(0.0, 1.0);
            let r_filled_rows = (r_fill_ratio * meter_height as f64).round() as usize;

            // Draw both meters
            for row_idx in (0..meter_height).rev() {
                let y = inner.y + (meter_height - 1 - row_idx) as u16;
                let level_ratio = row_idx as f64 / meter_height as f64;
                let color = if level_ratio > 0.95 {
                    app.theme.accent_error
                } else if level_ratio > 0.90 {
                    app.theme.accent_warning
                } else {
                    app.theme.accent_success
                };

                // L meter (3 chars)
                let l_is_filled = row_idx < l_filled_rows;
                let l_bar = if l_is_filled {
                    "███"
                } else {
                    "░░░"
                };
                let l_style = if l_is_filled {
                    Style::default().fg(color)
                } else {
                    Style::default().fg(app.theme.fg_muted)
                };
                f.render_widget(
                    Paragraph::new(l_bar).style(l_style),
                    Rect {
                        x: inner.x + group_start_x as u16,
                        y,
                        width: 3,
                        height: 1,
                    },
                );

                // R meter (3 chars) - skip 2 chars for spacing
                let r_is_filled = row_idx < r_filled_rows;
                let r_bar = if r_is_filled {
                    "███"
                } else {
                    "░░░"
                };
                let r_style = if r_is_filled {
                    Style::default().fg(color)
                } else {
                    Style::default().fg(app.theme.fg_muted)
                };
                f.render_widget(
                    Paragraph::new(r_bar).style(r_style),
                    Rect {
                        x: inner.x + group_start_x as u16 + 5, // 3 (L) + 2 (spacing)
                        y,
                        width: 3,
                        height: 1,
                    },
                );
            }

            // Draw "L - R" label centered below meters
            let name_start_y = inner.y + meter_height as u16;
            let label = "L - R";
            let label_x = group_start_x + (group_width - label.len()) / 2;
            f.render_widget(
                Paragraph::new(label).style(Style::default().fg(app.theme.fg_primary)),
                Rect {
                    x: inner.x + label_x as u16,
                    y: name_start_y,
                    width: label.len() as u16,
                    height: 1,
                },
            );
        } else {
            // Original rendering for non-stereo: 1 char wide meters
            for (ch_idx, channel) in group.channels.iter().enumerate() {
                let ch_x_offset = x_offset + ch_idx;
                if ch_x_offset >= available_width {
                    break;
                }

                // Get the peak level for this channel
                let peak = loudness
                    .channel_peaks
                    .get(channel.index)
                    .copied()
                    .unwrap_or(0.0);
                let peak_db = 20.0 * peak.max(0.0001).log10();

                // Linear dB scale: -60 dB to 0 dB
                let fill_ratio = ((peak_db + 60.0) / 60.0).clamp(0.0, 1.0);
                let filled_rows = (fill_ratio * meter_height as f64).round() as usize;

                // Draw vertical meter (1 char wide)
                let meter_x = inner.x + ch_x_offset as u16;
                for row_idx in (0..meter_height).rev() {
                    let y = inner.y + (meter_height - 1 - row_idx) as u16;
                    let is_filled = row_idx < filled_rows;

                    let level_ratio = row_idx as f64 / meter_height as f64;
                    let color = if level_ratio > 0.95 {
                        app.theme.accent_error
                    } else if level_ratio > 0.90 {
                        app.theme.accent_warning
                    } else {
                        app.theme.accent_success
                    };

                    let bar = if is_filled { "█" } else { "░" };
                    let style = if is_filled {
                        Style::default().fg(color)
                    } else {
                        Style::default().fg(app.theme.fg_muted)
                    };

                    f.render_widget(
                        Paragraph::new(bar).style(style),
                        Rect {
                            x: meter_x,
                            y,
                            width: 1,
                            height: 1,
                        },
                    );
                }

                // Draw vertical channel name below meter
                let name_start_y = inner.y + meter_height as u16;
                for (line_idx, line) in channel.display_name.iter().enumerate() {
                    let y = name_start_y + line_idx as u16;
                    if y < inner.y + inner.height - 2 {
                        // -2 for M/S controls
                        f.render_widget(
                            Paragraph::new(line.as_str())
                                .style(Style::default().fg(app.theme.fg_primary)),
                            Rect {
                                x: meter_x,
                                y,
                                width: 1,
                                height: 1,
                            },
                        );
                    }
                }
            }
        }

        // Draw M/S/D controls for this group
        // Show controls if there's space, or always for stereo (centered layout)
        let show_controls =
            is_stereo || (app.level_meters.groups.len() > 1 && x_offset + 3 <= available_width);
        if show_controls {
            // Center [M][S][D] (3 chars) under the group
            let controls_x = if is_stereo {
                let meter_area_start = scale_width;
                let meter_area_end = available_width.saturating_sub(right_scale_width);
                let meter_area_width = meter_area_end.saturating_sub(meter_area_start);
                let group_start_x = if meter_area_width > group_width {
                    meter_area_start + (meter_area_width - group_width) / 2
                } else {
                    meter_area_start
                };
                // Center [M][S][D] under the 8-char stereo group
                let ctrl_offset = group_start_x + (group_width - 3) / 2;
                if ctrl_offset + 3 > available_width {
                    x_offset += group_width + 1;
                    continue;
                }
                inner.x + ctrl_offset as u16
            } else {
                // Center [M][S][D] (3 chars) under the group_width
                let ctrl_offset = x_offset + group_width.saturating_sub(3) / 2;
                if ctrl_offset + 3 > available_width {
                    x_offset += group_width + 1;
                    continue;
                }
                inner.x + ctrl_offset as u16
            };

            // Position controls below the label/channel names
            let controls_y = inner.y + meter_height as u16 + max_name_lines as u16;

            // Mute button
            let mute_style = if is_selected && app.level_meters.control_selection == 0 {
                Style::default()
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.bg_selected)
                    .add_modifier(Modifier::BOLD)
            } else if group.muted {
                Style::default().fg(app.theme.accent_error)
            } else {
                Style::default().fg(app.theme.fg_muted)
            };

            f.render_widget(
                Paragraph::new("[M]").style(mute_style),
                Rect {
                    x: controls_x,
                    y: controls_y,
                    width: 3,
                    height: 1,
                },
            );

            // Solo button
            let solo_style = if is_selected && app.level_meters.control_selection == 1 {
                Style::default()
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.bg_selected)
                    .add_modifier(Modifier::BOLD)
            } else if group.soloed {
                Style::default().fg(app.theme.accent_warning)
            } else {
                Style::default().fg(app.theme.fg_muted)
            };

            f.render_widget(
                Paragraph::new("[S]").style(solo_style),
                Rect {
                    x: controls_x,
                    y: controls_y + 1,
                    width: 3,
                    height: 1,
                },
            );

            // Dim button
            let dim_style = if is_selected && app.level_meters.control_selection == 2 {
                Style::default()
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.bg_selected)
                    .add_modifier(Modifier::BOLD)
            } else if group.dimmed {
                Style::default().fg(app.theme.accent_info)
            } else {
                Style::default().fg(app.theme.fg_muted)
            };

            f.render_widget(
                Paragraph::new("[D]").style(dim_style),
                Rect {
                    x: controls_x,
                    y: controls_y + 2,
                    width: 3,
                    height: 1,
                },
            );
        }

        // Advance by group width + 1 space between groups
        x_offset += group_width + 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::theme::Theme;
    use ratatui::{Terminal, backend::TestBackend};
    use sotf_audio::LoudnessData;
    use sotf_plugins::analyzer::{LoudnessRangeData, LoudnessRangeMode, LoudnessRangeStatus};
    use std::sync::Arc;

    fn test_app_with_loudness() -> App {
        let mut app = App::new(Theme::default(), /* read_only */ true);
        app.playback.loudness_info = Some(LoudnessData {
            measurement_valid: true,
            query_error_generation: 0,
            measurement_enabled: true,
            channel_layout_is_compliant: true,
            momentary_lufs: -10.5,
            shortterm_lufs: -12.0,
            integrated_lufs: -14.0,
            peak: 0.5,
            channel_peaks: Arc::new(vec![0.5, 0.3]),
            true_peaks_dbtp: Arc::new(vec![-3.5, -6.0]),
            maximum_true_peak_dbtp: Some(-1.2),
            maximum_momentary_lufs: Some(-9.4),
            maximum_shortterm_lufs: Some(-11.2),
            momentary_valid: false,
            shortterm_valid: false,
            true_peak_is_compliant: true,
            integrated_window_seconds: 3_600,
            correlation_lr: Some(0.8),
            correlation_matrix: Arc::new(Vec::new()),
            correlation_samples_seen: 0,
            loudness_range: Some(LoudnessRangeData {
                range_lu: Some(0.0),
                is_stable: false,
                status: LoudnessRangeStatus::Valid,
                mode: LoudnessRangeMode::Rolling,
                retained_windows: 1,
                observed_windows: 1,
                capacity_windows: 36_000,
                timebase_is_exact: true,
            }),
            ..Default::default()
        });
        app
    }

    #[test]
    fn unsupported_programme_max_is_visible_without_interval_channels() {
        let mut app = App::new(Theme::default(), /* read_only */ true);
        app.ui.language = crate::i18n::Language::German;
        app.playback.loudness_info = Some(LoudnessData::new(0));
        let backend = TestBackend::new(24, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_lufs_box(f, area, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert!(
            content.contains("Max TP") && content.contains("Nicht verfügbar"),
            "expected the localized unsupported status to wrap within a 22-column inner box; got {content:?}"
        );
    }

    /// Smoke / regression test: `draw_lufs_box` must render the loudness box
    /// without relying on per-frame `String` allocations for labels or scale
    /// strings. We verify that the expected sections are written into the
    /// terminal buffer.
    #[test]
    fn draw_lufs_box_renders_all_sections() {
        let backend = TestBackend::new(24, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = test_app_with_loudness();
        terminal
            .draw(|f| {
                let area = f.area();
                draw_lufs_box(f, area, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("Max TP: -1.2 dBTP"),
            "expected programme maximum header; got {:?}",
            content
        );
        assert!(
            content.contains("-3.5"),
            "expected latest interval true-peak bar value; got {:?}",
            content
        );
        assert!(
            content.contains("LUFS"),
            "expected LUFS section; got {:?}",
            content
        );
        assert!(
            content.contains("Max momentary -9.4") && content.contains("Max short-term -11.2"),
            "expected latched M/S maxima despite invalid current-window flags; got {:?}",
            content
        );
        assert!(
            content.contains("LRA: 0.0 LU") && content.contains("Not stable"),
            "valid zero LU must display as an unstable early LRA value; got {content:?}"
        );
        assert!(
            content.contains("Stereo width"),
            "expected Stereo width section; got {:?}",
            content
        );
    }

    fn rendered_lufs_content(app: &App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                draw_lufs_box(frame, area, app);
            })
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn lra_display_requires_valid_finite_range_and_localizes_instability() {
        let mut app = test_app_with_loudness();
        for (language, marker) in [
            (crate::i18n::Language::French, "Pas encore stable"),
            (crate::i18n::Language::German, "Noch nicht stabil"),
            (crate::i18n::Language::Spanish, "Aún no estable"),
        ] {
            app.ui.language = language;
            let content = rendered_lufs_content(&app, 24, 20);
            assert!(content.contains("LRA: 0.0 LU"), "{content:?}");
            assert!(content.contains(marker), "{content:?}");
        }

        {
            let loudness = app.playback.loudness_info.as_mut().unwrap();
            loudness.loudness_range.as_mut().unwrap().is_stable = true;
        }
        app.ui.language = crate::i18n::Language::German;
        let stable = rendered_lufs_content(&app, 24, 20);
        assert!(stable.contains("LRA: 0.0 LU"), "{stable:?}");
        assert!(!stable.contains("Noch nicht stabil"), "{stable:?}");

        {
            let loudness = app.playback.loudness_info.as_mut().unwrap();
            let range = loudness.loudness_range.as_mut().unwrap();
            range.range_lu = None;
            range.status = LoudnessRangeStatus::BelowGate;
        }
        let unavailable = rendered_lufs_content(&app, 24, 20);
        assert!(
            unavailable.contains("LRA: Nicht verfügbar"),
            "{unavailable:?}"
        );
        assert!(
            !unavailable.contains("Noch nicht stabil"),
            "{unavailable:?}"
        );

        {
            let loudness = app.playback.loudness_info.as_mut().unwrap();
            let range = loudness.loudness_range.as_mut().unwrap();
            range.range_lu = Some(f64::INFINITY);
            range.status = LoudnessRangeStatus::Valid;
            range.is_stable = false;
        }
        let malformed = rendered_lufs_content(&app, 24, 20);
        assert!(malformed.contains("LRA: Nicht verfügbar"), "{malformed:?}");
        assert!(!malformed.contains("Noch nicht stabil"), "{malformed:?}");
    }

    #[test]
    fn short_lufs_box_keeps_existing_maxima_bars_and_border_before_lra() {
        let app = test_app_with_loudness();
        let backend = TestBackend::new(24, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                draw_lufs_box(frame, area, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert!(content.contains("Max momentary -9.4"), "{content:?}");
        assert!(content.contains("Max short-term -11.2"), "{content:?}");
        assert!(
            content.contains("-10.5") && content.contains("-14.0"),
            "{content:?}"
        );
        assert!(
            !content.contains("Not stable"),
            "incomplete LRA rows must be omitted: {content:?}"
        );
        for x in 1..23 {
            assert_eq!(
                buffer[(x, 9)].symbol(),
                "─",
                "LRA summary must not overwrite the bottom border at x={x}"
            );
        }
    }

    #[test]
    fn lufs_maxima_and_current_bars_survive_short_terminal_heights() {
        for (language, momentary_label, shortterm_label) in [
            (crate::i18n::Language::French, "M max", "S max"),
            (crate::i18n::Language::German, "Max M", "Max S"),
        ] {
            for (width, height) in [(24, 12), (24, 10)] {
                let backend = TestBackend::new(width, height);
                let mut terminal = Terminal::new(backend).unwrap();
                let mut app = test_app_with_loudness();
                app.ui.language = language;
                terminal.draw(|f| draw_lufs_box(f, f.area(), &app)).unwrap();

                let buffer = terminal.backend().buffer();
                let content: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
                let max_m_label = format!("{momentary_label} -9.4");
                let max_s_label = format!("{shortterm_label} -11.2");
                assert!(
                    content.contains(&max_m_label),
                    "{language:?} {width}x{height}: {content:?}"
                );
                assert!(
                    content.contains(&max_s_label),
                    "{language:?} {width}x{height}: {content:?}"
                );
                assert!(content.contains("M -10.5"), "{width}x{height}: {content:?}");
                assert!(content.contains("S -12.0"), "{width}x{height}: {content:?}");
                assert!(content.contains("I -14.0"), "{width}x{height}: {content:?}");

                let row_text: Vec<String> = buffer
                    .content
                    .chunks(usize::from(width))
                    .map(|row| row.iter().map(|cell| cell.symbol()).collect())
                    .collect();
                let row_of = |fragment: &str| {
                    row_text
                        .iter()
                        .position(|row| row.contains(fragment))
                        .unwrap_or_else(|| {
                            panic!(
                                "{language:?} {width}x{height} missing {fragment:?}: {row_text:?}"
                            )
                        })
                };
                let max_m = row_of(&max_m_label);
                let max_s = row_of(&max_s_label);
                let current_m = row_of("M -10.5");
                let current_s = row_of("S -12.0");
                let current_i = row_of("I -14.0");
                assert!(
                    max_m < max_s
                        && max_s < current_m
                        && current_m < current_s
                        && current_s < current_i,
                    "summary rows must not overlap or displace current bars: {row_text:?}"
                );
                if height == 10 {
                    assert!(
                        !content.contains("-6.0"),
                        "second TP bar should be suppressed first at {width}x{height}: {content:?}"
                    );
                    assert!(
                        !content.contains("+6"),
                        "scale should be suppressed at {width}x{height}: {content:?}"
                    );
                } else {
                    assert!(
                        content.contains("-6.0"),
                        "second TP bar should remain at {width}x{height}: {content:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn undersized_lufs_box_keeps_its_border_intact() {
        let width = 24;
        let height = 8;
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = test_app_with_loudness();
        terminal.draw(|f| draw_lufs_box(f, f.area(), &app)).unwrap();

        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(0, 0)].symbol(), "┌");
        assert_eq!(buffer[(width - 1, 0)].symbol(), "┐");
        assert_eq!(buffer[(0, height - 1)].symbol(), "└");
        assert_eq!(buffer[(width - 1, height - 1)].symbol(), "┘");
        for x in 1..width - 1 {
            assert_eq!(
                buffer[(x, height - 1)].symbol(),
                "─",
                "bottom border interior at column {x}"
            );
        }
        for y in 1..height - 1 {
            assert_eq!(buffer[(0, y)].symbol(), "│", "left border at row {y}");
            assert_eq!(
                buffer[(width - 1, y)].symbol(),
                "│",
                "right border at row {y}"
            );
        }

        // The localized labels need two rows at this width. The summary
        // helper must leave the bottom border intact when both rows cannot fit.
        let width = 12;
        let height = 7;
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = test_app_with_loudness();
        app.ui.language = crate::i18n::Language::French;
        terminal.draw(|f| draw_lufs_box(f, f.area(), &app)).unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(0, height - 1)].symbol(), "└");
        assert_eq!(buffer[(width - 1, height - 1)].symbol(), "┘");
        for x in 1..width - 1 {
            assert_eq!(
                buffer[(x, height - 1)].symbol(),
                "─",
                "narrow stacked summary crossed bottom border at column {x}"
            );
        }
    }
}
