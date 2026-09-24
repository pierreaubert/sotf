//! Worker-thread acquisition and durable raw artifacts for multi-microphone sessions.
//!
//! Raw acquisition is intentionally not an import-ready corrected measurement.
//! The journal records pending clock/calibration processing rather than inventing
//! timing certainty. Completed sources survive a later device failure or cancel.

use super::protocol::{CaptureStimulusLayout, prepare_capture_stimulus};
use super::{CaptureSessionPlan, ValidatedCaptureSession};
use crate::recording_helpers::save_recording_session_json;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sotf_audio::signal_recorder::multi_capture::{
    CaptureInput, CaptureOutputSegment, MultiCaptureRequest, MultiCaptureResult,
    capture_multidevice,
};
use sotf_audio::signal_recorder::{CancelFlag, write_wav_file};
use std::path::Path;
use std::sync::atomic::Ordering;

/// Raw acquisition state; completion does not imply coherent measurement validity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawCaptureStatus {
    /// One or more sources remain to be recorded.
    Capturing,
    /// All raw takes exist; timing and calibrated analysis remain pending.
    RawComplete,
    /// User cancellation stopped acquisition.
    Cancelled,
    /// Acquisition or artifact writing failed.
    Failed,
}

/// An immutable calibration snapshot associated with one physical microphone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureCalibration {
    /// Microphone identity from the session declaration.
    pub microphone_id: String,
    /// Snapshot file relative to the capture directory.
    pub file: String,
    /// SHA-256 of the exact snapshot bytes.
    pub sha256: String,
}

/// One source/microphone pair captured on its original device clock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawCaptureTake {
    /// Actual device ID returned by the audio backend.
    pub device_id: String,
    /// Actual output device ID returned by the audio backend.
    pub output_device_id: String,
    /// Source label from the session declaration.
    pub source_id: String,
    /// Microphone label from the session declaration.
    pub microphone_id: String,
    /// Raw float WAV relative to the session directory.
    pub wav_file: String,
    /// Number of raw device-clock samples, including pre/post-roll.
    pub samples: usize,
    /// Negotiated input PCM format before float conversion.
    pub input_sample_format: String,
    /// Negotiated playback PCM format.
    pub output_sample_format: String,
    /// Peak absolute raw amplitude; values at or above 1 require clipping review.
    pub peak_amplitude: f32,
    /// Number of raw samples at or above full scale.
    pub clipped_samples: usize,
}

/// Durable acquisition journal, separate from corrected recording imports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawCaptureManifest {
    /// Raw artifact schema version.
    pub version: u32,
    /// Frozen user declaration, including geometry, gains, and orientations.
    pub plan: CaptureSessionPlan,
    /// Explicit timing-chirp and sweep positions on the stimulus clock.
    pub stimulus: CaptureStimulusLayout,
    /// Fixed chirp output route; absent in older journals that used each source.
    #[serde(default)]
    pub timing_reference_output_channel: Option<u16>,
    /// Raw acquisition lifecycle state.
    pub status: RawCaptureStatus,
    /// Processing still required before measurement import/coherent consumers.
    pub pending_processing: Vec<String>,
    /// Calibration snapshots made before opening devices.
    pub calibrations: Vec<CaptureCalibration>,
    /// Complete raw source/microphone recordings saved so far.
    pub takes: Vec<RawCaptureTake>,
    /// Failure or cancellation reason, when acquisition did not complete.
    pub error: Option<String>,
}

/// Progress notification emitted on the caller's worker thread.
#[derive(Debug, Clone)]
pub struct CaptureProgress {
    /// Zero-based index of the source about to be captured.
    pub source_index: usize,
    /// Total number of sequential sources.
    pub source_count: usize,
    /// Source label for frontend display.
    pub source_id: String,
}

fn save_manifest(directory: &Path, manifest: &RawCaptureManifest) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(manifest)
        .map_err(|e| format!("cannot serialize capture journal: {e}"))?;
    save_recording_session_json(&directory.join("capture-raw.json"), &json)
        .map_err(|e| format!("cannot save capture journal: {e}"))
}

/// Record all declared sources with simultaneous microphone input streams.
///
/// Resolve calibration paths against `plan_directory`. `output_directory` must
/// not already exist; existing recordings are never overwritten. Run on a worker
/// thread. Cancellation retains complete sources with an explicit cancelled state.
///
/// # Errors
/// Returns configuration, calibration, device, cancellation, or filesystem errors.
/// An error after acquisition starts preserves its journal and completed sources.
pub fn record_capture_session(
    session: &ValidatedCaptureSession,
    plan_directory: &Path,
    output_directory: &Path,
    cancel: &CancelFlag,
    progress: impl FnMut(CaptureProgress),
) -> Result<RawCaptureManifest, String> {
    record_with(
        session,
        plan_directory,
        output_directory,
        cancel,
        progress,
        capture_multidevice,
    )
}

fn record_with(
    session: &ValidatedCaptureSession,
    plan_directory: &Path,
    output_directory: &Path,
    cancel: &CancelFlag,
    mut progress: impl FnMut(CaptureProgress),
    mut capture: impl FnMut(MultiCaptureRequest, &CancelFlag) -> Result<MultiCaptureResult, String>,
) -> Result<RawCaptureManifest, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled".into());
    }
    let stimulus = prepare_capture_stimulus(session)?;
    // Parse copies of the exact bytes we hash/save, not a second read of mutable
    // user calibration files. Every calibration must load before audio starts.
    let snapshots =
        tempfile::tempdir().map_err(|e| format!("cannot stage calibration files: {e}"))?;
    let mut calibrations = Vec::new();
    for (index, mic) in session.plan().microphones.iter().enumerate() {
        let path = plan_directory.join(&mic.calibration_file);
        // Calibration is a small text curve. Bound external file reads to 1 MiB.
        use std::io::Read;
        let file = std::fs::File::open(&path)
            .map_err(|e| format!("cannot read calibration for {}: {e}", mic.id))?;
        let mut bytes = Vec::new();
        file.take(1_048_577)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("cannot read calibration for {}: {e}", mic.id))?;
        if bytes.len() > 1_048_576 {
            return Err(format!("calibration for {} exceeds 1 MiB", mic.id));
        }
        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("txt");
        if !matches!(extension, "txt" | "csv" | "TXT" | "CSV") {
            return Err(format!(
                "calibration for {} must be a .txt or .csv curve",
                mic.id
            ));
        }
        let file = format!("calibration-{index:02}.{}", extension.to_ascii_lowercase());
        let snapshot = snapshots.path().join(&file);
        std::fs::write(&snapshot, &bytes).map_err(|e| e.to_string())?;
        sotf_audio::signal_analysis::MicrophoneCompensation::from_file(&snapshot)
            .map_err(|e| format!("invalid calibration for {}: {e}", mic.id))?;
        calibrations.push(CaptureCalibration {
            microphone_id: mic.id.clone(),
            file,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
        });
    }
    std::fs::create_dir(output_directory)
        .map_err(|e| format!("cannot create new capture directory: {e}"))?;
    let mut manifest = RawCaptureManifest {
        version: 1,
        plan: session.plan().clone(),
        stimulus: stimulus.layout,
        timing_reference_output_channel: Some(
            session
                .plan()
                .timing_reference
                .as_ref()
                .map_or(session.plan().sources[0].output_channel, |reference| {
                    reference.output_channel
                }),
        ),
        status: RawCaptureStatus::Capturing,
        pending_processing: vec![
            "clock_correction".into(),
            "calibrated_analysis".into(),
            "take_quality".into(),
        ],
        calibrations,
        takes: Vec::new(),
        error: None,
    };
    save_manifest(output_directory, &manifest)?;
    let result = (|| -> Result<(), String> {
        for calibration in &manifest.calibrations {
            std::fs::copy(
                snapshots.path().join(&calibration.file),
                output_directory.join(&calibration.file),
            )
            .map_err(|e| format!("cannot save calibration snapshot: {e}"))?;
        }
        write_wav_file(
            &output_directory.join("stimulus.wav"),
            &stimulus.samples,
            session.plan().sample_rate_hz,
            1,
        )?;
        write_wav_file(
            &output_directory.join("timing-chirp.wav"),
            &stimulus.timing_chirp,
            session.plan().sample_rate_hz,
            1,
        )?;
        for (source_index, source) in session.plan().sources.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled".into());
            }
            progress(CaptureProgress {
                source_index,
                source_count: session.plan().sources.len(),
                source_id: source.id.clone(),
            });
            let captured = capture(
                MultiCaptureRequest {
                    inputs: session
                        .plan()
                        .microphones
                        .iter()
                        .map(|mic| CaptureInput {
                            device: mic.device.clone(),
                            channel: mic.input_channel,
                        })
                        .collect(),
                    output_device: session.plan().output_device.clone(),
                    output_channel: source.output_channel,
                    output_overrides: [
                        manifest.stimulus.start_chirp_offset,
                        manifest.stimulus.end_chirp_offset,
                    ]
                    .into_iter()
                    .map(|start_frame| CaptureOutputSegment {
                        start_frame,
                        end_frame: start_frame + manifest.stimulus.chirp_samples,
                        channel: manifest
                            .timing_reference_output_channel
                            .unwrap_or(source.output_channel),
                    })
                    .collect(),
                    sample_rate_hz: session.plan().sample_rate_hz,
                    stimulus: stimulus.samples.clone(),
                },
                cancel,
            )?;
            if captured.sample_rate_hz != session.plan().sample_rate_hz
                || captured.recordings.len() != session.plan().microphones.len()
                || captured.input_sample_formats.len() != captured.recordings.len()
                || captured.input_device_ids.len() != captured.recordings.len()
                || captured.input_device_ids.iter().any(String::is_empty)
                || captured.output_device_id.is_empty()
                || captured.recordings.iter().any(|samples| {
                    samples.len() < stimulus.samples.len() || samples.iter().any(|s| !s.is_finite())
                })
            {
                return Err(
                    "capture backend returned mismatched rate, channels, or invalid samples".into(),
                );
            }
            for (mic_index, (mic, samples)) in session
                .plan()
                .microphones
                .iter()
                .zip(&captured.recordings)
                .enumerate()
            {
                // Numeric names avoid collisions and path traversal in user labels.
                let wav_file = format!("source-{source_index:02}-mic-{mic_index:02}-raw.wav");
                write_wav_file(
                    &output_directory.join(&wav_file),
                    samples,
                    captured.sample_rate_hz,
                    1,
                )?;
                manifest.takes.push(RawCaptureTake {
                    device_id: captured.input_device_ids[mic_index].clone(),
                    output_device_id: captured.output_device_id.clone(),
                    source_id: source.id.clone(),
                    microphone_id: mic.id.clone(),
                    wav_file,
                    samples: samples.len(),
                    input_sample_format: captured.input_sample_formats[mic_index].clone(),
                    output_sample_format: captured.output_sample_format.clone(),
                    peak_amplitude: samples.iter().fold(0.0_f32, |peak, s| peak.max(s.abs())),
                    clipped_samples: samples.iter().filter(|s| s.abs() >= 1.0).count(),
                });
            }
            save_manifest(output_directory, &manifest)?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            manifest.status = RawCaptureStatus::RawComplete;
            save_manifest(output_directory, &manifest)?;
            Ok(manifest)
        }
        Err(error) => {
            manifest.status = if error == "cancelled" {
                RawCaptureStatus::Cancelled
            } else {
                RawCaptureStatus::Failed
            };
            manifest.error = Some(error.clone());
            save_manifest(output_directory, &manifest)
                .map_err(|save_error| format!("{error}; additionally {save_error}"))?;
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests;
