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
pub mod config;
pub mod database;
pub mod library;
pub mod library_scanner;
pub mod player;
pub mod plugins;
pub mod replay_gain_scanner;
pub mod security;
pub mod waveform_scanner;

// Re-export commonly used types
pub use config::AppConfig;
pub use database::MusicDatabase;
pub use library::{
    Album, AlbumChannelType, DirectoryInfo, MusicLibrary, Playlist, PlaylistEntry, Track,
};
pub use library_scanner::{LibraryScanMessage, LibraryScanner};
pub use player::{PlaybackState, Player};
pub use plugins::{EQFilter, Plugin, PluginChain, PluginSettings, PluginType};
pub use replay_gain_scanner::{ReplayGainScanManager, ReplayGainScanner, ScanMessage};
pub use sotf_audio::replaygain::ReplayGainInfo;
pub use waveform_scanner::{WaveformScanManager, WaveformScanMessage, WaveformScanner};

// Re-export measurement functionality
pub use sotf_audio::signal_recorder;

// Re-export autoeq_iir types needed by TUI
pub use autoeq_iir::BiquadFilterType;

// Re-export analyzer types
pub use sotf_plugins::{LoudnessData, LoudnessInfo, SpectrumData, SpectrumInfo};
