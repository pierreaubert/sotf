#![allow(clippy::collapsible_if)]
pub mod autoeq;
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
pub mod bliss;
pub mod config;
pub mod database;
pub mod library;
pub mod library_scanner;
pub mod player;
pub mod plugin_graph;
// plugins module is now in sotf-audio-engine
pub mod recommendation;
pub mod replay_gain_scanner;
// Backward compatibility alias
pub use autoeq as room_eq;
pub mod security;
pub mod waveform_scanner;

// Re-export commonly used types
pub use bliss::{BlissAnalysis, BlissScanManager, BlissScanMessage, BlissScanner};
pub use config::AppConfig;
pub use database::MusicDatabase;
pub use library::{
    Album, AlbumChannelType, DirectoryInfo, MusicLibrary, Playlist, PlaylistEntry, Track,
};
pub use library_scanner::{LibraryScanMessage, LibraryScanner};
pub use player::{PlaybackState, Player};
pub use plugin_graph::{
    ConnectionDrag, GraphConnection, GraphNodeId, GraphSelection, NodeDrag, NodePosition,
    PluginGraph, PluginGraphNode, SpecialNode, SpecialNodeType,
};
// Re-export plugins from sotf-audio-engine
pub use replay_gain_scanner::{ReplayGainScanManager, ReplayGainScanner, ScanMessage};
pub use sotf_audio::plugins::{
    EQFilter,
    Plugin,
    PluginChain,
    PluginSettings,
    PluginType,
    // Matrix helper functions
    apply_matrix_preset,
    db_to_linear,
    detect_matrix_preset,
    get_channel_label,
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
