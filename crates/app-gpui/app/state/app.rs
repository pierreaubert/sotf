//! Application state definitions.
//!
//! Contains the main App struct and AppState wrapper.

use std::collections::{HashMap, VecDeque};
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::Arc;

use gpui::Entity;
use gpui_themes::{
    AccessibilityPalette, CommunityThemeBundle, ThemeAppearance, ThemeModePreference,
    ThemeSchedule, ThemeTransition,
};
use gpui_ui_kit::workflow::NodeId;
use sotf_audio_player::{Player, QueueController, QueuePlaybackEffect};

use crate::i18n::{Language, Translations};
use crate::keybindings::KeymapPreset;
use crate::theme::{CommunityThemeId, Theme, ThemeAccentPreference, ThemeId};

use crate::app::debug::StateHistory;
use crate::app::types::{
    ChannelGroup, InputMode, LayoutOrientation, LibraryStats, MeterDisplayMode,
    OptimizationUiState, RackDisplayMode, ToastMessage,
};

use super::ui::LayoutState;
use super::{InputState, LibraryState, PlaybackState, PluginState, UIState};
use crate::app::constants::recording::DEFAULT_SIGNAL_LEVEL_DB;
use crate::app::manager::{Manager, ManagerError};
use crate::app::state::library::LibraryEvent;
use crate::components::plugins::editing::PluginEditingManager;
use crate::components::plugins::level_meters::LevelMeterManager;

/// Messages that can be dispatched to the App
#[derive(Debug, Clone)]
pub enum AppMessage {
    Library(LibraryEvent),
}

/// Mapping between workflow NodeIds and plugin indices / special nodes
#[derive(Clone, Default, Debug)]
pub struct WorkflowNodeMapping {
    pub node_to_plugin: HashMap<NodeId, usize>,
    pub plugin_to_node: HashMap<usize, NodeId>,
    /// Workflow node ID for the Input special node
    pub input_node_id: Option<NodeId>,
    /// Workflow node ID for the Output special node
    pub output_node_id: Option<NodeId>,
}

/// Level meter display and peak hold state
#[derive(Debug, Default)]
pub struct LevelMeterState {
    pub groups: Vec<ChannelGroup>,
    pub selected_group: usize,
    pub control_selection: usize, // 0 = Mute, 1 = Solo, 2 = Dim
    /// Cached channel count to avoid rebuilding meter groups every frame
    pub last_channel_count: usize,
    /// Cached speaker config to avoid rebuilding meter groups every frame
    pub last_speaker_config: Option<String>,
    /// Peak hold values per channel (linear scale, 0.0 to 1.0+)
    pub peak_hold: Vec<f64>,
    /// Last update time for peak hold decay
    pub peak_hold_last_update: Option<std::time::Instant>,
    pub display_mode: MeterDisplayMode,
}

/// Speaker optimization workflow state
#[derive(Debug)]
pub struct SpeakerOptState {
    pub model: String,
    pub params: sotf_audio_player::autoeq::OptimizationParams,
    pub running: bool,
    pub progress: Vec<(usize, f64)>,
    pub result: Option<sotf_audio_player::autoeq::SpeakerOptimizationResult>,
    pub export_format: String,
    pub ui: OptimizationUiState,
}

impl Default for SpeakerOptState {
    fn default() -> Self {
        Self {
            model: String::new(),
            params: sotf_audio_player::autoeq::OptimizationParams::speaker_defaults(),
            running: false,
            progress: Vec::new(),
            result: None,
            export_format: String::from("json"),
            ui: OptimizationUiState::default(),
        }
    }
}

/// Active knob/slider drag operation
#[derive(Debug, Clone, Copy)]
pub struct KnobDragState {
    pub plugin_idx: usize,
    pub param_idx: usize,
    pub start_y: f32,
    pub start_value: f64,
    pub min: f64,
    pub max: f64,
}

/// Active volume slider drag operation
#[derive(Debug, Clone, Copy)]
pub struct VolumeDragState {
    pub start_y: f32,
    pub start_value: f32,
}

/// Which divider is being dragged
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DividerType {
    InputMeter,
    OutputMeter,
    RackDetail,
    PluginAutoConfig { plugin_idx: usize },
    PluginAutoOutput { plugin_idx: usize },
}

/// State for tracking divider drag operations
#[derive(Debug, Clone)]
pub struct DividerDragState {
    pub divider_type: DividerType,
    pub start_x: f32,
    pub start_width: f32,
}

pub const RACK_STRIP_DEFAULT_HEIGHT: f32 = 180.0;
pub const RACK_STRIP_MIN_HEIGHT: f32 = 128.0;
pub const RACK_STRIP_MAX_HEIGHT: f32 = 360.0;

pub fn rack_strip_height_from_drag(start_height: f32, drag_delta_y: f32) -> f32 {
    (start_height + drag_delta_y).clamp(RACK_STRIP_MIN_HEIGHT, RACK_STRIP_MAX_HEIGHT)
}

/// Queue state — wraps `QueueController` with per-item UI expansion tracking.
///
/// Deref/DerefMut to `QueueController` so `.len()`, `.iter()`, `.current_index`,
/// `.peek_next_track()`, etc. work transparently. Mutations that change item count
/// (add, remove, clear, fill_magic) are shadowed to keep `expanded` in sync.
#[derive(Debug)]
pub struct QueueState {
    ctrl: QueueController,
    /// Per-queue-item UI expansion state (true = expanded to show tracks)
    pub expanded: Vec<bool>,
    /// Currently selected queue item index in the UI
    pub selected_index: usize,
}

impl Deref for QueueState {
    type Target = QueueController;
    fn deref(&self) -> &Self::Target {
        &self.ctrl
    }
}

impl DerefMut for QueueState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ctrl
    }
}

impl Default for QueueState {
    fn default() -> Self {
        Self::new()
    }
}

impl QueueState {
    pub fn new() -> Self {
        Self {
            ctrl: QueueController::new(),
            expanded: Vec::new(),
            selected_index: 0,
        }
    }

    /// Add an album to the queue, tracking its expansion state.
    pub fn add_album(&mut self, album: sotf_audio_player::Album) -> Result<usize, String> {
        let idx = self.ctrl.add_album(album)?;
        self.expanded.push(false);
        Ok(idx)
    }

    /// Add album and immediately jump to it for playback.
    pub fn play_album_now(
        &mut self,
        album: sotf_audio_player::Album,
    ) -> Result<QueuePlaybackEffect, String> {
        let effect = self.ctrl.play_album_now(album)?;
        self.expanded.push(false);
        Ok(effect)
    }

    /// Remove the album at `index`, keeping expansion in sync.
    pub fn remove(&mut self, index: usize) -> (QueuePlaybackEffect, bool) {
        if index >= self.ctrl.len() {
            return (QueuePlaybackEffect::None, false);
        }
        let result = self.ctrl.remove(index);
        if index < self.expanded.len() {
            self.expanded.remove(index);
        } else {
            self.expanded.resize(self.ctrl.len(), false);
        }
        if self.selected_index >= self.ctrl.len() && self.selected_index > 0 {
            self.selected_index = self.ctrl.len() - 1;
        }
        result
    }

    /// Clear all items from the queue.
    pub fn clear(&mut self) {
        self.ctrl.clear();
        self.expanded.clear();
        self.selected_index = 0;
    }

    /// Fill queue with "magic" recommendations.
    pub fn fill_magic(
        &mut self,
        db: &sotf_audio_player::MusicDatabase,
        library_albums: &[sotf_audio_player::Album],
    ) -> Result<Vec<sotf_audio_player::Album>, String> {
        let added = self.ctrl.fill_magic(db, library_albums)?;
        for _ in &added {
            self.expanded.push(false);
        }
        Ok(added)
    }

    /// Toggle expansion of the currently selected queue item.
    pub fn toggle_expansion(&mut self) {
        if self.selected_index < self.expanded.len() {
            self.expanded[self.selected_index] = !self.expanded[self.selected_index];
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreamUiState {
    pub store: sotf_audio_player::SavedStreamStore,
    pub selected_index: usize,
    pub name_input: String,
    pub url_input: String,
    pub format_hint_input: String,
    pub seekable_input: bool,
    pub last_error: Option<String>,
    pub last_status: Option<String>,
}

impl Default for StreamUiState {
    fn default() -> Self {
        Self {
            store: sotf_audio_player::load_saved_streams().unwrap_or_default(),
            selected_index: 0,
            name_input: String::new(),
            url_input: String::new(),
            format_hint_input: String::new(),
            seekable_input: false,
            last_error: None,
            last_status: None,
        }
    }
}

impl StreamUiState {
    pub fn format_hint(&self) -> Option<String> {
        let hint = self.format_hint_input.trim();
        (!hint.is_empty()).then(|| hint.to_string())
    }
}

#[derive(Debug)]
pub struct App {
    // Library state - now managed via library_state
    /// Cached library statistics (artists, tracks, genres, years, etc.)
    /// Call invalidate_library_stats() when library changes, get_library_stats() to access
    pub library_stats: LibraryStats,
    pub library_scanner: Option<sotf_audio_player::LibraryScanner>,

    // Queue state
    pub queue_state: QueueState,

    // Saved HTTP/SOTF streams
    pub stream_state: StreamUiState,

    // Speaker Optimization State
    pub speaker_opt: SpeakerOptState,

    // Selection indices
    pub selected_directory_index: usize,
    pub album_list_offset: usize,

    // Level meters
    pub level_meters: LevelMeterState,

    // Spectrum analyzer
    pub spectrum_visible: bool,

    // Flags
    pub needs_rescan: bool,
    pub is_loading_initial_data: bool,
    pub library_stats_computing: bool,
    pub pending_library_stats: Arc<parking_lot::Mutex<Option<LibraryStats>>>,

    // Scan progress modal (for library, bliss, waveform, replaygain scans)
    pub scan_progress_modal: Option<crate::app::types::ScanProgressModal>,

    // Layout configuration is now managed via AppState.layout entity
    pub divider_click_start: Option<std::time::Instant>,

    // 3-Panel Layout (Library | Queue | Rack)
    pub layout_orientation: LayoutOrientation,
    pub rack_display_mode: RackDisplayMode,
    // Hide queue meters when rack is visible in 3-panel layout
    pub hide_queue_meters_for_rack: bool,

    // Scan progress for threaded scanning
    pub scan_total_files: usize,

    // Drag states (None = not dragging)
    pub volume_drag: Option<VolumeDragState>,
    pub knob_drag: Option<KnobDragState>,

    // Settings accordion expanded sections
    pub expanded_settings_sections: Vec<String>,

    // Playlists
    pub playlist_controller: sotf_audio_player::PlaylistController,

    // Scan managers (ReplayGain, Waveform, Bliss)
    pub scan_ctrl: sotf_audio_player::ScanController,

    // Plugin UI states
    pub upmixer_config_open: bool,
    pub upmixer_tab: usize,
    /// State for the spatial spider visualizer (shared across upmixer / XTC / AAE).
    pub spatial_spider: crate::components::plugins::spatial_spider::SpatialSpiderUiState,
    pub spectrum_tilt_select_open: bool,
    pub spectrum_reference_select_open: bool,
    pub show_add_plugin_menu: bool,
    /// Active secondary tab index for auto-layout plugins (per-plugin, keyed by plugin_idx)
    pub plugin_auto_tab: std::collections::HashMap<usize, usize>,
    /// User-resized config column width for auto-layout plugins.
    pub plugin_auto_config_width: std::collections::HashMap<usize, f32>,
    /// User-resized output column width for auto-layout plugins.
    pub plugin_auto_output_width: std::collections::HashMap<usize, f32>,

    // Rack panel collapse states
    pub rack_detail_collapsed: bool, // Horizontal divider between rack and detail
    pub input_meter_collapsed: bool, // Left meter panel
    pub output_meter_collapsed: bool, // Right meter panel

    // Rack panel widths (for resizing)
    pub rack_strip_height: f32, // Height of signal-chain strip above plugin detail
    pub input_meter_width: f32, // Width of input meter panel
    pub output_meter_width: f32, // Width of output meter panel

    // Divider drag state
    pub dragging_divider: Option<DividerDragState>,

    // Composed state structs (for better separation of concerns)
    /// Playback-related state
    pub playback: PlaybackState,
    /// Library-related state
    pub library_state: LibraryState,
    /// Plugin-related state
    pub plugin_state: PluginState,
    /// UI-related state
    pub ui_state: UIState,
    /// Input-related state (text fields, autocomplete)
    pub input_state: InputState,
    /// Audio device state (input/output devices, playback source)
    pub audio_device_state: super::AudioDeviceState,
    /// Measurement and EQ workflow state
    pub measurement_state: super::MeasurementState,

    /// Shared state across managers
    pub shared_state: Arc<super::SharedState>,

    /// Debug state history tracker
    pub state_history: StateHistory,

    /// Event sourcing for playback state
    pub playback_events: super::playback_events::PlaybackEventStore,

    // Play tracking for statistics (30s threshold)
    pub track_tracking: TrackTrackingState,

    // Channel conflict dialog state
    pub channel_conflict_path: Option<sotf_audio::decoder::AudioSource>,
    pub channel_conflicts: Vec<sotf_audio_player::ChannelConflict>,
    pub channel_conflict_track_channels: usize,

    /// Whether the tutorial has been completed/dismissed (persisted to config)
    pub tutorial_completed: bool,

    /// Hint IDs that have been shown and dismissed (persisted to config).
    /// Uses string IDs so new hints can be added without migration.
    pub seen_hints: Vec<String>,
    /// Currently displayed contextual hint (None if no hint active).
    pub current_hint: Option<crate::components::dialogs::tutorial::ContextualHint>,

    /// Cached window geometry to avoid re-reading config from disk on every save
    pub last_saved_geometry: Option<crate::config::WindowGeometry>,

    // Federation & Server configuration
    pub federation: FederationState,

    // Native SOTF remote-control server picker state.
    pub remote: RemoteState,
}

fn stream_queue_album(stream: &sotf_audio_player::SavedStream) -> sotf_audio_player::Album {
    sotf_audio_player::Album {
        title: stream.name.clone(),
        tracks: vec![sotf_audio_player::Track {
            path: PathBuf::from(&stream.url),
            source: Some(stream.audio_source()),
            title: Some(stream.name.clone()),
            artist: Some("Streams".to_string()),
            duration_secs: None,
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// Play tracking for statistics — records a play after 30s threshold
#[derive(Debug, Default)]
pub struct TrackTrackingState {
    pub path: Option<std::path::PathBuf>,
    pub start_time: Option<std::time::Instant>,
    pub already_recorded: bool,
}

/// Trusted client info for display in the pairing UI.
#[derive(Debug, Clone)]
pub struct TrustedClientInfo {
    pub fingerprint: String,
    pub name: String,
    pub paired_at: String,
}

/// Federation & server configuration and background scan state
#[derive(Debug)]
pub struct FederationState {
    pub sources: Vec<sotf_audio_player::federation_config::FederationSourceEntry>,
    pub source_statuses: HashMap<String, sotf_audio_player::federation_config::ConnectionStatus>,
    pub server_config: sotf_audio_player::federation_config::ServerConfig,
    pub scan_receiver: Option<std::sync::mpsc::Receiver<FederationScanMessage>>,
    pub scan_cancel: Arc<std::sync::atomic::AtomicBool>,
    pub scan_progress: Option<FederationScanProgress>,
    pub cast_discovery_receiver:
        Option<std::sync::mpsc::Receiver<Vec<crate::app::state::audio_device::CastDeviceInfo>>>,
    /// Whether the local SOTF API server is in pairing mode.
    pub pairing_enabled: bool,
    /// Current pairing nonce (valid only when pairing_enabled is true).
    pub pairing_nonce: Option<String>,
    /// Server TLS fingerprint for QR code display.
    pub server_fingerprint: Option<String>,
    /// List of trusted clients paired with this server.
    pub trusted_clients: Vec<TrustedClientInfo>,
    /// Last pairing operation error message.
    pub pairing_error: Option<String>,
}

impl Default for FederationState {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            source_statuses: HashMap::new(),
            server_config: sotf_audio_player::federation_config::ServerConfig::default(),
            scan_receiver: None,
            scan_cancel: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            scan_progress: None,
            cast_discovery_receiver: None,
            pairing_enabled: false,
            pairing_nonce: None,
            server_fingerprint: None,
            trusted_clients: Vec::new(),
            pairing_error: None,
        }
    }
}

/// Progress state shown in the UI during a federation scan.
#[derive(Debug, Clone)]
pub struct FederationScanProgress {
    pub source_name: String,
    pub albums_total: usize,
    pub albums_merged: usize,
    pub tracks_merged: usize,
}

/// Messages sent from the federation scan background thread.
#[derive(Debug)]
pub enum FederationScanMessage {
    /// Fetched album list from provider — now starting merge.
    FetchedAlbums { total: usize },
    /// Progress update after merging an album.
    Progress {
        albums_merged: usize,
        tracks_merged: usize,
    },
    /// Scan completed (or failed).
    Done(sotf_audio_player::federation_scan::FederationScanResult),
}

pub use sotf_audio_player::federation_scan::FederationScanResult;

/// Native SOTF remote-control server picker and discovery state.
#[derive(Debug, Default)]
pub struct RemoteState {
    pub server_store: sotf_audio_player::SotfRemoteServerStore,
    pub discovered_servers: Vec<sotf_audio_player::lan_discovery::DiscoveredSotfApiServer>,
    pub server_probe_statuses: HashMap<String, RemoteServerProbeStatus>,
    /// Monotonic marker for probe-status changes observed by the UI tick.
    pub server_probe_revision: u64,
    pub discovery_running: bool,
    pub discovery_error: Option<String>,
    pub manual_server_name: String,
    pub manual_api_base_url: String,
    pub manual_auth_token: String,
    pub server_probe_receiver: Option<std::sync::mpsc::Receiver<(String, RemoteServerProbeStatus)>>,
    pub discovery_receiver: Option<
        std::sync::mpsc::Receiver<
            Result<Vec<sotf_audio_player::lan_discovery::DiscoveredSotfApiServer>, String>,
        >,
    >,
    /// Receiver for live SSE events from the selected remote server.
    pub event_stream_receiver: Option<
        std::sync::mpsc::Receiver<
            Result<sotf_audio_player::sotf_api_client::SotfApiStreamEvent, String>,
        >,
    >,
    /// Receiver for quiet remote cache refresh jobs.
    pub cache_refresh_receiver: Option<
        std::sync::mpsc::Receiver<Result<RemoteCacheRefreshResult, RemoteCacheRefreshError>>,
    >,
    /// Whether a quiet remote cache refresh job is currently running.
    pub cache_refresh_in_progress: bool,
    /// Consecutive quiet cache refresh failures for the selected remote.
    pub cache_refresh_failures: u8,
    /// Disable quiet cache refreshes after repeated network failures.
    pub cache_updates_disabled: bool,
    /// Last quiet cache refresh error, kept for diagnostics only.
    pub cache_last_error: Option<String>,
    /// In-memory bearer token cache keyed by server ID. Persisted credentials
    /// live in platform storage or the shared internal token store.
    pub server_tokens: HashMap<String, String>,
    /// Bounded in-memory cache for remote album metadata and artwork.
    /// This is a performance cache only, not a local library mirror.
    pub album_cache: RemoteAlbumCache,
    /// Latest remote state snapshot received from the selected server.
    pub current_state: Option<sotf_audio_player::sotf_api_client::SotfApiState>,
    /// Latest remote queue snapshot received from the selected server.
    pub current_queue: Option<sotf_audio_player::sotf_api_client::SotfApiQueue>,
    /// Visible remote album page, sourced from the server API.
    pub current_album_page: Option<sotf_audio_player::sotf_api_client::SotfApiAlbumList>,
    /// Server ID that produced the visible remote album page.
    pub current_album_page_server_id: Option<String>,
    /// Search query used to produce the visible remote album page.
    pub current_album_page_query: String,
    /// Monotonic marker for remote album-page changes observed by the UI tick.
    pub remote_album_page_revision: u64,
    /// Remote library identity currently associated with the local database.
    pub local_library_identity: Option<crate::config::RemoteLibraryIdentity>,
    /// Minimal refresh work requested by SSE events.
    pub refresh_requests: RemoteRefreshRequests,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RemoteRefreshRequests {
    pub state: bool,
    pub queue: bool,
    pub visible_album_page: bool,
}

impl RemoteRefreshRequests {
    pub fn is_empty(&self) -> bool {
        !self.state && !self.queue && !self.visible_album_page
    }

    pub fn merge(&mut self, other: Self) {
        self.state |= other.state;
        self.queue |= other.queue;
        self.visible_album_page |= other.visible_album_page;
    }
}

#[derive(Debug)]
pub struct RemoteCacheRefreshResult {
    pub server_id: String,
    pub state: Option<sotf_audio_player::sotf_api_client::SotfApiState>,
    pub queue: Option<sotf_audio_player::sotf_api_client::SotfApiQueue>,
    pub album_page: Option<sotf_audio_player::sotf_api_client::SotfApiAlbumList>,
    pub album_query: Option<String>,
    pub artwork: Vec<(String, Vec<u8>)>,
}

#[derive(Debug)]
pub struct RemoteCacheRefreshError {
    pub requests: RemoteRefreshRequests,
    pub message: String,
}

const DEFAULT_REMOTE_ALBUM_CACHE_LIMIT: usize = 250;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RemoteAlbumCacheKey {
    server_id: String,
    library_version: u64,
    album_id: String,
}

#[derive(Debug)]
pub struct RemoteAlbumCache {
    max_albums: usize,
    metadata: HashMap<RemoteAlbumCacheKey, sotf_audio_player::sotf_api_client::SotfApiAlbum>,
    metadata_lru: VecDeque<RemoteAlbumCacheKey>,
    artwork: HashMap<RemoteAlbumCacheKey, Vec<u8>>,
    artwork_lru: VecDeque<RemoteAlbumCacheKey>,
}

impl Default for RemoteAlbumCache {
    fn default() -> Self {
        Self::with_limit(DEFAULT_REMOTE_ALBUM_CACHE_LIMIT)
    }
}

impl RemoteAlbumCache {
    pub fn with_limit(max_albums: usize) -> Self {
        Self {
            max_albums: max_albums.max(1),
            metadata: HashMap::new(),
            metadata_lru: VecDeque::new(),
            artwork: HashMap::new(),
            artwork_lru: VecDeque::new(),
        }
    }

    pub fn max_albums(&self) -> usize {
        self.max_albums
    }

    pub fn metadata_len(&self) -> usize {
        self.metadata.len()
    }

    pub fn artwork_len(&self) -> usize {
        self.artwork.len()
    }

    pub fn upsert_metadata_page(
        &mut self,
        server_id: &str,
        library_version: u64,
        albums: &[sotf_audio_player::sotf_api_client::SotfApiAlbum],
    ) {
        for album in albums {
            let key = RemoteAlbumCacheKey {
                server_id: server_id.to_string(),
                library_version,
                album_id: album.id.clone(),
            };
            self.metadata.insert(key.clone(), album.clone());
            touch_lru(&mut self.metadata_lru, key);
        }
        evict_lru(&mut self.metadata, &mut self.metadata_lru, self.max_albums);
    }

    pub fn metadata(
        &self,
        server_id: &str,
        library_version: u64,
        album_id: &str,
    ) -> Option<&sotf_audio_player::sotf_api_client::SotfApiAlbum> {
        self.metadata.get(&RemoteAlbumCacheKey {
            server_id: server_id.to_string(),
            library_version,
            album_id: album_id.to_string(),
        })
    }

    pub fn upsert_artwork(
        &mut self,
        server_id: &str,
        library_version: u64,
        album_id: &str,
        bytes: Vec<u8>,
    ) {
        let key = RemoteAlbumCacheKey {
            server_id: server_id.to_string(),
            library_version,
            album_id: album_id.to_string(),
        };
        self.artwork.insert(key.clone(), bytes);
        touch_lru(&mut self.artwork_lru, key);
        evict_lru(&mut self.artwork, &mut self.artwork_lru, self.max_albums);
    }

    pub fn artwork(&self, server_id: &str, library_version: u64, album_id: &str) -> Option<&[u8]> {
        self.artwork
            .get(&RemoteAlbumCacheKey {
                server_id: server_id.to_string(),
                library_version,
                album_id: album_id.to_string(),
            })
            .map(Vec::as_slice)
    }

    pub fn invalidate_server(&mut self, server_id: &str) {
        self.metadata.retain(|key, _| key.server_id != server_id);
        self.metadata_lru.retain(|key| key.server_id != server_id);
        self.artwork.retain(|key, _| key.server_id != server_id);
        self.artwork_lru.retain(|key| key.server_id != server_id);
    }

    pub fn invalidate_all(&mut self) {
        self.metadata.clear();
        self.metadata_lru.clear();
        self.artwork.clear();
        self.artwork_lru.clear();
    }
}

fn touch_lru<T: Clone + Eq>(lru: &mut VecDeque<T>, key: T) {
    lru.retain(|existing| existing != &key);
    lru.push_back(key);
}

fn evict_lru<T: Clone + Eq + std::hash::Hash, V>(
    values: &mut HashMap<T, V>,
    lru: &mut VecDeque<T>,
    max_len: usize,
) {
    while values.len() > max_len {
        let Some(key) = lru.pop_front() else {
            break;
        };
        values.remove(&key);
    }
}

impl RemoteState {
    pub const CACHE_REFRESH_FAILURE_DISABLE_THRESHOLD: u8 = 3;

    pub fn set_server_probe_status(
        &mut self,
        server_id: impl Into<String>,
        status: RemoteServerProbeStatus,
    ) {
        self.server_probe_statuses.insert(server_id.into(), status);
        self.server_probe_revision = self.server_probe_revision.wrapping_add(1);
    }

    pub fn remove_server_probe_status(&mut self, server_id: &str) {
        if self.server_probe_statuses.remove(server_id).is_some() {
            self.server_probe_revision = self.server_probe_revision.wrapping_add(1);
        }
    }

    pub fn merge_discovered_servers(
        &mut self,
        servers: Vec<sotf_audio_player::lan_discovery::DiscoveredSotfApiServer>,
    ) -> usize {
        let mut merged = 0;
        let had_selection = self.server_store.selected_server_id.is_some();
        let mut first_id = None;

        for discovered in &servers {
            match sotf_audio_player::SotfRemoteServer::from_discovered(discovered) {
                Ok(server) => {
                    first_id.get_or_insert_with(|| server.id.clone());
                    self.server_store.upsert(server);
                    merged += 1;
                }
                Err(err) => {
                    log::warn!("Ignoring invalid discovered SOTF server: {err}");
                }
            }
        }

        if !had_selection && let Some(id) = first_id {
            let _ = self.server_store.select(id);
        }

        self.discovered_servers = servers;
        self.discovery_error = None;
        merged
    }

    pub fn add_manual_server_record(
        &mut self,
        friendly_name: impl Into<String>,
        api_base_url: impl Into<String>,
    ) -> Result<String, String> {
        let server = sotf_audio_player::SotfRemoteServer::manual(friendly_name, api_base_url)
            .map_err(|err| err.to_string())?;
        let id = server.id.clone();
        self.server_store.upsert(server);
        let _ = self.server_store.select(id.clone());
        Ok(id)
    }

    pub fn set_manual_server_name(&mut self, name: impl Into<String>) {
        self.manual_server_name = name.into();
    }

    pub fn set_manual_api_base_url(&mut self, api_base_url: impl Into<String>) {
        self.manual_api_base_url = api_base_url.into();
    }

    pub fn set_manual_auth_token(&mut self, token: impl Into<String>) {
        self.manual_auth_token = token.into();
    }

    pub fn add_manual_server_from_inputs(&mut self) -> Result<String, String> {
        let name = self.manual_server_name.trim().to_string();
        let mut api_base_url = self.manual_api_base_url.trim().to_string();
        let auth_token = self.manual_auth_token.trim().to_string();
        if api_base_url.is_empty() {
            return Err("remote server URL must not be empty".to_string());
        }
        if auth_token.is_empty() {
            return Err("remote API token must not be empty".to_string());
        }
        if !api_base_url.starts_with("http://") && !api_base_url.starts_with("https://") {
            api_base_url = format!("http://{api_base_url}");
        }

        let id = self.add_manual_server_record(name, api_base_url)?;
        self.server_tokens.insert(id.clone(), auth_token);
        self.manual_server_name.clear();
        self.manual_api_base_url.clear();
        self.manual_auth_token.clear();
        Ok(id)
    }

    pub fn apply_remote_album_page(
        &mut self,
        server_id: impl Into<String>,
        page: sotf_audio_player::sotf_api_client::SotfApiAlbumList,
        query: impl Into<String>,
    ) {
        let server_id = server_id.into();
        self.album_cache
            .upsert_metadata_page(&server_id, page.library_version, &page.albums);
        self.current_album_page = Some(page);
        self.current_album_page_server_id = Some(server_id);
        self.current_album_page_query = query.into();
        self.remote_album_page_revision = self.remote_album_page_revision.wrapping_add(1);
        self.refresh_requests.visible_album_page = false;
    }

    pub fn update_local_library_identity(
        &mut self,
        identity: crate::config::RemoteLibraryIdentity,
    ) -> bool {
        if self.local_library_identity.as_ref() == Some(&identity) {
            return false;
        }

        self.album_cache.invalidate_all();
        self.clear_remote_album_page();
        self.local_library_identity = Some(identity);
        true
    }

    pub fn clear_remote_album_page(&mut self) {
        if self.current_album_page.is_some()
            || self.current_album_page_server_id.is_some()
            || !self.current_album_page_query.is_empty()
        {
            self.remote_album_page_revision = self.remote_album_page_revision.wrapping_add(1);
        }
        self.current_album_page = None;
        self.current_album_page_server_id = None;
        self.current_album_page_query.clear();
    }

    pub fn reset_remote_cache_updater(&mut self) {
        self.cache_refresh_receiver = None;
        self.cache_refresh_in_progress = false;
        self.cache_refresh_failures = 0;
        self.cache_updates_disabled = false;
        self.cache_last_error = None;
    }

    pub fn record_remote_cache_refresh_success(&mut self) {
        self.cache_refresh_in_progress = false;
        self.cache_refresh_receiver = None;
        self.cache_refresh_failures = 0;
        self.cache_last_error = None;
    }

    pub fn record_remote_cache_refresh_failure(&mut self, err: RemoteCacheRefreshError) {
        self.cache_refresh_in_progress = false;
        self.cache_refresh_receiver = None;
        self.cache_last_error = Some(err.message);
        self.cache_refresh_failures = self.cache_refresh_failures.saturating_add(1);
        if self.cache_refresh_failures >= Self::CACHE_REFRESH_FAILURE_DISABLE_THRESHOLD {
            self.cache_updates_disabled = true;
            self.refresh_requests = RemoteRefreshRequests::default();
        } else {
            self.refresh_requests.merge(err.requests);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteServerProbeStatus {
    Testing,
    Reachable {
        friendly_name: String,
        version: String,
        auth_required: bool,
        api_version: u32,
        media_range: bool,
        events: bool,
    },
    Failed(String),
}

impl RemoteServerProbeStatus {
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Testing => "testing".to_string(),
            Self::Reachable {
                version,
                auth_required,
                media_range,
                events,
                ..
            } => {
                let media = if *media_range { "media" } else { "no media" };
                let live = if *events { "events" } else { "polling" };
                if *auth_required {
                    format!("reachable, auth required ({version}, {media}, {live})")
                } else {
                    format!("reachable ({version}, {media}, {live})")
                }
            }
            Self::Failed(err) => format!("failed: {err}"),
        }
    }
}

/// GPUI-compatible state wrapper
pub struct AppState {
    pub app: App,
    pub layout: Entity<LayoutState>,
    pub player: Arc<parking_lot::Mutex<Player>>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn current_minutes_after_midnight() -> u16 {
    use chrono::Timelike;

    let now = chrono::Local::now();
    (now.hour() * 60 + now.minute()) as u16
}

impl App {
    pub fn new() -> Self {
        let mut app = Self {
            library_stats: LibraryStats::default(),
            library_scanner: None,
            queue_state: QueueState::new(),
            stream_state: StreamUiState::default(),

            speaker_opt: SpeakerOptState::default(),

            selected_directory_index: 0,
            album_list_offset: 0,
            level_meters: LevelMeterState::default(),
            spectrum_visible: false,
            needs_rescan: false,
            is_loading_initial_data: true,
            library_stats_computing: false,
            pending_library_stats: Arc::new(parking_lot::Mutex::new(None)),
            scan_progress_modal: None,
            divider_click_start: None,
            // 3-Panel Layout defaults
            layout_orientation: LayoutOrientation::default(),
            rack_display_mode: RackDisplayMode::default(),
            hide_queue_meters_for_rack: false,
            scan_total_files: 0,
            volume_drag: None,
            knob_drag: None,
            expanded_settings_sections: vec!["library".to_string()],
            playlist_controller: sotf_audio_player::PlaylistController::new(),
            scan_ctrl: sotf_audio_player::ScanController::new(),
            upmixer_config_open: false,
            upmixer_tab: 1,
            spatial_spider:
                crate::components::plugins::spatial_spider::SpatialSpiderUiState::default(),
            spectrum_tilt_select_open: false,
            spectrum_reference_select_open: false,
            show_add_plugin_menu: false,
            plugin_auto_tab: std::collections::HashMap::new(),
            plugin_auto_config_width: std::collections::HashMap::new(),
            plugin_auto_output_width: std::collections::HashMap::new(),
            rack_detail_collapsed: false,
            input_meter_collapsed: false,
            output_meter_collapsed: false,
            rack_strip_height: RACK_STRIP_DEFAULT_HEIGHT,
            input_meter_width: 80.0,   // Default width for input meter panel
            output_meter_width: 140.0, // Default width for output meter panel
            dragging_divider: None,
            // Initialize composed state structs
            playback: PlaybackState::new(),
            library_state: LibraryState::new(),
            plugin_state: PluginState::new(),
            ui_state: UIState::new(),
            input_state: InputState::new(),
            audio_device_state: super::AudioDeviceState::new(),
            measurement_state: super::MeasurementState::new(),
            shared_state: Arc::new(super::SharedState::new()),
            state_history: StateHistory::new(),
            playback_events: super::playback_events::PlaybackEventStore::new(),
            track_tracking: TrackTrackingState::default(),
            channel_conflict_path: None,
            channel_conflicts: Vec::new(),
            channel_conflict_track_channels: 2,
            tutorial_completed: false,
            seen_hints: Vec::new(),
            current_hint: None,

            last_saved_geometry: None,

            federation: FederationState::default(),
            remote: RemoteState::default(),
        };

        // Initialize default stereo meter layout so meters are visible before audio starts
        app.update_level_meter_groups();

        app
    }

    pub fn save_stream_from_inputs(&mut self) -> Result<(), String> {
        let stream = sotf_audio_player::SavedStream::new(
            self.stream_state.name_input.clone(),
            self.stream_state.url_input.clone(),
            self.stream_state.format_hint(),
            self.stream_state.seekable_input,
        )
        .map_err(|err| err.to_string())?;
        self.stream_state.store.upsert(stream);
        sotf_audio_player::save_saved_streams(&self.stream_state.store)
            .map_err(|err| err.to_string())?;
        self.stream_state.last_error = None;
        self.stream_state.last_status = Some("Stream saved".to_string());
        Ok(())
    }

    pub fn add_stream_to_queue(
        &mut self,
        stream: sotf_audio_player::SavedStream,
    ) -> Result<Option<sotf_audio::decoder::AudioSource>, String> {
        let was_empty = self.queue_state.is_empty();
        let was_not_playing = !self.playback.is_playing;
        let album = stream_queue_album(&stream);
        self.queue_state.add_album(album)?;
        self.stream_state.last_error = None;
        self.stream_state.last_status = Some(format!("Added {}", stream.name));
        if was_empty || was_not_playing {
            return Ok(self.start_queue());
        }
        Ok(None)
    }

    pub fn play_stream_now(
        &mut self,
        stream: sotf_audio_player::SavedStream,
    ) -> Result<Option<sotf_audio::decoder::AudioSource>, String> {
        let effect = self
            .queue_state
            .play_album_now(stream_queue_album(&stream))?;
        self.playback.current_queue_index = self.queue_state.current_index();
        self.stream_state.last_error = None;
        self.stream_state.last_status = Some(format!("Playing {}", stream.name));
        if let QueuePlaybackEffect::Play(source) = effect {
            self.playback.is_playing = true;
            return Ok(Some(source));
        }
        Ok(None)
    }

    pub fn remove_stream_at(&mut self, index: usize) -> Result<(), String> {
        let Some(stream) = self.stream_state.store.streams.get(index).cloned() else {
            return Err("No stream selected".to_string());
        };
        if self.stream_state.store.remove_by_url(&stream.url) {
            sotf_audio_player::save_saved_streams(&self.stream_state.store)
                .map_err(|err| err.to_string())?;
            if self.stream_state.selected_index >= self.stream_state.store.streams.len()
                && self.stream_state.selected_index > 0
            {
                self.stream_state.selected_index -= 1;
            }
            self.stream_state.last_error = None;
            self.stream_state.last_status = Some(format!("Removed {}", stream.name));
        }
        Ok(())
    }

    pub fn set_stream_inputs_from_selected(&mut self, index: usize) {
        if let Some(stream) = self.stream_state.store.streams.get(index) {
            self.stream_state.selected_index = index;
            self.stream_state.name_input = stream.name.clone();
            self.stream_state.url_input = stream.url.clone();
            self.stream_state.format_hint_input = stream.format_hint.clone().unwrap_or_default();
            self.stream_state.seekable_input = stream.seekable;
            self.stream_state.last_error = None;
        }
    }

    pub fn save_remote_media_stream(
        &mut self,
        name: impl Into<String>,
        url: impl Into<String>,
    ) -> Result<sotf_audio_player::SavedStream, String> {
        let stream = sotf_audio_player::SavedStream::new(name, url, None, true)
            .map_err(|err| err.to_string())?;
        self.stream_state.store.upsert(stream.clone());
        sotf_audio_player::save_saved_streams(&self.stream_state.store)
            .map_err(|err| err.to_string())?;
        Ok(stream)
    }

    pub fn record_stream_error(&mut self, error: impl Into<String>) {
        self.stream_state.last_error = Some(error.into());
        self.stream_state.last_status = None;
    }

    pub fn rollback_failed_plugin_update(
        &mut self,
        plugin_state_snapshot: PluginState,
        error: impl Into<String>,
    ) {
        let error = error.into();
        self.plugin_state = plugin_state_snapshot;
        self.sync_spectrum_visible();
        self.update_level_meter_groups();
        self.ui_state.toast_message = Some(ToastMessage::error(format!(
            "Plugin update failed: {}",
            error
        )));
    }

    /// Dispatch a message to the appropriate manager
    pub fn dispatch(&mut self, msg: AppMessage) -> Result<(), ManagerError> {
        match msg {
            AppMessage::Library(event) => self.library_state.handle_event(event),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // State transition methods with debug logging
    // ─────────────────────────────────────────────────────────────────────────

    /// Set input mode with debug logging and state history capture
    pub fn set_input_mode(&mut self, mode: InputMode, trigger: &str) {
        let old_mode = self.ui_state.input_mode;
        if old_mode != mode {
            crate::app::debug::log_input_mode_transition(old_mode, mode, trigger);
            self.state_history.capture(
                self.ui_state.current_screen,
                mode,
                self.audio_device_state.current_output_device_name.clone(),
                format!("input_mode: {}", trigger),
            );
            self.ui_state.input_mode = mode;
        }
    }

    /// Set current screen with debug logging and state history capture.
    /// If the screen's maturity exceeds the current release channel, redirects to Library.
    pub fn set_screen(&mut self, screen: crate::app::Screen, trigger: &str) {
        let target = if self.ui_state.release_channel.allows(screen.maturity()) {
            screen
        } else {
            log::info!(
                "Screen {:?} requires {:?}, but release channel is {:?} — redirecting to Library",
                screen,
                screen.maturity(),
                self.ui_state.release_channel
            );
            self.ui_state.toast_message = Some(crate::app::ToastMessage::info(format!(
                "{:?} is not available on the {:?} release channel",
                screen, self.ui_state.release_channel
            )));
            crate::app::Screen::Library
        };
        let old_screen = self.ui_state.current_screen;
        if old_screen != target {
            crate::app::debug::log_screen_transition(old_screen, target, trigger);
            self.ui_state.last_screen = old_screen;
            self.state_history.capture(
                target,
                self.ui_state.input_mode,
                self.audio_device_state.current_output_device_name.clone(),
                format!("screen: {}", trigger),
            );
            self.ui_state.current_screen = target;
            self.plugin_state.clear_confirmations();

            // Trigger contextual hints for first-time screen visits
            use crate::components::dialogs::tutorial::HintId;
            match target {
                crate::app::Screen::Studio => self.try_show_hint(HintId::StudioFirstVisit),
                crate::app::Screen::RoomEq => self.try_show_hint(HintId::RoomEqFirstVisit),
                _ => {}
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Contextual hints
    // ─────────────────────────────────────────────────────────────────────────

    /// Try to show a contextual hint. Only shows if the hint hasn't been seen before.
    pub fn try_show_hint(&mut self, hint_id: crate::components::dialogs::tutorial::HintId) {
        let id_str = hint_id.as_str();
        if !self.seen_hints.iter().any(|s| s == id_str) && self.current_hint.is_none() {
            self.current_hint =
                Some(crate::components::dialogs::tutorial::ContextualHint { hint_id });
        }
    }

    /// Dismiss the current hint and mark it as seen.
    pub fn dismiss_hint(&mut self) {
        if let Some(hint) = self.current_hint.take() {
            let id_str = hint.hint_id.as_str().to_string();
            if !self.seen_hints.contains(&id_str) {
                self.seen_hints.push(id_str);
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Toast action handling
    // ─────────────────────────────────────────────────────────────────────────

    /// Handle a toast action button click.
    /// Override this to add custom action_id handlers for different toast actions.
    pub fn handle_toast_action(&mut self, action_id: &str) {
        log::info!("Toast action triggered: {}", action_id);
        // Add domain-specific action handlers here as needed, e.g.:
        // "retry-plugin-update" => self.retry_last_plugin_update(),
        log::warn!("Unhandled toast action: {}", action_id);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Release channel helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Current feature release channel.
    pub fn release_channel(&self) -> sotf_audio_player::ReleaseChannel {
        self.ui_state.release_channel
    }

    /// Whether the given screen is accessible at the current release channel.
    pub fn is_screen_available(&self, screen: &crate::app::Screen) -> bool {
        self.ui_state.release_channel.allows(screen.maturity())
    }

    /// Whether the given plugin type is accessible at the current release channel.
    pub fn is_plugin_available(&self, plugin_type: &sotf_audio_player::PluginType) -> bool {
        self.ui_state.release_channel.allows(plugin_type.maturity())
    }

    /// Maximum number of RoomEQ channels at the current release channel.
    pub fn max_room_eq_channels(&self) -> usize {
        match self.ui_state.release_channel {
            sotf_audio_player::ReleaseChannel::Prod => 0,
            sotf_audio_player::ReleaseChannel::Beta => 3,
            sotf_audio_player::ReleaseChannel::Alpha => 128,
        }
    }

    /// Set the release channel and redirect if current screen is no longer available.
    pub fn set_release_channel(&mut self, channel: sotf_audio_player::ReleaseChannel) {
        self.ui_state.release_channel = channel;
        // If current screen is no longer available, redirect to Library
        if !self.is_screen_available(&self.ui_state.current_screen) {
            self.ui_state.current_screen = crate::app::Screen::Library;
        }
    }

    /// Set output device with debug logging
    pub fn set_output_device(&mut self, device_name: Option<String>, trigger: &str) {
        let old_device = self
            .audio_device_state
            .current_output_device_name
            .as_deref();
        let new_device = device_name.as_deref();
        if old_device != new_device {
            crate::app::debug::log_device_change(old_device, new_device, trigger);
            self.state_history.capture(
                self.ui_state.current_screen,
                self.ui_state.input_mode,
                device_name.clone(),
                format!("device: {}", trigger),
            );
            self.audio_device_state.current_output_device_name = device_name;
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Playback event recording methods
    // ─────────────────────────────────────────────────────────────────────────

    /// Record playback started event
    pub fn record_playback_started(
        &mut self,
        queue_index: usize,
        track_path: Option<std::path::PathBuf>,
    ) {
        use super::playback_events::PlaybackEvent;
        self.playback_events.record_event(PlaybackEvent::Started {
            queue_index,
            track_path,
        });
    }

    /// Record playback paused event
    pub fn record_playback_paused(&mut self) {
        use super::playback_events::PlaybackEvent;
        self.playback_events.record_event(PlaybackEvent::Paused);
    }

    /// Record playback resumed event
    pub fn record_playback_resumed(&mut self) {
        use super::playback_events::PlaybackEvent;
        self.playback_events.record_event(PlaybackEvent::Resumed);
    }

    /// Record playback stopped event
    pub fn record_playback_stopped(&mut self) {
        use super::playback_events::PlaybackEvent;
        self.playback_events.record_event(PlaybackEvent::Stopped);
    }

    /// Record track change event
    pub fn record_track_changed(
        &mut self,
        from_index: Option<usize>,
        to_index: usize,
        trigger: super::playback_events::TrackChangeTrigger,
    ) {
        use super::playback_events::PlaybackEvent;
        self.playback_events
            .record_event(PlaybackEvent::TrackChanged {
                from_index,
                to_index,
                trigger,
            });
    }

    /// Record volume change event
    pub fn record_volume_changed(&mut self, from: f32, to: f32) {
        use super::playback_events::PlaybackEvent;
        self.playback_events
            .record_event(PlaybackEvent::VolumeChanged { from, to });
    }

    /// Record mute change event
    pub fn record_mute_changed(&mut self, muted: bool) {
        use super::playback_events::PlaybackEvent;
        self.playback_events
            .record_event(PlaybackEvent::MuteChanged { muted });
    }

    /// Record seek event
    pub fn record_seek(&mut self, from_position: f64, to_position: f64) {
        use super::playback_events::PlaybackEvent;
        self.playback_events.record_event(PlaybackEvent::Seeked {
            from_position,
            to_position,
        });
    }

    /// Record track ended event
    pub fn record_track_ended(&mut self, queue_index: usize) {
        use super::playback_events::PlaybackEvent;
        self.playback_events
            .record_event(PlaybackEvent::TrackEnded { queue_index });
    }

    /// Record playback error event
    pub fn record_playback_error(&mut self, message: impl Into<String>) {
        use super::playback_events::PlaybackEvent;
        self.playback_events.record_event(PlaybackEvent::Error {
            message: message.into(),
        });
    }

    /// Get playback event summary for debugging
    pub fn playback_event_summary(&self) -> super::playback_events::EventStoreSummary {
        self.playback_events.summary()
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Play tracking for statistics (30s threshold)
    // ─────────────────────────────────────────────────────────────────────────

    /// Start tracking a new track for play statistics
    pub fn start_track_tracking(&mut self, track_path: std::path::PathBuf) {
        self.track_tracking.path = Some(track_path);
        self.track_tracking.start_time = Some(std::time::Instant::now());
        self.track_tracking.already_recorded = false;
    }

    /// Check if current track has been played for 30+ seconds and record it
    pub fn check_and_record_play(&mut self) {
        if self.track_tracking.already_recorded {
            return;
        }

        if let (Some(path), Some(start_time)) =
            (&self.track_tracking.path, self.track_tracking.start_time)
        {
            let elapsed = start_time.elapsed().as_secs();
            if elapsed >= 30
                && let Some(db) = self.library_state.library.get_database()
            {
                let duration = self.playback.position_secs as u64;
                if let Err(e) = db.record_play(path, duration) {
                    log::error!("Failed to record play: {}", e);
                } else {
                    log::info!("Recorded play for {:?} ({}s)", path, duration);
                    self.track_tracking.already_recorded = true;

                    // Update in-memory play_count so UI reflects immediately
                    let path = path.clone();
                    for item in self.queue_state.iter_mut() {
                        for track in &mut item.album.tracks {
                            if track.path == path {
                                track.play_count += 1;
                            }
                        }
                    }
                    for album in &mut self.library_state.library.albums {
                        for track in &mut album.tracks {
                            if track.path == path {
                                track.play_count += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Stop tracking the current track (called when track changes or stops)
    pub fn stop_track_tracking(&mut self) {
        self.track_tracking.path = None;
        self.track_tracking.start_time = None;
        self.track_tracking.already_recorded = false;
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Favorites
    // ─────────────────────────────────────────────────────────────────────────

    /// Toggle favorite status for a track and update in-memory state
    pub fn toggle_track_favorite(&mut self, track_path: &std::path::Path) {
        let new_state = if let Some(db) = self.library_state.library.get_database() {
            match db.toggle_track_favorite(track_path) {
                Ok(state) => state,
                Err(e) => {
                    log::error!("Failed to toggle track favorite: {}", e);
                    return;
                }
            }
        } else {
            return;
        };

        // Update in queue
        for item in self.queue_state.iter_mut() {
            for track in &mut item.album.tracks {
                if track.path == track_path {
                    track.is_favorite = new_state;
                }
            }
        }

        // Update in library cache
        for album in &mut self.library_state.library.albums {
            for track in &mut album.tracks {
                if track.path == track_path {
                    track.is_favorite = new_state;
                }
            }
        }
    }

    /// Toggle favorite status for an album and update in-memory state
    pub fn toggle_album_favorite(&mut self, album_id: i64) {
        let new_state = if let Some(db) = self.library_state.library.get_database() {
            match db.toggle_album_favorite(album_id) {
                Ok(state) => state,
                Err(e) => {
                    log::error!("Failed to toggle album favorite: {}", e);
                    return;
                }
            }
        } else {
            return;
        };

        // Update in queue
        for item in self.queue_state.iter_mut() {
            if item.album.id == Some(album_id) {
                item.album.is_favorite = new_state;
            }
        }

        // Update in library cache
        for album in &mut self.library_state.library.albums {
            if album.id == Some(album_id) {
                album.is_favorite = new_state;
            }
        }

        // Invalidate library cache so sorted views update
        self.library_state.invalidate_cache();
    }

    /// Load library from database if available
    pub fn load_library_from_database(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.library_state.load_from_database()?;
        self.library_state.ensure_cache_valid();
        // Update last scan times for directories from database
        self.update_directory_scan_times();
        // Invalidate cached stats since library content changed
        self.invalidate_library_stats();
        Ok(())
    }

    /// Perform the initial database check on startup.
    /// Called from the UI update loop after the first render.
    /// Shows appropriate modal/toast based on database state.
    pub fn check_library_on_startup(&mut self) {
        // Only run once
        if self.ui_state.startup_db_check_done {
            return;
        }
        self.ui_state.startup_db_check_done = true;

        // Show tutorial on first launch
        if !self.tutorial_completed {
            self.ui_state.input_mode = InputMode::Tutorial;
            self.ui_state.tutorial_screen = 0;
            self.is_loading_initial_data = false;
            return;
        }

        // Try to load from database
        let t0 = std::time::Instant::now();
        if let Err(e) = self.load_library_from_database() {
            log::warn!("Failed to load library from database: {}", e);
        }
        log::info!(
            "[startup] check_library_on_startup: {:.1}ms ({} albums)",
            t0.elapsed().as_secs_f64() * 1000.0,
            self.library_state.library.albums.len(),
        );

        // Check if library is empty
        if self.library_state.library.albums.is_empty() {
            // Show modal prompting to scan for music
            self.ui_state.input_mode = InputMode::EmptyLibraryPrompt;
        }

        self.is_loading_initial_data = false;
    }

    /// Update directory scan times from database
    fn update_directory_scan_times(&mut self) {
        self.library_state.library.update_directory_scan_times();
    }

    /// Select the best default sample rate from available rates.
    /// Prefers 48000, then 44100, then device default, then first available, then 48000 as fallback.
    fn select_default_sample_rate(
        available_rates: &[u32],
        device_default_rate: Option<u32>,
    ) -> u32 {
        if available_rates.contains(&48000) {
            48000
        } else if available_rates.contains(&44100) {
            44100
        } else {
            device_default_rate.unwrap_or_else(|| available_rates.first().copied().unwrap_or(48000))
        }
    }

    pub fn load_audio_devices(&mut self) {
        use crate::app::state::audio_device::AudioDeviceState;

        // Load available devices
        if let Ok(devices_map) = sotf_audio::devices::get_audio_devices() {
            if let Some(output_devices) = devices_map.get("output") {
                // Use smart default selection that avoids virtual devices like HAL/BlackHole
                self.audio_device_state
                    .set_output_devices_with_smart_default(output_devices.clone());

                let selected_idx = self.audio_device_state.selected_output_device_index;

                // Initialize recording state playback device if not already set
                if self
                    .measurement_state
                    .recording_state
                    .playback_config
                    .device_name
                    .is_empty()
                {
                    // For recording config, prefer a device that's not virtual
                    let recording_device_idx = output_devices
                        .iter()
                        .position(|d| d.is_default && !AudioDeviceState::is_virtual_device(&d.name))
                        .or_else(|| {
                            output_devices
                                .iter()
                                .position(|d| !AudioDeviceState::is_virtual_device(&d.name))
                        })
                        .unwrap_or(selected_idx);

                    if let Some(device) = output_devices.get(recording_device_idx) {
                        self.measurement_state
                            .recording_state
                            .playback_config
                            .device_name = device.name.clone();
                        self.measurement_state
                            .recording_state
                            .playback_config
                            .device_id = device.name.clone();
                        if let Some(ref config) = device.default_config {
                            self.measurement_state
                                .recording_state
                                .playback_config
                                .num_channels = config.channels as usize;
                        }
                        self.measurement_state
                            .recording_state
                            .playback_config
                            .available_sample_rates = device.available_sample_rates.clone();
                        self.measurement_state
                            .recording_state
                            .playback_config
                            .sample_rate = Self::select_default_sample_rate(
                            &device.available_sample_rates,
                            device.default_config.as_ref().map(|c| c.sample_rate),
                        );
                    }
                }
            }
            if let Some(input_devices) = devices_map.get("input") {
                self.audio_device_state.input_devices = input_devices.clone();
                // Find the default device
                if let Some(default_idx) = input_devices.iter().position(|d| d.is_default) {
                    self.audio_device_state.selected_input_device_index = default_idx;
                    // Initialize recording state recording device if not already set
                    if self
                        .measurement_state
                        .recording_state
                        .recording_config
                        .device_name
                        .is_empty()
                    {
                        let device = &input_devices[default_idx];
                        self.measurement_state
                            .recording_state
                            .recording_config
                            .device_name = device.name.clone();
                        self.measurement_state
                            .recording_state
                            .recording_config
                            .device_id = device.name.clone();
                        if let Some(ref config) = device.default_config {
                            self.measurement_state
                                .recording_state
                                .recording_config
                                .num_channels = config.channels as usize;
                        }
                        self.measurement_state
                            .recording_state
                            .recording_config
                            .available_sample_rates = device.available_sample_rates.clone();
                        self.measurement_state
                            .recording_state
                            .recording_config
                            .sample_rate = Self::select_default_sample_rate(
                            &device.available_sample_rates,
                            device.default_config.as_ref().map(|c| c.sample_rate),
                        );
                    }
                }
                // Clamp recording channel count to selected device's max channels
                let rec_config = &self.measurement_state.recording_state.recording_config;
                if let Some(device) = input_devices
                    .iter()
                    .find(|d| d.name == rec_config.device_name)
                    && let Some(max_ch) =
                        device.default_config.as_ref().map(|c| c.channels as usize)
                    && self
                        .measurement_state
                        .recording_state
                        .recording_config
                        .num_channels
                        > max_ch
                {
                    self.measurement_state
                        .recording_state
                        .recording_config
                        .num_channels = max_ch;
                }
            }
        }
    }

    pub fn load_config(&mut self) -> Result<LayoutState, Box<dyn std::error::Error>> {
        use crate::config::Config;
        let config = Config::load()?;
        self.load_config_from(config)
    }

    /// Load configuration from an already-loaded Config (avoids duplicate disk read).
    pub fn load_config_from(
        &mut self,
        config: crate::config::Config,
    ) -> Result<LayoutState, Box<dyn std::error::Error>> {
        // Cache window geometry so save_config doesn't need to re-read from disk
        self.last_saved_geometry = Some(config.window_geometry.clone());

        // Restore directories
        self.library_state.library.directories = config.directories;
        self.install_external_plugin_runtime_sandbox();

        // Restore theme
        self.ui_state.theme_id = config.theme;
        self.ui_state.theme_mode_preference = config.theme_mode_preference;
        self.ui_state.accessibility_palette = config.accessibility_palette;
        self.ui_state.theme_accent_preference = config.theme_accent_preference;
        self.ui_state.community_theme_id = config.community_theme_id;
        if let Some(theme_id) = self.ui_state.community_theme_id {
            self.set_community_theme(theme_id);
        } else {
            self.apply_theme(config.theme);
        }
        self.ui_state.reduce_motion = config.reduce_motion;
        self.ui_state.density_mode = config.density_mode;

        // Restore language
        self.ui_state.language = config.language;
        self.ui_state.translations = Translations::for_language(config.language);

        // Restore keymap preset
        self.ui_state.keymap_preset = config.keymap_preset;

        // Restore font scale
        self.ui_state.font_scale = config.font_scale;

        // Restore font size bounds
        self.ui_state.min_font_size_px = config.min_font_size_px;
        self.ui_state.max_font_size_px = config.max_font_size_px;

        // Restore release channel
        self.ui_state.release_channel = config.release_channel;

        // Restore design language
        self.ui_state.design_language = config.design_language;

        // Restore plugin chassis theme state (rack default + overrides)
        self.plugin_state.rack_theme_state = config.rack_theme_state;

        // Build LayoutState from config
        let layout_state = LayoutState {
            queue_panel_ratio: config.panel_layout.queue_ratio,
            meters_panel_ratio: config.panel_layout.meters_ratio,
            queue_list_ratio: config.panel_layout.queue_list_ratio,
            lufs_panel_ratio: config.panel_layout.lufs_ratio,
            rack_detail_ratio: config.panel_layout.rack_detail_ratio,
            library_h_ratio: config.panel_layout.library_h_ratio,
            queue_h_ratio: config.panel_layout.queue_h_ratio,
            rack_h_ratio: config.panel_layout.rack_h_ratio,
            library_v_ratio: config.panel_layout.library_v_ratio,
            queue_v_ratio: config.panel_layout.queue_v_ratio,
            rack_v_ratio: config.panel_layout.rack_v_ratio,
            ..Default::default()
        };
        self.rack_detail_collapsed = config.panel_layout.rack_detail_ratio <= 0.05;

        // Restore volume and muted state
        // self.playback.volume = config.volume; // Always start at default (10%) per requirement
        self.playback.muted = config.muted;

        // Restore recording config
        if !config.recording_config.playback.device_name.is_empty() {
            self.measurement_state.recording_state.playback_config =
                config.recording_config.playback;
        }
        if !config.recording_config.recording.device_name.is_empty() {
            self.measurement_state.recording_state.recording_config =
                config.recording_config.recording;
        }

        self.measurement_state.recording_state.signal_type = config.recording_config.signal_type;
        self.measurement_state.recording_state.signal_duration_secs =
            config.recording_config.signal_duration_secs;
        self.measurement_state.recording_state.signal_level_db =
            if (config.recording_config.signal_level_db - -20.0).abs() < f32::EPSILON {
                DEFAULT_SIGNAL_LEVEL_DB
            } else {
                config.recording_config.signal_level_db
            };
        self.measurement_state.recording_state.mic_calibration_path =
            config.recording_config.mic_calibration_path.clone();
        // Migrate per-channel calibration paths
        let mut mic_cal_paths = config.recording_config.mic_calibration_paths;
        if mic_cal_paths.is_empty()
            && let Some(ref path) = config.recording_config.mic_calibration_path
        {
            mic_cal_paths = vec![Some(path.clone())];
        }
        self.measurement_state.recording_state.mic_calibration_paths = mic_cal_paths;
        self.measurement_state.recording_state.recording_directory =
            config.recording_config.recording_directory;
        self.measurement_state
            .recording_state
            .recording_base_directory = config.recording_config.recording_base_directory;

        // Reload calibration data if path exists (global)
        if let Some(ref path) = self.measurement_state.recording_state.mic_calibration_path
            && let Ok(content) = std::fs::read_to_string(path)
        {
            self.measurement_state.recording_state.mic_calibration_data =
                crate::app::types::CalibrationData::parse(&content);
        }

        // Reload per-channel calibration data
        self.measurement_state
            .recording_state
            .mic_calibration_data_per_channel = self
            .measurement_state
            .recording_state
            .mic_calibration_paths
            .iter()
            .map(|opt_path| {
                opt_path.as_ref().and_then(|path| {
                    std::fs::read_to_string(path)
                        .ok()
                        .and_then(|content| crate::app::types::CalibrationData::parse(&content))
                })
            })
            .collect();

        // Restore plugin presets path if we had a last loaded preset
        if let Some(preset_name) = config.last_loaded_plugin_preset {
            self.plugin_state.last_loaded_preset = Some(preset_name.clone());
            // Load the preset file
            let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
                log::warn!("Could not find presets directory, skipping preset restore");
                return Ok(layout_state);
            };
            match self
                .plugin_state
                .graph
                .load_from_file(&presets_dir, &preset_name)
            {
                Ok(warnings) => {
                    self.plugin_state.pending_plugin_update =
                        Some(crate::app::types::PluginUpdateType::Structural);
                    self.sync_spectrum_visible();
                    if warnings.is_empty() {
                        log::info!("Restored plugin preset: {}", preset_name);
                    } else {
                        log::warn!(
                            "Restored plugin preset '{}' with {} skipped plugin(s)",
                            preset_name,
                            warnings.len()
                        );
                        for w in &warnings {
                            log::warn!("  {}", w);
                        }
                        self.ui_state.toast_message =
                            Some(crate::app::ToastMessage::warning(format!(
                                "Preset '{}': {} plugin(s) skipped",
                                preset_name,
                                warnings.len()
                            )));
                    }
                }
                Err(e) => {
                    log::warn!("Could not restore preset '{}': {}", preset_name, e);
                }
            }
        }

        // Restore tutorial completed state
        self.tutorial_completed = config.tutorial_completed;
        self.seen_hints = config.seen_hints.clone();

        // Restore scanner thread count
        self.ui_state.scanner_threads = config.scanner_threads;
        if let Some(threads) = config.scanner_threads {
            self.scan_ctrl.set_num_threads(Some(threads as usize));
        }

        // Restore max CPU cores
        self.ui_state.max_cpu_cores = config.max_cpu_cores;

        // Restore the remote library identity associated with any local
        // database/cache content.
        self.remote.local_library_identity = config.remote_library_identity;

        // Restore non-secret native remote server records. Bearer tokens live
        // in platform credential stores keyed by each server's token key.
        match sotf_audio_player::config::load_remote_server_store() {
            Ok(store) => {
                self.remote.server_store = store;
                self.load_persisted_remote_server_tokens();
            }
            Err(e) => {
                log::warn!("Could not load remote server store: {e}");
            }
        }

        Ok(layout_state)
    }

    pub fn save_config(&mut self, layout: &LayoutState) -> Result<(), Box<dyn std::error::Error>> {
        self.save_config_with_geometry(layout, None)
    }

    /// Save config with optional window geometry
    pub fn save_config_with_geometry(
        &mut self,
        layout: &LayoutState,
        window_geometry: Option<crate::config::WindowGeometry>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::config::{Config, PanelLayout};
        let geometry =
            window_geometry.unwrap_or_else(|| self.last_saved_geometry.clone().unwrap_or_default());
        // Update cache so future saves without geometry don't need disk I/O
        self.last_saved_geometry = Some(geometry.clone());
        let config = Config {
            directories: self.library_state.library.directories.clone(),
            last_loaded_plugin_preset: self.plugin_state.last_loaded_preset.clone(),
            theme: self.ui_state.theme_id,
            theme_mode_preference: self.ui_state.theme_mode_preference.clone(),
            accessibility_palette: self.ui_state.accessibility_palette,
            theme_accent_preference: self.ui_state.theme_accent_preference,
            community_theme_id: self.ui_state.community_theme_id,
            reduce_motion: self.ui_state.reduce_motion,
            density_mode: self.ui_state.density_mode,
            language: self.ui_state.language,
            keymap_preset: self.ui_state.keymap_preset,
            panel_layout: PanelLayout {
                queue_ratio: layout.queue_panel_ratio,
                meters_ratio: layout.meters_panel_ratio,
                queue_list_ratio: layout.queue_list_ratio,
                lufs_ratio: layout.lufs_panel_ratio,
                rack_detail_ratio: layout.rack_detail_ratio,
                // 3-Panel Layout ratios
                library_h_ratio: layout.library_h_ratio,
                queue_h_ratio: layout.queue_h_ratio,
                rack_h_ratio: layout.rack_h_ratio,
                library_v_ratio: layout.library_v_ratio,
                queue_v_ratio: layout.queue_v_ratio,
                rack_v_ratio: layout.rack_v_ratio,
            },
            window_geometry: geometry,
            volume: self.playback.volume,
            muted: self.playback.muted,
            recording_config: crate::app::config::RecordingConfigState {
                playback: self
                    .measurement_state
                    .recording_state
                    .playback_config
                    .clone(),
                recording: self
                    .measurement_state
                    .recording_state
                    .recording_config
                    .clone(),
                signal_type: self.measurement_state.recording_state.signal_type,
                signal_duration_secs: self.measurement_state.recording_state.signal_duration_secs,
                signal_level_db: self.measurement_state.recording_state.signal_level_db,
                mic_calibration_path: self
                    .measurement_state
                    .recording_state
                    .mic_calibration_path
                    .clone(),
                mic_calibration_paths: self
                    .measurement_state
                    .recording_state
                    .mic_calibration_paths
                    .clone(),
                recording_directory: self
                    .measurement_state
                    .recording_state
                    .recording_directory
                    .clone(),
                recording_base_directory: self
                    .measurement_state
                    .recording_state
                    .recording_base_directory
                    .clone(),
            },
            font_scale: self.ui_state.font_scale,
            release_channel: self.ui_state.release_channel,
            scanner_threads: self.ui_state.scanner_threads,
            max_cpu_cores: self.ui_state.max_cpu_cores,
            min_font_size_px: self.ui_state.min_font_size_px,
            max_font_size_px: self.ui_state.max_font_size_px,
            tutorial_completed: self.tutorial_completed,
            seen_hints: self.seen_hints.clone(),
            design_language: self.ui_state.design_language.clone(),
            rack_theme_state: self.plugin_state.rack_theme_state.clone(),
            remote_library_identity: self.remote.local_library_identity.clone(),
        };
        config.save()?;
        Ok(())
    }

    pub fn get_device_max_channels(&self) -> Option<usize> {
        self.audio_device_state
            .output_devices
            .get(self.audio_device_state.selected_output_device_index)
            .and_then(|device| {
                device
                    .supported_configs
                    .iter()
                    .map(|c| c.channels as usize)
                    .max()
            })
    }

    /// Check and dismiss expired toast messages
    pub fn update_toast(&mut self) {
        if let Some(ref toast) = self.ui_state.toast_message
            && toast.should_dismiss()
        {
            self.ui_state.toast_message = None;
        }
    }

    /// Dismiss the current toast message manually
    pub fn dismiss_toast(&mut self) {
        self.ui_state.toast_message = None;
    }

    /// Cycle to the next theme
    pub fn next_theme(&mut self) {
        let theme_id = self.ui_state.theme_id.next();
        self.set_theme(theme_id);
    }

    /// Cycle to the next language
    pub fn next_language(&mut self) {
        self.ui_state.language = self.ui_state.language.next();
        self.ui_state.translations = Translations::for_language(self.ui_state.language);
    }

    /// Set a specific theme
    pub fn set_theme(&mut self, theme_id: ThemeId) {
        self.ui_state.community_theme_id = None;
        self.apply_theme(theme_id);
        self.ui_state.theme_mode_preference = theme_id.mode_preference();
        self.ui_state.accessibility_palette = theme_id.accessibility_palette();
    }

    pub fn set_community_theme(&mut self, theme_id: CommunityThemeId) {
        let manifest = theme_id.manifest();
        self.ui_state.community_theme_id = Some(theme_id);
        self.ui_state.theme_mode_preference = manifest.preferred_mode;
        self.ui_state.accessibility_palette = manifest.accessibility;
        self.apply_community_theme(theme_id);
    }

    pub fn set_community_theme_from_json(&mut self, json: &str) -> Result<(), String> {
        let bundle = CommunityThemeBundle::from_json(json)
            .map_err(|error| format!("failed to parse community theme JSON: {error}"))?;
        let theme = Theme::from_community_bundle(&bundle)?
            .with_accent_preference(self.ui_state.theme_accent_preference);

        self.ui_state.community_theme_id = CommunityThemeId::from_id(&bundle.manifest.id);
        self.ui_state.theme_id = ThemeId::for_appearance(theme.appearance());
        self.ui_state.theme = theme;
        self.ui_state.theme_mode_preference = bundle.manifest.preferred_mode;
        self.ui_state.accessibility_palette = bundle.manifest.accessibility;
        Ok(())
    }

    pub fn set_community_theme_json_draft(&mut self, json: impl Into<String>) {
        self.ui_state.community_theme_json_draft = json.into();
    }

    pub fn apply_community_theme_json_draft(&mut self) -> Result<(), String> {
        let json = self.ui_state.community_theme_json_draft.clone();
        let trimmed = json.trim();
        if trimmed.is_empty() {
            return Err("community theme JSON is empty".to_string());
        }
        self.set_community_theme_from_json(trimmed)
    }

    /// Set the light/dark mode policy and apply its current effective theme.
    pub fn set_theme_mode_preference(&mut self, preference: ThemeModePreference) {
        self.set_theme_mode_preference_with_system(preference, ThemeAppearance::Dark);
    }

    /// Set the light/dark mode policy using the supplied system appearance.
    pub fn set_theme_mode_preference_with_system(
        &mut self,
        preference: ThemeModePreference,
        system_appearance: ThemeAppearance,
    ) {
        self.set_theme_mode_preference_at_minutes(
            preference,
            system_appearance,
            current_minutes_after_midnight(),
        );
    }

    pub fn set_theme_mode_preference_at_minutes(
        &mut self,
        preference: ThemeModePreference,
        system_appearance: ThemeAppearance,
        minutes_after_midnight: u16,
    ) {
        let appearance = preference.resolve(system_appearance, minutes_after_midnight);
        self.ui_state.theme_mode_preference = preference;
        self.ui_state.community_theme_id = None;
        self.apply_theme(ThemeId::for_accessibility_palette(
            self.ui_state.accessibility_palette,
            appearance,
        ));
    }

    pub fn theme_schedule(&self) -> ThemeSchedule {
        match &self.ui_state.theme_mode_preference {
            ThemeModePreference::Scheduled { schedule } => *schedule,
            _ => ThemeSchedule::default(),
        }
    }

    pub fn set_theme_schedule(&mut self, schedule: ThemeSchedule) {
        self.set_theme_schedule_with_system(schedule, ThemeAppearance::Dark);
    }

    pub fn set_theme_schedule_with_system(
        &mut self,
        schedule: ThemeSchedule,
        system_appearance: ThemeAppearance,
    ) {
        self.set_theme_mode_preference_with_system(
            ThemeModePreference::Scheduled { schedule },
            system_appearance,
        );
    }

    pub fn set_theme_schedule_at_minutes(
        &mut self,
        schedule: ThemeSchedule,
        system_appearance: ThemeAppearance,
        minutes_after_midnight: u16,
    ) {
        self.set_theme_mode_preference_at_minutes(
            ThemeModePreference::Scheduled { schedule },
            system_appearance,
            minutes_after_midnight,
        );
    }

    pub fn refresh_scheduled_theme(&mut self) -> bool {
        self.refresh_scheduled_theme_at_minutes(current_minutes_after_midnight())
    }

    pub fn refresh_scheduled_theme_at_minutes(&mut self, minutes_after_midnight: u16) -> bool {
        let ThemeModePreference::Scheduled { schedule } = &self.ui_state.theme_mode_preference
        else {
            return false;
        };
        let appearance = schedule.resolve_at_minutes(minutes_after_midnight);
        let theme_id =
            ThemeId::for_accessibility_palette(self.ui_state.accessibility_palette, appearance);

        if self.ui_state.community_theme_id.is_none() && self.ui_state.theme_id == theme_id {
            false
        } else {
            self.ui_state.community_theme_id = None;
            self.apply_theme(theme_id);
            true
        }
    }

    /// Set an accessibility palette and apply the matching theme.
    pub fn set_accessibility_palette(&mut self, palette: AccessibilityPalette) {
        self.set_accessibility_palette_with_system(palette, ThemeAppearance::Dark);
    }

    /// Set an accessibility palette using the supplied system appearance.
    pub fn set_accessibility_palette_with_system(
        &mut self,
        palette: AccessibilityPalette,
        system_appearance: ThemeAppearance,
    ) {
        let appearance = self.resolved_theme_appearance_with_system(system_appearance);
        self.ui_state.accessibility_palette = palette;
        self.ui_state.community_theme_id = None;
        self.apply_theme(ThemeId::for_accessibility_palette(palette, appearance));
    }

    pub fn set_reduce_motion(&mut self, reduce_motion: bool) {
        self.ui_state.reduce_motion = reduce_motion;
    }

    pub fn set_theme_accent_preference(&mut self, preference: ThemeAccentPreference) {
        self.ui_state.theme_accent_preference = preference;
        self.apply_selected_theme();
    }

    pub fn resolved_theme_appearance(&self) -> ThemeAppearance {
        self.resolved_theme_appearance_with_system(ThemeAppearance::Dark)
    }

    pub fn resolved_theme_appearance_with_system(
        &self,
        system_appearance: ThemeAppearance,
    ) -> ThemeAppearance {
        self.ui_state
            .theme_mode_preference
            .resolve(system_appearance, current_minutes_after_midnight())
    }

    pub fn theme_transition_duration_ms(&self) -> u16 {
        ThemeTransition::default().effective_duration_ms(self.ui_state.reduce_motion)
    }

    fn apply_theme(&mut self, theme_id: ThemeId) {
        self.ui_state.theme_id = theme_id;
        self.ui_state.theme =
            Theme::from_id(theme_id).with_accent_preference(self.ui_state.theme_accent_preference);
    }

    fn apply_community_theme(&mut self, theme_id: CommunityThemeId) {
        let theme = theme_id
            .theme()
            .with_accent_preference(self.ui_state.theme_accent_preference);
        self.ui_state.theme_id = ThemeId::for_appearance(theme.appearance());
        self.ui_state.theme = theme;
    }

    fn apply_selected_theme(&mut self) {
        if let Some(theme_id) = self.ui_state.community_theme_id {
            self.apply_community_theme(theme_id);
        } else {
            self.apply_theme(self.ui_state.theme_id);
        }
    }

    /// Set a specific language
    pub fn set_language(&mut self, language: Language) {
        self.ui_state.language = language;
        self.ui_state.translations = Translations::for_language(language);
    }

    /// Cycle to the next keymap preset
    pub fn next_keymap_preset(&mut self) {
        self.ui_state.keymap_preset = self.ui_state.keymap_preset.next();
    }

    /// Set a specific keymap preset
    pub fn set_keymap_preset(&mut self, preset: KeymapPreset) {
        self.ui_state.keymap_preset = preset;
    }

    /// Invalidate cached library statistics. Call this when the library changes
    /// (albums added/removed, tracks modified, etc.)
    pub fn invalidate_library_stats(&mut self) {
        self.library_stats.valid = false;
    }

    /// Compute library statistics from scratch.
    /// This is expensive - O(n) over all albums and tracks.
    pub fn compute_library_stats_static(albums: &[sotf_audio_player::Album]) -> LibraryStats {
        LibraryStats::compute(albums)
    }

    /// Get library statistics, returning currently cached values.
    /// If stats are invalid, they should be updated via compute_library_stats_async.
    pub fn get_library_stats(&self) -> &LibraryStats {
        &self.library_stats
    }

    // ============== Dynamic Text Truncation ==============

    /// Helper to calculate max characters based on panel width and text properties.
    /// Formula: ((panel_width - fixed_offset).max(min_width) / char_width).clamp(min_chars, max_chars)
    fn calculate_max_chars(
        panel_width: f32,
        fixed_offset: f32,
        min_width: f32,
        char_width: f32,
        min_chars: usize,
        max_chars: usize,
    ) -> usize {
        let available_width = (panel_width - fixed_offset).max(min_width);
        ((available_width / char_width) as usize).clamp(min_chars, max_chars)
    }

    /// Calculate the width of the queue list panel.
    fn queue_list_width(&self, layout: &LayoutState) -> f32 {
        self.ui_state.window_width * layout.queue_list_ratio
    }

    /// Calculate the width of the center panel (remaining after queue list and meters).
    pub fn center_panel_width(&self, layout: &LayoutState) -> f32 {
        let center_ratio = 1.0 - layout.queue_list_ratio - layout.meters_panel_ratio;
        self.ui_state.window_width * center_ratio
    }

    /// Maximum characters for queue list album title (text_sm ~7px).
    pub fn max_chars_queue_list_title(&self, layout: &LayoutState) -> usize {
        Self::calculate_max_chars(self.queue_list_width(layout), 56.0, 50.0, 7.0, 15, 100)
    }

    /// Maximum characters for queue list artist (text_xs ~6px).
    pub fn max_chars_queue_list_artist(&self, layout: &LayoutState) -> usize {
        Self::calculate_max_chars(self.queue_list_width(layout), 56.0, 50.0, 6.0, 15, 120)
    }

    /// Maximum characters for Now Playing album title (text_lg ~9px).
    pub fn max_chars_now_playing_title(&self, layout: &LayoutState) -> usize {
        Self::calculate_max_chars(self.center_panel_width(layout), 60.0, 0.0, 9.0, 6, 150)
    }

    /// Maximum characters for Now Playing artist (text_sm ~7px).
    pub fn max_chars_now_playing_artist(&self, layout: &LayoutState) -> usize {
        Self::calculate_max_chars(self.center_panel_width(layout), 60.0, 0.0, 7.0, 6, 180)
    }

    /// Maximum characters for track titles (text_sm ~7px).
    pub fn max_chars_track_title(&self, layout: &LayoutState) -> usize {
        Self::calculate_max_chars(self.center_panel_width(layout), 120.0, 100.0, 7.0, 20, 200)
    }
}
