use crate::database::MusicDatabase;
use sotf_audio::replaygain;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

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
    pub fn new(num_threads: usize, db_path: PathBuf, pause_flag: Arc<AtomicBool>) -> Self {
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
            let pause_flag = Arc::clone(&pause_flag);

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

                    // Wait while paused (check every 200ms, also check for stop)
                    while pause_flag.load(Ordering::Relaxed) {
                        if stop_rx.lock().unwrap().try_recv().is_ok() {
                            log::info!("[ReplayGain Worker {}] Stopping while paused", worker_id);
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
                            let error_msg = e.to_string();
                            log::error!(
                                "[ReplayGain Worker {}] Failed to analyze {}: {}",
                                worker_id,
                                path.display(),
                                error_msg
                            );
                            if let Err(db_err) = db.mark_replay_gain_error(&path, &error_msg) {
                                log::error!(
                                    "[ReplayGain Worker {}] Failed to persist error for {}: {}",
                                    worker_id,
                                    path.display(),
                                    db_err
                                );
                            }
                            let _ = message_tx.send(ScanMessage::Error {
                                path: path.clone(),
                                error: error_msg,
                            });
                        }
                    }
                }

                // Checkpoint WAL before exiting to prevent unbounded growth
                if let Err(e) = db.checkpoint_wal() {
                    log::warn!("[ReplayGain Worker {}] WAL checkpoint failed: {}", worker_id, e);
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

/// Progress state for album gain scanning
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
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
enum AlbumGainMessage {
    Progress { albums_done: usize },
    Complete { succeeded: usize, failed: usize },
}

/// Helper struct to manage ReplayGain scanning state
#[derive(Debug)]
pub struct ReplayGainScanManager {
    pub scanner: Option<Arc<ReplayGainScanner>>,
    pub in_progress: bool,
    pub total: usize,
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,

    // Album gain scanning (second pass)
    album_gain_rx: Option<Receiver<AlbumGainMessage>>,
    pub album_gain_phase: AlbumGainPhase,
    pub album_gain_done: usize,
    pub album_gain_total: usize,

    // Shared pause flag — scanners sleep while this is true
    pause_flag: Arc<AtomicBool>,

    // Configurable thread count (None = auto-detect, capped at 4)
    num_threads: Option<usize>,
}


impl Default for ReplayGainScanManager {
    fn default() -> Self {
        Self::with_pause_flag(Arc::new(AtomicBool::new(false)))
    }
}

impl ReplayGainScanManager {
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
            album_gain_rx: None,
            album_gain_phase: AlbumGainPhase::Idle,
            album_gain_done: 0,
            album_gain_total: 0,
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

    /// Start scanning all tracks in the database that are missing replaygain data
    /// Clear all existing ReplayGain data and rescan every track.
    pub fn start_force_scan(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        if self.in_progress {
            return Ok("Scan already in progress".to_string());
        }

        let db_path = MusicDatabase::default_path().ok_or("Could not determine database path")?;
        let db = MusicDatabase::open(&db_path)?;
        db.clear_all_replay_gain()?;
        log::info!("Cleared all ReplayGain data for force rescan");

        self.start_scan()
    }

    pub fn start_scan(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        // Skip if already in progress
        if self.in_progress {
            return Ok("Scan already in progress".to_string());
        }

        // Get database path
        let db_path = MusicDatabase::default_path().ok_or("Could not determine database path")?;

        // Get tracks that need analysis and total counts
        let db = MusicDatabase::open(&db_path)?;
        let tracks = db.get_tracks_without_replay_gain()?;
        let total_tracks = db.get_track_count()?;
        let (already_succeeded, already_failed) = db.get_replay_gain_done_counts()?;

        if tracks.is_empty() {
            log::debug!("All tracks already have ReplayGain data");
            // No track scanning needed, but check if album gains are missing
            self.start_album_gain_scan();
            if self.album_gain_phase == AlbumGainPhase::Scanning {
                return Ok("Computing album ReplayGain...".to_string());
            }
            return Ok("All tracks already have ReplayGain data".to_string());
        }

        let remaining = tracks.len();
        log::info!(
            "Starting ReplayGain scan for {} tracks ({} already done)",
            remaining,
            already_succeeded + already_failed
        );

        // Create scanner with configured thread count
        let num_threads = self.effective_num_threads();
        log::info!("ReplayGain scanner using {} threads", num_threads);
        let scanner = Arc::new(ReplayGainScanner::new(
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
        self.album_gain_phase = AlbumGainPhase::Idle;

        Ok(format!("Analyzing {} tracks for ReplayGain...", remaining))
    }

    /// Start the album gain computation pass.
    /// This analyzes all tracks in each album that's missing album_gain and computes
    /// the combined album-level ReplayGain using EBU R128 gating block accumulation.
    fn start_album_gain_scan(&mut self) {
        if self.album_gain_phase == AlbumGainPhase::Scanning {
            return;
        }

        let db_path = match MusicDatabase::default_path() {
            Some(p) => p,
            None => return,
        };

        let db = match MusicDatabase::open(&db_path) {
            Ok(db) => db,
            Err(e) => {
                log::error!("[AlbumGain] Failed to open database: {}", e);
                return;
            }
        };

        let albums = match db.get_albums_without_album_gain() {
            Ok(a) => a,
            Err(e) => {
                log::error!("[AlbumGain] Failed to get albums: {}", e);
                return;
            }
        };

        if albums.is_empty() {
            log::info!("[AlbumGain] All albums already have album gain data");
            self.album_gain_phase = AlbumGainPhase::Done;
            return;
        }

        let album_count = albums.len();
        log::info!(
            "[AlbumGain] Starting album gain computation for {} albums",
            album_count
        );

        let (tx, rx) = mpsc::channel();
        self.album_gain_rx = Some(rx);
        self.album_gain_phase = AlbumGainPhase::Scanning;
        self.album_gain_done = 0;
        self.album_gain_total = album_count;
        self.in_progress = true;

        thread::spawn(move || {
            let db = match MusicDatabase::open(&db_path) {
                Ok(db) => db,
                Err(e) => {
                    log::error!("[AlbumGain] Worker failed to open database: {}", e);
                    let _ = tx.send(AlbumGainMessage::Complete {
                        succeeded: 0,
                        failed: album_count,
                    });
                    return;
                }
            };

            let mut succeeded = 0;
            let mut failed = 0;

            for (idx, (_album_id, track_paths)) in albums.iter().enumerate() {
                // Analyze each track in the album using the extended function
                let mut track_data: Vec<(f64, u64, f64)> = Vec::new();
                let mut album_failed = false;

                for path in track_paths {
                    match replaygain::analyze_file_extended(path) {
                        Ok(data) => {
                            track_data.push((data.peak, data.gating_block_count, data.energy));
                        }
                        Err(e) => {
                            log::warn!("[AlbumGain] Failed to analyze {}: {}", path.display(), e);
                            album_failed = true;
                            break;
                        }
                    }
                }

                if album_failed {
                    failed += 1;
                    let _ = tx.send(AlbumGainMessage::Progress {
                        albums_done: idx + 1,
                    });
                    continue;
                }

                // Compute album gain from accumulated data
                if let Some((album_gain, album_peak)) = replaygain::compute_album_gain(&track_data)
                {
                    // Write album gain to all tracks in this album
                    let mut write_ok = true;
                    for path in track_paths {
                        if let Err(e) = db.update_album_gain(path, album_gain, album_peak) {
                            log::error!(
                                "[AlbumGain] Failed to update DB for {}: {}",
                                path.display(),
                                e
                            );
                            write_ok = false;
                        }
                    }
                    if write_ok {
                        log::info!(
                            "[AlbumGain] Album {} ({} tracks): gain={:+.2}dB, peak={:.4}",
                            idx + 1,
                            track_paths.len(),
                            album_gain,
                            album_peak
                        );
                        succeeded += 1;
                    } else {
                        failed += 1;
                    }
                } else {
                    log::warn!("[AlbumGain] Could not compute gain for album {}", idx + 1);
                    failed += 1;
                }

                let _ = tx.send(AlbumGainMessage::Progress {
                    albums_done: idx + 1,
                });
            }

            let _ = tx.send(AlbumGainMessage::Complete { succeeded, failed });
        });
    }

    /// Update scanning progress by processing messages
    /// Returns true if scan just completed
    pub fn update(&mut self) -> bool {
        let mut just_completed = false;

        // Process track scan messages
        if let Some(scanner) = &self.scanner {
            let scanner = Arc::clone(scanner);
            while let Some(msg) = scanner.try_recv() {
                match msg {
                    ScanMessage::Started { .. } => {}
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
                        log::info!(
                            "ReplayGain track scan complete: {}/{} succeeded, {} failed",
                            succeeded,
                            total,
                            failed
                        );
                    }
                }
            }

            // Check if track scanning is done (all tracks processed)
            if self.processed >= self.total && self.total > 0 {
                self.scanner = None;
                // Automatically start album gain scan
                self.start_album_gain_scan();
                if self.album_gain_phase != AlbumGainPhase::Scanning {
                    // No albums to scan, we're fully done
                    self.in_progress = false;
                    just_completed = true;
                }
            }
        }

        // Process album gain messages
        if self.album_gain_phase == AlbumGainPhase::Scanning {
            let mut completed = false;
            if let Some(rx) = &self.album_gain_rx {
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        AlbumGainMessage::Progress { albums_done } => {
                            self.album_gain_done = albums_done;
                        }
                        AlbumGainMessage::Complete { succeeded, failed } => {
                            log::info!(
                                "[AlbumGain] Complete: {} succeeded, {} failed",
                                succeeded,
                                failed
                            );
                            self.album_gain_phase = AlbumGainPhase::Done;
                            self.in_progress = false;
                            just_completed = true;
                            completed = true;
                        }
                    }
                }
            }
            if completed {
                self.album_gain_rx = None;
            }
        }

        just_completed
    }

    /// Stop the current scan
    pub fn stop(&mut self) {
        if let Some(scanner) = &self.scanner {
            scanner.stop();
        }
        self.in_progress = false;
        self.scanner = None;
        self.album_gain_phase = AlbumGainPhase::Idle;
        self.album_gain_rx = None;
    }

    /// Get progress as a percentage
    pub fn progress(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        (self.processed as f32 / self.total as f32) * 100.0
    }
}
