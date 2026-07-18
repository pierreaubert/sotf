//! Recording screen module
//!
//! Multi-channel audio recording workflow with six steps:
//! 1. Config - Device selection and channel mapping
//! 2. Capture - Record frequency response for each channel
//! 3. Probe - Tone-burst arrival-time probe per channel
//! 4. BassAnchor - Low-frequency tone burst for first-bin phase anchor
//!    (GD-Opt v2; `docs/gd_opt_v2_plan.md` §2.6)
//! 5. Evaluating - View and analyze frequency response graphs
//! 6. Saving - Save recordings and configuration to disk

mod bass_anchor;
mod capture;
mod config;
mod evaluating;
mod probe;
mod saving;
mod spl_calibration;

use crate::app::types::{RecordingStep, Screen};
use crate::components::icons::{Icon, IconName};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonTheme, ButtonVariant, HStack, StackSpacing, StepStatus, WizardHeader,
    WizardStep, WizardTheme,
};

impl PlayerView {
    /// Main recording screen renderer - dispatches to the appropriate step
    pub(crate) fn render_recording_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = crate::components::design::Ds::from_cx(cx);
        let (theme, current_step) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.measurement_state.recording_state.step,
            )
        };

        let step_content = match current_step {
            RecordingStep::Config => self.render_recording_config_step(cx).into_any_element(),
            RecordingStep::SplCalibration => self
                .render_recording_spl_calibration_step(cx)
                .into_any_element(),
            RecordingStep::Capture => self.render_recording_capture_step(cx).into_any_element(),
            RecordingStep::Probe => self.render_recording_probe_step(cx).into_any_element(),
            RecordingStep::BassAnchor => self
                .render_recording_bass_anchor_step(cx)
                .into_any_element(),
            RecordingStep::Evaluating => {
                self.render_recording_evaluating_step(cx).into_any_element()
            }
            RecordingStep::Saving => self.render_recording_saving_step(cx).into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .bg(theme.background)
            .child(self.render_recording_header(cx))
            .child(
                div()
                    .id("recording-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(d.card)
                    .child(step_content),
            )
    }

    /// Render the recording screen header with step indicators
    fn render_recording_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = crate::components::design::Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let title = state.app.ui_state.translations.screen_recording;
        let theme_id = state.app.ui_state.theme_id;
        let current_step = state.app.measurement_state.recording_state.step;
        let is_recording = state.app.measurement_state.recording_state.is_recording();

        // Build wizard steps from `RecordingStep::all()` so new
        // variants (e.g. `Probe`) show up automatically and can't be
        // silently skipped — see the Room EQ wizard header bug for
        // why hand-rolling this list is dangerous.
        let all_steps = RecordingStep::all();
        let step_index = all_steps
            .iter()
            .position(|s| *s == current_step)
            .unwrap_or(0);

        let step_statuses: Vec<StepStatus> = all_steps
            .iter()
            .enumerate()
            .map(|(i, _)| {
                if i < step_index {
                    StepStatus::Completed
                } else if i == step_index {
                    StepStatus::Active
                } else {
                    StepStatus::NotVisited
                }
            })
            .collect();

        let steps: Vec<WizardStep> = all_steps
            .iter()
            .map(|s| {
                let id = match s {
                    RecordingStep::Config => "config",
                    RecordingStep::SplCalibration => "spl_calibration",
                    RecordingStep::Capture => "capture",
                    RecordingStep::Probe => "probe",
                    RecordingStep::BassAnchor => "bass_anchor",
                    RecordingStep::Evaluating => "evaluating",
                    RecordingStep::Saving => "saving",
                };
                WizardStep::new(id, s.label())
            })
            .collect();

        let ui_kit_theme = theme.to_ui_kit_theme(theme_id, cx);
        let wizard_theme = WizardTheme::from(&ui_kit_theme);
        let button_theme = ButtonTheme::from(&ui_kit_theme);

        let next_disabled = !state.app.can_advance_workflow_step();

        let header = WizardHeader::new()
            .title(title)
            .steps(steps)
            .step_statuses(step_statuses)
            .current_step(step_index)
            .theme(wizard_theme.clone());

        let back_label = match current_step {
            RecordingStep::Config => "Close",
            _ => "Back",
        };
        let next_label =
            crate::components::wizard_continue_label(current_step.next().map(|next| next.label()));

        let navigation = HStack::new()
            .spacing(StackSpacing::Sm)
            .child(
                Button::new("back", back_label)
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Sm)
                    .disabled(is_recording)
                    .theme(button_theme.clone())
                    .build()
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|view, _, _, cx| {
                            view.state.update(cx, |state, _| {
                                state.app.move_workflow_step(false);
                            });
                            cx.notify();
                        }),
                    ),
            )
            .child(
                Button::new("next", next_label)
                    .variant(ButtonVariant::Primary)
                    .size(ButtonSize::Sm)
                    .disabled(next_disabled)
                    .theme(button_theme.clone())
                    .on_click_event(cx.listener(|view, _, _, cx| {
                        view.state.update(cx, |state, _| {
                            state.app.move_workflow_step(true);
                        });
                        cx.notify();
                    })),
            );
        let navigation = navigation.build().flex_none();

        // Home button for navigation back to Library
        let state_for_home = self.state.clone();
        let text_muted = theme.text_muted;
        let surface_hover = theme.surface_hover;

        div()
            .flex()
            .items_center()
            .justify_between()
            .min_w_0()
            .px(d.card)
            .py(d.card)
            .bg(theme.background_secondary)
            .border_b_1()
            .border_color(theme.border)
            // Home button on the left
            .child(
                div()
                    .id("recording-home-button")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(rems(2.5))
                    .h(rems(2.0))
                    .cursor_pointer()
                    .rounded(d.r_md)
                    .hover(move |s| s.bg(surface_hover))
                    .child(Icon::new(IconName::Home).color(text_muted))
                    .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                        state_for_home.update(cx, |state, _cx| {
                            state.app.ui_state.current_screen = Screen::Library;
                        });
                    }),
            )
            // Centered header with flex-1
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .justify_center()
                    .child(header),
            )
            // Navigation buttons on the right
            .child(navigation)
    }
}
