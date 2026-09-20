//! Headphone EQ Screen
//!
//! Multi-step wizard for headphone EQ optimization:
//! 1. Measurement & Target - Choose measurement file and target curve
//! 2. Optimization - EQ design, fine tuning, and generate EQ
//! 3. Listen - Preview and apply EQ to playback
//! 4. Export - Apply to playback, export format selection and save

mod actions;
mod measurement_identity;
mod step_1_measurements;
mod step_2_optimisation;
mod step_3_listen;
mod step_4_export;
mod target_controls;

use crate::app::types::HeadphoneEqStep;
use crate::ui::PlayerView;
use gpui::prelude::*;

impl PlayerView {
    // ========================================================================
    // Headphone EQ Wizard Screen
    // ========================================================================

    /// Main Headphone EQ screen entry point (wizard)
    pub(crate) fn render_headphone_eq_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
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

        self.render_workflow_shell("headphone-eq-content", content, cx)
    }
}
