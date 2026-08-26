use super::misc::spinorama_runtime;
use super::spawn::spawn_phase_data_check_thread;
use super::spawn::spawn_spinorama_curves_thread;
use crate::app::types::{PluginUpdateType, Screen, SpinoramaStep};
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonTheme, ButtonVariant, HStack, StackSpacing, StepStatus, WizardHeader,
    WizardStep, WizardTheme,
};
use sotf_audio_player::autoeq::speaker::{
    CallbackConfig, MeasurementInput, SpeakerOptimizationCallback, SpeakerOptimizationConfig,
    SpeakerOptimizationProgress,
};
use sotf_audio_player::autoeq::types::SpeakerConfigType;
use std::time::Duration;

/// One progress sample from the optimizer:
/// `(iteration, loss, optional_score, progress_pct)`.
type ProgressSample = (usize, f64, Option<f64>, f32);

/// Final optimization result delivered via a oneshot channel.
/// `(success, result, full_result, error_message)`.
#[allow(clippy::type_complexity)]
type OptimizationOutcome = (
    bool,
    Option<crate::app::types::SpinoramaEqResult>,
    Option<sotf_audio_player::autoeq::SpeakerOptimizationResult>,
    Option<String>,
);

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

const SPINORAMA_FETCH_RETRIES: usize = 3;
const SPINORAMA_RETRY_DELAY: Duration = Duration::from_secs(2);

fn classify_spinorama_fetch_error(error: &str) -> String {
    let lower = error.to_ascii_lowercase();
    if lower.contains("dns")
        || lower.contains("resolve")
        || lower.contains("connection")
        || lower.contains("network")
        || lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("tcp")
    {
        "No network access. Check your connection and try again.".to_string()
    } else {
        "spinorama.org is unavailable. Try again later.".to_string()
    }
}

async fn fetch_json_with_spinorama_retries<T>(url: String) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let mut last_error = String::new();

    for attempt in 1..=SPINORAMA_FETCH_RETRIES {
        match reqwest::get(&url).await {
            Ok(response) => {
                if response.status().is_success() {
                    return response.json::<T>().await.map_err(|e| e.to_string());
                }
                last_error = format!("API request failed: {}", response.status());
            }
            Err(error) => {
                last_error = error.to_string();
            }
        }

        if attempt < SPINORAMA_FETCH_RETRIES {
            smol::Timer::after(SPINORAMA_RETRY_DELAY).await;
        }
    }

    Err(classify_spinorama_fetch_error(&last_error))
}

impl PlayerView {
    // ========================================================================
    // Spinorama EQ Wizard Screen
    // ========================================================================

    /// Main Spinorama EQ screen entry point (wizard)
    pub(crate) fn render_spinorama_eq_screen(
        &mut self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Check if we need to auto-fetch speakers before reading state
        let needs_fetch = {
            let state = self.state.read(cx);
            let spinorama = &state.app.measurement_state.spinorama_eq_state;
            // QA fixtures intentionally leave discovery to the scenario's
            // explicit Refresh action, so their queued response is not spent
            // by the initial render.
            #[cfg(feature = "dev-api")]
            let fixture_controls_discovery = spinorama.qa_discovery_fixture.is_some();
            #[cfg(not(feature = "dev-api"))]
            let fixture_controls_discovery = false;

            spinorama.needs_speaker_refresh()
                && !spinorama.loading_speakers
                && !fixture_controls_discovery
        };

        if needs_fetch {
            // Set loading flag immediately to prevent duplicate fetches
            self.state.update(cx, |state, _| {
                state
                    .app
                    .measurement_state
                    .spinorama_eq_state
                    .loading_speakers = true;
            });
            // Schedule fetch
            cx.spawn(async move |view, cx| {
                let _ = view.update(cx, |view, cx| {
                    view.fetch_spinorama_speakers(cx);
                });
            })
            .detach();
        }

        // Check if we need to auto-load spinorama curves (Step 1 with speaker selected but curves not loaded)
        let needs_curves = {
            let state = self.state.read(cx);
            let spinorama = &state.app.measurement_state.spinorama_eq_state;
            spinorama.step == SpinoramaStep::SelectSpeaker
                && spinorama.selected_speaker.is_some()
                && !spinorama.selected_version.is_empty()
                && (spinorama.selected_measurement == "CEA2034"
                    || spinorama.selected_measurement == "CEA2034 Normalized")
                && !spinorama.spinorama_curves.is_valid()
                && !spinorama.loading_spinorama_curves
                && spinorama.spinorama_curves_error.is_none()
        };

        if needs_curves {
            let (speaker, version) = {
                let state = self.state.read(cx);
                (
                    state
                        .app
                        .measurement_state
                        .spinorama_eq_state
                        .selected_speaker
                        .clone()
                        .unwrap_or_default(),
                    state
                        .app
                        .measurement_state
                        .spinorama_eq_state
                        .selected_version
                        .clone(),
                )
            };
            let request_speaker = speaker.clone();
            let request_version = version.clone();
            log::info!(
                "Auto-loading spinorama curves for {} / {}",
                speaker,
                version
            );
            self.state.update(cx, |state, _| {
                state
                    .app
                    .measurement_state
                    .spinorama_eq_state
                    .loading_spinorama_curves = true;
            });
            let curves_rx = spawn_spinorama_curves_thread(speaker, version);

            // Await the per-request oneshot channel instead of polling a
            // global mutex. Dropping the receiver if the wizard is closed
            // is harmless — the producing thread's `send_blocking` returns
            // `Err` and is ignored.
            let weak_state = self.state.downgrade();
            cx.spawn(async move |_, cx| {
                let Ok(result) = curves_rx.recv().await else {
                    return;
                };
                let Some(state_for_poll) = weak_state.upgrade() else {
                    return;
                };
                state_for_poll.update(cx, |state, cx| {
                    let spinorama = &mut state.app.measurement_state.spinorama_eq_state;
                    if spinorama.selected_speaker.as_deref() != Some(request_speaker.as_str())
                        || spinorama.selected_version != request_version
                    {
                        log::debug!(
                            "Discarding stale spinorama curves for {} / {}",
                            request_speaker,
                            request_version
                        );
                        return;
                    }
                    spinorama.loading_spinorama_curves = false;
                    match result {
                        Ok(curves) => {
                            log::info!("Auto-loaded spinorama curves successfully");
                            spinorama.spinorama_curves = curves;
                            spinorama.spinorama_curves_error = None;
                        }
                        Err(e) => {
                            log::error!("Failed to auto-load spinorama curves: {}", e);
                            spinorama.spinorama_curves_error = Some(e);
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        }

        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let current_step = state.app.measurement_state.spinorama_eq_state.step;

        // Content for current step
        let content = match current_step {
            SpinoramaStep::SelectSpeaker => {
                self.render_spinorama_select_speaker(cx).into_any_element()
            }
            SpinoramaStep::Configure => self.render_spinorama_configure(cx).into_any_element(),
            SpinoramaStep::Review => self.render_spinorama_review(cx).into_any_element(),
            SpinoramaStep::Export => self.render_spinorama_export(cx).into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .bg(theme.background)
            .child(self.render_spinorama_header(cx))
            .child(dev_track!(
                div()
                    .id("spinorama-eq-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(d.card)
                    .child(content),
                "spinorama.content"
            ))
    }

    /// Render the spinorama EQ screen header with step indicators
    pub(super) fn render_spinorama_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let title = state.app.ui_state.translations.screen_spinorama;
        let theme_id = state.app.ui_state.theme_id;
        let current_step = state.app.measurement_state.spinorama_eq_state.step;
        let can_go_next = state.app.can_advance_workflow_step();
        let is_busy = state
            .app
            .measurement_state
            .spinorama_eq_state
            .is_optimizing();

        let step_index = current_step.index();

        // Build step statuses
        let step_statuses: Vec<StepStatus> = (0..4)
            .map(|i| {
                if i < step_index {
                    StepStatus::Completed
                } else if i == step_index {
                    StepStatus::Active
                } else {
                    StepStatus::NotVisited
                }
            })
            .collect();

        // Build wizard steps
        let steps = vec![
            WizardStep::new("select", "Select"),
            WizardStep::new("configure", "Optimize"),
            WizardStep::new("review", "Review"),
            WizardStep::new("export", "Export"),
        ];

        let ui_kit_theme = theme.to_ui_kit_theme(theme_id, cx);
        let wizard_theme = WizardTheme::from(&ui_kit_theme);
        let button_theme = ButtonTheme::from(&ui_kit_theme);

        let header = WizardHeader::new()
            .title(title)
            .steps(steps)
            .step_statuses(step_statuses)
            .current_step(step_index)
            .theme(wizard_theme.clone());

        let back_label = match current_step {
            SpinoramaStep::SelectSpeaker => "Close",
            _ => "Back",
        };
        let next_label =
            crate::components::wizard_continue_label(current_step.next().map(|next| next.label()));

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
                "spinorama.back"
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
                "spinorama.next"
            ));
        let navigation = navigation.build().flex_none();

        // Home button for navigation back to Library
        let state_for_home = self.state.clone();
        let text_muted = theme.text_muted;
        let surface_hover = theme.surface_hover;

        div()
            .flex()
            .items_center()
            .justify_between()
            .min_w_0()
            .px(d.card)
            .py(d.card)
            .bg(theme.background_secondary)
            .border_b_1()
            .border_color(theme.border)
            // Home button on the left
            .child(
                div()
                    .id("spinorama-home-button")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(rems(2.5))
                    .h(rems(2.0))
                    .cursor_pointer()
                    .rounded(d.r_md)
                    .hover(move |s| s.bg(surface_hover))
                    .child(Icon::new(IconName::Home).color(text_muted))
                    .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                        state_for_home.update(cx, |state, _cx| {
                            state.app.ui_state.current_screen = Screen::Library;
                        });
                    }),
            )
            // Centered header with flex-1
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .justify_center()
                    .child(header),
            )
            // Navigation buttons on the right
            .child(navigation)
    }

    // ========================================================================
    // Action Handlers
    // ========================================================================

    pub(crate) fn fetch_spinorama_speakers(&mut self, cx: &mut Context<Self>) {
        log::info!("Fetching spinorama speakers from API...");
        #[cfg(feature = "dev-api")]
        if let Some((speakers, delay_ms, should_fail, failure_message)) =
            self.state.update(cx, |state, _| {
                state
                    .app
                    .measurement_state
                    .spinorama_eq_state
                    .qa_discovery_fixture
                    .as_mut()
                    .map(|fixture| {
                        let should_fail = fixture.catalog_failures_remaining > 0;
                        fixture.catalog_failures_remaining =
                            fixture.catalog_failures_remaining.saturating_sub(1);
                        (
                            fixture.catalog.clone(),
                            fixture.catalog_delay_ms,
                            should_fail,
                            fixture.catalog_failure_message.clone(),
                        )
                    })
            })
        {
            let request_id = self.state.update(cx, |state, _| {
                let spinorama = &mut state.app.measurement_state.spinorama_eq_state;
                spinorama.loading_speakers = true;
                spinorama.error_message = None;
                spinorama.begin_speaker_list_request()
            });
            let weak_state = self.state.downgrade();
            cx.spawn(async move |_, cx| {
                smol::Timer::after(std::time::Duration::from_millis(delay_ms)).await;
                let Some(state_entity) = weak_state.upgrade() else {
                    return;
                };
                state_entity.update(cx, |state, cx| {
                    let spinorama = &mut state.app.measurement_state.spinorama_eq_state;
                    if spinorama.speaker_list_request_id != request_id {
                        return;
                    }
                    spinorama.loading_speakers = false;
                    if should_fail {
                        spinorama.error_message = Some(failure_message);
                        // Avoid immediately scheduling an identical automatic
                        // request: leave the actionable error visible until
                        // the user explicitly retries with Refresh.
                        spinorama.speakers_cached_at = Some(std::time::Instant::now());
                    } else {
                        spinorama.available_speakers = speakers;
                        spinorama.speakers_cached_at = Some(std::time::Instant::now());
                        spinorama.update_suggestions();
                    }
                    cx.notify();
                });
            })
            .detach();
            cx.notify();
            return;
        }
        // Note: loading_speakers is set to true before spawning to prevent duplicate fetches
        let request_id = self.state.update(cx, |state, _cx| {
            let spinorama = &mut state.app.measurement_state.spinorama_eq_state;
            spinorama.loading_speakers = true;
            spinorama.error_message = None;
            spinorama.begin_speaker_list_request()
        });
        cx.notify();

        // Per-request oneshot channel — no shared global state.
        let (tx, rx) = smol::channel::bounded::<Result<Vec<String>, String>>(1);
        std::thread::spawn(move || {
            let result = spinorama_runtime().and_then(|runtime| {
                runtime.block_on(async {
                    fetch_json_with_spinorama_retries::<Vec<String>>(
                        "https://api.spinorama.org/v1/speakers".to_string(),
                    )
                    .await
                })
            });
            let _ = tx.send_blocking(result);
        });

        // Poll the channel on the GPUI scheduler. Awaiting `rx.recv()` lets
        // the worker thread wake this local task directly, which violates the
        // deterministic GPUI test scheduler.
        let weak_state = self.state.downgrade();
        cx.spawn(async move |_, cx| {
            let result = loop {
                match rx.try_recv() {
                    Ok(result) => break result,
                    Err(smol::channel::TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(50))
                            .await;
                    }
                    Err(smol::channel::TryRecvError::Closed) => return,
                }
            };
            let Some(state_entity) = weak_state.upgrade() else {
                return;
            };
            match result {
                Ok(speakers) => {
                    log::info!("Fetched {} speakers from spinorama.org", speakers.len());
                    state_entity.update(cx, |state, cx| {
                        let spinorama = &mut state.app.measurement_state.spinorama_eq_state;
                        if spinorama.speaker_list_request_id != request_id {
                            log::debug!("Discarding stale Spinorama speaker catalog completion");
                            return;
                        }
                        spinorama.available_speakers = speakers;
                        spinorama.loading_speakers = false;
                        spinorama.speakers_cached_at = Some(std::time::Instant::now());
                        spinorama.update_suggestions();
                        cx.notify();
                    });
                }
                Err(e) => {
                    log::error!("Failed to fetch speakers: {}", e);
                    state_entity.update(cx, |state, cx| {
                        let spinorama = &mut state.app.measurement_state.spinorama_eq_state;
                        if spinorama.speaker_list_request_id != request_id {
                            log::debug!("Discarding stale Spinorama speaker catalog failure");
                            return;
                        }
                        spinorama.loading_speakers = false;
                        let msg = format!("Failed to fetch speakers: {}", e);
                        spinorama.error_message = Some(msg.clone());
                        // Surface the error via toast as well — the step_1
                        // inline banner only shows when the user is
                        // actively on the speaker-search screen.
                        state.app.ui_state.toast_message =
                            Some(crate::app::ToastMessage::error(msg));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    pub fn select_spinorama_speaker(&mut self, speaker: &str, cx: &mut Context<Self>) {
        log::info!("Selected speaker: {}", speaker);
        self.state.update(cx, |state, _cx| {
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .selected_speaker = Some(speaker.to_string());
            // Reset phase data flag and clear version/measurement lists
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .has_phase_data = false;
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .available_versions
                .clear();
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .available_measurements
                .clear();
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .selected_version
                .clear();
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .selected_measurement = "CEA2034".to_string();
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .spinorama_curves = crate::app::types::SpinoramaCurves::default();
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .spinorama_curves_error = None;
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .loading_spinorama_curves = false;
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .loading_versions = true;
        });
        cx.notify();

        // Fetch available versions for the selected speaker
        self.fetch_spinorama_versions(speaker, cx);
    }

    pub(super) fn fetch_spinorama_versions(&mut self, speaker: &str, cx: &mut Context<Self>) {
        log::info!("Fetching versions for speaker: {}", speaker);
        #[cfg(feature = "dev-api")]
        if let Some(versions) = self
            .state
            .read(cx)
            .app
            .measurement_state
            .spinorama_eq_state
            .qa_discovery_fixture
            .as_ref()
            .and_then(|fixture| fixture.versions.get(speaker).cloned())
        {
            let version = versions.first().cloned();
            self.state.update(cx, |state, _| {
                let spinorama = &mut state.app.measurement_state.spinorama_eq_state;
                if spinorama.selected_speaker.as_deref() != Some(speaker) {
                    return;
                }
                spinorama.available_versions = versions;
                spinorama.loading_versions = false;
                spinorama.error_message = None;
                if let Some(version) = &version {
                    spinorama.selected_version = version.clone();
                }
            });
            cx.notify();
            if let Some(version) = version {
                self.fetch_spinorama_measurements(speaker, &version, cx);
            }
            return;
        }
        let speaker_name = speaker.to_string();

        // Per-request oneshot channel.
        let (tx, rx) = smol::channel::bounded::<Result<Vec<String>, String>>(1);
        let speaker_for_fetch = speaker_name.clone();
        std::thread::spawn(move || {
            let result = spinorama_runtime().and_then(|runtime| {
                runtime.block_on(async {
                    let encoded_speaker = urlencoding::encode(&speaker_for_fetch);
                    let url = format!(
                        "https://api.spinorama.org/v1/speaker/{}/versions",
                        encoded_speaker
                    );
                    fetch_json_with_spinorama_retries::<Vec<String>>(url).await
                })
            });
            let _ = tx.send_blocking(result);
        });

        // Poll the channel on the GPUI scheduler. Awaiting `rx.recv()` lets
        // the worker thread wake this local task directly, which violates the
        // deterministic GPUI test scheduler.
        let weak_state = self.state.downgrade();
        let speaker_for_poll = speaker_name.clone();
        cx.spawn(async move |view, cx| {
            let result = loop {
                match rx.try_recv() {
                    Ok(result) => break result,
                    Err(smol::channel::TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(50))
                            .await;
                    }
                    Err(smol::channel::TryRecvError::Closed) => return,
                }
            };
            let Some(state_entity) = weak_state.upgrade() else {
                return;
            };
            match result {
                Ok(versions) => {
                    log::info!(
                        "Fetched {} versions for {}",
                        versions.len(),
                        speaker_for_poll
                    );
                    let first_version = versions.first().cloned();
                    let selected_version = first_version.clone();
                    let still_current = state_entity.update(cx, |state, cx| {
                        let spinorama = &mut state.app.measurement_state.spinorama_eq_state;
                        if spinorama.selected_speaker.as_deref() != Some(speaker_for_poll.as_str())
                        {
                            log::debug!("Discarding stale versions for {}", speaker_for_poll);
                            return false;
                        }
                        spinorama.available_versions = versions;
                        spinorama.loading_versions = false;
                        // Auto-select first version if available
                        if let Some(ref version) = selected_version {
                            spinorama.selected_version = version.clone();
                        }
                        cx.notify();
                        true
                    });
                    if !still_current {
                        return;
                    }
                    // Fetch measurements for the selected version
                    if let Some(version) = selected_version {
                        let _ = view.update(cx, |view, cx| {
                            view.fetch_spinorama_measurements(&speaker_for_poll, &version, cx);
                        });
                    }
                }
                Err(e) => {
                    log::error!("Failed to fetch versions: {}", e);
                    state_entity.update(cx, |state, cx| {
                        let spinorama = &mut state.app.measurement_state.spinorama_eq_state;
                        if spinorama.selected_speaker.as_deref() != Some(speaker_for_poll.as_str())
                        {
                            log::debug!("Discarding stale versions error for {}", speaker_for_poll);
                            return;
                        }
                        spinorama.loading_versions = false;
                        let msg = format!("Failed to fetch versions: {}", e);
                        spinorama.error_message = Some(msg.clone());
                        state.app.ui_state.toast_message =
                            Some(crate::app::ToastMessage::error(msg));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    pub(super) fn fetch_spinorama_measurements(
        &mut self,
        speaker: &str,
        version: &str,
        cx: &mut Context<Self>,
    ) {
        log::info!(
            "Fetching measurements for speaker: {}, version: {}",
            speaker,
            version
        );
        #[cfg(feature = "dev-api")]
        if let Some((measurements, response)) = self
            .state
            .read(cx)
            .app
            .measurement_state
            .spinorama_eq_state
            .qa_discovery_fixture
            .as_ref()
            .and_then(|fixture| {
                let key = (speaker.to_string(), version.to_string());
                fixture.measurements.get(&key).cloned().map(|measurements| {
                    let response = measurements.iter().find_map(|measurement| {
                        fixture
                            .responses
                            .get(&(key.0.clone(), key.1.clone(), measurement.clone()))
                            .cloned()
                    });
                    (measurements, response)
                })
            })
        {
            let selected = measurements
                .iter()
                .find(|measurement| measurement.as_str() == "CEA2034")
                .cloned()
                .or_else(|| measurements.first().cloned());
            self.state.update(cx, |state, _| {
                let spinorama = &mut state.app.measurement_state.spinorama_eq_state;
                if spinorama.selected_speaker.as_deref() != Some(speaker)
                    || spinorama.selected_version != version
                {
                    return;
                }
                spinorama.available_measurements = measurements;
                spinorama.loading_measurements = false;
                spinorama.loading_spinorama_curves = false;
                spinorama.has_phase_data = false;
                spinorama.error_message = None;
                if let Some(response) = &response {
                    let frequencies = response.frequencies.clone();
                    let on_axis = response.spl.clone();
                    spinorama.spinorama_curves = crate::app::types::SpinoramaCurves {
                        frequencies,
                        listening_window: on_axis.clone(),
                        early_reflections: on_axis.clone(),
                        sound_power: on_axis.clone(),
                        early_reflections_di: vec![0.0; on_axis.len()],
                        sound_power_di: vec![0.0; on_axis.len()],
                        estimated_in_room: on_axis.clone(),
                        on_axis,
                        horizontal_directivity: Vec::new(),
                        vertical_directivity: Vec::new(),
                    };
                }
                if let Some(selected) = &selected {
                    spinorama.selected_measurement = selected.clone();
                }
            });
            cx.notify();
            return;
        }

        self.state.update(cx, |state, _cx| {
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .loading_measurements = true;
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .available_measurements
                .clear();
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .has_phase_data = false;
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .spinorama_curves = crate::app::types::SpinoramaCurves::default();
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .spinorama_curves_error = None;
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .loading_spinorama_curves = false;
        });
        cx.notify();

        let speaker_name = speaker.to_string();
        let version_name = version.to_string();

        // Per-request oneshot channel.
        let (measurements_tx, measurements_rx) =
            smol::channel::bounded::<Result<Vec<String>, String>>(1);
        std::thread::spawn(move || {
            let result = spinorama_runtime().and_then(|runtime| {
                runtime.block_on(async {
                    let encoded_speaker = urlencoding::encode(&speaker_name);
                    let encoded_version = urlencoding::encode(&version_name);
                    let url = format!(
                        "https://api.spinorama.org/v1/speaker/{}/version/{}/measurements",
                        encoded_speaker, encoded_version
                    );
                    fetch_json_with_spinorama_retries::<Vec<String>>(url).await
                })
            });
            let _ = measurements_tx.send_blocking(result);
        });

        // Await the channel, then chain the phase-check + curves-load
        // followups, each on its own per-request channel.
        let weak_state = self.state.downgrade();
        let speaker_for_poll = speaker.to_string();
        let version_for_poll = version.to_string();
        cx.spawn(async move |_, cx| {
            let result = loop {
                match measurements_rx.try_recv() {
                    Ok(result) => break result,
                    Err(smol::channel::TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(50))
                            .await;
                    }
                    Err(smol::channel::TryRecvError::Closed) => return,
                }
            };
            let Some(state_entity) = weak_state.upgrade() else {
                return;
            };
            match result {
                Ok(measurements) => {
                    log::info!(
                        "Fetched {} measurements for {}/{}",
                        measurements.len(),
                        speaker_for_poll,
                        version_for_poll
                    );
                    let has_cea2034 = measurements.iter().any(|m| m == "CEA2034");
                    let selected_measurement = if has_cea2034 {
                        "CEA2034".to_string()
                    } else {
                        measurements
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "CEA2034".to_string())
                    };
                    let measurement_for_phase = selected_measurement.clone();
                    let still_current = state_entity.update(cx, |state, cx| {
                        let spinorama = &mut state.app.measurement_state.spinorama_eq_state;
                        if spinorama.selected_speaker.as_deref() != Some(speaker_for_poll.as_str())
                            || spinorama.selected_version != version_for_poll
                        {
                            log::debug!(
                                "Discarding stale measurements for {} / {}",
                                speaker_for_poll,
                                version_for_poll
                            );
                            return false;
                        }
                        spinorama.available_measurements = measurements;
                        spinorama.loading_measurements = false;
                        spinorama.selected_measurement = selected_measurement;
                        if has_cea2034 {
                            spinorama.loading_spinorama_curves = true;
                        }
                        cx.notify();
                        true
                    });
                    if !still_current {
                        return;
                    }

                    // Fire-and-await: each follow-up has its own channel,
                    // so a second `fetch_measurements` for a different
                    // speaker can't steal results.
                    let curves_rx = if has_cea2034 {
                        Some(spawn_spinorama_curves_thread(
                            speaker_for_poll.clone(),
                            version_for_poll.clone(),
                        ))
                    } else {
                        None
                    };
                    let phase_rx = spawn_phase_data_check_thread(
                        speaker_for_poll.clone(),
                        version_for_poll.clone(),
                        measurement_for_phase,
                    );

                    // Phase check first (it tends to complete sooner).
                    if let Ok(has_phase) = phase_rx.recv().await {
                        state_entity.update(cx, |state, cx| {
                            let spinorama = &mut state.app.measurement_state.spinorama_eq_state;
                            if spinorama.selected_speaker.as_deref()
                                != Some(speaker_for_poll.as_str())
                                || spinorama.selected_version != version_for_poll
                            {
                                log::debug!(
                                    "Discarding stale phase check for {} / {}",
                                    speaker_for_poll,
                                    version_for_poll
                                );
                                return;
                            }
                            spinorama.has_phase_data = has_phase;
                            log::info!("Phase data availability: {}", has_phase);
                            cx.notify();
                        });
                    }

                    if let Some(curves_rx) = curves_rx
                        && let Ok(curves_result) = curves_rx.recv().await
                    {
                        state_entity.update(cx, |state, cx| {
                            let spinorama = &mut state.app.measurement_state.spinorama_eq_state;
                            if spinorama.selected_speaker.as_deref()
                                != Some(speaker_for_poll.as_str())
                                || spinorama.selected_version != version_for_poll
                            {
                                log::debug!(
                                    "Discarding stale spinorama curves for {} / {}",
                                    speaker_for_poll,
                                    version_for_poll
                                );
                                return;
                            }
                            spinorama.loading_spinorama_curves = false;
                            match curves_result {
                                Ok(curves) => {
                                    log::info!("Auto-loaded spinorama curves successfully");
                                    spinorama.spinorama_curves = curves;
                                    spinorama.spinorama_curves_error = None;
                                }
                                Err(e) => {
                                    log::error!("Failed to auto-load spinorama curves: {}", e);
                                    spinorama.spinorama_curves_error = Some(e);
                                }
                            }
                            cx.notify();
                        });
                    }
                }
                Err(e) => {
                    log::error!("Failed to fetch measurements: {}", e);
                    state_entity.update(cx, |state, cx| {
                        let spinorama = &mut state.app.measurement_state.spinorama_eq_state;
                        if spinorama.selected_speaker.as_deref() != Some(speaker_for_poll.as_str())
                            || spinorama.selected_version != version_for_poll
                        {
                            log::debug!(
                                "Discarding stale measurements error for {} / {}",
                                speaker_for_poll,
                                version_for_poll
                            );
                            return;
                        }
                        spinorama.loading_measurements = false;
                        let msg = format!("Failed to fetch measurements: {}", e);
                        spinorama.error_message = Some(msg.clone());
                        state.app.ui_state.toast_message =
                            Some(crate::app::ToastMessage::error(msg));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    pub fn select_spinorama_version(&mut self, version: &str, cx: &mut Context<Self>) {
        log::info!("Selected version: {}", version);
        let speaker = {
            let state = self.state.read(cx);
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .selected_speaker
                .clone()
        };

        self.state.update(cx, |state, _cx| {
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .selected_version = version.to_string();
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .has_phase_data = false;
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .available_measurements
                .clear();
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .spinorama_curves = crate::app::types::SpinoramaCurves::default();
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .spinorama_curves_error = None;
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .loading_spinorama_curves = false;
        });
        cx.notify();

        // Fetch measurements for the new version
        if let Some(speaker) = speaker {
            self.fetch_spinorama_measurements(&speaker, version, cx);
        }
    }

    pub fn start_spinorama_optimization(&mut self, cx: &mut Context<Self>) {
        log::info!("Starting spinorama optimization...");

        // Gather config from state
        let (speaker_name, version, measurement, curve_name, optimizer_config, mode, target_curve) = {
            let state = self.state.read(cx);
            let spinorama = &state.app.measurement_state.spinorama_eq_state;
            let speaker = spinorama.selected_speaker.clone().unwrap_or_default();
            let version = spinorama.selected_version.clone();
            let measurement = spinorama.selected_measurement.clone();
            let curve = spinorama.selected_curve.clone();
            let config = spinorama.optimizer_config.clone();
            let mode = spinorama.optimizer_config.mode;
            let target_curve = spinorama.optimizer_config.target_curve;
            (
                speaker,
                version,
                measurement,
                curve,
                config,
                mode,
                target_curve,
            )
        };

        if speaker_name.is_empty() {
            self.state.update(cx, |state, _cx| {
                state.app.measurement_state.spinorama_eq_state.error_message =
                    Some("No speaker selected".to_string());
            });
            cx.notify();
            return;
        }

        #[cfg(feature = "dev-api")]
        let fixture_response = self
            .state
            .read(cx)
            .app
            .measurement_state
            .spinorama_eq_state
            .qa_discovery_fixture
            .as_ref()
            .and_then(|fixture| {
                fixture
                    .responses
                    .get(&(speaker_name.clone(), version.clone(), measurement.clone()))
                    .cloned()
            });

        let cancel_flag = self.state.update(cx, |state, _cx| {
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .cancel_requested
                .store(false, std::sync::atomic::Ordering::Relaxed);
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .optimization_status = crate::app::types::OptimizationStatus::Running;
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .status_message = "Loading measurement data...".to_string();
            state.app.measurement_state.spinorama_eq_state.progress = 0.0;
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .progress_history
                .clear();
            state.app.measurement_state.spinorama_eq_state.error_message = None;
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .cancel_requested
                .clone()
        });
        cx.notify();

        // Per-request channels — one progress stream and one oneshot
        // outcome. No shared global state between concurrent optimization
        // runs.
        let (progress_tx, progress_rx) = smol::channel::unbounded::<ProgressSample>();
        let (outcome_tx, outcome_rx) = smol::channel::bounded::<OptimizationOutcome>(1);

        // Build optimization params
        let loss = mode.to_loss_string().to_string();
        let algo = optimizer_config.algorithm.to_autoeq_string().to_string();

        // Use the user-selected curve, or override based on mode
        let effective_curve_name = if curve_name.is_empty() {
            // Default to PIR if no curve selected
            "Estimated In-Room Response".to_string()
        } else if mode == crate::app::types::SpinoramaOptimizationMode::FlatOnPir {
            // For FlatOnPir mode, use the target curve selection
            target_curve.api_name().to_string()
        } else if mode == crate::app::types::SpinoramaOptimizationMode::SpeakerScore {
            // For SpeakerScore mode, use target curve selection for proper target:
            // - On Axis: flat target (0 dB)
            // - Listening Window: slight roll-off (-0.5 dB at 20kHz)
            // The score loss function uses ON/LW/SP/PIR internally regardless
            target_curve.api_name().to_string()
        } else {
            curve_name.clone()
        };

        // Use CEA2034 as default measurement if none selected
        let effective_measurement = if measurement.is_empty() {
            "CEA2034".to_string()
        } else {
            measurement.clone()
        };

        // Use first available version or "asr" as fallback
        let effective_version = if version.is_empty() {
            "asr".to_string()
        } else {
            version.clone()
        };

        log::info!(
            "Spinorama optimization config: speaker={}, version={}, measurement={}, curve={}",
            speaker_name,
            effective_version,
            effective_measurement,
            effective_curve_name
        );
        log::info!(
            "Spinorama optimization mode: {:?}, loss={}, target_curve={:?}",
            mode,
            loss,
            target_curve
        );
        log::info!(
            "Spinorama optimization params: algo={}, maxeval={}, num_filters={}, population={}",
            algo,
            optimizer_config.max_iter,
            optimizer_config.num_filters,
            optimizer_config.population
        );

        // Build Args using library defaults
        let mut params = autoeq::Args::speaker_defaults();
        params.num_filters = optimizer_config.num_filters;
        params.sample_rate = optimizer_config.sample_rate as f64;
        params.min_db = optimizer_config.min_db;
        params.max_db = optimizer_config.max_db;
        params.min_q = optimizer_config.min_q;
        params.max_q = optimizer_config.max_q;
        params.min_freq = optimizer_config.min_freq;
        params.max_freq = optimizer_config.max_freq;
        params.maxeval = optimizer_config.max_iter;
        params.population = optimizer_config.population;
        params.recombination = optimizer_config.de_cr;
        params.strategy = optimizer_config.strategy.clone();
        params.refine = optimizer_config.refine;
        params.local_algo = optimizer_config.local_algo.clone();
        params.smooth = optimizer_config.smooth;
        params.smooth_n = optimizer_config.smooth_n;
        params.spacing_weight = optimizer_config.spacing_weight;
        params.min_spacing_oct = optimizer_config.min_spacing_oct;
        params.peq_model = sotf_audio_player::autoeq::parse_peq_model(&optimizer_config.peq_model);
        params.tolerance = optimizer_config.tolerance;
        params.atolerance = optimizer_config.atolerance;
        params.bo_initial_samples = optimizer_config.bo_initial_samples;
        params.bo_batch_size = optimizer_config.bo_batch_size;
        params.bo_posterior_std_threshold = optimizer_config.bo_posterior_std_threshold;
        params.bo_acquisition = optimizer_config.bo_acquisition.clone();
        params.bo_ehvi = optimizer_config.bo_ehvi;
        params.loss = sotf_audio_player::autoeq::parse_loss_type(&loss);
        params.algo = algo;
        params.curve_name = effective_curve_name.clone();

        // Run optimization in background thread (blocking tokio runtime)
        let cancel_for_thread = cancel_flag.clone();
        let progress_tx_for_thread = progress_tx.clone();
        let outcome_tx_for_thread = outcome_tx.clone();
        std::thread::spawn(move || {
            #[cfg(feature = "dev-api")]
            let fixture_measurement = fixture_response.map(|response| {
                MeasurementInput::Curve(autoeq::Curve {
                    freq: ndarray::Array1::from_vec(response.frequencies),
                    spl: ndarray::Array1::from_vec(response.spl),
                    ..Default::default()
                })
            });
            // Build the optimization config
            let config = SpeakerOptimizationConfig {
                config_type: SpeakerConfigType::Single,
                main_measurement: Some({
                    #[cfg(feature = "dev-api")]
                    if let Some(measurement) = fixture_measurement {
                        measurement
                    } else {
                        MeasurementInput::Spinorama {
                            speaker: speaker_name.clone(),
                            version: effective_version,
                            measurement: effective_measurement,
                            curve_name: effective_curve_name.clone(),
                        }
                    }
                    #[cfg(not(feature = "dev-api"))]
                    MeasurementInput::Spinorama {
                        speaker: speaker_name.clone(),
                        version: effective_version,
                        measurement: effective_measurement,
                        curve_name: effective_curve_name.clone(),
                    }
                }),
                driver_measurements: Vec::new(),
                crossover_type: None,
                crossover_freq_hints: Vec::new(),
                args: params.clone(),
                callback_config: Some(CallbackConfig {
                    interval: 25,
                    include_biquads: true,
                    include_filter_response: true,
                }),
                target: None,
            };

            // Create callback for progress updates
            let max_iter = params.maxeval;
            let cancel_for_cb = cancel_for_thread.clone();
            let callback: SpeakerOptimizationCallback =
                Box::new(move |progress: &SpeakerOptimizationProgress| {
                    if cancel_for_cb.load(std::sync::atomic::Ordering::Relaxed) {
                        return sotf_audio_player::autoeq::speaker::CallbackAction::Stop;
                    }

                    let progress_pct = progress.iteration as f32 / max_iter as f32;
                    let iter = progress.iteration;
                    let loss = progress.loss;
                    let score = progress.score;

                    // Send progress through the per-request channel. The
                    // GPUI side drains in batches (see below) to coalesce
                    // updates and keep the UI responsive. If the receiver
                    // has been dropped we silently ignore the error.
                    let _ = progress_tx_for_thread.send_blocking((iter, loss, score, progress_pct));

                    log::debug!(
                        "Spinorama optimization: iter={}, loss={:.4}, score={:?}, progress={:.1}%",
                        iter,
                        loss,
                        score,
                        progress_pct * 100.0
                    );

                    // Continue optimization
                    sotf_audio_player::autoeq::speaker::CallbackAction::Continue
                });

            // Run the actual optimization
            log::info!("Running speaker optimization for: {}", speaker_name);
            log::info!(
                "Args: algo={}, maxeval={}, population={}, num_filters={}",
                config.args.algo,
                config.args.maxeval,
                config.args.population,
                config.args.num_filters
            );
            log::info!(
                "Args: strategy={}, recombination={}, refine={}, local_algo={}",
                config.args.strategy,
                config.args.recombination,
                config.args.refine,
                config.args.local_algo
            );
            log::info!(
                "Args: tolerance={}, atolerance={}, smooth={}, peq_model={:?}",
                config.args.tolerance,
                config.args.atolerance,
                config.args.smooth,
                config.args.peq_model
            );
            log::info!(
                "Args: min_db={}, max_db={}, min_q={}, max_q={}, min_freq={}, max_freq={}",
                config.args.min_db,
                config.args.max_db,
                config.args.min_q,
                config.args.max_q,
                config.args.min_freq,
                config.args.max_freq
            );
            let result = sotf_audio_player::autoeq::speaker::run_speaker_optimization_with_callback(
                &config,
                Some(callback),
            );

            // Build the outcome and send it through the per-request oneshot
            // channel. No globals, no shared state — if the consumer has
            // gone away `send_blocking` returns `Err` and we ignore it.
            let outcome: OptimizationOutcome = match result {
                Ok(opt_result) => {
                    log::info!(
                        "Optimization complete: {} filters, loss {:.4} -> {:.4}",
                        opt_result.biquads.len(),
                        opt_result.initial_loss,
                        opt_result.final_loss
                    );

                    let biquads: Vec<crate::app::types::SpinoramaBiquad> = opt_result
                        .biquads
                        .iter()
                        .map(|b| crate::app::types::SpinoramaBiquad {
                            filter_type: format!("{:?}", b.filter_type),
                            freq: b.freq,
                            q: b.q,
                            db_gain: b.db_gain,
                        })
                        .collect();

                    let original_response: Vec<(f64, f64)> = opt_result
                        .frequencies
                        .iter()
                        .zip(opt_result.input_curve.iter())
                        .map(|(&f, &db)| (f, db))
                        .collect();
                    let corrected_response: Vec<(f64, f64)> = opt_result
                        .frequencies
                        .iter()
                        .zip(opt_result.corrected_curve.iter())
                        .map(|(&f, &db)| (f, db))
                        .collect();
                    let target_response: Vec<(f64, f64)> = opt_result
                        .frequencies
                        .iter()
                        .zip(opt_result.target_curve.iter())
                        .map(|(&f, &db)| (f, db))
                        .collect();

                    log::info!("Sending optimization result with {} filters", biquads.len());

                    let result = crate::app::types::SpinoramaEqResult {
                        biquads,
                        pre_score: opt_result.initial_loss,
                        post_score: opt_result.final_loss,
                        original_response: Some(original_response),
                        corrected_response: Some(corrected_response),
                        target_response: Some(target_response),
                    };
                    (true, Some(result), Some(opt_result), None)
                }
                Err(e) => {
                    log::error!("Optimization failed: {}", e);
                    (false, None, None, Some(e))
                }
            };
            let _ = outcome_tx_for_thread.send_blocking(outcome);
        });

        // Drop the local senders so the receivers can see "channel closed"
        // if the worker thread panics without sending — otherwise the
        // receive would hang forever.
        drop(progress_tx);
        drop(outcome_tx);

        // Drain progress messages in batches and update state. Same shape
        // as `room_eq/step_4_optimise.rs:1007-1156`: block on the next
        // message, then drain everything else that's been queued before
        // the UI yield. 50 ms between batches caps re-renders at ~20 fps.
        let weak_state_progress = self.state.downgrade();
        cx.spawn(async move |_, cx| {
            loop {
                let Ok(first) = progress_rx.recv().await else {
                    break;
                };
                let mut last = first;
                let mut batch: Vec<ProgressSample> = vec![first];
                while let Ok(sample) = progress_rx.try_recv() {
                    batch.push(sample);
                    last = sample;
                }
                let Some(state_for_poll) = weak_state_progress.upgrade() else {
                    break;
                };
                let last_pct = last.3;
                state_for_poll.update(cx, |state, cx| {
                    for (iter, loss, score, _) in batch {
                        state
                            .app
                            .measurement_state
                            .spinorama_eq_state
                            .progress_history
                            .push((iter, loss, score));
                    }
                    state.app.measurement_state.spinorama_eq_state.progress = last_pct;
                    // Cycle through animated "Optimizing..." text — only
                    // when the run is still active.
                    if state
                        .app
                        .measurement_state
                        .spinorama_eq_state
                        .optimization_status
                        == crate::app::types::OptimizationStatus::Running
                    {
                        let dots = match (std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis()
                            / 500)
                            % 4
                        {
                            0 => "",
                            1 => ".",
                            2 => "..",
                            _ => "...",
                        };
                        state
                            .app
                            .measurement_state
                            .spinorama_eq_state
                            .status_message = format!("Optimizing{}", dots);
                    }
                    cx.notify();
                });
                smol::Timer::after(std::time::Duration::from_millis(50)).await;
            }
        })
        .detach();

        // Await the per-request outcome channel — no polling, no globals.
        let weak_state_outcome = self.state.downgrade();
        let cancel_for_poll = cancel_flag.clone();
        cx.spawn(async move |_, cx| {
            let Ok((success, result, full_result, error)) = outcome_rx.recv().await else {
                return;
            };
            let Some(state_for_poll) = weak_state_outcome.upgrade() else {
                return;
            };
            let was_cancelled = cancel_for_poll.load(std::sync::atomic::Ordering::Relaxed);
            state_for_poll.update(cx, |state, cx| {
                if was_cancelled {
                    // The optimizer may have returned Ok with partial
                    // results, but the user asked us to stop — surface
                    // Cancelled status and stay on the Configure step.
                    state
                        .app
                        .measurement_state
                        .spinorama_eq_state
                        .optimization_status = crate::app::types::OptimizationStatus::Cancelled;
                    state
                        .app
                        .measurement_state
                        .spinorama_eq_state
                        .status_message = "Optimization cancelled".to_string();
                } else if success {
                    state
                        .app
                        .measurement_state
                        .spinorama_eq_state
                        .optimization_status = crate::app::types::OptimizationStatus::Completed;
                    state
                        .app
                        .measurement_state
                        .spinorama_eq_state
                        .status_message = "Complete!".to_string();
                    state.app.measurement_state.spinorama_eq_state.progress = 1.0;
                    state.app.measurement_state.spinorama_eq_state.result = result;
                    state.app.measurement_state.spinorama_eq_state.full_result = full_result;
                    state.app.measurement_state.spinorama_eq_state.step =
                        crate::app::types::SpinoramaStep::Review;
                } else {
                    state
                        .app
                        .measurement_state
                        .spinorama_eq_state
                        .optimization_status = crate::app::types::OptimizationStatus::Failed;
                    state.app.measurement_state.spinorama_eq_state.error_message = error;
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn cancel_spinorama_optimization(&mut self, cx: &mut Context<Self>) {
        log::info!("Cancel requested for spinorama optimization");
        self.state.update(cx, |state, cx| {
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .cancel_requested
                .store(true, std::sync::atomic::Ordering::Relaxed);
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .status_message = "Cancelling — finishing current iteration...".to_string();
            cx.notify();
        });
    }

    pub(super) fn apply_spinorama_eq_result(&mut self, cx: &mut Context<Self>) {
        log::info!("Applying spinorama EQ result to playback...");

        // Get the result biquads
        let biquads = {
            let state = self.state.read(cx);
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .result
                .as_ref()
                .map(|r| r.biquads.clone())
        };

        let Some(biquads) = biquads else {
            self.state.update(cx, |state, _cx| {
                state.app.ui_state.toast_message =
                    Some(crate::app::ToastMessage::error("No EQ result to apply"));
            });
            cx.notify();
            return;
        };

        if biquads.is_empty() {
            self.state.update(cx, |state, _cx| {
                state.app.ui_state.toast_message =
                    Some(crate::app::ToastMessage::warning("No filters in EQ result"));
            });
            cx.notify();
            return;
        }

        // Convert to EQFilter instances
        let eq_filters: Vec<sotf_audio_player::EQFilter> = biquads
            .iter()
            .map(|b| {
                sotf_audio_player::EQFilter::new(
                    math_audio_iir_fir::BiquadFilterType::Peak,
                    b.freq,
                    b.q,
                    b.db_gain,
                )
            })
            .collect();

        let num_filters = eq_filters.len();

        // Update the plugin chain
        self.state.update(cx, |state, _| {
            let plugin_graph = &mut state.app.plugin_state.graph;

            // Check if there's an existing EQ plugin
            if let Some(eq_idx) = plugin_graph.find_plugin_index(&sotf_audio_player::PluginType::EQ)
            {
                // Update existing EQ plugin
                if let Some(eq_plugin) = plugin_graph.get_plugin_mut(eq_idx) {
                    let channels =
                        if let sotf_audio_player::PluginSettings::EQ { channels, .. } =
                            &eq_plugin.settings
                        {
                            *channels
                        } else {
                            2
                        };
                    eq_plugin.settings = sotf_audio_player::PluginSettings::EQ {
                        channels,
                        filters: eq_filters.clone(),
                        channel_filters: None,
                        per_channel_mode: false,
                        max_filters: 10,
                        tdf2: false,
                        topology: 0.0,
                        auto_gain_enabled: false,
                        oversampling: 1.0,
                    };
                    log::info!("Updated existing EQ plugin at index {}", eq_idx);
                }
            } else {
                // No EQ plugin exists, add one before monitoring plugins
                let insert_idx = plugin_graph.user_plugin_insert_index();
                let _ = plugin_graph.insert_plugin(insert_idx, &sotf_audio_player::PluginType::EQ);

                // Configure the newly inserted plugin
                if let Some(eq_plugin) = plugin_graph.get_plugin_mut(insert_idx) {
                    eq_plugin.settings = sotf_audio_player::PluginSettings::EQ {
                        channels: 2,
                        filters: eq_filters.clone(),
                        channel_filters: None,
                        per_channel_mode: false,
                        max_filters: 10,
                        tdf2: false,
                        topology: 0.0,
                        auto_gain_enabled: false,
                        oversampling: 1.0,
                    };
                }
                log::info!("Inserted new EQ plugin at index {}", insert_idx);
            }

            // Mark that plugin chain was modified and needs sync
            state.app.plugin_state.update_state.plugin_graph_modified = true;
            state.app.plugin_state.update_state.pending_plugin_update =
                Some(PluginUpdateType::Structural);
            // Invalidate the workflow canvas so the graph view rebuilds
            state.app.plugin_state.graph_state.workflow_canvas = None;
            state.app.ui_state.toast_message = Some(crate::app::ToastMessage::success(format!(
                "Applied {} filter Spinorama EQ",
                num_filters
            )));
        });
        cx.notify();
    }

    pub(super) fn save_spinorama_eq_result(&mut self, cx: &mut Context<Self>) {
        log::info!("Saving spinorama EQ result...");

        // Get the result and export format
        let (result, export_format, speaker_name) = {
            let state = self.state.read(cx);
            let result = state
                .app
                .measurement_state
                .spinorama_eq_state
                .result
                .clone();
            let format = state
                .app
                .measurement_state
                .spinorama_eq_state
                .export_format
                .clone();
            let speaker = state
                .app
                .measurement_state
                .spinorama_eq_state
                .selected_speaker
                .clone()
                .unwrap_or_else(|| "speaker".to_string());
            (result, format, speaker)
        };

        let Some(result) = result else {
            self.state.update(cx, |state, _cx| {
                state.app.ui_state.toast_message =
                    Some(crate::app::ToastMessage::error("No EQ result to save"));
            });
            cx.notify();
            return;
        };

        // Get file extension for format
        let extension = sotf_audio_player::autoeq::get_export_extension(&export_format);

        let safe_speaker_name = speaker_name.replace([' ', '/', '\\'], "_");
        let default_filename = format!("spinorama_eq_{}.{}", safe_speaker_name, extension);

        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            let weak_state = self.state.downgrade();
            cx.spawn(async move |_, cx| {
                // Open save file dialog
                let file = rfd::AsyncFileDialog::new()
                    .add_filter(extension.to_uppercase(), &[extension])
                    .set_title("Save Spinorama EQ")
                    .set_file_name(&default_filename)
                    .save_file()
                    .await;

                if let Some(file) = file {
                    let Some(state_entity) = weak_state.upgrade() else {
                        return;
                    };
                    // Export using the appropriate format function
                    let comment = format!(
                        "Spinorama EQ for {} ({})",
                        speaker_name,
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
                    );
                    let biquads: Vec<math_audio_iir_fir::Biquad> = result
                        .biquads
                        .iter()
                        .map(|b| {
                            let ft = match b.filter_type.as_str() {
                                "peak" => math_audio_iir_fir::BiquadFilterType::Peak,
                                "lowshelf" => math_audio_iir_fir::BiquadFilterType::Lowshelf,
                                "highshelf" => math_audio_iir_fir::BiquadFilterType::Highshelf,
                                "lowpass" => math_audio_iir_fir::BiquadFilterType::Lowpass,
                                "highpass" => math_audio_iir_fir::BiquadFilterType::Highpass,
                                _ => math_audio_iir_fir::BiquadFilterType::Peak,
                            };
                            math_audio_iir_fir::Biquad::new(ft, b.freq, 48000.0, b.q, b.db_gain)
                        })
                        .collect();
                    let content = match sotf_audio_player::autoeq::format_peq_export(
                        &export_format,
                        &comment,
                        &biquads,
                        48000,
                    ) {
                        Ok(c) => c,
                        Err(e) => {
                            log::error!("Format error: {e}");
                            state_entity.update(cx, |state, cx| {
                                state.app.ui_state.toast_message = Some(
                                    crate::app::ToastMessage::error(format!("Format error: {e}")),
                                );
                                cx.notify();
                            });
                            return;
                        }
                    };

                    match std::fs::write(file.path(), content) {
                        Ok(()) => {
                            log::info!("Saved Spinorama EQ to {:?}", file.path());
                            state_entity.update(cx, |state, cx| {
                                state.app.ui_state.toast_message =
                                    Some(crate::app::ToastMessage::success(format!(
                                        "Saved to {}",
                                        file.path().display()
                                    )));
                                cx.notify();
                            });
                        }
                        Err(e) => {
                            log::error!("Failed to save Spinorama EQ: {}", e);
                            state_entity.update(cx, |state, cx| {
                                state.app.ui_state.toast_message =
                                    Some(crate::app::ToastMessage::error(format!(
                                        "Failed to save: {}",
                                        e
                                    )));
                                cx.notify();
                            });
                        }
                    }
                }
            })
            .detach();
        }
    }
}
