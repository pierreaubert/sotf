//! Audio feature analysis for music similarity
//!
//! This module provides audio analysis using a pure Rust implementation
//! (math-dsp audio_features) with a Symphonia-based decoder. It extracts
//! features that can be used to compute similarity between tracks for
//! intelligent playlist generation.
//!
//! # Features extracted (23 total, bliss v2 compatible)
//! - Tempo (BPM)
//! - Zero-crossing rate (ZCR)
//! - Spectral centroid (mean/std deviation)
//! - Spectral rolloff (mean/std deviation)
//! - Spectral flatness (mean/std deviation)
//! - Loudness (mean/std deviation)
//! - Chroma interval features (13)

use crate::database::MusicDatabase;
use audioadapter_buffers::direct::SequentialSliceOfVecs;
use crossbeam::channel::{self, Receiver, Sender};
use math_audio_dsp::audio_features;
use rubato::{Fft, FixedSync, Resampler};
use sotf_audio::decoder::create_decoder;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

/// Number of audio analysis features stored
pub const BLISS_FEATURES_COUNT: usize = audio_features::FEATURES_COUNT;

/// Analysis sample rate (matches bliss convention)
const ANALYSIS_SAMPLE_RATE: u32 = 22050;

/// Audio analysis result for a single track
#[derive(Debug, Clone)]
pub struct BlissAnalysis {
    /// All analysis features as a vector
    pub features: Vec<f32>,
    /// Tempo in BPM
    pub tempo: f32,
    /// Zero-crossing rate
    pub zcr: f32,
    /// Mean spectral centroid
    pub spectral_centroid_mean: f32,
    /// Mean spectral rolloff
    pub spectral_rolloff_mean: f32,
    /// Mean spectral flatness
    pub spectral_flatness_mean: f32,
    /// Mean loudness
    pub loudness_mean: f32,
}

impl BlissAnalysis {
    /// Create from a feature vector (23 elements, bliss v2 order)
    pub fn from_features(features: Vec<f32>) -> Self {
        Self {
            tempo: features.first().copied().unwrap_or(0.0),
            zcr: features.get(1).copied().unwrap_or(0.0),
            spectral_centroid_mean: features.get(2).copied().unwrap_or(0.0),
            spectral_rolloff_mean: features.get(4).copied().unwrap_or(0.0),
            spectral_flatness_mean: features.get(6).copied().unwrap_or(0.0),
            loudness_mean: features.get(8).copied().unwrap_or(0.0),
            features,
        }
    }

    /// Serialize features to bytes for database storage
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.features.len() * 4);
        for f in &self.features {
            bytes.extend_from_slice(&f.to_le_bytes());
        }
        bytes
    }

    /// Deserialize features from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if !bytes.len().is_multiple_of(4) {
            return None;
        }
        let features: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();

        // Accept both old 20-feature and new 23-feature vectors
        if features.len() < 20 {
            return None;
        }

        Some(Self::from_features(features))
    }

    /// Compute Euclidean distance to another analysis (for similarity)
    pub fn distance(&self, other: &BlissAnalysis) -> f32 {
        if self.features.len() != other.features.len() {
            return f32::MAX;
        }
        let sum: f32 = self
            .features
            .iter()
            .zip(other.features.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum();
        sum.sqrt()
    }
}

/// Decode an audio file to mono 22050 Hz samples for analysis
fn decode_for_analysis(path: &Path) -> Result<Vec<f32>, String> {
    let mut decoder = create_decoder(path).map_err(|e| e.to_string())?;

    let spec = decoder.spec().clone();
    let channels = spec.channels as usize;
    let source_sample_rate = spec.sample_rate;

    let mut all_samples: Vec<f32> = Vec::new();

    loop {
        match decoder.decode_next() {
            Ok(Some(audio)) => {
                all_samples.extend_from_slice(&audio.samples);
            }
            Ok(None) => break,
            Err(e) => return Err(e.to_string()),
        }
    }

    if all_samples.is_empty() {
        return Err("No audio samples decoded".to_string());
    }

    // Convert to mono
    let mono_samples: Vec<f32> = if channels == 1 {
        all_samples
    } else {
        let frame_count = all_samples.len() / channels;
        (0..frame_count)
            .map(|i| {
                let start = i * channels;
                let sum: f32 = (0..channels).map(|ch| all_samples[start + ch]).sum();
                sum / channels as f32
            })
            .collect()
    };

    // Resample to ANALYSIS_SAMPLE_RATE if needed
    if source_sample_rate == ANALYSIS_SAMPLE_RATE {
        Ok(mono_samples)
    } else {
        resample(&mono_samples, source_sample_rate, ANALYSIS_SAMPLE_RATE)
    }
}

/// Resample audio to the target sample rate using rubato
fn resample(samples: &[f32], source_rate: u32, target_rate: u32) -> Result<Vec<f32>, String> {
    if source_rate == target_rate {
        return Ok(samples.to_vec());
    }

    let resample_ratio = target_rate as f64 / source_rate as f64;
    let chunk_size = 1024;

    let mut resampler = Fft::<f32>::new(
        source_rate as usize,
        target_rate as usize,
        chunk_size,
        2,
        1,
        FixedSync::Both,
    )
    .map_err(|e| format!("Failed to create resampler: {e}"))?;

    let input_frames_needed = resampler.input_frames_next();
    let output_frames_per_chunk = resampler.output_frames_next();
    let estimated_output_len =
        ((samples.len() as f64 * resample_ratio) as usize) + output_frames_per_chunk;
    let mut output = Vec::with_capacity(estimated_output_len);
    let mut output_channels = vec![vec![0.0f32; output_frames_per_chunk]];

    let mut pos = 0;
    while pos < samples.len() {
        let end = (pos + input_frames_needed).min(samples.len());
        let chunk = &samples[pos..end];

        let input_chunk: Vec<f32> = if chunk.len() < input_frames_needed {
            let mut padded = chunk.to_vec();
            padded.resize(input_frames_needed, 0.0);
            padded
        } else {
            chunk.to_vec()
        };

        let input_channels = vec![input_chunk];
        let input_adapter = SequentialSliceOfVecs::new(&input_channels, 1, input_frames_needed)
            .map_err(|e| format!("Input adapter error: {e}"))?;
        let mut output_adapter =
            SequentialSliceOfVecs::new_mut(&mut output_channels, 1, output_frames_per_chunk)
                .map_err(|e| format!("Output adapter error: {e}"))?;

        match resampler.process_into_buffer(&input_adapter, &mut output_adapter, None) {
            Ok((_, written)) => {
                output.extend_from_slice(&output_channels[0][..written]);
            }
            Err(e) => {
                return Err(format!("Resampling error: {e}"));
            }
        }

        pos += input_frames_needed;
    }

    Ok(output)
}

/// Analyze a single audio file and return analysis features
pub fn analyze_file(path: &Path) -> Result<BlissAnalysis, String> {
    let samples = decode_for_analysis(path)?;
    let features = audio_features::analyze_audio_features(&samples, ANALYSIS_SAMPLE_RATE)
        .map_err(|e| e.to_string())?;
    Ok(BlissAnalysis::from_features(features))
}

// ============================================================================
// Scanner for batch processing
// ============================================================================

/// Message sent by scanner thread
#[derive(Debug, Clone)]
pub enum BlissScanMessage {
    /// Started scanning a track
    Started { path: PathBuf },
    /// Successfully scanned a track
    Success {
        path: PathBuf,
        tempo: f32,
        features_count: usize,
    },
    /// Failed to scan a track
    Error { path: PathBuf, error: String },
    /// Scanning complete
    Complete {
        total: usize,
        succeeded: usize,
        failed: usize,
    },
}

/// Scanner with thread pool for background processing
#[derive(Debug)]
pub struct BlissScanner {
    _workers: Vec<thread::JoinHandle<()>>,
    task_tx: Sender<PathBuf>,
    message_rx: Receiver<BlissScanMessage>,
    stop_tx: Sender<()>,
}

impl BlissScanner {
    /// Create a new scanner with the given number of worker threads
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
                log::info!("[Bliss Worker {}] Started", worker_id);

                let db = match MusicDatabase::open(&db_path) {
                    Ok(db) => db,
                    Err(e) => {
                        log::error!(
                            "[Bliss Worker {}] Failed to open database: {}",
                            worker_id,
                            e
                        );
                        return;
                    }
                };

                loop {
                    if stop_rx.try_recv().is_ok() {
                        log::info!("[Bliss Worker {}] Stopping", worker_id);
                        break;
                    }

                    while pause_flag.load(Ordering::Relaxed) {
                        if stop_rx.try_recv().is_ok() {
                            log::info!("[Bliss Worker {}] Stopping while paused", worker_id);
                            return;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(200));
                    }

                    // Block on (task, stop) concurrently — N workers wait
                    // simultaneously without serializing on Mutex<Receiver>.
                    let path = channel::select! {
                        recv(task_rx) -> msg => match msg {
                            Ok(path) => path,
                            Err(_) => {
                                log::info!("[Bliss Worker {}] Task channel closed", worker_id);
                                break;
                            }
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
                            let error_msg = e.to_string();
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
                    log::warn!("[Bliss Worker {}] WAL checkpoint failed: {}", worker_id, e);
                }

                log::info!("[Bliss Worker {}] Finished", worker_id);
            });

            workers.push(worker);
        }

        Self {
            _workers: workers,
            task_tx,
            message_rx,
            stop_tx,
        }
    }

    /// Queue a file for analysis
    pub fn queue(&self, path: PathBuf) -> Result<(), channel::SendError<PathBuf>> {
        self.task_tx.send(path)
    }

    /// Get a clone of the message receiver for progress updates.
    /// crossbeam Receivers are `Sync` and cloneable.
    pub fn messages(&self) -> Receiver<BlissScanMessage> {
        self.message_rx.clone()
    }

    /// Signal all workers to stop
    pub fn stop(&self) {
        for _ in 0..self._workers.len() {
            let _ = self.stop_tx.send(());
        }
    }
}

impl Drop for BlissScanner {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Scan manager for coordinating background analysis
#[derive(Debug)]
pub struct BlissScanManager {
    pub scanner: Option<Arc<BlissScanner>>,
    pub in_progress: bool,
    pub total: usize,
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,

    pause_flag: Arc<AtomicBool>,
    num_threads: Option<usize>,
}

impl Default for BlissScanManager {
    fn default() -> Self {
        Self::with_pause_flag(Arc::new(AtomicBool::new(false)))
    }
}

impl BlissScanManager {
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

    pub fn set_num_threads(&mut self, threads: Option<usize>) {
        self.num_threads = threads;
    }

    fn effective_num_threads(&self) -> usize {
        self.num_threads
            .unwrap_or_else(|| num_cpus::get().clamp(1, 4))
    }

    pub fn refresh_counts(&mut self) {
        if self.in_progress {
            return;
        }
        let db_path = match MusicDatabase::default_path() {
            Some(p) => p,
            None => return,
        };
        let db = match MusicDatabase::open(&db_path) {
            Ok(d) => d,
            Err(_) => return,
        };
        let total = db.get_track_count().unwrap_or(0);
        let (succeeded, failed) = db.get_bliss_done_counts().unwrap_or((0, 0));
        self.total = total;
        self.succeeded = succeeded;
        self.failed = failed;
    }

    pub fn start_force_scan(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        if self.in_progress {
            return Ok("Bliss scan already in progress".to_string());
        }

        let db_path = MusicDatabase::default_path().ok_or("Could not determine database path")?;
        let db = MusicDatabase::open(&db_path)?;
        db.clear_all_bliss()?;
        log::info!("Cleared all bliss data for force rescan");

        self.start_scan()
    }

    pub fn start_scan(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        if self.in_progress {
            return Ok("Bliss scan already in progress".to_string());
        }

        let db_path = MusicDatabase::default_path().ok_or("Could not determine database path")?;

        let db = MusicDatabase::open(&db_path)?;
        let tracks = db.get_tracks_without_bliss()?;
        let total_tracks = db.get_track_count()?;
        let (already_succeeded, already_failed) = db.get_bliss_done_counts()?;

        if tracks.is_empty() {
            log::debug!("All tracks already have bliss analysis data");
            self.total = total_tracks;
            self.succeeded = already_succeeded;
            self.failed = already_failed;
            return Ok("All tracks already have bliss analysis data".to_string());
        }

        let remaining = tracks.len();
        log::info!(
            "Starting bliss analysis scan for {} tracks ({} already done)",
            remaining,
            already_succeeded + already_failed
        );

        self.start(db_path, tracks);
        self.total = total_tracks;
        self.succeeded = already_succeeded;
        self.failed = already_failed;

        Ok(format!(
            "Analyzing {} tracks for bliss audio features...",
            remaining
        ))
    }

    pub fn start(&mut self, db_path: PathBuf, tracks: Vec<PathBuf>) {
        if self.in_progress {
            return;
        }

        let num_threads = self.effective_num_threads();
        log::info!("Bliss scanner using {} threads", num_threads);
        let scanner = Arc::new(BlissScanner::new(
            num_threads,
            db_path,
            Arc::clone(&self.pause_flag),
        ));

        self.total = tracks.len();
        self.processed = 0;
        self.succeeded = 0;
        self.failed = 0;
        self.in_progress = true;

        for path in tracks {
            if scanner.queue(path).is_err() {
                log::error!("Failed to queue track for bliss analysis");
            }
        }

        self.scanner = Some(scanner);
    }

    pub fn update(&mut self) {
        if let Some(scanner) = &self.scanner {
            let rx = scanner.messages();

            while let Ok(msg) = rx.try_recv() {
                match msg {
                    BlissScanMessage::Started { .. } => {}
                    BlissScanMessage::Success { .. } => {
                        self.processed += 1;
                        self.succeeded += 1;
                    }
                    BlissScanMessage::Error { .. } => {
                        self.processed += 1;
                        self.failed += 1;
                    }
                    BlissScanMessage::Complete { .. } => {
                        self.in_progress = false;
                    }
                }
            }

            if self.processed >= self.total {
                self.in_progress = false;
            }
        }
    }

    pub fn stop(&mut self) {
        if let Some(scanner) = &self.scanner {
            scanner.stop();
        }
        self.in_progress = false;
    }

    pub fn progress(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        (self.processed as f32 / self.total as f32) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bliss_analysis_serialization() {
        let analysis = BlissAnalysis {
            features: vec![1.0, 2.0, 3.0, 4.0, 5.0],
            tempo: 120.0,
            zcr: 0.5,
            spectral_centroid_mean: 1000.0,
            spectral_rolloff_mean: 5000.0,
            spectral_flatness_mean: 0.1,
            loudness_mean: -10.0,
        };

        let bytes = analysis.to_bytes();
        assert_eq!(bytes.len(), 5 * 4);
    }

    #[test]
    fn test_bliss_distance() {
        let a = BlissAnalysis {
            features: vec![0.0, 0.0, 0.0],
            tempo: 0.0,
            zcr: 0.0,
            spectral_centroid_mean: 0.0,
            spectral_rolloff_mean: 0.0,
            spectral_flatness_mean: 0.0,
            loudness_mean: 0.0,
        };

        let b = BlissAnalysis {
            features: vec![3.0, 4.0, 0.0],
            tempo: 0.0,
            zcr: 0.0,
            spectral_centroid_mean: 0.0,
            spectral_rolloff_mean: 0.0,
            spectral_flatness_mean: 0.0,
            loudness_mean: 0.0,
        };

        let dist = a.distance(&b);
        assert!((dist - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_from_bytes_old_format() {
        // 20 features (old bliss v1 format) should still work
        let features: Vec<f32> = (0..20).map(|i| i as f32).collect();
        let bytes: Vec<u8> = features.iter().flat_map(|f| f.to_le_bytes()).collect();
        let analysis = BlissAnalysis::from_bytes(&bytes).unwrap();
        assert_eq!(analysis.features.len(), 20);
    }

    #[test]
    fn test_from_bytes_new_format() {
        // 23 features (new v2 format)
        let features: Vec<f32> = (0..23).map(|i| i as f32).collect();
        let bytes: Vec<u8> = features.iter().flat_map(|f| f.to_le_bytes()).collect();
        let analysis = BlissAnalysis::from_bytes(&bytes).unwrap();
        assert_eq!(analysis.features.len(), 23);
    }
}
