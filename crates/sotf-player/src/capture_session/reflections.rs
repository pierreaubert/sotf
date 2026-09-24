//! SSIR arrival candidates and conditional compact-array directions.

// Rust guideline compliant 2026-02-21
use super::{CaptureGeometry, clock::io::ClockProcessedManifest};
use math_audio_dsp::{
    capture_array::{MicArray, estimate_doa_ls, pairwise_tdoas_vs_first},
    capture_tdoa::{TdoaConfig, TdoaWeighting},
};
use serde::Deserialize;
use std::{
    io::Read,
    path::{Component, Path},
};

pub use autoeq::capture_provenance::{CaptureArrival, CaptureReflectionReport};

#[derive(Deserialize)]
struct ImpulseArtifact {
    sample_rate_hz: u32,
    time_origin: String,
    samples: Vec<f64>,
}

fn event_energy_levels(
    channels: &[Vec<f32>],
    direct: std::ops::Range<usize>,
    event: std::ops::Range<usize>,
) -> Vec<f64> {
    channels
        .iter()
        .map(|channel| {
            let energy = |range| {
                channel.get(range).map(|samples: &[f32]| {
                    samples
                        .iter()
                        .map(|&value| f64::from(value).powi(2))
                        .sum::<f64>()
                })
            };
            let level = 10.0 * (energy(event.clone())? / energy(direct.clone())?).log10();
            level.is_finite().then_some(level)
        })
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default()
}

fn load_impulse(root: &Path, name: &str, rate: u32) -> Result<Vec<f32>, String> {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err("IR artifact must be a local filename".into());
    }
    // Capture IRs are generated locally, but processing must still bound reads.
    let file = std::fs::File::open(root.join(name)).map_err(|error| error.to_string())?;
    let artifact: ImpulseArtifact =
        serde_json::from_reader(file.take(128 * 1024 * 1024)).map_err(|error| error.to_string())?;
    if artifact.sample_rate_hz != rate
        || artifact.time_origin != "shared_stimulus_sweep_start"
        || artifact.samples.is_empty()
        || artifact
            .samples
            .iter()
            .any(|value| !value.is_finite() || value.abs() > f64::from(f32::MAX))
    {
        return Err("IR artifact has invalid samples, sample rate or time origin".into());
    }
    Ok(artifact
        .samples
        .into_iter()
        .map(|value| value as f32)
        .collect())
}

pub(super) fn analyze_sources(
    report: &ClockProcessedManifest,
    root: &Path,
) -> Vec<CaptureReflectionReport> {
    report
        .plan
        .sources
        .iter()
        .map(|source| {
            let mut result = CaptureReflectionReport {
                source_id: source.id.clone(),
                direct_sound: None,
                early_reflections: Vec::new(),
                issues: Vec::new(),
            };
            if let Err(error) = analyze_source(report, root, &source.id, &mut result) {
                result.issues.push(error);
            }
            result
        })
        .collect()
}

fn analyze_source(
    report: &ClockProcessedManifest,
    root: &Path,
    source: &str,
    result: &mut CaptureReflectionReport,
) -> Result<(), String> {
    if report.plan.geometry != CaptureGeometry::Compact {
        return Err("spread geometry does not support array directions".into());
    }
    let reference = report
        .plan
        .timing_reference
        .as_ref()
        .ok_or("surveyed timing reference missing")?;
    let mut channels = Vec::new();
    let mut bounds = Vec::new();
    let mut low_hz: f64 = 300.0;
    let mut high_hz: f64 = 3000.0;
    for mic in &report.plan.microphones {
        let take = report
            .takes
            .iter()
            .find(|take| take.raw.source_id == source && take.raw.microphone_id == mic.id)
            .ok_or("missing array take")?;
        let analysis = take.analysis.as_ref().ok_or("missing take analysis")?;
        if !analysis.issues.is_empty() {
            return Err(format!("{}: take quality requires review", mic.id));
        }
        let phase = analysis
            .common_reference
            .as_ref()
            .ok_or("accepted shared-origin IR unavailable")?;
        low_hz = low_hz.max(phase.low_hz);
        high_hz = high_hz.min(phase.high_hz);
        bounds.push(
            take.clock
                .residual_uncertainty_us
                .filter(|bound| bound.is_finite() && *bound > 0.0)
                .ok_or("unknown clock uncertainty")?,
        );
        channels.push(load_impulse(
            root,
            &phase.impulse_file,
            report.plan.sample_rate_hz,
        )?);
    }
    if channels.len() < 2
        || channels
            .iter()
            .any(|channel| channel.len() != channels[0].len())
    {
        return Err("array IR lengths differ or microphones are missing".into());
    }
    bounds.sort_by(|a, b| b.total_cmp(a));
    let relative_us = bounds[0] + bounds[1];
    high_hz = high_hz.min(25_000.0 / relative_us);
    // C5 uses 343 m/s internally. Scale geometry to preserve surveyed travel times.
    let scale = math_audio_dsp::capture_array::SPEED_OF_SOUND_M_S / reference.sound_speed_m_s;
    let array = MicArray::new(
        report
            .plan
            .microphones
            .iter()
            .map(|mic| mic.position_m.map(|coordinate| coordinate * scale))
            .collect(),
        f64::from(report.plan.sample_rate_hz),
    )?;
    let config = math_rir::SsirConfig::new(f64::from(report.plan.sample_rate_hz));
    let events = math_rir::analyze_rir(&channels[0], &config);
    let direct = events
        .direct_sound()
        .ok_or("no direct-arrival candidate detected")?;
    if direct.peak_energy <= 0.0 {
        return Err("direct candidate has no energy".into());
    }
    let rate = f64::from(report.plan.sample_rate_hz);
    for segment in events.segments.iter().take(65) {
        let relative_ms =
            segment.toa_sample.saturating_sub(direct.toa_sample) as f64 / rate * 1000.0;
        if relative_ms > 80.0 {
            break;
        }
        if segment.peak_energy <= 0.0 {
            continue;
        }
        let mut event = CaptureArrival {
            arrival_ms: segment.toa_sample as f64 / rate * 1000.0,
            relative_ms,
            level_db: 10.0 * (segment.peak_energy / direct.peak_energy).log10(),
            microphone_energy_db: event_energy_levels(
                &channels,
                direct.onset_sample..direct.end_sample,
                segment.onset_sample..segment.end_sample,
            ),
            direction: None,
            mirror_ambiguous: false,
            residual_samples: None,
            band_hz: None,
            issues: Vec::new(),
        };
        match event_direction(
            &channels,
            &array,
            segment.onset_sample,
            segment.end_sample,
            rate,
            [low_hz, high_hz],
        ) {
            Ok(estimate) => {
                event.direction = Some(estimate.direction);
                event.mirror_ambiguous = estimate.elevation_ambiguous;
                event.residual_samples = Some(estimate.rms_residual_samples);
                event.band_hz = Some([low_hz, high_hz]);
                if estimate.elevation_ambiguous {
                    event
                        .issues
                        .push("planar array: mirrored arrival direction remains unresolved".into());
                }
                event.issues.push("conditional plane-wave estimate; source distance and microphone phase are not independently verified".into());
            }
            Err(error) => event.issues.push(error),
        }
        if segment.is_direct_sound {
            result.direct_sound = Some(event);
        } else {
            result.early_reflections.push(event);
        }
    }
    Ok(())
}

fn event_direction(
    channels: &[Vec<f32>],
    array: &MicArray,
    start: usize,
    end: usize,
    rate: f64,
    band_hz: [f64; 2],
) -> Result<math_audio_dsp::capture_array::DoaEstimate, String> {
    if array.len() < 3 {
        return Err("at least three noncollinear microphones are required for direction".into());
    }
    if band_hz[1] <= band_hz[0] {
        return Err("clock uncertainty leaves no supported DOA band".into());
    }
    let windows: Vec<Vec<f32>> = channels
        .iter()
        .map(|channel| {
            channel
                .get(start..end)
                .map(<[f32]>::to_vec)
                .ok_or("invalid event window")
        })
        .collect::<Result<_, _>>()?;
    if end.saturating_sub(start) < (rate / band_hz[0]).ceil() as usize * 2 {
        return Err("event window is too short to resolve the DOA band".into());
    }
    let config = TdoaConfig {
        sample_rate_hz: rate,
        band_lo_hz: band_hz[0],
        band_hi_hz: band_hz[1],
        min_confidence_db: 6.0,
        weighting: TdoaWeighting::Matched,
    };
    let pairs = pairwise_tdoas_vs_first(&windows, &config)
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or("one or more event pair delays are unreliable")?;
    let max_delay = array.aperture() / math_audio_dsp::capture_array::SPEED_OF_SOUND_M_S * rate;
    if pairs
        .iter()
        .any(|pair| pair.delay_samples.abs() > max_delay + 0.5)
    {
        return Err("event delay exceeds the array aperture".into());
    }
    let estimate = estimate_doa_ls(array, &pairs)?;
    if estimate.rms_residual_samples > 0.5 {
        return Err("event delays do not fit a common plane wave within half a sample".into());
    }
    Ok(estimate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrival_energy_normalization_is_independent_of_microphone_gain() {
        let channels = vec![vec![1.0, 0.0, 0.5, 0.0], vec![2.0, 0.0, 1.0, 0.0]];
        let levels = event_energy_levels(&channels, 0..2, 2..4);
        assert_eq!(levels.len(), 2);
        for level in levels {
            assert!((level - 10.0 * 0.25_f64.log10()).abs() < 1e-12);
        }
        assert!(event_energy_levels(&[vec![0.0; 4]], 0..2, 2..4).is_empty());
        assert!(event_energy_levels(&channels, 0..2, 4..8).is_empty());
    }

    #[test]
    fn impulse_artifacts_require_local_paths_and_shared_origin() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("ir.json");
        for (rate, origin) in [
            (44100, "shared_stimulus_sweep_start"),
            (48000, "peak_aligned"),
        ] {
            std::fs::write(
                &file,
                serde_json::to_vec(&serde_json::json!({
                    "sample_rate_hz": rate, "time_origin": origin, "samples": [1.0, 0.0],
                }))
                .unwrap(),
            )
            .unwrap();
            assert!(load_impulse(root.path(), "ir.json", 48000).is_err());
        }
        assert!(load_impulse(root.path(), "../ir.json", 48000).is_err());
    }

    #[test]
    fn isolated_arrival_recovers_both_tdoa_signs() {
        let spacing = 14.0 / 48_000.0 * 343.0;
        let array = MicArray::new(
            vec![
                [0.0, 0.0, 0.0],
                [spacing, 0.0, 0.0],
                [0.0, spacing, 0.0],
                [0.0, 0.0, spacing],
            ],
            48_000.0,
        )
        .unwrap();
        for (arrival, expected_x) in [(242, 1.0), (270, -1.0)] {
            let mut channels = vec![vec![0.0; 1024]; 4];
            for channel in &mut channels {
                channel[256] = 1.0;
            }
            channels[1][256] = 0.0;
            channels[1][arrival] = 1.0;
            let direction =
                event_direction(&channels, &array, 0, 1024, 48_000.0, [300.0, 3000.0]).unwrap();
            assert!((direction.direction[0] - expected_x).abs() < 0.01);
            assert!(!direction.elevation_ambiguous);
            assert!(direction.rms_residual_samples < 0.1);
        }
    }

    #[test]
    fn inadequate_geometry_band_and_window_do_not_produce_directions() {
        let two = MicArray::new(vec![[0.0; 3], [0.1, 0.0, 0.0]], 48_000.0).unwrap();
        let channels = vec![vec![0.0; 1024]; 2];
        assert!(event_direction(&channels, &two, 0, 1024, 48_000.0, [300.0, 3000.0]).is_err());
        let three =
            MicArray::new(vec![[0.0; 3], [0.1, 0.0, 0.0], [0.0, 0.1, 0.0]], 48_000.0).unwrap();
        let channels = vec![vec![0.0; 1024]; 3];
        assert!(event_direction(&channels, &three, 0, 1024, 48_000.0, [300.0, 200.0]).is_err());
        assert!(event_direction(&channels, &three, 0, 10, 48_000.0, [300.0, 3000.0]).is_err());
        assert!(event_direction(&channels, &three, 0, 1024, 48_000.0, [300.0, 3000.0]).is_err());
    }
}
