//! Start/end acoustic timing chirps around a source's logarithmic sweep.

use super::ValidatedCaptureSession;
use serde::{Deserialize, Serialize};
use sotf_audio::signals::{apply_fade_in, apply_fade_out, try_gen_log_sweep};

/// Exact stimulus-clock positions used for subsequent drift estimation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureStimulusLayout {
    /// Nominal stimulus clock in Hz.
    pub sample_rate_hz: u32,
    /// First sample of the start timing chirp.
    pub start_chirp_offset: usize,
    /// First sample of the end timing chirp.
    pub end_chirp_offset: usize,
    /// Length of either timing chirp in samples.
    pub chirp_samples: usize,
    /// First sample of the measurement sweep.
    pub sweep_offset: usize,
    /// Measurement sweep length, excluding silence and timing chirps.
    pub sweep_samples: usize,
    /// Complete stimulus length including quiet intervals.
    pub total_samples: usize,
}

/// Prepared stimulus and reference chirp for one source capture.
#[derive(Debug)]
pub struct CaptureStimulus {
    /// Mono playback waveform, bounded by the session amplitude.
    pub samples: Vec<f32>,
    /// Identical start/end reference chirp for acoustic delay estimation.
    pub timing_chirp: Vec<f32>,
    /// Positions on the stimulus clock, never assumed to be input offsets.
    pub layout: CaptureStimulusLayout,
}

/// Build the fixed acoustic timing protocol for a validated session.
///
/// Protocol v1 uses 100 ms 2–8 kHz chirps with 5 ms tapers, 500 ms
/// pre-silence/chirp-to-sweep silence/post-silence, and a two-second sweep
/// decay interval before the final chirp. The estimator must reject obscured
/// chirps in rooms whose decay exceeds that interval.
///
/// # Errors
/// Returns the waveform generator's error if sweep values cannot be represented.
pub fn prepare_capture_stimulus(
    session: &ValidatedCaptureSession,
) -> Result<CaptureStimulus, String> {
    let plan = session.plan();
    let rate = plan.sample_rate_hz;
    let amplitude = plan.sweep.amplitude as f32;
    let mut chirp = try_gen_log_sweep(2000.0, 8000.0, amplitude, rate, 0.1)?;
    let mut sweep = try_gen_log_sweep(
        plan.sweep.start_hz as f32,
        plan.sweep.end_hz as f32,
        amplitude,
        rate,
        plan.sweep.duration_secs as f32,
    )?;
    if chirp.is_empty() || sweep.is_empty() {
        return Err("capture waveform generator returned an empty signal".into());
    }
    // Smooth on/off edges to avoid broadband clicks contaminating the timing fit.
    let fade_samples = (f64::from(rate) * 0.005).round() as usize;
    for signal in [&mut chirp, &mut sweep] {
        apply_fade_in(signal, fade_samples);
        apply_fade_out(signal, fade_samples);
    }
    let quiet = rate as usize / 2;
    let start_chirp_offset = quiet;
    let sweep_offset = start_chirp_offset + chirp.len() + quiet;
    let end_chirp_offset = sweep_offset + sweep.len() + rate as usize * 2;
    let total_samples = end_chirp_offset + chirp.len() + quiet;
    let mut samples = vec![0.0; total_samples];
    samples[start_chirp_offset..start_chirp_offset + chirp.len()].copy_from_slice(&chirp);
    samples[sweep_offset..sweep_offset + sweep.len()].copy_from_slice(&sweep);
    samples[end_chirp_offset..end_chirp_offset + chirp.len()].copy_from_slice(&chirp);
    Ok(CaptureStimulus {
        samples,
        layout: CaptureStimulusLayout {
            sample_rate_hz: rate,
            start_chirp_offset,
            end_chirp_offset,
            chirp_samples: chirp.len(),
            sweep_offset,
            sweep_samples: sweep.len(),
            total_samples,
        },
        timing_chirp: chirp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture_session::CaptureSessionPlan;

    #[test]
    fn chirps_and_sweep_have_explicit_separate_positions_at_multiple_rates() {
        for rate in [44_100, 48_000, 96_000] {
            let mut plan: CaptureSessionPlan =
                serde_json::from_str(include_str!("../../tests/fixtures/capture-session.json"))
                    .unwrap();
            plan.sample_rate_hz = rate;
            let signal = prepare_capture_stimulus(&plan.validate().unwrap()).unwrap();
            let layout = signal.layout;
            assert_eq!(layout.start_chirp_offset, rate as usize / 2);
            assert_eq!(
                layout.end_chirp_offset - layout.sweep_offset - layout.sweep_samples,
                rate as usize * 2
            );
            assert_eq!(
                &signal.samples
                    [layout.start_chirp_offset..layout.start_chirp_offset + layout.chirp_samples],
                signal.timing_chirp.as_slice()
            );
            assert_eq!(
                &signal.samples
                    [layout.end_chirp_offset..layout.end_chirp_offset + layout.chirp_samples],
                signal.timing_chirp.as_slice()
            );
            assert!(
                signal.samples[..layout.start_chirp_offset]
                    .iter()
                    .all(|s| *s == 0.0)
            );
            assert!(
                signal
                    .samples
                    .iter()
                    .all(|s| s.is_finite() && s.abs() <= 0.1)
            );
            assert_eq!(signal.samples.len(), layout.total_samples);
        }
    }
}
