//! Recording screen module
//!
//! Multi-channel audio recording workflow with four steps:
//! 1. Config - Device selection and channel mapping
//! 2. Capture - Record frequency response for each channel
//! 3. Evaluating - View and analyze frequency response graphs
//! 4. Saving - Save recordings and configuration to disk

mod capture;
mod config;
mod evaluating;
mod saving;

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
        let (theme, current_step) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.measurement_state.recording_state.step,
            )
        };

        let step_content = match current_step {
            RecordingStep::Config => self.render_recording_config_step(cx).into_any_element(),
            RecordingStep::Capture => self.render_recording_capture_step(cx).into_any_element(),
            RecordingStep::Evaluating => {
                self.render_recording_evaluating_step(cx).into_any_element()
            }
            RecordingStep::Saving => self.render_recording_saving_step(cx).into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .child(self.render_recording_header(cx))
            .child(
                div()
                    .id("recording-content")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_4()
                    .child(step_content),
            )
    }

    /// Render the recording screen header with step indicators
    fn render_recording_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let theme_id = state.app.ui_state.theme_id;
        let current_step = state.app.measurement_state.recording_state.step;
        let all_recorded = state
            .app
            .measurement_state
            .recording_state
            .all_channels_recorded();
        let is_recording = state.app.measurement_state.recording_state.is_recording();

        // Convert RecordingStep to step index
        let step_index = match current_step {
            RecordingStep::Config => 0,
            RecordingStep::Capture => 1,
            RecordingStep::Evaluating => 2,
            RecordingStep::Saving => 3,
        };

        // Build step statuses
        let step_statuses: Vec<StepStatus> = (0..4)
            .map(|i| {
                if i < step_index {
                    StepStatus::Completed
                } else if i == step_index {
                    StepStatus::Active
                } else {
                    StepStatus::NotVisited
                }
            })
            .collect();

        // Build wizard steps
        let steps = vec![
            WizardStep::new("config", "Setup"),
            WizardStep::new("capture", "Capture"),
            WizardStep::new("evaluating", "Evaluate"),
            WizardStep::new("saving", "Save"),
        ];

        let ui_kit_theme = theme.to_ui_kit_theme(theme_id);
        let wizard_theme = WizardTheme::from(&ui_kit_theme);
        let button_theme = ButtonTheme::from(&ui_kit_theme);

        let has_output_dir = state
            .app
            .measurement_state
            .recording_state
            .recording_directory
            .is_some();

        // Determine if next button should be disabled
        let next_disabled = match current_step {
            RecordingStep::Config => !has_output_dir,
            RecordingStep::Capture => !all_recorded || is_recording,
            RecordingStep::Evaluating => false,
            RecordingStep::Saving => false,
        };

        let header = WizardHeader::new()
            .title("Recording")
            .steps(steps)
            .step_statuses(step_statuses)
            .current_step(step_index)
            .theme(wizard_theme.clone());

        let back_label = match current_step {
            RecordingStep::Config => "Close",
            _ => "Back",
        };
        let next_label = match current_step {
            RecordingStep::Saving => "Finish",
            _ => "Next",
        };

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
                                match state.app.measurement_state.recording_state.step {
                                    RecordingStep::Config => {
                                        state.app.ui_state.current_screen =
                                            state.app.ui_state.last_screen;
                                    }
                                    RecordingStep::Capture => {
                                        state.app.measurement_state.recording_state.step =
                                            RecordingStep::Config;
                                    }
                                    RecordingStep::Evaluating => {
                                        state.app.measurement_state.recording_state.step =
                                            RecordingStep::Capture;
                                    }
                                    RecordingStep::Saving => {
                                        state.app.measurement_state.recording_state.step =
                                            RecordingStep::Evaluating;
                                    }
                                }
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
                    .build()
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|view, _, _, cx| {
                            view.state.update(cx, |state, _| {
                                match state.app.measurement_state.recording_state.step {
                                    RecordingStep::Config => {
                                        state
                                            .app
                                            .measurement_state
                                            .recording_state
                                            .init_channel_recordings();
                                        state.app.measurement_state.recording_state.step =
                                            RecordingStep::Capture;
                                    }
                                    RecordingStep::Capture => {
                                        state.app.measurement_state.recording_state.step =
                                            RecordingStep::Evaluating;
                                    }
                                    RecordingStep::Evaluating => {
                                        state.app.measurement_state.recording_state.step =
                                            RecordingStep::Saving;
                                    }
                                    RecordingStep::Saving => {
                                        state.app.ui_state.current_screen =
                                            state.app.ui_state.last_screen;
                                    }
                                }
                            });
                            cx.notify();
                        }),
                    ),
            );

        // Home button for navigation back to Library
        let state_for_home = self.state.clone();
        let text_muted = theme.text_muted;
        let surface_hover = theme.surface_hover;

        div()
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            .py_4()
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
                    .rounded_md()
                    .hover(move |s| s.bg(surface_hover))
                    .child(Icon::new(IconName::Home).color(text_muted))
                    .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                        state_for_home.update(cx, |state, _cx| {
                            state.app.ui_state.current_screen = Screen::Library;
                        });
                    }),
            )
            // Centered header with flex-1
            .child(div().flex_1().flex().justify_center().child(header))
            // Navigation buttons on the right
            .child(navigation)
    }
}
