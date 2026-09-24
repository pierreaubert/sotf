//! Calibrated shared-origin artifacts, without arrival alignment.

// Rust guideline compliant 2026-02-21
use super::{CaptureSessionPlan, protocol::CaptureStimulus};
use math_audio_dsp::analysis::{MicrophoneCompensation, deconvolve_common_clock};
use serde::{Deserialize, Serialize};
use std::{fmt::Write, path::Path};

/// Calibrated phase and IR artifacts with an explicit timing bandwidth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturePhaseArtifacts {
    /// Relative CSV path containing frequency, magnitude and measured phase.
    pub response_file: String,
    /// Relative JSON path containing the band-limited, periodic real IR.
    pub impulse_file: String,
    /// Upper frequency for at most nine degrees of conditional clock error.
    pub timing_limit_hz: f64,
    /// Lower calibrated sweep frequency.
    pub low_hz: f64,
    /// Upper calibrated sweep frequency; not itself a coherent-use guarantee.
    pub high_hz: f64,
}

pub(super) fn write_phase(
    samples: &[f32],
    stimulus: &CaptureStimulus,
    plan: &CaptureSessionPlan,
    calibration: &Path,
    output: &Path,
    index: usize,
    residual_us: f64,
) -> Result<CapturePhaseArtifacts, String> {
    if !residual_us.is_finite() || residual_us <= 0.0 {
        return Err("phase requires a finite positive timing bound".into());
    }
    let layout = &stimulus.layout;
    let end = layout.end_chirp_offset;
    // Identical origins retain acoustic travel time. Only the sweep and its
    // decay enter the FFT; neither timing chirp belongs to this source response.
    let capture = samples
        .get(layout.sweep_offset..end)
        .ok_or("missing sweep decay")?;
    let reference = stimulus
        .samples
        .get(layout.sweep_offset..end)
        .ok_or("invalid stimulus window")?;
    let capture: Vec<_> = capture.iter().map(|&x| f64::from(x)).collect();
    let reference: Vec<_> = reference.iter().map(|&x| f64::from(x)).collect();
    let response = deconvolve_common_clock(&capture, &reference, plan.sample_rate_hz)?;
    let compensation = MicrophoneCompensation::from_file(calibration)?;
    let low_hz = plan.sweep.start_hz.max(f64::from(
        *compensation
            .frequencies
            .first()
            .ok_or("empty calibration")?,
    ));
    let high_hz = plan.sweep.end_hz.min(f64::from(
        *compensation.frequencies.last().ok_or("empty calibration")?,
    ));
    if low_hz >= high_hz {
        return Err("calibration does not cover the sweep".into());
    }
    let bin_hz = f64::from(plan.sample_rate_hz) / response.impulse_response.len() as f64;
    let gains: Vec<_> = (0..response.spectrum.len())
        .map(|i| {
            let frequency = i as f64 * bin_hz;
            if frequency < low_hz || frequency > high_hz {
                0.0
            } else {
                10.0_f64.powf(-f64::from(compensation.interpolate_at(frequency as f32)) / 20.0)
            }
        })
        .collect();
    let response = response.with_magnitude_gains(&gains)?;
    let mut csv = String::from("frequency_hz,spl_db,phase_deg\n");
    let mut rows = 0;
    for (i, bin) in response.spectrum.iter().enumerate() {
        let frequency = i as f64 * bin_hz;
        if frequency < low_hz || frequency > high_hz {
            continue;
        }
        let magnitude = 20.0 * bin.norm().log10();
        if !magnitude.is_finite() {
            return Err("phase response contains an unexcited bin".into());
        }
        writeln!(csv, "{frequency},{magnitude},{}", bin.arg().to_degrees())
            .map_err(|e| e.to_string())?;
        rows += 1;
    }
    if rows < 2 {
        return Err("phase response has too few calibrated bins".into());
    }
    let artifacts = CapturePhaseArtifacts {
        response_file: format!("take-{index:03}-phase.csv"),
        impulse_file: format!("take-{index:03}-ir.json"),
        timing_limit_hz: (25_000.0 / residual_us).min(high_hz),
        low_hz,
        high_hz,
    };
    let impulse = serde_json::json!({
        "sample_rate_hz": plan.sample_rate_hz,
        "time_origin": "shared_stimulus_sweep_start",
        "low_hz": low_hz,
        "high_hz": high_hz,
        "timing_limit_hz": artifacts.timing_limit_hz,
        "microphone_phase_calibrated": false,
        "periodic_band_limited": true,
        "samples": response.impulse_response,
    });
    std::fs::write(output.join(&artifacts.response_file), csv).map_err(|e| e.to_string())?;
    std::fs::write(
        output.join(&artifacts.impulse_file),
        serde_json::to_vec(&impulse).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(artifacts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exported_ir_retains_delay_and_calibration_retains_phase() {
        let mut plan: CaptureSessionPlan =
            serde_json::from_str(include_str!("../../tests/fixtures/capture-session.json"))
                .unwrap();
        plan.sweep.duration_secs = 0.25;
        let stimulus =
            super::super::protocol::prepare_capture_stimulus(&plan.clone().validate().unwrap())
                .unwrap();
        let output = tempfile::tempdir().unwrap();
        let flat = output.path().join("flat.txt");
        let raised = output.path().join("raised.txt");
        std::fs::write(&flat, "10 0\n24000 0\n").unwrap();
        std::fs::write(&raised, "10 6\n24000 6\n").unwrap();
        for delay in [37, 61] {
            let mut recorded = vec![0.0; stimulus.samples.len()];
            recorded[delay..].copy_from_slice(&stimulus.samples[..stimulus.samples.len() - delay]);
            let original =
                write_phase(&recorded, &stimulus, &plan, &flat, output.path(), 0, 20.0).unwrap();
            let calibrated =
                write_phase(&recorded, &stimulus, &plan, &raised, output.path(), 1, 20.0).unwrap();
            assert_eq!(original.timing_limit_hz, 1250.0);
            let ir: serde_json::Value = serde_json::from_slice(
                &std::fs::read(output.path().join(&original.impulse_file)).unwrap(),
            )
            .unwrap();
            let values = ir["samples"].as_array().unwrap();
            let peak = values
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| {
                    a.as_f64()
                        .unwrap()
                        .abs()
                        .total_cmp(&b.as_f64().unwrap().abs())
                })
                .unwrap()
                .0;
            assert_eq!(peak, delay);
            let first =
                std::fs::read_to_string(output.path().join(original.response_file)).unwrap();
            let second =
                std::fs::read_to_string(output.path().join(calibrated.response_file)).unwrap();
            for (a, b) in first.lines().skip(1).zip(second.lines().skip(1)) {
                let a: Vec<f64> = a.split(',').map(|value| value.parse().unwrap()).collect();
                let b: Vec<f64> = b.split(',').map(|value| value.parse().unwrap()).collect();
                assert!((a[1] - b[1] - 6.0).abs() < 1e-8);
                assert!((a[2] - b[2]).abs() < 1e-8);
                let expected = -360.0 * a[0] * delay as f64 / f64::from(plan.sample_rate_hz);
                let error = (a[2] - expected + 180.0).rem_euclid(360.0) - 180.0;
                assert!(error.abs() < 1e-6, "phase error {error} at {} Hz", a[0]);
            }
        }
    }
}
