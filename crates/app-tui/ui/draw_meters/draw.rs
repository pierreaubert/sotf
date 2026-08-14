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

        if !loudness.true_peaks_dbtp.is_empty() && y_offset < inner.height {
            // Find max true peak for display
            let max_true_peak = loudness
                .true_peaks_dbtp
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);

            // Header: "True Peak      [XX.X]" — write into the
            // reused stack buffer to avoid a per-frame `format!`.
            label_buf.len = 0;
            if max_true_peak.is_finite() {
                let _ = write!(
                    &mut label_buf,
                    "{}      [{:>4.1}]",
                    i18n.ui("True Peak"),
                    max_true_peak
                );
            } else {
                let _ = write!(&mut label_buf, "{}        [-∞]", i18n.ui("True Peak"));
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

            // Render true peak bars for each channel (max 2 bars to save space)
            let num_peak_bars = loudness.true_peaks_dbtp.len().min(2);
            for ch_idx in 0..num_peak_bars {
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
            if y_offset < inner.height {
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
        if y_offset < inner.height {
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
    let title_lines = [Line::from(i18n.ui("Levels (help: ?)"))];
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
            true_peak_is_compliant: true,
            integrated_window_seconds: 3_600,
            correlation_lr: Some(0.8),
            correlation_matrix: Arc::new(Vec::new()),
            correlation_samples_seen: 0,
            ..Default::default()
        });
        app
    }

    /// Smoke / regression test: `draw_lufs_box` must render the loudness box
    /// without relying on per-frame `String` allocations for labels or scale
    /// strings. We verify that the expected sections are written into the
    /// terminal buffer.
    #[test]
    fn draw_lufs_box_renders_all_sections() {
        let backend = TestBackend::new(40, 20);
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
            content.contains("True Peak"),
            "expected True Peak header; got {:?}",
            content
        );
        assert!(
            content.contains("LUFS"),
            "expected LUFS section; got {:?}",
            content
        );
        assert!(
            content.contains("Stereo width"),
            "expected Stereo width section; got {:?}",
            content
        );
    }
}
