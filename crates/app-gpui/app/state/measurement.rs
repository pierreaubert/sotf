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
    #[cfg(not(target_os = "ios"))]
    pub multi_capture: MultiCaptureState,
    pub step_menu_open: bool,
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
        #[cfg(not(target_os = "ios"))]
        {
            self.multi_capture = MultiCaptureState::default();
        }
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

    /// Poll capture independently of the currently visible screen.
    pub(crate) fn poll_capture(&mut self) -> bool {
        #[cfg(not(target_os = "ios"))]
        {
            self.multi_capture.workflow.poll()
        }
        #[cfg(target_os = "ios")]
        {
            false
        }
    }

    /// Capture controls own keyboard input while their panel is open.
    pub(crate) fn capture_panel_open(&self) -> bool {
        #[cfg(not(target_os = "ios"))]
        {
            self.multi_capture.active
        }
        #[cfg(target_os = "ios")]
        {
            false
        }
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

#[cfg(not(target_os = "ios"))]
#[derive(Debug, Clone, Copy)]
pub enum CapturePath {
    Plan,
    Raw,
    Processed,
}

#[cfg(not(target_os = "ios"))]
#[derive(Debug, Clone, Copy)]
pub enum CaptureAction {
    LoadPlan,
    Record,
    Cancel,
    Process,
    Import,
}

/// GPUI input buffers and the shared background capture owner.
#[cfg(not(target_os = "ios"))]
#[derive(Debug, Default)]
pub struct MultiCaptureState {
    pub active: bool,
    pub plan_path: String,
    pub raw_directory: String,
    pub processed_directory: String,
    pub workflow: sotf_audio_player::ui_models::capture::CaptureWorkflow,
}

#[cfg(not(target_os = "ios"))]
impl MultiCaptureState {
    /// Opens the panel only when the legacy recording workflow is idle.
    pub fn open(&mut self, legacy_busy: bool) -> bool {
        if legacy_busy {
            return false;
        }
        self.active = true;
        true
    }

    /// Leaves the panel only after its current operation has returned.
    pub fn close(&mut self) -> bool {
        if self.workflow.is_busy() {
            return false;
        }
        self.active = false;
        true
    }

    /// Updates a path and invalidates evidence tied to earlier inputs.
    ///
    /// # Errors
    /// Rejects edits during recording or background processing.
    pub fn edit_path(&mut self, field: CapturePath, value: String) -> Result<(), String> {
        let unchanged = match field {
            CapturePath::Plan => self.plan_path == value,
            CapturePath::Raw => self.raw_directory == value,
            CapturePath::Processed => self.processed_directory == value,
        };
        if unchanged {
            return Ok(());
        }
        self.workflow
            .inputs_changed(matches!(field, CapturePath::Plan))?;
        match field {
            CapturePath::Plan => self.plan_path = value,
            CapturePath::Raw => self.raw_directory = value,
            CapturePath::Processed => self.processed_directory = value,
        }
        Ok(())
    }

    /// Dispatches a user action, returning processed channels only for explicit import.
    ///
    /// # Errors
    /// Rejects missing paths, overlapping work, unvalidated plans, and unavailable imports.
    pub fn execute(
        &mut self,
        action: CaptureAction,
    ) -> Result<Option<Vec<sotf_audio_player::room_eq_types::ChannelMeasurement>>, String> {
        use std::path::PathBuf;
        match action {
            CaptureAction::LoadPlan => {
                if self.plan_path.trim().is_empty() {
                    return Err("Choose a session plan JSON file".into());
                }
                self.workflow.load_plan(PathBuf::from(&self.plan_path))?;
            }
            CaptureAction::Record => {
                if self.raw_directory.trim().is_empty() {
                    return Err("Choose a new raw output directory".into());
                }
                self.workflow.record(PathBuf::from(&self.raw_directory))?;
            }
            CaptureAction::Cancel => self.workflow.cancel_recording(),
            CaptureAction::Process => {
                if self.raw_directory.trim().is_empty()
                    || self.processed_directory.trim().is_empty()
                {
                    return Err(
                        "Choose the saved raw directory and a new processed output directory"
                            .into(),
                    );
                }
                self.workflow.process(
                    PathBuf::from(&self.raw_directory),
                    PathBuf::from(&self.processed_directory),
                )?;
            }
            CaptureAction::Import => {
                return self
                    .workflow
                    .import_channels()
                    .map(Some)
                    .ok_or_else(|| "No complete processed RoomEQ manifest is available".into());
            }
        }
        Ok(None)
    }
}
