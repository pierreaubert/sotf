//! Shared configuration and preflight validation for multi-microphone capture.
//!
//! Validation freezes the declared geometry, gain and calibration assignments.
//! Device negotiation and take QA are separate steps: a valid plan is not evidence
//! of a synchronized recording or permission to render coherent measurements.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

#[cfg(not(target_os = "ios"))]
pub mod analysis;
#[cfg(not(target_os = "ios"))]
pub mod clock;
#[cfg(not(target_os = "ios"))]
pub mod manifest;
#[cfg(not(target_os = "ios"))]
mod phase;
pub mod protocol;
#[cfg(not(target_os = "ios"))]
pub mod record;
#[cfg(not(target_os = "ios"))]
pub mod reflections;

/// Spatial interpretation fixed for every take in a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureGeometry {
    /// Independent seats for spatial magnitude coverage.
    Spread,
    /// A measured microphone cluster for conditional direction estimation.
    Compact,
}

/// Calibration orientation relative to the capsule axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalibrationOrientation {
    /// Capsule aimed at the source.
    OnAxis,
    /// Capsule perpendicular to the source direction.
    NinetyDegrees,
}

/// One physical microphone and its fixed session settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureMicrophone {
    /// Unique identity used in take provenance, independent of device ordering.
    pub id: String,
    /// Exact audio input device ID or unique name; never silently use the default.
    pub device: String,
    /// Zero-based channel within the selected input device.
    pub input_channel: u16,
    /// Individual microphone calibration file, resolved relative to the plan.
    pub calibration_file: PathBuf,
    /// Orientation for which the assigned calibration was measured.
    pub calibration_orientation: CalibrationOrientation,
    /// Documented input gain, held fixed throughout the session.
    pub gain_db: f64,
    /// Microphone coordinates in meters in one session coordinate system.
    pub position_m: [f64; 3],
    /// Survey uncertainty in millimeters, required for compact-array use.
    pub position_uncertainty_mm: f64,
}

/// A loudspeaker excited separately while every microphone records.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureSource {
    /// Unique source label used to associate all simultaneous takes.
    pub id: String,
    /// Zero-based hardware output channel.
    pub output_channel: u16,
}

/// A fixed acoustic timing emitter surveyed in the microphone coordinate frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureTimingReference {
    /// Hardware channel used for both timing chirps on every source take.
    pub output_channel: u16,
    /// Timing emitter acoustic center, in meters.
    pub position_m: [f64; 3],
    /// Upper bound on emitter-position error, in millimeters.
    pub position_uncertainty_mm: f64,
    /// Measured or estimated speed of sound in meters per second.
    pub sound_speed_m_s: f64,
    /// Upper bound on sound-speed error in meters per second.
    pub sound_speed_uncertainty_m_s: f64,
}

/// Explicit logarithmic sweep settings shared by all sources.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureSweep {
    /// Sweep duration, excluding timing chirps and silence, in seconds.
    pub duration_secs: f64,
    /// Low sweep frequency in Hz.
    pub start_hz: f64,
    /// High sweep frequency in Hz, strictly below Nyquist.
    pub end_hz: f64,
    /// Linear playback peak amplitude, in (0, 1].
    pub amplitude: f64,
}

/// Serializable multi-microphone session declaration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureSessionPlan {
    /// Schema version; currently 1.
    pub version: u32,
    /// One geometry mode applies to the entire session.
    pub geometry: CaptureGeometry,
    /// Requested common nominal rate; all devices must support it exactly.
    pub sample_rate_hz: u32,
    /// Explicit output device selector.
    pub output_device: String,
    /// Two to four simultaneously recorded microphones.
    pub microphones: Vec<CaptureMicrophone>,
    /// Sources played sequentially in this order.
    pub sources: Vec<CaptureSource>,
    /// Sweep parameters fixed throughout the session.
    pub sweep: CaptureSweep,
    /// Fixed timing emitter and geometry; absent geometry disables coherent use.
    /// Without this declaration the first source channel emits timing chirps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing_reference: Option<CaptureTimingReference>,
}

/// Validated settings that cannot be changed during capture.
#[derive(Debug, Clone)]
pub struct ValidatedCaptureSession(CaptureSessionPlan);

/// A session declaration rejected before opening audio devices.
#[derive(Debug, thiserror::Error)]
#[error("invalid capture session: {0}")]
pub struct CapturePlanError(String);

impl CaptureSessionPlan {
    /// Validate and freeze all declared session settings.
    ///
    /// Compact sessions require positions known within one millimeter. This is
    /// only a survey precondition: aperture, frequency, clock uncertainty and
    /// TDOA consistency still determine whether individual directions are usable.
    ///
    /// # Errors
    /// Rejects unsupported versions, invalid rates/sweeps, duplicate identities
    /// or routes, non-finite settings, missing calibration, and invalid geometry.
    pub fn validate(self) -> Result<ValidatedCaptureSession, CapturePlanError> {
        let reject = |message: &str| CapturePlanError(message.to_owned());
        if self.version != 1 {
            return Err(reject("unsupported plan version (expected 1)"));
        }
        // The timing chirp occupies 2–8 kHz; Nyquist must exceed its upper edge.
        if !(16_001..=384_000).contains(&self.sample_rate_hz) {
            return Err(reject(
                "sample rate must be in 16001..=384000 Hz for capture",
            ));
        }
        if self.output_device.trim().is_empty() {
            return Err(reject("an explicit output device is required"));
        }
        if !(2..=4).contains(&self.microphones.len()) {
            return Err(reject("a session requires two to four microphones"));
        }
        let mut ids = HashSet::new();
        let mut routes = HashSet::new();
        for mic in &self.microphones {
            if mic.id.trim().is_empty() || !ids.insert(&mic.id) {
                return Err(reject("microphone identities must be nonempty and unique"));
            }
            if mic.device.trim().is_empty()
                || mic.input_channel >= 64
                || !routes.insert((&mic.device, mic.input_channel))
            {
                return Err(reject(
                    "microphone device/channel routes must be explicit, unique, and below channel 64",
                ));
            }
            if mic.calibration_file.as_os_str().is_empty() {
                return Err(reject("each microphone requires its own calibration file"));
            }
            if !mic.gain_db.is_finite()
                || mic.position_m.iter().any(|value| !value.is_finite())
                || !mic.position_uncertainty_mm.is_finite()
                || mic.position_uncertainty_mm < 0.0
            {
                return Err(reject(
                    "microphone gain and geometry must be finite and uncertainty nonnegative",
                ));
            }
            if self.geometry == CaptureGeometry::Compact && mic.position_uncertainty_mm > 1.0 {
                return Err(reject(
                    "compact microphone positions require at most 1 mm uncertainty",
                ));
            }
        }
        if self.geometry == CaptureGeometry::Compact {
            for (index, mic) in self.microphones.iter().enumerate() {
                if self.microphones[..index]
                    .iter()
                    .any(|other| other.position_m == mic.position_m)
                {
                    return Err(reject("compact microphones must occupy distinct positions"));
                }
            }
        }
        ids.clear();
        let mut outputs = HashSet::new();
        if self.sources.is_empty() {
            return Err(reject("at least one source is required"));
        }
        for source in &self.sources {
            if source.id.trim().is_empty()
                || source.output_channel >= 64
                || !ids.insert(&source.id)
                || !outputs.insert(source.output_channel)
            {
                return Err(reject(
                    "source identities must be nonempty and unique, with unique output channels below 64",
                ));
            }
        }
        let sweep = &self.sweep;
        if let Some(reference) = &self.timing_reference {
            if reference.output_channel >= 64
                || reference.position_m.iter().any(|v| !v.is_finite())
                || !reference.position_uncertainty_mm.is_finite()
                || reference.position_uncertainty_mm < 0.0
                || !reference.sound_speed_m_s.is_finite()
                || !(250.0..=400.0).contains(&reference.sound_speed_m_s)
                || !reference.sound_speed_uncertainty_m_s.is_finite()
                || reference.sound_speed_uncertainty_m_s < 0.0
                || reference.sound_speed_uncertainty_m_s >= reference.sound_speed_m_s
            {
                return Err(reject(
                    "invalid timing-reference route, geometry, or sound-speed uncertainty",
                ));
            }
        }
        if !sweep.duration_secs.is_finite()
            || sweep.duration_secs <= 0.0
            || !sweep.start_hz.is_finite()
            || sweep.start_hz <= 0.0
            || !sweep.end_hz.is_finite()
            || sweep.end_hz <= sweep.start_hz
            || sweep.end_hz >= f64::from(self.sample_rate_hz) / 2.0
            || !sweep.amplitude.is_finite()
            || sweep.amplitude <= 0.0
            || sweep.amplitude > 1.0
        {
            return Err(reject(
                "sweep requires finite positive duration, ascending frequencies below Nyquist, and amplitude in (0, 1]",
            ));
        }
        // Budget includes chirps, decay gaps and engine startup/drain storage.
        // Match the engine's bounded per-microphone capture memory budget.
        if (sweep.duration_secs + 14.0) * f64::from(self.sample_rate_hz) > 16_000_000.0 {
            return Err(reject(
                "sweep duration exceeds bounded capture storage at this sample rate",
            ));
        }
        if sweep.amplitude as f32 == 0.0
            || sweep.duration_secs * f64::from(self.sample_rate_hz) < 2.0
        {
            return Err(reject(
                "sweep must have at least two samples and a representable nonzero amplitude",
            ));
        }
        Ok(ValidatedCaptureSession(self))
    }
}

impl ValidatedCaptureSession {
    /// Inspect the frozen session configuration.
    pub fn plan(&self) -> &CaptureSessionPlan {
        &self.0
    }
}

#[cfg(test)]
mod tests;
