use std::path::PathBuf;

/// ReplayGain application mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayGainMode {
    Track,
    Album,
}

/// Message sent by scanner thread
#[derive(Debug, Clone)]
pub enum ScanMessage {
    /// Started scanning a track
    Started { path: PathBuf },
    /// Successfully scanned a track
    Success { path: PathBuf, gain: f64, peak: f64 },
    /// Failed to scan a track
    Error { path: PathBuf, error: String },
    /// Scanning complete
    Complete {
        total: usize,
        succeeded: usize,
        failed: usize,
    },
}

/// Progress state for album gain scanning
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlbumGainPhase {
    /// Not running
    #[default]
    Idle,
    /// Computing album gains
    Scanning,
    /// Finished
    Done,
}

/// Message from the album gain background thread
#[derive(Debug)]
pub(super) enum AlbumGainMessage {
    Progress { albums_done: usize },
    Complete { succeeded: usize, failed: usize },
}
