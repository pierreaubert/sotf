// ============================================================================
// Project Persistence — Save/load DAW sessions as .sotf project files
// ============================================================================

mod bridge;
mod project;

pub use project::{
    AutomationConfig, MidiRegionConfig, MidiTrackConfig, Project, RegionConfig, TrackConfig,
    TrackType,
};
