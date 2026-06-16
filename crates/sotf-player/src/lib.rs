#![allow(clippy::collapsible_if)]
pub mod album_art_generation;
pub mod audio_device;
pub mod autoeq;
pub mod bliss;
pub mod config;
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
pub mod controllers;
#[cfg(feature = "dev-api")]
pub mod dev_api_fixtures;
pub mod database;
pub mod federation_config;
pub mod federation_scan;
pub mod headphone_eq_types;
pub mod lan_discovery;
pub mod level_meter;
pub mod library;
pub mod library_scanner;
pub mod metadata;
pub mod peq_filter;
pub mod play_tracker;
pub mod player;
pub mod plugin_categories;
pub mod plugin_graph;
pub mod queue;
pub mod recording_types;
pub mod room_eq_types;
pub mod spinorama_eq_types;
// plugins module is now in engine
pub mod playlist_io;
pub mod recommendation;
pub mod replay_gain_scanner;
// Backward compatibility alias
pub use autoeq as room_eq;
pub mod library_stats;
pub mod security;
pub mod server;
pub mod sotf_api_client;
pub mod sotf_remote;
pub mod sotf_server_event;
pub mod streams;
pub mod ui_models;
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
    Playlist, PlaylistEntry, TRACK_WAVEFORM_SAMPLES, Track, TrackWaveform, group_and_merge_albums,
};
pub use library_scanner::{LibraryScanMessage, LibraryScanner};
pub use library_stats::{LibraryStats, format_channel_count};
pub use metadata::{
    AlbumMetadataSidecar, MetadataAffectedFile, MetadataController, MetadataEditPreview,
    MetadataError, MetadataImportCandidate, MetadataPatch, MetadataProviderConfig,
    MetadataServicesConfig, MetadataTarget, MusicBrainzProvider,
};
pub use play_tracker::PlayTracker;
pub use player::{PlaybackState, Player};
pub use plugin_graph::{
    ConnectionDrag, GraphConnection, GraphNodeId, GraphSelection, NodeDrag, NodePosition,
    PluginGraph, PluginGraphNode, SpecialNode, SpecialNodeType,
};
pub use queue::{Queue, QueueItem};
pub use sotf_remote::{
    SotfRemoteAuthToken, SotfRemoteConnection, SotfRemoteConnectionInfo, SotfRemoteServer,
    SotfRemoteServerStore, SotfRemoteSnapshot, SotfRemoteTransportCommand,
};
pub use streams::{
    SavedStream, SavedStreamStore, StreamStoreError, StreamValidationError, load_saved_streams,
    parse_service_stream_reference, save_saved_streams, validate_stream_url,
};
// Re-export plugins from engine
pub use replay_gain_scanner::{
    AlbumGainPhase, ReplayGainMode, ReplayGainScanManager, ReplayGainScanner, ScanMessage,
};
pub use sotf_audio::plugins::{
    ChannelConflict,
    EQFilter,
    Plugin,
    PluginChain,
    PluginSettings,
    PluginType,
    ReleaseChannel,
    // Upmixer decomposed settings
    UpmixerAmbientAnalysisSettings,
    UpmixerBypassSettings,
    UpmixerDecorrelationSettings,
    UpmixerDialogueSettings,
    UpmixerGainSettings,
    UpmixerHeightSettings,
    UpmixerLfeSettings,
    UpmixerOutputSettings,
    UpmixerSubharmonicSettings,
    // Matrix helper functions
    apply_matrix_preset,
    available_matrix_presets,
    db_to_linear,
    detect_matrix_preset,
    get_channel_label,
    get_channel_label_from_config,
    linear_to_db_string,
    preset_file_to_path_config_json,
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

// Re-export controllers
pub use controllers::{
    LibraryController, PlaybackController, PlaylistController, PluginController,
    PluginUpdateEffect, QueueController, QueuePlaybackEffect, ScanController, get_param_count,
    param_index_to_engine_param,
};
