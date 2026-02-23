//! Core types for the TUI application state management
use sotf_audio_player::{Album, Track};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
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

/// Step in the Spinorama EQ wizard
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

/// Optimization status for spinorama TUI
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SpinoramaOptStatus {
    #[default]
    Idle,
    Running,
    Completed,
    Failed(String),
}

/// A single PEQ filter result
#[derive(Debug, Clone)]
pub struct SpinoramaFilter {
    pub filter_type: String,
    pub freq: f64,
    pub q: f64,
    pub gain_db: f64,
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
    // Step 2: configuration — Filters
    pub num_filters: usize,
    pub min_freq: f64,
    pub max_freq: f64,
    pub min_db: f64,
    pub max_db: f64,
    pub min_q: f64,
    pub max_q: f64,
    pub peq_model: String,
    // Optimization
    pub algorithm: String,
    pub max_iter: usize,
    pub population: usize,
    pub strategy: String,
    pub de_f: f64,
    pub de_cr: f64,
    // Refinement
    pub refine: bool,
    pub local_algo: String,
    // Smoothing
    pub smooth: bool,
    pub smooth_n: usize,
    pub psychoacoustic: bool,
    // Constraints
    pub spacing_weight: f64,
    pub min_spacing_oct: f64,
    pub asymmetric_loss: bool,
    // Convergence
    pub tolerance: f64,
    pub atolerance: f64,
    pub sample_rate: u32,
    pub selected_field: usize, // which config field is selected
    // Step 3: optimization progress
    pub opt_status: SpinoramaOptStatus,
    pub opt_progress: f32,
    pub opt_loss: f64,
    pub opt_iteration: usize,
    pub opt_max_iter: usize,
    // Step 4: results
    pub filters: Vec<SpinoramaFilter>,
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
        Self {
            step: SpinoramaStep::Select,
            search_query: String::new(),
            available_speakers: Vec::new(),
            filtered_speakers: Vec::new(),
            selected_speaker_idx: 0,
            selected_speaker: None,
            loading_speakers: false,
            speakers_error: None,
            num_filters: 5,
            min_freq: 60.0,
            max_freq: 16000.0,
            min_db: -12.0,
            max_db: 4.0,
            min_q: 0.5,
            max_q: 6.0,
            peq_model: "pk".to_string(),
            algorithm: "de".to_string(),
            max_iter: 10000,
            population: 50,
            strategy: "currenttobest1bin".to_string(),
            de_f: 0.8,
            de_cr: 0.9,
            refine: false,
            local_algo: "cobyla".to_string(),
            smooth: true,
            smooth_n: 1,
            psychoacoustic: true,
            spacing_weight: 20.0,
            min_spacing_oct: 0.5,
            asymmetric_loss: true,
            tolerance: 1e-3,
            atolerance: 1e-4,
            sample_rate: 48000,
            selected_field: 0,
            opt_status: SpinoramaOptStatus::Idle,
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
    Grid,   // Editing matrix cells
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

/// ReplayGain application mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayGainMode {
    Track,
    Album,
}

/// Channel group for level meter display
#[derive(Debug, Clone)]
pub struct ChannelGroup {
    pub name: String,
    pub channels: Vec<ChannelInfo>,
    pub muted: bool,
    pub soloed: bool,
    pub dimmed: bool,
}

/// Individual channel information
#[derive(Debug, Clone)]
pub struct ChannelInfo {
    pub index: usize,              // Index in loudness.channel_peaks
    pub name: String,              // e.g., "FL", "FR", "C"
    pub display_name: Vec<String>, // Multi-line display: ["F", "L"] or ["T", "B", "R"]
}

/// Pending parameter update for zero-dropout updates
#[derive(Debug, Clone)]
pub struct PendingParameterUpdate {
    pub plugin_index: usize,
    pub param_id: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub album: Album,
    pub current_track_index: usize,
}

impl QueueItem {
    pub fn new(album: Album) -> Self {
        Self {
            album,
            current_track_index: 0,
        }
    }

    pub fn current_track(&self) -> Option<&Track> {
        self.album.tracks.get(self.current_track_index)
    }

    pub fn next_track(&mut self) -> Option<&Track> {
        if self.current_track_index + 1 < self.album.tracks.len() {
            self.current_track_index += 1;
            self.current_track()
        } else {
            None
        }
    }

    pub fn previous_track(&mut self) -> Option<&Track> {
        if self.current_track_index > 0 {
            self.current_track_index -= 1;
            self.current_track()
        } else {
            None
        }
    }
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
