#![allow(clippy::collapsible_if)]
/// Shared business logic for SOTF audio players (TUI, GPUI, etc.)
///
/// This crate provides:
/// - Music library management (`library`)
/// - Database persistence (`database`)
/// - Configuration (`config`)
/// - Plugin chain management (`plugins`)
/// - Player wrapper (`player`)
/// - ReplayGain scanning (`replay_gain_scanner`)
/// - Waveform scanning (`waveform_scanner`)
/// - Bliss audio analysis (`bliss`)
/// - Music recommendation engine (`recommendation`)
pub mod audio_device;
pub mod autoeq;
pub mod bliss;
pub mod config;
pub mod database;
pub mod headphone_eq_types;
pub mod level_meter;
pub mod library;
pub mod library_scanner;
pub mod play_tracker;
pub mod player;
pub mod plugin_graph;
pub mod queue;
pub mod recording_types;
pub mod room_eq_types;
pub mod spinorama_eq_types;
// plugins module is now in engine
pub mod recommendation;
pub mod replay_gain_scanner;
// Backward compatibility alias
pub use autoeq as room_eq;
pub mod library_stats;
pub mod security;
pub mod ui_params;
pub mod waveform_scanner;

// Re-export commonly used types
pub use audio_device::{AudioOutputDeviceState, is_virtual_device};
pub use bliss::{BlissAnalysis, BlissScanManager, BlissScanMessage, BlissScanner};
pub use config::AppConfig;
pub use database::MusicDatabase;
pub use level_meter::{ChannelGroup, ChannelInfo, build_level_meter_groups};
pub use library::{
    Album, AlbumChannelType, ChannelFilter, DirectoryInfo, LibrarySortOrder, MusicLibrary,
    Playlist, PlaylistEntry, Track,
};
pub use library_scanner::{LibraryScanMessage, LibraryScanner};
pub use library_stats::LibraryStats;
pub use play_tracker::PlayTracker;
pub use player::{PlaybackState, Player};
pub use plugin_graph::{
    ConnectionDrag, GraphConnection, GraphNodeId, GraphSelection, NodeDrag, NodePosition,
    PluginGraph, PluginGraphNode, SpecialNode, SpecialNodeType,
};
pub use queue::{Queue, QueueItem};
// Re-export plugins from engine
pub use replay_gain_scanner::{
    AlbumGainPhase, ReplayGainMode, ReplayGainScanManager, ReplayGainScanner, ScanMessage,
};
pub use sotf_audio::plugins::{
    EQFilter,
    Plugin,
    PluginChain,
    PluginSettings,
    PluginType,
    ReleaseChannel,
    // Matrix helper functions
    apply_matrix_preset,
    available_matrix_presets,
    db_to_linear,
    detect_matrix_preset,
    get_channel_label,
    get_channel_label_from_config,
    linear_to_db_string,
    resize_matrix,
};
pub use sotf_audio::replaygain::ReplayGainInfo;
pub use waveform_scanner::{WaveformScanManager, WaveformScanMessage, WaveformScanner};

// Re-export measurement functionality
pub use sotf_audio::signal_recorder;

// Re-export math_audio_iir_fir types needed by TUI
pub use math_audio_iir_fir::BiquadFilterType;

// Re-export analyzer types
pub use sotf_plugins::{LoudnessData, LoudnessInfo, SpectrumData, SpectrumInfo};

// Re-export parameter specifications for UI components
pub use sotf_plugins::param_specs;
