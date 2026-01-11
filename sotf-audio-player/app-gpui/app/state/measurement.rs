//! Measurement and EQ workflow state management.
//!
//! Contains state for all measurement and EQ optimization workflows:
//! - Room EQ measurements and optimization
//! - Headphone EQ optimization
//! - Spinorama/speaker EQ optimization
//! - General measurement state

use crate::app::types::{
    HeadphoneEqState, MeasureState, RecordingState, RoomEqState, SpinoramaEqState,
};

/// Unified state for all measurement and EQ workflows
#[derive(Debug, Default)]
pub struct MeasurementState {
    /// Generic measurement state (e.g., signal analysis)
    pub measure_state: Option<MeasureState>,

    /// Recording workflow state (capture, evaluate, save)
    pub recording_state: RecordingState,

    /// Room EQ measurement and optimization workflow
    pub room_eq_state: RoomEqState,
    /// Applied room EQ plugins (ready to be sent to audio engine)
    pub room_eq_applied_plugins: Option<Vec<sotf_audio::PluginConfig>>,

    /// Headphone EQ optimization workflow
    pub headphone_eq_state: HeadphoneEqState,

    /// Spinorama/speaker EQ optimization workflow
    pub spinorama_eq_state: SpinoramaEqState,
}

impl MeasurementState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset all measurement workflows to their initial state
    pub fn reset_all(&mut self) {
        self.measure_state = None;
        self.recording_state = RecordingState::default();
        self.room_eq_state = RoomEqState::default();
        self.room_eq_applied_plugins = None;
        self.headphone_eq_state = HeadphoneEqState::default();
        self.spinorama_eq_state = SpinoramaEqState::default();
    }

    /// Check if a generic measurement is in progress
    pub fn has_active_measurement(&self) -> bool {
        self.measure_state.is_some()
    }

    /// Check if room EQ has been applied
    pub fn has_room_eq_applied(&self) -> bool {
        self.room_eq_applied_plugins.is_some()
    }

    /// Clear applied room EQ plugins
    pub fn clear_room_eq_plugins(&mut self) {
        self.room_eq_applied_plugins = None;
    }
}
