//! Opens or exports the shared `AutoEQ` `RoomEQ` report.

// Rust guideline compliant 2026-02-21

use std::fmt::Write as _;

use super::PlayerView;
use crate::app::types::RoomEqResultIdentity;
use autoeq_report_gpui::native::{NativeReport, ReportView};
use gpui::{
    App, AppContext, AsyncApp, Entity, Subscription, TitlebarOptions, Window, WindowOptions, div,
    prelude::*,
};
#[cfg(all(
    feature = "report-webview",
    any(target_os = "macos", target_os = "windows")
))]
use sotf_report_host::ReportServer;
use sotf_report_host::{GeneratedReport, ReportDocument};
use std::sync::Arc;

/// Counts populated native report windows for read-only workflow verification.
#[cfg(feature = "dev-api")]
pub(crate) fn shared_report_window_count(cx: &mut App) -> usize {
    cx.windows()
        .into_iter()
        .filter(|handle| {
            handle
                .update(cx, |root, _, cx| {
                    root.downcast::<NativeReportWindow>()
                        .is_ok_and(|window| !window.read(cx).document.sections.is_empty())
                })
                .unwrap_or(false)
        })
        .count()
}

/// Counts the composed sections in open native report windows.
#[cfg(feature = "dev-api")]
pub(crate) fn native_report_section_count(cx: &mut App) -> usize {
    cx.windows()
        .into_iter()
        .map(|handle| {
            handle
                .update(cx, |root, _, cx| {
                    root.downcast::<NativeReportWindow>()
                        .map_or(0, |window| window.read(cx).document.sections.len())
                })
                .unwrap_or(0)
        })
        .sum()
}

impl PlayerView {
    pub(crate) fn open_shared_room_eq_report(&mut self, cx: &mut Context<Self>) {
        self.prepare_shared_room_eq_report(false, cx);
    }

    pub(crate) fn save_shared_room_eq_report(&mut self, cx: &mut Context<Self>) {
        self.prepare_shared_room_eq_report(true, cx);
    }

    #[expect(
        clippy::too_many_lines,
        reason = "Keep the ordered UI workflow and its state updates in one operation"
    )]
    fn prepare_shared_room_eq_report(&mut self, save: bool, cx: &mut Context<Self>) {
        let (result, artifacts, quality, dark, snapshot, labels) = {
            let app = &self.state.read(cx).app;
            let room = &app.measurement_state.room_eq_state;
            let labels = crate::app::i18n::SharedReportText::for_language(app.ui_state.language);
            let Some(snapshot) = room.current_result_identity() else {
                self.report_error(labels.stale.into(), cx);
                return;
            };
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
                snapshot,
                labels,
            )
        };
        self.state.update(cx, |state, _| {
            state.app.measurement_state.room_eq_state.status_message = labels.preparing.into();
        });
        let worker = cx.background_executor().spawn(async move {
            let document =
                sotf_report_host::compose_report(&result, artifacts.as_deref(), Some(&quality))?;
            Ok::<_, anyhow::Error>(Arc::new(document))
        });
        cx.spawn(async move |view, cx| match worker.await {
            Ok(document) => {
                if !view
                    .update(cx, |view, cx| view.report_result_is_current(snapshot, cx))
                    .unwrap_or(false)
                {
                    let _ = view.update(cx, |view, cx| view.report_error(labels.stale.into(), cx));
                    return;
                }
                if save {
                    // rfd has no iOS/tvOS backend (target-gated in
                    // Cargo.toml); on those platforms the save action
                    // falls through to the in-app report window below.
                    #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
                    {
                        let Some(path) = report_destination(labels.save).await else {
                            return;
                        };
                        if !view
                            .update(cx, |view, cx| view.report_result_is_current(snapshot, cx))
                            .unwrap_or(false)
                        {
                            let _ = view
                                .update(cx, |view, cx| view.report_error(labels.stale.into(), cx));
                            return;
                        }
                        let exported = export_document(document, cx).await;
                        if !view
                            .update(cx, |view, cx| view.report_result_is_current(snapshot, cx))
                            .unwrap_or(false)
                        {
                            let _ = view
                                .update(cx, |view, cx| view.report_error(labels.stale.into(), cx));
                            return;
                        }
                        let saved = match exported {
                            Ok(report) => save_exported_report(report, path, cx).await,
                            Err(error) => Err(error),
                        };
                        if let Err(error) = saved {
                            let _ = view
                                .update(cx, |view, cx| view.report_error(error.to_string(), cx));
                        }
                        return;
                    }
                }
                let native_document = std::sync::Arc::clone(&document);
                let report = cx
                    .background_executor()
                    .spawn(async move { NativeReport::try_complete(native_document) })
                    .await;
                if !view
                    .update(cx, |view, cx| view.report_result_is_current(snapshot, cx))
                    .unwrap_or(false)
                {
                    let _ = view.update(cx, |view, cx| view.report_error(labels.stale.into(), cx));
                    return;
                }
                if let Some(report) = report {
                    if let Err(error) = open_native_report_window(report, labels.save, dark, cx) {
                        let _ =
                            view.update(cx, |view, cx| view.report_error(error.to_string(), cx));
                    }
                    return;
                }
                let exported = export_document(document, cx).await;
                if !view
                    .update(cx, |view, cx| view.report_result_is_current(snapshot, cx))
                    .unwrap_or(false)
                {
                    let _ = view.update(cx, |view, cx| view.report_error(labels.stale.into(), cx));
                    return;
                }
                if let Err(error) = exported.and_then(|report| open_report_window(report, dark, cx))
                {
                    let _ = view.update(cx, |view, cx| view.report_error(error.to_string(), cx));
                }
            }
            Err(error) => {
                let _ = view.update(cx, |view, cx| {
                    view.report_error(format!("Cannot open AutoEQ report: {error:#}"), cx);
                });
            }
        })
        .detach();
    }

    fn report_result_is_current(&self, snapshot: RoomEqResultIdentity, cx: &App) -> bool {
        let room = &self.state.read(cx).app.measurement_state.room_eq_state;
        room.current_result_identity() == Some(snapshot)
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

#[cfg(not(any(target_os = "ios", target_os = "tvos")))]
async fn report_destination(label: &'static str) -> Option<std::path::PathBuf> {
    rfd::AsyncFileDialog::new()
        .add_filter("AutoEQ HTML", &["html"])
        .set_title(label)
        .set_file_name("roomeq-report.html")
        .save_file()
        .await
        .map(|file| file.path().to_owned())
}

#[cfg(not(any(target_os = "ios", target_os = "tvos")))]
async fn save_document(
    document: Arc<ReportDocument>,
    path: std::path::PathBuf,
    cx: &AsyncApp,
) -> anyhow::Result<()> {
    let report = export_document(document, cx).await?;
    save_exported_report(report, path, cx).await
}

async fn export_document(
    document: Arc<ReportDocument>,
    cx: &AsyncApp,
) -> anyhow::Result<GeneratedReport> {
    cx.background_executor()
        .spawn(async move { sotf_report_host::export_report(&document) })
        .await
}

#[cfg(not(any(target_os = "ios", target_os = "tvos")))]
async fn save_exported_report(
    report: GeneratedReport,
    path: std::path::PathBuf,
    cx: &AsyncApp,
) -> anyhow::Result<()> {
    cx.background_executor()
        .spawn(async move { report.save(&path) })
        .await
}

fn open_native_report_window(
    report: NativeReport,
    save_label: &'static str,
    dark: bool,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    cx.open_window(
        WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some(report.document().title().to_owned().into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        move |_, cx| {
            #[cfg(any(feature = "dev-api", not(any(target_os = "ios", target_os = "tvos"))))]
            let document = std::sync::Arc::clone(report.document());
            let view = cx.new(|_| ReportView::from_prepared(report).with_dark_appearance(dark));
            cx.new(|_| NativeReportWindow {
                view,
                #[cfg(any(feature = "dev-api", not(any(target_os = "ios", target_os = "tvos"))))]
                document,
                #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
                save_label,
                #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
                saving: false,
                #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
                save_error: None,
            })
        },
    )?;
    Ok(())
}

struct NativeReportWindow {
    view: Entity<ReportView>,
    #[cfg(any(feature = "dev-api", not(any(target_os = "ios", target_os = "tvos"))))]
    document: Arc<ReportDocument>,
    #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
    save_label: &'static str,
    #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
    saving: bool,
    #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
    save_error: Option<String>,
}

impl NativeReportWindow {
    #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
    fn save(&mut self, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.saving = true;
        self.save_error = None;
        let document = std::sync::Arc::clone(&self.document);
        let label = self.save_label;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let error = if let Some(path) = report_destination(label).await {
                save_document(document, path, cx).await.err()
            } else {
                None
            };
            let _ = view.update(cx, |view, cx| {
                view.saving = false;
                view.save_error = error.map(|error| error.to_string());
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for NativeReportWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = div().size_full().flex().flex_col();
        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        let root = root.child(
            div()
                .flex_none()
                .child(
                    gpui_ui_kit::Button::new("save-native-report", self.save_label)
                        .disabled(self.saving)
                        .on_click_event(cx.listener(|view, _, _, cx| view.save(cx))),
                )
                .when_some(self.save_error.as_ref(), |element, error| {
                    element.child(error.clone())
                }),
        );
        root.child(div().flex_1().min_h_0().child(self.view.clone()))
    }
}

fn capture_quality_report(
    channels: &[sotf_audio_player::room_eq_types::ChannelMeasurement],
) -> sotf_report_host::capture_quality::CaptureQualityReport {
    use sotf_report_host::capture_quality::CaptureQualityReport;
    let mut report = CaptureQualityReport::default();
    for channel in channels {
        if channel.driver_measurement_sets.is_empty() {
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
        } else {
            for (driver_index, driver) in channel.driver_measurement_sets.iter().enumerate() {
                for (index, result) in std::iter::once(&driver.measurement)
                    .chain(driver.multi_mic_measurements.iter())
                    .enumerate()
                {
                    report.sources.push(capture_quality_source(
                        &format!(
                            "{} / {}",
                            channel.channel_name,
                            driver.name.as_deref().map_or_else(
                                || format!("driver {}", driver_index + 1),
                                str::to_owned
                            )
                        ),
                        index,
                        result,
                        driver.provenance.get(index),
                    ));
                }
            }
        }
    }
    report
}

#[expect(
    clippy::too_many_lines,
    reason = "Keep the ordered UI workflow and its state updates in one operation"
)]
fn capture_quality_source(
    channel: &str,
    index: usize,
    result: &sotf_audio_player::recording_types::RecordingResult,
    provenance: Option<&sotf_audio_player::room_eq_types::MeasurementProvenance>,
) -> sotf_report_host::capture_quality::CaptureQualitySource {
    use sha2::{Digest, Sha256};

    use sotf_audio_player::capture_quality as capture;
    use sotf_report_host::capture_quality as report;
    let findings = result.quality.as_ref().map_or_default(|quality| {
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
                        capture::QualityStatus::Warning => report::CaptureQualityStatus::Warning,
                        capture::QualityStatus::Unknown => report::CaptureQualityStatus::Unknown,
                    },
                    value: finding.measured,
                    unit: unit.clone(),
                    band: finding.band.map(|band| report::CaptureQualityBand {
                        low_hz: band.low_hz,
                        high_hz: band.high_hz,
                    }),
                    method: Some(format!("{:?}", finding.method)),
                    policy: Some(format!("{:?}", finding.policy)),
                    threshold: finding
                        .threshold
                        .map(|threshold| report::CaptureQualityThreshold {
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
                        }),
                    unknown_reason: finding.unknown_reason.clone(),
                    recommendation: finding.recommendation.clone(),
                }
            })
            .collect::<Vec<_>>()
    });
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
    let source = serde_json::json!({"channel":channel,"index":index,"provenance":provenance,"measurement":result});
    let source_id = Sha256::digest(source.to_string().as_bytes()).iter().fold(
        String::with_capacity(64),
        |mut hex, byte| {
            let _ = write!(&mut hex, "{byte:02x}");
            hex
        },
    );
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
