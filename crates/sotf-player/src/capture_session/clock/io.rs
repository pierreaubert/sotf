//! File-backed clock processing of saved raw capture journals.

use super::{CaptureClockProvenance, correct_capture_clock};
use crate::capture_session::record::{
    CaptureCalibration, RawCaptureManifest, RawCaptureStatus, RawCaptureTake,
};
use crate::capture_session::{CaptureSessionPlan, protocol::CaptureStimulusLayout};
use crate::recording_helpers::save_recording_session_json;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Component, Path};

/// One processed audio artifact and the clock evidence governing its use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockProcessedTake {
    /// Original take metadata; original samples remain in the raw directory.
    pub raw: RawCaptureTake,
    /// Audio artifact relative to the processed output directory.
    pub audio_file: String,
    /// Per-take clock provenance; absent bounds force magnitude-only use.
    pub clock: CaptureClockProvenance,
    /// Calibrated magnitude and raw-input quality evidence, when processed.
    #[serde(default)]
    pub analysis: Option<crate::capture_session::analysis::CaptureAnalysisReport>,
}

/// Clock-stage output with magnitude analysis and incomplete measurement QA.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockProcessedManifest {
    /// Clock-processing schema version.
    pub version: u32,
    /// Original geometry, gains, routes, and timing-reference survey.
    pub plan: CaptureSessionPlan,
    /// Stimulus-clock sweep and chirp positions.
    pub stimulus: CaptureStimulusLayout,
    /// Calibration snapshots copied and verified against acquisition hashes.
    pub calibrations: Vec<CaptureCalibration>,
    /// Corrected or magnitude-only takes, each with explicit provenance.
    pub takes: Vec<ClockProcessedTake>,
    /// Remaining stages; clock correction alone is not complete measurement QA.
    pub pending_processing: Vec<String>,
    /// Canonical RoomEQ manifest, present only when every magnitude take is available.
    #[serde(default)]
    pub recording_manifest: Option<String>,
    /// Per-source arrival candidates and conditional compact-array directions.
    #[serde(default)]
    pub reflection_reports: Vec<crate::capture_session::reflections::CaptureReflectionReport>,
}

fn artifact(root: &Path, name: &str) -> Result<std::path::PathBuf, String> {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err("capture artifact paths must be single relative filenames".into());
    }
    let path = root
        .join(name)
        .canonicalize()
        .map_err(|e| format!("cannot resolve capture artifact {name}: {e}"))?;
    if !path.starts_with(root) || !path.is_file() {
        return Err("capture artifact is outside the session directory or is not a file".into());
    }
    Ok(path)
}

fn bounded_read(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("cannot open capture artifact: {e}"))?;
    let mut data = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() > limit {
        return Err("capture metadata exceeds its size limit".into());
    }
    Ok(data)
}

fn read_wave(path: &Path, rate: u32, frames: usize) -> Result<Vec<f32>, String> {
    if frames == 0 || frames > 16_000_000 {
        return Err("invalid capture wave length".into());
    }
    let mut reader =
        hound::WavReader::open(path).map_err(|e| format!("cannot read capture WAV: {e}"))?;
    let spec = reader.spec();
    if spec.channels != 1
        || spec.sample_rate != rate
        || spec.bits_per_sample != 32
        || spec.sample_format != hound::SampleFormat::Float
        || reader.duration() as usize != frames
    {
        return Err(
            "capture WAV rate, channel count, format, or length disagrees with the journal".into(),
        );
    }
    let samples = reader
        .samples::<f32>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    if samples.iter().any(|s| !s.is_finite()) {
        return Err("capture WAV contains non-finite samples".into());
    }
    Ok(samples)
}

/// Process saved captures without opening audio devices or altering raw takes.
///
/// `output_directory` must not exist. Invalid timing markers preserve raw samples
/// as magnitude-only artifacts. Missing/corrupt files fail before claiming a
/// completed processing report. Completed takes in cancelled sessions are usable.
///
/// # Errors
/// Rejects invalid journals, paths, calibration hashes, WAV metadata, and I/O errors.
pub fn process_capture_session(
    raw_directory: &Path,
    output_directory: &Path,
) -> Result<ClockProcessedManifest, String> {
    let root = raw_directory
        .canonicalize()
        .map_err(|e| format!("cannot open raw capture directory: {e}"))?;
    let manifest: RawCaptureManifest = serde_json::from_slice(&bounded_read(
        &artifact(&root, "capture-raw.json")?,
        2_097_152,
    )?)
    .map_err(|e| format!("cannot parse raw capture journal: {e}"))?;
    if manifest.version != 1
        || manifest.status == RawCaptureStatus::Capturing
        || manifest.takes.is_empty()
    {
        return Err(
            "raw capture must have a supported, terminal journal with completed takes".into(),
        );
    }
    let session = manifest
        .plan
        .clone()
        .validate()
        .map_err(|e| e.to_string())?;
    let expected = crate::capture_session::protocol::prepare_capture_stimulus(&session)?;
    if manifest.stimulus != expected.layout {
        return Err("stimulus layout disagrees with the capture plan".into());
    }
    let chirp = read_wave(
        &artifact(&root, "timing-chirp.wav")?,
        manifest.plan.sample_rate_hz,
        manifest.stimulus.chirp_samples,
    )?;
    let mut routes = HashSet::new();
    for take in &manifest.takes {
        if !manifest
            .plan
            .microphones
            .iter()
            .any(|mic| mic.id == take.microphone_id)
            || !manifest
                .plan
                .sources
                .iter()
                .any(|source| source.id == take.source_id)
            || !routes.insert((&take.source_id, &take.microphone_id))
            || take.device_id.is_empty()
            || take.output_device_id.is_empty()
        {
            return Err("raw capture contains unknown, duplicate, or unidentified takes".into());
        }
        artifact(&root, &take.wav_file)?;
    }
    if manifest.status == RawCaptureStatus::RawComplete
        && manifest.takes.len() != manifest.plan.microphones.len() * manifest.plan.sources.len()
    {
        return Err("completed raw capture is missing source/microphone takes".into());
    }
    let mut calibration_ids = HashSet::new();
    let mut calibration_bytes = Vec::new();
    for calibration in &manifest.calibrations {
        if !manifest
            .plan
            .microphones
            .iter()
            .any(|mic| mic.id == calibration.microphone_id)
            || !calibration_ids.insert(&calibration.microphone_id)
        {
            return Err("capture calibration identities are unknown or duplicated".into());
        }
        let bytes = bounded_read(&artifact(&root, &calibration.file)?, 1_048_576)?;
        if format!("{:x}", Sha256::digest(&bytes)) != calibration.sha256 {
            return Err(format!(
                "calibration snapshot for {} has changed",
                calibration.microphone_id
            ));
        }
        calibration_bytes.push((&calibration.file, bytes));
    }
    if calibration_ids.len() != manifest.plan.microphones.len() {
        return Err("capture is missing microphone calibration identities".into());
    }
    std::fs::create_dir(output_directory)
        .map_err(|e| format!("cannot create new processed capture directory: {e}"))?;
    for (file, bytes) in calibration_bytes {
        std::fs::write(output_directory.join(file), bytes).map_err(|e| e.to_string())?;
    }
    for name in ["timing-chirp.wav", "stimulus.wav"] {
        std::fs::copy(artifact(&root, name)?, output_directory.join(name))
            .map_err(|e| e.to_string())?;
    }
    let mut result = ClockProcessedManifest {
        version: 1,
        plan: manifest.plan.clone(),
        stimulus: manifest.stimulus.clone(),
        calibrations: manifest.calibrations.clone(),
        takes: Vec::new(),
        recording_manifest: None,
        reflection_reports: Vec::new(),
        pending_processing: vec![
            "calibrated_analysis".into(),
            "take_quality".into(),
            "reflection_directions".into(),
        ],
    };
    for (index, take) in manifest.takes.iter().enumerate() {
        let samples = read_wave(
            &artifact(&root, &take.wav_file)?,
            manifest.plan.sample_rate_hz,
            take.samples,
        )?;
        let corrected = correct_capture_clock(&manifest, take, &samples, &chirp);
        let calibration = manifest
            .calibrations
            .iter()
            .find(|calibration| calibration.microphone_id == take.microphone_id)
            .ok_or("missing microphone calibration")?;
        let mut analysis = crate::capture_session::analysis::analyze_capture(
            &samples,
            corrected.samples.as_deref(),
            &expected,
            &manifest.plan,
            &output_directory.join(&calibration.file),
            output_directory,
            index,
        );
        if corrected.provenance.basis == super::CaptureClockBasis::FixedAcousticReference
            && corrected.provenance.magnitude_only_reason.is_none()
            && analysis.issues.is_empty()
            && !analysis.frequency_snr.is_empty()
            && let (Some(samples), Some(residual_us)) = (
                corrected.samples.as_deref(),
                corrected.provenance.residual_uncertainty_us,
            )
        {
            match crate::capture_session::phase::write_phase(
                samples,
                &expected,
                &manifest.plan,
                &output_directory.join(&calibration.file),
                output_directory,
                index,
                residual_us,
            ) {
                Ok(artifacts) => analysis.common_reference = Some(artifacts),
                Err(error) => analysis
                    .issues
                    .push(format!("common-reference phase unavailable: {error}")),
            }
        }
        let audio_file = format!("take-{index:03}.wav");
        sotf_audio::signal_recorder::write_wav_file(
            &output_directory.join(&audio_file),
            corrected.samples.as_deref().unwrap_or(&samples),
            manifest.plan.sample_rate_hz,
            1,
        )?;
        result.takes.push(ClockProcessedTake {
            raw: take.clone(),
            audio_file,
            clock: corrected.provenance,
            analysis: Some(analysis),
        });
    }
    result.reflection_reports =
        crate::capture_session::reflections::analyze_sources(&result, output_directory);
    result
        .pending_processing
        .retain(|stage| stage != "reflection_directions");
    if result.takes.iter().all(|take| {
        take.analysis.as_ref().is_some_and(|analysis| {
            analysis.common_reference.is_some() && analysis.issues.is_empty()
        })
    }) {
        result
            .pending_processing
            .retain(|stage| stage != "take_quality");
    }
    if result.takes.iter().all(|take| {
        take.analysis
            .as_ref()
            .is_some_and(|analysis| analysis.magnitude_file.is_some())
    }) {
        result
            .pending_processing
            .retain(|stage| stage != "calibrated_analysis");
    }
    match crate::capture_session::manifest::recording_configuration(&result) {
        Ok(configuration) => {
            let name = crate::recording_helpers::RECORDINGS_FILENAME;
            let json = serde_json::to_vec_pretty(&configuration).map_err(|e| e.to_string())?;
            save_recording_session_json(&output_directory.join(name), &json)
                .map_err(|e| e.to_string())?;
            result.recording_manifest = Some(name.into());
        }
        Err(reason) => result
            .pending_processing
            .push(format!("recording_manifest: {reason}")),
    }
    let json = serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?;
    save_recording_session_json(&output_directory.join("capture-clock.json"), &json)
        .map_err(|e| e.to_string())?;
    Ok(result)
}
