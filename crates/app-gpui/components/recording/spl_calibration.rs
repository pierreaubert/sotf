//! Recording wizard — Step 2: SPL Calibration (GD-Opt v2 Phase GD-1e.5).
//!
//! Plays a 1 kHz reference tone on a single output channel; while it
//! plays the user reads the dBSPL their external meter shows at the
//! listening position and types it in. The captured
//! `(rms_sample_level, reported_db_spl)` pair gives GD-Opt v2 a
//! deterministic map from digital level to dBSPL at the mic, which
//! `sweep_level_db_spl` targets during the Capture step.
//!
//! See the GD-Opt v2 plan (`docs/gd_opt_v2_plan.md` in the autoeq repo)
//! §2.6 and §2.11 Q4 for the calibration
//! rationale and the "require SPL cap" decision.
//!
//! The live capture follows the same `smol::unblock + state.update`
//! pattern the probe and bass-anchor steps use.

use crate::app::types::recording::SplCalibrationCaptureStatus;
use crate::ui::PlayerView;
use gpui::{AnyElement, Context, IntoElement};
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, NumberInput, NumberInputSize, StackAlign,
    StackSpacing, Text, TextSize, TextWeight, VStack,
};

impl PlayerView {
    /// Render the SPL calibration step UI.
    pub(crate) fn render_recording_spl_calibration_step(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let text = RecordingTranslations::for_language(state.app.ui_state.language);
        let recording_text = crate::app::i18n::RecordingWorkflowTranslations::for_language(
            state.app.ui_state.language,
        );
        let rec = &state.app.measurement_state.recording_state;
        let cal = rec.spl_calibration_capture.clone();
        let signal_level_db = rec.signal_level_db;
        let running = matches!(cal.status, SplCalibrationCaptureStatus::Running { .. });

        let status_line = match &cal.status {
            SplCalibrationCaptureStatus::Idle => format!(
                "Ready — {:.0} Hz @ {:.1} dBFS for {:.1}s on ch {}",
                cal.reference_freq_hz, signal_level_db, cal.duration_s, cal.output_channel
            ),
            SplCalibrationCaptureStatus::Running { .. } => {
                "Tone playing — read your SPL meter now…".to_string()
            }
            SplCalibrationCaptureStatus::Complete => {
                let er = cal.engine_result.as_ref();
                match er {
                    Some(r) => format!(
                        "Tone captured — peak {:.4}, RMS {:.4}. Enter the dBSPL your meter showed.",
                        r.peak_sample_level, r.rms_sample_level
                    ),
                    None => "Complete".to_string(),
                }
            }
            SplCalibrationCaptureStatus::Failed(e) => format!("Failed: {e}"),
        };

        let view = cx.entity().clone();
        let start_button: AnyElement = if running {
            Button::new("spl-cal-cancel", recording_text.cancel)
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Md)
                .on_click({
                    let view = view.clone();
                    move |_, cx| {
                        view.update(cx, |this, cx| this.cancel_spl_calibration_capture(cx));
                    }
                })
                .into_any_element()
        } else {
            let button_label = match cal.status {
                SplCalibrationCaptureStatus::Idle | SplCalibrationCaptureStatus::Failed(_) => {
                    "Play calibration tone"
                }
                SplCalibrationCaptureStatus::Running { .. } => "Playing…",
                SplCalibrationCaptureStatus::Complete => "Re-play tone",
            };
            Button::new("spl-cal-start", button_label)
                .variant(ButtonVariant::Primary)
                .size(ButtonSize::Md)
                .on_click({
                    let view = view.clone();
                    move |_, cx| {
                        view.update(cx, |this, cx| this.start_spl_calibration_capture(cx));
                    }
                })
                .into_any_element()
        };

        let mut column = VStack::new()
            .spacing(StackSpacing::Sm)
            .child(
                Text::new(translations.recording_spl_calibration)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Md),
            )
            .child(Text::new(recording_text.spl_calibration_instructions).size(TextSize::Sm))
            .child(Text::new(status_line).size(TextSize::Sm))
            .child({
                let view = cx.entity().clone();
                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .align(StackAlign::Center)
                    .child(Text::new(translations.recording_level_label).size(TextSize::Sm))
                    .child(
                        NumberInput::new("spl_calibration_signal_level")
                            .value(signal_level_db as f64)
                            .min(-60.0)
                            .max(0.0)
                            .step(1.0)
                            .decimals(0)
                            .unit("dBFS")
                            .size(NumberInputSize::Sm)
                            .width(120.0)
                            .on_change(move |val, _window, cx| {
                                view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        let rec = &mut state.app.measurement_state.recording_state;
                                        // R3: reject levels above full scale
                                        // with a clear message instead of
                                        // producing a clipped tone/sweep.
                                        if val > 0.0 {
                                            rec.status_message = "Level must be ≤ 0 dB (positive values clip the signal)"
                                                .to_string();
                                        } else {
                                            rec.signal_level_db = val as f32;
                                            rec.spl_calibration_capture.tone_amp =
                                                10.0_f32.powf(val as f32 / 20.0).clamp(0.0, 1.0);
                                        }
                                    });
                                    cx.notify();
                                });
                            }),
                    )
            })
            .child(start_button);

        if let Some(r) = cal.engine_result.as_ref() {
            let view = cx.entity().clone();
            let initial_db_spl = cal.reported_db_spl.unwrap_or(75.0) as f64;
            column = column
                .child(
                    Text::new(format!(
                        "Ref freq {:.0} Hz  •  sample rate {} Hz  •  peak {:.4}  •  RMS {:.4}",
                        r.reference_freq_hz, r.sample_rate, r.peak_sample_level, r.rms_sample_level
                    ))
                    .size(TextSize::Sm),
                )
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(Text::new(text.reported_dbspl).size(TextSize::Sm))
                        .child(
                            NumberInput::new("spl_calibration_reported_db_spl")
                                .value(initial_db_spl)
                                .min(30.0)
                                .max(130.0)
                                .step(0.5)
                                .decimals(1)
                                .unit("dB SPL")
                                .size(NumberInputSize::Sm)
                                .width(140.0)
                                .on_change(move |val, _window, cx| {
                                    view.update(cx, |this, cx| {
                                        this.state.update(cx, |state, _| {
                                            state
                                                .app
                                                .measurement_state
                                                .recording_state
                                                .spl_calibration_capture
                                                .reported_db_spl = Some(val as f32);
                                        });
                                        cx.notify();
                                    });
                                }),
                        ),
                );
            if let Some(cal_out) = cal.to_spl_calibration() {
                column = column.child(
                    Text::new(format!(
                        "→ spl_offset_db = {:.2}  (stored on save)",
                        cal_out.spl_offset_db
                    ))
                    .size(TextSize::Sm),
                );
            }
        }

        Card::new()
            .background(theme.surface)
            .border(theme.border)
            .content(column)
    }

    /// Spawn the SPL calibration capture on a `smol::unblock` worker.
    /// Result is applied back onto `state` via `apply_engine_result`;
    /// the UI surfaces the engine's peak/RMS numbers and waits for
    /// the user to type the meter reading before the cal is "ready".
    pub(crate) fn start_spl_calibration_capture(&mut self, cx: &mut Context<Self>) {
        let (
            reference_freq_hz,
            tone_amp,
            duration_s,
            sample_rate,
            output_channel,
            input_channel,
            out_dev,
            in_dev,
        ) = {
            let state = self.state.read(cx);
            let rec = &state.app.measurement_state.recording_state;
            let cal = &rec.spl_calibration_capture;
            let tone_amp = 10.0_f32.powf(rec.signal_level_db / 20.0).clamp(0.0, 1.0);
            (
                cal.reference_freq_hz,
                tone_amp,
                cal.duration_s,
                cal.sample_rate,
                cal.output_channel,
                cal.input_channel,
                Some(rec.playback_config.device_name.clone()),
                Some(rec.recording_config.device_name.clone()),
            )
        };

        let started_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let cancel_flag = self.state.update(cx, |state, cx| {
            let rec = &mut state.app.measurement_state.recording_state;
            rec.spl_cancel_requested
                .store(false, std::sync::atomic::Ordering::Relaxed);
            rec.spl_calibration_capture.status =
                SplCalibrationCaptureStatus::Running { started_at_ms };
            rec.spl_calibration_capture.engine_result = None;
            rec.spl_calibration_capture.tone_amp =
                10.0_f32.powf(rec.signal_level_db / 20.0).clamp(0.0, 1.0);
            cx.notify();
            rec.spl_cancel_requested.clone()
        });

        let state_clone = self.state.clone();
        let cancel_for_task = cancel_flag.clone();
        cx.spawn(async move |_, cx| {
            let result = smol::unblock(move || {
                #[cfg(not(target_os = "ios"))]
                {
                    sotf_audio::signal_recorder::run_spl_calibration(
                        output_channel,
                        sample_rate,
                        reference_freq_hz,
                        tone_amp,
                        duration_s,
                        out_dev.as_deref(),
                        in_dev.as_deref(),
                        input_channel,
                        Some(cancel_for_task),
                    )
                }
                #[cfg(target_os = "ios")]
                {
                    let _ = (
                        output_channel,
                        sample_rate,
                        reference_freq_hz,
                        tone_amp,
                        duration_s,
                        out_dev.as_deref(),
                        in_dev.as_deref(),
                        input_channel,
                        cancel_for_task,
                    );
                    Err::<sotf_audio::signal_recorder::SplCalibrationResult, String>(
                        "SPL calibration is not available on iOS".to_string(),
                    )
                }
            })
            .await;

            state_clone.update(&mut cx.clone(), |state, cx| {
                let cal = &mut state
                    .app
                    .measurement_state
                    .recording_state
                    .spl_calibration_capture;
                match result {
                    Ok(res) => cal.apply_engine_result(res),
                    Err(e) if e == sotf_audio::signal_recorder::CANCELLED_ERR => {
                        log::info!("SPL calibration capture cancelled by user");
                        cal.status = SplCalibrationCaptureStatus::Idle;
                    }
                    Err(e) => {
                        log::warn!("SPL calibration capture failed: {e}");
                        cal.status = SplCalibrationCaptureStatus::Failed(e);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Request cancellation of the in-progress SPL calibration capture.
    /// Mirrors `cancel_probe_capture` — the engine returns
    /// `Err(CANCELLED_ERR)` on the next poll.
    pub(crate) fn cancel_spl_calibration_capture(&mut self, cx: &mut Context<Self>) {
        log::info!("Cancel requested for SPL calibration capture");
        self.state.update(cx, |state, cx| {
            state
                .app
                .measurement_state
                .recording_state
                .spl_cancel_requested
                .store(true, std::sync::atomic::Ordering::Relaxed);
            cx.notify();
        });
    }
}
use crate::app::i18n::RecordingTranslations;
