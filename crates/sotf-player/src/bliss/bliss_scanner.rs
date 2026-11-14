use super::bliss_analysis::analyze_file;
use super::types::BlissScanMessage;
use crate::database::MusicDatabase;
use crossbeam::channel::{self, Receiver, Sender};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

/// Scanner with thread pool for background processing
#[derive(Debug)]
pub struct BlissScanner {
    pub(super) workers: Vec<thread::JoinHandle<()>>,
    pub(super) task_tx: Sender<PathBuf>,
    pub(super) message_rx: Receiver<BlissScanMessage>,
    pub(super) stop_tx: Sender<()>,
}

impl BlissScanner {
    /// Create a new scanner with the given number of worker threads
    #[expect(
        clippy::too_many_lines,
        reason = "The constructor owns worker startup and each worker lifecycle"
    )]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "The public worker constructor owns its configuration and clones handles for each worker "
    )]
    pub fn new(num_threads: usize, db_path: PathBuf, pause_flag: Arc<AtomicBool>) -> Self {
        let (task_tx, task_rx) = channel::unbounded::<PathBuf>();
        let (message_tx, message_rx) = channel::unbounded::<BlissScanMessage>();
        // One stop signal sent per worker; `select!` lets each worker block on
        // the intersection of (next task, stop) without serializing on a Mutex.
        let (stop_tx, stop_rx) = channel::unbounded::<()>();

        let mut workers = Vec::new();

        for worker_id in 0..num_threads {
            // crossbeam Receivers are Sync + cloneable — no Mutex needed.
            let task_rx = task_rx.clone();
            let stop_rx = stop_rx.clone();
            let message_tx = message_tx.clone();
            let db_path = db_path.clone();
            let pause_flag = Arc::clone(&pause_flag);

            let worker = thread::spawn(move || {
                log::info!("[Bliss Worker {worker_id}] Started");

                let db = match MusicDatabase::open(&db_path) {
                    Ok(db) => db,
                    Err(e) => {
                        log::error!("[Bliss Worker {worker_id}] Failed to open database: {e}");
                        return;
                    }
                };

                'worker_loop: loop {
                    if stop_rx.try_recv().is_ok() {
                        log::info!("[Bliss Worker {worker_id}] Stopping");
                        break;
                    }

                    while pause_flag.load(Ordering::Relaxed) {
                        channel::select! {
                            recv(stop_rx) -> _ => {
                                log::info!("[Bliss Worker {}] Stopping while paused", worker_id);
                                break 'worker_loop;
                            }
                            recv(channel::after(std::time::Duration::from_millis(50))) -> _ => {}
                        }
                    }

                    // Block on (task, stop) concurrently — N workers wait
                    // simultaneously without serializing on Mutex<Receiver>.
                    let path = channel::select! {
                        recv(task_rx) -> msg => if let Ok(path) = msg { path } else {
                            log::info!("[Bliss Worker {worker_id}] Task channel closed");
                            break;
                        },
                        recv(stop_rx) -> _ => {
                            log::info!("[Bliss Worker {}] Stopping (select)", worker_id);
                            break;
                        }
                    };

                    log::debug!(
                        "[Bliss Worker {}] Processing: {}",
                        worker_id,
                        path.display()
                    );

                    let _ = message_tx.send(BlissScanMessage::Started { path: path.clone() });

                    match analyze_file(&path) {
                        Ok(analysis) => {
                            if let Err(e) = db.update_bliss(&path, &analysis) {
                                log::error!(
                                    "[Bliss Worker {}] Failed to update database for {}: {}",
                                    worker_id,
                                    path.display(),
                                    e
                                );
                                let _ = message_tx.send(BlissScanMessage::Error {
                                    path: path.clone(),
                                    error: format!("Database error: {e}"),
                                });
                                continue;
                            }

                            let _ = message_tx.send(BlissScanMessage::Success {
                                path,
                                tempo: analysis.tempo,
                                features_count: analysis.features.len(),
                            });
                        }
                        Err(e) => {
                            let error_msg = e.clone();
                            log::debug!(
                                "[Bliss Worker {}] Failed to analyze {}: {}",
                                worker_id,
                                path.display(),
                                error_msg
                            );
                            if let Err(db_err) = db.mark_bliss_error(&path, &error_msg) {
                                log::error!(
                                    "[Bliss Worker {}] Failed to persist error for {}: {}",
                                    worker_id,
                                    path.display(),
                                    db_err
                                );
                            }
                            let _ = message_tx.send(BlissScanMessage::Error {
                                path,
                                error: error_msg,
                            });
                        }
                    }
                }

                if let Err(e) = db.checkpoint_wal() {
                    log::warn!("[Bliss Worker {worker_id}] WAL checkpoint failed: {e}");
                }

                log::info!("[Bliss Worker {worker_id}] Finished");
            });

            workers.push(worker);
        }

        Self {
            workers,
            task_tx,
            message_rx,
            stop_tx,
        }
    }

    /// Queue a file for analysis
    ///
    /// # Errors
    /// Returns an error if the scan is already running or its database cannot be opened.
    pub fn queue(&self, path: PathBuf) -> Result<(), channel::SendError<PathBuf>> {
        self.task_tx.send(path)
    }

    /// Get a clone of the message receiver for progress updates.
    /// crossbeam Receivers are `Sync` and cloneable.
    #[must_use]
    pub fn messages(&self) -> Receiver<BlissScanMessage> {
        self.message_rx.clone()
    }

    /// Signal all workers to stop
    pub fn stop(&self) {
        for _ in 0..self.workers.len() {
            let _ = self.stop_tx.send(());
        }
    }
}

impl Drop for BlissScanner {
    fn drop(&mut self) {
        self.stop();
    }
}
