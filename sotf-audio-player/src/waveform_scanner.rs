use crate::database::MusicDatabase;
use sotf_audio::waveform;
use std::path::PathBuf;
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
    pub fn new(num_threads: usize, db_path: PathBuf) -> Self {
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

            let worker = thread::spawn(move || {
                log::info!("[Waveform Worker {}] Started", worker_id);

                loop {
                    // Check if we should stop
                    if stop_rx.lock().unwrap().try_recv().is_ok() {
                        log::info!("[Waveform Worker {}] Stopping", worker_id);
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
                            // Update database
                            if let Ok(db) = MusicDatabase::open(&db_path) {
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
                            } else {
                                log::error!(
                                    "[Waveform Worker {}] Failed to open database",
                                    worker_id
                                );
                                let _ = message_tx.send(WaveformScanMessage::Error {
                                    path: path.clone(),
                                    error: "Failed to open database".to_string(),
                                });
                                continue;
                            }

                            log::debug!(
                                "[Waveform Worker {}] Completed: {}",
                                worker_id,
                                path.display()
                            );

                            let _ = message_tx.send(WaveformScanMessage::Success {
                                path: path.clone(),
                                waveform: waveform_data,
                            });
                        }
                        Err(e) => {
                            log::error!(
                                "[Waveform Worker {}] Failed to analyze {}: {}",
                                worker_id,
                                path.display(),
                                e
                            );
                            let _ = message_tx.send(WaveformScanMessage::Error {
                                path: path.clone(),
                                error: e.to_string(),
                            });
                        }
                    }
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
#[derive(Debug, Default)]
pub struct WaveformScanManager {
    pub scanner: Option<Arc<WaveformScanner>>,
    pub in_progress: bool,
    pub total: usize,
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,
}

impl WaveformScanManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start scanning all tracks in the database that are missing waveform data
    pub fn start_scan(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Skip if already in progress
        if self.in_progress {
            return Ok(());
        }

        // Get database path
        let db_path = MusicDatabase::default_path().ok_or("Could not determine database path")?;

        // Get tracks that need waveform analysis
        let db = MusicDatabase::open(&db_path)?;
        let tracks = db.get_tracks_without_waveform()?;

        if tracks.is_empty() {
            log::debug!("All tracks already have waveform data");
            return Ok(());
        }

        let total = tracks.len();
        log::info!("Starting waveform scan for {} tracks", total);

        // Determine number of threads (use CPU count or max 4)
        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get().min(4))
            .unwrap_or(2);

        // Create scanner
        let scanner = Arc::new(WaveformScanner::new(num_threads, db_path));

        // Queue all tracks
        scanner.scan_tracks(tracks);

        // Store scanner and initialize progress
        self.scanner = Some(scanner);
        self.in_progress = true;
        self.total = total;
        self.processed = 0;
        self.succeeded = 0;
        self.failed = 0;

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
                }
            }
        }
    }
}
