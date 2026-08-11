//! Room EQ Screen
//!
//! Multi-step wizard for room EQ optimization:
//! 1. Load Data - Load/import measurement data
//! 2. Configure - Select mode, configure channels and optimizer settings
//! 3. Optimize - Run optimization (per-channel, then combined)
//! 4. Review - Review results and visualizations
//! 5. Export - Export DSP chain and apply

use crate::app::types::{RoomEqStep, Screen};
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonTheme, ButtonVariant, HStack, StackSpacing, StepStatus, WizardHeader,
    WizardStep, WizardTheme,
};

mod actions;
mod custom_target_modal;
pub mod render;
mod step_1_load;
mod step_2_delay_detection;
mod step_3_configure;
mod step_3_process;
mod step_4_optimise;
mod step_5_review;
pub mod step_6_export;

pub use step_4_optimise::{
    RoomEqProgressChartSeries, room_eq_channel_chain_by_name, room_eq_display_response_points,
    room_eq_initial_response_points, room_eq_progress_chart_series,
};

impl PlayerView {
    /// Main Room EQ screen entry point
    pub(crate) fn render_room_eq_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let (theme, current_step, current_hint, dismiss_hint_label) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.measurement_state.room_eq_state.step,
                state.app.tutorial.current_hint.clone(),
                crate::app::i18n::DialogTranslations::for_language(state.app.ui_state.language)
                    .about
                    .close,
            )
        };

        // Content for current step.
        let content = match current_step {
            RoomEqStep::LoadData => self.render_room_eq_load_data(cx).into_any_element(),
            RoomEqStep::Delay => self.render_room_eq_delay_detection(cx).into_any_element(),
            RoomEqStep::Process => self.render_room_eq_process(cx).into_any_element(),
            RoomEqStep::Configure => self.render_room_eq_configure(cx).into_any_element(),
            RoomEqStep::Optimize => self.render_room_eq_optimize(cx).into_any_element(),
            RoomEqStep::Review => self.render_room_eq_review(cx).into_any_element(),
            RoomEqStep::Export => self.render_room_eq_export(cx).into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .bg(theme.background)
            .child(self.render_room_eq_header(cx))
            // Contextual hint banner (only Room EQ hints)
            .when_some(
                current_hint.filter(|h| {
                    matches!(
                        h.hint_id,
                        crate::components::dialogs::tutorial::HintId::RoomEqFirstVisit
                    )
                }),
                |el, hint| {
                    el.child(
                        div()
                            .id("roomeq-hint-banner")
                            .cursor_pointer()
                            .on_mouse_up(
                                gpui::MouseButton::Left,
                                cx.listener(|view, _: &gpui::MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, cx| {
                                        state.app.dismiss_hint();
                                        let layout = state.layout.read(cx);
                                        if let Err(error) = state.app.save_config(layout) {
                                            log::error!("Failed to save config: {error}");
                                        }
                                    });
                                    cx.notify();
                                }),
                            )
                            .child(crate::components::dialogs::tutorial::render_hint_banner(
                                &hint,
                                &theme,
                                d,
                                dismiss_hint_label,
                                cx.listener(|view, _: &ClickEvent, _window, cx| {
                                    cx.stop_propagation();
                                    view.state.update(cx, |state, cx| {
                                        state.app.dismiss_hint();
                                        let layout = state.layout.read(cx);
                                        if let Err(error) = state.app.save_config(layout) {
                                            log::error!("Failed to save config: {error}");
                                        }
                                    });
                                    cx.notify();
                                }),
                            )),
                    )
                },
            )
            .child(
                div()
                    .id("room-eq-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(d.card)
                    .child(content),
            )
            // Custom target curve editor modal
            .child(self.render_custom_target_modal(cx))
    }

    /// Render the room EQ screen header with step indicators using WizardHeader
    fn render_room_eq_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let title = state.app.ui_state.translations.screen_room_eq;
        let theme_id = state.app.ui_state.theme_id;
        let current_step = state.app.measurement_state.room_eq_state.step;
        let can_go_next = state.app.can_advance_workflow_step();
        let is_busy = state.app.measurement_state.room_eq_state.is_optimizing();

        let step_index = current_step.index();

        // Build wizard steps from `RoomEqStep::all()` so new variants
        // (e.g. `DelayDetection`) show up in the tab bar automatically.
        // Hand-rolling this list is how the Delay step initially went
        // missing from the header even though it was wired everywhere
        // else.
        let steps: Vec<WizardStep> = RoomEqStep::all()
            .iter()
            .map(|s| {
                let id = match s {
                    RoomEqStep::LoadData => "load-data",
                    RoomEqStep::Delay => "delay",
                    RoomEqStep::Process => "process",
                    RoomEqStep::Configure => "configure",
                    RoomEqStep::Optimize => "optimize",
                    RoomEqStep::Review => "review",
                    RoomEqStep::Export => "export",
                };
                WizardStep::new(id, s.label())
            })
            .collect();

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

        let ui_kit_theme = theme.to_ui_kit_theme(theme_id, cx);
        let wizard_theme = WizardTheme::from(&ui_kit_theme);
        let button_theme = ButtonTheme::from(&ui_kit_theme);

        let wizard_header = WizardHeader::new()
            .title(title)
            .steps(steps)
            .step_statuses(step_statuses)
            .current_step(step_index)
            .theme(wizard_theme.clone());

        let back_label = match current_step {
            RoomEqStep::LoadData => "Close",
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
                    .id("room-eq-home-button")
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
                    .child(wizard_header),
            )
            // Navigation buttons on the right
            .child(navigation)
    }
}
