use super::super::ui::LayoutState;
use super::App;
use crate::app::player_handle::PlayerHandle;
use crate::app::state::library::LibraryEvent;
use crate::app::types::{ChannelGroup, MeterDisplayMode};
use gpui::Entity;
use gpui_ui_kit::workflow::NodeId;
use std::collections::HashMap;

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
pub struct RemoteAlbumQueueCommandResult {
    pub server_id: String,
    pub album_title: String,
    pub play_now: bool,
    pub result: Result<sotf_audio_player::sotf_api_client::SotfApiQueueEditResponse, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct RemoteAlbumCacheKey {
    pub(super) server_id: String,
    pub(super) library_version: u64,
    pub(super) album_id: String,
}

/// GPUI-compatible state wrapper
pub struct AppState {
    pub app: App,
    pub layout: Entity<LayoutState>,
    pub player: PlayerHandle,
}
