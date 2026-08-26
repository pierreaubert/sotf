use super::misc::PROBE_SIGNAL_BOOST_DB;
use super::misc::probe_number_row;
use crate::app::types::recording::ProbeCaptureStatus;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, Column, HStack, Progress, ProgressSize, Spinner,
    SpinnerSize, StackAlign, StackSpacing, Table, TableTheme, Text, TextSize, TextWeight, VStack,
};
use sotf_audio_player::room_eq_types::estimate_probe_sequence_ms;

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

impl PlayerView {
    /// Render the Probe step UI.
    pub(crate) fn render_recording_probe_step(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let translations = state.app.ui_state.translations.clone();
        let recording_text = crate::app::i18n::RecordingWorkflowTranslations::for_language(
            state.app.ui_state.language,
        );
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
                        Text::new(translations.recording_probe_delay)
                            .weight(TextWeight::Bold)
                            .size(TextSize::Md),
                    )
                    .child(
                        Text::new(recording_text.probe_description)
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
                    .child(HStack::new().spacing(StackSpacing::Sm).child(if running {
                        dev_track!(
                            Button::new("probe_cancel", translations.general_cancel)
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Sm)
                                .theme(theme.to_button_theme())
                                .on_click_event(cx.listener(|view, _, _, cx| {
                                    view.cancel_probe_capture(cx);
                                })),
                            "recording.probe_cancel"
                        )
                    } else {
                        dev_track!(
                            Button::new("probe_run", translations.recording_run_probe)
                                .variant(ButtonVariant::Primary)
                                .size(ButtonSize::Sm)
                                .theme(theme.to_button_theme())
                                .on_click_event(cx.listener(move |view, _, _, cx| {
                                    if !has_capture {
                                        return;
                                    }
                                    view.start_probe_capture(cx);
                                })),
                            "recording.probe_run"
                        )
                    })),
            );
        let _ = view; // silence unused warning if the closure didn't capture

        // --- Status banner -------------------------------------------
        // For the Running state, capture the elapsed-vs-estimated fraction so
        // a real Progress bar can be rendered next to the "Running… NN%"
        // caption. The previous UI showed only the caption — no bar — so
        // users couldn't tell at a glance how much was left.
        let running_progress = match &pc.status {
            ProbeCaptureStatus::Running { .. } => pc.status.progress(estimated_total, now_ms),
            _ => None,
        };
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
                let pct = running_progress
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
            .content({
                let mut content = VStack::new().spacing(StackSpacing::Xs).child(
                    Text::new(status_text)
                        .size(TextSize::Sm)
                        .color(status_color),
                );
                if let Some(p) = running_progress {
                    // Determinate Progress — wall-clock fraction. The
                    // estimate can drift if the probe sequence runs longer
                    // than predicted, so we clamp to 0..=1.0 in the
                    // Progress component itself.
                    content =
                        content.child(Progress::new(p.clamp(0.0, 1.0)).size(ProgressSize::Sm));
                } else if matches!(pc.status, ProbeCaptureStatus::Running { .. }) {
                    // No fraction available yet (e.g. estimated_total=0) —
                    // show an indeterminate Spinner so the user sees the
                    // probe is still alive.
                    content = content.child(Spinner::new().size(SpinnerSize::Sm));
                }
                content
            });

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
                    Text::new(recording_text.no_probe_captured)
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
        #[cfg(feature = "dev-api")]
        if self.complete_qa_fake_probe_capture(cx) {
            return;
        }

        // Snapshot the device config + form inputs outside the async
        // closure so the worker thread doesn't hold a borrow on the
        // GPUI state.
        let (
            probe_ms,
            silence_ms,
            sample_rate,
            input_channel,
            signal_level_db,
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
            // Probe one signal per *speaker output channel*, not per
            // (speaker × position × mic) entry in `channel_recordings`.
            // The latter multiplies the channel count well beyond the
            // physical layout (e.g. 9.1.6 × 2 mic positions × 1 mic =
            // 32 entries for a 16-speaker setup) and tries to address
            // hardware outputs that don't exist.
            let mappings = &rec.playback_config.channel_mappings;
            let names: Vec<String> = mappings.iter().map(|m| m.group_name.clone()).collect();
            let indices: Vec<u16> = mappings
                .iter()
                .map(|m| m.interface_channel() as u16)
                .collect();
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
                rec.signal_level_db + PROBE_SIGNAL_BOOST_DB,
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

        let (capture_generation, cancel_flag) = self.state.update(cx, |state, cx| {
            let rec = &mut state.app.measurement_state.recording_state;
            rec.probe_cancel_requested
                .store(false, std::sync::atomic::Ordering::Relaxed);
            let capture_generation = rec.probe_capture.next_capture_generation();
            rec.probe_capture.status = ProbeCaptureStatus::Running { started_at_ms };
            rec.probe_capture.results = None;
            cx.notify();
            (capture_generation, rec.probe_cancel_requested.clone())
        });

        let state_clone = self.state.clone();
        let wav_path_for_state = wav_path.clone();
        let cancel_for_task = cancel_flag.clone();
        cx.spawn(async move |_, cx| {
            let result = smol::unblock(move || {
                #[cfg(not(target_os = "ios"))]
                {
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
                        signal_level_db,
                        Some(cancel_for_task),
                    )
                }
                #[cfg(target_os = "ios")]
                {
                    let _ = (
                        &channel_indices,
                        &channel_names,
                        sample_rate,
                        probe_ms,
                        silence_ms,
                        out_dev.as_deref(),
                        in_dev.as_deref(),
                        input_channel,
                        signal_level_db,
                        &wav_path,
                        cancel_for_task,
                    );
                    Err::<sotf_audio::signal_recorder::ProbeDelayResults, String>(
                        "Probe capture is not available on iOS".to_string(),
                    )
                }
            })
            .await;

            state_clone.update(&mut cx.clone(), |state, cx| {
                let rec = &mut state.app.measurement_state.recording_state;
                if !rec.probe_capture.is_current_capture(capture_generation) {
                    log::info!(
                        "Discarding stale GPUI probe result (generation {}, current {})",
                        capture_generation,
                        rec.probe_capture.capture_generation
                    );
                    return;
                }
                let pc = &mut rec.probe_capture;
                match result {
                    Ok(results) => {
                        pc.apply_results(
                            results,
                            Some(wav_path_for_state.to_string_lossy().to_string()),
                        );
                    }
                    Err(e) if e == sotf_audio::signal_recorder::CANCELLED_ERR => {
                        log::info!("Probe capture cancelled by user");
                        // Reset to Idle so the UI shows the start button
                        // again rather than a red Failed banner.
                        pc.status = ProbeCaptureStatus::Idle;
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

    /// Complete the QA probe from deterministic fixture data only after the
    /// visible Run Probe control has been activated.
    #[cfg(feature = "dev-api")]
    fn complete_qa_fake_probe_capture(&mut self, cx: &mut Context<Self>) -> bool {
        let completed = self.state.update(cx, |state, cx| {
            let rec = &mut state.app.measurement_state.recording_state;
            if rec.qa_fake_capture.is_none() {
                return false;
            }
            let channels = rec
                .playback_config
                .channel_mappings
                .iter()
                .enumerate()
                .map(
                    |(index, mapping)| sotf_audio::signal_recorder::ProbeDelayChannelResult {
                        channel_name: mapping.group_name.clone(),
                        channel_index: mapping.interface_channel(),
                        arrival_ms: 4.0 + index as f64 * 0.75,
                        gain_db: -(index as f64 * 0.25),
                        snr_db: 36.0 - index as f64,
                    },
                )
                .collect::<Vec<_>>();
            let alignment_delays_ms = channels
                .iter()
                .map(|channel| (channels[0].arrival_ms - channel.arrival_ms).max(0.0))
                .collect();
            let sample_rate = rec.probe_capture.sample_rate;
            rec.probe_capture.apply_results(
                sotf_audio::signal_recorder::ProbeDelayResults {
                    channels,
                    sample_rate,
                    alignment_delays_ms,
                },
                None,
            );
            cx.notify();
            true
        });
        completed
    }

    /// Request cancellation of an in-progress probe capture. The engine
    /// honors the flag at its next stability poll (~50 ms latency) and
    /// returns `Err(CANCELLED_ERR)`, which the runner maps back to
    /// `ProbeCaptureStatus::Idle`.
    pub(crate) fn cancel_probe_capture(&mut self, cx: &mut Context<Self>) {
        log::info!("Cancel requested for probe capture");
        self.state.update(cx, |state, cx| {
            state
                .app
                .measurement_state
                .recording_state
                .probe_cancel_requested
                .store(true, std::sync::atomic::Ordering::Relaxed);
            cx.notify();
        });
    }
}

/// Row data for the probe results `Table`. Each field is pre-formatted
/// as a display string so the `cell_render` closures just clone.
#[derive(Clone)]
struct ProbeResultRow {
    pub(super) channel: String,
    pub(super) arrival_ms: String,
    pub(super) gain_db: String,
    /// Raw SNR value — used for colour selection inside `cell_render`.
    pub(super) snr_db: f64,
    /// Pre-formatted SNR text (e.g. "+14.2").
    pub(super) snr_text: String,
    pub(super) align_ms: String,
}
