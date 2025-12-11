//! Recording screen module
//!
//! Multi-channel audio recording workflow with two steps:
//! 1. Config - Device selection and channel mapping
//! 2. Capture - Record frequency response for each channel

mod capture;
mod config;

use crate::app::types::RecordingStep;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, HStack, StackAlign, StackSpacing, Text, TextSize, TextWeight,
};

impl PlayerView {
    /// Main recording screen renderer - dispatches to config or capture step
    pub(crate) fn render_recording_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, current_step) = {
            let state = self.state.read(cx);
            (state.app.theme.clone(), state.app.recording_state.step)
        };

        let step_content = match current_step {
            RecordingStep::Config => self.render_recording_config_step(cx).into_any_element(),
            RecordingStep::Capture => self.render_recording_capture_step(cx).into_any_element(),
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
        let theme = state.app.theme.clone();
        let current_step = state.app.recording_state.step;
        drop(state);

        // Helper function to build step indicator
        let build_step_indicator =
            |step: RecordingStep, label: &'static str, number: u8, theme: &crate::theme::Theme| {
                let is_active = current_step == step;
                let is_past = matches!(
                    (current_step, step),
                    (RecordingStep::Capture, RecordingStep::Config)
                );

                let (bg_color, text_color, border_color) = if is_active {
                    (theme.accent, theme.text_on_accent, theme.accent)
                } else if is_past {
                    (theme.success, theme.text_on_accent, theme.success)
                } else {
                    (theme.surface, theme.text_muted, theme.border)
                };

                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .align(StackAlign::Center)
                    .child(
                        div()
                            .w(px(28.0))
                            .h(px(28.0))
                            .rounded_full()
                            .bg(bg_color)
                            .border_2()
                            .border_color(border_color)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Text::new(number.to_string())
                                    .size(TextSize::Sm)
                                    .weight(TextWeight::Bold)
                                    .color(text_color),
                            ),
                    )
                    .child(
                        Text::new(label)
                            .size(TextSize::Sm)
                            .weight(if is_active {
                                TextWeight::Bold
                            } else {
                                TextWeight::Normal
                            })
                            .color(if is_active {
                                theme.text_primary
                            } else {
                                theme.text_muted
                            }),
                    )
            };

        div()
            .flex()
            .items_center()
            .justify_between()
            .px_6()
            .py_4()
            .bg(theme.background_secondary)
            .border_b_1()
            .border_color(theme.border)
            .child(
                HStack::new()
                    .spacing(StackSpacing::Lg)
                    .align(StackAlign::Center)
                    .child(
                        Text::new("Recording")
                            .size(TextSize::Xl)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    )
                    .child(div().w(px(1.0)).h(px(24.0)).bg(theme.border))
                    .child(build_step_indicator(
                        RecordingStep::Config,
                        "Device Setup",
                        1,
                        &theme,
                    ))
                    .child(div().w(px(32.0)).h(px(2.0)).bg(
                        if current_step == RecordingStep::Capture {
                            theme.success
                        } else {
                            theme.border
                        },
                    ))
                    .child(build_step_indicator(
                        RecordingStep::Capture,
                        "Capture",
                        2,
                        &theme,
                    )),
            )
            .child(self.render_recording_nav_buttons(cx))
    }

    /// Render navigation buttons (Back/Next)
    fn render_recording_nav_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let current_step = state.app.recording_state.step;
        let all_recorded = state.app.recording_state.all_channels_recorded();
        let is_recording = state.app.recording_state.is_recording();
        let view = cx.entity().clone();

        let back_label = match current_step {
            RecordingStep::Config => "Close",
            RecordingStep::Capture => "Back",
        };
        let next_label = match current_step {
            RecordingStep::Config => "Next",
            RecordingStep::Capture => "Finish",
        };

        HStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Button::new("back", back_label)
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Md)
                    .disabled(is_recording)
                    .on_click({
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    match state.app.recording_state.step {
                                        RecordingStep::Config => {
                                            // Go back to previous screen
                                            state.app.current_screen = state.app.last_screen;
                                        }
                                        RecordingStep::Capture => {
                                            // Go back to config step
                                            state.app.recording_state.step = RecordingStep::Config;
                                        }
                                    }
                                });
                                cx.notify();
                            });
                        }
                    }),
            )
            .child(
                Button::new("next", next_label)
                    .variant(ButtonVariant::Primary)
                    .size(ButtonSize::Md)
                    .disabled(match current_step {
                        RecordingStep::Config => false,
                        RecordingStep::Capture => !all_recorded || is_recording,
                    })
                    .on_click({
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    match state.app.recording_state.step {
                                        RecordingStep::Config => {
                                            // Initialize channel recordings and go to capture
                                            state.app.recording_state.init_channel_recordings();
                                            state.app.recording_state.step = RecordingStep::Capture;
                                        }
                                        RecordingStep::Capture => {
                                            // Finish - go back to previous screen
                                            // TODO: Save results and process data
                                            state.app.current_screen = state.app.last_screen;
                                        }
                                    }
                                });
                                cx.notify();
                            });
                        }
                    }),
            )
    }
}
