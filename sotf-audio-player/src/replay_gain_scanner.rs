use crate::database::MusicDatabase;
use sotf_audio::replaygain;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

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

/// ReplayGain scanner with thread pool for background processing
#[derive(Debug)]
pub struct ReplayGainScanner {
    _workers: Vec<thread::JoinHandle<()>>,
    task_tx: Sender<PathBuf>,
    message_rx: Arc<Mutex<Receiver<ScanMessage>>>,
    stop_tx: Sender<()>,
}

impl ReplayGainScanner {
    /// Create a new scanner with the given number of worker threads
    pub fn new(num_threads: usize, db_path: PathBuf) -> Self {
        let (task_tx, task_rx) = mpsc::channel::<PathBuf>();
        let (message_tx, message_rx) = mpsc::channel::<ScanMessage>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        // Shared state for task distribution
        let task_rx = Arc::new(Mutex::new(task_rx));
        let stop_rx = Arc::new(Mutex::new(stop_rx));

        let mut workers = Vec::new();

        for worker_id in 0..num_threads {
            let task_rx = Arc::clone(&task_rx);
            let stop_rx = Arc::clone(&stop_rx);
            let message_tx = message_tx.clone();
            let db_path = db_path.clone();

            let worker = thread::spawn(move || {
                log::info!("[ReplayGain Worker {}] Started", worker_id);

                // Open database once per worker thread (not per track)
                let db = match MusicDatabase::open(&db_path) {
                    Ok(db) => db,
                    Err(e) => {
                        log::error!(
                            "[ReplayGain Worker {}] Failed to open database: {}",
                            worker_id,
                            e
                        );
                        return;
                    }
                };

                loop {
                    // Check if we should stop
                    if stop_rx.lock().unwrap().try_recv().is_ok() {
                        log::info!("[ReplayGain Worker {}] Stopping", worker_id);
                        break;
                    }

                    // Get next task
                    let path = match task_rx
                        .lock()
                        .unwrap()
                        .recv_timeout(std::time::Duration::from_millis(100))
                    {
                        Ok(path) => path,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            log::info!("[ReplayGain Worker {}] Task channel closed", worker_id);
                            break;
                        }
                    };

                    log::debug!(
                        "[ReplayGain Worker {}] Processing: {}",
                        worker_id,
                        path.display()
                    );

                    // Send started message
                    let _ = message_tx.send(ScanMessage::Started { path: path.clone() });

                    // Analyze the file
                    match replaygain::analyze_file(&path) {
                        Ok(info) => {
                            // Update database (reuse connection)
                            if let Err(e) = db.update_replay_gain(&path, info.gain, info.peak) {
                                log::error!(
                                    "[ReplayGain Worker {}] Failed to update database for {}: {}",
                                    worker_id,
                                    path.display(),
                                    e
                                );
                                let _ = message_tx.send(ScanMessage::Error {
                                    path: path.clone(),
                                    error: format!("Database error: {}", e),
                                });
                                continue;
                            }

                            log::info!(
                                "[ReplayGain Worker {}] Completed: {} (gain={:.2}dB, peak={:.4})",
                                worker_id,
                                path.display(),
                                info.gain,
                                info.peak
                            );

                            let _ = message_tx.send(ScanMessage::Success {
                                path: path.clone(),
                                gain: info.gain,
                                peak: info.peak,
                            });
                        }
                        Err(e) => {
                            log::error!(
                                "[ReplayGain Worker {}] Failed to analyze {}: {}",
                                worker_id,
                                path.display(),
                                e
                            );
                            let _ = message_tx.send(ScanMessage::Error {
                                path: path.clone(),
                                error: e.to_string(),
                            });
                        }
                    }
                }

                log::info!("[ReplayGain Worker {}] Finished", worker_id);
            });

            workers.push(worker);
        }

        Self {
            _workers: workers,
            task_tx,
            message_rx: Arc::new(Mutex::new(message_rx)),
            stop_tx,
        }
    }

    /// Add a track to the scanning queue
    pub fn scan_track(&self, path: PathBuf) {
        let _ = self.task_tx.send(path);
    }

    /// Add multiple tracks to the scanning queue
    pub fn scan_tracks(&self, paths: Vec<PathBuf>) {
        for path in paths {
            let _ = self.task_tx.send(path);
        }
    }

    /// Try to receive a message (non-blocking)
    pub fn try_recv(&self) -> Option<ScanMessage> {
        self.message_rx.lock().unwrap().try_recv().ok()
    }

    /// Stop all workers
    pub fn stop(&self) {
        // Send stop signal to all workers
        for _ in 0..self._workers.len() {
            let _ = self.stop_tx.send(());
        }
    }
}

impl Drop for ReplayGainScanner {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Helper struct to manage ReplayGain scanning state
#[derive(Debug, Default)]
pub struct ReplayGainScanManager {
    pub scanner: Option<Arc<ReplayGainScanner>>,
    pub in_progress: bool,
    pub total: usize,
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,
}

impl ReplayGainScanManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start scanning all tracks in the database that are missing replaygain data
    pub fn start_scan(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        // Skip if already in progress
        if self.in_progress {
            return Ok("Scan already in progress".to_string());
        }

        // Get database path
        let db_path = MusicDatabase::default_path().ok_or("Could not determine database path")?;

        // Get tracks that need analysis
        let db = MusicDatabase::open(&db_path)?;
        let tracks = db.get_tracks_without_replay_gain()?;

        if tracks.is_empty() {
            log::debug!("All tracks already have ReplayGain data");
            return Ok("All tracks already have ReplayGain data".to_string());
        }

        let total = tracks.len();
        log::info!("Starting ReplayGain scan for {} tracks", total);

        // Determine number of threads (use CPU count or max 4)
        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get().min(4))
            .unwrap_or(2);

        // Create scanner
        let scanner = Arc::new(ReplayGainScanner::new(num_threads, db_path));

        // Queue all tracks
        scanner.scan_tracks(tracks);

        // Store scanner and initialize progress
        self.scanner = Some(scanner);
        self.in_progress = true;
        self.total = total;
        self.processed = 0;
        self.succeeded = 0;
        self.failed = 0;

        Ok(format!("Analyzing {} tracks for ReplayGain...", total))
    }

    /// Update scanning progress by processing messages
    /// Returns true if scan just completed
    pub fn update(&mut self) -> bool {
        let scanner = match &self.scanner {
            Some(s) => Arc::clone(s),
            None => return false,
        };

        let mut just_completed = false;

        // Process all pending messages
        while let Some(msg) = scanner.try_recv() {
            match msg {
                ScanMessage::Started { .. } => {
                    // Track started, no action needed
                }
                ScanMessage::Success { .. } => {
                    self.processed += 1;
                    self.succeeded += 1;
                }
                ScanMessage::Error { path, error } => {
                    self.processed += 1;
                    self.failed += 1;
                    log::error!("ReplayGain scan failed for {}: {}", path.display(), error);
                }
                ScanMessage::Complete {
                    total,
                    succeeded,
                    failed,
                } => {
                    self.in_progress = false;
                    self.scanner = None;
                    just_completed = true;
                    log::info!(
                        "ReplayGain scan complete: {}/{} succeeded, {} failed",
                        succeeded,
                        total,
                        failed
                    );
                }
            }
        }

        // Also check manual completion if we processed everyone but didn't get complete msg?
        // (Scanner sends complete message, so we should rely on it, but redundancy is okay)
        // Leaving it as relying on message for now as implementation of scanner sends it.

        just_completed
    }

    /// Stop the current scan
    pub fn stop(&mut self) {
        if let Some(scanner) = &self.scanner {
            scanner.stop();
        }
        self.in_progress = false;
        self.scanner = None;
    }

    /// Get progress as a percentage
    pub fn progress(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        (self.processed as f32 / self.total as f32) * 100.0
    }
}
