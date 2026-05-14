// ============================================================================
// Async ML Inference Thread for Vocal Detection
// ============================================================================
//
// Runs ONNX model inference on a separate thread, communicating with the
// audio thread via a lock-free ring buffer (features in) and atomic (V_prob out).
//
// The audio thread never blocks: features are pushed non-blocking via rtrb,
// and V_prob is read via a relaxed atomic load.

use super::ml_features::{CONTEXT_FRAMES, FEATURE_SIZE, FRAME_FEATURE_SIZE};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::{self, JoinHandle};
use tract_onnx::prelude::*;

/// Optimised+runnable tract model. Type alias for the value returned by
/// `TypedModel::into_runnable()`; keeps function signatures readable.
type RunnableOnnxModel = TypedRunnableModel<TypedModel>;

/// Ring buffer capacity in contexts (blocks arrive every ~21ms at 2048/48k with 50% overlap).
const RING_BUFFER_CAPACITY: usize = 4;

/// A single feature context sent from audio thread to inference thread.
pub struct MfccFrame {
    pub features: [f32; FEATURE_SIZE],
}

/// Shared state between audio thread and inference thread
struct SharedState {
    /// V_prob stored as raw f32 bits in AtomicU32
    v_prob_bits: AtomicU32,
    /// Whether at least one inference result is available
    has_result: AtomicBool,
    /// Signal to shut down the inference thread
    shutdown: AtomicBool,
}

/// Audio-thread side handle for the ML inference system.
///
/// Owns the ring buffer producer and a reference to shared atomic state.
/// All methods are non-blocking and safe for real-time use.
pub struct MlInferenceHandle {
    producer: rtrb::Producer<MfccFrame>,
    shared: Arc<SharedState>,
    thread_handle: Option<JoinHandle<()>>,
}

impl MlInferenceHandle {
    /// Create a new inference handle, loading the ONNX model and spawning the worker thread.
    ///
    /// Returns `Err` if the model cannot be loaded.
    pub fn new(model_path: &str) -> Result<Self, String> {
        // Load the raw ONNX proto first so we can validate optional metadata
        // properties (informational; model still loads if absent).
        let proto = tract_onnx::onnx()
            .proto_model_for_path(model_path)
            .map_err(|e| format!("Failed to read ONNX model '{}': {}", model_path, e))?;
        validate_metadata_contract(&proto.metadata_props)?;

        // Build the runnable model from the proto we already parsed.
        let model = tract_onnx::onnx()
            .model_for_proto_model(&proto)
            .map_err(|e| format!("Failed to parse ONNX model '{}': {}", model_path, e))?
            .into_optimized()
            .map_err(|e| format!("Failed to optimise ONNX model: {}", e))?
            .into_runnable()
            .map_err(|e| format!("Failed to make ONNX model runnable: {}", e))?;

        validate_input_contract(&model)?;

        let (producer, consumer) = rtrb::RingBuffer::<MfccFrame>::new(RING_BUFFER_CAPACITY);

        let shared = Arc::new(SharedState {
            v_prob_bits: AtomicU32::new(0.5_f32.to_bits()),
            has_result: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
        });

        let shared_clone = Arc::clone(&shared);
        let thread_handle = thread::Builder::new()
            .name("ml-vocal-detect".to_string())
            .spawn(move || {
                inference_worker(consumer, model, shared_clone);
            })
            .map_err(|e| format!("Failed to spawn inference thread: {}", e))?;

        Ok(Self {
            producer,
            shared,
            thread_handle: Some(thread_handle),
        })
    }

    /// Send feature context to the inference thread. Non-blocking.
    ///
    /// If the ring buffer is full, the frame is silently dropped (inference
    /// is slower than audio — the latest frame that fits will be used).
    #[inline]
    pub fn send_features(&mut self, features: &[f32; FEATURE_SIZE]) {
        let frame = MfccFrame {
            features: *features,
        };
        // Non-blocking push — drop frame if buffer is full
        let _ = self.producer.push(frame);
    }

    /// Read the latest V_prob from the inference thread. Non-blocking.
    ///
    /// Returns `None` until the first inference completes, then returns
    /// `Some(probability)` with the latest vocal detection probability.
    #[inline]
    pub fn read_v_prob(&self) -> Option<f32> {
        if self.shared.has_result.load(Ordering::Relaxed) {
            let bits = self.shared.v_prob_bits.load(Ordering::Relaxed);
            Some(f32::from_bits(bits))
        } else {
            None
        }
    }

    /// Shut down the inference thread and wait for it to finish.
    pub fn shutdown(&mut self) {
        self.shared.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for MlInferenceHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn validate_input_contract(model: &RunnableOnnxModel) -> Result<(), String> {
    let fact = model
        .model()
        .input_fact(0)
        .map_err(|e| format!("ONNX model has no input #0: {}", e))?;

    if fact.datum_type != f32::datum_type() {
        return Err(format!(
            "ONNX model input must be f32, got {:?}",
            fact.datum_type
        ));
    }

    // Convert tract's symbolic shape to a concrete `Vec<i64>`-style view that
    // mirrors the original ort shape contract (dim or -1 wildcard).
    let dims: Vec<i64> = fact
        .shape
        .iter()
        .map(|d| d.to_i64().unwrap_or(-1))
        .collect();

    if !shape_accepts_feature_size(&dims) {
        return Err(format!(
            "ONNX model input must have shape [1, {}] or [-1, {}], got {:?}",
            FEATURE_SIZE, FEATURE_SIZE, dims
        ));
    }
    Ok(())
}

fn shape_accepts_feature_size(shape: &[i64]) -> bool {
    shape.len() == 2
        && (shape[0] == 1 || shape[0] == -1)
        && (shape[1] == FEATURE_SIZE as i64 || shape[1] == -1)
}

fn validate_metadata_contract(
    props: &[tract_onnx::pb::StringStringEntryProto],
) -> Result<(), String> {
    let lookup = |key: &str| -> Option<&str> {
        props
            .iter()
            .find(|p| p.key == key)
            .map(|p| p.value.as_str())
    };

    for (key, expected) in [
        ("feature_size", FEATURE_SIZE),
        ("frame_feature_size", FRAME_FEATURE_SIZE),
        ("context_frames", CONTEXT_FRAMES),
    ] {
        let Some(value) = lookup(key) else {
            continue;
        };
        let parsed = value.parse::<usize>().map_err(|_| {
            format!("ONNX metadata '{}' must be an integer, got '{}'", key, value)
        })?;
        if parsed != expected {
            return Err(format!(
                "ONNX metadata '{}' mismatch: model has {}, plugin expects {}",
                key, parsed, expected
            ));
        }
    }

    if let Some(threshold) = lookup("recommended_threshold") {
        log::info!("ML vocal detector recommended threshold: {}", threshold);
    }

    Ok(())
}

/// Inference worker function running on the dedicated thread.
fn inference_worker(
    mut consumer: rtrb::Consumer<MfccFrame>,
    model: RunnableOnnxModel,
    shared: Arc<SharedState>,
) {
    // Pre-allocate input buffer: shape [1, FEATURE_SIZE]
    let mut input_data = vec![0.0_f32; FEATURE_SIZE];

    loop {
        if shared.shutdown.load(Ordering::Relaxed) {
            break;
        }

        // Drain all available frames, keeping only the latest
        let mut got_frame = false;
        while let Ok(frame) = consumer.pop() {
            input_data.copy_from_slice(&frame.features);
            got_frame = true;
        }

        if got_frame {
            // Run inference
            match run_inference(&model, &input_data) {
                Ok(v_prob) => {
                    let clamped = v_prob.clamp(0.0, 1.0);
                    shared
                        .v_prob_bits
                        .store(clamped.to_bits(), Ordering::Relaxed);
                    shared.has_result.store(true, Ordering::Relaxed);
                }
                Err(e) => {
                    log::warn!("ML inference error: {}", e);
                }
            }
        } else {
            // No frames available — sleep briefly to avoid busy-waiting
            thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

/// Run a single inference pass. Returns the vocal probability (0.0-1.0).
fn run_inference(model: &RunnableOnnxModel, input_data: &[f32]) -> Result<f32, String> {
    let input = tract_ndarray::Array2::from_shape_vec((1, FEATURE_SIZE), input_data.to_vec())
        .map_err(|e| format!("Failed to create input array: {}", e))?;

    let outputs = model
        .run(tvec!(Tensor::from(input).into()))
        .map_err(|e| format!("Inference error: {}", e))?;

    let view = outputs[0]
        .to_array_view::<f32>()
        .map_err(|e| format!("Failed to extract output tensor: {}", e))?;

    view.iter()
        .next()
        .copied()
        .ok_or_else(|| "Empty output tensor".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_state_atomic_roundtrip() {
        let shared = SharedState {
            v_prob_bits: AtomicU32::new(0.0_f32.to_bits()),
            has_result: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
        };

        // Initially no result
        assert!(!shared.has_result.load(Ordering::Relaxed));

        // Store a probability
        let prob = 0.75_f32;
        shared.v_prob_bits.store(prob.to_bits(), Ordering::Relaxed);
        shared.has_result.store(true, Ordering::Relaxed);

        // Read it back
        let bits = shared.v_prob_bits.load(Ordering::Relaxed);
        let read_prob = f32::from_bits(bits);
        assert!((read_prob - prob).abs() < 1e-7);
    }

    #[test]
    fn test_mfcc_frame_size() {
        let frame = MfccFrame {
            features: [0.0; FEATURE_SIZE],
        };
        assert_eq!(frame.features.len(), FEATURE_SIZE);
    }

    #[test]
    fn test_shape_accepts_current_feature_contract() {
        assert!(shape_accepts_feature_size(&[1, FEATURE_SIZE as i64]));
        assert!(shape_accepts_feature_size(&[-1, FEATURE_SIZE as i64]));
        assert!(shape_accepts_feature_size(&[1, -1]));
        assert!(!shape_accepts_feature_size(&[1, 40]));
        assert!(!shape_accepts_feature_size(&[FEATURE_SIZE as i64]));
        assert!(!shape_accepts_feature_size(&[2, FEATURE_SIZE as i64]));
    }

    #[test]
    fn test_inference_handle_with_nonexistent_model() {
        let result = MlInferenceHandle::new("/nonexistent/model.onnx");
        assert!(result.is_err());
    }

    #[test]
    fn test_inference_with_dummy_model() {
        // Find the dummy model relative to the workspace root
        let model_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/test_data/dummy_vocal_detector.onnx"
        );
        if !std::path::Path::new(model_path).exists() {
            eprintln!("Skipping test: dummy model not found at {}", model_path);
            return;
        }

        let mut handle = MlInferenceHandle::new(model_path).expect("Should load dummy model");

        // Send a feature frame
        let features = [0.0_f32; FEATURE_SIZE];
        handle.send_features(&features);

        // Wait for inference to complete (dummy model should be fast)
        let mut v_prob = None;
        for _ in 0..100 {
            v_prob = handle.read_v_prob();
            if v_prob.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let prob = v_prob.expect("Should have received inference result");
        // Dummy model outputs sigmoid(0) = 0.5
        assert!(
            (prob - 0.5).abs() < 0.01,
            "Dummy model should output ~0.5, got {}",
            prob
        );

        handle.shutdown();
    }

    #[test]
    fn test_fallback_when_no_model() {
        // When ML handle is None, read_v_prob should return None
        // This is tested implicitly through the detection.rs dispatch logic,
        // but we verify the handle behavior here
        let model_path = "/nonexistent/model.onnx";
        let result = MlInferenceHandle::new(model_path);
        assert!(result.is_err(), "Should fail for nonexistent model");
    }
}
