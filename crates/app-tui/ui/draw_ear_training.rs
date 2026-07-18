use super::*;
use crate::app::{EarTrainingTab, Screen};
use sotf_audio_player::{EarTrainingCourse, EqTrainingSession};

pub(crate) fn draw_ear_training_screen(f: &mut Frame, area: Rect, app: &App) {
    let rows = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(area);
    draw_help_box(f, rows[0], app, Screen::EarTraining);
    match app.ui.ear_training.tab {
        EarTrainingTab::Practice => draw_practice(f, rows[1], app),
        EarTrainingTab::Courses => draw_courses(f, rows[1], app),
        EarTrainingTab::Progress => draw_training_progress(f, rows[1], app),
    }
}

fn draw_practice(f: &mut Frame, area: Rect, app: &App) {
    let panels = if area.width >= 76 {
        Layout::horizontal([Constraint::Percentage(38), Constraint::Percentage(62)]).split(area)
    } else {
        Layout::vertical([Constraint::Percentage(43), Constraint::Percentage(57)]).split(area)
    };
    draw_practice_setup(f, panels[0], app);
    draw_practice_question(f, panels[1], app);
}

fn draw_practice_setup(f: &mut Frame, area: Rect, app: &App) {
    let training = &app.ui.ear_training;
    let source = training
        .sources
        .get(training.source_index)
        .and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .or_else(|| {
            app.current_track_path().and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
        })
        .unwrap_or_else(|| crate::tui_text!(app, "No track selected"));
    let loop_text = training.loop_range.map_or_else(
        || crate::tui_text!(app, "not set"),
        |(start, end)| {
            format!(
                "{start:.1}–{end:.1}s ({})",
                if training.loop_enabled { "on" } else { "off" }
            )
        },
    );
    let lines = vec![
        Line::from(vec![
            Span::styled(
                crate::tui_text!(app, "Exercise: "),
                Style::default().fg(app.theme.fg_secondary),
            ),
            Span::raw(crate::tui_text!(app, training.config.exercise.label())),
        ]),
        Line::from(format!(
            "{} {}",
            crate::tui_text!(app, "Bands:"),
            training.config.band_count
        )),
        Line::from(format!(
            "{} {:+.0} dB",
            crate::tui_text!(app, "Gain:"),
            training.config.gain_db
        )),
        Line::from(format!("Q: {:.1}", training.config.q)),
        Line::from(format!(
            "{} {}",
            crate::tui_text!(app, "Trials:"),
            training.config.trial_count
        )),
        Line::from(format!(
            "{} {}",
            crate::tui_text!(app, "Change:"),
            crate::tui_text!(app, training.config.change_mode.label())
        )),
        Line::from(format!(
            "{} {}",
            crate::tui_text!(app, "Adaptive:"),
            if training.adaptive { "on" } else { "off" }
        )),
        Line::from(""),
        Line::from(format!("{} {source}", crate::tui_text!(app, "Source:"))),
        Line::from(format!("{} {loop_text}", crate::tui_text!(app, "Loop:"))),
        Line::from(""),
        Line::from(crate::tui_text!(app, "e exercise · a adaptive · c change")),
        Line::from(crate::tui_text!(
            app,
            "b/B bands · g/G gain · v/V Q · t/T trials"
        )),
        Line::from(crate::tui_text!(
            app,
            "i add source · ,/. source · [/] loop · \\ toggle"
        )),
    ];
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }).block(
            Block::default()
                .borders(Borders::ALL)
                .title(crate::tui_text!(app, "Practice setup")),
        ),
        area,
    );
}

fn draw_practice_question(f: &mut Frame, area: Rect, app: &App) {
    let session = app.ui.ear_training.session.as_ref();
    let chart_height = area.height.saturating_sub(9).max(5);
    let rows = Layout::vertical([
        Constraint::Length(chart_height),
        Constraint::Min(4),
        Constraint::Length(3),
    ])
    .split(area);
    draw_question_chart(f, rows[0], app, session);
    draw_answers(f, rows[1], app, session);
    let score = session.map_or_else(
        || crate::tui_text!(app, "Press s to start"),
        |session| {
            format!(
                "{}/{} · {:.0}%",
                session.correct_count(),
                session.trials.len(),
                session.accuracy() * 100.0
            )
        },
    );
    let controls = format!("{score}   1 original · 2 filtered · ←/→ answer · Enter submit/next");
    f.render_widget(
        Paragraph::new(crate::tui_text!(app, controls))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL)),
        rows[2],
    );
}

fn draw_question_chart(f: &mut Frame, area: Rect, app: &App, session: Option<&EqTrainingSession>) {
    let data: Vec<(f64, f64)> = session
        .and_then(|session| session.current_question.as_ref())
        .map(|question| {
            question
                .preview_curve(100)
                .into_iter()
                .map(|(frequency, gain)| (frequency.log10(), gain))
                .collect()
        })
        .unwrap_or_else(|| vec![(20.0_f64.log10(), 0.0), (20_000.0_f64.log10(), 0.0)]);
    let datasets = vec![
        Dataset::default()
            .name(crate::tui_text!(app, "EQ change"))
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(app.theme.accent_primary))
            .data(&data),
    ];
    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(crate::tui_text!(app, "Listen before answering")),
        )
        .x_axis(
            Axis::default()
                .bounds([20.0_f64.log10(), 20_000.0_f64.log10()])
                .labels(vec![Span::raw("20"), Span::raw("1k"), Span::raw("20k Hz")]),
        )
        .y_axis(Axis::default().bounds([-16.0, 16.0]).labels(vec![
            Span::raw("-16"),
            Span::raw("0"),
            Span::raw("+16 dB"),
        ]))
        .style(
            Style::default()
                .fg(app.theme.fg_primary)
                .bg(app.theme.bg_secondary),
        );
    f.render_widget(chart, area);
}

fn draw_answers(f: &mut Frame, area: Rect, app: &App, session: Option<&EqTrainingSession>) {
    let Some(session) = session else {
        f.render_widget(
            Paragraph::new(crate::tui_text!(
                app,
                "Start a session to reveal answer choices"
            ))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL)),
            area,
        );
        return;
    };
    let labels = session
        .current_question
        .as_ref()
        .map(|question| question.answer_labels(session.config.exercise, &session.band_frequencies))
        .unwrap_or_default();
    let answered = session.current_is_answered();
    let correct = session
        .current_question
        .as_ref()
        .map(|question| question.correct_answer(session.config.exercise));
    let spans = labels
        .into_iter()
        .enumerate()
        .flat_map(|(index, label)| {
            let selected = index == app.ui.ear_training.selected_answer;
            let is_correct = answered && correct == Some(index);
            let style = if is_correct {
                Style::default()
                    .fg(app.theme.playing_indicator)
                    .add_modifier(Modifier::BOLD)
            } else if selected {
                Style::default()
                    .fg(app.theme.border_color)
                    .add_modifier(Modifier::REVERSED)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };
            [Span::styled(format!(" {label} "), style), Span::raw(" ")]
        })
        .collect::<Vec<_>>();
    f.render_widget(
        Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(crate::tui_text!(app, "Your answer")),
            ),
        area,
    );
}

fn draw_courses(f: &mut Frame, area: Rect, app: &App) {
    let items = EarTrainingCourse::ALL
        .iter()
        .enumerate()
        .map(|(index, course)| {
            let config = course.config();
            let completed = app
                .ui
                .ear_training
                .progress
                .sessions
                .iter()
                .filter(|session| session.course == Some(*course))
                .count();
            let style = if index == app.ui.ear_training.course_selection {
                Style::default()
                    .fg(app.theme.border_color)
                    .add_modifier(Modifier::REVERSED | Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };
            ListItem::new(format!(
                "{:<20} {:>2} bands  {:+.0} dB  {:>2} trials  · {} complete",
                crate::tui_text!(app, course.label()),
                config.band_count,
                config.gain_db,
                config.trial_count,
                completed
            ))
            .style(style)
        })
        .collect::<Vec<_>>();
    f.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(crate::tui_text!(
                    app,
                    "Guided courses · ↑/↓ select · Enter start"
                )),
        ),
        area,
    );
}

fn draw_training_progress(f: &mut Frame, area: Rect, app: &App) {
    let progress = &app.ui.ear_training.progress;
    let rows = Layout::vertical([Constraint::Length(7), Constraint::Min(0)]).split(area);
    let summary = vec![
        Line::from(format!(
            "{} {}",
            crate::tui_text!(app, "Sessions:"),
            progress.sessions.len()
        )),
        Line::from(format!(
            "{} {:.0}%",
            crate::tui_text!(app, "Accuracy:"),
            progress.accuracy() * 100.0
        )),
        Line::from(format!(
            "{} {}",
            crate::tui_text!(app, "70% streak:"),
            progress.streak()
        )),
        Line::from(format!(
            "{} {}",
            crate::tui_text!(app, "Coach:"),
            crate::tui_text!(app, progress.recommendation())
        )),
    ];
    f.render_widget(
        Paragraph::new(summary).wrap(Wrap { trim: true }).block(
            Block::default()
                .borders(Borders::ALL)
                .title(crate::tui_text!(app, "Progress and coaching")),
        ),
        rows[0],
    );
    let recent = progress
        .sessions
        .iter()
        .rev()
        .take(rows[1].height.saturating_sub(2) as usize)
        .map(|session| {
            ListItem::new(format!(
                "{:<18} {}/{}  {:.0}%",
                crate::tui_text!(app, session.exercise.label()),
                session.correct,
                session.attempts,
                session.accuracy * 100.0
            ))
        })
        .collect::<Vec<_>>();
    f.render_widget(
        List::new(recent).block(
            Block::default()
                .borders(Borders::ALL)
                .title(crate::tui_text!(app, "Recent sessions")),
        ),
        rows[1],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use ratatui::{Terminal, backend::TestBackend};

    fn rendered(
        tab: EarTrainingTab,
        language: crate::i18n::Language,
        width: u16,
        height: u16,
    ) -> String {
        let mut app = App::new(Theme::default(), false);
        app.ui.ear_training.tab = tab;
        app.ui.language = language;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| draw_ear_training_screen(frame, frame.area(), &app))
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
    fn all_suite_tabs_render_at_desktop_terminal_size() {
        assert!(
            rendered(
                EarTrainingTab::Practice,
                crate::i18n::Language::English,
                140,
                45
            )
            .contains("Practice setup")
        );
        assert!(
            rendered(
                EarTrainingTab::Courses,
                crate::i18n::Language::English,
                140,
                45
            )
            .contains("Guided courses")
        );
        assert!(
            rendered(
                EarTrainingTab::Progress,
                crate::i18n::Language::English,
                140,
                45
            )
            .contains("Progress and coaching")
        );
    }

    #[test]
    fn practice_render_is_responsive_at_small_terminal_size() {
        let content = rendered(
            EarTrainingTab::Practice,
            crate::i18n::Language::English,
            70,
            28,
        );
        assert!(content.contains("Practice setup"));
        assert!(content.contains("Press s to start"));
    }

    #[test]
    fn all_suite_tabs_render_in_every_supported_language() {
        for language in crate::i18n::Language::ALL {
            for tab in [
                EarTrainingTab::Practice,
                EarTrainingTab::Courses,
                EarTrainingTab::Progress,
            ] {
                let _ = rendered(tab, language, 140, 45);
            }
        }
    }
}
