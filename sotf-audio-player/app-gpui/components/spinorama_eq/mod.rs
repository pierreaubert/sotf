//! Spinorama EQ Screen
//!
//! Multi-step wizard for speaker EQ optimization using spinorama.org data:
//! 1. Select Speaker - Search and select speaker from spinorama.org API
//! 2. Configure - Optimization parameters and mode selection
//! 3. Optimize - Run optimization with progress display
//! 4. Review - View results, apply to playback, export

use crate::app::types::{PluginUpdateType, SpinoramaOptimizationMode, SpinoramaStep};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, HStack, StackAlign, StackSpacing, Text, TextSize,
    TextWeight, VStack,
};
use sotf_audio_player::autoeq::speaker::{
    CallbackConfig, MeasurementInput, SpeakerOptimizationCallback, SpeakerOptimizationConfig,
    SpeakerOptimizationProgress,
};
use sotf_audio_player::autoeq::types::SpeakerConfigType;
use std::sync::Mutex;

mod step_1_select;
mod step_2_configure;
mod step_3_review;

// Global for sharing optimization result between threads
// Format: (success, result, full_result, error_message)
static SPINORAMA_RESULT: Mutex<
    Option<(
        bool,
        Option<crate::app::types::SpinoramaEqResult>,
        Option<sotf_audio_player::autoeq::SpeakerOptimizationResult>,
        Option<String>,
    )>,
> = Mutex::new(None);

// Global mutex for sharing phase check results between threads
static PHASE_CHECK_RESULT: Mutex<Option<bool>> = Mutex::new(None);

// Global mutex for sharing optimization progress between threads
// Format: Vec<(iteration, loss, optional_score, progress_pct)>
static SPINORAMA_PROGRESS: Mutex<Vec<(usize, f64, Option<f64>, f32)>> = Mutex::new(Vec::new());

// Global mutex for sharing preview curves result between threads
// Format: Option<Result<PreviewCurves, error_string>>
static SPINORAMA_PREVIEW: Mutex<Option<Result<sotf_audio_player::autoeq::speaker::PreviewCurves, String>>> = Mutex::new(None);

// Global mutex for sharing spinorama CEA2034 curves result between threads
static SPINORAMA_CURVES: Mutex<Option<Result<crate::app::types::SpinoramaCurves, String>>> = Mutex::new(None);

/// Spawn a background thread to load CEA2034 spinorama curves for the plot.
fn spawn_spinorama_curves_thread(
    speaker: String,
    version: String,
) {
    // Clear previous result
    *SPINORAMA_CURVES.lock().unwrap() = None;

    std::thread::spawn(move || {
        log::info!("Loading spinorama CEA2034 curves for {} / {}", speaker, version);
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        let result = rt.block_on(async {
            // Fetch CEA2034 measurement data
            let plot_data = autoeq::read::fetch_measurement_plot_data(&speaker, &version, "CEA2034")
                .await
                .map_err(|e| format!("API error: {}", e))?;

            // Extract curves using original frequency grid
            let curves = autoeq::read::extract_cea2034_curves_original(&plot_data, "CEA2034")
                .map_err(|e| format!("Extraction error: {}", e))?;

            // Convert to our SpinoramaCurves format
            let on_axis = curves.get("On Axis").ok_or("On Axis curve not found")?;
            let frequencies: Vec<f64> = on_axis.freq.to_vec();

            // Get PIR (Estimated In-Room Response)
            let estimated_in_room = curves
                .get("Estimated In-Room Response")
                .map(|c| c.spl.to_vec())
                .unwrap_or_else(|| vec![0.0; frequencies.len()]);

            // Try to fetch directivity data (SPL Horizontal and SPL Vertical)
            let directivity = autoeq::read::fetch_directivity_data(&speaker, &version).await.ok();

            let (horizontal_directivity, vertical_directivity) = if let Some(dir) = directivity {
                let horizontal: Vec<crate::app::types::DirectivityCurve> = dir
                    .horizontal
                    .iter()
                    .map(|c| crate::app::types::DirectivityCurve {
                        angle: c.angle,
                        frequencies: c.freq.to_vec(),
                        spl: c.spl.to_vec(),
                    })
                    .collect();
                let vertical: Vec<crate::app::types::DirectivityCurve> = dir
                    .vertical
                    .iter()
                    .map(|c| crate::app::types::DirectivityCurve {
                        angle: c.angle,
                        frequencies: c.freq.to_vec(),
                        spl: c.spl.to_vec(),
                    })
                    .collect();
                (horizontal, vertical)
            } else {
                log::warn!("Directivity data not available for {} / {}", speaker, version);
                (Vec::new(), Vec::new())
            };

            let spinorama_curves = crate::app::types::SpinoramaCurves {
                frequencies: frequencies.clone(),
                on_axis: on_axis.spl.to_vec(),
                listening_window: curves
                    .get("Listening Window")
                    .map(|c| c.spl.to_vec())
                    .unwrap_or_else(|| vec![0.0; frequencies.len()]),
                early_reflections: curves
                    .get("Early Reflections")
                    .map(|c| c.spl.to_vec())
                    .unwrap_or_else(|| vec![0.0; frequencies.len()]),
                sound_power: curves
                    .get("Sound Power")
                    .map(|c| c.spl.to_vec())
                    .unwrap_or_else(|| vec![0.0; frequencies.len()]),
                early_reflections_di: curves
                    .get("Early Reflections DI")
                    .map(|c| c.spl.to_vec())
                    .unwrap_or_else(|| vec![0.0; frequencies.len()]),
                sound_power_di: curves
                    .get("Sound Power DI")
                    .map(|c| c.spl.to_vec())
                    .unwrap_or_else(|| vec![0.0; frequencies.len()]),
                estimated_in_room,
                horizontal_directivity,
                vertical_directivity,
            };

            Ok::<crate::app::types::SpinoramaCurves, String>(spinorama_curves)
        });

        match &result {
            Ok(curves) => {
                log::info!(
                    "Spinorama curves loaded: {} frequencies, {} horizontal, {} vertical",
                    curves.frequencies.len(),
                    curves.horizontal_directivity.len(),
                    curves.vertical_directivity.len()
                );
            }
            Err(e) => {
                log::error!("Failed to load spinorama curves: {}", e);
            }
        }
        *SPINORAMA_CURVES.lock().unwrap() = Some(result);
    });
}

/// Spawn a background thread to load preview curves for the Configure step.
fn spawn_preview_curves_thread(
    speaker: String,
    version: String,
    measurement: String,
    curve_name: String,
) {
    // Clear previous result
    *SPINORAMA_PREVIEW.lock().unwrap() = None;

    std::thread::spawn(move || {
        log::info!(
            "Loading preview curves for {} / {} / {} / {}",
            speaker, version, measurement, curve_name
        );
        let result = sotf_audio_player::autoeq::speaker::load_preview_curves(
            &speaker, &version, &measurement, &curve_name,
        );
        match &result {
            Ok(curves) => {
                log::info!(
                    "Preview curves loaded: {} frequencies, input range [{:.1}, {:.1}] dB",
                    curves.frequencies.len(),
                    curves.input_curve.iter().cloned().fold(f64::INFINITY, f64::min),
                    curves.input_curve.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                );
            }
            Err(e) => {
                log::error!("Failed to load preview curves: {}", e);
            }
        }
        *SPINORAMA_PREVIEW.lock().unwrap() = Some(result);
    });
}

/// Spawn a background task to check phase data availability for a speaker/version/measurement.
/// This updates the state asynchronously when the result is ready.
fn spawn_phase_data_check_thread(
    speaker: String,
    version: String,
    measurement: String,
) {
    *PHASE_CHECK_RESULT.lock().unwrap() = None;

    let curve_name = "Estimated In-Room Response".to_string();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        let has_phase = rt.block_on(async {
            match autoeq::read::read_spinorama(&speaker, &version, &measurement, &curve_name).await {
                Ok(curve) => curve.phase.is_some(),
                Err(e) => {
                    log::warn!("Failed to fetch curve for phase check: {}", e);
                    false
                }
            }
        });
        *PHASE_CHECK_RESULT.lock().unwrap() = Some(has_phase);
    });
}

impl PlayerView {
    // ========================================================================
    // Spinorama EQ Wizard Screen
    // ========================================================================

    /// Clear the spinorama EQ from the playback chain
    pub fn clear_spinorama_eq_from_playback(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Find and remove EQ plugins
            let plugins = state.app.plugin_chain.plugins();
            let eq_indices: Vec<_> = plugins
                .iter()
                .enumerate()
                .filter_map(|(i, p)| {
                    if matches!(p.plugin_type(), sotf_audio_player::PluginType::EQ) {
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect();

            // Remove in reverse order to maintain correct indices
            for idx in eq_indices.into_iter().rev() {
                state.app.plugin_chain.remove_plugin(idx);
            }

            state.app.pending_plugin_update = Some(PluginUpdateType::Structural);
            state.app.toast_message = Some(crate::app::ToastMessage::success(
                "Cleared EQ from playback",
            ));
        });
        cx.notify();
    }

    /// Main Spinorama EQ screen entry point (wizard)
    pub(crate) fn render_spinorama_eq_screen(
        &mut self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Check if we need to auto-fetch speakers before reading state
        let needs_fetch = {
            let state = self.state.read(cx);
            state.app.spinorama_eq_state.needs_speaker_refresh()
                && !state.app.spinorama_eq_state.loading_speakers
        };

        if needs_fetch {
            // Set loading flag immediately to prevent duplicate fetches
            self.state.update(cx, |state, _| {
                state.app.spinorama_eq_state.loading_speakers = true;
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
            let spinorama = &state.app.spinorama_eq_state;
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
                    state.app.spinorama_eq_state.selected_speaker.clone().unwrap_or_default(),
                    state.app.spinorama_eq_state.selected_version.clone(),
                )
            };
            log::info!("Auto-loading spinorama curves for {} / {}", speaker, version);
            self.state.update(cx, |state, _| {
                state.app.spinorama_eq_state.loading_spinorama_curves = true;
            });
            spawn_spinorama_curves_thread(speaker, version);

            // Poll for results
            let state_for_poll = self.state.clone();
            cx.spawn(async move |_, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(100))
                        .await;

                    let spinorama_result = {
                        let mut guard = SPINORAMA_CURVES.lock().unwrap();
                        guard.take()
                    };

                    if let Some(result) = spinorama_result {
                        let _ = state_for_poll.update(cx, |state, cx| {
                            state.app.spinorama_eq_state.loading_spinorama_curves = false;
                            match result {
                                Ok(curves) => {
                                    log::info!("Auto-loaded spinorama curves successfully");
                                    state.app.spinorama_eq_state.spinorama_curves = curves;
                                    state.app.spinorama_eq_state.spinorama_curves_error = None;
                                }
                                Err(e) => {
                                    log::error!("Failed to auto-load spinorama curves: {}", e);
                                    state.app.spinorama_eq_state.spinorama_curves_error = Some(e);
                                }
                            }
                            cx.notify();
                        });
                        break;
                    }
                }
            })
            .detach();
        }

        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let current_step = state.app.spinorama_eq_state.step;

        // Content for current step
        let content = match current_step {
            SpinoramaStep::SelectSpeaker => {
                self.render_spinorama_select_speaker(cx).into_any_element()
            }
            SpinoramaStep::Configure => self.render_spinorama_configure(cx).into_any_element(),
            SpinoramaStep::Review => self.render_spinorama_review(cx).into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .child(self.render_spinorama_header(cx))
            .child(
                div()
                    .id("spinorama-eq-content")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_4()
                    .child(content),
            )
    }

    /// Render the spinorama EQ screen header with step indicators
    fn render_spinorama_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let current_step = state.app.spinorama_eq_state.step;

        // Helper function to build step indicator
        let build_step_indicator =
            |step: SpinoramaStep, label: &'static str, number: u8, theme: &crate::theme::Theme| {
                let is_active = current_step == step;
                let is_past = current_step.index() > step.index();

                let (bg_color, text_color, border_color) = if is_active {
                    (theme.accent, theme.text_on_accent, theme.accent)
                } else if is_past {
                    (theme.success, theme.text_on_accent, theme.success)
                } else {
                    (theme.surface, theme.text_muted, theme.border)
                };

                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .align(StackAlign::Center)
                    .child(
                        div()
                            .w(px(28.0))
                            .h(px(28.0))
                            .rounded_full()
                            .bg(bg_color)
                            .border_2()
                            .border_color(border_color)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Text::new(number.to_string())
                                    .size(TextSize::Sm)
                                    .weight(TextWeight::Bold)
                                    .color(text_color),
                            ),
                    )
                    .child(
                        Text::new(label)
                            .size(TextSize::Sm)
                            .weight(if is_active {
                                TextWeight::Bold
                            } else {
                                TextWeight::Normal
                            })
                            .color(if is_active {
                                theme.text_primary
                            } else {
                                theme.text_muted
                            }),
                    )
            };

        // Build step connector
        let connector = |from: SpinoramaStep, theme: &crate::theme::Theme| {
            let is_completed = current_step.index() > from.index();
            div().w(px(32.0)).h(px(2.0)).bg(if is_completed {
                theme.success
            } else {
                theme.border
            })
        };

        div()
            .flex()
            .items_center()
            .justify_between()
            .px_6()
            .py_4()
            .bg(theme.background_secondary)
            .border_b_1()
            .border_color(theme.border)
            .child(
                HStack::new()
                    .spacing(StackSpacing::Lg)
                    .align(StackAlign::Center)
                    .child(
                        Text::new("Spinorama EQ")
                            .size(TextSize::Xl)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    )
                    .child(div().w(px(1.0)).h(px(24.0)).bg(theme.border))
                    .child(build_step_indicator(
                        SpinoramaStep::SelectSpeaker,
                        "Select",
                        1,
                        &theme,
                    ))
                    .child(connector(SpinoramaStep::SelectSpeaker, &theme))
                    .child(build_step_indicator(
                        SpinoramaStep::Configure,
                        "Configure",
                        2,
                        &theme,
                    ))
                    .child(connector(SpinoramaStep::Configure, &theme))
                    .child(build_step_indicator(
                        SpinoramaStep::Review,
                        "Review",
                        3,
                        &theme,
                    )),
            )
            .child(self.render_spinorama_nav_buttons(cx))
    }

    /// Render navigation buttons (Close/Back and Next/Finish)
    fn render_spinorama_nav_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let current_step = state.app.spinorama_eq_state.step;
        let can_go_next = state.app.spinorama_eq_state.can_advance();
        let is_busy = state.app.spinorama_eq_state.is_optimizing();
        let view = cx.entity().clone();

        let back_label = match current_step {
            SpinoramaStep::SelectSpeaker => "Close",
            _ => "Back",
        };
        let next_label = match current_step {
            SpinoramaStep::Review => "Finish",
            _ => "Next",
        };

        HStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Button::new("back", back_label)
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Md)
                    .disabled(is_busy)
                    .on_click({
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    match state.app.spinorama_eq_state.step {
                                        SpinoramaStep::SelectSpeaker => {
                                            // Go back to previous screen
                                            state.app.current_screen = state.app.last_screen;
                                        }
                                        _ => {
                                            // Go back to previous step
                                            if let Some(prev) =
                                                state.app.spinorama_eq_state.step.previous()
                                            {
                                                state.app.spinorama_eq_state.step = prev;
                                            }
                                        }
                                    }
                                });
                                cx.notify();
                            });
                        }
                    }),
            )
            .child(
                Button::new("next", next_label)
                    .variant(ButtonVariant::Primary)
                    .size(ButtonSize::Md)
                    .disabled(!can_go_next || is_busy)
                    .on_click({
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    match state.app.spinorama_eq_state.step {
                                        SpinoramaStep::Review => {
                                            // Finish - go back
                                            state.app.current_screen = state.app.last_screen;
                                        }
                                        _ => {
                                            // Go to next step
                                            if let Some(next) =
                                                state.app.spinorama_eq_state.step.next()
                                            {
                                                state.app.spinorama_eq_state.step = next;
                                            }
                                        }
                                    }
                                });

                                cx.notify();
                            });
                        }
                    }),
            )
    }

    // ========================================================================
    // Action Handlers
    // ========================================================================

    fn fetch_spinorama_speakers(&mut self, cx: &mut Context<Self>) {
        log::info!("Fetching spinorama speakers from API...");
        // Note: loading_speakers is set to true before spawning to prevent duplicate fetches
        self.state.update(cx, |state, _cx| {
            state.app.spinorama_eq_state.loading_speakers = true;
            state.app.spinorama_eq_state.error_message = None;
        });
        cx.notify();

        // Use a global mutex to share results between threads (like optimization does)
        static SPEAKERS_RESULT: std::sync::Mutex<Option<Result<Vec<String>, String>>> =
            std::sync::Mutex::new(None);

        // Clear any previous result
        *SPEAKERS_RESULT.lock().unwrap() = None;

        // Spawn a background thread with its own tokio runtime for the HTTP request
        std::thread::spawn(|| {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            let result = rt.block_on(async { autoeq::fetch_available_speakers().await });

            let mapped_result = result.map_err(|e| e.to_string());
            *SPEAKERS_RESULT.lock().unwrap() = Some(mapped_result);
        });

        // Poll for results from GPUI's async context
        let state_entity = self.state.clone();
        cx.spawn(async move |_, cx| {
            loop {
                smol::Timer::after(std::time::Duration::from_millis(100)).await;

                // Check if result is ready
                let result = SPEAKERS_RESULT.lock().unwrap().take();

                if let Some(result) = result {
                    match result {
                        Ok(speakers) => {
                            log::info!("Fetched {} speakers from spinorama.org", speakers.len());
                            let _ = state_entity.update(cx, |state, cx| {
                                state.app.spinorama_eq_state.available_speakers = speakers;
                                state.app.spinorama_eq_state.loading_speakers = false;
                                state.app.spinorama_eq_state.speakers_cached_at =
                                    Some(std::time::Instant::now());
                                state.app.spinorama_eq_state.update_suggestions();
                                cx.notify();
                            });
                        }
                        Err(e) => {
                            log::error!("Failed to fetch speakers: {}", e);
                            let _ = state_entity.update(cx, |state, cx| {
                                state.app.spinorama_eq_state.loading_speakers = false;
                                state.app.spinorama_eq_state.error_message =
                                    Some(format!("Failed to fetch speakers: {}", e));
                                cx.notify();
                            });
                        }
                    }
                    break;
                }
            }
        })
        .detach();
    }

    fn select_spinorama_speaker(&mut self, speaker: &str, cx: &mut Context<Self>) {
        log::info!("Selected speaker: {}", speaker);
        self.state.update(cx, |state, _cx| {
            state.app.spinorama_eq_state.selected_speaker = Some(speaker.to_string());
            // Reset phase data flag and clear version/measurement lists
            state.app.spinorama_eq_state.has_phase_data = false;
            state.app.spinorama_eq_state.available_versions.clear();
            state.app.spinorama_eq_state.available_measurements.clear();
            state.app.spinorama_eq_state.loading_versions = true;
        });
        cx.notify();

        // Fetch available versions for the selected speaker
        self.fetch_spinorama_versions(speaker, cx);
    }

    fn fetch_spinorama_versions(&mut self, speaker: &str, cx: &mut Context<Self>) {
        log::info!("Fetching versions for speaker: {}", speaker);
        let speaker_name = speaker.to_string();

        // Use a global mutex to share results
        static VERSIONS_RESULT: std::sync::Mutex<Option<Result<Vec<String>, String>>> =
            std::sync::Mutex::new(None);
        *VERSIONS_RESULT.lock().unwrap() = None;

        // Spawn background thread for HTTP request
        let speaker_for_fetch = speaker_name.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            let result = rt.block_on(async {
                let encoded_speaker = urlencoding::encode(&speaker_for_fetch);
                let url = format!(
                    "https://api.spinorama.org/v1/speaker/{}/versions",
                    encoded_speaker
                );
                let response = reqwest::get(&url).await?;
                if !response.status().is_success() {
                    return Err(format!("API request failed: {}", response.status()).into());
                }
                let versions: Vec<String> = response.json().await?;
                Ok::<Vec<String>, Box<dyn std::error::Error + Send + Sync>>(versions)
            });
            *VERSIONS_RESULT.lock().unwrap() = Some(result.map_err(|e| e.to_string()));
        });

        // Poll for results
        let state_entity = self.state.clone();
        let speaker_for_poll = speaker_name.clone();
        cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;

                let result = VERSIONS_RESULT.lock().unwrap().take();
                if let Some(result) = result {
                    match result {
                        Ok(versions) => {
                            log::info!("Fetched {} versions for {}", versions.len(), speaker_for_poll);
                            let first_version = versions.first().cloned();
                            let selected_version = first_version.clone();
                            let _ = state_entity.update(cx, |state, cx| {
                                state.app.spinorama_eq_state.available_versions = versions;
                                state.app.spinorama_eq_state.loading_versions = false;
                                // Auto-select first version if available
                                if let Some(ref version) = selected_version {
                                    state.app.spinorama_eq_state.selected_version = version.clone();
                                }
                                cx.notify();
                            });
                            // Fetch measurements for the selected version
                            if let Some(version) = selected_version {
                                let _ = view.update(cx, |view, cx| {
                                    view.fetch_spinorama_measurements(&speaker_for_poll, &version, cx);
                                });
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to fetch versions: {}", e);
                            let _ = state_entity.update(cx, |state, cx| {
                                state.app.spinorama_eq_state.loading_versions = false;
                                state.app.spinorama_eq_state.error_message =
                                    Some(format!("Failed to fetch versions: {}", e));
                                cx.notify();
                            });
                        }
                    }
                    break;
                }
            }
        })
        .detach();
    }

    fn fetch_spinorama_measurements(&mut self, speaker: &str, version: &str, cx: &mut Context<Self>) {
        log::info!("Fetching measurements for speaker: {}, version: {}", speaker, version);

        self.state.update(cx, |state, _cx| {
            state.app.spinorama_eq_state.loading_measurements = true;
            state.app.spinorama_eq_state.available_measurements.clear();
        });
        cx.notify();

        let speaker_name = speaker.to_string();
        let version_name = version.to_string();

        // Use a global mutex to share results
        static MEASUREMENTS_RESULT: std::sync::Mutex<Option<Result<Vec<String>, String>>> =
            std::sync::Mutex::new(None);
        *MEASUREMENTS_RESULT.lock().unwrap() = None;

        // Spawn background thread for HTTP request
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            let result = rt.block_on(async {
                let encoded_speaker = urlencoding::encode(&speaker_name);
                let encoded_version = urlencoding::encode(&version_name);
                let url = format!(
                    "https://api.spinorama.org/v1/speaker/{}/version/{}/measurements",
                    encoded_speaker, encoded_version
                );
                let response = reqwest::get(&url).await?;
                if !response.status().is_success() {
                    return Err(format!("API request failed: {}", response.status()).into());
                }
                let measurements: Vec<String> = response.json().await?;
                Ok::<Vec<String>, Box<dyn std::error::Error + Send + Sync>>(measurements)
            });
            *MEASUREMENTS_RESULT.lock().unwrap() = Some(result.map_err(|e| e.to_string()));
        });

        // Poll for results
        let state_entity = self.state.clone();
        let speaker_for_poll = speaker.to_string();
        let version_for_poll = version.to_string();
        cx.spawn(async move |_, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;

                let result = MEASUREMENTS_RESULT.lock().unwrap().take();
                if let Some(result) = result {
                    match result {
                        Ok(measurements) => {
                            log::info!("Fetched {} measurements for {}/{}", measurements.len(), speaker_for_poll, version_for_poll);
                            let has_cea2034 = measurements.iter().any(|m| m == "CEA2034");
                            // Determine which measurement to auto-select
                            let selected_measurement = if has_cea2034 {
                                "CEA2034".to_string()
                            } else {
                                measurements.first().cloned().unwrap_or_else(|| "CEA2034".to_string())
                            };
                            let measurement_for_phase = selected_measurement.clone();
                            let _ = state_entity.update(cx, |state, cx| {
                                state.app.spinorama_eq_state.available_measurements = measurements;
                                state.app.spinorama_eq_state.loading_measurements = false;
                                state.app.spinorama_eq_state.selected_measurement = selected_measurement;
                                // Auto-load spinorama curves if CEA2034 is selected
                                if has_cea2034 {
                                    state.app.spinorama_eq_state.loading_spinorama_curves = true;
                                }
                                cx.notify();
                            });
                            // Auto-load spinorama curves when CEA2034 is available
                            if has_cea2034 {
                                spawn_spinorama_curves_thread(speaker_for_poll.clone(), version_for_poll.clone());
                            }
                            // Check for phase data availability
                            spawn_phase_data_check_thread(speaker_for_poll.clone(), version_for_poll.clone(), measurement_for_phase);
                            // Continue polling for phase check and spinorama curves results
                            let mut phase_done = false;
                            let mut spinorama_done = !has_cea2034; // Skip if not CEA2034
                            loop {
                                cx.background_executor()
                                    .timer(std::time::Duration::from_millis(100))
                                    .await;

                                // Check for phase result
                                if !phase_done {
                                    if let Some(has_phase) = PHASE_CHECK_RESULT.lock().unwrap().take() {
                                        let _ = state_entity.update(cx, |state, cx| {
                                            state.app.spinorama_eq_state.has_phase_data = has_phase;
                                            log::info!("Phase data availability: {}", has_phase);
                                            cx.notify();
                                        });
                                        phase_done = true;
                                    }
                                }

                                // Check for spinorama curves result
                                if !spinorama_done {
                                    let spinorama_result = {
                                        let mut guard = SPINORAMA_CURVES.lock().unwrap();
                                        guard.take()
                                    };
                                    if let Some(result) = spinorama_result {
                                        let _ = state_entity.update(cx, |state, cx| {
                                            state.app.spinorama_eq_state.loading_spinorama_curves = false;
                                            match result {
                                                Ok(curves) => {
                                                    log::info!("Auto-loaded spinorama curves successfully");
                                                    state.app.spinorama_eq_state.spinorama_curves = curves;
                                                    state.app.spinorama_eq_state.spinorama_curves_error = None;
                                                }
                                                Err(e) => {
                                                    log::error!("Failed to auto-load spinorama curves: {}", e);
                                                    state.app.spinorama_eq_state.spinorama_curves_error = Some(e);
                                                }
                                            }
                                            cx.notify();
                                        });
                                        spinorama_done = true;
                                    }
                                }

                                if phase_done && spinorama_done {
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to fetch measurements: {}", e);
                            let _ = state_entity.update(cx, |state, cx| {
                                state.app.spinorama_eq_state.loading_measurements = false;
                                state.app.spinorama_eq_state.error_message =
                                    Some(format!("Failed to fetch measurements: {}", e));
                                cx.notify();
                            });
                        }
                    }
                    break;
                }
            }
        })
        .detach();
    }

    fn select_spinorama_version(&mut self, version: &str, cx: &mut Context<Self>) {
        log::info!("Selected version: {}", version);
        let speaker = {
            let state = self.state.read(cx);
            state.app.spinorama_eq_state.selected_speaker.clone()
        };

        self.state.update(cx, |state, _cx| {
            state.app.spinorama_eq_state.selected_version = version.to_string();
            state.app.spinorama_eq_state.has_phase_data = false;
        });
        cx.notify();

        // Fetch measurements for the new version
        if let Some(speaker) = speaker {
            self.fetch_spinorama_measurements(&speaker, version, cx);
        }
    }

    fn select_spinorama_measurement(&mut self, measurement: &str, cx: &mut Context<Self>) {
        log::info!("Selected measurement: {}", measurement);
        let (speaker, version, mode, target_curve) = {
            let state = self.state.read(cx);
            (
                state.app.spinorama_eq_state.selected_speaker.clone(),
                state.app.spinorama_eq_state.selected_version.clone(),
                state.app.spinorama_eq_state.optimizer_config.mode,
                state.app.spinorama_eq_state.optimizer_config.target_curve,
            )
        };

        self.state.update(cx, |state, _cx| {
            state.app.spinorama_eq_state.selected_measurement = measurement.to_string();
            state.app.spinorama_eq_state.has_phase_data = false;
            // Clear old preview data
            state.app.spinorama_eq_state.preview_frequencies.clear();
            state.app.spinorama_eq_state.preview_input_curve.clear();
            state.app.spinorama_eq_state.preview_target_curve.clear();
            state.app.spinorama_eq_state.preview_deviation_curve.clear();
            state.app.spinorama_eq_state.preview_error = None;
            // Clear old spinorama curves data
            state.app.spinorama_eq_state.spinorama_curves = Default::default();
            state.app.spinorama_eq_state.spinorama_curves_error = None;
        });
        cx.notify();

        // Check phase data and load preview for the new measurement
        if let Some(speaker_name) = speaker {
            let measurement_str = measurement.to_string();
            let version_str = if version.is_empty() { "asr".to_string() } else { version.clone() };

            // Check phase data
            spawn_phase_data_check_thread(speaker_name.clone(), version.clone(), measurement_str.clone());

            // Load preview curves
            let curve_name = if mode == SpinoramaOptimizationMode::FlatOnPir {
                target_curve.api_name().to_string()
            } else {
                "Estimated In-Room Response".to_string()
            };

            // Check if this is a CEA2034 measurement (for spinorama curves loading)
            let is_cea2034 = measurement_str == "CEA2034" || measurement_str == "CEA2034 Normalized";

            // Set loading state
            self.state.update(cx, |state, _| {
                state.app.spinorama_eq_state.loading_preview = true;
                if is_cea2034 {
                    state.app.spinorama_eq_state.loading_spinorama_curves = true;
                }
            });

            // Spawn background thread to load preview
            spawn_preview_curves_thread(
                speaker_name.clone(),
                version_str.clone(),
                measurement_str,
                curve_name,
            );

            // Spawn background thread to load spinorama curves (only for CEA2034)
            if is_cea2034 {
                spawn_spinorama_curves_thread(speaker_name, version_str);
            }

            // Start polling for phase check, preview result, and spinorama curves
            let state_for_poll = self.state.clone();
            cx.spawn(async move |_, cx| {
                let mut phase_done = false;
                let mut preview_done = false;
                let mut spinorama_done = !is_cea2034; // Skip if not CEA2034

                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(100))
                        .await;

                    // Check for phase result
                    if !phase_done {
                        if let Some(has_phase) = PHASE_CHECK_RESULT.lock().unwrap().take() {
                            let _ = state_for_poll.update(cx, |state, cx| {
                                state.app.spinorama_eq_state.has_phase_data = has_phase;
                                log::info!("Phase data availability: {}", has_phase);
                                cx.notify();
                            });
                            phase_done = true;
                        }
                    }

                    // Check for preview result
                    if !preview_done {
                        let preview_result = {
                            let mut guard = SPINORAMA_PREVIEW.lock().unwrap();
                            guard.take()
                        };

                        if let Some(result) = preview_result {
                            let _ = state_for_poll.update(cx, |state, cx| {
                                state.app.spinorama_eq_state.loading_preview = false;
                                match result {
                                    Ok(curves) => {
                                        state.app.spinorama_eq_state.preview_frequencies = curves.frequencies;
                                        state.app.spinorama_eq_state.preview_input_curve = curves.input_curve;
                                        state.app.spinorama_eq_state.preview_target_curve = curves.target_curve;
                                        state.app.spinorama_eq_state.preview_deviation_curve = curves.deviation_curve;
                                        state.app.spinorama_eq_state.preview_error = None;
                                    }
                                    Err(e) => {
                                        state.app.spinorama_eq_state.preview_error = Some(e);
                                    }
                                }
                                cx.notify();
                            });
                            preview_done = true;
                        }
                    }

                    // Check for spinorama curves result
                    if !spinorama_done {
                        let spinorama_result = {
                            let mut guard = SPINORAMA_CURVES.lock().unwrap();
                            guard.take()
                        };

                        if let Some(result) = spinorama_result {
                            let _ = state_for_poll.update(cx, |state, cx| {
                                state.app.spinorama_eq_state.loading_spinorama_curves = false;
                                match result {
                                    Ok(curves) => {
                                        state.app.spinorama_eq_state.spinorama_curves = curves;
                                        state.app.spinorama_eq_state.spinorama_curves_error = None;
                                    }
                                    Err(e) => {
                                        state.app.spinorama_eq_state.spinorama_curves_error = Some(e);
                                    }
                                }
                                cx.notify();
                            });
                            spinorama_done = true;
                        }
                    }

                    if phase_done && preview_done && spinorama_done {
                        break;
                    }
                }
            })
            .detach();
        }
    }

    fn start_spinorama_optimization(&mut self, cx: &mut Context<Self>) {
        log::info!("Starting spinorama optimization...");

        // Gather config from state
        let (speaker_name, version, measurement, curve_name, optimizer_config, mode, target_curve) = {
            let state = self.state.read(cx);
            let spinorama = &state.app.spinorama_eq_state;
            let speaker = spinorama.selected_speaker.clone().unwrap_or_default();
            let version = spinorama.selected_version.clone();
            let measurement = spinorama.selected_measurement.clone();
            let curve = spinorama.selected_curve.clone();
            let config = spinorama.optimizer_config.clone();
            let mode = spinorama.optimizer_config.mode;
            let target_curve = spinorama.optimizer_config.target_curve;
            (speaker, version, measurement, curve, config, mode, target_curve)
        };

        if speaker_name.is_empty() {
            self.state.update(cx, |state, _cx| {
                state.app.spinorama_eq_state.error_message =
                    Some("No speaker selected".to_string());
            });
            cx.notify();
            return;
        }

        self.state.update(cx, |state, _cx| {
            state.app.spinorama_eq_state.optimization_status =
                crate::app::types::OptimizationStatus::Running;
            state.app.spinorama_eq_state.status_message = "Loading measurement data...".to_string();
            state.app.spinorama_eq_state.progress = 0.0;
            state.app.spinorama_eq_state.progress_history.clear();
            state.app.spinorama_eq_state.error_message = None;
        });
        cx.notify();

        // Clear progress mutex for fresh start
        SPINORAMA_PROGRESS.lock().unwrap().clear();

        let state_entity = self.state.clone();

        // Build optimization params
        let loss = mode.to_loss_string().to_string();
        let algo = match optimizer_config.algorithm {
            crate::app::types::RoomEqAlgorithm::Cobyla => "nlopt:cobyla",
            crate::app::types::RoomEqAlgorithm::DifferentialEvolution => "autoeq:de",
            crate::app::types::RoomEqAlgorithm::NelderMead => "nlopt:neldermead",
        }
        .to_string();

        // Use the user-selected curve, or override based on mode
        let effective_curve_name = if curve_name.is_empty() {
            // Default to PIR if no curve selected
            "Estimated In-Room Response".to_string()
        } else if mode == crate::app::types::SpinoramaOptimizationMode::FlatOnPir {
            // For FlatOnPir mode, use the target curve selection
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
            speaker_name, effective_version, effective_measurement, effective_curve_name
        );
        log::info!(
            "Spinorama optimization mode: {:?}, loss={}, target_curve={:?}",
            mode, loss, target_curve
        );
        log::info!(
            "Spinorama optimization params: algo={}, maxeval={}, num_filters={}, population={}",
            algo, optimizer_config.max_iter, optimizer_config.num_filters, optimizer_config.population
        );

        let params = sotf_audio_player::autoeq::params::OptimizationParams {
            num_filters: optimizer_config.num_filters,
            sample_rate: 48000,
            min_db: optimizer_config.min_db,
            max_db: optimizer_config.max_db,
            min_q: optimizer_config.min_q,
            max_q: optimizer_config.max_q,
            min_freq: optimizer_config.min_freq,
            max_freq: optimizer_config.max_freq,
            maxeval: optimizer_config.max_iter,
            population: optimizer_config.population,
            de_f: optimizer_config.de_f,
            de_cr: optimizer_config.de_cr,
            strategy: optimizer_config.strategy.clone(),
            refine: optimizer_config.refine,
            local_algo: optimizer_config.local_algo.clone(),
            smooth: optimizer_config.smooth,
            peq_model: optimizer_config.peq_model.clone(),
            // Set very small tolerances to prevent early convergence - run full maxeval iterations
            tolerance: 1e-10,
            abs_tolerance: 1e-10,
            loss,
            algo,
            curve_name: effective_curve_name.clone(),
            ..Default::default()
        };

        // Run optimization in background thread (blocking tokio runtime)
        std::thread::spawn(move || {
            // Build the optimization config
            let config = SpeakerOptimizationConfig {
                config_type: SpeakerConfigType::Single,
                main_measurement: Some(MeasurementInput::Spinorama {
                    speaker: speaker_name.clone(),
                    version: effective_version,
                    measurement: effective_measurement,
                    curve_name: effective_curve_name.clone(),
                }),
                driver_measurements: Vec::new(),
                crossover_type: None,
                crossover_freq_hints: Vec::new(),
                params: params.clone(),
                callback_config: Some(CallbackConfig {
                    interval: 25,
                    include_biquads: true,
                    include_filter_response: true,
                }),
                target: None,
            };

            // Create callback for progress updates
            let max_iter = params.maxeval;
            let callback: SpeakerOptimizationCallback =
                Box::new(move |progress: &SpeakerOptimizationProgress| {
                    let progress_pct = progress.iteration as f32 / max_iter as f32;
                    let iter = progress.iteration;
                    let loss = progress.loss;
                    let score = progress.score;

                    // Push progress to global mutex for GPUI polling
                    if let Ok(mut progress_vec) = SPINORAMA_PROGRESS.lock() {
                        progress_vec.push((iter, loss, score, progress_pct));
                    }

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
                "OptimizationParams: algo={}, maxeval={}, population={}, num_filters={}",
                config.params.algo,
                config.params.maxeval,
                config.params.population,
                config.params.num_filters
            );
            log::info!(
                "OptimizationParams: strategy={}, de_f={}, de_cr={}, refine={}, local_algo={}",
                config.params.strategy,
                config.params.de_f,
                config.params.de_cr,
                config.params.refine,
                config.params.local_algo
            );
            log::info!(
                "OptimizationParams: tolerance={}, abs_tolerance={}, smooth={}, peq_model={}",
                config.params.tolerance,
                config.params.abs_tolerance,
                config.params.smooth,
                config.params.peq_model
            );
            log::info!(
                "OptimizationParams: min_db={}, max_db={}, min_q={}, max_q={}, min_freq={}, max_freq={}",
                config.params.min_db,
                config.params.max_db,
                config.params.min_q,
                config.params.max_q,
                config.params.min_freq,
                config.params.max_freq
            );
            let result = sotf_audio_player::autoeq::speaker::run_speaker_optimization_with_callback(
                &config,
                Some(callback),
            );

            // Update state with result (need to use smol to get back to GPUI context)
            smol::block_on(async {
                match result {
                    Ok(opt_result) => {
                        log::info!(
                            "Optimization complete: {} filters, loss {:.4} -> {:.4}",
                            opt_result.biquads.len(),
                            opt_result.initial_loss,
                            opt_result.final_loss
                        );

                        // Convert biquads to our result format
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

                        // Convert curves for plotting
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

                        // Note: We can't directly call state_entity.update() from a std::thread
                        // We need to use a channel or store in a shared Arc<Mutex<>>
                        // For now, we'll store in a temporary and poll from GPUI
                        // This is a limitation - ideally we'd use cx.spawn() but that requires async
                        log::info!("Storing optimization result with {} filters", biquads.len());

                        // Store result in a global for pickup (temporary hack)
                        let result = crate::app::types::SpinoramaEqResult {
                            biquads,
                            pre_score: opt_result.initial_loss,
                            post_score: opt_result.final_loss,
                            original_response: Some(original_response),
                            corrected_response: Some(corrected_response),
                            target_response: Some(target_response),
                        };

                        // Use parking_lot or std Mutex to share result
                        SPINORAMA_RESULT
                            .lock()
                            .unwrap()
                            .replace((true, Some(result), Some(opt_result), None));
                    }
                    Err(e) => {
                        log::error!("Optimization failed: {}", e);
                        SPINORAMA_RESULT
                            .lock()
                            .unwrap()
                            .replace((false, None, None, Some(e)));
                    }
                }
            });
        });

        // Start a polling timer to check for results and progress
        let state_for_poll = self.state.clone();
        cx.spawn(async move |_, cx| {
            loop {
                smol::Timer::after(std::time::Duration::from_millis(100)).await;

                // Check for progress updates and transfer to state
                let new_progress: Vec<(usize, f64, Option<f64>, f32)> = {
                    let mut progress_guard = SPINORAMA_PROGRESS.lock().unwrap();
                    std::mem::take(&mut *progress_guard)
                };

                if !new_progress.is_empty() {
                    let _ = state_for_poll.update(cx, |state, cx| {
                        // Append new progress points to history
                        for (iter, loss, score, _) in &new_progress {
                            state
                                .app
                                .spinorama_eq_state
                                .progress_history
                                .push((*iter, *loss, *score));
                        }
                        // Update progress from last entry
                        if let Some((_, _, _, pct)) = new_progress.last() {
                            state.app.spinorama_eq_state.progress = *pct;
                        }
                        cx.notify();
                    });
                }

                // Check if result is ready
                let result_ready = SPINORAMA_RESULT.lock().unwrap().take();

                if let Some((success, result, full_result, error)) = result_ready {
                    let _ = state_for_poll.update(cx, |state, cx| {
                        if success {
                            state.app.spinorama_eq_state.optimization_status =
                                crate::app::types::OptimizationStatus::Completed;
                            state.app.spinorama_eq_state.status_message = "Complete!".to_string();
                            state.app.spinorama_eq_state.progress = 1.0;
                            state.app.spinorama_eq_state.result = result;
                            state.app.spinorama_eq_state.full_result = full_result;
                            state.app.spinorama_eq_state.step =
                                crate::app::types::SpinoramaStep::Review;
                        } else {
                            state.app.spinorama_eq_state.optimization_status =
                                crate::app::types::OptimizationStatus::Failed;
                            state.app.spinorama_eq_state.error_message = error;
                        }
                        cx.notify();
                    });
                    break;
                }

                // Update progress message
                let _ = state_for_poll.update(cx, |state, cx| {
                    if state.app.spinorama_eq_state.optimization_status
                        == crate::app::types::OptimizationStatus::Running
                    {
                        // Cycle through messages
                        let dots = match (std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis()
                            / 500)
                            % 4
                        {
                            0 => "",
                            1 => ".",
                            2 => "..",
                            _ => "...",
                        };
                        state.app.spinorama_eq_state.status_message = format!("Optimizing{}", dots);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    fn apply_spinorama_eq_result(&mut self, cx: &mut Context<Self>) {
        log::info!("Applying spinorama EQ result to playback...");

        // Get the result biquads
        let biquads = {
            let state = self.state.read(cx);
            state
                .app
                .spinorama_eq_state
                .result
                .as_ref()
                .map(|r| r.biquads.clone())
        };

        let Some(biquads) = biquads else {
            self.state.update(cx, |state, _cx| {
                state.app.toast_message =
                    Some(crate::app::ToastMessage::error("No EQ result to apply"));
            });
            cx.notify();
            return;
        };

        if biquads.is_empty() {
            self.state.update(cx, |state, _cx| {
                state.app.toast_message =
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
                    autoeq_iir::BiquadFilterType::Peak,
                    b.freq,
                    b.q,
                    b.db_gain,
                )
            })
            .collect();

        let num_filters = eq_filters.len();

        // Update the plugin chain
        self.state.update(cx, |state, _| {
            let plugin_chain = &mut state.app.plugin_chain;

            // Check if there's an existing EQ plugin
            if let Some(eq_idx) = plugin_chain.find_plugin_index(&sotf_audio_player::PluginType::EQ)
            {
                // Update existing EQ plugin
                if let Some(eq_plugin) = plugin_chain.get_plugin_mut(eq_idx) {
                    eq_plugin.settings = sotf_audio_player::PluginSettings::EQ {
                        filters: eq_filters.clone(),
                    };
                    log::info!("Updated existing EQ plugin at index {}", eq_idx);
                }
            } else {
                // No EQ plugin exists, add one before monitoring plugins
                let insert_idx = plugin_chain.find_processing_insert_index();
                plugin_chain.insert_plugin(insert_idx, &sotf_audio_player::PluginType::EQ);

                // Configure the newly inserted plugin
                if let Some(eq_plugin) = plugin_chain.get_plugin_mut(insert_idx) {
                    eq_plugin.settings = sotf_audio_player::PluginSettings::EQ {
                        filters: eq_filters.clone(),
                    };
                }
                log::info!("Inserted new EQ plugin at index {}", insert_idx);
            }

            // Mark that plugin chain was modified and needs sync
            state.app.plugin_chain_modified = true;
            state.app.pending_plugin_update = Some(PluginUpdateType::Structural);
            state.app.toast_message = Some(crate::app::ToastMessage::success(&format!(
                "Applied {} filter Spinorama EQ",
                num_filters
            )));
        });
        cx.notify();
    }

    fn save_spinorama_eq_result(&mut self, cx: &mut Context<Self>) {
        log::info!("Saving spinorama EQ result...");

        // Get the result and export format
        let (result, export_format, speaker_name) = {
            let state = self.state.read(cx);
            let result = state.app.spinorama_eq_state.result.clone();
            let format = state.app.spinorama_eq_state.export_format.clone();
            let speaker = state
                .app
                .spinorama_eq_state
                .selected_speaker
                .clone()
                .unwrap_or_else(|| "speaker".to_string());
            (result, format, speaker)
        };

        let Some(result) = result else {
            self.state.update(cx, |state, _cx| {
                state.app.toast_message =
                    Some(crate::app::ToastMessage::error("No EQ result to save"));
            });
            cx.notify();
            return;
        };

        // Convert biquads to autoeq_iir::Peq for export (Vec<(f64, Biquad)> with preamp gains)
        let peq: autoeq_iir::Peq = result
            .biquads
            .iter()
            .map(|b| {
                (
                    0.0, // preamp gain
                    autoeq_iir::Biquad::new(
                        autoeq_iir::BiquadFilterType::Peak,
                        b.freq,
                        48000.0,
                        b.q,
                        b.db_gain,
                    ),
                )
            })
            .collect();

        // Get file extension for format
        let extension = sotf_audio_player::autoeq::get_export_extension(&export_format);

        let safe_speaker_name = speaker_name
            .replace(' ', "_")
            .replace('/', "_")
            .replace('\\', "_");
        let default_filename = format!("spinorama_eq_{}.{}", safe_speaker_name, extension);

        let state_entity = self.state.clone();
        cx.spawn(async move |_, cx| {
            // Open save file dialog
            let file = rfd::AsyncFileDialog::new()
                .add_filter(extension.to_uppercase(), &[extension])
                .set_title("Save Spinorama EQ")
                .set_file_name(&default_filename)
                .save_file()
                .await;

            if let Some(file) = file {
                // Export using the appropriate format function
                let comment = format!(
                    "# Spinorama EQ for {}\n# Generated: {}",
                    speaker_name,
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
                );
                let content = match export_format.as_str() {
                    "apo" => autoeq_iir::peq_format_apo(&comment, &peq),
                    "rme-channel" => autoeq_iir::peq_format_rme_channel(&peq),
                    "rme-room" => autoeq_iir::peq_format_rme_room(&peq, &peq),
                    "aupreset" => autoeq_iir::peq_format_aupreset(
                        &peq,
                        &format!("Spinorama EQ {}", speaker_name),
                    ),
                    _ => {
                        // JSON format - serialize the biquads directly
                        serde_json::to_string_pretty(&result.biquads).unwrap_or_default()
                    }
                };

                match std::fs::write(file.path(), content) {
                    Ok(()) => {
                        log::info!("Saved Spinorama EQ to {:?}", file.path());
                        let _ = state_entity.update(cx, |state, cx| {
                            state.app.toast_message = Some(crate::app::ToastMessage::success(
                                &format!("Saved to {}", file.path().display()),
                            ));
                            cx.notify();
                        });
                    }
                    Err(e) => {
                        log::error!("Failed to save Spinorama EQ: {}", e);
                        let _ = state_entity.update(cx, |state, cx| {
                            state.app.toast_message = Some(crate::app::ToastMessage::error(
                                &format!("Failed to save: {}", e),
                            ));
                            cx.notify();
                        });
                    }
                }
            }
        })
        .detach();
    }
}
