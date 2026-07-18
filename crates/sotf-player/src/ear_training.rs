//! Reusable domain model for critical-listening exercises.
//!
//! The model is intentionally UI and audio-engine independent. Frontends can
//! render a session, persist it, and turn [`EqTrainingQuestion::filter`] into
//! an isolated comparison path without duplicating progression or scoring.

use math_audio_iir_fir::BiquadFilterType;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::EQFilter;

const MIN_BANDS: usize = 2;
const MAX_BANDS: usize = 25;
const MIN_FREQUENCY_HZ: f64 = 20.0;
const MAX_FREQUENCY_HZ: f64 = 20_000.0;
const GAIN_CHOICES_DB: [f64; 4] = [3.0, 6.0, 9.0, 12.0];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EqTrainingExercise {
    #[default]
    BandIdentification,
    BoostCutIdentification,
    GainIdentification,
}

impl EqTrainingExercise {
    pub const ALL: [Self; 3] = [
        Self::BandIdentification,
        Self::BoostCutIdentification,
        Self::GainIdentification,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::BandIdentification => "Frequency band",
            Self::BoostCutIdentification => "Boost or cut",
            Self::GainIdentification => "Gain amount",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EarTrainingCourse {
    #[default]
    Foundations,
    FrequencyRegions,
    Cuts,
    FineBands,
    Mastery,
}

impl EarTrainingCourse {
    pub const ALL: [Self; 5] = [
        Self::Foundations,
        Self::FrequencyRegions,
        Self::Cuts,
        Self::FineBands,
        Self::Mastery,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Foundations => "Foundations",
            Self::FrequencyRegions => "Frequency regions",
            Self::Cuts => "Hearing cuts",
            Self::FineBands => "Fine bands",
            Self::Mastery => "Mastery",
        }
    }

    pub fn config(self) -> EqTrainingConfig {
        match self {
            Self::Foundations => EqTrainingConfig {
                band_count: 3,
                gain_db: 12.0,
                trial_count: 10,
                ..Default::default()
            },
            Self::FrequencyRegions => EqTrainingConfig {
                band_count: 5,
                gain_db: 9.0,
                trial_count: 15,
                ..Default::default()
            },
            Self::Cuts => EqTrainingConfig {
                band_count: 5,
                gain_db: 9.0,
                change_mode: EqChangeMode::Cut,
                trial_count: 15,
                ..Default::default()
            },
            Self::FineBands => EqTrainingConfig {
                band_count: 10,
                gain_db: 6.0,
                trial_count: 20,
                ..Default::default()
            },
            Self::Mastery => EqTrainingConfig {
                band_count: 15,
                gain_db: 3.0,
                trial_count: 25,
                ..Default::default()
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EqChangeMode {
    Boost,
    Cut,
    #[default]
    Mixed,
}

impl EqChangeMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Boost => "Boost",
            Self::Cut => "Cut",
            Self::Mixed => "Boost + cut",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EqChange {
    Boost,
    Cut,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EqTrainingConfig {
    #[serde(default)]
    pub exercise: EqTrainingExercise,
    pub band_count: usize,
    pub min_frequency_hz: f64,
    pub max_frequency_hz: f64,
    pub gain_db: f64,
    pub q: f64,
    pub change_mode: EqChangeMode,
    pub trial_count: usize,
    pub seed: u64,
}

impl Default for EqTrainingConfig {
    fn default() -> Self {
        Self {
            exercise: EqTrainingExercise::BandIdentification,
            band_count: 10,
            min_frequency_hz: 31.5,
            max_frequency_hz: 16_000.0,
            gain_db: 6.0,
            q: 1.4,
            change_mode: EqChangeMode::Mixed,
            trial_count: 20,
            seed: 0x534f_5446_4541_5254,
        }
    }
}

impl EqTrainingConfig {
    pub fn validate(&self) -> Result<(), EqTrainingError> {
        if !(MIN_BANDS..=MAX_BANDS).contains(&self.band_count) {
            return Err(EqTrainingError::InvalidBandCount);
        }
        if !self.min_frequency_hz.is_finite()
            || !self.max_frequency_hz.is_finite()
            || self.min_frequency_hz < MIN_FREQUENCY_HZ
            || self.max_frequency_hz > MAX_FREQUENCY_HZ
            || self.min_frequency_hz >= self.max_frequency_hz
        {
            return Err(EqTrainingError::InvalidFrequencyRange);
        }
        if !self.gain_db.is_finite() || !(1.0..=15.0).contains(&self.gain_db) {
            return Err(EqTrainingError::InvalidGain);
        }
        if !self.q.is_finite() || !(0.2..=10.0).contains(&self.q) {
            return Err(EqTrainingError::InvalidQ);
        }
        if self.trial_count == 0 || self.trial_count > 500 {
            return Err(EqTrainingError::InvalidTrialCount);
        }
        Ok(())
    }

    pub fn band_frequencies(&self) -> Result<Vec<f64>, EqTrainingError> {
        self.validate()?;
        let intervals = (self.band_count - 1) as f64;
        let ratio = (self.max_frequency_hz / self.min_frequency_hz).powf(1.0 / intervals);
        Ok((0..self.band_count)
            .map(|index| self.min_frequency_hz * ratio.powf(index as f64))
            .collect())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EqTrainingQuestion {
    pub number: usize,
    pub band_index: usize,
    pub center_frequency_hz: f64,
    pub change: EqChange,
    pub gain_db: f64,
    pub q: f64,
}

impl EqTrainingQuestion {
    pub fn signed_gain_db(&self) -> f64 {
        match self.change {
            EqChange::Boost => self.gain_db,
            EqChange::Cut => -self.gain_db,
        }
    }

    pub fn filter(&self) -> EQFilter {
        EQFilter::new(
            BiquadFilterType::Peak,
            self.center_frequency_hz,
            self.q,
            self.signed_gain_db(),
        )
    }

    pub fn plugin_parameters(&self) -> serde_json::Value {
        serde_json::json!({ "filters": [self.filter()] })
    }

    pub fn correct_answer(&self, exercise: EqTrainingExercise) -> usize {
        match exercise {
            EqTrainingExercise::BandIdentification => self.band_index,
            EqTrainingExercise::BoostCutIdentification => usize::from(self.change == EqChange::Cut),
            EqTrainingExercise::GainIdentification => GAIN_CHOICES_DB
                .iter()
                .position(|gain| (*gain - self.gain_db).abs() < f64::EPSILON)
                .unwrap_or(0),
        }
    }

    pub fn answer_labels(&self, exercise: EqTrainingExercise, bands: &[f64]) -> Vec<String> {
        match exercise {
            EqTrainingExercise::BandIdentification => bands
                .iter()
                .map(|frequency| format!("{frequency:.0} Hz"))
                .collect(),
            EqTrainingExercise::BoostCutIdentification => vec!["Boost".into(), "Cut".into()],
            EqTrainingExercise::GainIdentification => GAIN_CHOICES_DB
                .iter()
                .map(|gain| format!("{gain:.0} dB"))
                .collect(),
        }
    }

    /// Lightweight log-frequency preview used before an answer is revealed.
    /// Audio always uses the actual biquad returned by [`Self::filter`].
    pub fn preview_curve(&self, points: usize) -> Vec<(f64, f64)> {
        if points < 2 {
            return Vec::new();
        }
        let min_hz: f64 = 20.0;
        let max_hz: f64 = 20_000.0;
        let octaves_per_sigma = 1.0 / self.q.max(0.2);
        (0..points)
            .map(|index| {
                let t = index as f64 / (points - 1) as f64;
                let frequency = min_hz * (max_hz / min_hz).powf(t);
                let distance = (frequency / self.center_frequency_hz).log2();
                let magnitude =
                    self.signed_gain_db() * (-0.5 * (distance / octaves_per_sigma).powi(2)).exp();
                (frequency, magnitude)
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EqTrainingResult {
    pub question: EqTrainingQuestion,
    pub selected_band_index: usize,
    pub correct: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EqBandStats {
    pub attempts: usize,
    pub correct: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EqTrainingSession {
    pub config: EqTrainingConfig,
    pub band_frequencies: Vec<f64>,
    pub trials: Vec<EqTrainingResult>,
    pub current_question: Option<EqTrainingQuestion>,
}

impl EqTrainingSession {
    pub fn new(config: EqTrainingConfig) -> Result<Self, EqTrainingError> {
        let band_frequencies = config.band_frequencies()?;
        Ok(Self {
            config,
            band_frequencies,
            trials: Vec::new(),
            current_question: None,
        })
    }

    pub fn start(&mut self) -> Result<&EqTrainingQuestion, EqTrainingError> {
        if self.current_question.is_some() || !self.trials.is_empty() {
            return Err(EqTrainingError::SessionAlreadyStarted);
        }
        self.current_question = Some(self.generate_question(0));
        Ok(self
            .current_question
            .as_ref()
            .expect("question was just created"))
    }

    pub fn submit_answer(
        &mut self,
        selected_band_index: usize,
    ) -> Result<&EqTrainingResult, EqTrainingError> {
        let answer_count = match self.config.exercise {
            EqTrainingExercise::BandIdentification => self.band_frequencies.len(),
            EqTrainingExercise::BoostCutIdentification => 2,
            EqTrainingExercise::GainIdentification => GAIN_CHOICES_DB.len(),
        };
        if selected_band_index >= answer_count {
            return Err(EqTrainingError::InvalidAnswer);
        }
        let question = self
            .current_question
            .clone()
            .ok_or(EqTrainingError::NoActiveQuestion)?;
        if self.current_is_answered() {
            return Err(EqTrainingError::QuestionAlreadyAnswered);
        }
        self.trials.push(EqTrainingResult {
            correct: selected_band_index == question.correct_answer(self.config.exercise),
            question,
            selected_band_index,
        });
        Ok(self.trials.last().expect("result was just appended"))
    }

    pub fn advance(&mut self) -> Result<Option<&EqTrainingQuestion>, EqTrainingError> {
        if !self.current_is_answered() {
            return Err(EqTrainingError::AnswerRequired);
        }
        if self.trials.len() >= self.config.trial_count {
            self.current_question = None;
            return Ok(None);
        }
        self.current_question = Some(self.generate_question(self.trials.len()));
        Ok(self.current_question.as_ref())
    }

    pub fn current_is_answered(&self) -> bool {
        self.current_question.as_ref().is_some_and(|question| {
            self.trials
                .last()
                .is_some_and(|result| result.question.number == question.number)
        })
    }

    pub fn is_complete(&self) -> bool {
        self.current_question.is_none() && self.trials.len() == self.config.trial_count
    }

    pub fn correct_count(&self) -> usize {
        self.trials.iter().filter(|trial| trial.correct).count()
    }

    pub fn accuracy(&self) -> f64 {
        if self.trials.is_empty() {
            0.0
        } else {
            self.correct_count() as f64 / self.trials.len() as f64
        }
    }

    pub fn band_stats(&self) -> Vec<EqBandStats> {
        let mut stats = vec![EqBandStats::default(); self.band_frequencies.len()];
        for trial in &self.trials {
            let band = &mut stats[trial.question.band_index];
            band.attempts += 1;
            band.correct += usize::from(trial.correct);
        }
        stats
    }

    pub fn weakest_band(&self) -> Option<usize> {
        self.band_stats()
            .into_iter()
            .enumerate()
            .filter(|(_, stats)| stats.attempts > 0)
            .min_by(|(_, left), (_, right)| {
                let left_score = left.correct as f64 / left.attempts as f64;
                let right_score = right.correct as f64 / right.attempts as f64;
                left_score.total_cmp(&right_score)
            })
            .map(|(index, _)| index)
    }

    fn generate_question(&self, number: usize) -> EqTrainingQuestion {
        let previous_band = self.trials.last().map(|trial| trial.question.band_index);
        let mut band_index = (splitmix64(self.config.seed.wrapping_add(number as u64)) as usize)
            % self.band_frequencies.len();
        if previous_band == Some(band_index) && self.band_frequencies.len() > 1 {
            band_index = (band_index + 1) % self.band_frequencies.len();
        }
        let change = match self.config.change_mode {
            EqChangeMode::Boost => EqChange::Boost,
            EqChangeMode::Cut => EqChange::Cut,
            EqChangeMode::Mixed => {
                if splitmix64(self.config.seed ^ (number as u64).wrapping_mul(0x9e37)) & 1 == 0 {
                    EqChange::Boost
                } else {
                    EqChange::Cut
                }
            }
        };
        let gain_db = if self.config.exercise == EqTrainingExercise::GainIdentification {
            GAIN_CHOICES_DB[(splitmix64(self.config.seed ^ (number as u64).wrapping_mul(0x517c))
                as usize)
                % GAIN_CHOICES_DB.len()]
        } else {
            self.config.gain_db
        };
        EqTrainingQuestion {
            number,
            band_index,
            center_frequency_hz: self.band_frequencies[band_index],
            change,
            gain_db,
            q: self.config.q,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EarTrainingSessionSummary {
    pub completed_at_unix_secs: u64,
    pub course: Option<EarTrainingCourse>,
    pub exercise: EqTrainingExercise,
    pub accuracy: f64,
    pub correct: usize,
    pub attempts: usize,
    pub band_frequencies: Vec<f64>,
    pub band_stats: Vec<(usize, usize)>,
}

impl EarTrainingSessionSummary {
    pub fn from_session(session: &EqTrainingSession, course: Option<EarTrainingCourse>) -> Self {
        let completed_at_unix_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            completed_at_unix_secs,
            course,
            exercise: session.config.exercise,
            accuracy: session.accuracy(),
            correct: session.correct_count(),
            attempts: session.trials.len(),
            band_frequencies: session.band_frequencies.clone(),
            band_stats: session
                .band_stats()
                .into_iter()
                .map(|stats| (stats.attempts, stats.correct))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EarTrainingProgress {
    #[serde(default)]
    pub sessions: Vec<EarTrainingSessionSummary>,
}

impl EarTrainingProgress {
    pub fn record(&mut self, session: &EqTrainingSession, course: Option<EarTrainingCourse>) {
        self.sessions
            .push(EarTrainingSessionSummary::from_session(session, course));
    }

    pub fn total_attempts(&self) -> usize {
        self.sessions.iter().map(|session| session.attempts).sum()
    }

    pub fn accuracy(&self) -> f64 {
        let attempts = self.total_attempts();
        if attempts == 0 {
            0.0
        } else {
            self.sessions
                .iter()
                .map(|session| session.correct)
                .sum::<usize>() as f64
                / attempts as f64
        }
    }

    pub fn streak(&self) -> usize {
        self.sessions
            .iter()
            .rev()
            .take_while(|session| session.accuracy >= 0.7)
            .count()
    }

    pub fn weakest_frequency_hz(&self) -> Option<f64> {
        let mut aggregate: Vec<(f64, usize, usize)> = Vec::new();
        for session in &self.sessions {
            for (index, &(attempts, correct)) in session.band_stats.iter().enumerate() {
                let Some(&frequency) = session.band_frequencies.get(index) else {
                    continue;
                };
                if let Some(entry) = aggregate
                    .iter_mut()
                    .find(|entry| (entry.0 - frequency).abs() < 0.5)
                {
                    entry.1 += attempts;
                    entry.2 += correct;
                } else {
                    aggregate.push((frequency, attempts, correct));
                }
            }
        }
        aggregate
            .into_iter()
            .filter(|(_, attempts, _)| *attempts > 0)
            .min_by(|left, right| {
                (left.2 as f64 / left.1 as f64).total_cmp(&(right.2 as f64 / right.1 as f64))
            })
            .map(|entry| entry.0)
    }

    pub fn recommendation(&self) -> String {
        if self.sessions.is_empty() {
            return "Start with Foundations at 12 dB.".into();
        }
        self.weakest_frequency_hz().map_or_else(
            || "Try a boost/cut identification session.".into(),
            |frequency| format!("Focus around {frequency:.0} Hz, then reduce gain by 3 dB."),
        )
    }

    pub fn adaptive_config(&self) -> EqTrainingConfig {
        let mut config = EqTrainingConfig::default();
        if self.accuracy() >= 0.85 {
            config.band_count = 15;
            config.gain_db = 3.0;
        } else if self.accuracy() >= 0.7 {
            config.band_count = 10;
            config.gain_db = 6.0;
        } else {
            config.band_count = 5;
            config.gain_db = 9.0;
        }
        config
    }

    pub fn load(path: &Path) -> std::io::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = std::fs::read(path)?;
        serde_json::from_slice(&bytes)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    }

    pub fn save_atomic(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        std::fs::write(&temporary, bytes)?;
        std::fs::rename(temporary, path)
    }
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqTrainingError {
    InvalidBandCount,
    InvalidFrequencyRange,
    InvalidGain,
    InvalidQ,
    InvalidTrialCount,
    SessionAlreadyStarted,
    NoActiveQuestion,
    InvalidAnswer,
    QuestionAlreadyAnswered,
    AnswerRequired,
}

impl std::fmt::Display for EqTrainingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidBandCount => "band count must be between 2 and 25",
            Self::InvalidFrequencyRange => "frequency range must stay within 20 Hz–20 kHz",
            Self::InvalidGain => "gain must be between 1 and 15 dB",
            Self::InvalidQ => "Q must be between 0.2 and 10",
            Self::InvalidTrialCount => "trial count must be between 1 and 500",
            Self::SessionAlreadyStarted => "session has already started",
            Self::NoActiveQuestion => "no EQ training question is active",
            Self::InvalidAnswer => "selected band is outside the session range",
            Self::QuestionAlreadyAnswered => "the current question has already been answered",
            Self::AnswerRequired => "answer the current question before advancing",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for EqTrainingError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_builds_logarithmic_bands() {
        let bands = EqTrainingConfig::default().band_frequencies().unwrap();
        assert_eq!(bands.len(), 10);
        let first_ratio = bands[1] / bands[0];
        for pair in bands.windows(2) {
            assert!((pair[1] / pair[0] - first_ratio).abs() < 1.0e-9);
        }
        assert!((bands[0] - 31.5).abs() < 1.0e-9);
        assert!((bands[9] - 16_000.0).abs() < 1.0e-6);
    }

    #[test]
    fn seeded_sessions_are_reproducible_and_avoid_adjacent_repeats() {
        fn questions() -> Vec<(usize, EqChange)> {
            let mut session = EqTrainingSession::new(EqTrainingConfig {
                trial_count: 12,
                ..Default::default()
            })
            .unwrap();
            session.start().unwrap();
            let mut questions = Vec::new();
            loop {
                let question = session.current_question.clone().unwrap();
                questions.push((question.band_index, question.change));
                session.submit_answer(0).unwrap();
                if session.advance().unwrap().is_none() {
                    break;
                }
            }
            questions
        }

        let first = questions();
        assert_eq!(first, questions());
        assert!(first.windows(2).all(|pair| pair[0].0 != pair[1].0));
    }

    #[test]
    fn scoring_tracks_accuracy_and_weakest_band() {
        let mut session = EqTrainingSession::new(EqTrainingConfig {
            trial_count: 2,
            ..Default::default()
        })
        .unwrap();
        session.start().unwrap();
        let first = session.current_question.as_ref().unwrap().band_index;
        session.submit_answer(first).unwrap();
        session.advance().unwrap();
        let second = session.current_question.as_ref().unwrap().band_index;
        session
            .submit_answer((second + 1) % session.band_frequencies.len())
            .unwrap();
        assert_eq!(session.correct_count(), 1);
        assert_eq!(session.accuracy(), 0.5);
        assert_eq!(session.weakest_band(), Some(second));
        assert!(session.advance().unwrap().is_none());
        assert!(session.is_complete());
    }

    #[test]
    fn answer_must_be_submitted_once_before_advance() {
        let mut session = EqTrainingSession::new(EqTrainingConfig::default()).unwrap();
        session.start().unwrap();
        assert_eq!(session.advance(), Err(EqTrainingError::AnswerRequired));
        session.submit_answer(0).unwrap();
        assert_eq!(
            session.submit_answer(0),
            Err(EqTrainingError::QuestionAlreadyAnswered)
        );
    }

    #[test]
    fn question_produces_peak_eq_parameters() {
        let mut session = EqTrainingSession::new(EqTrainingConfig {
            change_mode: EqChangeMode::Cut,
            ..Default::default()
        })
        .unwrap();
        let question = session.start().unwrap();
        let filter = question.filter();
        assert_eq!(filter.filter_type, BiquadFilterType::Peak);
        assert_eq!(filter.gain_db, -6.0);
        assert_eq!(question.plugin_parameters()["filters"][0]["gain_db"], -6.0);
        assert_eq!(question.preview_curve(64).len(), 64);
    }

    #[test]
    fn in_progress_session_round_trips_without_losing_progress() {
        let mut session = EqTrainingSession::new(EqTrainingConfig::default()).unwrap();
        session.start().unwrap();
        session.submit_answer(0).unwrap();
        let json = serde_json::to_string(&session).unwrap();
        let restored: EqTrainingSession = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.config, session.config);
        assert_eq!(restored.trials, session.trials);
        assert_eq!(restored.current_question, session.current_question);
        assert!(
            restored
                .band_frequencies
                .iter()
                .zip(&session.band_frequencies)
                .all(|(left, right)| (left - right).abs() < 1.0e-9)
        );
        assert!(restored.current_is_answered());
    }

    #[test]
    fn all_exercises_have_stable_answer_contracts() {
        for exercise in EqTrainingExercise::ALL {
            let mut session = EqTrainingSession::new(EqTrainingConfig {
                exercise,
                trial_count: 1,
                ..Default::default()
            })
            .unwrap();
            let question = session.start().unwrap().clone();
            let labels = question.answer_labels(exercise, &session.band_frequencies);
            let answer = question.correct_answer(exercise);
            assert!(answer < labels.len());
            assert!(session.submit_answer(answer).unwrap().correct);
        }
    }

    #[test]
    fn courses_progress_and_adaptation_form_a_learning_path() {
        assert_eq!(EarTrainingCourse::Foundations.config().band_count, 3);
        assert_eq!(EarTrainingCourse::Mastery.config().gain_db, 3.0);
        let mut session = EqTrainingSession::new(EqTrainingConfig {
            trial_count: 1,
            ..Default::default()
        })
        .unwrap();
        let answer = session.start().unwrap().band_index;
        session.submit_answer(answer).unwrap();
        session.advance().unwrap();
        let mut progress = EarTrainingProgress::default();
        progress.record(&session, Some(EarTrainingCourse::Foundations));
        assert_eq!(progress.total_attempts(), 1);
        assert_eq!(progress.streak(), 1);
        assert_eq!(progress.adaptive_config().gain_db, 3.0);
        assert!(progress.recommendation().contains("Hz"));
    }

    #[test]
    fn progress_persistence_is_atomic_and_missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("progress.json");
        assert_eq!(
            EarTrainingProgress::load(&path).unwrap(),
            EarTrainingProgress::default()
        );
        let progress = EarTrainingProgress::default();
        progress.save_atomic(&path).unwrap();
        assert_eq!(EarTrainingProgress::load(&path).unwrap(), progress);
        assert!(!path.with_extension("json.tmp").exists());
    }
}
