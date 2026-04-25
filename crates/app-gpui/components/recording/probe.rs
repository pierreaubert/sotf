//! Recording wizard — Step 3: Tone-burst Delay Probe.
//!
//! Plays a narrowband 800–2000 Hz tone burst sequentially on each
//! output channel captured in the Capture step, records the result
//! via the microphone, and analyses arrival / gain / SNR via FFT
//! cross-correlation. Unlike the Room EQ "Delay Detection" step, this
//! one runs *while the mic is still set up* right after sweeps, and
//! persists both the raw probe WAV and the analysed results into the
//! recording session JSON so downstream tools (Room EQ, convert_recording,
//! etc.) can pick them up at load time with no extra action from the user.
//!
//! All state lives in [`ProbeCaptureState`] on the shared `RecordingState`
//! — this file only adds the render + spawn glue. Render-time mutations
//! that can't be done safely (they would re-enter the GPUI update cycle
//! and panic with `cannot update PlayerView while it is already being
//! updated`) are wrapped in `cx.defer`.

use crate::app::theme::Theme;
use crate::app::types::recording::{ProbeCaptureStatus, RecordingState};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, Column, HStack, StackAlign, StackSpacing, Table,
    TableTheme, Text, TextSize, TextWeight, VStack,
};
use sotf_audio_player::room_eq_types::estimate_probe_sequence_ms;

impl PlayerView {
    /// Render the Probe step UI.
    pub(crate) fn render_recording_probe_step(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let rec = &state.app.measurement_state.recording_state;
        let pc = rec.probe_capture.clone();
        let channel_count = rec.channel_recordings.len();
        let has_capture = channel_count > 0;
        let running = matches!(pc.status, ProbeCaptureStatus::Running { .. });

        // Progress estimate — wall-clock elapsed / predicted total.
        // Same pattern as the Room EQ Delay Detection step.
        let estimated_total =
            estimate_probe_sequence_ms(channel_count, pc.probe_duration_ms, pc.silence_duration_ms);
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        // --- Form -----------------------------------------------------
        let view = cx.entity().clone();
        let form = Card::new()
            .background(theme.surface)
            .border(theme.border)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new("Tone-Burst Delay Probe")
                            .weight(TextWeight::Bold)
                            .size(TextSize::Md),
                    )
                    .child(
                        Text::new(
                            "Measures per-channel acoustic delay with a narrowband probe. \
                             Results and the raw WAV are saved alongside the sweep \
                             recordings so the Room EQ optimizer can use them automatically.",
                        )
                        .size(TextSize::Xs)
                        .color(theme.text_secondary),
                    )
                    .child(probe_number_row(
                        cx,
                        &theme,
                        "Probe duration (ms)",
                        pc.probe_duration_ms as f64,
                        100.0,
                        |rec, delta| {
                            rec.probe_capture.probe_duration_ms =
                                (rec.probe_capture.probe_duration_ms + delta).clamp(100.0, 5000.0);
                        },
                    ))
                    .child(probe_number_row(
                        cx,
                        &theme,
                        "Silence gap (ms)",
                        pc.silence_duration_ms as f64,
                        100.0,
                        |rec, delta| {
                            rec.probe_capture.silence_duration_ms =
                                (rec.probe_capture.silence_duration_ms + delta)
                                    .clamp(100.0, 5000.0);
                        },
                    ))
                    .child(probe_number_row(
                        cx,
                        &theme,
                        "Mic input channel",
                        pc.input_channel as f64,
                        1.0,
                        |rec, delta| {
                            let next = rec.probe_capture.input_channel as i32 + delta as i32;
                            rec.probe_capture.input_channel = next.max(0) as u16;
                        },
                    ))
                    .child(
                        HStack::new().spacing(StackSpacing::Sm).child(
                            Button::new(
                                "probe_run",
                                if running { "Running..." } else { "Run Probe" },
                            )
                            .variant(if running {
                                ButtonVariant::Secondary
                            } else {
                                ButtonVariant::Primary
                            })
                            .size(ButtonSize::Sm)
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(move |view, _, _, cx| {
                                    if running || !has_capture {
                                        return;
                                    }
                                    view.start_probe_capture(cx);
                                })),
                        ),
                    ),
            );
        let _ = view; // silence unused warning if the closure didn't capture

        // --- Status banner -------------------------------------------
        let (status_text, status_color) = match &pc.status {
            ProbeCaptureStatus::Idle => (
                if has_capture {
                    "Idle — click Run Probe to measure per-channel delays".to_string()
                } else {
                    "Record some sweeps first (Capture step)".to_string()
                },
                theme.text_secondary,
            ),
            ProbeCaptureStatus::Running { .. } => {
                let pct = pc
                    .status
                    .progress(estimated_total, now_ms)
                    .map(|p| format!("{:.0}%", p * 100.0))
                    .unwrap_or_else(|| "…".to_string());
                (format!("Running... {}", pct), theme.accent)
            }
            ProbeCaptureStatus::Complete => (
                format!(
                    "Complete — detected {} channel(s)",
                    pc.results.as_ref().map(|r| r.channels.len()).unwrap_or(0)
                ),
                theme.success,
            ),
            ProbeCaptureStatus::Failed(msg) => (format!("Failed: {}", msg), theme.error),
        };
        let status = Card::new()
            .background(theme.surface)
            .border(theme.border)
            .content(
                Text::new(status_text)
                    .size(TextSize::Sm)
                    .color(status_color),
            );

        // --- Results + persisted WAV path -----------------------------
        // --- Results table -----------------------------------------------
        let results_block: AnyElement = if let Some(results) = pc.results.as_ref() {
            let text_color = theme.text_primary;
            let success_color = theme.success;
            let accent_color = theme.accent;
            let error_color = theme.error;

            let table_rows: Vec<ProbeResultRow> = results
                .channels
                .iter()
                .enumerate()
                .map(|(i, ch)| ProbeResultRow {
                    channel: ch.channel_name.clone(),
                    arrival_ms: format!("{:.2}", ch.arrival_ms),
                    gain_db: format!("{:+.1}", ch.gain_db),
                    snr_db: ch.snr_db,
                    snr_text: format!("{:+.1}", ch.snr_db),
                    align_ms: format!(
                        "{:.2}",
                        results.alignment_delays_ms.get(i).copied().unwrap_or(0.0)
                    ),
                })
                .collect();

            let table_theme = TableTheme {
                header_bg: theme.background_secondary,
                header_text: theme.text_primary,
                header_border: theme.border,
                row_bg: theme.surface,
                row_alt_bg: theme.background_secondary,
                row_hover_bg: theme.surface_hover,
                row_selected_bg: theme.accent_muted,
                cell_text: theme.text_secondary,
                cell_border: theme.border,
                sort_icon_color: theme.accent,
                ..Default::default()
            };

            let mut content = VStack::new().spacing(StackSpacing::Sm).child(
                Table::new("probe-results-table", table_rows)
                    .column(
                        Column::new("channel", "Channel")
                            .width(px(100.0)) // intentional: fixed probe-results column
                            .sortable(false)
                            .resizable(false)
                            .cell_render(move |row: &ProbeResultRow, _, _, _| {
                                Text::label(row.channel.clone()).color(text_color)
                            }),
                    )
                    .column(
                        Column::new("arrival", "Arrival (ms)")
                            .width(px(110.0)) // intentional: fixed probe-results column
                            .sortable(false)
                            .resizable(false)
                            .cell_render(move |row: &ProbeResultRow, _, _, _| {
                                Text::new(row.arrival_ms.clone())
                                    .size(TextSize::Xs)
                                    .color(text_color)
                            }),
                    )
                    .column(
                        Column::new("gain", "Gain (dB)")
                            .width(px(100.0)) // intentional: fixed probe-results column
                            .sortable(false)
                            .resizable(false)
                            .cell_render(move |row: &ProbeResultRow, _, _, _| {
                                Text::new(row.gain_db.clone())
                                    .size(TextSize::Xs)
                                    .color(text_color)
                            }),
                    )
                    .column(
                        Column::new("snr", "SNR (dB)")
                            .width(px(100.0)) // intentional: fixed probe-results column
                            .sortable(false)
                            .resizable(false)
                            .cell_render(move |row: &ProbeResultRow, _, _, _| {
                                let color = if row.snr_db > 10.0 {
                                    success_color
                                } else if row.snr_db > 0.0 {
                                    accent_color
                                } else {
                                    error_color
                                };
                                Text::label(row.snr_text.clone()).color(color)
                            }),
                    )
                    .column(
                        Column::new("align", "Align (ms)")
                            .width(px(110.0)) // intentional: fixed probe-results column
                            .sortable(false)
                            .resizable(false)
                            .cell_render(move |row: &ProbeResultRow, _, _, _| {
                                Text::new(row.align_ms.clone())
                                    .size(TextSize::Xs)
                                    .color(text_color)
                            }),
                    )
                    .alternating_rows(true)
                    .theme(table_theme),
            );
            if let Some(wav) = pc.wav_path.clone() {
                content = content.child(Text::caption(format!("Recording saved to: {}", wav)));
            }
            Card::new()
                .background(theme.surface)
                .border(theme.border)
                .content(content)
                .into_any_element()
        } else {
            Card::new()
                .background(theme.surface)
                .border(theme.border)
                .content(
                    Text::new(
                        "No probe captured yet — run the probe to measure per-channel delays.",
                    )
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
                )
                .into_any_element()
        };

        VStack::new()
            .spacing(StackSpacing::Md)
            .align(StackAlign::Stretch)
            .child(form)
            .child(status)
            .child(results_block)
    }

    /// Spawn the tone-burst probe on a blocking worker via
    /// `smol::unblock`. Reuses the Recording wizard's playback /
    /// recording device configuration and writes the raw mono mic
    /// buffer to `<recording_directory>/probe_all_channels.wav` via
    /// `sotf_audio::signal_recorder::probe_channel_delays_with_recording`.
    pub(crate) fn start_probe_capture(&mut self, cx: &mut Context<Self>) {
        // Snapshot the device config + form inputs outside the async
        // closure so the worker thread doesn't hold a borrow on the
        // GPUI state.
        let (
            probe_ms,
            silence_ms,
            sample_rate,
            input_channel,
            channel_indices,
            channel_names,
            out_dev,
            in_dev,
            wav_path,
        ) = {
            let state = self.state.read(cx);
            let rec = &state.app.measurement_state.recording_state;
            if rec.channel_recordings.is_empty() {
                log::warn!("Probe capture: no channel recordings — run Capture step first");
                return;
            }
            let names: Vec<String> = rec
                .channel_recordings
                .iter()
                .map(|c| c.channel_name.clone())
                .collect();
            let indices: Vec<u16> = (0..names.len() as u16).collect();
            let dir = rec
                .recording_directory
                .clone()
                .unwrap_or_else(|| ".".to_string());
            let wav = std::path::PathBuf::from(&dir).join("probe_all_channels.wav");
            (
                rec.probe_capture.probe_duration_ms,
                rec.probe_capture.silence_duration_ms,
                rec.probe_capture.sample_rate,
                rec.probe_capture.input_channel,
                indices,
                names,
                Some(rec.playback_config.device_name.clone()),
                Some(rec.recording_config.device_name.clone()),
                wav,
            )
        };

        let started_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        self.state.update(cx, |state, cx| {
            let pc = &mut state.app.measurement_state.recording_state.probe_capture;
            pc.status = ProbeCaptureStatus::Running { started_at_ms };
            pc.results = None;
            cx.notify();
        });

        let state_clone = self.state.clone();
        let wav_path_for_state = wav_path.clone();
        cx.spawn(async move |_, cx| {
            let result = smol::unblock(move || {
                sotf_audio::signal_recorder::probe_channel_delays_with_recording(
                    &channel_indices,
                    &channel_names,
                    sample_rate,
                    probe_ms,
                    silence_ms,
                    out_dev.as_deref(),
                    in_dev.as_deref(),
                    input_channel,
                    &wav_path,
                )
            })
            .await;

            state_clone.update(&mut cx.clone(), |state, cx| {
                let pc = &mut state.app.measurement_state.recording_state.probe_capture;
                match result {
                    Ok(results) => {
                        pc.apply_results(
                            results,
                            Some(wav_path_for_state.to_string_lossy().to_string()),
                        );
                    }
                    Err(e) => {
                        log::warn!("Probe capture failed: {}", e);
                        pc.status = ProbeCaptureStatus::Failed(e);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

/// Labelled numeric row with `-` / `+` buttons. Mirrors the helper in
/// `components/room_eq/step_2_delay_detection.rs` but mutates the
/// recording state rather than the Room EQ state. Closures are `Fn`
/// so no per-click clone is needed.
fn probe_number_row(
    cx: &mut Context<PlayerView>,
    theme: &Theme,
    label: &'static str,
    current: f64,
    step: f32,
    apply: impl Fn(&mut RecordingState, f32) + Send + 'static + Clone,
) -> impl IntoElement {
    let apply_plus = apply.clone();
    let apply_minus = apply;
    HStack::new()
        .spacing(StackSpacing::Md)
        .align(StackAlign::Center)
        .child(Text::new(label).size(TextSize::Xs))
        .child(
            // intentional: numeric value emphasis in stepper, not a kicker label
            Text::new(format!("{:.0}", current))
                .size(TextSize::Xs)
                .weight(TextWeight::Bold),
        )
        .child(
            Button::new(format!("{label}_minus"), "−")
                .aria_label(format!("Decrease {label}"))
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Xs)
                .theme(theme.to_button_theme())
                .on_click_event(cx.listener(move |view, _, _, cx| {
                        view.state.update(cx, |state, _| {
                            apply_minus(&mut state.app.measurement_state.recording_state, -step);
                        });
                        cx.notify();
                    })),
        )
        .child(
            Button::new(format!("{label}_plus"), "+")
                .aria_label(format!("Increase {label}"))
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Xs)
                .theme(theme.to_button_theme())
                .on_click_event(cx.listener(move |view, _, _, cx| {
                        view.state.update(cx, |state, _| {
                            apply_plus(&mut state.app.measurement_state.recording_state, step);
                        });
                        cx.notify();
                    })),
        )
}

/// Row data for the probe results `Table`. Each field is pre-formatted
/// as a display string so the `cell_render` closures just clone.
#[derive(Clone)]
struct ProbeResultRow {
    channel: String,
    arrival_ms: String,
    gain_db: String,
    /// Raw SNR value — used for colour selection inside `cell_render`.
    snr_db: f64,
    /// Pre-formatted SNR text (e.g. "+14.2").
    snr_text: String,
    align_ms: String,
}
