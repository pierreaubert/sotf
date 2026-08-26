use super::default::default_fake_channels;
use super::default::default_fake_points;
use super::default::default_room_eq_max_iter;
use super::default::default_room_eq_num_filters;
use super::default::default_room_eq_population;
use super::default::default_scenario_timeout;
use super::default::default_true;
use super::runner_config::RunnerConfig;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub(super) struct SuiteFile {
    #[serde(default)]
    pub(super) runner: RunnerConfig,
    #[serde(default, alias = "scenario")]
    pub(super) scenarios: Vec<ScenarioConfig>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ScenarioConfig {
    pub(super) name: String,
    pub(super) path: PathBuf,
    #[serde(default = "default_scenario_timeout")]
    pub(super) timeout: String,
    #[serde(default)]
    pub(super) tags: Vec<String>,
    #[serde(default)]
    pub(super) seed_demo_audio: bool,
    #[serde(default)]
    pub(super) require_virtual_audio: bool,
    /// Explicit substrings permitted to match the clean-log gate. Keep this
    /// narrow and scenario-specific so genuine regressions remain visible.
    #[serde(default)]
    pub(super) allowed_log_patterns: Vec<String>,
    #[serde(default)]
    pub(super) fake_recording: Option<FakeRecordingConfig>,
    #[serde(default)]
    pub(super) room_eq: Option<RoomEqConfig>,
    #[serde(default)]
    pub(super) headphone_discovery: Option<HeadphoneDiscoveryConfig>,
    #[serde(default)]
    pub(super) spinorama_discovery: Option<SpinoramaDiscoveryConfig>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FakeRecordingConfig {
    #[serde(default = "default_fake_channels")]
    pub(super) channels: usize,
    #[serde(default = "default_fake_points")]
    pub(super) points: usize,
    /// Optional one-shot deterministic error triggered by the first visible
    /// Capture action. Valid values are device-loss, clipping, and io-failure.
    pub(super) fault: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RoomEqConfig {
    pub(super) fixture_dir: PathBuf,
    #[serde(default)]
    pub(super) dist_path: Option<PathBuf>,
    pub(super) target: String,
    pub(super) loss: String,
    pub(super) processing: String,
    pub(super) crossover: String,
    #[serde(default = "default_room_eq_num_filters")]
    pub(super) num_filters: usize,
    #[serde(default = "default_room_eq_max_iter")]
    pub(super) max_iter: usize,
    #[serde(default = "default_room_eq_population")]
    pub(super) population: usize,
    #[serde(default = "default_true")]
    pub(super) start: bool,
    /// Arrange a recording fixture, then require the RoomEQ UI to load it.
    #[serde(default)]
    pub(super) ui_driven: bool,
    /// Optional intentionally invalid recording fixture used to verify the
    /// visible diagnosis and repair path. Currently: missing-channel.
    #[serde(default)]
    pub(super) invalid: Option<String>,
}

/// Hermetic responses for the visible Headphone EQ discovery workflow.
#[derive(Debug, Deserialize)]
pub(super) struct HeadphoneDiscoveryConfig {
    pub(super) catalog: Vec<String>,
    #[serde(default)]
    pub(super) downloads: Vec<HeadphoneDownloadConfig>,
}

#[derive(Debug, Deserialize)]
pub(super) struct HeadphoneDownloadConfig {
    pub(super) headphone: String,
    pub(super) path: String,
    pub(super) points: Vec<[f64; 2]>,
    #[serde(default)]
    pub(super) delay_ms: u64,
    #[serde(default)]
    pub(super) failures: usize,
    #[serde(default)]
    pub(super) failure_message: Option<String>,
}

/// Hermetic speaker discovery responses for the Spinorama workflow.
#[derive(Debug, Deserialize)]
pub(super) struct SpinoramaDiscoveryConfig {
    pub(super) catalog: Vec<String>,
    pub(super) speakers: Vec<SpinoramaSpeakerConfig>,
    #[serde(default)]
    pub(super) catalog_delay_ms: u64,
    #[serde(default)]
    pub(super) catalog_failures: usize,
    #[serde(default)]
    pub(super) catalog_failure_message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SpinoramaSpeakerConfig {
    pub(super) speaker: String,
    pub(super) versions: Vec<SpinoramaVersionConfig>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SpinoramaVersionConfig {
    pub(super) version: String,
    pub(super) measurements: Vec<String>,
    /// Optional local [Hz, dB] response used by the real offline optimizer.
    #[serde(default)]
    pub(super) response: Vec<[f64; 2]>,
}

pub(super) enum ScenarioOutcome {
    Passed,
    Skipped(String),
}
