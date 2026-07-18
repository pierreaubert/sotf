//! Control-thread preparation and persistence for reproducible A/B sessions.
//!
//! Rendering and filesystem work in this module must stay off the audio thread.
//! The realtime path receives only the prebuilt `ABComparePluginParams` produced
//! by `AbTestSession::runtime_config_for_pending_cue`.

use std::path::Path;

use serde::{Deserialize, Serialize};
use sotf_plugins::plugin_ab_compare::{PathConfig, build_path_from_config_with_factory};

use super::ab_test_session::{
    AbTestError, AbTestSession, ChainSnapshot, LevelMatchMeasurement, LevelMatchMetric,
    ListeningTestSetup, MediaSegment, measure_level_match,
};

#[derive(Debug, Clone)]
pub struct DecodedLevelMatchSegment {
    pub interleaved_samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: usize,
    pub start_ms: u64,
    pub duration_ms: u64,
}

pub struct AbTestSessionPreparationRequest<'a> {
    pub session_id: &'a str,
    pub assignment_seed: u64,
    pub path_a_label: &'a str,
    pub path_b_label: &'a str,
    pub path_a: &'a PathConfig,
    pub path_b: &'a PathConfig,
    pub media_path: &'a Path,
    pub start_ms: u64,
    pub duration_ms: u64,
    pub metric: LevelMatchMetric,
    pub max_correction_db: f64,
    pub block_frames: usize,
    pub switch_transition_ms: f32,
    pub participant_id: Option<String>,
    pub app_version: &'a str,
}

/// Decode one synchronized media segment, render both chains, level match them,
/// and return a validated reproducible session ready for blind trials.
pub fn prepare_ab_test_session(
    request: AbTestSessionPreparationRequest<'_>,
) -> Result<(AbTestSession, LevelMatchPreparation), AbTestError> {
    let segment =
        decode_level_match_segment(request.media_path, request.start_ms, request.duration_ms)?;
    let preparation = prepare_level_match(LevelMatchPreparationRequest {
        path_a: request.path_a,
        path_b: request.path_b,
        sample_rate: segment.sample_rate,
        channels: segment.channels,
        interleaved_segment: &segment.interleaved_samples,
        metric: request.metric,
        max_correction_db: request.max_correction_db,
        block_frames: request.block_frames,
    })?;
    let setup = ListeningTestSetup {
        path_a: ChainSnapshot::new(request.path_a_label, request.path_a.clone())?,
        path_b: ChainSnapshot::new(request.path_b_label, request.path_b.clone())?,
        media: MediaSegment {
            media_id: request.media_path.to_string_lossy().into_owned(),
            start_ms: segment.start_ms,
            duration_ms: segment.duration_ms,
        },
        sample_rate: segment.sample_rate,
        channels: segment.channels,
        level_match: preparation.measurement.clone(),
        switch_transition_ms: request.switch_transition_ms,
        participant_id: request.participant_id,
        app_version: request.app_version.to_owned(),
    };
    let session = AbTestSession::new(request.session_id, setup, request.assignment_seed)?;
    Ok((session, preparation))
}

/// Decode an exact interleaved segment for offline, synchronized path rendering.
pub fn decode_level_match_segment(
    path: impl AsRef<Path>,
    start_ms: u64,
    duration_ms: u64,
) -> Result<DecodedLevelMatchSegment, AbTestError> {
    use sotf_audio::decoder::{DecodedAudio, create_decoder};

    if duration_ms == 0 {
        return Err(AbTestError::InvalidLevelMatchInput);
    }
    let mut decoder = create_decoder(path.as_ref())
        .map_err(|error| AbTestError::PathPreparation(error.to_string()))?;
    let spec = decoder.spec().clone();
    let channels = usize::from(spec.channels);
    if spec.sample_rate == 0 || channels == 0 {
        return Err(AbTestError::InvalidLevelMatchInput);
    }
    let start_frame = ((u128::from(start_ms) * u128::from(spec.sample_rate)) / 1_000)
        .try_into()
        .map_err(|_| AbTestError::InvalidLevelMatchInput)?;
    let requested_frames: usize = ((u128::from(duration_ms) * u128::from(spec.sample_rate))
        / 1_000)
        .try_into()
        .map_err(|_| AbTestError::InvalidLevelMatchInput)?;
    if requested_frames == 0 {
        return Err(AbTestError::InvalidLevelMatchInput);
    }
    decoder
        .seek(start_frame)
        .map_err(|error| AbTestError::PathPreparation(error.to_string()))?;

    let mut decoded = DecodedAudio::new(spec.clone());
    let mut samples = Vec::with_capacity(requested_frames.saturating_mul(channels));
    while samples.len() < requested_frames.saturating_mul(channels) {
        let frames = decoder
            .decode_into(&mut decoded)
            .map_err(|error| AbTestError::PathPreparation(error.to_string()))?;
        if frames == 0 {
            break;
        }
        let remaining = requested_frames
            .saturating_mul(channels)
            .saturating_sub(samples.len());
        samples.extend_from_slice(&decoded.samples[..decoded.samples.len().min(remaining)]);
    }
    if samples.len() != requested_frames.saturating_mul(channels) {
        return Err(AbTestError::LevelMatchWindowTooShort);
    }

    Ok(DecodedLevelMatchSegment {
        interleaved_samples: samples,
        sample_rate: spec.sample_rate,
        channels,
        start_ms,
        duration_ms: (u64::try_from(requested_frames)
            .map_err(|_| AbTestError::InvalidLevelMatchInput)?
            * 1_000)
            / u64::from(spec.sample_rate),
    })
}

/// Evidence captured while preparing a fixed level match for a session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevelMatchPreparation {
    pub measurement: LevelMatchMeasurement,
    pub path_a_latency_samples: usize,
    pub path_b_latency_samples: usize,
    pub rendered_frames: usize,
    pub block_frames: usize,
}

/// Inputs for deterministic control-thread rendering of one comparison segment.
#[derive(Debug, Clone, Copy)]
pub struct LevelMatchPreparationRequest<'a> {
    pub path_a: &'a PathConfig,
    pub path_b: &'a PathConfig,
    pub sample_rate: u32,
    pub channels: usize,
    pub interleaved_segment: &'a [f32],
    pub metric: LevelMatchMetric,
    pub max_correction_db: f64,
    pub block_frames: usize,
}

/// Render both paths from the same source segment and measure a fixed path-B
/// correction after removing each path's reported startup latency.
///
/// This is intentionally a control-thread/offline operation. It instantiates
/// fresh hosts so adaptive or history-dependent state from playback cannot
/// influence a repeatable trial.
pub fn prepare_level_match(
    request: LevelMatchPreparationRequest<'_>,
) -> Result<LevelMatchPreparation, AbTestError> {
    let LevelMatchPreparationRequest {
        path_a,
        path_b,
        sample_rate,
        channels,
        interleaved_segment,
        metric,
        max_correction_db,
        block_frames,
    } = request;
    validate_segment(sample_rate, channels, interleaved_segment, block_frames)?;

    let (rendered_a, latency_a) = render_path(
        path_a,
        sample_rate,
        channels,
        interleaved_segment,
        block_frames,
    )?;
    let (rendered_b, latency_b) = render_path(
        path_b,
        sample_rate,
        channels,
        interleaved_segment,
        block_frames,
    )?;
    let measurement = measure_level_match(
        metric,
        sample_rate,
        channels,
        &rendered_a,
        &rendered_b,
        max_correction_db,
    )?;

    Ok(LevelMatchPreparation {
        measurement,
        path_a_latency_samples: latency_a,
        path_b_latency_samples: latency_b,
        rendered_frames: interleaved_segment.len() / channels,
        block_frames,
    })
}

/// Persist a validated session as human-readable JSON.
///
/// Pending blind assignments are omitted by the session's serialization
/// contract, so saving cannot disclose an in-progress answer.
pub fn save_ab_test_session(
    session: &AbTestSession,
    path: impl AsRef<Path>,
) -> Result<(), AbTestError> {
    session.validate()?;
    let json = serde_json::to_vec_pretty(session)
        .map_err(|error| AbTestError::Serialization(error.to_string()))?;
    std::fs::write(path, json).map_err(|error| AbTestError::SessionIo(error.to_string()))
}

/// Load and validate a session before exposing it to a UI shell.
pub fn load_ab_test_session(path: impl AsRef<Path>) -> Result<AbTestSession, AbTestError> {
    let json = std::fs::read(path).map_err(|error| AbTestError::SessionIo(error.to_string()))?;
    let session: AbTestSession = serde_json::from_slice(&json)
        .map_err(|error| AbTestError::Serialization(error.to_string()))?;
    session.validate()?;
    Ok(session)
}

fn validate_segment(
    sample_rate: u32,
    channels: usize,
    samples: &[f32],
    block_frames: usize,
) -> Result<(), AbTestError> {
    if sample_rate == 0
        || channels == 0
        || samples.is_empty()
        || !samples.len().is_multiple_of(channels)
        || block_frames == 0
        || samples.iter().any(|sample| !sample.is_finite())
    {
        return Err(AbTestError::InvalidLevelMatchInput);
    }
    Ok(())
}

fn render_path(
    config: &PathConfig,
    sample_rate: u32,
    channels: usize,
    input: &[f32],
    block_frames: usize,
) -> Result<(Vec<f32>, usize), AbTestError> {
    let mut host = build_path_from_config_with_factory(
        config,
        channels,
        sample_rate,
        Some(sotf_plugins::create_plugin),
    )
    .map_err(AbTestError::PathPreparation)?;
    host.build().map_err(AbTestError::PathPreparation)?;

    if host.input_channels() != channels
        || host.output_channels() != channels
        || host.output_sample_rate(sample_rate) != sample_rate
        || host.output_frames_for_input(block_frames) != block_frames
    {
        return Err(AbTestError::IncompatiblePathLayout);
    }

    let latency = host.total_latency_samples();
    let source_frames = input.len() / channels;
    let frames_to_process = (source_frames + latency).div_ceil(block_frames) * block_frames;
    let mut rendered = Vec::with_capacity(frames_to_process * channels);
    let mut block_input = vec![0.0_f32; block_frames * channels];
    let mut block_output = vec![0.0_f32; block_frames * channels];

    for block_start in (0..frames_to_process).step_by(block_frames) {
        block_input.fill(0.0);
        block_output.fill(0.0);
        let available_frames = source_frames.saturating_sub(block_start).min(block_frames);
        if available_frames > 0 {
            let source_start = block_start * channels;
            let source_end = source_start + available_frames * channels;
            block_input[..available_frames * channels]
                .copy_from_slice(&input[source_start..source_end]);
        }

        let produced = host
            .process(&block_input, &mut block_output)
            .map_err(AbTestError::PathPreparation)?;
        if produced != block_frames {
            return Err(AbTestError::IncompatiblePathLayout);
        }
        rendered.extend_from_slice(&block_output);
    }

    let start = latency
        .checked_mul(channels)
        .ok_or(AbTestError::InvalidLevelMatchInput)?;
    let end = start
        .checked_add(input.len())
        .ok_or(AbTestError::InvalidLevelMatchInput)?;
    let aligned = rendered
        .get(start..end)
        .ok_or(AbTestError::PathPreparation(
            "path did not produce enough samples to drain its declared latency".to_owned(),
        ))?
        .to_vec();
    if aligned.iter().any(|sample| !sample.is_finite()) {
        return Err(AbTestError::PathPreparation(
            "path produced non-finite audio during level matching".to_owned(),
        ));
    }
    Ok((aligned, latency))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controllers::ab_test_session::{
        ChainSnapshot, ListeningTestSetup, MediaSegment, TrialMode,
    };
    use sotf_plugins::plugin_ab_compare::PluginInRack;

    fn stereo_sine(seconds: usize, amplitude: f32) -> Vec<f32> {
        let mut samples = Vec::with_capacity(48_000 * seconds * 2);
        for frame in 0..48_000 * seconds {
            let sample =
                amplitude * (std::f32::consts::TAU * 1_000.0 * frame as f32 / 48_000.0).sin();
            samples.extend_from_slice(&[sample, sample]);
        }
        samples
    }

    #[test]
    fn preparation_uses_full_factory_and_removes_reported_path_latency() {
        let path_a = PathConfig::Plugin {
            plugin_type: "gain".into(),
            parameters: serde_json::json!({"gain_db": -6.0}),
        };
        let path_b = PathConfig::Rack {
            plugins: vec![
                PluginInRack {
                    plugin_type: "loudness_monitor".into(),
                    parameters: serde_json::json!({}),
                },
                PluginInRack {
                    plugin_type: "limiter".into(),
                    parameters: serde_json::json!({
                        "threshold_db": 0.0,
                        "lookahead_ms": 10.0,
                        "true_peak": false,
                        "feed_forward": false,
                    }),
                },
                PluginInRack {
                    plugin_type: "gain".into(),
                    parameters: serde_json::json!({"gain_db": -6.0}),
                },
            ],
        };
        let segment = stereo_sine(1, 0.25);
        let preparation = prepare_level_match(LevelMatchPreparationRequest {
            path_a: &path_a,
            path_b: &path_b,
            sample_rate: 48_000,
            channels: 2,
            interleaved_segment: &segment,
            metric: LevelMatchMetric::Rms,
            max_correction_db: 12.0,
            block_frames: 480,
        })
        .unwrap();

        assert_eq!(preparation.path_a_latency_samples, 0);
        assert_eq!(preparation.path_b_latency_samples, 480);
        assert!(preparation.measurement.correction_b_db.abs() < 0.01);
        assert_eq!(preparation.rendered_frames, 48_000);
    }

    #[test]
    fn preparation_rejects_output_layout_changes_the_runtime_cannot_compare() {
        let mono_to_stereo = PathConfig::Plugin {
            plugin_type: "mono_to_stereo".into(),
            parameters: serde_json::json!({}),
        };
        assert!(matches!(
            prepare_level_match(LevelMatchPreparationRequest {
                path_a: &PathConfig::None,
                path_b: &mono_to_stereo,
                sample_rate: 48_000,
                channels: 1,
                interleaved_segment: &vec![0.1; 48_000],
                metric: LevelMatchMetric::Rms,
                max_correction_db: 6.0,
                block_frames: 480,
            }),
            Err(AbTestError::IncompatiblePathLayout)
        ));
    }

    #[test]
    fn validated_session_persistence_omits_pending_assignment() {
        let level_match = LevelMatchMeasurement {
            metric: LevelMatchMetric::Rms,
            window_ms: 1_000,
            path_a_db: -12.0,
            path_b_db: -12.0,
            correction_b_db: 0.0,
            max_correction_db: 6.0,
        };
        let setup = ListeningTestSetup {
            path_a: ChainSnapshot::new("TDF I", PathConfig::None).unwrap(),
            path_b: ChainSnapshot::new("TDF II", PathConfig::None).unwrap(),
            media: MediaSegment {
                media_id: "fixture".into(),
                start_ms: 0,
                duration_ms: 1_000,
            },
            sample_rate: 48_000,
            channels: 2,
            level_match,
            switch_transition_ms: 20.0,
            participant_id: None,
            app_version: "test".into(),
        };
        let mut session = AbTestSession::new("session", setup, 42).unwrap();
        session.start_trial(TrialMode::Abx).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.json");

        save_ab_test_session(&session, &path).unwrap();
        let json = std::fs::read_to_string(&path).unwrap();
        assert!(!json.contains("pending"));
        let restored = load_ab_test_session(&path).unwrap();
        assert!(restored.trials.is_empty());
        assert_eq!(restored.pending_mode(), None);
        restored.validate().unwrap();
    }
}
