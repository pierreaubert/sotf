//! Bliss audio analysis integration
//!
//! This module provides audio analysis using the bliss-rs library with a custom
//! Symphonia-based decoder (instead of ffmpeg). Bliss extracts audio features
//! that can be used to compute similarity between tracks for intelligent
//! playlist generation.
//!
//! # Features extracted
//! - Tempo
//! - Zero-crossing rate (ZCR)
//! - Spectral centroid (mean/std deviation)
//! - Spectral rolloff (mean/std deviation)
//! - Spectral flatness (mean/std deviation)
//! - Loudness (mean/std deviation)
//! - Chroma features (for key/mode detection)

use crate::database::MusicDatabase;
use audioadapter_buffers::direct::SequentialSliceOfVecs;
use bliss_audio::decoder::Decoder as BlissDecoder;
use bliss_audio::decoder::PreAnalyzedSong;
use bliss_audio::{Analysis, AnalysisIndex, BlissError, BlissResult};
use rubato::{Fft, FixedSync, Resampler};
use sotf_audio::decoder::create_decoder;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Number of bliss analysis features stored
pub const BLISS_FEATURES_COUNT: usize = 20;

/// Bliss analysis sample rate (fixed by the bliss library)
const BLISS_SAMPLE_RATE: u32 = 22050;

/// Bliss analysis result for a single track
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
    /// Create from a bliss Analysis object
    pub fn from_analysis(analysis: &Analysis) -> Self {
        let features = analysis.as_vec();
        Self {
            features: features.clone(),
            tempo: analysis[AnalysisIndex::Tempo],
            zcr: analysis[AnalysisIndex::Zcr],
            spectral_centroid_mean: analysis[AnalysisIndex::MeanSpectralCentroid],
            spectral_rolloff_mean: analysis[AnalysisIndex::MeanSpectralRolloff],
            spectral_flatness_mean: analysis[AnalysisIndex::MeanSpectralFlatness],
            loudness_mean: analysis[AnalysisIndex::MeanLoudness],
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

        if features.len() < BLISS_FEATURES_COUNT {
            return None;
        }

        Some(Self {
            features: features.clone(),
            tempo: features.first().copied().unwrap_or(0.0),
            zcr: features.get(1).copied().unwrap_or(0.0),
            spectral_centroid_mean: features.get(2).copied().unwrap_or(0.0),
            spectral_rolloff_mean: features.get(6).copied().unwrap_or(0.0),
            spectral_flatness_mean: features.get(8).copied().unwrap_or(0.0),
            loudness_mean: features.get(10).copied().unwrap_or(0.0),
        })
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

/// Custom Symphonia-based decoder for bliss-rs
///
/// This implements the bliss_audio::decoder::Decoder trait using our existing
/// Symphonia-based decoder instead of requiring ffmpeg.
pub struct SymphoniaBlissDecoder;

impl BlissDecoder for SymphoniaBlissDecoder {
    fn decode(path: &Path) -> BlissResult<PreAnalyzedSong> {
        decode_for_bliss(path)
    }
}

/// Decode an audio file and prepare it for bliss analysis
///
/// This function:
/// 1. Decodes the audio file using Symphonia
/// 2. Converts to mono if stereo/multi-channel
/// 3. Resamples to 22050 Hz (bliss requirement)
/// 4. Returns a PreAnalyzedSong ready for bliss analysis
pub fn decode_for_bliss(path: &Path) -> BlissResult<PreAnalyzedSong> {
    // Create decoder
    let mut decoder = create_decoder(path).map_err(|e| BlissError::DecodingError(e.to_string()))?;

    let spec = decoder.spec().clone();
    let channels = spec.channels as usize;
    let source_sample_rate = spec.sample_rate;

    // Collect all samples
    let mut all_samples: Vec<f32> = Vec::new();

    loop {
        match decoder.decode_next() {
            Ok(Some(audio)) => {
                all_samples.extend_from_slice(&audio.samples);
            }
            Ok(None) => break, // EOF
            Err(e) => return Err(BlissError::DecodingError(e.to_string())),
        }
    }

    if all_samples.is_empty() {
        return Err(BlissError::DecodingError(
            "No audio samples decoded".to_string(),
        ));
    }

    // Convert to mono by averaging channels
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

    // Resample to BLISS_SAMPLE_RATE (22050 Hz) if needed
    let resampled = if source_sample_rate == BLISS_SAMPLE_RATE {
        mono_samples
    } else {
        resample_to_bliss_rate(&mono_samples, source_sample_rate, BLISS_SAMPLE_RATE)?
    };

    // Calculate duration
    let duration_secs = resampled.len() as f64 / BLISS_SAMPLE_RATE as f64;
    let duration = Duration::from_secs_f64(duration_secs);

    Ok(PreAnalyzedSong {
        path: path.to_path_buf(),
        sample_array: resampled,
        duration,
        // Metadata fields - we don't extract them here since we have them in the database
        artist: None,
        album_artist: None,
        title: None,
        album: None,
        track_number: None,
        disc_number: None,
        genre: None,
    })
}

/// Resample audio to the target sample rate using rubato
fn resample_to_bliss_rate(
    samples: &[f32],
    source_rate: u32,
    target_rate: u32,
) -> BlissResult<Vec<f32>> {
    if source_rate == target_rate {
        return Ok(samples.to_vec());
    }

    // Calculate resampling ratio
    let resample_ratio = target_rate as f64 / source_rate as f64;

    // Use FFT-based resampler for quality
    // chunk_size should be a power of 2 for FFT efficiency
    let chunk_size = 1024;

    let mut resampler = Fft::<f32>::new(
        source_rate as usize,
        target_rate as usize,
        chunk_size,
        2,
        1,
        FixedSync::Both,
    )
    .map_err(|e| BlissError::DecodingError(format!("Failed to create resampler: {}", e)))?;

    let input_frames_needed = resampler.input_frames_next();
    let output_frames_per_chunk = resampler.output_frames_next();
    let estimated_output_len =
        ((samples.len() as f64 * resample_ratio) as usize) + output_frames_per_chunk;
    let mut output = Vec::with_capacity(estimated_output_len);
    let mut output_channels = vec![vec![0.0f32; output_frames_per_chunk]];

    // Process in chunks
    let mut pos = 0;
    while pos < samples.len() {
        let end = (pos + input_frames_needed).min(samples.len());
        let chunk = &samples[pos..end];

        // Pad last chunk if needed
        let input_chunk: Vec<f32> = if chunk.len() < input_frames_needed {
            let mut padded = chunk.to_vec();
            padded.resize(input_frames_needed, 0.0);
            padded
        } else {
            chunk.to_vec()
        };

        let input_channels = vec![input_chunk];
        let input_adapter = SequentialSliceOfVecs::new(&input_channels, 1, input_frames_needed)
            .map_err(|e| BlissError::DecodingError(format!("Input adapter error: {}", e)))?;
        let mut output_adapter =
            SequentialSliceOfVecs::new_mut(&mut output_channels, 1, output_frames_per_chunk)
                .map_err(|e| BlissError::DecodingError(format!("Output adapter error: {}", e)))?;

        match resampler.process_into_buffer(&input_adapter, &mut output_adapter, None) {
            Ok((_, written)) => {
                output.extend_from_slice(&output_channels[0][..written]);
            }
            Err(e) => {
                return Err(BlissError::DecodingError(format!(
                    "Resampling error: {}",
                    e
                )));
            }
        }

        pos += input_frames_needed;
    }

    Ok(output)
}

/// Analyze a single audio file and return bliss features
pub fn analyze_file(path: &Path) -> BlissResult<BlissAnalysis> {
    let song = SymphoniaBlissDecoder::song_from_path(path)?;
    Ok(BlissAnalysis::from_analysis(&song.analysis))
}

// ============================================================================
// Scanner for batch processing
// ============================================================================

/// Message sent by bliss scanner thread
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

/// Bliss scanner with thread pool for background processing
#[derive(Debug)]
pub struct BlissScanner {
    _workers: Vec<thread::JoinHandle<()>>,
    task_tx: Sender<PathBuf>,
    message_rx: Arc<Mutex<Receiver<BlissScanMessage>>>,
    stop_tx: Sender<()>,
}

impl BlissScanner {
    /// Create a new scanner with the given number of worker threads
    pub fn new(num_threads: usize, db_path: PathBuf, pause_flag: Arc<AtomicBool>) -> Self {
        let (task_tx, task_rx) = mpsc::channel::<PathBuf>();
        let (message_tx, message_rx) = mpsc::channel::<BlissScanMessage>();
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
                log::info!("[Bliss Worker {}] Started", worker_id);

                // Open database once per worker thread (not per track)
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
                    // Check if we should stop
                    if stop_rx.lock().unwrap().try_recv().is_ok() {
                        log::info!("[Bliss Worker {}] Stopping", worker_id);
                        break;
                    }

                    // Wait while paused (check every 200ms, also check for stop)
                    while pause_flag.load(Ordering::Relaxed) {
                        if stop_rx.lock().unwrap().try_recv().is_ok() {
                            log::info!("[Bliss Worker {}] Stopping while paused", worker_id);
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
                            log::info!("[Bliss Worker {}] Task channel closed", worker_id);
                            break;
                        }
                    };

                    log::debug!(
                        "[Bliss Worker {}] Processing: {}",
                        worker_id,
                        path.display()
                    );

                    // Send started message
                    let _ = message_tx.send(BlissScanMessage::Started { path: path.clone() });

                    // Analyze the file
                    match analyze_file(&path) {
                        Ok(analysis) => {
                            // Update database (reuse connection)
                            if let Err(e) = db.update_bliss(&path, &analysis) {
                                log::error!(
                                    "[Bliss Worker {}] Failed to update database for {}: {}",
                                    worker_id,
                                    path.display(),
                                    e
                                );
                                let _ = message_tx.send(BlissScanMessage::Error {
                                    path: path.clone(),
                                    error: format!("Database error: {}", e),
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

                log::info!("[Bliss Worker {}] Finished", worker_id);
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

    /// Queue a file for analysis
    pub fn queue(&self, path: PathBuf) -> Result<(), mpsc::SendError<PathBuf>> {
        self.task_tx.send(path)
    }

    /// Get the message receiver for progress updates
    pub fn messages(&self) -> Arc<Mutex<Receiver<BlissScanMessage>>> {
        Arc::clone(&self.message_rx)
    }

    /// Signal all workers to stop
    pub fn stop(&self) {
        let _ = self.stop_tx.send(());
    }
}

/// Bliss scan manager for coordinating background analysis
#[derive(Debug)]
pub struct BlissScanManager {
    pub scanner: Option<Arc<BlissScanner>>,
    pub in_progress: bool,
    pub total: usize,
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,

    // Shared pause flag — scanners sleep while this is true
    pause_flag: Arc<AtomicBool>,
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
        }
    }

    /// Clear all bliss data and rescan every track.
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

    /// Start scanning all tracks in the database that are missing bliss analysis data
    pub fn start_scan(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        // Skip if already in progress
        if self.in_progress {
            return Ok("Bliss scan already in progress".to_string());
        }

        // Get database path
        let db_path = MusicDatabase::default_path().ok_or("Could not determine database path")?;

        // Get tracks that need analysis
        let db = MusicDatabase::open(&db_path)?;
        let tracks = db.get_tracks_without_bliss()?;

        if tracks.is_empty() {
            log::debug!("All tracks already have bliss analysis data");
            return Ok("All tracks already have bliss analysis data".to_string());
        }

        let total = tracks.len();
        log::info!("Starting bliss analysis scan for {} tracks", total);

        // Start the scan
        self.start(db_path, tracks);

        Ok(format!(
            "Analyzing {} tracks for bliss audio features...",
            total
        ))
    }

    /// Start a bliss scan for the given tracks
    pub fn start(&mut self, db_path: PathBuf, tracks: Vec<PathBuf>) {
        if self.in_progress {
            return;
        }

        let num_threads = num_cpus::get().clamp(1, 4); // Limit to 4 threads
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

        // Queue all tracks
        for path in tracks {
            if scanner.queue(path).is_err() {
                log::error!("Failed to queue track for bliss analysis");
            }
        }

        self.scanner = Some(scanner);
    }

    /// Process pending messages and update state
    pub fn update(&mut self) {
        if let Some(scanner) = &self.scanner {
            let rx = scanner.messages();
            let rx = rx.lock().unwrap();

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

            // Check if done
            if self.processed >= self.total {
                self.in_progress = false;
            }
        }
    }

    /// Stop the current scan
    pub fn stop(&mut self) {
        if let Some(scanner) = &self.scanner {
            scanner.stop();
        }
        self.in_progress = false;
    }

    /// Get progress as a percentage
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
        assert_eq!(bytes.len(), 5 * 4); // 5 features * 4 bytes each
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
        assert!((dist - 5.0).abs() < 0.001); // 3-4-5 triangle
    }
}
