//! Background library scanner for non-blocking UI updates

use crate::library::MusicLibrary;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

/// Message sent by the library scanner thread
#[derive(Debug, Clone)]
pub enum LibraryScanMessage {
    /// Progress update during scanning
    Progress { tracks: usize, albums: usize },
    /// Scanning completed successfully
    Complete { tracks: usize, albums: usize },
    /// Scanning failed
    Error { message: String },
}

/// Background library scanner
pub struct LibraryScanner {
    _worker: JoinHandle<()>,
    message_rx: Arc<Mutex<Receiver<LibraryScanMessage>>>,
    cancellation_token: Arc<AtomicBool>,
    pause_flag: Option<Arc<AtomicBool>>,
}

impl std::fmt::Debug for LibraryScanner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryScanner")
            .field("_worker", &"JoinHandle<()>")
            .field("message_rx", &"Arc<Mutex<Receiver<LibraryScanMessage>>>")
            .field("cancellation_token", &self.cancellation_token)
            .field("pause_flag", &self.pause_flag)
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
        Self::start_with_options(directories, false, None)
    }

    /// Start with a pause flag that suspends scanning during playback
    pub fn start_with_pause(directories: Vec<PathBuf>, pause_flag: Arc<AtomicBool>) -> Self {
        Self::start_with_options(directories, false, Some(pause_flag))
    }

    /// Start a new background library scan with force option
    ///
    /// If `force` is true, all files will be rescanned regardless of modification time.
    /// ReplayGain values are preserved in the database (not overwritten).
    pub fn start_force(directories: Vec<PathBuf>) -> Self {
        Self::start_with_options(directories, true, None)
    }

    /// Start a force scan with a pause flag
    pub fn start_force_with_pause(directories: Vec<PathBuf>, pause_flag: Arc<AtomicBool>) -> Self {
        Self::start_with_options(directories, true, Some(pause_flag))
    }

    /// Start a new background library scan with options
    fn start_with_options(
        directories: Vec<PathBuf>,
        force: bool,
        pause_flag: Option<Arc<AtomicBool>>,
    ) -> Self {
        let (message_tx, message_rx) = mpsc::channel::<LibraryScanMessage>();
        let cancellation_token = Arc::new(AtomicBool::new(false));
        let cancellation_token_clone = cancellation_token.clone();

        let pause_flag_clone = pause_flag.clone();

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
            let scan_token = cancellation_token_clone.clone();

            // Run the scan with progress callback and pause support
            // Use incremental=true (skip unchanged files) unless force is set
            let result = library.scan_incremental_with_progress_and_pause(
                !force,
                Some(scan_token),
                pause_flag_clone,
                &mut move |tracks, albums| {
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
                        let _ =
                            message_tx_clone.send(LibraryScanMessage::Progress { tracks, albums });
                    }
                },
            );

            match result {
                Ok(()) => {
                    let track_count: usize = library.albums.iter().map(|a| a.tracks.len()).sum();
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
            cancellation_token,
            pause_flag,
        }
    }

    /// Cancel the ongoing scan
    pub fn cancel(&self) {
        log::info!("[LibraryScanner] Cancelling scan...");
        self.cancellation_token.store(true, Ordering::Relaxed);
    }

    /// Try to receive a message without blocking
    pub fn try_recv(&self) -> Option<LibraryScanMessage> {
        self.message_rx.lock().ok()?.try_recv().ok()
    }
}
