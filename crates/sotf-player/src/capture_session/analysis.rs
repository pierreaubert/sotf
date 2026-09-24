//! Calibrated magnitude analysis and conservative capture-quality evidence.

use super::CaptureSessionPlan;
pub use super::phase::CapturePhaseArtifacts;
use super::protocol::{CaptureStimulus, CaptureStimulusLayout};
use serde::{Deserialize, Serialize};
use sotf_audio::signal_analysis::{
    MeasurementQualityConfig, MicrophoneCompensation, analyze_recording,
    estimate_lag_with_confidence,
};
use std::io::Write;
use std::path::Path;

/// Measurement evidence independent of clock-fit eligibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureAnalysisReport {
    /// Calibrated magnitude CSV, relative to the processed directory.
    pub magnitude_file: Option<String>,
    /// Original input samples at or above full scale.
    pub clipped_samples: usize,
    /// Broadband sweep-to-quiet power ratio; not frequency-dependent SNR.
    pub broadband_snr_db: Option<f64>,
    /// Sweep-window time-averaged SNR in octave-width bands.
    #[serde(default)]
    pub frequency_snr: Vec<CaptureFrequencySnr>,
    /// Shared-origin calibrated phase and band-limited IR, when eligible.
    #[serde(default)]
    pub common_reference: Option<CapturePhaseArtifacts>,
    /// Reasons this take requires review or could not be analyzed.
    pub issues: Vec<String>,
}

/// Frequency-dependent signal-to-noise evidence for one sweep band.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureFrequencySnr {
    /// Lower band edge in Hz, inclusive.
    pub low_hz: f64,
    /// Upper band edge in Hz, exclusive.
    pub high_hz: f64,
    /// Noise-subtracted sweep power relative to quiet power; missing is unknown.
    pub snr_db: Option<f64>,
}

fn frequency_snr(
    samples: &[f32],
    layout: &CaptureStimulusLayout,
    plan: &CaptureSessionPlan,
) -> Result<Vec<CaptureFrequencySnr>, String> {
    if !plan.sweep.start_hz.is_finite()
        || !plan.sweep.end_hz.is_finite()
        || plan.sweep.start_hz <= 0.0
        || plan.sweep.end_hz <= plan.sweep.start_hz
    {
        return Err("SNR sweep bounds are invalid".into());
    }
    let quiet = samples
        .get(layout.start_chirp_offset / 4..layout.start_chirp_offset / 2)
        .ok_or("quiet window is missing")?;
    let sweep = samples
        .get(layout.sweep_offset..layout.sweep_offset + layout.sweep_samples)
        .ok_or("sweep window is missing")?;
    let mut bands = Vec::new();
    let mut low = plan.sweep.start_hz;
    while low < plan.sweep.end_hz {
        let high = (low * 2.0).min(plan.sweep.end_hz);
        bands.push((low, high));
        low = high;
    }
    math_audio_dsp::analysis::capture_band_snr(sweep, quiet, plan.sample_rate_hz, &bands).map(
        |bands| {
            bands
                .into_iter()
                .map(|band| CaptureFrequencySnr {
                    low_hz: band.low_hz,
                    high_hz: band.high_hz,
                    snr_db: band.snr_db,
                })
                .collect()
        },
    )
}

fn power(samples: &[f32]) -> Option<f64> {
    if samples.is_empty() || samples.iter().any(|x| !x.is_finite()) {
        return None;
    }
    Some(samples.iter().map(|&x| f64::from(x).powi(2)).sum::<f64>() / samples.len() as f64)
}

fn broadband_snr(samples: &[f32], layout: &CaptureStimulusLayout) -> Option<f64> {
    // Use the middle of pre-silence, avoiding resampler edges and chirp onset.
    let noise = power(samples.get(layout.start_chirp_offset / 4..layout.start_chirp_offset / 2)?)?;
    let signal = power(
        samples.get(layout.sweep_offset..layout.sweep_offset.checked_add(layout.sweep_samples)?)?,
    )?;
    // Digital silence is not evidence of an infinitely quiet acoustic input.
    if noise <= 0.0 || signal <= noise {
        return None;
    }
    let snr = 10.0 * ((signal - noise) / noise).log10();
    snr.is_finite().then_some(snr)
}

pub(super) fn analyze_capture(
    original: &[f32],
    corrected: Option<&[f32]>,
    stimulus: &CaptureStimulus,
    plan: &CaptureSessionPlan,
    calibration: &Path,
    output: &Path,
    index: usize,
) -> CaptureAnalysisReport {
    let mut report = CaptureAnalysisReport {
        magnitude_file: None,
        clipped_samples: original.iter().filter(|x| x.abs() >= 1.0).count(),
        broadband_snr_db: None,
        frequency_snr: Vec::new(),
        common_reference: None,
        issues: Vec::new(),
    };
    if report.clipped_samples > 0 {
        report
            .issues
            .push("input reached full scale; reduce input or playback gain and repeat".into());
    }
    let samples = corrected.unwrap_or(original);
    if corrected.is_none() {
        report.issues.push("timing markers unavailable: magnitude uses the uncorrected device clock; drift may smear the response and coherent use is forbidden".into());
    } else {
        report.broadband_snr_db = broadband_snr(samples, &stimulus.layout);
        match frequency_snr(samples, &stimulus.layout, plan) {
            Ok(bands) => {
                for band in &bands {
                    match band.snr_db {
                        Some(snr) if snr >= 30.0 => {}
                        Some(_) => report.issues.push(format!(
                            "SNR in {:.1}–{:.1} Hz is below 30 dB; reduce room noise and repeat",
                            band.low_hz, band.high_hz)),
                        None => report.issues.push(format!(
                            "SNR in {:.1}–{:.1} Hz is unavailable; no quality acceptance for this band",
                            band.low_hz, band.high_hz)),
                    }
                }
                report.frequency_snr = bands;
            }
            Err(error) => report
                .issues
                .push(format!("frequency-dependent SNR unavailable: {error}")),
        }
    }
    match report.broadband_snr_db {
        Some(snr) if snr >= 30.0 => {}
        Some(_) => report.issues.push("broadband SNR below 30 dB; reduce room noise and repeat".into()),
        None => report.issues.push("broadband SNR unavailable: missing quiet interval, digital silence, or signal below noise".into()),
    }
    let name = format!("take-{index:03}-magnitude.csv");
    match write_magnitude(
        samples,
        stimulus,
        plan,
        calibration,
        &output.join(&name),
        corrected.is_none(),
    ) {
        Ok(()) => report.magnitude_file = Some(name),
        Err(error) => report
            .issues
            .push(format!("calibrated magnitude analysis failed: {error}")),
    }
    report
}

fn write_magnitude(
    samples: &[f32],
    stimulus: &CaptureStimulus,
    plan: &CaptureSessionPlan,
    calibration: &Path,
    destination: &Path,
    uncorrected: bool,
) -> Result<(), String> {
    let layout = &stimulus.layout;
    // Exclude timing chirps. Keep 250 ms before the sweep for arrival-aligned
    // takes whose measured source is closer than the reference emitter.
    let start = layout
        .sweep_offset
        .saturating_sub(plan.sample_rate_hz as usize / 4);
    let reference = stimulus
        .samples
        .get(layout.sweep_offset..layout.sweep_offset + layout.sweep_samples)
        .ok_or("stimulus sweep range is invalid")?;
    let (start, end) = if uncorrected {
        let lag = estimate_lag_with_confidence(reference, samples)?;
        if !lag.confidence.is_finite()
            || lag.confidence < MeasurementQualityConfig::default().minimum_lag_confidence
        {
            return Err("uncorrected measurement sweep has no reliable signal lock".into());
        }
        let onset = usize::try_from(lag.lag_samples)
            .map_err(|_| "uncorrected sweep starts before the captured audio")?;
        // Leave room for the maximum supported clock dilation and a 250 ms
        // guard before the end chirp. Never use the chirp as source response.
        let dilation = (layout.sweep_samples as f64
            * math_audio_dsp::capture_tdoa::MAX_PLAUSIBLE_SKEW_PPM
            / 1e6)
            .ceil() as usize;
        let guard = dilation.saturating_add(plan.sample_rate_hz as usize / 4);
        let tail = (layout.end_chirp_offset - layout.sweep_offset - layout.sweep_samples)
            .checked_sub(guard)
            .ok_or("sweep too long to isolate safely without clock correction")?;
        let end = onset
            .checked_add(layout.sweep_samples)
            .and_then(|end| end.checked_add(tail))
            .ok_or("uncorrected sweep range overflow")?;
        (onset.saturating_sub(plan.sample_rate_hz as usize / 4), end)
    } else {
        (start, layout.end_chirp_offset)
    };
    let capture = samples
        .get(start..end)
        .ok_or("capture does not contain the sweep and required decay")?;
    if power(capture).unwrap_or(0.0) <= 0.0 {
        return Err("sweep capture is silent".into());
    }
    let lag = estimate_lag_with_confidence(reference, capture)?;
    if !lag.confidence.is_finite()
        || lag.confidence < MeasurementQualityConfig::default().minimum_lag_confidence
    {
        return Err("measurement sweep has no reliable signal lock".into());
    }
    let temporary = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    sotf_audio::signal_recorder::write_wav_file(temporary.path(), capture, plan.sample_rate_hz, 1)?;
    let analysis = analyze_recording(
        temporary.path(),
        reference,
        plan.sample_rate_hz,
        Some((plan.sweep.start_hz as f32, plan.sweep.end_hz as f32)),
    )?;
    let compensation = MicrophoneCompensation::from_file(calibration)?;
    let calibration_start = compensation
        .frequencies
        .first()
        .copied()
        .ok_or("microphone calibration is empty")?;
    let calibration_end = compensation
        .frequencies
        .last()
        .copied()
        .ok_or("microphone calibration is empty")?;
    let mut csv = String::from("frequency_hz,spl_db\n");
    let mut rows = 0;
    for (&frequency, &magnitude) in analysis.frequencies.iter().zip(&analysis.spl_db) {
        if f64::from(frequency) < plan.sweep.start_hz || f64::from(frequency) > plan.sweep.end_hz {
            continue;
        }
        let calibrated = magnitude - compensation.interpolate_at(frequency);
        if frequency < calibration_start || frequency > calibration_end {
            continue;
        }
        if !frequency.is_finite() || !calibrated.is_finite() {
            return Err("analysis produced non-finite magnitude data".into());
        }
        use std::fmt::Write;
        writeln!(csv, "{frequency},{calibrated}").map_err(|e| e.to_string())?;
        rows += 1;
    }
    if rows < 2 {
        return Err("analysis has insufficient in-band frequency points".into());
    }
    let mut file = std::fs::File::create(destination).map_err(|e| e.to_string())?;
    file.write_all(csv.as_bytes()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frequency_quality_covers_sweep_and_preserves_unknown_noise() {
        let plan: CaptureSessionPlan =
            serde_json::from_str(include_str!("../../tests/fixtures/capture-session.json"))
                .unwrap();
        let stimulus =
            super::super::protocol::prepare_capture_stimulus(&plan.clone().validate().unwrap())
                .unwrap();
        let bands = frequency_snr(&stimulus.samples, &stimulus.layout, &plan).unwrap();
        assert_eq!(bands.first().unwrap().low_hz, plan.sweep.start_hz);
        assert_eq!(bands.last().unwrap().high_hz, plan.sweep.end_hz);
        assert!(
            bands
                .windows(2)
                .all(|pair| pair[0].high_hz == pair[1].low_hz)
        );
        assert!(bands.iter().all(|band| band.snr_db.is_none()));
    }

    #[test]
    fn legacy_analysis_has_no_frequency_quality_claim() {
        let report: CaptureAnalysisReport = serde_json::from_str(
            r#"{"magnitude_file":null,"clipped_samples":0,"broadband_snr_db":40.0,"issues":[]}"#,
        )
        .unwrap();
        assert!(report.frequency_snr.is_empty());
    }

    #[test]
    fn snr_subtracts_noise_power_and_rejects_digital_silence() {
        let layout = CaptureStimulusLayout {
            sample_rate_hz: 100,
            start_chirp_offset: 40,
            end_chirp_offset: 90,
            chirp_samples: 5,
            sweep_offset: 50,
            sweep_samples: 30,
            total_samples: 100,
        };
        let mut samples = vec![0.01; 100];
        samples[50..80].fill(0.1);
        assert!((broadband_snr(&samples, &layout).unwrap() - 99.0_f64.log10() * 10.0).abs() < 1e-5);
        samples[10..20].fill(0.0);
        assert_eq!(broadband_snr(&samples, &layout), None);
        assert_eq!(broadband_snr(&samples[..60], &layout), None);
    }

    #[test]
    fn magnitude_uses_each_microphones_inverse_calibration() {
        let mut plan: CaptureSessionPlan =
            serde_json::from_str(include_str!("../../tests/fixtures/capture-session.json"))
                .unwrap();
        plan.sweep.duration_secs = 0.25;
        let stimulus =
            super::super::protocol::prepare_capture_stimulus(&plan.clone().validate().unwrap())
                .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let flat = directory.path().join("flat.txt");
        let raised = directory.path().join("raised.txt");
        std::fs::write(&flat, "10 0\n24000 0\n").unwrap();
        std::fs::write(&raised, "10 6\n24000 6\n").unwrap();
        let first = directory.path().join("first.csv");
        let second = directory.path().join("second.csv");
        write_magnitude(&stimulus.samples, &stimulus, &plan, &flat, &first, false).unwrap();
        write_magnitude(&stimulus.samples, &stimulus, &plan, &raised, &second, false).unwrap();
        let first = std::fs::read_to_string(first).unwrap();
        let second = std::fs::read_to_string(second).unwrap();
        assert!(first.lines().count() > 10);
        for (a, b) in first.lines().skip(1).zip(second.lines().skip(1)) {
            let (fa, ma) = a.split_once(',').unwrap();
            let (fb, mb) = b.split_once(',').unwrap();
            assert_eq!(fa, fb);
            assert!((ma.parse::<f64>().unwrap() - mb.parse::<f64>().unwrap() - 6.0).abs() < 1e-4);
        }
        let report = analyze_capture(
            &[1.0, -1.1, 0.1],
            None,
            &stimulus,
            &plan,
            &flat,
            directory.path(),
            2,
        );
        assert_eq!(report.clipped_samples, 2);
        assert!(report.magnitude_file.is_none());
        assert!(report.broadband_snr_db.is_none());
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.contains("coherent use is forbidden"))
        );
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.contains("analysis failed"))
        );
    }

    #[test]
    fn missing_chirps_still_export_magnitude_without_timing_evidence() {
        let mut plan: CaptureSessionPlan =
            serde_json::from_str(include_str!("../../tests/fixtures/capture-session.json"))
                .unwrap();
        plan.sweep.duration_secs = 0.25;
        let stimulus =
            super::super::protocol::prepare_capture_stimulus(&plan.clone().validate().unwrap())
                .unwrap();
        let mut raw = vec![0.0; 2107];
        raw.extend_from_slice(&stimulus.samples);
        for offset in [
            stimulus.layout.start_chirp_offset,
            stimulus.layout.end_chirp_offset,
        ] {
            raw[2107 + offset..2107 + offset + stimulus.layout.chirp_samples].fill(0.0);
        }
        let dir = tempfile::tempdir().unwrap();
        let calibration = dir.path().join("mic.txt");
        std::fs::write(&calibration, "10 0\n24000 0\n").unwrap();
        let report = analyze_capture(&raw, None, &stimulus, &plan, &calibration, dir.path(), 0);
        assert!(report.magnitude_file.is_some(), "{:?}", report.issues);
        assert!(report.broadband_snr_db.is_none());
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.contains("uncorrected device clock"))
        );
        let csv = std::fs::read_to_string(dir.path().join(report.magnitude_file.unwrap())).unwrap();
        assert_eq!(csv.lines().next(), Some("frequency_hz,spl_db"));
        assert!(csv.lines().count() > 10);
        let reference_path = dir.path().join("reference.csv");
        write_magnitude(
            &stimulus.samples,
            &stimulus,
            &plan,
            &calibration,
            &reference_path,
            false,
        )
        .unwrap();
        let reference = std::fs::read_to_string(reference_path).unwrap();
        for (expected, actual) in reference.lines().skip(1).zip(csv.lines().skip(1)) {
            let (frequency, expected) = expected.split_once(',').unwrap();
            let (actual_frequency, actual) = actual.split_once(',').unwrap();
            assert_eq!(frequency, actual_frequency);
            assert!(
                (expected.parse::<f64>().unwrap() - actual.parse::<f64>().unwrap()).abs() < 0.1,
                "fallback magnitude differs at {frequency} Hz"
            );
        }
    }
}
