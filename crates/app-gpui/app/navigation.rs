//! Navigation and selection methods.
//!
//! Contains methods for navigating and selecting items in various lists.

use super::state::App;
use super::types::{HeadphoneEqStep, RecordingStep, RoomEqStep, Screen, SpinoramaStep};

impl App {
    /// Step selectors share the same validation as footer and keyboard actions.
    /// Unvisited future steps cannot skip configuration or launch side effects.
    pub fn select_workflow_step(&mut self, target: usize) -> bool {
        let (current, count) = match self.ui_state.current_screen {
            Screen::Recording => (
                RecordingStep::all()
                    .iter()
                    .position(|step| *step == self.measurement_state.recording_state.step)
                    .unwrap_or(0),
                RecordingStep::all().len(),
            ),
            Screen::RoomEq => (
                self.measurement_state.room_eq_state.step.index(),
                RoomEqStep::all().len(),
            ),
            Screen::HeadphoneEq => (
                match self.measurement_state.headphone_eq_state.step {
                    HeadphoneEqStep::MeasurementTarget => 0,
                    HeadphoneEqStep::Optimization => 1,
                    HeadphoneEqStep::Listen => 2,
                    HeadphoneEqStep::Export => 3,
                },
                4,
            ),
            Screen::Spinorama => (self.measurement_state.spinorama_eq_state.step.index(), 4),
            _ => return false,
        };
        if target >= count || target > current + 1 || !self.can_rewind_workflow_step() {
            return false;
        }
        if target > current {
            return self.move_workflow_step(true);
        }
        for _ in target..current {
            if !self.move_workflow_step(false) {
                return false;
            }
        }
        true
    }

    /// Whether the current domain wizard can advance through its primary action.
    ///
    /// Desktop buttons, phone controls, and keyboard actions all use this gate so
    /// a shortcut cannot bypass the validation shown by the wizard UI.
    pub fn can_advance_workflow_step(&self) -> bool {
        match self.ui_state.current_screen {
            Screen::Recording => {
                let recording = &self.measurement_state.recording_state;
                if recording.workflow_is_busy() {
                    return false;
                }
                match recording.step {
                    RecordingStep::Config => recording.recording_directory.is_some(),
                    RecordingStep::SplCalibration => true,
                    RecordingStep::Capture => {
                        recording.all_channels_recorded() && !recording.workflow_is_busy()
                    }
                    RecordingStep::Evaluating => recording.all_takes_accepted(),
                    RecordingStep::Probe | RecordingStep::BassAnchor | RecordingStep::Saving => {
                        true
                    }
                }
            }
            Screen::RoomEq => {
                let room_eq = &self.measurement_state.room_eq_state;
                !room_eq.is_optimizing()
                    && match room_eq.step {
                        RoomEqStep::LoadData => room_eq.has_measurements(),
                        RoomEqStep::Delay | RoomEqStep::Process => true,
                        RoomEqStep::Configure => !room_eq.speaker_configs.is_empty(),
                        RoomEqStep::Optimize => room_eq.is_optimization_complete(),
                        RoomEqStep::Review | RoomEqStep::Export => true,
                    }
            }
            Screen::HeadphoneEq => {
                let headphone_eq = &self.measurement_state.headphone_eq_state;
                !headphone_eq.is_optimizing() && headphone_eq.can_advance()
            }
            Screen::Spinorama => {
                let spinorama = &self.measurement_state.spinorama_eq_state;
                !spinorama.is_optimizing() && spinorama.can_advance()
            }
            _ => false,
        }
    }

    fn can_rewind_workflow_step(&self) -> bool {
        match self.ui_state.current_screen {
            Screen::Recording => !self.measurement_state.recording_state.workflow_is_busy(),
            Screen::RoomEq => !self.measurement_state.room_eq_state.is_optimizing(),
            Screen::HeadphoneEq => !self.measurement_state.headphone_eq_state.is_optimizing(),
            Screen::Spinorama => !self.measurement_state.spinorama_eq_state.is_optimizing(),
            _ => false,
        }
    }

    /// Move the active domain wizard backward or forward.
    ///
    /// Returns `true` when navigation was accepted. Advancing preserves each
    /// workflow's validation and side effects, including Recording channel
    /// initialization when leaving Config.
    pub fn move_workflow_step(&mut self, forward: bool) -> bool {
        if (forward && !self.can_advance_workflow_step())
            || (!forward && !self.can_rewind_workflow_step())
        {
            return false;
        }

        match self.ui_state.current_screen {
            Screen::Recording => {
                let recording = &mut self.measurement_state.recording_state;
                let step = recording.step;
                if forward {
                    if step == RecordingStep::Config {
                        recording.init_channel_recordings();
                    }
                    if step == RecordingStep::Saving {
                        self.ui_state.current_screen = self.ui_state.navigation.last_screen;
                    } else if let Some(next) = step.next() {
                        recording.step = next;
                    }
                } else if step == RecordingStep::Config {
                    self.ui_state.current_screen = self.ui_state.navigation.last_screen;
                } else if let Some(previous) = step.previous() {
                    recording.step = previous;
                }
            }
            Screen::RoomEq => {
                let room_eq = &mut self.measurement_state.room_eq_state;
                let step = room_eq.step;
                if forward {
                    if step == RoomEqStep::Export {
                        self.ui_state.current_screen = self.ui_state.navigation.last_screen;
                    } else if let Some(next) = step.next() {
                        room_eq.step = next;
                    }
                } else if step == RoomEqStep::LoadData {
                    self.ui_state.current_screen = self.ui_state.navigation.last_screen;
                } else if let Some(previous) = step.previous() {
                    room_eq.step = previous;
                }
            }
            Screen::HeadphoneEq => {
                let headphone_eq = &mut self.measurement_state.headphone_eq_state;
                let step = headphone_eq.step;
                if forward {
                    if step == HeadphoneEqStep::Export {
                        self.ui_state.current_screen = self.ui_state.navigation.last_screen;
                    } else if let Some(next) = step.next() {
                        headphone_eq.model.step = next;
                    }
                } else if step == HeadphoneEqStep::MeasurementTarget {
                    self.ui_state.current_screen = self.ui_state.navigation.last_screen;
                } else if let Some(previous) = step.previous() {
                    headphone_eq.model.step = previous;
                }
            }
            Screen::Spinorama => {
                let spinorama = &mut self.measurement_state.spinorama_eq_state;
                let step = spinorama.step;
                if forward {
                    if step == SpinoramaStep::Export {
                        self.ui_state.current_screen = self.ui_state.navigation.last_screen;
                    } else if let Some(next) = step.next() {
                        spinorama.step = next;
                    }
                } else if step == SpinoramaStep::SelectSpeaker {
                    self.ui_state.current_screen = self.ui_state.navigation.last_screen;
                } else if let Some(previous) = step.previous() {
                    spinorama.step = previous;
                }
            }
            _ => return false,
        }

        if let Err(error) = self.synchronize_headphone_audition() {
            self.ui_state.toast_message = Some(super::types::ToastMessage::error(error));
        }
        true
    }

    pub fn select_next_album(&mut self) {
        let albums = self.filtered_albums();
        if !albums.is_empty() {
            self.library_state.selected_index =
                (self.library_state.selected_index + 1) % albums.len();
        }
    }

    pub fn select_previous_album(&mut self) {
        let albums = self.filtered_albums();
        if !albums.is_empty() {
            if self.library_state.selected_index == 0 {
                self.library_state.selected_index = albums.len() - 1;
            } else {
                self.library_state.selected_index -= 1;
            }
        }
    }

    pub fn page_up_albums(&mut self, page_size: usize) {
        let current_page_albums = self.get_paginated_albums();
        if current_page_albums.is_empty() {
            return;
        }

        // Move selection up by page size
        if self.library_state.selected_index >= page_size {
            self.library_state.selected_index -= page_size;
        } else {
            // Move to first item
            self.library_state.selected_index = 0;
        }
    }

    pub fn select_next_queue_item(&mut self) {
        if !self.queue_state.is_empty() {
            self.queue_state.selected_index =
                (self.queue_state.selected_index + 1) % self.queue_state.len();
        }
    }

    pub fn select_previous_queue_item(&mut self) {
        if !self.queue_state.is_empty() {
            if self.queue_state.selected_index == 0 {
                self.queue_state.selected_index = self.queue_state.len() - 1;
            } else {
                self.queue_state.selected_index -= 1;
            }
        }
    }

    pub fn page_down_queue(&mut self, page_size: usize) {
        if !self.queue_state.is_empty() {
            self.queue_state.selected_index =
                (self.queue_state.selected_index + page_size).min(self.queue_state.len() - 1);
        }
    }

    pub fn page_up_queue(&mut self, page_size: usize) {
        if !self.queue_state.is_empty() {
            self.queue_state.selected_index =
                self.queue_state.selected_index.saturating_sub(page_size);
        }
    }

    pub fn select_next_directory(&mut self) {
        let tree_items = self.get_directory_tree_items();
        if !tree_items.is_empty() {
            self.library_view.selected_directory_index =
                (self.library_view.selected_directory_index + 1) % tree_items.len();
        }
    }

    pub fn select_previous_directory(&mut self) {
        let tree_items = self.get_directory_tree_items();
        if !tree_items.is_empty() {
            if self.library_view.selected_directory_index == 0 {
                self.library_view.selected_directory_index = tree_items.len() - 1;
            } else {
                self.library_view.selected_directory_index -= 1;
            }
        }
    }

    pub fn page_down_directories(&mut self, page_size: usize) {
        let tree_items = self.get_directory_tree_items();
        if !tree_items.is_empty() {
            self.library_view.selected_directory_index =
                (self.library_view.selected_directory_index + page_size).min(tree_items.len() - 1);
        }
    }

    pub fn page_up_directories(&mut self, page_size: usize) {
        let tree_items = self.get_directory_tree_items();
        if !tree_items.is_empty() {
            self.library_view.selected_directory_index = self
                .library_view
                .selected_directory_index
                .saturating_sub(page_size);
        }
    }
}
