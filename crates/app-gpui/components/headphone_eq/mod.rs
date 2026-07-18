//! Headphone EQ Screen
//!
//! Multi-step wizard for headphone EQ optimization:
//! 1. Measurement & Target - Choose measurement file and target curve
//! 2. Optimization - EQ design, fine tuning, and generate EQ
//! 3. Listen - Preview and apply EQ to playback
//! 4. Export - Apply to playback, export format selection and save

mod actions;
mod step_1_measurements;
mod step_2_optimisation;
mod step_3_listen;
mod step_4_export;

use crate::app::types::{HeadphoneEqStep, Screen};
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName};
use crate::i18n::HeadphoneEqTranslations;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonTheme, ButtonVariant, HStack, StackSpacing, StepStatus, WizardHeader,
    WizardStep, WizardTheme,
};

impl PlayerView {
    // ========================================================================
    // Headphone EQ Wizard Screen
    // ========================================================================

    /// Main Headphone EQ screen entry point (wizard)
    pub(crate) fn render_headphone_eq_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let current_step = state.app.measurement_state.headphone_eq_state.step;

        // Content for current step
        let content = match current_step {
            HeadphoneEqStep::MeasurementTarget => self
                .render_headphone_eq_measurement_target(cx)
                .into_any_element(),
            HeadphoneEqStep::Optimization => {
                self.render_headphone_eq_optimization(cx).into_any_element()
            }
            HeadphoneEqStep::Listen => self.render_headphone_eq_listen(cx).into_any_element(),
            HeadphoneEqStep::Export => self.render_headphone_eq_export(cx).into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .bg(theme.background)
            .child(self.render_headphone_eq_header(cx))
            .child(
                div()
                    .id("headphone-eq-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(d.card)
                    .child(content),
            )
    }

    /// Render the headphone EQ screen header with step indicators
    fn render_headphone_eq_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = HeadphoneEqTranslations::for_language(state.app.ui_state.language);
        let theme_id = state.app.ui_state.theme_id;
        let current_step = state.app.measurement_state.headphone_eq_state.step;
        let can_go_next = state.app.can_advance_workflow_step();
        let is_busy = state
            .app
            .measurement_state
            .headphone_eq_state
            .is_optimizing();

        // Map current step to index
        let step_index = match current_step {
            HeadphoneEqStep::MeasurementTarget => 0,
            HeadphoneEqStep::Optimization => 1,
            HeadphoneEqStep::Listen => 2,
            HeadphoneEqStep::Export => 3,
        };

        // Define steps
        let steps = vec![
            WizardStep::new("measure", translations.measurement_step),
            WizardStep::new("optimize", translations.optimization_step),
            WizardStep::new("listen", translations.listen_step),
            WizardStep::new("export", translations.export),
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

        let ui_kit_theme = theme.to_ui_kit_theme(theme_id, cx);
        let wizard_theme = WizardTheme::from(&ui_kit_theme);
        let button_theme = ButtonTheme::from(&ui_kit_theme);

        let header = WizardHeader::new()
            .title(translations.title)
            .steps(steps)
            .step_statuses(step_statuses)
            .current_step(step_index)
            .theme(wizard_theme.clone());

        let back_label = match current_step {
            HeadphoneEqStep::MeasurementTarget => translations.close,
            _ => translations.back,
        };
        let next_label =
            crate::components::wizard_continue_label(current_step.next().map(|next| next.label()));

        let navigation = HStack::new()
            .spacing(StackSpacing::Sm)
            .child(
                Button::new("back", back_label)
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Sm)
                    .disabled(is_busy)
                    .theme(button_theme.clone())
                    .on_click_event(cx.listener(|view, _, _, cx| {
                        view.state.update(cx, |state, _| {
                            state.app.move_workflow_step(false);
                        });
                        cx.notify();
                    })),
            )
            .child(
                Button::new("next", next_label)
                    .variant(ButtonVariant::Primary)
                    .size(ButtonSize::Sm)
                    .disabled(!can_go_next || is_busy)
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
                    .id("headphone-eq-home-button")
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
