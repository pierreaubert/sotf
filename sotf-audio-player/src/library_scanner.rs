//! Background library scanner for non-blocking UI updates

use crate::library::MusicLibrary;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

/// Message sent by the library scanner thread
#[derive(Debug, Clone)]
pub enum LibraryScanMessage {
    /// Progress update during scanning
    Progress { tracks: usize, albums: usize },
    /// Scanning completed successfully
    Complete {
        tracks: usize,
        albums: usize,
    },
    /// Scanning failed
    Error { message: String },
}

/// Background library scanner
pub struct LibraryScanner {
    _worker: JoinHandle<()>,
    message_rx: Arc<Mutex<Receiver<LibraryScanMessage>>>,
}

impl std::fmt::Debug for LibraryScanner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryScanner")
            .field("_worker", &"JoinHandle<()>")
            .field("message_rx", &"Arc<Mutex<Receiver<LibraryScanMessage>>>")
            .finish()
    }
}

impl LibraryScanner {
    /// Start a new background library scan (incremental - only new/modified files)
    ///
    /// The scan runs in a background thread and sends progress updates via messages.
    /// The scan results are saved to the database, so after completion the caller
    /// should reload the library from the database.
    pub fn start(directories: Vec<PathBuf>) -> Self {
        Self::start_with_options(directories, false)
    }

    /// Start a new background library scan with force option
    ///
    /// If `force` is true, all files will be rescanned regardless of modification time.
    /// ReplayGain values are preserved in the database (not overwritten).
    pub fn start_force(directories: Vec<PathBuf>) -> Self {
        Self::start_with_options(directories, true)
    }

    /// Start a new background library scan with options
    fn start_with_options(directories: Vec<PathBuf>, force: bool) -> Self {
        let (message_tx, message_rx) = mpsc::channel::<LibraryScanMessage>();

        let worker = thread::spawn(move || {
            if force {
                log::info!("[LibraryScanner] Starting FORCE background scan (all files)");
            } else {
                log::info!("[LibraryScanner] Starting incremental background scan");
            }

            // Create a new library with database for scanning
            let mut library = match MusicLibrary::with_database() {
                Ok(lib) => lib,
                Err(e) => {
                    log::error!("[LibraryScanner] Failed to create library: {}", e);
                    let _ = message_tx.send(LibraryScanMessage::Error {
                        message: format!("Failed to create library: {}", e),
                    });
                    return;
                }
            };

            // Add directories to scan
            for dir in directories {
                if let Err(e) = library.add_directory(dir.clone()) {
                    log::warn!(
                        "[LibraryScanner] Failed to add directory {}: {}",
                        dir.display(),
                        e
                    );
                }
            }

            // Track progress
            let message_tx_clone = message_tx.clone();
            let last_track_count = Arc::new(Mutex::new(0usize));

            // Run the scan with progress callback
            // Use incremental=true (skip unchanged files) unless force is set
            let result = library.scan_incremental_with_progress(!force, move |tracks, albums| {
                // Update UI every 500 tracks
                let should_update = {
                    let mut last = last_track_count.lock().unwrap();
                    if tracks >= *last + 500 {
                        *last = tracks;
                        true
                    } else {
                        false
                    }
                };

                if should_update {
                    let _ = message_tx_clone.send(LibraryScanMessage::Progress { tracks, albums });
                }
            });

            match result {
                Ok(()) => {
                    let track_count: usize =
                        library.albums.iter().map(|a| a.tracks.len()).sum();
                    let album_count = library.albums.len();

                    log::info!(
                        "[LibraryScanner] Scan complete: {} tracks in {} albums",
                        track_count,
                        album_count
                    );

                    let _ = message_tx.send(LibraryScanMessage::Complete {
                        tracks: track_count,
                        albums: album_count,
                    });
                }
                Err(e) => {
                    log::error!("[LibraryScanner] Scan failed: {}", e);
                    let _ = message_tx.send(LibraryScanMessage::Error {
                        message: format!("Scan failed: {}", e),
                    });
                }
            }
        });

        Self {
            _worker: worker,
            message_rx: Arc::new(Mutex::new(message_rx)),
        }
    }

    /// Try to receive a message without blocking
    pub fn try_recv(&self) -> Option<LibraryScanMessage> {
        self.message_rx.lock().ok()?.try_recv().ok()
    }
}
