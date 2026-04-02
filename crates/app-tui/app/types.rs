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
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Loading,
    Library,
    Queue,
    Playlists,
    Plugins,
    Devices,
    Configure,
}

/// Sub-mode within the Playlists screen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaylistMode {
    /// Browsing the list of playlists
    List,
    /// Browsing tracks within the open playlist
    Tracks,
    /// Text input for creating a new playlist
    Create,
    /// Text input for renaming
    Rename,
    /// Confirmation prompt before deleting
    ConfirmDelete,
}

/// Sub-screens within the Configure section
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigureSubScreen {
    Directories,
    Recording,
    RoomEq,
    HeadphoneEq,
    SpinoramaEq,
    FederationSources,
    Servers,
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
            SpinoramaStep::Select => "Select",
            SpinoramaStep::Configure => "Configure",
            SpinoramaStep::Optimize => "Optimize",
            SpinoramaStep::Results => "Results",
            SpinoramaStep::UpdatePlugin => "Update Plugin",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpinUpdateSubStep {
    #[default]
    Ready,
    ConfirmOverwrite,
}

/// TUI state for the Spinorama EQ wizard
#[derive(Debug, Clone)]
pub struct SpinoramaEqTuiState {
    pub step: SpinoramaStep,
    /// When true, the wizard step tab bar has focus (Left/Right change step).
    pub step_tab_focused: bool,
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
    /// True when a numerical field is being directly edited via keyboard
    pub editing_value: bool,
    pub edit_buffer: String,
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
    // Optimization loss history: (iteration, loss, optional score)
    pub loss_history: Vec<(usize, f64, Option<f64>)>,
    // Step 5: update plugin confirmation
    pub update_substep: SpinUpdateSubStep,
    /// (slot_index, filter_count) of existing EQ to overwrite
    pub update_existing_eq_info: Option<(usize, usize)>,
}

impl Default for SpinoramaEqTuiState {
    fn default() -> Self {
        // TUI uses slightly different defaults than GPUI
        let config = SpinoramaOptimizerConfig {
            population: 50,
            smooth: true,
            smooth_n: 1,
            spacing_weight: 20.0,
            min_spacing_oct: 0.5,
            tolerance: 1e-3,
            atolerance: 1e-4,
            ..SpinoramaOptimizerConfig::default()
        };
        Self {
            step: SpinoramaStep::Select,
            step_tab_focused: false,
            search_query: String::new(),
            available_speakers: Vec::new(),
            filtered_speakers: Vec::new(),
            selected_speaker_idx: 0,
            selected_speaker: None,
            loading_speakers: false,
            speakers_error: None,
            config,
            selected_field: 0,
            editing_value: false,
            edit_buffer: String::new(),
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
            update_substep: SpinUpdateSubStep::Ready,
            update_existing_eq_info: None,
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

/// Step in the Headphone EQ wizard (TUI-specific: 5 steps)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeadphoneEqStep {
    #[default]
    SelectFile,
    Configure,
    Optimize,
    Results,
    UpdatePlugin,
}

impl HeadphoneEqStep {
    pub fn label(self) -> &'static str {
        match self {
            HeadphoneEqStep::SelectFile => "File",
            HeadphoneEqStep::Configure => "Configure",
            HeadphoneEqStep::Optimize => "Optimize",
            HeadphoneEqStep::Results => "Results",
            HeadphoneEqStep::UpdatePlugin => "Update Plugin",
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
    /// When true, the wizard step tab bar has focus (Left/Right change step).
    pub step_tab_focused: bool,
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
    /// True when a numerical field is being directly edited via keyboard
    pub editing_value: bool,
    pub edit_buffer: String,
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
    // Step 5: update plugin confirmation
    pub update_substep: SpinUpdateSubStep,
    /// (slot_index, filter_count) of existing EQ to overwrite
    pub update_existing_eq_info: Option<(usize, usize)>,
}

impl Default for HeadphoneEqTuiState {
    fn default() -> Self {
        Self {
            step: HeadphoneEqStep::SelectFile,
            step_tab_focused: false,
            measurement_path: String::new(),
            target_preset: "harman-over-ear-2018".to_string(),
            custom_target_path: String::new(),
            editing_measurement: false,
            editing_custom_target: false,
            selected_field: 0,
            config: HeadphoneEqOptimizerConfig::default(),
            config_selected_field: 0,
            editing_value: false,
            edit_buffer: String::new(),
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
            update_substep: SpinUpdateSubStep::Ready,
            update_existing_eq_info: None,
        }
    }
}

/// TUI state for the Room EQ wizard
#[derive(Debug, Clone)]
pub struct RoomEqTuiState {
    pub step: RoomEqStep,
    /// When true, focus is on the step tabs row; Left/Right/Tab cycle steps.
    /// When false, focus is inside the current step's content.
    pub step_tab_focused: bool,
    // Step 1: load measurement file (JSON)
    pub file_path: String,
    pub editing_file_path: bool,
    pub channel_measurements: Vec<ChannelMeasurement>,
    pub load_error: Option<String>,
    // Step 2: configure (shared config struct)
    pub config: RoomEqOptimizerConfig,
    pub selected_field: usize,
    pub selected_section: usize,
    /// True when a numerical field is being directly edited via keyboard
    pub editing_value: bool,
    pub edit_buffer: String,
    // Step 3: optimization
    pub opt_status: OptimizationStatus,
    pub opt_error: Option<String>,
    pub opt_progress: f32,
    pub opt_iteration: usize,
    pub opt_max_iter: usize,
    pub opt_loss: f64,
    pub channel_results: Vec<ChannelOptResult>,
    pub loss_history: Vec<(usize, f64)>,
    /// Log buffer for optimization messages (max 300 lines)
    pub opt_log_lines: VecDeque<String>,
    /// Scroll offset from bottom (0 = bottom)
    pub opt_log_scroll: usize,
    // Step 4: review
    pub selected_channel: usize,
    // Step 5: export
    pub export_path: String,
    pub editing_export_path: bool,
    pub export_format: usize,
    pub export_error: Option<String>,
    pub export_success: bool,
}

impl Default for RoomEqTuiState {
    fn default() -> Self {
        Self {
            step: RoomEqStep::LoadData,
            step_tab_focused: false,
            file_path: String::new(),
            editing_file_path: false,
            channel_measurements: Vec::new(),
            load_error: None,
            config: RoomEqOptimizerConfig::default(),
            selected_field: 0,
            selected_section: 0,
            editing_value: false,
            edit_buffer: String::new(),
            opt_status: OptimizationStatus::Idle,
            opt_error: None,
            opt_progress: 0.0,
            opt_iteration: 0,
            opt_max_iter: 0,
            opt_loss: 0.0,
            channel_results: Vec::new(),
            loss_history: Vec::new(),
            opt_log_lines: VecDeque::new(),
            opt_log_scroll: 0,
            selected_channel: 0,
            export_path: String::new(),
            editing_export_path: false,
            export_format: 0,
            export_error: None,
            export_success: false,
        }
    }
}

impl RoomEqTuiState {
    /// Compute the average slope for L and R channels in dB/octave.
    pub fn compute_lr_slope(&self) -> Option<(f64, f64, f64)> {
        sotf_audio_player::room_eq_types::compute_lr_slope(&self.channel_measurements)
    }
}

/// TUI state for the Recording wizard
#[derive(Debug, Clone)]
pub struct RecordingTuiState {
    pub step: RecordingStep,
    /// When true, the wizard step tab bar has focus (Left/Right change step).
    pub step_tab_focused: bool,
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
    /// True when a numerical field is being directly edited via keyboard
    pub editing_value: bool,
    pub edit_buffer: String,
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
    pub selected_save_field: usize,
    pub save_error: Option<String>,
    pub save_success: bool,
}

impl Default for RecordingTuiState {
    fn default() -> Self {
        Self {
            step: RecordingStep::Config,
            step_tab_focused: false,
            playback_config: PlaybackDeviceConfig::default(),
            recording_config: RecordingDeviceConfig::default(),
            available_playback_devices: Vec::new(),
            available_recording_devices: Vec::new(),
            selected_playback_idx: 0,
            selected_recording_idx: 0,
            mic_calibration_path: String::new(),
            signal_type: RecordingSignalType::Sweep,
            signal_duration_secs: 5.0,
            signal_level_db: -20.0,
            sweep_start_freq: 20.0,
            sweep_end_freq: 20000.0,
            output_directory: String::new(),
            editing_output_dir: false,
            editing_mic_cal: false,
            selected_field: 0,
            editing_value: false,
            edit_buffer: String::new(),
            channel_recordings: Vec::new(),
            current_channel: None,
            recording_progress: 0.0,
            auto_record: false,
            status_message: String::new(),
            selected_channel_view: 0,
            save_name: String::new(),
            editing_save_name: false,
            selected_save_field: 0,
            save_error: None,
            save_success: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    AddPlugin,
    EditPlugin,
    SavePlugins,
    LoadPlugins,
    LoadApoFile,
    LoadSofaFile,
    FileExplorer,
    ShowHelp,
    ShowError,
    /// Shown when a multichannel file conflicts with the upmixer plugin
    ChannelConflict,
    /// Level meters pane is focused
    LevelMeters,
    /// Configure tab bar is focused
    Configure,
    /// Configure sub-screen: Directories
    ConfigureDirectories,
    /// Configure sub-screen: Recording
    ConfigureRecording,
    /// Configure sub-screen: Room EQ
    ConfigureRoomEq,
    /// Configure sub-screen: Headphone EQ
    ConfigureHeadphoneEq,
    /// Configure sub-screen: Spinorama EQ
    ConfigureSpinoramaEq,
    /// Configure sub-screen: Federation Sources
    ConfigureFederationSources,
    /// Configure sub-screen: Servers
    ConfigureServers,
}

impl InputMode {
    /// Returns true for Configure tab bar and all 5 sub-screens
    pub fn is_configure(self) -> bool {
        matches!(
            self,
            InputMode::Configure
                | InputMode::ConfigureDirectories
                | InputMode::ConfigureRecording
                | InputMode::ConfigureRoomEq
                | InputMode::ConfigureHeadphoneEq
                | InputMode::ConfigureSpinoramaEq
                | InputMode::ConfigureFederationSources
                | InputMode::ConfigureServers
        )
    }

    /// Returns true for configure sub-screens only (not the tab bar)
    pub fn is_configure_sub_screen(self) -> bool {
        matches!(
            self,
            InputMode::ConfigureDirectories
                | InputMode::ConfigureRecording
                | InputMode::ConfigureRoomEq
                | InputMode::ConfigureHeadphoneEq
                | InputMode::ConfigureSpinoramaEq
                | InputMode::ConfigureFederationSources
                | InputMode::ConfigureServers
        )
    }

    /// Convert a ConfigureSubScreen to the corresponding InputMode
    pub fn from_configure_sub_screen(sub: ConfigureSubScreen) -> Self {
        match sub {
            ConfigureSubScreen::Directories => InputMode::ConfigureDirectories,
            ConfigureSubScreen::Recording => InputMode::ConfigureRecording,
            ConfigureSubScreen::RoomEq => InputMode::ConfigureRoomEq,
            ConfigureSubScreen::HeadphoneEq => InputMode::ConfigureHeadphoneEq,
            ConfigureSubScreen::SpinoramaEq => InputMode::ConfigureSpinoramaEq,
            ConfigureSubScreen::FederationSources => InputMode::ConfigureFederationSources,
            ConfigureSubScreen::Servers => InputMode::ConfigureServers,
        }
    }

    /// Return the corresponding ConfigureSubScreen, if this is a configure sub-screen mode
    pub fn configure_sub_screen(self) -> Option<ConfigureSubScreen> {
        match self {
            InputMode::ConfigureDirectories => Some(ConfigureSubScreen::Directories),
            InputMode::ConfigureRecording => Some(ConfigureSubScreen::Recording),
            InputMode::ConfigureRoomEq => Some(ConfigureSubScreen::RoomEq),
            InputMode::ConfigureHeadphoneEq => Some(ConfigureSubScreen::HeadphoneEq),
            InputMode::ConfigureSpinoramaEq => Some(ConfigureSubScreen::SpinoramaEq),
            InputMode::ConfigureFederationSources => Some(ConfigureSubScreen::FederationSources),
            InputMode::ConfigureServers => Some(ConfigureSubScreen::Servers),
            _ => None,
        }
    }
}

/// Whether the file picker selects a file or a directory
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerMode {
    File,
    Directory,
}

/// Tracks which feature opened the file explorer so we can apply the result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerOrigin {
    SofaFile,
    IrFile,
    RecordingOutputDir,
    RecordingMicCalibration,
    RoomEqFilePath,
    RoomEqExportPath,
    HeadphoneMeasurement,
    HeadphoneCustomTarget,
    AddDirectory,
    ApoFile,
    ABConfigA,
    ABConfigB,
    PlaylistImport,
    PlaylistExport,
}

/// Options presented in the channel conflict dialog
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelConflictChoice {
    /// Suspend incompatible plugins and play (auto-restores on next compatible track)
    SuspendIncompatible,
    /// Remove incompatible plugins from the chain permanently
    RemoveIncompatible,
    /// Cancel playback
    Cancel,
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

// ============================================================================
// Federation & Server TUI state
// ============================================================================

use sotf_audio_player::federation_config::{ConnectionStatus, FederationSourceEntry, ServerConfig};
use std::collections::HashMap;

/// TUI state for the Federation Sources configuration screen.
#[derive(Debug, Clone)]
pub struct FederationTuiState {
    pub sources: Vec<FederationSourceEntry>,
    pub statuses: HashMap<String, ConnectionStatus>,
    pub selected_idx: usize,
    pub mode: FederationMode,
    pub edit: Option<FederationEditState>,
}

impl Default for FederationTuiState {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            statuses: HashMap::new(),
            selected_idx: 0,
            mode: FederationMode::List,
            edit: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FederationMode {
    List,
    EditSource,
    AddSource,
}

#[derive(Debug, Clone)]
pub struct FederationEditState {
    pub source: FederationSourceEntry,
    /// Field index within the source-specific connection fields
    /// 0..N are connection fields, N is display_name, N+1 is priority, N+2 is enabled
    pub selected_field: usize,
    pub editing_value: bool,
    pub edit_buffer: String,
    pub is_new: bool,
}

impl FederationEditState {
    pub fn new(source: FederationSourceEntry, is_new: bool) -> Self {
        Self {
            source,
            selected_field: 0,
            editing_value: false,
            edit_buffer: String::new(),
            is_new,
        }
    }

    /// Total number of editable fields (connection fields + name + priority)
    pub fn field_count(&self) -> usize {
        self.source.connection.field_names().len() + 2
    }

    /// Get label for the field at the given index
    pub fn field_label(&self, index: usize) -> &str {
        let conn_fields = self.source.connection.field_names();
        if index < conn_fields.len() {
            conn_fields[index]
        } else if index == conn_fields.len() {
            "Display Name"
        } else {
            "Priority"
        }
    }

    /// Get value for the field at the given index
    pub fn field_value(&self, index: usize) -> String {
        let conn_fields = self.source.connection.field_names();
        if index < conn_fields.len() {
            self.source.connection.field_value(index)
        } else if index == conn_fields.len() {
            self.source.display_name.clone()
        } else {
            self.source.priority.to_string()
        }
    }

    /// Set value for the field at the given index
    pub fn set_field_value(&mut self, index: usize, value: &str) {
        let conn_field_count = self.source.connection.field_names().len();
        if index < conn_field_count {
            self.source.connection.set_field_value(index, value);
        } else if index == conn_field_count {
            self.source.display_name = value.to_string();
        } else if let Ok(p) = value.parse() {
            self.source.priority = p;
        }
    }
}

/// TUI state for the Servers configuration screen.
#[derive(Debug, Clone)]
pub struct ServersTuiState {
    pub config: ServerConfig,
    pub selected_section: ServerSection,
    pub selected_field: usize,
    pub editing_value: bool,
    pub edit_buffer: String,
    pub tls_fingerprint: Option<String>,
}

impl Default for ServersTuiState {
    fn default() -> Self {
        Self {
            config: ServerConfig::default(),
            selected_section: ServerSection::Mpd,
            selected_field: 0,
            editing_value: false,
            edit_buffer: String::new(),
            tls_fingerprint: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerSection {
    Mpd,
    Dlna,
}

/// Source type names for the "Add Source" selection.
pub const SOURCE_TYPE_NAMES: &[(&str, &str)] = &[
    ("subsonic", "Subsonic"),
    ("mpd", "MPD"),
    ("dlna", "DLNA"),
    ("peer", "Peer (SotF)"),
    ("tidal", "Tidal"),
    ("spotify", "Spotify"),
    ("icy_radio", "Radio"),
];

/// Index of the selected source type when in AddSource mode
pub static ADD_SOURCE_TYPE_IDX: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
