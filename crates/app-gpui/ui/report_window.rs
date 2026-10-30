//! Native window hosting the shared AutoEQ report and its WASM renderer.

// Rust guideline compliant 2026-02-21

use super::PlayerView;
use gpui::{prelude::*, *};
use sotf_report_host::GeneratedReport;
#[cfg(all(
    feature = "report-webview",
    any(target_os = "macos", target_os = "windows")
))]
use sotf_report_host::ReportServer;

/// Counts initialized shared report views for read-only workflow verification.
#[cfg(feature = "dev-api")]
pub(crate) fn shared_report_window_count(cx: &mut App) -> usize {
    #[cfg(all(
        feature = "report-webview",
        any(target_os = "macos", target_os = "windows")
    ))]
    {
        cx.windows()
            .into_iter()
            .filter(|handle| {
                handle
                    .update(cx, |root, _, cx| {
                        root.downcast::<SharedReportWindow>()
                            .map(|entity| {
                                let report = entity.read(cx);
                                report.webview.is_some() && report.error.is_none()
                            })
                            .unwrap_or(false)
                    })
                    .unwrap_or(false)
            })
            .count()
    }
    #[cfg(not(all(
        feature = "report-webview",
        any(target_os = "macos", target_os = "windows")
    )))]
    {
        let _ = cx;
        0
    }
}

impl PlayerView {
    pub(crate) fn open_shared_room_eq_report(&mut self, cx: &mut Context<Self>) {
        self.prepare_shared_room_eq_report(false, cx);
    }

    pub(crate) fn save_shared_room_eq_report(&mut self, cx: &mut Context<Self>) {
        self.prepare_shared_room_eq_report(true, cx);
    }

    fn prepare_shared_room_eq_report(&mut self, save: bool, cx: &mut Context<Self>) {
        let (result, artifacts, quality, dark, inputs, labels) = {
            let app = &self.state.read(cx).app;
            let room = &app.measurement_state.room_eq_state;
            let labels = crate::app::i18n::SharedReportText::for_language(app.ui_state.language);
            if !room.result_is_current() {
                self.report_error(labels.stale.into(), cx);
                return;
            }
            let Some(result) = room.dsp_output.as_ref() else {
                return;
            };
            let result = match serde_json::to_value(result) {
                Ok(value) => value,
                Err(error) => {
                    self.report_error(error.to_string(), cx);
                    return;
                }
            };
            (
                result,
                room.artifact_dir.clone(),
                capture_quality_report(&room.channel_measurements),
                app.ui_state.theme_id != crate::app::theme::ThemeId::Light,
                room.result_inputs,
                labels,
            )
        };
        self.state.update(cx, |state, _| {
            state.app.measurement_state.room_eq_state.status_message = labels.preparing.into();
        });
        let worker = cx.background_executor().spawn(async move {
            sotf_report_host::generate_report_with_quality(
                &result,
                artifacts.as_deref(),
                Some(&quality),
            )
        });
        cx.spawn(async move |view, cx| match worker.await {
            Ok(report) => {
                let current = view
                    .update(cx, |view, cx| {
                        let room = &view.state.read(cx).app.measurement_state.room_eq_state;
                        room.result_is_current() && room.result_inputs == inputs
                    })
                    .unwrap_or(false);
                if !current {
                    let _ = view.update(cx, |view, cx| view.report_error(labels.stale.into(), cx));
                    return;
                }
                if save {
                    // rfd has no iOS/tvOS backend (target-gated in
                    // Cargo.toml); on those platforms the save action
                    // falls through to the in-app report window below.
                    #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
                    {
                        let file = rfd::AsyncFileDialog::new()
                            .add_filter("AutoEQ HTML", &["html"])
                            .set_title(labels.save)
                            .set_file_name("roomeq-report.html")
                            .save_file()
                            .await;
                        if let Some(file) = file
                            && let Err(error) = report.save(file.path())
                        {
                            let _ = view
                                .update(cx, |view, cx| view.report_error(error.to_string(), cx));
                        }
                        return;
                    }
                }
                if let Err(error) = open_report_window(report, dark, cx) {
                    let _ = view.update(cx, |view, cx| view.report_error(error.to_string(), cx));
                }
            }
            Err(error) => {
                let _ = view.update(cx, |view, cx| {
                    view.report_error(format!("Cannot open AutoEQ report: {error:#}"), cx)
                });
            }
        })
        .detach();
    }

    fn report_error(&mut self, error: String, cx: &mut Context<Self>) {
        log::error!("Shared AutoEQ report failed: {error}");
        self.state.update(cx, |state, _| {
            state.app.measurement_state.room_eq_state.error_message = Some(error.clone());
            state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(error));
        });
        cx.notify();
    }
}

fn capture_quality_report(
    channels: &[sotf_audio_player::room_eq_types::ChannelMeasurement],
) -> sotf_report_host::capture_quality::CaptureQualityReport {
    use sotf_report_host::capture_quality::CaptureQualityReport;
    let mut report = CaptureQualityReport::default();
    for channel in channels {
        if !channel.driver_measurement_sets.is_empty() {
            for (driver_index, driver) in channel.driver_measurement_sets.iter().enumerate() {
                for (index, result) in std::iter::once(&driver.measurement)
                    .chain(driver.multi_mic_measurements.iter())
                    .enumerate()
                {
                    report.sources.push(capture_quality_source(
                        &format!(
                            "{} / {}",
                            channel.channel_name,
                            driver
                                .name
                                .as_deref()
                                .map(str::to_owned)
                                .unwrap_or_else(|| format!("driver {}", driver_index + 1))
                        ),
                        index,
                        result,
                        driver.provenance.get(index),
                    ));
                }
            }
        } else {
            for (index, result) in std::iter::once(&channel.measurement)
                .chain(channel.multi_mic_measurements.iter())
                .enumerate()
            {
                report.sources.push(capture_quality_source(
                    &channel.channel_name,
                    index,
                    result,
                    channel.provenance.get(index),
                ));
            }
        }
    }
    report
}

fn capture_quality_source(
    channel: &str,
    index: usize,
    result: &sotf_audio_player::recording_types::RecordingResult,
    provenance: Option<&sotf_audio_player::room_eq_types::MeasurementProvenance>,
) -> sotf_report_host::capture_quality::CaptureQualitySource {
    use sotf_audio_player::capture_quality as capture;
    use sotf_report_host::capture_quality as report;
    let findings = result
        .quality
        .as_ref()
        .map(|quality| {
            quality
                .findings
                .iter()
                .map(|finding| {
                    let unit = match finding.unit {
                        capture::QualityUnit::Percent => "%",
                        capture::QualityUnit::Decibels => "dB",
                        capture::QualityUnit::Samples => "samples",
                        capture::QualityUnit::Count => "count",
                    }
                    .to_owned();
                    report::CaptureQualityFinding {
                        metric: match finding.metric {
                            capture::QualityMetric::TotalHarmonicDistortion => {
                                report::CaptureQualityMetric::Thd
                            }
                            capture::QualityMetric::BroadbandSnr => {
                                report::CaptureQualityMetric::BroadbandSnr
                            }
                            capture::QualityMetric::BandSnr => {
                                report::CaptureQualityMetric::Other("Band SNR".into())
                            }
                            capture::QualityMetric::Clipping => {
                                report::CaptureQualityMetric::Other("Clipped samples".into())
                            }
                            capture::QualityMetric::TimingMarkers => {
                                report::CaptureQualityMetric::Other("Timing markers".into())
                            }
                        },
                        status: match finding.status {
                            capture::QualityStatus::Pass => report::CaptureQualityStatus::Pass,
                            capture::QualityStatus::Warning => {
                                report::CaptureQualityStatus::Warning
                            }
                            capture::QualityStatus::Unknown => {
                                report::CaptureQualityStatus::Unknown
                            }
                        },
                        value: finding.measured,
                        unit: unit.clone(),
                        band: finding.band.map(|band| report::CaptureQualityBand {
                            low_hz: band.low_hz,
                            high_hz: band.high_hz,
                        }),
                        method: Some(format!("{:?}", finding.method)),
                        policy: Some(format!("{:?}", finding.policy)),
                        threshold: finding.threshold.map(|threshold| {
                            report::CaptureQualityThreshold {
                                comparison: match threshold.comparison {
                                    capture::ThresholdComparison::AtMost => {
                                        report::CaptureThresholdComparison::LessThanOrEqual
                                    }
                                    capture::ThresholdComparison::AtLeast => {
                                        report::CaptureThresholdComparison::GreaterThanOrEqual
                                    }
                                    capture::ThresholdComparison::EqualTo => {
                                        report::CaptureThresholdComparison::EqualTo
                                    }
                                },
                                value: threshold.value,
                                unit,
                            }
                        }),
                        unknown_reason: finding.unknown_reason.clone(),
                        recommendation: finding.recommendation.clone(),
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let findings = if findings.is_empty() {
        vec![report::CaptureQualityFinding {
            metric: report::CaptureQualityMetric::Other("Capture quality".into()),
            status: report::CaptureQualityStatus::Unknown,
            value: None,
            unit: "evidence".into(),
            band: None,
            method: None,
            policy: None,
            threshold: None,
            unknown_reason: Some(
                "No typed capture-quality evidence is retained for this imported source".into(),
            ),
            recommendation: Some(
                "Review or repeat the source measurement before relying on its quality".into(),
            ),
        }]
    } else {
        findings
    };
    // Bind the viewer identity to this measured source, including its provenance
    // and evidence. A later retake produces a different report identity.
    use sha2::{Digest, Sha256};
    let source = serde_json::json!({"channel":channel,"index":index,"provenance":provenance,"measurement":result});
    let source_id = Sha256::digest(source.to_string().as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    report::CaptureQualitySource {
        source_id,
        channel: Some(channel.into()),
        mic: provenance.and_then(|source| source.mic_index.map(|mic| format!("{}", mic + 1))),
        position: provenance.and_then(|source| {
            source
                .mic_position_index
                .map(|position| format!("{}", position + 1))
        }),
        // Measurement order is not an acquisition take number; leave it unknown.
        take: None,
        stimulus: result
            .stimulus_config
            .as_ref()
            .map(|config| report::CaptureStimulusMetadata {
                signal_type: config.signal_type.clone(),
                sample_rate_hz: Some(config.sample_rate_hz),
                duration_secs: Some(f64::from(config.playback_duration_secs)),
                band: config
                    .low_frequency_hz
                    .zip(config.high_frequency_hz)
                    .map(|(low, high)| report::CaptureQualityBand {
                        low_hz: f64::from(low),
                        high_hz: f64::from(high),
                    }),
                level_dbfs: config.level_dbfs.map(f64::from),
            }),
        findings,
    }
}

#[cfg(all(
    feature = "report-webview",
    any(target_os = "macos", target_os = "windows")
))]
fn open_report_window(
    report: GeneratedReport,
    dark: bool,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let server = ReportServer::start(report.read_html()?)?;
    cx.open_window(
        WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some("RoomEQ — AutoEQ report".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        move |window, cx| cx.new(|cx| SharedReportWindow::new(report, server, dark, window, cx)),
    )?;
    Ok(())
}

#[cfg(not(all(
    feature = "report-webview",
    any(target_os = "macos", target_os = "windows")
)))]
fn open_report_window(
    _report: GeneratedReport,
    _dark: bool,
    _cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    anyhow::bail!("Embedded reports require the report-webview feature on macOS or Windows")
}

#[cfg(all(
    feature = "report-webview",
    any(target_os = "macos", target_os = "windows")
))]
struct SharedReportWindow {
    // Drop the child view before stopping its origin or deleting report files.
    webview: Option<wry::WebView>,
    _server: ReportServer,
    _report: GeneratedReport,
    error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

#[cfg(all(
    feature = "report-webview",
    any(target_os = "macos", target_os = "windows")
))]
impl SharedReportWindow {
    fn new(
        report: GeneratedReport,
        server: ReportServer,
        dark: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let url = server.url().to_owned();
        let navigation_url = url.clone();
        let report_id = server.report_id().to_owned();
        let builder = wry::WebViewBuilder::new()
            .with_url(&url)
            .with_bounds(webview_bounds(window))
            .with_initialization_script(sotf_report_host::initialization_script(&report_id, dark))
            .with_navigation_handler(move |candidate| {
                candidate.split('#').next() == Some(navigation_url.as_str())
            })
            .with_ipc_handler(move |request| {
                match sotf_report_host::parse_message(request.body(), &report_id) {
                    Ok(action) => log::debug!("AutoEQ report interaction: {action:?}"),
                    Err(error) => log::warn!("Rejected AutoEQ report interaction: {error}"),
                }
            });
        let (webview, error) = match builder.build_as_child(&*window) {
            Ok(view) => (Some(view), None),
            Err(error) => {
                log::error!("Cannot create shared report view: {error}");
                (
                    None,
                    Some(format!("Cannot create shared report view: {error}")),
                )
            }
        };
        let subscription = cx.observe_window_bounds(window, |this, window, cx| {
            if let Some(view) = &this.webview
                && let Err(error) = view.set_bounds(webview_bounds(window))
            {
                this.error = Some(error.to_string());
                cx.notify();
            }
        });
        Self {
            webview,
            _server: server,
            _report: report,
            error,
            _subscriptions: vec![subscription],
        }
    }
}

#[cfg(all(
    feature = "report-webview",
    any(target_os = "macos", target_os = "windows")
))]
fn webview_bounds(window: &Window) -> wry::Rect {
    let size = window.viewport_size();
    wry::Rect {
        position: wry::dpi::LogicalPosition::new(0.0, 0.0).into(),
        size: wry::dpi::LogicalSize::new(
            f64::from(f32::from(size.width)),
            f64::from(f32::from(size.height)),
        )
        .into(),
    }
}

#[cfg(all(
    feature = "report-webview",
    any(target_os = "macos", target_os = "windows")
))]
impl Render for SharedReportWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .when_some(self.error.as_ref(), |view, error| view.child(error.clone()))
    }
}
