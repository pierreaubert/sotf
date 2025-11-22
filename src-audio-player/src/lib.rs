/// Shared business logic for SOTF audio players (TUI, GPUI, etc.)
///
/// This crate provides:
/// - Music library management (`library`)
/// - Database persistence (`database`)
/// - Configuration (`config`)
/// - Plugin chain management (`plugins`)
/// - Player wrapper (`player`)
/// - ReplayGain scanning (`replay_gain_scanner`)

pub mod config;
pub mod database;
pub mod library;
pub mod player;
pub mod plugins;
pub mod replay_gain_scanner;

// Re-export commonly used types
pub use config::Config;
pub use database::MusicDatabase;
pub use library::{Album, AlbumChannelType, DirectoryInfo, MusicLibrary, Track};
pub use player::{PlaybackState, Player};
pub use plugins::{PluginChain, PluginSettings, PluginType};
pub use replay_gain_scanner::{ReplayGainInfo, ReplayGainScanner};
