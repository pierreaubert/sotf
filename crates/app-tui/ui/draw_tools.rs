use super::*;
use crate::app::{AbTestingStep, Tool};
use sotf_audio_player::{AbTestPhase, TrialMode};

pub(crate) fn draw_tools_screen(f: &mut Frame, area: Rect, app: &App) {
    let i18n = crate::i18n::TuiTranslations::for_language(app.ui.language);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let help = i18n.dynamic("↑↓=Navigate  Enter=Open  Esc=Back".to_string());
    draw_help_box_with_text(f, chunks[0], app, &help);

    let options = [
        (
            Tool::EarTraining,
            "1",
            "Ear Training  – Develop frequency and EQ recognition",
        ),
        (
            Tool::AbTesting,
            "2",
            "A/B Testing   – Compare two processing paths",
        ),
    ];

    let items = options
        .iter()
        .map(|(tool, key, label)| {
            let is_selected = *tool == app.ui.selected_tool;
            let style = if is_selected {
                Style::default()
                    .fg(app.theme.bg_primary)
                    .bg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" [{}] ", key),
                    if is_selected {
                        style.add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(app.theme.accent_primary)
                    },
                ),
                Span::styled(i18n.dynamic((*label).to_string()), style),
            ]))
        })
        .collect::<Vec<_>>();

    let selected = options
        .iter()
        .position(|(tool, _, _)| *tool == app.ui.selected_tool);
    let mut state = ListState::default();
    state.select(selected);

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(app.theme.accent_primary))
                .title(i18n.dynamic(" Tools – select a tool ".to_string())),
        )
        .highlight_style(
            Style::default()
                .fg(app.theme.bg_primary)
                .bg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        );
    f.render_stateful_widget(list, chunks[1], &mut state);
}

pub(crate) fn draw_ab_testing_screen(f: &mut Frame, area: Rect, app: &App) {
    let i18n = crate::i18n::TuiTranslations::for_language(app.ui.language);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    let help = i18n.dynamic(
        "A/B Testing · a/b=Capture  p=Prepare  Tab=Mode  Enter/n=New trial  q/w/e=Cue  1/2=Answer  s/l=Save/Load  Esc=Back"
            .to_string(),
    );
    draw_help_box_with_text(f, chunks[0], app, &help);

    let steps = [
        ("Setup", AbTestingStep::Setup),
        ("Trial", AbTestingStep::Trial),
        ("Results", AbTestingStep::Results),
    ];
    let tabs = Tabs::new(
        steps
            .iter()
            .map(|(label, _)| Line::from(i18n.dynamic((*label).to_string())))
            .collect::<Vec<_>>(),
    )
    .select(
        steps
            .iter()
            .position(|(_, step)| *step == app.ui.ab_testing.step)
            .unwrap_or(0),
    )
    .highlight_style(
        Style::default()
            .fg(app.theme.accent_primary)
            .add_modifier(Modifier::BOLD),
    )
    .divider("│")
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border_color)),
    );
    f.render_widget(tabs, chunks[1]);

    let state = &app.ui.ab_testing;
    let view = state.controller.view();
    let body = match state.step {
        AbTestingStep::Setup => format!(
            "Compare two processing paths using the hidden A/B Compare runtime.\n\n\
             Path A: {}\nPath B: {}\nMode: {}\nSession: {}\n\n\
             Capture the current plugin graph as A, change the graph, capture B, then load a local track and press p.",
            ready_label(state.path_a.is_some()),
            ready_label(state.path_b.is_some()),
            mode_label(state.trial_mode),
            match view.phase {
                AbTestPhase::Setup => "Not prepared",
                AbTestPhase::Ready => "Ready",
                AbTestPhase::Trial(_) => "Trial active",
            }
        ),
        AbTestingStep::Trial => format!(
            "{} trial\n\nAudition: {}\nAnswer: {}\nConfidence: {}%\n\n\
             Cue assignments remain concealed until the answer is committed.",
            mode_label(state.trial_mode),
            view.available_cues
                .iter()
                .enumerate()
                .map(|(index, cue)| format!("{}={}", ["q", "w", "e"][index], cue_label(*cue)))
                .collect::<Vec<_>>()
                .join("  "),
            view.available_answers
                .iter()
                .enumerate()
                .map(|(index, answer)| format!("{}={}", index + 1, answer_label(*answer)))
                .collect::<Vec<_>>()
                .join("  "),
            state.confidence,
        ),
        AbTestingStep::Results => format!(
            "Completed trials: {}\nABX score: {}/{}\n\n\
             Press n for another {} trial, s to save, or l to load the last TUI session.",
            view.completed_trials,
            view.abx_score.0,
            view.abx_score.1,
            mode_label(state.trial_mode),
        ),
    };
    let content = Paragraph::new(i18n.dynamic(body))
        .style(Style::default().fg(app.theme.fg_primary))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(app.theme.border_color))
                .title(i18n.dynamic(" A/B Testing ".to_string())),
        );
    f.render_widget(content, chunks[2]);

    let status = Paragraph::new(i18n.dynamic(state.status.clone()))
        .style(Style::default().fg(app.theme.fg_secondary))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.theme.border_color))
                .title(" Status "),
        );
    f.render_widget(status, chunks[3]);
}

fn ready_label(ready: bool) -> &'static str {
    if ready { "Captured" } else { "Missing" }
}

fn mode_label(mode: TrialMode) -> &'static str {
    match mode {
        TrialMode::BlindAb => "Blind A/B",
        TrialMode::Abx => "ABX",
    }
}

fn cue_label(cue: sotf_audio_player::TrialCue) -> &'static str {
    use sotf_audio_player::TrialCue;
    match cue {
        TrialCue::First => "First",
        TrialCue::Second => "Second",
        TrialCue::ReferenceA => "A",
        TrialCue::ReferenceB => "B",
        TrialCue::Unknown => "X",
    }
}

fn answer_label(answer: sotf_audio_player::TrialAnswer) -> &'static str {
    use sotf_audio_player::TrialAnswer;
    match answer {
        TrialAnswer::First => "First",
        TrialAnswer::Second => "Second",
        TrialAnswer::A => "A",
        TrialAnswer::B => "B",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use ratatui::{Terminal, backend::TestBackend};

    fn rendered(draw: fn(&mut Frame, Rect, &App)) -> String {
        let app = App::new(Theme::default(), false);
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal
            .draw(|frame| draw(frame, frame.area(), &app))
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
    fn tools_page_lists_initial_tools() {
        let content = rendered(draw_tools_screen);
        assert!(content.contains("Ear Training"));
        assert!(content.contains("A/B Testing"));
    }

    #[test]
    fn ab_testing_setup_page_renders() {
        let content = rendered(draw_ab_testing_screen);
        assert!(content.contains("A/B Testing"));
        assert!(content.contains("Setup"));
        assert!(content.contains("Path A"));
        assert!(content.contains("Path B"));
    }
}
