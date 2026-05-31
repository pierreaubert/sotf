// ============================================================================
// Project Persistence — Save/load DAW sessions as .sotf project files
// ============================================================================

mod bridge;
#[allow(
    clippy::module_inception,
    reason = "project::project is the existing persisted-project implementation module"
)]
mod project;

pub use project::{
    AutomationConfig, MidiRegionConfig, MidiTrackConfig, Project, RegionConfig, TrackConfig,
    TrackType,
};
