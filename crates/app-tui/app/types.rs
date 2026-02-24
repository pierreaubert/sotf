//! Core types for the TUI application state management
pub use sotf_audio_player::QueueItem;
use sotf_audio_player::headphone_eq_types::{HeadphoneEqBiquad, HeadphoneEqOptimizerConfig};
use sotf_audio_player::recording_types::{
    ChannelRecording, PlaybackDeviceConfig, RecordingDeviceConfig, RecordingSignalType,
    RecordingStep,
};
use sotf_audio_player::room_eq_types::{
    ChannelMeasurement, ChannelOptResult, OptimizationStatus, RoomEqOptimizerConfig, RoomEqStep,
};
use sotf_audio_player::spinorama_eq_types::{SpinoramaBiquad, SpinoramaOptimizerConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Loading,
    Library,
    Queue,
    Plugins,
    Devices,
    Configure,
}

/// Sub-screens within the Configure section
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigureSubScreen {
    Directories,
    Recording,
    RoomEq,
    HeadphoneEq,
    SpinoramaEq,
}

/// Step in the Spinorama EQ wizard (TUI-specific: 5 steps)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpinoramaStep {
    #[default]
    Select,
    Configure,
    Optimize,
    Results,
    UpdatePlugin,
}

impl SpinoramaStep {
    pub fn label(self) -> &'static str {
        match self {
            SpinoramaStep::Select => "1:Select",
            SpinoramaStep::Configure => "2:Configure",
            SpinoramaStep::Optimize => "3:Optimize",
            SpinoramaStep::Results => "4:Results",
            SpinoramaStep::UpdatePlugin => "5:Update Plugin",
        }
    }
}

/// TUI state for the Spinorama EQ wizard
#[derive(Debug, Clone)]
pub struct SpinoramaEqTuiState {
    pub step: SpinoramaStep,
    // Step 1: speaker selection
    pub search_query: String,
    pub available_speakers: Vec<String>,
    pub filtered_speakers: Vec<String>,
    pub selected_speaker_idx: usize,
    pub selected_speaker: Option<String>,
    pub loading_speakers: bool,
    pub speakers_error: Option<String>,
    // Step 2: configuration (shared config struct)
    pub config: SpinoramaOptimizerConfig,
    pub selected_field: usize, // which config field is selected
    // Step 3: optimization progress
    pub opt_status: OptimizationStatus,
    pub opt_error: Option<String>,
    pub opt_progress: f32,
    pub opt_loss: f64,
    pub opt_iteration: usize,
    pub opt_max_iter: usize,
    // Step 4: results
    pub filters: Vec<SpinoramaBiquad>,
    pub pre_loss: f64,
    pub post_loss: f64,
    // Frequency response curves (log-spaced Hz, dB values)
    pub curve_frequencies: Vec<f64>,
    pub curve_input: Vec<f64>,
    pub curve_target: Vec<f64>,
    pub curve_corrected: Vec<f64>,
    pub curve_filter_response: Vec<f64>,
    // Optimization loss history: (iteration, loss)
    pub loss_history: Vec<(usize, f64)>,
}

impl Default for SpinoramaEqTuiState {
    fn default() -> Self {
        // TUI uses slightly different defaults than GPUI
        let mut config = SpinoramaOptimizerConfig::default();
        config.population = 50;
        config.smooth = true;
        config.smooth_n = 1;
        config.spacing_weight = 20.0;
        config.min_spacing_oct = 0.5;
        config.tolerance = 1e-3;
        config.atolerance = 1e-4;
        Self {
            step: SpinoramaStep::Select,
            search_query: String::new(),
            available_speakers: Vec::new(),
            filtered_speakers: Vec::new(),
            selected_speaker_idx: 0,
            selected_speaker: None,
            loading_speakers: false,
            speakers_error: None,
            config,
            selected_field: 0,
            opt_status: OptimizationStatus::Idle,
            opt_error: None,
            opt_progress: 0.0,
            opt_loss: 0.0,
            opt_iteration: 0,
            opt_max_iter: 0,
            filters: Vec::new(),
            pre_loss: 0.0,
            post_loss: 0.0,
            curve_frequencies: Vec::new(),
            curve_input: Vec::new(),
            curve_target: Vec::new(),
            curve_corrected: Vec::new(),
            curve_filter_response: Vec::new(),
            loss_history: Vec::new(),
        }
    }
}

impl SpinoramaEqTuiState {
    pub fn update_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_speakers = self.available_speakers.clone();
        } else {
            let q = self.search_query.to_lowercase();
            self.filtered_speakers = self
                .available_speakers
                .iter()
                .filter(|s| s.to_lowercase().contains(&q))
                .cloned()
                .collect();
        }
        self.selected_speaker_idx = 0;
    }
}

/// Step in the Headphone EQ wizard (TUI-specific: 4 steps)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeadphoneEqStep {
    #[default]
    SelectFile,
    Configure,
    Optimize,
    Results,
}

impl HeadphoneEqStep {
    pub fn label(self) -> &'static str {
        match self {
            HeadphoneEqStep::SelectFile => "1:File",
            HeadphoneEqStep::Configure => "2:Configure",
            HeadphoneEqStep::Optimize => "3:Optimize",
            HeadphoneEqStep::Results => "4:Results",
        }
    }
}

/// Available headphone target curve presets
pub const HEADPHONE_TARGET_PRESETS: &[&str] = &[
    "harman-over-ear-2018",
    "harman-over-ear-2015",
    "harman-over-ear-2013",
    "harman-in-ear-2019",
    "custom",
];

/// TUI state for the Headphone EQ wizard
#[derive(Debug, Clone)]
pub struct HeadphoneEqTuiState {
    pub step: HeadphoneEqStep,
    // Step 1: file selection
    pub measurement_path: String,
    pub target_preset: String,
    pub custom_target_path: String,
    pub editing_measurement: bool,
    pub editing_custom_target: bool,
    pub selected_field: usize,
    // Step 2: configuration (shared config struct)
    pub config: HeadphoneEqOptimizerConfig,
    pub config_selected_field: usize,
    // Step 3: optimization progress
    pub opt_status: OptimizationStatus,
    pub opt_error: Option<String>,
    pub opt_progress: f32,
    pub opt_loss: f64,
    pub opt_iteration: usize,
    pub opt_max_iter: usize,
    // Step 4: results
    pub filters: Vec<HeadphoneEqBiquad>,
    pub pre_loss: f64,
    pub post_loss: f64,
    pub curve_frequencies: Vec<f64>,
    pub curve_input: Vec<f64>,
    pub curve_target: Vec<f64>,
    pub curve_corrected: Vec<f64>,
    pub curve_filter_response: Vec<f64>,
    pub loss_history: Vec<(usize, f64)>,
}

impl Default for HeadphoneEqTuiState {
    fn default() -> Self {
        Self {
            step: HeadphoneEqStep::SelectFile,
            measurement_path: String::new(),
            target_preset: "harman-over-ear-2018".to_string(),
            custom_target_path: String::new(),
            editing_measurement: false,
            editing_custom_target: false,
            selected_field: 0,
            config: HeadphoneEqOptimizerConfig::default(),
            config_selected_field: 0,
            opt_status: OptimizationStatus::Idle,
            opt_error: None,
            opt_progress: 0.0,
            opt_loss: 0.0,
            opt_iteration: 0,
            opt_max_iter: 0,
            filters: Vec::new(),
            pre_loss: 0.0,
            post_loss: 0.0,
            curve_frequencies: Vec::new(),
            curve_input: Vec::new(),
            curve_target: Vec::new(),
            curve_corrected: Vec::new(),
            curve_filter_response: Vec::new(),
            loss_history: Vec::new(),
        }
    }
}

/// TUI state for the Room EQ wizard
#[derive(Debug, Clone)]
pub struct RoomEqTuiState {
    pub step: RoomEqStep,
    // Step 1: load measurement file (JSON)
    pub file_path: String,
    pub editing_file_path: bool,
    pub channel_measurements: Vec<ChannelMeasurement>,
    pub load_error: Option<String>,
    // Step 2: configure (shared config struct)
    pub config: RoomEqOptimizerConfig,
    pub selected_field: usize,
    pub selected_section: usize,
    // Step 3: optimization
    pub opt_status: OptimizationStatus,
    pub opt_error: Option<String>,
    pub opt_progress: f32,
    pub opt_iteration: usize,
    pub opt_max_iter: usize,
    pub opt_loss: f64,
    pub channel_results: Vec<ChannelOptResult>,
    pub loss_history: Vec<(usize, f64)>,
    // Step 4: review
    pub selected_channel: usize,
    // Step 5: export
    pub export_path: String,
    pub editing_export_path: bool,
    pub export_error: Option<String>,
    pub export_success: bool,
}

impl Default for RoomEqTuiState {
    fn default() -> Self {
        Self {
            step: RoomEqStep::LoadData,
            file_path: String::new(),
            editing_file_path: false,
            channel_measurements: Vec::new(),
            load_error: None,
            config: RoomEqOptimizerConfig::default(),
            selected_field: 0,
            selected_section: 0,
            opt_status: OptimizationStatus::Idle,
            opt_error: None,
            opt_progress: 0.0,
            opt_iteration: 0,
            opt_max_iter: 0,
            opt_loss: 0.0,
            channel_results: Vec::new(),
            loss_history: Vec::new(),
            selected_channel: 0,
            export_path: String::new(),
            editing_export_path: false,
            export_error: None,
            export_success: false,
        }
    }
}

/// TUI state for the Recording wizard
#[derive(Debug, Clone)]
pub struct RecordingTuiState {
    pub step: RecordingStep,
    // Step 1: config
    pub playback_config: PlaybackDeviceConfig,
    pub recording_config: RecordingDeviceConfig,
    pub available_playback_devices: Vec<(String, String)>, // (id, name)
    pub available_recording_devices: Vec<(String, String)>,
    pub selected_playback_idx: usize,
    pub selected_recording_idx: usize,
    pub mic_calibration_path: String,
    pub signal_type: RecordingSignalType,
    pub signal_duration_secs: f32,
    pub signal_level_db: f32,
    pub sweep_start_freq: f32,
    pub sweep_end_freq: f32,
    pub output_directory: String,
    pub editing_output_dir: bool,
    pub editing_mic_cal: bool,
    pub selected_field: usize,
    // Step 2: capture
    pub channel_recordings: Vec<ChannelRecording>,
    pub current_channel: Option<usize>,
    pub recording_progress: f32,
    pub auto_record: bool,
    pub status_message: String,
    // Step 3: evaluate
    pub selected_channel_view: usize,
    // Step 4: save
    pub save_name: String,
    pub editing_save_name: bool,
    pub save_error: Option<String>,
    pub save_success: bool,
}

impl Default for RecordingTuiState {
    fn default() -> Self {
        Self {
            step: RecordingStep::Config,
            playback_config: PlaybackDeviceConfig::default(),
            recording_config: RecordingDeviceConfig::default(),
            available_playback_devices: Vec::new(),
            available_recording_devices: Vec::new(),
            selected_playback_idx: 0,
            selected_recording_idx: 0,
            mic_calibration_path: String::new(),
            signal_type: RecordingSignalType::Sweep,
            signal_duration_secs: 5.0,
            signal_level_db: -12.0,
            sweep_start_freq: 20.0,
            sweep_end_freq: 20000.0,
            output_directory: String::new(),
            editing_output_dir: false,
            editing_mic_cal: false,
            selected_field: 0,
            channel_recordings: Vec::new(),
            current_channel: None,
            recording_progress: 0.0,
            auto_record: false,
            status_message: String::new(),
            selected_channel_view: 0,
            save_name: String::new(),
            editing_save_name: false,
            save_error: None,
            save_success: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    AddDirectory,
    AddPlugin,
    EditPlugin,
    SavePlugins,
    LoadPlugins,
    LoadApoFile,
    LoadSofaFile,
    BrowseSofaFile,
    BrowseIrFile,
    ShowHelp,
    ShowError,
    /// Shown when a multichannel file conflicts with the upmixer plugin
    ChannelConflict,
}

/// Options presented in the channel conflict dialog
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelConflictChoice {
    /// Disable the upmixer and play with native channels
    DisableUpmixer,
    /// Remove the upmixer from the chain entirely
    RemoveUpmixer,
    /// Cancel playback
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedPane {
    Main,   // Main content area (library, queue, etc.)
    Meters, // Right column with level meters
}

/// Matrix plugin editor mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatrixEditMode {
    #[default]
    Header, // Editing input/output channels, preset
    Grid, // Editing matrix cells
}

/// Tree view mode for library
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryViewMode {
    Flat,     // Original list view
    TreeView, // Hierarchical artist → albums
}

/// Library sort order
pub use sotf_audio_player::library::LibrarySortOrder;

/// Channel filter options
pub use sotf_audio_player::library::ChannelFilter;

/// Artist node in tree view
#[derive(Debug, Clone)]
pub struct ArtistNode {
    pub artist: String,
    pub album_indices: Vec<usize>, // Indices into library.albums
    pub expanded: bool,
}

/// Tree item type for rendering
#[derive(Debug, Clone)]
pub enum TreeItem {
    Artist { name: String, expanded: bool },
    Album { index: usize },
}

pub use sotf_audio_player::ReplayGainMode;

pub use sotf_audio_player::{ChannelGroup, ChannelInfo};

/// Pending parameter update for zero-dropout updates
#[derive(Debug, Clone)]
pub struct PendingParameterUpdate {
    pub plugin_index: usize,
    pub param_id: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct QueueEntry {
    pub item: QueueItem,
    pub expanded: bool,
}

impl QueueEntry {
    pub fn new(item: QueueItem) -> Self {
        Self {
            item,
            expanded: false,
        }
    }
}
