// ============================================================================
// Head Tracker - Main Tracking Interface
// ============================================================================
//
// Coordinates camera capture, vision processing, and smoothing into a
// unified real-time head tracking system.

use crate::backend::{FaceDetection, MacOSVisionBackend};
use crate::camera::CameraCapture;
use crate::smoother::HeadPositionSmoother;
use crate::types::{CalibrationData, HeadPosition, HeadTrackerConfig, HeadTrackerError};
use crossbeam::queue::ArrayQueue;
use log::{debug, error, info, warn};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Head tracker providing real-time face-based head position tracking
pub struct HeadTracker {
    /// Configuration
    config: Arc<Mutex<HeadTrackerConfig>>,

    /// Latest head position (lock-free queue for audio thread access)
    position_queue: Arc<ArrayQueue<HeadPosition>>,

    /// Calibration data
    calibration: Arc<Mutex<CalibrationData>>,

    /// Tracking thread handle
    thread_handle: Option<JoinHandle<()>>,

    /// Signal to stop tracking
    stop_signal: Arc<AtomicBool>,

    /// Is tracking currently active
    is_tracking: Arc<AtomicBool>,

    /// Latest position (for non-queue access)
    latest_position: Arc<Mutex<HeadPosition>>,
}

impl std::fmt::Debug for HeadTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeadTracker")
            .field("is_tracking", &self.is_tracking.load(Ordering::Relaxed))
            .field("config", &*self.config.lock())
            .finish()
    }
}

impl HeadTracker {
    /// Create a new head tracker with default configuration
    pub fn new() -> Self {
        Self::with_config(HeadTrackerConfig::default())
    }

    /// Create a new head tracker with custom configuration
    pub fn with_config(config: HeadTrackerConfig) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
            position_queue: Arc::new(ArrayQueue::new(16)), // Small queue, we only care about latest
            calibration: Arc::new(Mutex::new(CalibrationData::default())),
            thread_handle: None,
            stop_signal: Arc::new(AtomicBool::new(false)),
            is_tracking: Arc::new(AtomicBool::new(false)),
            latest_position: Arc::new(Mutex::new(HeadPosition::default())),
        }
    }

    /// Start head tracking
    ///
    /// Spawns a background thread that captures camera frames and runs
    /// face detection at the configured frame rate.
    pub fn start(&mut self) -> Result<(), HeadTrackerError> {
        if self.is_tracking.load(Ordering::Relaxed) {
            return Err(HeadTrackerError::AlreadyRunning);
        }

        let config = self.config.lock().clone();
        info!("Starting head tracker at {} FPS", config.target_fps);

        // Reset stop signal
        self.stop_signal.store(false, Ordering::Relaxed);

        // Clone Arc references for the thread
        let config_arc = Arc::clone(&self.config);
        let position_queue = Arc::clone(&self.position_queue);
        let calibration = Arc::clone(&self.calibration);
        let stop_signal = Arc::clone(&self.stop_signal);
        let is_tracking = Arc::clone(&self.is_tracking);
        let latest_position = Arc::clone(&self.latest_position);

        // Spawn tracking thread
        let handle = thread::Builder::new()
            .name("head-tracker".to_string())
            .spawn(move || {
                tracking_thread(
                    config_arc,
                    position_queue,
                    calibration,
                    stop_signal,
                    is_tracking,
                    latest_position,
                );
            })
            .map_err(|e| HeadTrackerError::Camera(format!("Failed to spawn thread: {}", e)))?;

        self.thread_handle = Some(handle);
        Ok(())
    }

    /// Stop head tracking
    pub fn stop(&mut self) {
        info!("Stopping head tracker");
        self.stop_signal.store(true, Ordering::Relaxed);

        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }

    /// Check if tracking is active
    pub fn is_tracking(&self) -> bool {
        self.is_tracking.load(Ordering::Relaxed)
    }

    /// Get the latest head position (non-blocking)
    ///
    /// Returns None if no position is available or tracking hasn't started.
    /// This is safe to call from the audio thread.
    pub fn get_position(&self) -> Option<HeadPosition> {
        // Try to get from queue first (drains old values)
        let mut latest = None;
        while let Some(pos) = self.position_queue.pop() {
            latest = Some(pos);
        }

        // If nothing in queue, return the cached latest
        latest.or_else(|| {
            let pos = *self.latest_position.lock();
            if pos.confidence > 0.0 {
                Some(pos)
            } else {
                None
            }
        })
    }

    /// Get latest position without consuming from queue
    pub fn peek_position(&self) -> HeadPosition {
        *self.latest_position.lock()
    }

    /// Run calibration
    ///
    /// Captures the current face position as the reference "center" position.
    /// Call this when the user is at their normal listening position.
    pub fn calibrate(&mut self) -> Result<(), HeadTrackerError> {
        info!("Running calibration...");

        let config = self.config.lock().clone();

        // Open camera temporarily for calibration
        let mut camera = CameraCapture::new(config.camera_index, config.target_fps);
        camera.open()?;

        // Capture a few frames and average
        let backend = MacOSVisionBackend::new(config.min_confidence);
        let mut calibration_detections: Vec<FaceDetection> = Vec::new();

        for _ in 0..10 {
            thread::sleep(Duration::from_millis(100));

            if let Ok(frame) = camera.capture_frame() {
                if let Ok(faces) = backend.detect_faces(&frame) {
                    if let Some(face) = faces.first() {
                        calibration_detections.push(face.clone());
                    }
                }
            }
        }

        camera.close();

        if calibration_detections.is_empty() {
            return Err(HeadTrackerError::Vision(
                "No face detected during calibration".to_string(),
            ));
        }

        // Average the detections
        let n = calibration_detections.len() as f32;
        let avg_center_x: f32 =
            calibration_detections.iter().map(|d| d.bounding_box.center_x()).sum::<f32>() / n;
        let avg_center_y: f32 =
            calibration_detections.iter().map(|d| d.bounding_box.center_y()).sum::<f32>() / n;
        let avg_size: f32 = calibration_detections
            .iter()
            .map(|d| d.bounding_box.area().sqrt())
            .sum::<f32>()
            / n;

        // Get camera resolution
        let (width, height) = (640, 480); // TODO: get from camera

        // Update calibration data
        let mut cal = self.calibration.lock();
        cal.reference_face_rect.x = avg_center_x - avg_size / 2.0;
        cal.reference_face_rect.y = avg_center_y - avg_size / 2.0;
        cal.reference_face_rect.width = avg_size;
        cal.reference_face_rect.height = avg_size;
        cal.reference_face_size = avg_size;
        cal.camera_width = width;
        cal.camera_height = height;
        cal.is_valid = true;

        info!(
            "Calibration complete: center=({:.3}, {:.3}), size={:.3}",
            avg_center_x, avg_center_y, avg_size
        );

        Ok(())
    }

    /// Update configuration
    pub fn set_config(&mut self, config: HeadTrackerConfig) {
        *self.config.lock() = config;
    }

    /// Get current configuration
    pub fn config(&self) -> HeadTrackerConfig {
        self.config.lock().clone()
    }

    /// Check if calibration is valid
    pub fn is_calibrated(&self) -> bool {
        self.calibration.lock().is_valid
    }
}

impl Default for HeadTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for HeadTracker {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Background thread that performs tracking
fn tracking_thread(
    config: Arc<Mutex<HeadTrackerConfig>>,
    position_queue: Arc<ArrayQueue<HeadPosition>>,
    calibration: Arc<Mutex<CalibrationData>>,
    stop_signal: Arc<AtomicBool>,
    is_tracking: Arc<AtomicBool>,
    latest_position: Arc<Mutex<HeadPosition>>,
) {
    let cfg = config.lock().clone();

    // Initialize camera
    let mut camera = CameraCapture::new(cfg.camera_index, cfg.target_fps);
    if let Err(e) = camera.open() {
        error!("Failed to open camera: {}", e);
        return;
    }

    // Initialize vision backend
    let backend = MacOSVisionBackend::new(cfg.min_confidence);

    // Initialize smoother
    let mut smoother = HeadPositionSmoother::new(cfg.smoothing_time_s);

    // Frame timing
    let frame_duration = Duration::from_secs_f64(1.0 / cfg.target_fps as f64);
    let mut last_detection: Option<FaceDetection> = None;
    let mut lost_face_counter = 0u32;

    is_tracking.store(true, Ordering::Relaxed);
    info!("Tracking thread started");

    while !stop_signal.load(Ordering::Relaxed) {
        let frame_start = Instant::now();

        // Get current config (may have changed)
        let cfg = config.lock().clone();

        // Capture frame
        match camera.capture_frame() {
            Ok(frame) => {
                // Run face detection
                match backend.detect_faces(&frame) {
                    Ok(faces) => {
                        if let Some(face) = faces.first() {
                            // Got a face
                            last_detection = Some(face.clone());
                            lost_face_counter = 0;

                            // Get calibration data
                            let cal = calibration.lock();

                            // Convert to head position
                            let raw_position = if cal.is_valid {
                                face.to_head_position(
                                    cal.reference_face_rect.center_x(),
                                    cal.reference_face_rect.center_y(),
                                    cal.reference_face_size,
                                    cfg.camera_distance_m,
                                    cfg.camera_fov_deg,
                                    frame.timestamp_ms,
                                )
                            } else {
                                // No calibration - use center as reference
                                face.to_head_position(
                                    0.5,
                                    0.5,
                                    face.bounding_box.area().sqrt(),
                                    cfg.camera_distance_m,
                                    cfg.camera_fov_deg,
                                    frame.timestamp_ms,
                                )
                            };

                            // Apply smoothing
                            let smoothed = smoother.update(raw_position);

                            // Update latest position
                            *latest_position.lock() = smoothed;

                            // Push to queue (non-blocking, drops old values if full)
                            let _ = position_queue.force_push(smoothed);

                            debug!(
                                "Position: x={:.3}m z={:.3}m yaw={:.1}° conf={:.2}",
                                smoothed.x, smoothed.z, smoothed.yaw, smoothed.confidence
                            );
                        } else {
                            // No face detected
                            lost_face_counter += 1;

                            if lost_face_counter > cfg.lost_face_hold_frames {
                                // Face lost for too long, reduce confidence
                                if let Some(last) = &last_detection {
                                    let mut pos = last.to_head_position(
                                        0.5, 0.5, 0.2,
                                        cfg.camera_distance_m,
                                        cfg.camera_fov_deg,
                                        frame.timestamp_ms,
                                    );
                                    pos.confidence = 0.0;
                                    *latest_position.lock() = pos;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Face detection error: {}", e);
                    }
                }
            }
            Err(e) => {
                warn!("Frame capture error: {}", e);
            }
        }

        // Maintain frame rate
        let elapsed = frame_start.elapsed();
        if elapsed < frame_duration {
            thread::sleep(frame_duration - elapsed);
        }
    }

    camera.close();
    is_tracking.store(false, Ordering::Relaxed);
    info!("Tracking thread stopped");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_head_tracker_creation() {
        let tracker = HeadTracker::new();
        assert!(!tracker.is_tracking());
        assert!(!tracker.is_calibrated());
    }

    #[test]
    fn test_head_tracker_config() {
        let mut config = HeadTrackerConfig::default();
        config.target_fps = 60;

        let tracker = HeadTracker::with_config(config.clone());
        assert_eq!(tracker.config().target_fps, 60);
    }

    #[test]
    fn test_position_queue() {
        let tracker = HeadTracker::new();

        // Initially no position
        assert!(tracker.get_position().is_none());
    }
}
