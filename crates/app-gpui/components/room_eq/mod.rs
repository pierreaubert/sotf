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
use crate::i18n::{RoomEqWorkflowTranslations, WizardNavigationTranslations};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonTheme, ButtonVariant, HStack, Heading, StackSpacing, StepStatus,
    WizardHeader, WizardStep, WizardTheme,
};

macro_rules! dev_track {
    ($element:expr, $selector:expr) => {{
        #[cfg(feature = "dev-api")]
        {
            use crate::app::dev_api::DevTrackExt;
            $element.dev_track($selector)
        }
        #[cfg(not(feature = "dev-api"))]
        {
            $element
        }
    }};
}

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

const COMPACT_STEP_LABEL_EFFECTIVE_WIDTH: f32 = 800.0;

#[doc(hidden)]
pub fn room_eq_header_uses_icon_only_steps(window_width: f32, font_scale: f32) -> bool {
    window_width / font_scale.max(f32::EPSILON) < COMPACT_STEP_LABEL_EFFECTIVE_WIDTH
}

fn render_compact_step_indicators(step_statuses: &[StepStatus], theme: &WizardTheme) -> Div {
    let mut indicators = div().w_full().flex().items_center();

    for (index, status) in step_statuses.iter().copied().enumerate() {
        let (background, text, border) = match status {
            StepStatus::NotVisited | StepStatus::Skipped => {
                (theme.step_bg, theme.label_text, theme.step_border)
            }
            StepStatus::Active => (theme.step_active_bg, theme.step_text, theme.step_active_bg),
            StepStatus::Completed => (
                theme.step_completed_bg,
                theme.step_text,
                theme.step_completed_bg,
            ),
            StepStatus::Error => (theme.step_error_bg, theme.step_text, theme.step_error_bg),
        };
        let marker = match status {
            StepStatus::Completed => "✓".to_string(),
            StepStatus::Error => "✗".to_string(),
            _ => (index + 1).to_string(),
        };

        indicators = indicators.child(
            div()
                .flex_none()
                .w(rems(1.5))
                .h(rems(1.5))
                .rounded_full()
                .border_2()
                .border_color(border)
                .bg(background)
                .text_color(text)
                .flex()
                .items_center()
                .justify_center()
                .child(marker),
        );

        if index + 1 < step_statuses.len() {
            let connector = if status == StepStatus::Completed {
                theme.connector_completed_color
            } else {
                theme.connector_color
            };
            indicators = indicators.child(
                div()
                    .flex_1()
                    .min_w(rems(0.25))
                    // intentional: sub-token inset keeps all seven compact wizard markers visible.
                    .mx(rems(0.2))
                    .h(rems(0.1))
                    .bg(connector),
            );
        }
    }

    indicators
}

impl PlayerView {
    /// Main Room EQ screen entry point
    pub(crate) fn render_room_eq_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let (theme, current_step, current_hint, language, dismiss_hint_label) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.measurement_state.room_eq_state.step,
                state.app.tutorial.current_hint.clone(),
                state.app.ui_state.language,
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
                                language,
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
        let language = state.app.ui_state.language;
        let translations = RoomEqWorkflowTranslations::for_language(language);
        let wizard_text = WizardNavigationTranslations::for_language(language);
        let theme_id = state.app.ui_state.theme_id;
        let current_step = state.app.measurement_state.room_eq_state.step;
        let can_go_next = state.app.can_advance_workflow_step();
        let is_busy = state.app.measurement_state.room_eq_state.is_optimizing();
        let icon_only_steps = room_eq_header_uses_icon_only_steps(
            state.app.ui_state.window_width,
            state.app.ui_state.font_scale,
        );

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
                WizardStep::new(id, translations.step_label(*s))
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

        let wizard_header = if icon_only_steps {
            render_compact_step_indicators(&step_statuses, &wizard_theme)
        } else {
            div().child(
                WizardHeader::new()
                    .steps(steps)
                    .step_statuses(step_statuses)
                    .current_step(step_index)
                    .theme(wizard_theme.clone()),
            )
        };

        let back_label = match current_step {
            RoomEqStep::LoadData => wizard_text.close,
            _ => wizard_text.back,
        };
        let next_label = crate::components::wizard_continue_label(
            language,
            current_step
                .next()
                .map(|next| translations.step_label(next)),
        );

        let navigation = HStack::new()
            .spacing(StackSpacing::Sm)
            .child(dev_track!(
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
                "roomeq.back"
            ))
            .child(dev_track!(
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
                "roomeq.next"
            ));
        let navigation = navigation.build().flex_none().ml_auto();

        // Home button for navigation back to Library
        let state_for_home = self.state.clone();
        let text_muted = theme.text_muted;
        let surface_hover = theme.surface_hover;

        div()
            .flex()
            .flex_col()
            .gap(d.gap)
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
                    .gap(d.gap)
                    .min_w_0()
                    .h(rems(2.0))
                    .cursor_pointer()
                    .rounded(d.r_md)
                    .hover(move |s| s.bg(surface_hover))
                    .child(Icon::new(IconName::Home).color(text_muted))
                    .child(Heading::h4(title))
                    .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                        state_for_home.update(cx, |state, _cx| {
                            state.app.ui_state.current_screen = Screen::Library;
                        });
                    }),
            )
            // Centered header with flex-1
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .flex()
                    .justify_center()
                    .child(wizard_header),
            )
            // Navigation buttons on the right
            .child(navigation)
    }
}
