//! Room EQ Screen
//!
//! Multi-step wizard for room EQ optimization:
//! 1. Load Data - Load/import measurement data
//! 2. Configure - Configure channels and optimizer settings
//! 3. Optimize - Run optimization (per-channel, then combined)
//! 4. Review - Review results and visualizations
//! 5. Export - Export DSP chain and apply

use crate::app::types::RoomEqStep;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, HStack, StackSpacing, StepStatus, WizardHeader, WizardStep,
    WizardTheme,
};

mod actions;
mod render;
mod step_1_load;
mod step_2_configure;
mod step_3_optimise;
mod step_4_review;
mod step_5_export;

impl PlayerView {
    /// Main Room EQ screen entry point
    pub(crate) fn render_room_eq_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let current_step = state.app.room_eq_state.step;

        // Content for current step
        let content = match current_step {
            RoomEqStep::LoadData => self.render_room_eq_load_data(cx).into_any_element(),
            RoomEqStep::Configure => self.render_room_eq_configure(cx).into_any_element(),
            RoomEqStep::Optimize => self.render_room_eq_optimize(cx).into_any_element(),
            RoomEqStep::Review => self.render_room_eq_review(cx).into_any_element(),
            RoomEqStep::Export => self.render_room_eq_export(cx).into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .child(self.render_room_eq_header(cx))
            .child(
                div()
                    .id("room-eq-content")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_4()
                    .child(content),
            )
    }

    /// Render the room EQ screen header with step indicators using WizardHeader
    fn render_room_eq_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let theme_id = state.app.theme_id;
        let current_step = state.app.room_eq_state.step;
        let can_go_next = self.room_eq_can_advance(cx);
        let is_busy = state.app.room_eq_state.is_optimizing();

        let step_index = current_step.index();

        // Build wizard steps
        let steps = vec![
            WizardStep::new("load-data", "Load Data"),
            WizardStep::new("configure", "Configure"),
            WizardStep::new("optimize", "Optimize"),
            WizardStep::new("review", "Review"),
            WizardStep::new("export", "Export"),
        ];

        // Build step statuses based on current step
        let step_statuses: Vec<StepStatus> = RoomEqStep::all()
            .iter()
            .map(|step| {
                if step.index() < step_index {
                    StepStatus::Completed
                } else if step.index() == step_index {
                    StepStatus::Active
                } else {
                    StepStatus::NotVisited
                }
            })
            .collect();

        let wizard_theme = WizardTheme::from(&theme.to_ui_kit_theme(theme_id));

        let wizard_header = WizardHeader::new()
            .title("Room EQ")
            .steps(steps)
            .step_statuses(step_statuses)
            .current_step(step_index)
            .theme(wizard_theme.clone());

        let back_label = match current_step {
            RoomEqStep::LoadData => "Close",
            _ => "Back",
        };
        let next_label = match current_step {
            RoomEqStep::Export => "Finish",
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
                            view.state
                                .update(cx, |state, _| match state.app.room_eq_state.step {
                                    RoomEqStep::LoadData => {
                                        state.app.current_screen = state.app.last_screen;
                                    }
                                    _ => {
                                        if let Some(prev) = state.app.room_eq_state.step.previous()
                                        {
                                            state.app.room_eq_state.step = prev;
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
                            view.state
                                .update(cx, |state, _| match state.app.room_eq_state.step {
                                    RoomEqStep::Export => {
                                        state.app.current_screen = state.app.last_screen;
                                    }
                                    _ => {
                                        if let Some(next) = state.app.room_eq_state.step.next() {
                                            state.app.room_eq_state.step = next;
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
            .child(wizard_header)
            .child(navigation)
    }

    /// Check if we can advance from current step
    fn room_eq_can_advance(&self, cx: &Context<Self>) -> bool {
        let state = self.state.read(cx);
        let room_eq = &state.app.room_eq_state;

        match room_eq.step {
            RoomEqStep::LoadData => room_eq.has_measurements(),
            RoomEqStep::Configure => !room_eq.speaker_configs.is_empty(),
            RoomEqStep::Optimize => room_eq.is_optimization_complete(),
            RoomEqStep::Review => true,
            RoomEqStep::Export => true,
        }
    }
}