//! Simultaneous capture plan review and per-take QA display.

use crate::app::App;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub(super) fn draw_capture(frame: &mut Frame, area: Rect, app: &App) {
    let panel = &app.recording.multi_capture;
    let regions = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(4),
            Constraint::Min(1),
        ])
        .split(area);
    let fields = [
        ("Plan JSON", &panel.plan_path),
        ("Raw dir", &panel.raw_directory),
        ("Processed dir", &panel.processed_directory),
    ];
    let mut lines = vec![
        Line::from("↑↓/Tab select · Enter edit · V load/review · R record · C cancel"),
        Line::from("P process raw session · I import RoomEQ · PgUp/PgDn review · Esc back"),
    ];
    for (index, (label, value)) in fields.iter().enumerate() {
        let selected = index == panel.selected_field;
        let available = area.width.saturating_sub(label.len() as u16 + 8) as usize;
        let visible: String = value
            .chars()
            .rev()
            .take(available)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        lines.push(Line::from(Span::styled(
            format!(
                "{} {label}: {visible}{}",
                if selected { ">" } else { " " },
                if selected && panel.editing { "▏" } else { "" }
            ),
            Style::default().fg(if selected {
                app.theme.accent_primary
            } else {
                app.theme.fg_primary
            }),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .style(
                Style::default()
                    .fg(app.theme.fg_primary)
                    .bg(app.theme.bg_primary),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Multi-mic RoomEQ capture — 2–4 inputs"),
            ),
        regions[0],
    );
    frame.render_widget(
        Paragraph::new(if panel.workflow.message.is_empty() {
            "Recording requires a new raw directory. Processing reads a saved raw session and requires a new processed directory."
        } else { panel.workflow.message.as_str() })
            .style(Style::default().fg(app.theme.fg_primary).bg(app.theme.bg_primary))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("{:?}", panel.workflow.stage)),
            )
            .wrap(Wrap { trim: false }),
        regions[1],
    );
    let mut review: Vec<Line<'_>> = panel
        .workflow
        .review_lines
        .iter()
        .map(|line| Line::from(line.as_str()))
        .collect();
    if !panel.workflow.qa_lines.is_empty() {
        review.push(Line::from("── Capture QA ──"));
    }
    review.extend(
        panel
            .workflow
            .qa_lines
            .iter()
            .map(|line| Line::from(line.as_str())),
    );
    if let Some(path) = &panel.workflow.recording_manifest {
        review.push(Line::from(format!("RoomEQ manifest: {}", path.display())));
    }
    if review.is_empty() {
        review.push(Line::from("Load a session plan to review geometry, device routes, per-mic calibration, and fixed gains. You can also process a saved raw session."));
    }
    frame.render_widget(
        Paragraph::new(
            review
                .into_iter()
                .skip(panel.scroll as usize)
                .collect::<Vec<_>>(),
        )
        .style(
            Style::default()
                .fg(app.theme.fg_primary)
                .bg(app.theme.bg_primary),
        )
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Plan and evidence (processing does not imply coherent eligibility)"),
        ),
        regions[2],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn multi_capture_panel_renders_on_small_and_regular_terminals() {
        let mut app = crate::events::tests::make_app();
        app.recording.multi_capture.active = true;
        app.recording.multi_capture.workflow.message = "Raw takes saved; QA pending".into();
        app.recording.multi_capture.workflow.qa_lines =
            vec!["Magnitude-only: invalid timing markers".into()];
        for (width, height) in [(40, 12), (80, 24), (120, 40)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| draw_capture(frame, frame.area(), &app))
                .unwrap();
            let buffer = terminal.backend().buffer();
            let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
            assert!(text.contains("Multi-mic RoomEQ"));
            if height >= 24 {
                assert!(text.contains("QA pending"));
                assert!(text.contains("Magnitude-only"));
            }
        }
    }
}
