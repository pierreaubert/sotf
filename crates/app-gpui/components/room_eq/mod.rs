//! Room EQ Screen
//!
//! Multi-step wizard for room EQ optimization:
//! 1. Load Data - Load/import measurement data
//! 2. Configure - Select mode, configure channels and optimizer settings
//! 3. Optimize - Run optimization (per-channel, then combined)
//! 4. Review - Review results and visualizations
//! 5. Export - Export DSP chain and apply

use crate::app::types::RoomEqStep;
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;

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
            .child(self.render_workflow_shell("room-eq-content", content, cx))
            // Custom target curve editor modal
            .child(self.render_custom_target_modal(cx))
    }
}
