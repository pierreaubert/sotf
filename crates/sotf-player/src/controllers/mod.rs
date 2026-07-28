//! Domain controllers that encapsulate shared business logic.
//!
//! These controllers own state and orchestrate operations so that UIs
//! (GPUI, TUI, etc.) become thin wrappers that delegate business operations
//! and only manage UI-specific state.

pub mod ab_compare_path;
pub mod ab_test_controller;
pub mod ab_test_execution;
pub mod ab_test_session;
pub mod library;
pub mod playback;
pub mod playlist;
pub mod plugin;
pub mod plugin_param_map;
pub mod queue;
pub mod scan;

pub use ab_test_controller::{AbTestController, AbTestPhase, AbTestView};
pub use ab_test_execution::{
    LevelMatchPreparation, LevelMatchPreparationRequest, load_ab_test_session, media_file_identity,
    prepare_level_match, save_ab_test_session, verify_media_segment,
};
pub use ab_test_session::{
    AbTestError, AbTestSession, ChainSnapshot, LevelMatchConfig, LevelMatchMeasurement,
    LevelMatchMetric, ListeningTestSetup, MediaSegment, PathSelection, TrialAnswer, TrialCue,
    TrialMode, TrialRecord, TrialResult, measure_level_match,
};
pub use library::LibraryController;
pub use playback::PlaybackController;
pub use playlist::PlaylistController;
pub use plugin::{PluginController, PluginUpdateEffect, get_param_count};
pub use plugin_param_map::param_index_to_engine_param;
pub use queue::{QueueController, QueuePlaybackEffect};
pub use scan::ScanController;
