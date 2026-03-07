use crate::database::MusicDatabase;
use sotf_audio::waveform;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

/// Message sent by waveform scanner thread
#[derive(Debug, Clone)]
pub enum WaveformScanMessage {
    /// Started scanning a track
    Started { path: PathBuf },
    /// Successfully scanned a track
    Success { path: PathBuf, waveform: Vec<u8> },
    /// Failed to scan a track
    Error { path: PathBuf, error: String },
    /// Scanning complete
    Complete {
        total: usize,
        succeeded: usize,
        failed: usize,
    },
}

/// Waveform scanner with thread pool for background processing
#[derive(Debug)]
pub struct WaveformScanner {
    _workers: Vec<thread::JoinHandle<()>>,
    task_tx: Sender<PathBuf>,
    message_rx: Arc<Mutex<Receiver<WaveformScanMessage>>>,
    stop_tx: Sender<()>,
}

impl WaveformScanner {
    /// Create a new scanner with the given number of worker threads
    pub fn new(num_threads: usize, db_path: PathBuf, pause_flag: Arc<AtomicBool>) -> Self {
        let (task_tx, task_rx) = mpsc::channel::<PathBuf>();
        let (message_tx, message_rx) = mpsc::channel::<WaveformScanMessage>();
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
            let pause_flag = Arc::clone(&pause_flag);

            let worker = thread::spawn(move || {
                log::info!("[Waveform Worker {}] Started", worker_id);

                // Open database once per worker thread (not per track)
                let db = match MusicDatabase::open(&db_path) {
                    Ok(db) => db,
                    Err(e) => {
                        log::error!(
                            "[Waveform Worker {}] Failed to open database: {}",
                            worker_id,
                            e
                        );
                        return;
                    }
                };

                loop {
                    // Check if we should stop
                    if stop_rx.lock().unwrap().try_recv().is_ok() {
                        log::info!("[Waveform Worker {}] Stopping", worker_id);
                        break;
                    }

                    // Wait while paused (check every 200ms, also check for stop)
                    while pause_flag.load(Ordering::Relaxed) {
                        if stop_rx.lock().unwrap().try_recv().is_ok() {
                            log::info!("[Waveform Worker {}] Stopping while paused", worker_id);
                            return;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(200));
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
                            log::info!("[Waveform Worker {}] Task channel closed", worker_id);
                            break;
                        }
                    };

                    log::debug!(
                        "[Waveform Worker {}] Processing: {}",
                        worker_id,
                        path.display()
                    );

                    // Send started message
                    let _ = message_tx.send(WaveformScanMessage::Started { path: path.clone() });

                    // Analyze the file
                    match waveform::analyze_waveform(&path) {
                        Ok(waveform_data) => {
                            // Update database (reuse connection)
                            if let Err(e) = db.update_waveform(&path, &waveform_data) {
                                log::error!(
                                    "[Waveform Worker {}] Failed to update database for {}: {}",
                                    worker_id,
                                    path.display(),
                                    e
                                );
                                let _ = message_tx.send(WaveformScanMessage::Error {
                                    path: path.clone(),
                                    error: format!("Database error: {}", e),
                                });
                                continue;
                            }

                            log::debug!(
                                "[Waveform Worker {}] Completed: {}",
                                worker_id,
                                path.display()
                            );

                            // Don't send waveform data through channel - it's already in DB
                            let _ = message_tx.send(WaveformScanMessage::Success {
                                path: path.clone(),
                                waveform: Vec::new(), // Empty - data is in DB
                            });
                        }
                        Err(e) => {
                            let error_msg = e.to_string();
                            log::error!(
                                "[Waveform Worker {}] Failed to analyze {}: {}",
                                worker_id,
                                path.display(),
                                error_msg
                            );
                            if let Err(db_err) = db.mark_waveform_error(&path, &error_msg) {
                                log::error!(
                                    "[Waveform Worker {}] Failed to persist error for {}: {}",
                                    worker_id,
                                    path.display(),
                                    db_err
                                );
                            }
                            let _ = message_tx.send(WaveformScanMessage::Error {
                                path: path.clone(),
                                error: error_msg,
                            });
                        }
                    }
                }

                // Checkpoint WAL before exiting to prevent unbounded growth
                if let Err(e) = db.checkpoint_wal() {
                    log::warn!("[Waveform Worker {}] WAL checkpoint failed: {}", worker_id, e);
                }

                log::info!("[Waveform Worker {}] Finished", worker_id);
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
    pub fn try_recv(&self) -> Option<WaveformScanMessage> {
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

impl Drop for WaveformScanner {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Helper struct to manage waveform scanning state
#[derive(Debug)]
pub struct WaveformScanManager {
    pub scanner: Option<Arc<WaveformScanner>>,
    pub in_progress: bool,
    pub total: usize,
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,

    // Shared pause flag — scanners sleep while this is true
    pause_flag: Arc<AtomicBool>,

    // Configurable thread count (None = auto-detect, capped at 4)
    num_threads: Option<usize>,
}

impl Default for WaveformScanManager {
    fn default() -> Self {
        Self::with_pause_flag(Arc::new(AtomicBool::new(false)))
    }
}

impl WaveformScanManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_pause_flag(pause_flag: Arc<AtomicBool>) -> Self {
        Self {
            scanner: None,
            in_progress: false,
            total: 0,
            processed: 0,
            succeeded: 0,
            failed: 0,
            pause_flag,
            num_threads: None,
        }
    }

    /// Set the number of scanner threads. If None, auto-detect (capped at 4).
    pub fn set_num_threads(&mut self, threads: Option<usize>) {
        self.num_threads = threads;
    }

    /// Get the effective number of threads to use.
    fn effective_num_threads(&self) -> usize {
        self.num_threads.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get().min(4))
                .unwrap_or(2)
        })
    }

    /// Clear all waveform data and rescan every track.
    pub fn start_force_scan(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.in_progress {
            return Ok(());
        }

        let db_path = MusicDatabase::default_path().ok_or("Could not determine database path")?;
        let db = MusicDatabase::open(&db_path)?;
        db.clear_all_waveform()?;
        log::info!("Cleared all waveform data for force rescan");

        self.start_scan()
    }

    /// Start scanning all tracks in the database that are missing waveform data
    pub fn start_scan(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Skip if already in progress
        if self.in_progress {
            return Ok(());
        }

        // Get database path
        let db_path = MusicDatabase::default_path().ok_or("Could not determine database path")?;

        // Get tracks that need waveform analysis and total counts
        let db = MusicDatabase::open(&db_path)?;
        let tracks = db.get_tracks_without_waveform()?;
        let total_tracks = db.get_track_count()?;
        let (already_succeeded, already_failed) = db.get_waveform_done_counts()?;

        if tracks.is_empty() {
            log::debug!("All tracks already have waveform data");
            return Ok(());
        }

        let remaining = tracks.len();
        log::info!(
            "Starting waveform scan for {} tracks ({} already done)",
            remaining,
            already_succeeded + already_failed
        );

        // Create scanner with configured thread count
        let num_threads = self.effective_num_threads();
        log::info!("Waveform scanner using {} threads", num_threads);
        let scanner = Arc::new(WaveformScanner::new(
            num_threads,
            db_path,
            Arc::clone(&self.pause_flag),
        ));

        // Queue all tracks
        scanner.scan_tracks(tracks);

        // Store scanner and initialize progress — total reflects the whole library
        self.scanner = Some(scanner);
        self.in_progress = true;
        self.total = total_tracks;
        self.processed = already_succeeded + already_failed;
        self.succeeded = already_succeeded;
        self.failed = already_failed;

        Ok(())
    }

    /// Update scanning progress by processing messages
    pub fn update(&mut self) {
        let scanner = match &self.scanner {
            Some(s) => Arc::clone(s),
            None => return,
        };

        // Process all pending messages
        while let Some(msg) = scanner.try_recv() {
            match msg {
                WaveformScanMessage::Started { .. } => {
                    // Track started, no action needed
                }
                WaveformScanMessage::Success { .. } => {
                    self.processed += 1;
                    self.succeeded += 1;
                }
                WaveformScanMessage::Error { path, error } => {
                    self.processed += 1;
                    self.failed += 1;
                    log::error!("Waveform scan failed for {}: {}", path.display(), error);
                }
                WaveformScanMessage::Complete {
                    total,
                    succeeded,
                    failed,
                } => {
                    self.in_progress = false;
                    self.scanner = None;
                    log::info!(
                        "Waveform scan complete: {}/{} succeeded, {} failed",
                        succeeded,
                        total,
                        failed
                    );
                    return;
                }
            }
        }

        // Workers don't send Complete — detect completion from counters
        if self.total > 0 && self.processed >= self.total {
            log::info!(
                "Waveform scan complete: {}/{} succeeded, {} failed",
                self.succeeded,
                self.total,
                self.failed
            );
            self.in_progress = false;
            self.scanner = None;
        }
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
