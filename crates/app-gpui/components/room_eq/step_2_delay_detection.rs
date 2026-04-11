//! Room EQ wizard — Step 2: Tone-burst Delay Detection.
//!
//! Measures per-channel acoustic propagation delays with a narrowband
//! probe, then auto-feeds the results into the Room EQ optimizer via
//! `run_room_optimization_with_probe_arrivals`. Skipping this step is
//! always allowed — the optimizer falls back to WAV-onset detection.
//!
//! The shared business state lives in
//! [`sotf_audio_player::room_eq_types::DelayDetectionState`] so both the
//! TUI and the GPUI frontends hold the same measurement data. UI here is
//! intentionally minimal: three numeric form rows, a Run button, a
//! status line, and a results table. Full feature parity (device
//! pickers, override editors) is a follow-up.

use crate::app::theme::Theme;
use crate::app::types::room_eq::DelayDetectionStatus;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, StackAlign, StackSpacing, Text, TextSize,
    TextWeight, VStack,
};
use sotf_audio_player::room_eq_types::estimate_probe_sequence_ms;

impl PlayerView {
    /// Render the Delay Detection wizard step.
    pub(crate) fn render_room_eq_delay_detection(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let dd = state
            .app
            .measurement_state
            .room_eq_state
            .delay_detection
            .clone();
        let has_measurements = !state
            .app
            .measurement_state
            .room_eq_state
            .channel_measurements
            .is_empty();

        let running = matches!(dd.status, DelayDetectionStatus::Running { .. });

        // ── Form: probe_duration / silence / mic_channel ─────────────────
        let form = Card::new()
            .background(theme.surface)
            .border(theme.border)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new("Tone-Burst Delay Detection")
                            .weight(TextWeight::Bold)
                            .size(TextSize::Md),
                    )
                    .child(
                        Text::new(
                            "Measures per-channel acoustic delay with a narrowband probe. \
                             Results auto-feed into the optimizer. This step is optional — \
                             skip it to use WAV-onset detection instead.",
                        )
                        .size(TextSize::Xs)
                        .color(theme.text_secondary),
                    )
                    .child(number_row(
                        cx,
                        &theme,
                        "Probe duration (ms)",
                        dd.probe_duration_ms as f64,
                        100.0,
                        |dd, delta| {
                            dd.probe_duration_ms =
                                (dd.probe_duration_ms + delta).clamp(100.0, 5000.0);
                        },
                    ))
                    .child(number_row(
                        cx,
                        &theme,
                        "Silence gap (ms)",
                        dd.silence_duration_ms as f64,
                        100.0,
                        |dd, delta| {
                            dd.silence_duration_ms =
                                (dd.silence_duration_ms + delta).clamp(100.0, 5000.0);
                        },
                    ))
                    .child(number_row(
                        cx,
                        &theme,
                        "Mic input channel",
                        dd.input_channel as f64,
                        1.0,
                        |dd, delta| {
                            let next = dd.input_channel as i32 + delta as i32;
                            dd.input_channel = next.max(0) as u16;
                        },
                    ))
                    .child(
                        HStack::new().spacing(StackSpacing::Sm).child(
                            Button::new(
                                "dd_run",
                                if running {
                                    "Running..."
                                } else {
                                    "Run detection"
                                },
                            )
                            .variant(if running {
                                ButtonVariant::Secondary
                            } else {
                                ButtonVariant::Primary
                            })
                            .size(ButtonSize::Sm)
                            .theme(theme.to_button_theme())
                            .build()
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |view, _, _, cx| {
                                    if running || !has_measurements {
                                        return;
                                    }
                                    view.start_delay_detection(cx);
                                }),
                            ),
                        ),
                    ),
            );

        // ── Status banner ─────────────────────────────────────────────
        // See the TUI version's rationale — wall-clock elapsed / estimated
        // total, because the engine has no per-channel callback yet.
        let num_channels = self
            .state
            .read(cx)
            .app
            .measurement_state
            .room_eq_state
            .channel_measurements
            .len();
        let estimated_total =
            estimate_probe_sequence_ms(num_channels, dd.probe_duration_ms, dd.silence_duration_ms);
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let (status_text, status_color) = match &dd.status {
            DelayDetectionStatus::Idle => (
                "Idle — click Run detection to start".to_string(),
                theme.text_secondary,
            ),
            DelayDetectionStatus::Running { .. } => {
                let pct = dd
                    .status
                    .progress(estimated_total, now_ms)
                    .map(|p| format!("{:.0}%", p * 100.0))
                    .unwrap_or_else(|| "…".to_string());
                (format!("Running... {}", pct), theme.accent)
            }
            DelayDetectionStatus::Complete => (
                format!(
                    "Complete — detected {} channel(s)",
                    dd.results.as_ref().map(|r| r.channels.len()).unwrap_or(0)
                ),
                theme.success,
            ),
            DelayDetectionStatus::Failed(msg) => (format!("Failed: {}", msg), theme.error),
        };
        let status = Card::new()
            .background(theme.surface)
            .border(theme.border)
            .content(
                Text::new(status_text)
                    .size(TextSize::Sm)
                    .color(status_color),
            );

        // ── Results table ─────────────────────────────────────────────
        let results_block: AnyElement = if let Some(results) = dd.results.as_ref() {
            let header = HStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    Text::new("Channel")
                        .weight(TextWeight::Bold)
                        .size(TextSize::Xs),
                )
                .child(
                    Text::new("Arrival ms")
                        .weight(TextWeight::Bold)
                        .size(TextSize::Xs),
                )
                .child(
                    Text::new("Gain dB")
                        .weight(TextWeight::Bold)
                        .size(TextSize::Xs),
                )
                .child(
                    Text::new("SNR dB")
                        .weight(TextWeight::Bold)
                        .size(TextSize::Xs),
                )
                .child(
                    Text::new("Align ms")
                        .weight(TextWeight::Bold)
                        .size(TextSize::Xs),
                );

            let mut rows = VStack::new().spacing(StackSpacing::Xs).child(header);
            for (i, ch) in results.channels.iter().enumerate() {
                let snr_color = if ch.snr_db > 10.0 {
                    theme.success
                } else if ch.snr_db > 0.0 {
                    theme.accent
                } else {
                    theme.error
                };
                let arrival = dd
                    .edited_arrival_ms
                    .get(i)
                    .copied()
                    .unwrap_or(ch.arrival_ms);
                let alignment = results.alignment_delays_ms.get(i).copied().unwrap_or(0.0);
                rows = rows.child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .child(Text::new(ch.channel_name.clone()).size(TextSize::Xs))
                        .child(Text::new(format!("{:.2}", arrival)).size(TextSize::Xs))
                        .child(Text::new(format!("{:+.1}", ch.gain_db)).size(TextSize::Xs))
                        .child(
                            Text::new(format!("{:+.1}", ch.snr_db))
                                .size(TextSize::Xs)
                                .color(snr_color),
                        )
                        .child(Text::new(format!("{:.2}", alignment)).size(TextSize::Xs)),
                );
            }
            Card::new()
                .background(theme.surface)
                .border(theme.border)
                .content(rows)
                .into_any_element()
        } else {
            Card::new()
                .background(theme.surface)
                .border(theme.border)
                .content(
                    Text::new("No results yet — run detection to measure per-channel delays.")
                        .size(TextSize::Xs)
                        .color(theme.text_secondary),
                )
                .into_any_element()
        };

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(form)
            .child(status)
            .child(results_block)
    }

    /// Spawn the delay-detection measurement on a blocking worker.
    ///
    /// Builds the channel list from the loaded measurements, sets the
    /// status to Running, then runs `probe_channel_delays` via
    /// `smol::unblock` so the GPUI thread stays responsive. On
    /// completion, `DelayDetectionState::apply_results` stores the
    /// engine's result (no translation needed — `DelayProbeResults` is
    /// a type alias for the engine's `ProbeDelayResults`).
    pub(crate) fn start_delay_detection(&mut self, cx: &mut Context<Self>) {
        // Snapshot the form inputs and the channel list from state so the
        // worker thread owns its inputs and doesn't borrow the UI state.
        let (
            probe_ms,
            silence_ms,
            sample_rate,
            input_ch,
            channel_indices,
            channel_names,
            out_dev,
            in_dev,
        ) = {
            let state = self.state.read(cx);
            let room_eq = &state.app.measurement_state.room_eq_state;
            let names: Vec<String> = room_eq
                .channel_measurements
                .iter()
                .map(|m| m.channel_name.clone())
                .collect();
            if names.is_empty() {
                log::warn!("Delay detection: no measurements loaded — cannot determine channels");
                return;
            }
            // 0..N is a best-effort hardware channel map; see the TUI
            // twin for the same limitation and the plumbing story.
            let indices: Vec<u16> = (0..names.len() as u16).collect();
            let dd = &room_eq.delay_detection;
            (
                dd.probe_duration_ms,
                dd.silence_duration_ms,
                dd.sample_rate,
                dd.input_channel,
                indices,
                names,
                dd.output_device_name.clone(),
                dd.input_device_name.clone(),
            )
        };

        let started_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        // Mark running. Preserve any existing results until the new
        // measurement completes — `apply_results` replaces them atomically.
        self.state.update(cx, |state, cx| {
            let dd = &mut state.app.measurement_state.room_eq_state.delay_detection;
            dd.status = DelayDetectionStatus::Running { started_at_ms };
            dd.results = None;
            dd.edited_arrival_ms.clear();
            cx.notify();
        });

        let state_clone = self.state.clone();
        cx.spawn(async move |_, cx| {
            let result = smol::unblock(move || {
                sotf_audio::signal_recorder::probe_channel_delays(
                    &channel_indices,
                    &channel_names,
                    sample_rate,
                    probe_ms,
                    silence_ms,
                    out_dev.as_deref(),
                    in_dev.as_deref(),
                    input_ch,
                )
            })
            .await;

            state_clone.update(&mut cx.clone(), |state, cx| {
                let dd = &mut state.app.measurement_state.room_eq_state.delay_detection;
                match result {
                    Ok(results) => dd.apply_results(results),
                    Err(e) => {
                        log::warn!("Delay detection failed: {}", e);
                        dd.status = DelayDetectionStatus::Failed(e);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

/// A labelled numeric form row with `-` / `+` buttons.
///
/// The listener receives a `delta` (the increment amount, signed) and a
/// mutable borrow of the shared `DelayDetectionState` so it can clamp
/// the updated value to that field's valid range. The `Fn` bound means
/// each button listener only holds a single copy — no per-click clone.
fn number_row(
    cx: &mut Context<PlayerView>,
    theme: &Theme,
    label: &'static str,
    current: f64,
    step: f32,
    apply: impl Fn(&mut sotf_audio_player::room_eq_types::DelayDetectionState, f32)
    + Send
    + 'static
    + Clone,
) -> impl IntoElement {
    // Two owned copies — one for each listener. The closures are `Fn`
    // so each invocation just calls `apply(...)` directly; there's no
    // per-click clone.
    let apply_plus = apply.clone();
    let apply_minus = apply;
    HStack::new()
        .spacing(StackSpacing::Md)
        .align(StackAlign::Center)
        .child(Text::new(label).size(TextSize::Xs))
        .child(
            Text::new(format!("{:.0}", current))
                .size(TextSize::Xs)
                .weight(TextWeight::Bold),
        )
        .child(
            Button::new(format!("{label}_minus"), "−")
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Xs)
                .theme(theme.to_button_theme())
                .build()
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |view, _, _, cx| {
                        view.state.update(cx, |state, _| {
                            apply_minus(
                                &mut state.app.measurement_state.room_eq_state.delay_detection,
                                -step,
                            );
                        });
                        cx.notify();
                    }),
                ),
        )
        .child(
            Button::new(format!("{label}_plus"), "+")
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Xs)
                .theme(theme.to_button_theme())
                .build()
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |view, _, _, cx| {
                        view.state.update(cx, |state, _| {
                            apply_plus(
                                &mut state.app.measurement_state.room_eq_state.delay_detection,
                                step,
                            );
                        });
                        cx.notify();
                    }),
                ),
        )
}
