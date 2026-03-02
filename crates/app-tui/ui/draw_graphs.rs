use super::*;

pub(crate) fn draw_loss_chart(
    f: &mut Frame,
    area: Rect,
    app: &App,
    history: &[(usize, f64, Option<f64>)],
) {
    if history.len() < 2 {
        return;
    }

    let x_bound = history.last().map(|h| h.0).unwrap_or(1) as f64;
    let min_loss = history.iter().map(|h| h.1).fold(f64::INFINITY, f64::min);
    let max_loss = history
        .iter()
        .map(|h| h.1)
        .fold(f64::NEG_INFINITY, f64::max);
    let margin = (max_loss - min_loss).abs() * 0.05;
    let y_lo = min_loss - margin;
    let y_hi = max_loss + margin;

    // Downsample to at most 200 points for chart performance
    let ds_step = (history.len() / 200).max(1);
    let loss_data: Vec<(f64, f64)> = history
        .iter()
        .step_by(ds_step)
        .map(|(iter, loss, _)| (*iter as f64, *loss))
        .collect();

    let datasets = vec![
        Dataset::default()
            .name("Loss")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(app.theme.accent_primary))
            .data(&loss_data),
    ];

    let x_labels = vec![
        Span::raw("0"),
        Span::raw(format!("{:.0}", x_bound / 2.0)),
        Span::raw(format!("{:.0}", x_bound)),
    ];
    let y_labels = vec![
        Span::raw(format!("{:.4}", y_lo)),
        Span::raw(format!("{:.4}", (y_lo + y_hi) / 2.0)),
        Span::raw(format!("{:.4}", y_hi)),
    ];

    let title = format!("Loss History  ({} iterations)", x_bound as usize);

    let chart = Chart::new(datasets)
        .style(
            Style::default()
                .fg(app.theme.fg_primary)
                .bg(app.theme.bg_secondary),
        )
        .block(Block::default().borders(Borders::ALL).title(title))
        .x_axis(
            Axis::default()
                .title("Iteration")
                .style(Style::default().fg(app.theme.fg_secondary))
                .labels(x_labels)
                .bounds([0.0, x_bound]),
        )
        .y_axis(
            Axis::default()
                .title("Loss")
                .style(Style::default().fg(app.theme.fg_secondary))
                .labels(y_labels)
                .bounds([y_lo, y_hi]),
        );

    f.render_widget(chart, area);
}

pub(crate) fn draw_freq_response_chart(
    f: &mut Frame,
    area: Rect,
    app: &App,
    s: &crate::app::SpinoramaEqTuiState,
) {
    if s.curve_frequencies.len() < 2 {
        let placeholder = Paragraph::new("No curve data")
            .style(Style::default().fg(app.theme.fg_secondary))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Frequency Response"),
            );
        f.render_widget(placeholder, area);
        return;
    }

    let freqs = &s.curve_frequencies;
    let n = freqs.len();

    // Downsample to at most 300 points
    let step = (n / 300).max(1);

    let input_data: Vec<(f64, f64)> = freqs
        .iter()
        .zip(s.curve_input.iter())
        .step_by(step)
        .map(|(f, v)| (*f, *v))
        .collect();

    let corrected_data: Vec<(f64, f64)> = freqs
        .iter()
        .zip(s.curve_corrected.iter())
        .step_by(step)
        .map(|(f, v)| (*f, *v))
        .collect();

    let filter_data: Vec<(f64, f64)> = freqs
        .iter()
        .zip(s.curve_filter_response.iter())
        .step_by(step)
        .map(|(f, v)| (*f, *v))
        .collect();

    // Compute y bounds from filter response SPL for appropriate zoom
    let y_min = s
        .curve_filter_response
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let y_max = s
        .curve_filter_response
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let y_bound_lo = y_min.floor();
    let y_bound_hi = y_max.ceil();

    let x_min = freqs.first().copied().unwrap_or(20.0);
    let x_max = freqs.last().copied().unwrap_or(20000.0);

    let datasets = vec![
        Dataset::default()
            .name("Input")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Gray))
            .data(&input_data),
        Dataset::default()
            .name("Corrected")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(app.theme.accent_success))
            .data(&corrected_data),
        Dataset::default()
            .name("Filter")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(app.theme.accent_primary))
            .data(&filter_data),
    ];

    let x_labels = vec![
        Span::raw(format!("{:.0}", x_min)),
        Span::raw("1k"),
        Span::raw("5k"),
        Span::raw(format!("{:.0}", x_max)),
    ];
    let y_labels = vec![
        Span::raw(format!("{:.0}", y_bound_lo)),
        Span::raw(format!("{:.0}", (y_bound_lo + y_bound_hi) / 2.0)),
        Span::raw(format!("{:.0}", y_bound_hi)),
    ];

    let chart = Chart::new(datasets)
        .style(
            Style::default()
                .fg(app.theme.fg_primary)
                .bg(app.theme.bg_secondary),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Frequency Response (Gray=Input  Green=Corrected  Blue=Filter)"),
        )
        .x_axis(
            Axis::default()
                .title("Hz")
                .style(Style::default().fg(app.theme.fg_secondary))
                .labels(x_labels)
                .bounds([x_min, x_max]),
        )
        .y_axis(
            Axis::default()
                .title("dB")
                .style(Style::default().fg(app.theme.fg_secondary))
                .labels(y_labels)
                .bounds([y_bound_lo, y_bound_hi]),
        );

    f.render_widget(chart, area);
}
