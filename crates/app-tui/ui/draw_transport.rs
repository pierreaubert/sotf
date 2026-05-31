use super::*;

pub(crate) fn draw_transport(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.border_color))
        .title(" Transport ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 || inner.width < 20 {
        return;
    }

    // Get current track waveform and duration
    let (waveform, duration, position) = if let Some(idx) = app.current_queue_index {
        if let Some(entry) = app.queue.get(idx) {
            if let Some(track) = entry.item.current_track() {
                let waveform = track.waveform.as_deref().map(|samples| &samples[..]);
                (waveform, track.duration_secs, app.position_secs)
            } else {
                (None, None, 0.0)
            }
        } else {
            (None, None, 0.0)
        }
    } else {
        (None, None, 0.0)
    };

    // Transport buttons
    let buttons = if app.is_playing {
        "\u{23EE}  \u{23F8}  \u{23ED}"
    } else {
        "\u{23EE}  \u{25B6}  \u{23ED}"
    };
    let button_style = if app.is_playing {
        Style::default().fg(app.theme.playing_indicator)
    } else {
        Style::default().fg(app.theme.fg_primary)
    };

    // Time display
    let time_str = if let Some(dur) = duration {
        let pos = position as u64;
        format!(
            " {:>2}:{:02}/{:>2}:{:02} ",
            pos / 60,
            pos % 60,
            dur / 60,
            dur % 60,
        )
    } else {
        "  --:--/--:-- ".to_string()
    };

    // Waveform: fill remaining space between buttons and time
    let buttons_width = 11_u16; // "⏮  ▶  ⏭" display width
    let time_width = time_str.len() as u16;
    let padding = 2_u16; // spaces around waveform
    let waveform_width = inner
        .width
        .saturating_sub(buttons_width + time_width + padding) as usize;

    let block_chars: [char; 9] = [
        ' ', '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}',
        '\u{2588}',
    ];

    let progress_ratio = if let Some(dur) = duration {
        if dur > 0 {
            (position / dur as f64).clamp(0.0, 1.0)
        } else {
            0.0
        }
    } else {
        0.0
    };

    let mut spans = vec![Span::styled(format!("{} ", buttons), button_style)];

    if waveform_width > 0 {
        if let Some(wf) = waveform {
            // Downsample 128 samples to waveform_width chars
            let played_chars = (progress_ratio * waveform_width as f64).round() as usize;

            for i in 0..waveform_width {
                // Map display position to waveform sample range
                let start = i * wf.len() / waveform_width;
                let end = ((i + 1) * wf.len() / waveform_width).min(wf.len());
                let avg = if end > start {
                    wf[start..end].iter().map(|&s| s as u32).sum::<u32>() / (end - start) as u32
                } else {
                    0
                };
                // Map 0-255 to 0-8 block index
                let idx = (avg * 8 / 255).min(8) as usize;
                let ch = block_chars[idx];
                let color = if i < played_chars {
                    app.theme.playing_indicator
                } else {
                    app.theme.fg_muted
                };
                spans.push(Span::styled(ch.to_string(), Style::default().fg(color)));
            }
        } else {
            // No waveform data — show empty bar
            let bar: String = std::iter::repeat_n(' ', waveform_width).collect();
            spans.push(Span::styled(bar, Style::default().fg(app.theme.fg_muted)));
        }
    }

    spans.push(Span::styled(
        time_str,
        Style::default().fg(app.theme.fg_primary),
    ));

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    f.render_widget(paragraph, inner);
}
