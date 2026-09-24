//! Background multi-microphone capture workflow shared by desktop frontends.
//!
//! Loading validates a frozen plan; recording and processing require separate
//! user actions. No audio, file reading, or signal processing runs during polling.

use crate::capture_session::clock::io::process_capture_session;
use crate::capture_session::record::record_capture_session;
use crate::capture_session::{CaptureSessionPlan, ValidatedCaptureSession};
use crate::room_eq_types::{ChannelMeasurement, RoomEqMeasurementsFile};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

/// Current stage; processing success alone does not certify coherent evidence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CaptureStage {
    #[default]
    Empty,
    Loading,
    Ready,
    Recording,
    Cancelling,
    RawReady,
    Processing,
    Complete,
    Cancelled,
    Failed,
}

#[derive(Debug)]
struct CaptureReview {
    lines: Vec<String>,
    manifest: Option<PathBuf>,
    channels: Vec<ChannelMeasurement>,
}

#[derive(Debug)]
enum WorkerEvent {
    Loaded(Result<(ValidatedCaptureSession, PathBuf), String>),
    Progress(String),
    Recorded(Result<usize, String>),
    Processed(Result<CaptureReview, String>),
}

/// Owns one background operation and the evidence available for RoomEQ import.
#[derive(Debug, Default)]
pub struct CaptureWorkflow {
    pub stage: CaptureStage,
    pub message: String,
    pub review_lines: Vec<String>,
    pub qa_lines: Vec<String>,
    pub recording_manifest: Option<PathBuf>,
    plan: Option<ValidatedCaptureSession>,
    plan_directory: PathBuf,
    channels: Vec<ChannelMeasurement>,
    receiver: Option<mpsc::Receiver<WorkerEvent>>,
    cancel: Option<Arc<AtomicBool>>,
}

impl CaptureWorkflow {
    /// Indicates whether an operation owns the workflow.
    pub fn is_busy(&self) -> bool {
        self.receiver.is_some()
    }

    /// Indicates whether a validated plan is available for explicit recording.
    pub fn can_record(&self) -> bool {
        !self.is_busy() && self.plan.is_some()
    }

    /// Invalidates stale results when the frontend changes its path inputs.
    ///
    /// # Errors
    /// Rejects edits while a worker owns frozen inputs.
    pub fn inputs_changed(&mut self, plan_changed: bool) -> Result<(), String> {
        self.ensure_idle()?;
        self.clear_results();
        if plan_changed {
            self.plan = None;
            self.review_lines.clear();
        }
        self.stage = if self.plan.is_some() {
            CaptureStage::Ready
        } else {
            CaptureStage::Empty
        };
        self.message = "Inputs changed. Review the loaded plan before recording; new output directories must not exist.".into();
        Ok(())
    }

    /// Exposes processed channels without manufacturing evidence from raw takes.
    pub fn import_channels(&self) -> Option<Vec<ChannelMeasurement>> {
        self.can_import().then(|| self.channels.clone())
    }

    /// Checks import availability without cloning response arrays during rendering.
    pub fn can_import(&self) -> bool {
        !self.is_busy() && self.recording_manifest.is_some() && !self.channels.is_empty()
    }

    /// Reads a bounded session plan in the background, without opening audio devices.
    ///
    /// # Errors
    /// Rejects overlapping operations and worker creation failures.
    pub fn load_plan(&mut self, path: PathBuf) -> Result<(), String> {
        self.ensure_idle()?;
        self.plan = None;
        self.review_lines.clear();
        self.clear_results();
        self.start(CaptureStage::Loading, "Loading session plan", move |_| {
            WorkerEvent::Loaded(read_plan(&path))
        })
    }

    /// Records all declared microphones, keeping gains, geometry, and routes frozen.
    ///
    /// # Errors
    /// Requires an idle workflow, a validated plan, and a new output directory.
    /// Device and file failures are delivered by `poll`.
    pub fn record(&mut self, output: PathBuf) -> Result<(), String> {
        self.ensure_idle()?;
        let session = self
            .plan
            .clone()
            .ok_or("Load and review a session plan first")?;
        let directory = self.plan_directory.clone();
        self.clear_results();
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = Some(Arc::clone(&cancel));
        self.start(
            CaptureStage::Recording,
            "Starting simultaneous capture",
            move |sender| {
                let result =
                    record_capture_session(&session, &directory, &output, &cancel, |progress| {
                        let _ = sender.send(WorkerEvent::Progress(format!(
                            "Recording source {}/{}: {} (all microphones)",
                            progress.source_index + 1,
                            progress.source_count,
                            progress.source_id,
                        )));
                    })
                    .map(|manifest| manifest.takes.len());
                WorkerEvent::Recorded(result)
            },
        )
    }

    /// Requests capture cancellation; completed raw takes remain on disk.
    pub fn cancel_recording(&mut self) {
        if matches!(
            self.stage,
            CaptureStage::Recording | CaptureStage::Cancelling
        ) && let Some(cancel) = &self.cancel
        {
            cancel.store(true, Ordering::Relaxed);
            self.stage = CaptureStage::Cancelling;
            self.message = "Cancelling; preserving completed raw takes".into();
        }
    }

    /// Processes a saved raw session into a new directory without opening audio devices.
    ///
    /// # Errors
    /// Rejects overlapping operations and worker creation failures. Processing
    /// cannot be cancelled; `poll` reports failures and incomplete QA explicitly.
    pub fn process(&mut self, raw: PathBuf, output: PathBuf) -> Result<(), String> {
        self.ensure_idle()?;
        self.clear_results();
        // Saved raw sessions may come from a different plan. Do not leave an
        // unrelated plan armed for recording while displaying their results.
        self.plan = None;
        self.review_lines.clear();
        self.start(
            CaptureStage::Processing,
            "Correcting clocks and analyzing takes",
            move |_| WorkerEvent::Processed(process_and_review(&raw, &output)),
        )
    }

    /// Drains available worker messages without blocking the frontend.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Some(receiver) = &self.receiver {
            match receiver.try_recv() {
                Ok(WorkerEvent::Progress(message)) => {
                    if self.stage != CaptureStage::Cancelling {
                        self.message = message;
                    }
                    changed = true;
                }
                Ok(event) => {
                    self.receiver = None;
                    self.finish(event);
                    self.cancel = None;
                    return true;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.receiver = None;
                    self.cancel = None;
                    self.fail("Capture worker disconnected before returning a result".into());
                    return true;
                }
            }
        }
        changed
    }

    fn finish(&mut self, event: WorkerEvent) {
        match event {
            WorkerEvent::Loaded(Ok((session, directory))) => {
                self.review_lines = plan_review(&session);
                self.plan = Some(session);
                self.plan_directory = directory;
                self.stage = CaptureStage::Ready;
                self.message = "Plan valid. Review routes, calibration, positions, and frozen gains before recording. Hardware and take QA are unchecked.".into();
            }
            WorkerEvent::Recorded(Ok(count)) => {
                self.stage = CaptureStage::RawReady;
                self.message = format!(
                    "Saved {count} raw takes. Clock correction and measurement QA remain pending."
                );
            }
            WorkerEvent::Recorded(Err(error))
                if self
                    .cancel
                    .as_ref()
                    .is_some_and(|flag| flag.load(Ordering::Relaxed)) =>
            {
                self.stage = CaptureStage::Cancelled;
                self.message =
                    format!("Capture cancelled. Completed raw takes are preserved. {error}");
            }
            WorkerEvent::Processed(Ok(review)) => {
                self.qa_lines = review.lines;
                self.recording_manifest = review.manifest;
                self.channels = review.channels;
                self.stage = CaptureStage::Complete;
                self.message = if self.recording_manifest.is_some() {
                    "Processing finished. Review per-take QA before importing into RoomEQ."
                } else {
                    "Processing finished without a complete RoomEQ manifest. Import unavailable; review pending stages."
                }.into();
            }
            WorkerEvent::Loaded(Err(error))
            | WorkerEvent::Recorded(Err(error))
            | WorkerEvent::Processed(Err(error)) => self.fail(error),
            WorkerEvent::Progress(_) => {}
        }
    }

    fn ensure_idle(&self) -> Result<(), String> {
        if self.is_busy() {
            Err("Wait for the current capture operation to finish".into())
        } else {
            Ok(())
        }
    }

    fn clear_results(&mut self) {
        self.qa_lines.clear();
        self.recording_manifest = None;
        self.channels.clear();
    }

    fn fail(&mut self, message: String) {
        self.stage = CaptureStage::Failed;
        self.message = message;
    }

    fn start(
        &mut self,
        stage: CaptureStage,
        message: &str,
        work: impl FnOnce(mpsc::Sender<WorkerEvent>) -> WorkerEvent + Send + 'static,
    ) -> Result<(), String> {
        let (sender, receiver) = mpsc::channel();
        if let Err(error) = std::thread::Builder::new()
            .name("roomeq-capture".into())
            .spawn(move || {
                let result = work(sender.clone());
                let _ = sender.send(result);
            })
        {
            self.cancel = None;
            let error = format!("Cannot start capture worker: {error}");
            self.fail(error.clone());
            return Err(error);
        }
        self.stage = stage;
        self.message = message.into();
        self.receiver = Some(receiver);
        Ok(())
    }
}

impl Drop for CaptureWorkflow {
    fn drop(&mut self) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}

fn read_plan(path: &Path) -> Result<(ValidatedCaptureSession, PathBuf), String> {
    // More than enough for four microphones and bounded hardware source routes.
    const PLAN_LIMIT: u64 = 1_048_576;
    let file =
        std::fs::File::open(path).map_err(|error| format!("Cannot open session plan: {error}"))?;
    let mut json = Vec::new();
    file.take(PLAN_LIMIT + 1)
        .read_to_end(&mut json)
        .map_err(|error| error.to_string())?;
    if json.len() as u64 > PLAN_LIMIT {
        return Err("Session plan exceeds 1 MiB".into());
    }
    let plan: CaptureSessionPlan = serde_json::from_slice(&json)
        .map_err(|error| format!("Cannot parse session plan: {error}"))?;
    let directory = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .to_path_buf();
    Ok((
        plan.validate().map_err(|error| error.to_string())?,
        directory,
    ))
}

fn plan_review(session: &ValidatedCaptureSession) -> Vec<String> {
    let plan = session.plan();
    let mut lines = vec![
        format!(
            "{:?} geometry | {} Hz | output {}",
            plan.geometry, plan.sample_rate_hz, plan.output_device
        ),
        format!(
            "Sweep: {}–{} Hz, {} s, peak {}",
            plan.sweep.start_hz, plan.sweep.end_hz, plan.sweep.duration_secs, plan.sweep.amplitude
        ),
    ];
    for mic in &plan.microphones {
        lines.push(format!(
            "{}: {} input {} | gain {} dB | {:?} m ±{} mm",
            mic.id,
            mic.device,
            mic.input_channel + 1,
            mic.gain_db,
            mic.position_m,
            mic.position_uncertainty_mm
        ));
        lines.push(format!(
            "  Calibration: {} ({:?})",
            mic.calibration_file.display(),
            mic.calibration_orientation
        ));
    }
    for source in &plan.sources {
        lines.push(format!(
            "Source {} → output {}",
            source.id,
            source.output_channel + 1
        ));
    }
    lines.push(match &plan.timing_reference {
        Some(reference) => format!(
            "Timing emitter: output {}, {:?} m ±{} mm; c={} ±{} m/s",
            reference.output_channel + 1,
            reference.position_m,
            reference.position_uncertainty_mm,
            reference.sound_speed_m_s,
            reference.sound_speed_uncertainty_m_s
        ),
        None => "No surveyed timing emitter: magnitude-only; coherent timing unavailable".into(),
    });
    lines.push("Use stationary stands, orientation-matched calibration, fixed gains, and a quiet room. Channel numbers shown are 1-based.".into());
    lines
}

fn process_and_review(raw: &Path, output: &Path) -> Result<CaptureReview, String> {
    let report = process_capture_session(raw, output)?;
    let session = report
        .plan
        .clone()
        .validate()
        .map_err(|error| error.to_string())?;
    let mut lines = vec![format!(
        "{:?} geometry | {} processed takes",
        report.plan.geometry,
        report.takes.len()
    )];
    lines.extend(plan_review(&session));
    for take in &report.takes {
        lines.push(format!(
            "{} / {}: offset {:?} samples, skew {:?} ppm, bound {:?} µs",
            take.raw.source_id,
            take.raw.microphone_id,
            take.clock.offset_samples,
            take.clock.skew_ppm,
            take.clock.residual_uncertainty_us
        ));
        if let Some(reason) = &take.clock.magnitude_only_reason {
            lines.push(format!("  Magnitude-only: {reason}"));
        }
        lines.push(format!(
            "  Correction: {}; clipped samples: {}",
            take.clock.correction_applied, take.raw.clipped_samples
        ));
        if let Some(analysis) = &take.analysis {
            lines.push(format!(
                "  Broadband SNR: {:?} dB (unknown is not passing)",
                analysis.broadband_snr_db
            ));
            for band in &analysis.frequency_snr {
                lines.push(format!(
                    "  {}–{} Hz SNR: {:?} dB",
                    band.low_hz, band.high_hz, band.snr_db
                ));
            }
            if let Some(phase) = &analysis.common_reference {
                lines.push(format!(
                    "  Conditional shared phase bandwidth: ≤{} Hz",
                    phase.timing_limit_hz
                ));
            } else {
                lines.push("  Shared-reference phase unavailable".into());
            }
            for issue in &analysis.issues {
                lines.push(format!("  QA: {issue}"));
            }
        } else {
            lines.push("  Analysis unavailable; measurement QA pending".into());
        }
    }
    for report in &report.reflection_reports {
        lines.push(format!(
            "{}: {} early-arrival candidates",
            report.source_id,
            report.early_reflections.len()
        ));
        lines.extend(report.issues.iter().map(|issue| format!("  {issue}")));
        for arrival in report.direct_sound.iter().chain(&report.early_reflections) {
            lines.push(format!(
                "  {:.2} ms: direction {:?}, mirror ambiguity {}",
                arrival.relative_ms, arrival.direction, arrival.mirror_ambiguous
            ));
            lines.extend(arrival.issues.iter().map(|issue| format!("    {issue}")));
        }
    }
    lines.push(format!(
        "Pending: {}",
        if report.pending_processing.is_empty() {
            "none".into()
        } else {
            report.pending_processing.join(", ")
        }
    ));
    let manifest = report.recording_manifest.map(|name| output.join(name));
    let channels = if let Some(path) = &manifest {
        let json = std::fs::read_to_string(path)
            .map_err(|error| format!("Cannot read processed RoomEQ manifest: {error}"))?;
        RoomEqMeasurementsFile::load_from_json(&json, Some(output))?
    } else {
        Vec::new()
    };
    Ok(CaptureReview {
        lines,
        manifest,
        channels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> ValidatedCaptureSession {
        serde_json::from_str::<CaptureSessionPlan>(include_str!(
            "../../tests/fixtures/capture-session.json"
        ))
        .unwrap()
        .validate()
        .unwrap()
    }

    #[expect(
        clippy::field_reassign_with_default,
        reason = "Drop prevents struct update syntax"
    )]
    fn pending(stage: CaptureStage) -> (CaptureWorkflow, mpsc::Sender<WorkerEvent>) {
        let (sender, receiver) = mpsc::channel();
        let mut workflow = CaptureWorkflow::default();
        workflow.stage = stage;
        workflow.receiver = Some(receiver);
        (workflow, sender)
    }

    #[test]
    fn plan_validation_never_claims_hardware_or_measurement_acceptance() {
        let (mut workflow, sender) = pending(CaptureStage::Loading);
        sender
            .send(WorkerEvent::Loaded(Ok((
                session(),
                PathBuf::from("calibrations"),
            ))))
            .unwrap();
        assert!(workflow.poll());
        assert_eq!(workflow.stage, CaptureStage::Ready);
        assert!(workflow.can_record());
        assert!(workflow.message.contains("unchecked"));
        assert!(
            workflow
                .review_lines
                .iter()
                .any(|line| line.contains("Calibration:"))
        );
        assert!(workflow.import_channels().is_none());
        workflow.inputs_changed(true).unwrap();
        assert!(!workflow.can_record());
        assert!(workflow.review_lines.is_empty());
    }

    #[test]
    fn worker_ownership_blocks_overlapping_operations_and_edits() {
        let (mut workflow, _sender) = pending(CaptureStage::Recording);
        assert!(workflow.load_plan("unused".into()).is_err());
        assert!(workflow.record("unused".into()).is_err());
        assert!(workflow.process("unused".into(), "unused".into()).is_err());
        assert!(workflow.inputs_changed(true).is_err());
        assert_eq!(workflow.stage, CaptureStage::Recording);
        assert!(!workflow.poll());
    }

    #[test]
    fn cancellation_waits_for_worker_and_preserves_raw_only_status() {
        let (mut workflow, sender) = pending(CaptureStage::Recording);
        let cancel = Arc::new(AtomicBool::new(false));
        workflow.cancel = Some(Arc::clone(&cancel));
        workflow.cancel_recording();
        assert!(cancel.load(Ordering::Relaxed));
        assert!(workflow.is_busy());
        sender
            .send(WorkerEvent::Progress("late progress".into()))
            .unwrap();
        workflow.poll();
        assert!(workflow.message.starts_with("Cancelling"));
        sender
            .send(WorkerEvent::Recorded(Err("cancelled".into())))
            .unwrap();
        workflow.poll();
        assert_eq!(workflow.stage, CaptureStage::Cancelled);
        assert!(!workflow.is_busy());
        assert!(workflow.import_channels().is_none());
    }

    #[test]
    fn raw_completion_and_disconnection_never_expose_importable_measurements() {
        let (mut workflow, sender) = pending(CaptureStage::Recording);
        sender.send(WorkerEvent::Recorded(Ok(4))).unwrap();
        workflow.poll();
        assert_eq!(workflow.stage, CaptureStage::RawReady);
        assert!(workflow.message.contains("pending"));
        assert!(workflow.import_channels().is_none());
        let (mut workflow, sender) = pending(CaptureStage::Processing);
        drop(sender);
        assert!(workflow.poll());
        assert_eq!(workflow.stage, CaptureStage::Failed);
        assert!(!workflow.is_busy());
    }

    #[test]
    fn incomplete_processing_reports_pending_evidence_and_no_import() {
        let (mut workflow, sender) = pending(CaptureStage::Processing);
        sender
            .send(WorkerEvent::Processed(Ok(CaptureReview {
                lines: vec!["Pending: missing take".into()],
                manifest: None,
                channels: vec![],
            })))
            .unwrap();
        workflow.poll();
        assert_eq!(workflow.stage, CaptureStage::Complete);
        assert!(workflow.message.contains("Import unavailable"));
        assert_eq!(workflow.qa_lines, ["Pending: missing take"]);
        assert!(workflow.import_channels().is_none());
    }

    #[test]
    fn completed_manifest_enables_import_until_inputs_change() {
        let mut config = autoeq::RoomConfig::default();
        config.speakers.insert(
            "L".into(),
            autoeq::SpeakerConfig::Single(autoeq::MeasurementSource::Single(
                autoeq::read::MeasurementSingle {
                    measurement: autoeq::read::MeasurementRef::Inline(
                        autoeq::read::InlineMeasurement {
                            frequencies: vec![100.0, 1000.0],
                            magnitude_db: vec![0.0, -1.0],
                            phase_deg: None,
                            name: None,
                            wav_path: None,
                            csv_path: None,
                        },
                    ),
                    speaker_name: None,
                    provenance: Default::default(),
                },
            )),
        );
        let channels =
            RoomEqMeasurementsFile::load_from_json(&serde_json::to_string(&config).unwrap(), None)
                .unwrap();
        let (mut workflow, sender) = pending(CaptureStage::Processing);
        sender
            .send(WorkerEvent::Processed(Ok(CaptureReview {
                lines: vec!["Magnitude-only".into()],
                manifest: Some("recordings.json".into()),
                channels,
            })))
            .unwrap();
        workflow.poll();
        assert_eq!(workflow.import_channels().unwrap()[0].channel_name, "L");
        workflow.inputs_changed(false).unwrap();
        assert!(workflow.import_channels().is_none());
        assert!(workflow.recording_manifest.is_none());
    }

    #[test]
    #[expect(
        clippy::field_reassign_with_default,
        reason = "Drop prevents struct update syntax"
    )]
    fn dropping_workflow_requests_recording_cancellation() {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut workflow = CaptureWorkflow::default();
        workflow.cancel = Some(Arc::clone(&cancel));
        drop(workflow);
        assert!(cancel.load(Ordering::Relaxed));
    }

    #[test]
    fn plan_reader_resolves_calibration_base_and_bounds_input() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("plan.json");
        std::fs::write(
            &path,
            include_str!("../../tests/fixtures/capture-session.json"),
        )
        .unwrap();
        let (_, base) = read_plan(&path).unwrap();
        assert_eq!(base, directory.path());
        std::fs::write(&path, vec![b' '; 1_048_577]).unwrap();
        assert!(read_plan(&path).unwrap_err().contains("1 MiB"));
    }
}
