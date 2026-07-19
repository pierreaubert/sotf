//! Type definitions for the GPUI audio player application.
//!
//! Contains enums and simple structs used throughout the application.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    HomeShelf,
    NowPlaying,
    Library,
    Streams,
    Queue,
    Playlists,
    Spectrum,
    Settings,
    SettingsDetail,
    StudioHub,
    EqCurve,
    Studio,
    Recording,
    RoomEq,
    HeadphoneEq,
    Spinorama,
    PluginGraph,
    ListeningTest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PhoneHomeShelf {
    #[default]
    RecentlyPlayed,
    MostPlayed,
    Favorites,
    NewInLibrary,
}

impl PhoneHomeShelf {
    pub fn title(self) -> &'static str {
        match self {
            Self::RecentlyPlayed => "Recently Played",
            Self::MostPlayed => "Most Played",
            Self::Favorites => "Favorites",
            Self::NewInLibrary => "New in Library",
        }
    }
}

impl Screen {
    pub const fn all() -> &'static [Self] {
        &[
            Self::Home,
            Self::HomeShelf,
            Self::NowPlaying,
            Self::Library,
            Self::Streams,
            Self::Queue,
            Self::Playlists,
            Self::Spectrum,
            Self::Settings,
            Self::SettingsDetail,
            Self::StudioHub,
            Self::EqCurve,
            Self::Studio,
            Self::Recording,
            Self::RoomEq,
            Self::HeadphoneEq,
            Self::Spinorama,
            Self::PluginGraph,
            Self::ListeningTest,
        ]
    }

    pub fn primary_destinations() -> &'static [Self] {
        const DESTINATIONS: &[Screen] = &[
            Screen::Home,
            Screen::NowPlaying,
            Screen::Library,
            Screen::Streams,
            Screen::Queue,
            Screen::Studio,
        ];

        DESTINATIONS
    }

    pub fn primary_destination_index(self) -> usize {
        if let Some(index) = Self::primary_destinations()
            .iter()
            .position(|screen| *screen == self)
        {
            return index;
        }

        if self == Screen::HomeShelf {
            0
        } else if self == Screen::StudioHub || self == Screen::EqCurve || self.is_studio_tool() {
            Self::primary_destinations()
                .iter()
                .position(|screen| *screen == Screen::Studio)
                .unwrap_or(0)
        } else {
            0
        }
    }

    pub fn is_studio_tool(self) -> bool {
        matches!(
            self,
            Screen::PluginGraph
                | Screen::Recording
                | Screen::RoomEq
                | Screen::HeadphoneEq
                | Screen::Spinorama
                | Screen::ListeningTest
                | Screen::Spectrum
                | Screen::EqCurve
                | Screen::Studio
        )
    }

    pub fn from_view_menu_id(id: &str) -> Option<Self> {
        match id {
            "now-playing" => Some(Screen::NowPlaying),
            "home" => Some(Screen::Home),
            "library" => Some(Screen::Library),
            "streams" => Some(Screen::Streams),
            "queue" => Some(Screen::Queue),
            "studio" => Some(Screen::Studio),
            "plugingraph" => Some(Screen::PluginGraph),
            "recording" => Some(Screen::Recording),
            "roomeq" => Some(Screen::RoomEq),
            "headphoneeq" => Some(Screen::HeadphoneEq),
            "spinorama" => Some(Screen::Spinorama),
            "listening-test" => Some(Screen::ListeningTest),
            "settings" => Some(Screen::Settings),
            "settings-detail" => Some(Screen::SettingsDetail),
            _ => None,
        }
    }
}

pub use sotf_audio_player::ReplayGainMode;

/// Audio playback source mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PlaybackSource {
    /// Play from audio files (normal music player mode)
    #[default]
    File,
    /// Process audio from HAL virtual device (macOS only)
    /// This captures system-wide audio and processes it through the plugin chain
    #[cfg(all(target_os = "macos", feature = "hal"))]
    HalDevice,
}

// PluginViewMode is defined in state::plugin and re-exported from state::mod
pub use crate::app::state::plugin::PluginViewMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    /// Searchable overlay for dispatching registered application commands.
    CommandPalette,
    Search,
    AddDirectory,
    SavePlugins,
    LoadPlugins,
    LoadApoFile,
    LoadSofaFile,
    Help,
    HelpSupport,
    KeyboardShortcuts,
    About,
    EditingParam,
    SpinoramaSpeakerSearch,
    HeadphoneSearch,
    /// Modal shown on startup when library is empty
    EmptyLibraryPrompt,
    /// Modal for editing a plugin node in the graph view
    EditingPluginNode,
    /// Modal shown when track channels conflict with plugins in the chain
    ChannelConflict,
    /// Context menu is open (album, queue item, etc.)
    ContextMenu,
    /// Modal for manual album/track metadata editing.
    MetadataEditor,
    /// Tutorial dialog shown on first launch
    Tutorial,
    /// Contextual help guide for the current screen
    ScreenGuide,
}

impl InputMode {
    /// Check if this mode captures text input (blocking keyboard shortcuts).
    /// Use this to determine if actions should be blocked when in text entry modes.
    pub fn is_text_input(&self) -> bool {
        matches!(
            self,
            InputMode::CommandPalette
                | InputMode::Search
                | InputMode::AddDirectory
                | InputMode::SavePlugins
                | InputMode::LoadPlugins
                | InputMode::LoadApoFile
                | InputMode::LoadSofaFile
                | InputMode::SpinoramaSpeakerSearch
                | InputMode::HeadphoneSearch
                | InputMode::MetadataEditor
        )
    }
}

/// Active menu dropdown (if any)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveMenu {
    None,
    File,
    Show,
    Help,
    AddPlugin, // Plugin rack "Add" menu
}

/// Layout mode based on window height (legacy - kept for compatibility)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutMode {
    Compact, // Below 800px - tabs bar visible
    #[default]
    Expanded, // Above 800px - split Library/Queue view
}

/// Presentation style selected from platform and viewport geometry.
///
/// Desktop includes macOS, desktop Linux/Windows, and iPad-class iOS windows.
/// Phone is reserved for iPhone-class iOS/tvOS windows so narrow desktop
/// windows keep the established desktop compact layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlatformStyle {
    #[default]
    Desktop,
    Phone,
}

impl PlatformStyle {
    pub fn for_window(width: f32, height: f32, is_ios_family: bool) -> Self {
        if is_ios_family && crate::ui::is_phone_sized_window(width, height) {
            Self::Phone
        } else {
            Self::Desktop
        }
    }

    pub fn is_phone(self) -> bool {
        matches!(self, Self::Phone)
    }
}

/// Product density mode.
///
/// Standard keeps one primary destination visible at a time. Expert opts into
/// the dense multi-panel Library | Queue | Rack workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DensityMode {
    #[default]
    Standard,
    Expert,
}

impl DensityMode {
    pub fn all() -> &'static [Self] {
        const MODES: &[DensityMode] = &[DensityMode::Standard, DensityMode::Expert];

        MODES
    }

    pub fn label(self) -> &'static str {
        match self {
            DensityMode::Standard => "Standard",
            DensityMode::Expert => "Expert",
        }
    }

    pub fn value(self) -> &'static str {
        match self {
            DensityMode::Standard => "standard",
            DensityMode::Expert => "expert",
        }
    }

    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "standard" => Some(DensityMode::Standard),
            "expert" => Some(DensityMode::Expert),
            _ => None,
        }
    }

    pub fn layout_mode_for_window(self, width: f32, height: f32) -> LayoutMode {
        match self {
            DensityMode::Expert
                if !crate::ui::is_phone_sized_window(width, height)
                    && width >= 600.0
                    && height >= 500.0 =>
            {
                LayoutMode::Expanded
            }
            _ => LayoutMode::Compact,
        }
    }
}

/// Layout orientation based on window aspect ratio
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutOrientation {
    #[default]
    Horizontal, // width > height: panels side-by-side (Library | Queue | Rack)
    Vertical, // height >= width: panels stacked vertically
}

/// Rack display mode based on available space
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RackDisplayMode {
    #[default]
    Full, // Full rack with all controls
    Mini,      // Compact mode with output level meters only
    Collapsed, // Hidden
}

/// Meter display mode for Queue screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MeterDisplayMode {
    #[default]
    Lufs, // Show LUFS loudness meters
    Levels, // Show level meters
}

// Library enums (shared via player crate)
pub use sotf_audio_player::{ChannelFilter, LibrarySortOrder};

pub use sotf_audio_player::{ChannelGroup, ChannelInfo};

/// Context menu state
#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub menu_type: ContextMenuType,
    pub position_x: f32,
    pub position_y: f32,
    pub item_index: usize, // Index of the item that was right-clicked
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContextMenuType {
    Album,
    QueueItem,
    Plugin,
    Directory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataEditorScope {
    Album,
    Track,
}

#[derive(Debug, Clone, Default)]
pub struct MetadataEditorFields {
    pub title: String,
    pub artist: String,
    pub album_artist: String,
    pub year: String,
    pub genre: String,
    pub composer: String,
    pub disc_number: String,
    pub track_number: String,
    pub conductor: String,
    pub performer: String,
    pub isrc: String,
    pub ensemble: String,
    pub edition: String,
}

#[derive(Debug, Clone)]
pub struct MetadataEditorState {
    pub scope: MetadataEditorScope,
    pub target: sotf_audio_player::MetadataTarget,
    pub target_label: String,
    pub fields: MetadataEditorFields,
    pub preview: Option<sotf_audio_player::MetadataEditPreview>,
    pub error: Option<String>,
    pub search_query: String,
    pub search_results: Vec<sotf_audio_player::MetadataImportCandidate>,
    pub selected_result: usize,
    pub search_error: Option<String>,
    pub search_in_progress: bool,
}

impl MetadataEditorState {
    pub fn for_album(album: &sotf_audio_player::Album) -> Result<Self, String> {
        let album_id = album
            .id
            .ok_or_else(|| "Metadata editing requires a persisted album".to_string())?;
        let first = album.tracks.first();
        let artist = album.artist();
        let album_artist = first
            .and_then(|track| track.album_artist.clone())
            .unwrap_or_else(|| artist.clone());
        let title = album.title.clone();
        Ok(Self {
            scope: MetadataEditorScope::Album,
            target: sotf_audio_player::MetadataTarget::AlbumId(album_id),
            target_label: format!("Album \"{}\"", album.title),
            fields: MetadataEditorFields {
                title: title.clone(),
                artist,
                album_artist,
                year: album.year.map(|year| year.to_string()).unwrap_or_default(),
                genre: first
                    .and_then(|track| track.genre.clone())
                    .unwrap_or_default(),
                composer: first
                    .and_then(|track| track.composer.clone())
                    .unwrap_or_default(),
                disc_number: first
                    .and_then(|track| track.disc_number)
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                conductor: first
                    .and_then(|track| track.conductor.clone())
                    .unwrap_or_default(),
                performer: first
                    .and_then(|track| track.performer.clone())
                    .unwrap_or_default(),
                isrc: first
                    .and_then(|track| track.isrc.clone())
                    .unwrap_or_default(),
                ensemble: first
                    .and_then(|track| track.ensemble.clone())
                    .unwrap_or_default(),
                edition: album.edition.clone().unwrap_or_default(),
                ..Default::default()
            },
            preview: None,
            error: None,
            search_query: format!("{} {}", album.artist(), title).trim().to_string(),
            search_results: Vec::new(),
            selected_result: 0,
            search_error: None,
            search_in_progress: false,
        })
    }

    pub fn for_track(track: &sotf_audio_player::Track) -> Self {
        let title = track
            .title
            .clone()
            .unwrap_or_else(|| track.path.display().to_string());
        let artist = track.artist.clone().unwrap_or_default();
        Self {
            scope: MetadataEditorScope::Track,
            target: sotf_audio_player::MetadataTarget::TrackPath(track.path.clone()),
            target_label: format!("Track \"{}\"", title),
            fields: MetadataEditorFields {
                title: title.clone(),
                artist: artist.clone(),
                album_artist: track.album_artist.clone().unwrap_or_default(),
                year: String::new(),
                genre: track.genre.clone().unwrap_or_default(),
                composer: track.composer.clone().unwrap_or_default(),
                disc_number: track
                    .disc_number
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                track_number: track
                    .track_number
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                conductor: track.conductor.clone().unwrap_or_default(),
                performer: track.performer.clone().unwrap_or_default(),
                isrc: track.isrc.clone().unwrap_or_default(),
                ensemble: track.ensemble.clone().unwrap_or_default(),
                edition: track.edition.clone().unwrap_or_default(),
            },
            preview: None,
            error: None,
            search_query: format!("{} {}", artist, title).trim().to_string(),
            search_results: Vec::new(),
            selected_result: 0,
            search_error: None,
            search_in_progress: false,
        }
    }

    pub fn patch(&self) -> Result<sotf_audio_player::MetadataPatch, String> {
        fn text(value: &str) -> Option<String> {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }

        fn number(label: &str, value: &str) -> Result<Option<u32>, String> {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            trimmed
                .parse::<u32>()
                .map(Some)
                .map_err(|_| format!("{label} must be a positive number"))
        }

        Ok(sotf_audio_player::MetadataPatch {
            title: (self.scope == MetadataEditorScope::Track)
                .then(|| text(&self.fields.title))
                .flatten(),
            album_title: (self.scope == MetadataEditorScope::Album)
                .then(|| text(&self.fields.title))
                .flatten(),
            artist: text(&self.fields.artist),
            album_artist: text(&self.fields.album_artist),
            year: number("Year", &self.fields.year)?,
            genre: text(&self.fields.genre),
            composer: text(&self.fields.composer),
            disc_number: number("Disc", &self.fields.disc_number)?,
            track_number: number("Track", &self.fields.track_number)?,
            conductor: text(&self.fields.conductor),
            performer: text(&self.fields.performer),
            isrc: text(&self.fields.isrc),
            ensemble: text(&self.fields.ensemble),
            edition: text(&self.fields.edition),
        })
    }

    pub fn apply_candidate(&mut self, candidate: sotf_audio_player::MetadataImportCandidate) {
        if let Some(title) = candidate.title.or(candidate.album_title) {
            self.fields.title = title;
        }
        if let Some(artist) = candidate.artist {
            self.fields.artist = artist;
        }
        if let Some(album_artist) = candidate.album_artist {
            self.fields.album_artist = album_artist;
        }
        if let Some(year) = candidate.year {
            self.fields.year = year.to_string();
        }
        if let Some(track_number) = candidate.track_number {
            self.fields.track_number = track_number.to_string();
        }
        if let Some(disc_number) = candidate.disc_number {
            self.fields.disc_number = disc_number.to_string();
        }
        if let Some(isrc) = candidate.isrc {
            self.fields.isrc = isrc;
        }
        self.search_error = None;
    }
}

/// Type of plugin update needed for audio engine synchronization
#[derive(Debug, Clone)]
pub enum PluginUpdateType {
    /// Single parameter change - use set_plugin_parameter() for zero-dropout update
    Parameter {
        plugin_index: usize,
        param_index: usize,
    },
    /// Parameter change addressed by graph node ID (works for non-linear graphs)
    ParameterByNodeId {
        node_id: sotf_audio_player::GraphNodeId,
        param_index: usize,
    },
    /// Structural change (add/remove/reorder/toggle) - use update_plugins() for full rebuild
    Structural,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeasureStep {
    DeviceSelection,
    SignalConfig,
    Running,
    Results,
}

#[derive(Debug, Clone)]
pub struct MeasurementResult {
    pub frequencies: Vec<f32>,
    pub magnitude_db: Vec<f32>,
    pub phase_deg: Vec<f32>,
    pub csv_path: String,
}

#[derive(Debug, Clone)]
pub struct MeasureState {
    pub step: MeasureStep,
    pub signal_type: String, // "sweep", "pink-noise"
    pub duration: String,    // "5.0", "10.0"
    pub level: f32,          // -20dB etc
    pub output_channel: usize,
    pub input_channel: usize,
    pub progress: f32,
    pub status_message: String,
    pub measurement_result: Option<MeasurementResult>,
    // UI state for dropdowns
    pub output_ch_open: bool,
    pub input_ch_open: bool,
}

impl Default for MeasureState {
    fn default() -> Self {
        Self {
            step: MeasureStep::DeviceSelection,
            signal_type: "sweep".to_string(),
            duration: "5.0".to_string(),
            level: -20.0,
            output_channel: 0,
            input_channel: 0,
            progress: 0.0,
            status_message: String::new(),
            measurement_result: None,
            output_ch_open: false,
            input_ch_open: false,
        }
    }
}

/// UI state for optimization forms (dropdowns open/closed)
#[derive(Debug, Clone, Default)]
pub struct OptimizationUiState {
    pub peq_model_open: bool,
    pub algo_open: bool,
    pub strategy_open: bool,
    pub local_algo_open: bool,
}

pub mod calibration;
pub mod maturity;
pub mod queue;
pub mod settings;
pub mod stats;
pub mod toast;

pub mod headphone_eq;
pub mod recording;
pub mod room_eq;
pub mod spinorama_eq;

// Re-export commonly used types for convenience
pub use calibration::CalibrationData;
pub use headphone_eq::{HeadphoneEqBiquad, HeadphoneEqResult, HeadphoneEqState, HeadphoneEqStep};
pub use queue::QueueItem;
pub use recording::{
    ChannelMapping, ChannelRecording, ChannelRecordingState, CtcMatrixExportStrategy,
    PlaybackDeviceConfig, PlotSmoothing, RecordingDeviceConfig, RecordingResult,
    RecordingSignalType, RecordingState, RecordingStep, SpeakerConfiguration,
    TransferMatrixLoopbackRecording,
};
pub use room_eq::{
    ChannelDspChain, ChannelMeasurement, ChannelOptResult, CrossoverType, CustomTargetCurve,
    DriverDspChain, DspChainMetadata, DspChainOutput, DspPluginConfig, EqFilterConfig,
    MultiSpeakerMode, OptimizationStatus, RoomEqAlgorithm, RoomEqDataSource,
    RoomEqMeasurementsFile, RoomEqOptimizationMode, RoomEqOptimizerConfig, RoomEqSpeakerConfig,
    RoomEqState, RoomEqStep, SpeakerConfigType, TargetCurveControlPoint,
};
pub use settings::SettingsTab;
pub use spinorama_eq::{
    DirectivityCurve, SpinoramaBiquad, SpinoramaCurves, SpinoramaEqResult, SpinoramaEqState,
    SpinoramaOptimizationMode, SpinoramaStep, SpinoramaTargetCurve,
};
pub use stats::LibraryStats;
pub use toast::{ToastAction, ToastMessage, ToastType};
