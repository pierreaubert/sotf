//! Reproducible listening-test sessions for chain-level A/B and ABX comparisons.
//!
//! This module owns the test protocol and persistence model. UI shells only
//! choose cues, submit answers, and render committed records. Audio routing
//! remains in the existing A/B Compare plugin.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sotf_plugins::plugin_ab_compare::{
    ABComparePluginParams, GraphEdgeConfig, GraphNodeConfig, MixMode, PathConfig, PluginInRack,
};
use sotf_plugins::{LoudnessData, LoudnessMonitor};

const SESSION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathSelection {
    A,
    B,
}

impl PathSelection {
    fn opposite(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrialMode {
    BlindAb,
    Abx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrialCue {
    First,
    Second,
    ReferenceA,
    ReferenceB,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrialAnswer {
    First,
    Second,
    A,
    B,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainSnapshot {
    pub label: String,
    pub config: PathConfig,
    pub sha256: String,
}

impl ChainSnapshot {
    pub fn new(label: impl Into<String>, config: PathConfig) -> Result<Self, AbTestError> {
        let sha256 = hash_path_config(&config)?;
        Ok(Self {
            label: label.into(),
            config,
            sha256,
        })
    }

    pub fn verify(&self) -> Result<(), AbTestError> {
        if hash_path_config(&self.config)? == self.sha256 {
            Ok(())
        } else {
            Err(AbTestError::ChainSnapshotHashMismatch)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaSegment {
    /// Stable media identity, such as a library track ID or content hash.
    pub media_id: String,
    pub start_ms: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LevelMatchMetric {
    MomentaryLufs,
    ShortTermLufs,
    Rms,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevelMatchMeasurement {
    pub metric: LevelMatchMetric,
    pub window_ms: u64,
    pub path_a_db: f64,
    pub path_b_db: f64,
    /// Gain applied to path B to match path A.
    pub correction_b_db: f64,
    pub max_correction_db: f64,
}

impl LevelMatchMeasurement {
    pub fn validate(&self) -> Result<(), AbTestError> {
        let values = [
            self.path_a_db,
            self.path_b_db,
            self.correction_b_db,
            self.max_correction_db,
        ];
        if self.window_ms == 0 || values.iter().any(|value| !value.is_finite()) {
            return Err(AbTestError::InvalidLevelMatch);
        }
        if self.max_correction_db < 0.0
            || self.correction_b_db.abs() > self.max_correction_db + f64::EPSILON
        {
            return Err(AbTestError::InvalidLevelMatch);
        }
        Ok(())
    }
}

/// Measure two synchronized, interleaved renders and return the fixed gain
/// correction that makes path B match path A.
///
/// LUFS measurements use the same EBU R128 implementation as the runtime
/// loudness monitor. The correction is deliberately measured once and saved
/// in the session instead of adapting during a blind trial.
pub fn measure_level_match(
    metric: LevelMatchMetric,
    sample_rate: u32,
    channels: usize,
    path_a: &[f32],
    path_b: &[f32],
    max_correction_db: f64,
) -> Result<LevelMatchMeasurement, AbTestError> {
    if sample_rate == 0
        || channels == 0
        || u32::try_from(channels).is_err()
        || path_a.is_empty()
        || path_a.len() != path_b.len()
        || !path_a.len().is_multiple_of(channels)
        || !max_correction_db.is_finite()
        || max_correction_db < 0.0
    {
        return Err(AbTestError::InvalidLevelMatchInput);
    }
    if path_a
        .iter()
        .chain(path_b)
        .any(|sample| !sample.is_finite())
    {
        return Err(AbTestError::InvalidLevelMatchInput);
    }

    let frames = path_a.len() / channels;
    let minimum_window_ms = match metric {
        LevelMatchMetric::MomentaryLufs => 400,
        LevelMatchMetric::ShortTermLufs => 3_000,
        LevelMatchMetric::Rms => 0,
    };
    let minimum_frames = (u64::from(sample_rate) * minimum_window_ms).div_ceil(1_000) as usize;
    if frames < minimum_frames {
        return Err(AbTestError::LevelMatchWindowTooShort);
    }

    let path_a_db = measure_path_level(metric, sample_rate, channels, path_a)?;
    let path_b_db = measure_path_level(metric, sample_rate, channels, path_b)?;
    let measurement = LevelMatchMeasurement {
        metric,
        window_ms: ((frames as u128 * 1_000) / u128::from(sample_rate))
            .try_into()
            .map_err(|_| AbTestError::InvalidLevelMatchInput)?,
        path_a_db,
        path_b_db,
        correction_b_db: (path_a_db - path_b_db).clamp(-max_correction_db, max_correction_db),
        max_correction_db,
    };
    measurement.validate()?;
    Ok(measurement)
}

fn measure_path_level(
    metric: LevelMatchMetric,
    sample_rate: u32,
    channels: usize,
    samples: &[f32],
) -> Result<f64, AbTestError> {
    let level = match metric {
        LevelMatchMetric::Rms => {
            let mean_square = samples
                .iter()
                .map(|sample| f64::from(*sample).powi(2))
                .sum::<f64>()
                / samples.len() as f64;
            if mean_square <= 0.0 {
                return Err(AbTestError::LevelMatchSignalTooQuiet);
            }
            10.0 * mean_square.log10()
        }
        LevelMatchMetric::MomentaryLufs | LevelMatchMetric::ShortTermLufs => {
            let mut monitor = LoudnessMonitor::new(channels as u32, sample_rate)
                .map_err(AbTestError::LevelMatchMeasurement)?;
            monitor
                .add_frames(samples)
                .map_err(AbTestError::LevelMatchMeasurement)?;
            let mut data = LoudnessData::new(channels);
            monitor.update_loudness_data(&mut data);
            match metric {
                LevelMatchMetric::MomentaryLufs => data.momentary_lufs,
                LevelMatchMetric::ShortTermLufs => data.shortterm_lufs,
                LevelMatchMetric::Rms => unreachable!(),
            }
        }
    };
    if !level.is_finite() || level <= -120.0 {
        return Err(AbTestError::LevelMatchSignalTooQuiet);
    }
    Ok(level)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListeningTestSetup {
    pub path_a: ChainSnapshot,
    pub path_b: ChainSnapshot,
    pub media: MediaSegment,
    pub sample_rate: u32,
    pub channels: usize,
    pub level_match: LevelMatchMeasurement,
    /// Equal-power path switch/crossfade duration used during every trial.
    pub switch_transition_ms: f32,
    pub participant_id: Option<String>,
    pub app_version: String,
}

impl ListeningTestSetup {
    pub fn validate(&self) -> Result<(), AbTestError> {
        self.path_a.verify()?;
        self.path_b.verify()?;
        self.level_match.validate()?;
        if !self.switch_transition_ms.is_finite()
            || !(0.0..=1_000.0).contains(&self.switch_transition_ms)
        {
            return Err(AbTestError::InvalidSwitchTransition);
        }
        if self.media.media_id.is_empty()
            || self.media.duration_ms == 0
            || self.sample_rate == 0
            || self.channels == 0
            || self.app_version.is_empty()
        {
            return Err(AbTestError::InvalidSetup);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
enum TrialAssignment {
    BlindAb { first: PathSelection },
    Abx { unknown: PathSelection },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrialResult {
    Preference(PathSelection),
    Correct,
    Incorrect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrialRecord {
    pub index: u32,
    pub mode: TrialMode,
    pub answer: TrialAnswer,
    pub result: TrialResult,
    pub confidence: Option<u8>,
    pub notes: Option<String>,
    assignment: TrialAssignment,
}

impl TrialRecord {
    /// Reveal the assignment only after the trial has been committed.
    pub fn path_for_cue(&self, cue: TrialCue) -> Result<PathSelection, AbTestError> {
        route_for_assignment(self.assignment, cue)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct PendingTrial {
    index: u32,
    mode: TrialMode,
    assignment: TrialAssignment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbTestSession {
    pub schema_version: u32,
    pub session_id: String,
    pub setup: ListeningTestSetup,
    pub assignment_seed: u64,
    pub trials: Vec<TrialRecord>,
    /// In-progress assignments are deliberately not persisted: a saved session
    /// must not disclose a blind answer before it is committed.
    #[serde(skip)]
    pending: Option<PendingTrial>,
}

impl AbTestSession {
    pub fn new(
        session_id: impl Into<String>,
        setup: ListeningTestSetup,
        assignment_seed: u64,
    ) -> Result<Self, AbTestError> {
        setup.validate()?;
        let session_id = session_id.into();
        if session_id.is_empty() {
            return Err(AbTestError::InvalidSetup);
        }
        Ok(Self {
            schema_version: SESSION_SCHEMA_VERSION,
            session_id,
            setup,
            assignment_seed,
            trials: Vec::new(),
            pending: None,
        })
    }

    pub fn start_trial(&mut self, mode: TrialMode) -> Result<u32, AbTestError> {
        if self.pending.is_some() {
            return Err(AbTestError::TrialAlreadyPending);
        }
        let index = u32::try_from(self.trials.len()).map_err(|_| AbTestError::TooManyTrials)?;
        self.pending = Some(PendingTrial {
            index,
            mode,
            assignment: assignment_for(self.assignment_seed, index, mode),
        });
        Ok(index)
    }

    pub fn pending_mode(&self) -> Option<TrialMode> {
        self.pending.as_ref().map(|trial| trial.mode)
    }

    /// Resolve a playback cue without exposing the concealed assignment in UI state.
    pub fn path_for_pending_cue(&self, cue: TrialCue) -> Result<PathSelection, AbTestError> {
        let pending = self.pending.as_ref().ok_or(AbTestError::NoPendingTrial)?;
        route_for_assignment(pending.assignment, cue)
    }

    /// Build the exact AB-Compare runtime configuration for a concealed cue.
    ///
    /// The saved level-match correction is baked into path B as a final gain
    /// stage. AB-Compare's adaptive auto-gain is disabled so repeated trials
    /// use identical DSP rather than a programme-dependent learned state.
    pub fn runtime_config_for_pending_cue(
        &self,
        cue: TrialCue,
    ) -> Result<ABComparePluginParams, AbTestError> {
        let selected = self.path_for_pending_cue(cue)?;
        self.setup.runtime_config(selected)
    }

    pub fn commit_trial(
        &mut self,
        answer: TrialAnswer,
        confidence: Option<u8>,
        notes: Option<String>,
    ) -> Result<&TrialRecord, AbTestError> {
        if confidence.is_some_and(|value| value > 100) {
            return Err(AbTestError::InvalidConfidence);
        }
        let pending = self.pending.as_ref().ok_or(AbTestError::NoPendingTrial)?;
        let result = result_for(pending.assignment, answer)?;
        let pending = self.pending.take().ok_or(AbTestError::NoPendingTrial)?;
        self.trials.push(TrialRecord {
            index: pending.index,
            mode: pending.mode,
            answer,
            result,
            confidence,
            notes: notes.filter(|note| !note.trim().is_empty()),
            assignment: pending.assignment,
        });
        let record_index = self.trials.len() - 1;
        Ok(&self.trials[record_index])
    }

    pub fn cancel_pending_trial(&mut self) -> bool {
        self.pending.take().is_some()
    }

    pub fn abx_score(&self) -> (usize, usize) {
        self.trials
            .iter()
            .fold((0, 0), |(correct, total), trial| match trial.result {
                TrialResult::Correct => (correct + 1, total + 1),
                TrialResult::Incorrect => (correct, total + 1),
                TrialResult::Preference(_) => (correct, total),
            })
    }

    pub fn validate(&self) -> Result<(), AbTestError> {
        if self.schema_version != SESSION_SCHEMA_VERSION {
            return Err(AbTestError::UnsupportedSchema(self.schema_version));
        }
        self.setup.validate()?;
        for (expected, trial) in self.trials.iter().enumerate() {
            if trial.index as usize != expected
                || trial.assignment != assignment_for(self.assignment_seed, trial.index, trial.mode)
                || result_for(trial.assignment, trial.answer)? != trial.result
            {
                return Err(AbTestError::InvalidTrialRecord);
            }
        }
        Ok(())
    }
}

impl ListeningTestSetup {
    fn runtime_config(
        &self,
        selected: PathSelection,
    ) -> Result<ABComparePluginParams, AbTestError> {
        self.validate()?;
        Ok(ABComparePluginParams {
            path_a: self.path_a.config.clone(),
            path_b: append_gain_stage(&self.path_b.config, self.level_match.correction_b_db as f32),
            mix_mode: MixMode::Binary,
            mix: if selected == PathSelection::A {
                -1.0
            } else {
                1.0
            },
            selected_path: match selected {
                PathSelection::A => 0,
                PathSelection::B => 1,
            },
            bypass: false,
            auto_gain_enabled: false,
            gain_smoothing_ms: 0.0,
            max_auto_gain_db: self.level_match.max_correction_db as f32,
            mix_transition_ms: self.switch_transition_ms,
            ..ABComparePluginParams::default()
        })
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AbTestError {
    #[error("invalid listening-test setup")]
    InvalidSetup,
    #[error("invalid level-match measurement")]
    InvalidLevelMatch,
    #[error("invalid synchronized audio supplied for level matching")]
    InvalidLevelMatchInput,
    #[error("audio is shorter than the selected level-match metric requires")]
    LevelMatchWindowTooShort,
    #[error("audio is too quiet to level match reliably")]
    LevelMatchSignalTooQuiet,
    #[error("level-match measurement failed: {0}")]
    LevelMatchMeasurement(String),
    #[error("failed to prepare a comparison path: {0}")]
    PathPreparation(String),
    #[error(
        "comparison paths must preserve the selected channel layout, sample rate, and frame rate"
    )]
    IncompatiblePathLayout,
    #[error("failed to read or write a listening-test session: {0}")]
    SessionIo(String),
    #[error("invalid listening-test switch transition")]
    InvalidSwitchTransition,
    #[error("chain snapshot no longer matches its hash")]
    ChainSnapshotHashMismatch,
    #[error("a trial is already pending")]
    TrialAlreadyPending,
    #[error("there is no pending trial")]
    NoPendingTrial,
    #[error("answer is not valid for this trial mode")]
    InvalidAnswer,
    #[error("confidence must be between 0 and 100")]
    InvalidConfidence,
    #[error("too many trials in one session")]
    TooManyTrials,
    #[error("unsupported listening-test schema version {0}")]
    UnsupportedSchema(u32),
    #[error("trial record failed reproducibility validation")]
    InvalidTrialRecord,
    #[error("failed to serialize a chain snapshot: {0}")]
    Serialization(String),
}

fn append_gain_stage(config: &PathConfig, gain_db: f32) -> PathConfig {
    if gain_db.abs() <= f32::EPSILON {
        return config.clone();
    }

    let gain = PluginInRack {
        plugin_type: "gain".to_owned(),
        parameters: serde_json::json!({"gain_db": gain_db}),
    };
    match config {
        PathConfig::None => PathConfig::Rack {
            plugins: vec![gain],
        },
        PathConfig::Plugin {
            plugin_type,
            parameters,
        } => PathConfig::Rack {
            plugins: vec![
                PluginInRack {
                    plugin_type: plugin_type.clone(),
                    parameters: parameters.clone(),
                },
                gain,
            ],
        },
        PathConfig::Rack { plugins } => {
            let mut plugins = plugins.clone();
            plugins.push(gain);
            PathConfig::Rack { plugins }
        }
        PathConfig::Graph { nodes, edges } => append_graph_gain(nodes, edges, gain_db),
    }
}

fn append_graph_gain(
    nodes: &[GraphNodeConfig],
    edges: &[GraphEdgeConfig],
    gain_db: f32,
) -> PathConfig {
    let mut runtime_nodes = nodes.to_vec();
    let mut runtime_edges = edges.to_vec();
    let mut gain_id = "__sotf_ab_level_match_b".to_owned();
    let mut suffix = 1usize;
    while runtime_nodes.iter().any(|node| node.id == gain_id) {
        gain_id = format!("__sotf_ab_level_match_b_{suffix}");
        suffix += 1;
    }

    let sinks: Vec<String> = runtime_nodes
        .iter()
        .filter(|node| !runtime_edges.iter().any(|edge| edge.from == node.id))
        .map(|node| node.id.clone())
        .collect();
    runtime_nodes.push(GraphNodeConfig {
        id: gain_id.clone(),
        plugin_type: "gain".to_owned(),
        parameters: serde_json::json!({"gain_db": gain_db}),
    });
    runtime_edges.extend(sinks.into_iter().map(|sink| GraphEdgeConfig {
        from: sink,
        to: gain_id.clone(),
        channel_map: None,
        destination_offset: 0,
    }));

    PathConfig::Graph {
        nodes: runtime_nodes,
        edges: runtime_edges,
    }
}

fn hash_path_config(config: &PathConfig) -> Result<String, AbTestError> {
    let encoded = serde_json::to_vec(config)
        .map_err(|error| AbTestError::Serialization(error.to_string()))?;
    let digest = Sha256::digest(encoded);
    Ok(format!("{digest:x}"))
}

fn assignment_for(seed: u64, index: u32, mode: TrialMode) -> TrialAssignment {
    // Randomize the first trial in each pair, then invert the second. This is
    // deterministic and keeps assignments balanced over every complete pair.
    let pair = index / 2;
    let mut hasher = Sha256::new();
    hasher.update(seed.to_le_bytes());
    hasher.update(pair.to_le_bytes());
    hasher.update([match mode {
        TrialMode::BlindAb => 0,
        TrialMode::Abx => 1,
    }]);
    let first = if hasher.finalize()[0] & 1 == 0 {
        PathSelection::A
    } else {
        PathSelection::B
    };
    let selected = if index.is_multiple_of(2) {
        first
    } else {
        first.opposite()
    };
    match mode {
        TrialMode::BlindAb => TrialAssignment::BlindAb { first: selected },
        TrialMode::Abx => TrialAssignment::Abx { unknown: selected },
    }
}

fn route_for_assignment(
    assignment: TrialAssignment,
    cue: TrialCue,
) -> Result<PathSelection, AbTestError> {
    match (assignment, cue) {
        (TrialAssignment::BlindAb { first }, TrialCue::First) => Ok(first),
        (TrialAssignment::BlindAb { first }, TrialCue::Second) => Ok(first.opposite()),
        (TrialAssignment::Abx { .. }, TrialCue::ReferenceA) => Ok(PathSelection::A),
        (TrialAssignment::Abx { .. }, TrialCue::ReferenceB) => Ok(PathSelection::B),
        (TrialAssignment::Abx { unknown }, TrialCue::Unknown) => Ok(unknown),
        _ => Err(AbTestError::InvalidAnswer),
    }
}

fn result_for(
    assignment: TrialAssignment,
    answer: TrialAnswer,
) -> Result<TrialResult, AbTestError> {
    match (assignment, answer) {
        (TrialAssignment::BlindAb { first }, TrialAnswer::First) => {
            Ok(TrialResult::Preference(first))
        }
        (TrialAssignment::BlindAb { first }, TrialAnswer::Second) => {
            Ok(TrialResult::Preference(first.opposite()))
        }
        (TrialAssignment::Abx { unknown }, TrialAnswer::A) => Ok(if unknown == PathSelection::A {
            TrialResult::Correct
        } else {
            TrialResult::Incorrect
        }),
        (TrialAssignment::Abx { unknown }, TrialAnswer::B) => Ok(if unknown == PathSelection::B {
            TrialResult::Correct
        } else {
            TrialResult::Incorrect
        }),
        _ => Err(AbTestError::InvalidAnswer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> ListeningTestSetup {
        ListeningTestSetup {
            path_a: ChainSnapshot::new("TDF I", PathConfig::None).unwrap(),
            path_b: ChainSnapshot::new("TDF II", PathConfig::Rack { plugins: vec![] }).unwrap(),
            media: MediaSegment {
                media_id: "track-sha256".into(),
                start_ms: 10_000,
                duration_ms: 20_000,
            },
            sample_rate: 48_000,
            channels: 2,
            level_match: LevelMatchMeasurement {
                metric: LevelMatchMetric::ShortTermLufs,
                window_ms: 3_000,
                path_a_db: -18.0,
                path_b_db: -16.5,
                correction_b_db: -1.5,
                max_correction_db: 6.0,
            },
            switch_transition_ms: 20.0,
            participant_id: Some("listener-1".into()),
            app_version: "0.5.125".into(),
        }
    }

    fn stereo_sine(sample_rate: u32, duration_seconds: usize, amplitude: f32) -> Vec<f32> {
        let frames = sample_rate as usize * duration_seconds;
        let mut samples = Vec::with_capacity(frames * 2);
        for frame in 0..frames {
            let sample = amplitude
                * (std::f32::consts::TAU * 1_000.0 * frame as f32 / sample_rate as f32).sin();
            samples.extend_from_slice(&[sample, sample]);
        }
        samples
    }

    #[test]
    fn rms_level_match_measures_and_clamps_fixed_path_b_correction() {
        let path_a = stereo_sine(48_000, 1, 0.25);
        let path_b = stereo_sine(48_000, 1, 0.5);
        let measured =
            measure_level_match(LevelMatchMetric::Rms, 48_000, 2, &path_a, &path_b, 3.0).unwrap();

        assert!((measured.path_b_db - measured.path_a_db - 6.0206).abs() < 0.001);
        assert_eq!(measured.correction_b_db, -3.0);
        assert_eq!(measured.window_ms, 1_000);
    }

    #[test]
    fn ebu_level_match_uses_runtime_loudness_measurement() {
        let path_a = stereo_sine(48_000, 3, 0.1);
        let path_b = stereo_sine(48_000, 3, 0.2);

        for metric in [
            LevelMatchMetric::MomentaryLufs,
            LevelMatchMetric::ShortTermLufs,
        ] {
            let measured = measure_level_match(metric, 48_000, 2, &path_a, &path_b, 12.0).unwrap();
            assert!((measured.correction_b_db + 6.0206).abs() < 0.05);
            assert_eq!(measured.window_ms, 3_000);
        }
    }

    #[test]
    fn level_match_rejects_unsynchronized_short_or_silent_audio() {
        let short = vec![0.1; 48_000 * 2];
        assert_eq!(
            measure_level_match(
                LevelMatchMetric::ShortTermLufs,
                48_000,
                2,
                &short,
                &short,
                6.0,
            ),
            Err(AbTestError::LevelMatchWindowTooShort)
        );

        assert_eq!(
            measure_level_match(LevelMatchMetric::Rms, 48_000, 2, &[0.0; 4], &[0.0; 4], 6.0),
            Err(AbTestError::LevelMatchSignalTooQuiet)
        );
        assert_eq!(
            measure_level_match(LevelMatchMetric::Rms, 48_000, 2, &[0.1; 4], &[0.1; 2], 6.0),
            Err(AbTestError::InvalidLevelMatchInput)
        );
    }

    #[test]
    fn assignments_are_balanced_and_repeatable() {
        for mode in [TrialMode::BlindAb, TrialMode::Abx] {
            let a0 = assignment_for(42, 0, mode);
            let a1 = assignment_for(42, 1, mode);
            assert_eq!(a0, assignment_for(42, 0, mode));
            assert_ne!(a0, a1);
        }
    }

    #[test]
    fn pending_assignment_is_not_persisted_or_exposed_as_a_record() {
        let mut session = AbTestSession::new("session-1", setup(), 42).unwrap();
        session.start_trial(TrialMode::Abx).unwrap();
        assert!(session.trials.is_empty());
        assert_eq!(session.pending_mode(), Some(TrialMode::Abx));
        let json = serde_json::to_value(&session).unwrap();
        assert!(json.get("pending").is_none());

        let restored: AbTestSession = serde_json::from_value(json).unwrap();
        assert_eq!(restored.pending_mode(), None);
    }

    #[test]
    fn abx_commit_scores_and_reveals_assignment() {
        let mut session = AbTestSession::new("session-1", setup(), 42).unwrap();
        session.start_trial(TrialMode::Abx).unwrap();
        let unknown = session.path_for_pending_cue(TrialCue::Unknown).unwrap();
        let answer = match unknown {
            PathSelection::A => TrialAnswer::A,
            PathSelection::B => TrialAnswer::B,
        };
        let record = session
            .commit_trial(answer, Some(80), Some("No switching cue".into()))
            .unwrap();
        assert_eq!(record.result, TrialResult::Correct);
        assert_eq!(record.path_for_cue(TrialCue::Unknown).unwrap(), unknown);
        assert_eq!(session.abx_score(), (1, 1));
    }

    #[test]
    fn invalid_answer_keeps_pending_trial() {
        let mut session = AbTestSession::new("session-1", setup(), 42).unwrap();
        session.start_trial(TrialMode::BlindAb).unwrap();
        assert_eq!(
            session.commit_trial(TrialAnswer::A, None, None),
            Err(AbTestError::InvalidAnswer)
        );
        assert_eq!(session.pending_mode(), Some(TrialMode::BlindAb));
    }

    #[test]
    fn session_round_trip_preserves_and_validates_results() {
        let mut session = AbTestSession::new("session-1", setup(), 7).unwrap();
        session.start_trial(TrialMode::BlindAb).unwrap();
        session
            .commit_trial(TrialAnswer::First, Some(65), None)
            .unwrap();
        session.start_trial(TrialMode::Abx).unwrap();
        let unknown = session.path_for_pending_cue(TrialCue::Unknown).unwrap();
        session
            .commit_trial(
                if unknown == PathSelection::A {
                    TrialAnswer::A
                } else {
                    TrialAnswer::B
                },
                None,
                None,
            )
            .unwrap();

        let json = serde_json::to_string_pretty(&session).unwrap();
        let restored: AbTestSession = serde_json::from_str(&json).unwrap();
        assert_eq!(
            serde_json::to_value(&restored).unwrap(),
            serde_json::to_value(&session).unwrap()
        );
        restored.validate().unwrap();
    }

    #[test]
    fn snapshot_hash_detects_modified_graph() {
        let mut snapshot = ChainSnapshot::new("A", PathConfig::None).unwrap();
        snapshot.config = PathConfig::Rack { plugins: vec![] };
        assert_eq!(
            snapshot.verify(),
            Err(AbTestError::ChainSnapshotHashMismatch)
        );
    }

    #[test]
    fn rejects_out_of_bounds_level_match() {
        let mut invalid = setup();
        invalid.level_match.correction_b_db = 7.0;
        assert_eq!(invalid.validate(), Err(AbTestError::InvalidLevelMatch));
    }

    #[test]
    fn runtime_config_uses_saved_fixed_level_match() {
        let mut session = AbTestSession::new("session-1", setup(), 42).unwrap();
        session.start_trial(TrialMode::BlindAb).unwrap();
        let config = session
            .runtime_config_for_pending_cue(TrialCue::First)
            .unwrap();

        assert_eq!(config.mix_mode, MixMode::Binary);
        assert!(!config.auto_gain_enabled);
        assert_eq!(config.mix_transition_ms, 20.0);
        assert!(matches!(config.selected_path, 0 | 1));
        match config.path_b {
            PathConfig::Rack { plugins } => {
                assert_eq!(plugins.len(), 1);
                assert_eq!(plugins[0].plugin_type, "gain");
                assert_eq!(plugins[0].parameters["gain_db"], -1.5);
            }
            _ => panic!("level-matched empty rack must end in a fixed gain stage"),
        }
    }

    #[test]
    fn level_matching_graph_connects_every_sink_without_mutating_snapshot() {
        let original = PathConfig::Graph {
            nodes: vec![
                GraphNodeConfig {
                    id: "source".into(),
                    plugin_type: "gain".into(),
                    parameters: serde_json::json!({}),
                },
                GraphNodeConfig {
                    id: "left_sink".into(),
                    plugin_type: "eq".into(),
                    parameters: serde_json::json!({}),
                },
                GraphNodeConfig {
                    id: "right_sink".into(),
                    plugin_type: "delay".into(),
                    parameters: serde_json::json!({}),
                },
            ],
            edges: vec![GraphEdgeConfig {
                from: "source".into(),
                to: "left_sink".into(),
                channel_map: None,
                destination_offset: 0,
            }],
        };
        let snapshot = ChainSnapshot::new("B", original.clone()).unwrap();
        let runtime = append_gain_stage(&original, -2.25);
        snapshot.verify().unwrap();

        let PathConfig::Graph { nodes, edges } = runtime else {
            panic!("graph level matching must preserve graph topology");
        };
        let gain = nodes
            .iter()
            .find(|node| node.id.starts_with("__sotf_ab_level_match_b"))
            .expect("runtime graph needs a final fixed-gain node");
        assert_eq!(gain.plugin_type, "gain");
        assert_eq!(gain.parameters["gain_db"], -2.25);
        let gain_inputs: Vec<_> = edges
            .iter()
            .filter(|edge| edge.to == gain.id)
            .map(|edge| edge.from.as_str())
            .collect();
        assert_eq!(gain_inputs.len(), 2);
        assert!(gain_inputs.contains(&"left_sink"));
        assert!(gain_inputs.contains(&"right_sink"));
    }

    #[test]
    fn rejects_invalid_switch_transition() {
        let mut invalid = setup();
        invalid.switch_transition_ms = f32::NAN;
        assert_eq!(
            invalid.validate(),
            Err(AbTestError::InvalidSwitchTransition)
        );
    }
}
