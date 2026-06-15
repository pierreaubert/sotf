use sotf_audio_player::ui_models::recording::RecordingScreenModel;
use std::collections::HashMap;

/// TUI state for the Recording wizard. Holds only TUI-specific view state;
/// all domain state lives in the embedded [`RecordingScreenModel`].
// Note: not `Clone` because `save_receiver` (mpsc::Receiver) is not
// `Clone`. The state is owned by `App` and accessed via `&mut`; no
// caller needs to clone the wizard wholesale.
#[derive(Debug)]
pub struct RecordingTuiState {
    /// Shared, UI-agnostic Recording wizard domain model.
    pub model: RecordingScreenModel,

    /// When true, the wizard step tab bar has focus (Left/Right change step).
    pub step_tab_focused: bool,

    // Step 1: config (TUI UI-only fields)
    pub available_playback_devices: Vec<(String, String)>, // (id, name)
    pub available_recording_devices: Vec<(String, String)>,
    pub selected_playback_idx: usize,
    pub selected_recording_idx: usize,
    /// TUI edit buffer for the output/base directory. The canonical value
    /// lives in `model.recording_base_directory`.
    pub output_directory: String,
    pub editing_output_dir: bool,
    /// `Some(ch)` while editing channel `ch`'s mic-calibration path.
    pub editing_mic_cal_channel: Option<usize>,
    pub selected_field: usize,
    /// True when a numerical field is being directly edited via keyboard.
    pub editing_value: bool,
    pub edit_buffer: String,

    // Step 3 (Probe): tone-burst delay probe capture.
    pub probe_selected_field: usize,
    pub probe_editing_value: bool,

    // SPL Calibration step (GD-Opt v2 Phase GD-1e.5).
    pub spl_selected_field: usize,
    pub spl_editing_value: bool,

    // Step 4: evaluate (TUI UI-only fields)
    pub selected_channel_view: usize,

    // Step 4: save (TUI UI-only fields)
    pub editing_save_name: bool,
    /// Cursor within the save-step form. 0 = save_name, 1..=3 = room
    /// width/depth/height, 4 = unit toggle, 5 = setup description,
    /// 6..6+N-1 = per-channel speaker entries (N = channel count).
    pub selected_save_field: usize,
    /// When true, a text/number field under the save cursor is being typed.
    pub editing_save_value: bool,
    pub save_error: Option<String>,
    pub save_success: bool,
    /// True while a background save thread is serializing and writing the JSON.
    pub save_in_progress: bool,
    /// Receiver for the background save result.
    pub save_receiver: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
}

impl Default for RecordingTuiState {
    fn default() -> Self {
        Self {
            model: RecordingScreenModel {
                signal_level_db: -20.0,
                save_name: String::new(),
                ..Default::default()
            },
            step_tab_focused: false,
            available_playback_devices: Vec::new(),
            available_recording_devices: Vec::new(),
            selected_playback_idx: 0,
            selected_recording_idx: 0,
            output_directory: String::new(),
            editing_output_dir: false,
            editing_mic_cal_channel: None,
            selected_field: 0,
            editing_value: false,
            edit_buffer: String::new(),
            probe_selected_field: 0,
            probe_editing_value: false,
            spl_selected_field: 0,
            spl_editing_value: false,
            selected_channel_view: 0,
            editing_save_name: false,
            selected_save_field: 0,
            editing_save_value: false,
            save_error: None,
            save_success: false,
            save_in_progress: false,
            save_receiver: None,
        }
    }
}

impl RecordingTuiState {
    /// Currently-active mic-calibration path string (read-only).
    pub fn active_mic_cal_path(&self) -> &str {
        self.model
            .active_mic_cal_path(self.editing_mic_cal_channel)
    }

    /// Mutable reference to the currently-active mic-calibration string.
    pub fn active_mic_cal_path_mut(&mut self) -> Option<&mut String> {
        self.model
            .active_mic_cal_path_mut(self.editing_mic_cal_channel)
    }

    /// Replace the active mic-calibration path.
    pub fn set_active_mic_cal_path(&mut self, val: String) {
        self.model
            .set_active_mic_cal_path(self.editing_mic_cal_channel, val);
    }

    /// Resize the mic-calibration and recording-channel-mapping vecs to
    /// match `num_channels`.
    pub fn sync_recording_channel_vecs(&mut self) {
        self.model.sync_recording_channel_vecs();
    }

    /// Ensure `channel_speakers` has one slot per physical playback channel.
    pub fn sync_channel_speakers_length(&mut self) {
        self.model.sync_channel_speakers_length();
    }

    /// Build the canonical-metric `RoomDimensions` to persist.
    pub fn room_dimensions_for_save(&self) -> Option<autoeq::roomeq::RoomDimensions> {
        self.model.room_dimensions_for_save()
    }

    /// Build the channel-name → "brand model" map persisted in
    /// `RecordingConfiguration`.
    pub fn channel_speakers_map_for_save(&self) -> Option<HashMap<String, String>> {
        self.model.channel_speakers_map_for_save()
    }

    /// How many fields are in the Save-step form given the current channel list.
    pub fn save_field_count(&self) -> usize {
        self.model.save_field_count()
    }
}
