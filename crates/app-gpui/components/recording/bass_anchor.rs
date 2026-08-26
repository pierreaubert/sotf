//! Recording wizard — Step 4: Bass Anchor (GD-Opt v2 Phase GD-1e).
//!
//! Plays a low-frequency Hann-windowed tone burst sequentially on
//! each output channel, records the mic, and extracts the per-channel
//! phase + stability of the burst's fundamental via a single-bin DFT.
//! The result anchors the sweep-derived phase at the first bin,
//! eliminating the 2π wraparound ambiguity that plagues log-sweep
//! bass measurements (§2.6 of the GD-Opt v2 plan,
//! `docs/gd_opt_v2_plan.md` in the autoeq repo).
//!
//! Mirrors the Probe step (`probe.rs`): status line, Run/Cancel
//! control, per-channel results table once Complete.

use crate::app::theme::Theme;
use crate::app::types::recording::{BassAnchorCaptureStatus, RecordingState};
use crate::ui::PlayerView;
use gpui::{Context, IntoElement};
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, StackSpacing, Text, TextSize, TextWeight,
    VStack,
};

const BASS_ANCHOR_SIGNAL_BOOST_DB: f32 = 10.0;

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
    /// Render the BassAnchor step UI.
    pub(crate) fn render_recording_bass_anchor_step(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let recording_text = crate::app::i18n::RecordingWorkflowTranslations::for_language(
            state.app.ui_state.language,
        );
        let rec = &state.app.measurement_state.recording_state;
        let bac = rec.bass_anchor_capture.clone();
        let has_speakers = !rec.playback_config.channel_mappings.is_empty();
        let running = matches!(bac.status, BassAnchorCaptureStatus::Running { .. });
        let loopback_channel = rec.recording_config.ctc_loopback_input_channel;
        let loopback_hint = match loopback_channel {
            Some(ch) => format!(" + loopback ref ch {}", ch),
            None => String::new(),
        };

        let status_line = match &bac.status {
            BassAnchorCaptureStatus::Idle => {
                format!(
                    "Not started — {:.1} Hz × {:.1} s ({} sub-windows{})",
                    bac.bass_freq_hz, bac.bass_duration_s, bac.num_windows, loopback_hint
                )
            }
            BassAnchorCaptureStatus::Running { .. } => "Capturing bass anchor…".to_string(),
            BassAnchorCaptureStatus::Complete => "Complete".to_string(),
            BassAnchorCaptureStatus::Failed(e) => format!("Failed: {e}"),
        };

        let run_button = if running {
            dev_track!(
                Button::new("bass_anchor_cancel", translations.general_cancel)
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Sm)
                    .theme(theme.to_button_theme())
                    .on_click_event(cx.listener(|view, _, _, cx| {
                        view.cancel_bass_anchor_capture(cx);
                    })),
                "recording.bass_anchor_cancel"
            )
        } else {
            dev_track!(
                Button::new("bass_anchor_run", recording_text.run_bass_anchor)
                    .variant(ButtonVariant::Primary)
                    .size(ButtonSize::Sm)
                    .theme(theme.to_button_theme())
                    .on_click_event(cx.listener(move |view, _, _, cx| {
                        if !has_speakers {
                            return;
                        }
                        view.start_bass_anchor_capture(cx);
                    })),
                "recording.bass_anchor_run"
            )
        };

        let mut column = VStack::new()
            .spacing(StackSpacing::Sm)
            .child(
                Text::new(translations.recording_bass_anchor)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Md),
            )
            .child(Text::new(recording_text.bass_anchor_description).size(TextSize::Sm))
            .child(bass_anchor_number_row(
                cx,
                &theme,
                "Frequency (Hz)",
                bac.bass_freq_hz as f64,
                5.0,
                |rec, delta| {
                    rec.bass_anchor_capture.bass_freq_hz =
                        (rec.bass_anchor_capture.bass_freq_hz + delta).clamp(20.0, 120.0);
                },
            ))
            .child(bass_anchor_number_row(
                cx,
                &theme,
                "Duration (s)",
                bac.bass_duration_s as f64,
                0.5,
                |rec, delta| {
                    rec.bass_anchor_capture.bass_duration_s =
                        (rec.bass_anchor_capture.bass_duration_s + delta).clamp(0.5, 10.0);
                },
            ))
            .child(bass_anchor_number_row(
                cx,
                &theme,
                "Sub-windows",
                bac.num_windows as f64,
                1.0,
                |rec, delta| {
                    let next = rec.bass_anchor_capture.num_windows as i32 + delta as i32;
                    rec.bass_anchor_capture.num_windows = next.clamp(4, 16) as u16;
                },
            ))
            .child(Text::new(status_line).size(TextSize::Sm))
            .child(HStack::new().spacing(StackSpacing::Sm).child(run_button));

        if let Some(results) = bac.results.as_ref() {
            column = column.child(
                Text::new(format!(
                    "Channels captured: {} @ {} Hz",
                    results.channels.len(),
                    results.sample_rate
                ))
                .size(TextSize::Sm),
            );
            for ch in &results.channels {
                let reliable = ch.bass_anchor_stability_deg < 20.0;
                let lb_part = match (ch.bass_anchor_loopback_phase_deg, ch.bass_anchor_coherence) {
                    (Some(lb), Some(coh)) => format!(", lb {:+.1}°, γ² {:.3}", lb, coh),
                    _ => String::new(),
                };
                let line = format!(
                    "  {} — phase {:+.1}°, |mag| {:.3}, stability {:.1}°{}{}",
                    ch.channel_name,
                    ch.bass_anchor_phase_deg,
                    ch.bass_anchor_magnitude,
                    ch.bass_anchor_stability_deg,
                    lb_part,
                    if reliable {
                        ""
                    } else {
                        "  ⚠ unreliable (> 20°)"
                    }
                );
                column = column.child(Text::new(line).size(TextSize::Sm));
            }
        }

        Card::new()
            .background(theme.surface)
            .border(theme.border)
            .content(column)
    }

    /// Spawn the bass-anchor tone-burst capture on a `smol::unblock`
    /// worker. Mirrors `start_probe_capture` (probe.rs) — derives the
    /// channel list from `playback_config.channel_mappings`, sets the
    /// status to Running, and on completion calls `apply_results`.
    pub(crate) fn start_bass_anchor_capture(&mut self, cx: &mut Context<Self>) {
        #[cfg(feature = "dev-api")]
        if self.complete_qa_fake_bass_anchor_capture(cx) {
            return;
        }

        let (
            bass_freq_hz,
            bass_duration_s,
            fade_ms,
            num_windows,
            silence_ms,
            sample_rate,
            input_channel,
            loopback_input_channel,
            signal_level_db,
            channel_indices,
            channel_names,
            out_dev,
            in_dev,
            wav_path,
        ) = {
            let state = self.state.read(cx);
            let rec = &state.app.measurement_state.recording_state;
            let mappings = &rec.playback_config.channel_mappings;
            if mappings.is_empty() {
                log::warn!("Bass anchor capture: no speaker channels configured");
                return;
            }
            let names: Vec<String> = mappings.iter().map(|m| m.group_name.clone()).collect();
            let indices: Vec<u16> = mappings
                .iter()
                .map(|m| m.interface_channel() as u16)
                .collect();
            let dir = rec
                .recording_directory
                .clone()
                .unwrap_or_else(|| ".".to_string());
            let wav = std::path::PathBuf::from(&dir).join("bass_anchor_all_channels.wav");
            (
                rec.bass_anchor_capture.bass_freq_hz,
                rec.bass_anchor_capture.bass_duration_s,
                rec.bass_anchor_capture.fade_ms,
                rec.bass_anchor_capture.num_windows,
                rec.bass_anchor_capture.silence_duration_ms,
                rec.bass_anchor_capture.sample_rate,
                rec.bass_anchor_capture.input_channel,
                rec.recording_config
                    .ctc_loopback_input_channel
                    .and_then(|c| match u16::try_from(c) {
                        Ok(v) => Some(v),
                        Err(_) => {
                            log::warn!(
                                "Loopback input channel {c} exceeds u16::MAX — bass anchor will run without loopback reference",
                            );
                            None
                        }
                    }),
                rec.signal_level_db + BASS_ANCHOR_SIGNAL_BOOST_DB,
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
            rec.bass_anchor_cancel_requested
                .store(false, std::sync::atomic::Ordering::Relaxed);
            let capture_generation = rec.bass_anchor_capture.next_capture_generation();
            rec.bass_anchor_capture.status = BassAnchorCaptureStatus::Running { started_at_ms };
            rec.bass_anchor_capture.results = None;
            cx.notify();
            (capture_generation, rec.bass_anchor_cancel_requested.clone())
        });

        let state_clone = self.state.clone();
        let wav_path_for_state = wav_path.clone();
        let cancel_for_task = cancel_flag.clone();
        cx.spawn(async move |_, cx| {
            let result = smol::unblock(move || {
                #[cfg(not(target_os = "ios"))]
                {
                    sotf_audio::signal_recorder::run_bass_anchor_with_recording(
                        &channel_indices,
                        &channel_names,
                        sample_rate,
                        bass_freq_hz,
                        bass_duration_s,
                        fade_ms,
                        num_windows,
                        silence_ms,
                        out_dev.as_deref(),
                        in_dev.as_deref(),
                        input_channel,
                        loopback_input_channel,
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
                        bass_freq_hz,
                        bass_duration_s,
                        fade_ms,
                        num_windows,
                        silence_ms,
                        out_dev.as_deref(),
                        in_dev.as_deref(),
                        input_channel,
                        loopback_input_channel,
                        signal_level_db,
                        &wav_path,
                        cancel_for_task,
                    );
                    Err::<sotf_audio::signal_recorder::BassAnchorResults, String>(
                        "Bass anchor capture is not available on iOS".to_string(),
                    )
                }
            })
            .await;

            state_clone.update(&mut cx.clone(), |state, cx| {
                let rec = &mut state.app.measurement_state.recording_state;
                if !rec
                    .bass_anchor_capture
                    .is_current_capture(capture_generation)
                {
                    log::info!(
                        "Discarding stale GPUI bass-anchor result (generation {}, current {})",
                        capture_generation,
                        rec.bass_anchor_capture.capture_generation
                    );
                    return;
                }
                let bac = &mut rec.bass_anchor_capture;
                match result {
                    Ok(results) => {
                        bac.apply_results(
                            results,
                            Some(wav_path_for_state.to_string_lossy().to_string()),
                        );
                    }
                    Err(e) if e == sotf_audio::signal_recorder::CANCELLED_ERR => {
                        log::info!("Bass anchor capture cancelled by user");
                        bac.status = BassAnchorCaptureStatus::Idle;
                    }
                    Err(e) => {
                        log::warn!("Bass anchor capture failed: {}", e);
                        bac.status = BassAnchorCaptureStatus::Failed(e);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Complete the QA bass-anchor capture through the visible Run action.
    #[cfg(feature = "dev-api")]
    fn complete_qa_fake_bass_anchor_capture(&mut self, cx: &mut Context<Self>) -> bool {
        self.state.update(cx, |state, cx| {
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
                    |(index, mapping)| sotf_audio::signal_recorder::BassAnchorChannelResult {
                        channel_name: mapping.group_name.clone(),
                        channel_index: mapping.interface_channel(),
                        bass_anchor_phase_deg: index as f64 * 8.0,
                        bass_anchor_magnitude: 0.5 - index as f64 * 0.05,
                        bass_anchor_stability_deg: 3.0 + index as f64,
                        bass_anchor_loopback_phase_deg: None,
                        bass_anchor_coherence: None,
                    },
                )
                .collect();
            let sample_rate = rec.bass_anchor_capture.sample_rate;
            let bass_freq_hz = rec.bass_anchor_capture.bass_freq_hz;
            let bass_duration_s = rec.bass_anchor_capture.bass_duration_s;
            rec.bass_anchor_capture.apply_results(
                sotf_audio::signal_recorder::BassAnchorResults {
                    channels,
                    sample_rate,
                    bass_freq_hz,
                    bass_duration_s,
                },
                None,
            );
            cx.notify();
            true
        })
    }

    /// Request cancellation of an in-progress bass anchor capture. The
    /// engine honors the flag at its next stability poll (~50 ms
    /// latency) and returns `Err(CANCELLED_ERR)`.
    pub(crate) fn cancel_bass_anchor_capture(&mut self, cx: &mut Context<Self>) {
        log::info!("Cancel requested for bass anchor capture");
        self.state.update(cx, |state, cx| {
            state
                .app
                .measurement_state
                .recording_state
                .bass_anchor_cancel_requested
                .store(true, std::sync::atomic::Ordering::Relaxed);
            cx.notify();
        });
    }
}

fn bass_anchor_number_row(
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
        .spacing(StackSpacing::Sm)
        .child(Text::new(label).size(TextSize::Xs))
        // intentional: compact numeric value uses bold microtype, not an eyebrow label.
        .child(
            Text::new(format!("{:.0}", current))
                .size(TextSize::Xs)
                .weight(TextWeight::Bold),
        )
        .child(
            Button::new(format!("{label}_minus"), "-")
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
