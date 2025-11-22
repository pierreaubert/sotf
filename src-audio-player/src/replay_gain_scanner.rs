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

                    log::info!(
                        "[ReplayGain Worker {}] Processing: {}",
                        worker_id,
                        path.display()
                    );

                    // Send started message
                    let _ = message_tx.send(ScanMessage::Started { path: path.clone() });

                    // Analyze the file
                    match replaygain::analyze_file(&path) {
                        Ok(info) => {
                            // Update database
                            if let Ok(db) = MusicDatabase::open(&db_path) {
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
                            } else {
                                log::error!(
                                    "[ReplayGain Worker {}] Failed to open database",
                                    worker_id
                                );
                                let _ = message_tx.send(ScanMessage::Error {
                                    path: path.clone(),
                                    error: "Failed to open database".to_string(),
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
