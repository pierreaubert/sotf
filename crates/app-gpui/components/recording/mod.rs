//! Recording screen module
//!
//! Multi-channel audio recording workflow with six steps:
//! 1. Config - Device selection and channel mapping
//! 2. Capture - Record frequency response for each channel
//! 3. Probe - Tone-burst arrival-time probe per channel
//! 4. BassAnchor - Low-frequency tone burst for first-bin phase anchor
//!    (GD-Opt v2 plan §2.6, `docs/gd_opt_v2_plan.md` in the autoeq repo)
//! 5. Evaluating - View and analyze frequency response graphs
//! 6. Saving - Save recordings and configuration to disk

mod bass_anchor;
mod capture;
mod config;
mod evaluating;
#[cfg(not(target_os = "ios"))]
mod multi_capture;
mod probe;
mod saving;
mod spl_calibration;

use crate::app::types::RecordingStep;
use crate::ui::PlayerView;
use gpui::prelude::*;

impl PlayerView {
    /// Main recording screen renderer - dispatches to the appropriate step
    pub(crate) fn render_recording_screen(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        #[cfg(not(target_os = "ios"))]
        if self
            .state
            .read(cx)
            .app
            .measurement_state
            .multi_capture
            .active
        {
            return self.render_multi_capture_panel(cx).into_any_element();
        }
        let current_step = self
            .state
            .read(cx)
            .app
            .measurement_state
            .recording_state
            .step;
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

        #[cfg(not(target_os = "ios"))]
        let step_content = gpui_ui_kit::VStack::new()
            .spacing(gpui_ui_kit::StackSpacing::Md)
            .child(self.render_multi_capture_launcher(cx))
            .child(step_content)
            .into_any_element();
        self.render_workflow_shell("recording-content", step_content, cx)
    }
}
