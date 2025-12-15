//! Speaker EQ optimization - GPUI frontend
//!
//! This module provides GPUI-specific UI interactions for speaker EQ optimization.
//! The actual optimization logic is in sotf_audio_player::autoeq::speaker.

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;

// Re-export the result type from the common library
pub use sotf_audio_player::autoeq::SpeakerOptimizationResult;

impl PlayerView {
    /// Run speaker optimization
    pub fn run_speaker_optimization(&mut self, cx: &mut Context<Self>) {
        let (speaker_model, params, _export_format) = {
            let state = self.state.read(cx);
            if state.app.speaker_model.is_empty() {
                self.state.update(cx, |state, _cx| {
                    state.app.toast_message = Some(crate::app::ToastMessage::error(
                        "Please select a speaker model",
                    ));
                });
                cx.notify();
                return;
            }
            (
                state.app.speaker_model.clone(),
                state.app.speaker_params.clone(),
                state.app.speaker_export_format.clone(),
            )
        };

        self.state.update(cx, |state, _cx| {
            state.app.speaker_optimization_running = true;
            state.app.speaker_optimization_progress.clear();
            state.app.speaker_optimization_result = None;
        });
        cx.notify();

        cx.spawn(async move |view: WeakEntity<PlayerView>, cx| {
            // Run speaker optimization using the common library
            let result = smol::unblock(move || {
                // Simulate delay for dummy speaker
                if speaker_model == "Dummy Speaker" {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
                sotf_audio_player::autoeq::run_speaker_optimization(&speaker_model, &params)
            })
            .await;

            view.update(cx, |view, cx| {
                view.state.update(cx, |state, _cx| {
                    state.app.speaker_optimization_running = false;
                    match result {
                        Ok(res) => {
                            state.app.speaker_optimization_result = Some(res);
                            state.app.toast_message = Some(crate::app::ToastMessage::success(
                                "Speaker optimization completed",
                            ));
                        }
                        Err(e) => {
                            state.app.toast_message = Some(crate::app::ToastMessage::error(
                                format!("Optimization failed: {}", e),
                            ));
                        }
                    }
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
