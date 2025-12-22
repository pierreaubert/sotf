//! Headphone EQ Screen
//!
//! Multi-step wizard for headphone EQ optimization:
//! 1. Measurement & Target - Choose measurement file and target curve
//! 2. Optimization - EQ design, fine tuning, and generate EQ
//! 3. Listen - Preview and apply EQ to playback
//! 4. Save - Export format selection and save

mod actions;
mod step4_step;
mod step_1_measurements;
mod step_2_optimisation;
mod step_3_listen;

use crate::app::types::{HeadphoneEqStep, PluginUpdateType};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, HStack, StackSpacing, StepStatus, WizardHeader, WizardStep,
    WizardTheme,
};

impl PlayerView {
    // ========================================================================
    // Headphone EQ Wizard Screen
    // ========================================================================

    /// Clear the headphone EQ from the playback chain
    pub fn clear_headphone_eq_from_playback(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Find and remove EQ plugins
            let plugins = state.app.plugin_chain.plugins();
            let eq_indices: Vec<_> = plugins
                .iter()
                .enumerate()
                .filter_map(|(i, p)| {
                    if matches!(p.plugin_type(), sotf_audio_player::PluginType::EQ) {
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect();

            // Remove in reverse order to maintain correct indices
            for idx in eq_indices.into_iter().rev() {
                state.app.plugin_chain.remove_plugin(idx);
            }

            state.app.pending_plugin_update = Some(PluginUpdateType::Structural);
            state.app.toast_message = Some(crate::app::ToastMessage::success(
                "Cleared EQ from playback",
            ));
        });
        cx.notify();
    }

    /// Main Headphone EQ screen entry point (wizard)
    pub(crate) fn render_headphone_eq_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let current_step = state.app.headphone_eq_state.step;

        // Content for current step
        let content = match current_step {
            HeadphoneEqStep::MeasurementTarget => self
                .render_headphone_eq_measurement_target(cx)
                .into_any_element(),
            HeadphoneEqStep::Optimization => {
                self.render_headphone_eq_optimization(cx).into_any_element()
            }
            HeadphoneEqStep::Listen => self.render_headphone_eq_listen(cx).into_any_element(),
            HeadphoneEqStep::Save => self.render_headphone_eq_save(cx).into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .child(self.render_headphone_eq_header(cx))
            .child(
                div()
                    .id("headphone-eq-content")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_4()
                    .child(content),
            )
    }

    /// Render the headphone EQ screen header with step indicators
    fn render_headphone_eq_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let theme_id = state.app.theme_id;
        let current_step = state.app.headphone_eq_state.step;
        let can_go_next = state.app.headphone_eq_state.can_advance();
        let is_busy = state.app.headphone_eq_state.is_optimizing();

        // Map current step to index
        let step_index = match current_step {
            HeadphoneEqStep::MeasurementTarget => 0,
            HeadphoneEqStep::Optimization => 1,
            HeadphoneEqStep::Listen => 2,
            HeadphoneEqStep::Save => 3,
        };

        // Define steps
        let steps = vec![
            WizardStep::new("measure", "Measurement"),
            WizardStep::new("optimize", "Optimization"),
            WizardStep::new("listen", "Listen"),
            WizardStep::new("save", "Save"),
        ];

        // Calculate statuses
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

        let wizard_theme = WizardTheme::from(&theme.to_ui_kit_theme(theme_id));

        let header = WizardHeader::new()
            .title("Headphone EQ")
            .steps(steps)
            .step_statuses(step_statuses)
            .current_step(step_index)
            .theme(wizard_theme.clone());

        let back_label = match current_step {
            HeadphoneEqStep::MeasurementTarget => "Close",
            _ => "Back",
        };
        let next_label = match current_step {
            HeadphoneEqStep::Save => "Finish",
            _ => "Next",
        };

        let navigation = HStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Button::new("back", back_label)
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Md)
                    .disabled(is_busy)
                    .build()
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|view, _, _, cx| {
                            view.state.update(cx, |state, _| {
                                match state.app.headphone_eq_state.step {
                                    HeadphoneEqStep::MeasurementTarget => {
                                        state.app.current_screen = state.app.last_screen;
                                    }
                                    _ => {
                                        if let Some(prev) =
                                            state.app.headphone_eq_state.step.previous()
                                        {
                                            state.app.headphone_eq_state.step = prev;
                                        }
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
                    .size(ButtonSize::Md)
                    .disabled(!can_go_next || is_busy)
                    .build()
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|view, _, _, cx| {
                            view.state.update(cx, |state, _| {
                                match state.app.headphone_eq_state.step {
                                    HeadphoneEqStep::Save => {
                                        state.app.current_screen = state.app.last_screen;
                                    }
                                    _ => {
                                        if let Some(next) = state.app.headphone_eq_state.step.next()
                                        {
                                            state.app.headphone_eq_state.step = next;
                                        }
                                    }
                                }
                            });
                            cx.notify();
                        }),
                    ),
            );

        div()
            .flex()
            .items_center()
            .justify_between()
            .px_6()
            .py_4()
            .bg(theme.background_secondary)
            .border_b_1()
            .border_color(theme.border)
            .child(header)
            .child(navigation)
    }
}
