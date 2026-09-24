//! Canonical RoomEQ recording export with ordered per-microphone clock evidence.

use super::clock::CaptureClockBasis;
use super::clock::io::ClockProcessedManifest;
use super::{CalibrationOrientation, CaptureGeometry};
use autoeq::capture_provenance::{CaptureCorrection, CaptureProvenance, CaptureTakeProvenance};
use autoeq::read::{MeasurementMultiple, MeasurementRef};
use autoeq::roomeq::{MeasurementSource, RecordingConfiguration, RoomConfig, SpeakerConfig};
use autoeq::{MeasurementProvenance, ProvenanceCaptureKind};
use std::collections::HashMap;
use std::path::{Component, Path};

/// Build the canonical configuration from a complete set of analyzed capture takes.
///
/// Sources with complete accepted shared-reference artifacts export measured
/// phase. Other sources export magnitude. Clock bounds remain mandatory for
/// frequency-specific coherent eligibility. Calibration is already applied.
///
/// # Errors
/// Rejects incomplete or duplicated takes, missing calibration, and nonlocal artifacts.
pub fn recording_configuration(report: &ClockProcessedManifest) -> Result<RoomConfig, String> {
    report
        .plan
        .clone()
        .validate()
        .map_err(|error| error.to_string())?;
    if report.takes.len() != report.plan.sources.len() * report.plan.microphones.len() {
        return Err("not every source/microphone take is available".into());
    }
    let mut speakers = HashMap::new();
    for source in &report.plan.sources {
        let phase_complete = report
            .takes
            .iter()
            .filter(|take| take.raw.source_id == source.id)
            .all(|take| {
                take.analysis.as_ref().is_some_and(|analysis| {
                    analysis.common_reference.is_some()
                        && analysis.issues.is_empty()
                        && analysis.clipped_samples == 0
                        && analysis
                            .broadband_snr_db
                            .is_some_and(|value| value.is_finite() && value >= 30.0)
                        && !analysis.frequency_snr.is_empty()
                        && analysis.frequency_snr.iter().all(|band| {
                            band.snr_db
                                .is_some_and(|value| value.is_finite() && value >= 30.0)
                        })
                })
            });
        let mut measurements = Vec::new();
        let mut takes = Vec::new();
        for mic in &report.plan.microphones {
            let mut candidates = report
                .takes
                .iter()
                .filter(|take| take.raw.source_id == source.id && take.raw.microphone_id == mic.id);
            let take = candidates
                .next()
                .ok_or_else(|| format!("{} / {}: take is missing", source.id, mic.id))?;
            if candidates.next().is_some() {
                return Err(format!("{} / {}: duplicate take", source.id, mic.id));
            }
            let file = take
                .analysis
                .as_ref()
                .and_then(|analysis| {
                    if phase_complete {
                        analysis
                            .common_reference
                            .as_ref()
                            .map(|artifacts| &artifacts.response_file)
                    } else {
                        analysis.magnitude_file.as_ref()
                    }
                })
                .ok_or_else(|| {
                    format!(
                        "{} / {}: calibrated magnitude is unavailable",
                        source.id, mic.id
                    )
                })?;
            let mut components = Path::new(file).components();
            if !matches!(components.next(), Some(Component::Normal(_)))
                || components.next().is_some()
            {
                return Err("measurement artifact must be a local filename".into());
            }
            let calibration = report
                .calibrations
                .iter()
                .find(|entry| entry.microphone_id == mic.id)
                .ok_or_else(|| format!("{}: calibration identity is unavailable", mic.id))?;
            measurements.push(MeasurementRef::Named {
                path: file.into(),
                name: Some(mic.id.clone()),
            });
            takes.push(CaptureTakeProvenance {
                microphone_id: mic.id.clone(),
                device_id: take.raw.device_id.clone(),
                offset_samples: take.clock.offset_samples,
                skew_ppm: take.clock.skew_ppm,
                residual_uncertainty_us: take.clock.residual_uncertainty_us,
                correction_applied: match take.clock.correction_applied.as_str() {
                    "resampled" => CaptureCorrection::Resampled,
                    "none" => CaptureCorrection::None,
                    _ => return Err("unknown capture correction mode".into()),
                },
                timing_reference_id: take.clock.reference_id.clone(),
                calibration_id: calibration.sha256.clone(),
                gain_db: mic.gain_db,
                calibration_orientation: match mic.calibration_orientation {
                    CalibrationOrientation::OnAxis => "on_axis",
                    CalibrationOrientation::NinetyDegrees => "ninety_degrees",
                }
                .into(),
                position_m: mic.position_m,
                position_uncertainty_mm: mic.position_uncertainty_mm,
                preserves_acoustic_delay: take.clock.basis
                    == CaptureClockBasis::FixedAcousticReference,
                quality_passed: phase_complete,
            });
        }
        let timing_reference_id = takes
            .first()
            .and_then(|take| take.timing_reference_id.clone());
        let provenance = MeasurementProvenance {
            capture_kind: if phase_complete {
                ProvenanceCaptureKind::StationaryIr
            } else {
                ProvenanceCaptureKind::SpatialMagnitude
            },
            timing_reference_id,
            capture: Some(CaptureProvenance {
                reflection_report: report
                    .reflection_reports
                    .iter()
                    .find(|reflection| reflection.source_id == source.id)
                    .cloned(),
                geometry: match report.plan.geometry {
                    CaptureGeometry::Spread => autoeq::capture_provenance::CaptureGeometry::Spread,
                    CaptureGeometry::Compact => {
                        autoeq::capture_provenance::CaptureGeometry::Compact
                    }
                },
                takes,
            }),
            ..Default::default()
        };
        speakers.insert(
            source.id.clone(),
            SpeakerConfig::Single(MeasurementSource::Multiple(MeasurementMultiple {
                measurements,
                speaker_name: None,
                provenance,
            })),
        );
    }
    Ok(RoomConfig {
        speakers,
        recording_config: Some(RecordingConfiguration {
            recording_sample_rate: Some(report.plan.sample_rate_hz),
            recording_channels: Some(report.plan.microphones.len()),
            signal_type: Some("Sweep".into()),
            signal_duration_secs: Some(report.plan.sweep.duration_secs as f32),
            signal_level_db: Some((20.0 * report.plan.sweep.amplitude.log10()) as f32),
            sweep_start_freq: Some(report.plan.sweep.start_hz as f32),
            sweep_end_freq: Some(report.plan.sweep.end_hz as f32),
            num_sweeps: Some(1),
            setup_description: Some("Calibrated multi-microphone capture. Phase requires accepted shared-reference artifacts and frequency-specific clock eligibility. See capture-clock.json for raw artifacts, timing bandwidth and review issues.".into()),
            ..Default::default()
        }),
        ..Default::default()
    })
}
