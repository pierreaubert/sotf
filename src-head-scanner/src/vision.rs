//! Computer vision module for feature detection and head tracking

use crate::camera::Frame;
use crate::error::{ScannerError, ScannerResult};
use nalgebra::Point2;
use ndarray::{Array, Array4};
use opencv::{
    core::{Mat, Point as CvPoint, Scalar, Vector},
    features2d, imgproc, objdetect,
    prelude::*,
};
use ort::{session::Session, value::Value};
use serde::{Deserialize, Serialize};

/// A detected facial or head feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    /// 2D position in image (pixels)
    pub position: Point2<f32>,

    /// Feature type (e.g., "nose", "eye", "ear")
    pub feature_type: String,

    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,

    /// Depth estimate (optional, in cm)
    pub depth: Option<f32>,
}

impl Feature {
    pub fn new(x: f32, y: f32, feature_type: String, confidence: f32) -> Self {
        Self {
            position: Point2::new(x, y),
            feature_type,
            confidence,
            depth: None,
        }
    }

    pub fn with_depth(mut self, depth: f32) -> Self {
        self.depth = Some(depth);
        self
    }
}

/// Vision model for advanced feature detection using ONNX
pub struct VisionModel {
    session: Session,
    use_gpu: bool,
}

impl VisionModel {
    /// Load a vision model from an ONNX file with optional GPU acceleration
    pub fn load(model_path: &str) -> ScannerResult<Self> {
        Self::load_with_gpu(model_path, true)
    }

    /// Load a vision model with explicit GPU preference
    pub fn load_with_gpu(model_path: &str, prefer_gpu: bool) -> ScannerResult<Self> {
        // In ort 2.0, load model using commit_from_file on the builder
        let model_bytes = std::fs::read(model_path)
            .map_err(|e| ScannerError::VisionModel(format!("Failed to read model file: {}", e)))?;

        let mut builder = Session::builder()
            .map_err(|e| {
                ScannerError::VisionModel(format!("Failed to create session builder: {}", e))
            })?;

        // Try to enable GPU execution providers
        let mut use_gpu = false;
        if prefer_gpu {
            // Note: GPU execution providers require additional setup
            // For now, we'll use CPU but mark GPU as "requested"
            // Future: Add CUDA, CoreML, DirectML when ort crate supports them
            
            #[cfg(target_os = "macos")]
            {
                // Apple Silicon has Neural Engine support via CoreML
                log::info!("GPU acceleration requested (CoreML/Neural Engine on Apple Silicon)");
                // TODO: Enable when ort 2.0 stable supports CoreML
                use_gpu = false; // Will be true when CoreML is available
            }
            
            #[cfg(target_os = "windows")]
            {
                // Windows can use DirectML for GPU acceleration
                log::info!("GPU acceleration requested (DirectML on Windows)");
                // TODO: Enable when ort 2.0 stable supports DirectML
                use_gpu = false; // Will be true when DirectML is available
            }
            
            #[cfg(target_os = "linux")]
            {
                // Linux can use CUDA for NVIDIA GPUs
                log::info!("GPU acceleration requested (CUDA on Linux)");
                // TODO: Enable when ort 2.0 stable supports CUDA
                use_gpu = false; // Will be true when CUDA is available
            }

            if !use_gpu {
                log::info!("GPU execution providers not yet available in ort 2.0-rc.10, using optimized CPU");
                log::info!("Note: ONNX Runtime will still use CPU optimizations (SIMD, multi-threading)");
            }
        } else {
            log::info!("GPU acceleration disabled, using CPU");
        }

        let session = builder
            .commit_from_memory(&model_bytes)
            .map_err(|e| ScannerError::VisionModel(format!("Failed to load model: {}", e)))?;

        // Log model input information
        if let Some(input) = session.inputs.first() {
            log::info!("Model input: name='{}', shape={:?}", input.name, input.input_type);
        }

        Ok(Self { session, use_gpu })
    }

    /// Check if GPU acceleration is enabled
    pub fn is_using_gpu(&self) -> bool {
        self.use_gpu
    }

    /// Detect features in a frame using the neural network
    pub fn detect_features(&mut self, frame: &Frame) -> ScannerResult<Vec<Feature>> {
        // Get model input shape to determine preprocessing
        let (target_height, target_width, channels_last) = if let Some(input) = self.session.inputs.first() {
            // Parse shape from input type
            let shape_str = format!("{:?}", input.input_type);
            log::debug!("Model input shape: {}", shape_str);
            
            // Try to detect if model expects NHWC (channels last) or NCHW (channels first)
            // YOLOv4 typically uses NHWC: [batch, height, width, channels]
            // Most PyTorch models use NCHW: [batch, channels, height, width]
            
            // Heuristic: if last dimension is 3, it's likely NHWC
            let channels_last = shape_str.contains(", 3]") || shape_str.ends_with("3)");
            
            // Try to extract dimensions (default to 416x416 for YOLO, 224x224 for others)
            let (h, w) = if shape_str.contains("416") {
                (416, 416)
            } else {
                (224, 224)
            };
            
            (h, w, channels_last)
        } else {
            (224, 224, false) // Default to NCHW format
        };

        log::debug!("Preprocessing for {}x{}, channels_last={}", target_width, target_height, channels_last);

        // Preprocess image for the model
        let input_tensor = preprocess_image_for_model(frame, target_width, target_height, channels_last)?;

        // Run inference
        let outputs = self
            .session
            .run(ort::inputs![input_tensor])
            .map_err(|e| ScannerError::VisionModel(format!("Inference failed: {}", e)))?;

        // Post-process outputs to extract features
        // In ort 2.0, outputs are accessed by index or name
        postprocess_model_outputs(outputs, frame.width, frame.height)
    }

    fn postprocess_outputs(
        &self,
        outputs: ort::session::SessionOutputs,
        frame: &Frame,
    ) -> ScannerResult<Vec<Feature>> {
        postprocess_model_outputs(outputs, frame.width, frame.height)
    }
}

/// Detect facial features using classical computer vision (Haar cascades)
///
/// This is a fallback method when no ML model is available
pub fn detect_features_classical(frame: &Frame) -> ScannerResult<Vec<Feature>> {
    let mut features = Vec::new();

    // Convert to grayscale for face detection
    let gray = frame.to_gray()?;

    // Load Haar cascade for face detection
    // Try environment variable first, then common platform paths
    let face_cascade_path = std::env::var("OPENCV_HAARCASCADES_PATH")
        .ok()
        .and_then(|base| {
            let full_path = format!("{}/haarcascade_frontalface_default.xml", base);
            if std::path::Path::new(&full_path).exists() {
                Some(full_path)
            } else {
                None
            }
        })
        .or_else(|| {
            // Try common installation paths
            let paths = [
                "/usr/share/opencv4/haarcascades/haarcascade_frontalface_default.xml", // Linux (opencv4)
                "/usr/share/opencv/haarcascades/haarcascade_frontalface_default.xml", // Linux (opencv3)
                "/usr/local/share/opencv4/haarcascades/haarcascade_frontalface_default.xml", // macOS (opencv4)
                "/usr/local/share/opencv/haarcascades/haarcascade_frontalface_default.xml", // macOS (opencv3)
                "C:/opencv/build/etc/haarcascades/haarcascade_frontalface_default.xml", // Windows
            ];

            paths
                .iter()
                .find(|&&p| std::path::Path::new(p).exists())
                .map(|&p| p.to_string())
        })
        .ok_or_else(|| {
            ScannerError::VisionModel(
                "Could not find haarcascade_frontalface_default.xml. \
             Set OPENCV_HAARCASCADES_PATH environment variable or install OpenCV properly."
                    .to_string(),
            )
        })?;

    let mut face_cascade = objdetect::CascadeClassifier::new(&face_cascade_path).map_err(|e| {
        ScannerError::VisionModel(format!(
            "Failed to load face cascade from '{}': {}",
            face_cascade_path, e
        ))
    })?;

    if face_cascade.empty()? {
        return Err(ScannerError::VisionModel(
            "Face cascade is empty - check the cascade file path".to_string(),
        ));
    }

    // Detect faces
    let mut faces = Vector::<opencv::core::Rect>::new();
    face_cascade.detect_multi_scale(
        &gray,
        &mut faces,
        1.1,                             // scale factor
        3,                               // min neighbors
        0,                               // flags
        opencv::core::Size::new(30, 30), // min size
        opencv::core::Size::new(0, 0),   // max size
    )?;

    // Convert detected faces to features
    for face in faces.iter() {
        let center_x = face.x as f32 + face.width as f32 / 2.0;
        let center_y = face.y as f32 + face.height as f32 / 2.0;

        features.push(Feature::new(
            center_x,
            center_y,
            "face".to_string(),
            0.8, // Haar cascades don't provide confidence scores
        ));

        // Estimate facial landmarks within the face region
        // Approximate positions based on typical face proportions
        let face_width = face.width as f32;
        let face_height = face.height as f32;

        // Eyes (approximately 1/3 from top, 1/4 and 3/4 across)
        features.push(Feature::new(
            face.x as f32 + face_width * 0.35,
            face.y as f32 + face_height * 0.4,
            "left_eye".to_string(),
            0.6,
        ));

        features.push(Feature::new(
            face.x as f32 + face_width * 0.65,
            face.y as f32 + face_height * 0.4,
            "right_eye".to_string(),
            0.6,
        ));

        // Nose (center, 2/3 from top)
        features.push(Feature::new(
            center_x,
            face.y as f32 + face_height * 0.65,
            "nose".to_string(),
            0.6,
        ));

        // Mouth (center, 5/6 from top)
        features.push(Feature::new(
            center_x,
            face.y as f32 + face_height * 0.85,
            "mouth".to_string(),
            0.6,
        ));
    }

    Ok(features)
}

/// Estimate depth from stereo images or monocular depth estimation
pub fn estimate_depth(frame: &Frame, features: &[Feature]) -> ScannerResult<Vec<f32>> {
    // Placeholder for depth estimation
    // In a real implementation, this would use:
    // - Stereo matching if multiple cameras available
    // - Monocular depth estimation model
    // - Structure from motion over multiple frames

    // For now, return dummy depth values
    let depth_estimates: Vec<f32> = features.iter().map(|_| 50.0).collect(); // 50cm default

    Ok(depth_estimates)
}

/// Track features across multiple frames for 3D reconstruction
pub struct FeatureTracker {
    /// Previous frame features
    previous_features: Vec<Feature>,

    /// Feature correspondences (tracks)
    tracks: Vec<Vec<Feature>>,
}

impl FeatureTracker {
    pub fn new() -> Self {
        Self {
            previous_features: Vec::new(),
            tracks: Vec::new(),
        }
    }

    /// Update tracker with features from a new frame
    pub fn update(&mut self, features: Vec<Feature>) {
        if self.previous_features.is_empty() {
            self.previous_features = features;
            return;
        }

        // Match features between frames
        // This is a simplified implementation - in practice, use optical flow or feature descriptors
        for feature in &features {
            // Find closest feature from previous frame
            let mut min_dist = f32::MAX;
            let mut closest_idx = 0;

            for (idx, prev_feature) in self.previous_features.iter().enumerate() {
                if prev_feature.feature_type == feature.feature_type {
                    let dist = (feature.position - prev_feature.position).norm();
                    if dist < min_dist {
                        min_dist = dist;
                        closest_idx = idx;
                    }
                }
            }

            // If close enough, add to track
            if min_dist < 50.0 {
                // threshold in pixels
                // Find or create track for this feature
                // Simplified: just add to a new track
                self.tracks.push(vec![feature.clone()]);
            }
        }

        self.previous_features = features;
    }

    /// Get all feature tracks
    pub fn get_tracks(&self) -> &[Vec<Feature>] {
        &self.tracks
    }

    /// Reset the tracker
    pub fn reset(&mut self) {
        self.previous_features.clear();
        self.tracks.clear();
    }
}

impl Default for FeatureTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Preprocess image for ML model inference
///
/// Converts frame to model input format with:
/// - Resizing to target dimensions
/// - Normalization to [0, 1] or ImageNet mean/std
/// - Channel ordering (RGB)
/// - Batch dimension
/// - Support for both NCHW and NHWC formats
pub fn preprocess_image_for_model(
    frame: &Frame,
    target_width: i32,
    target_height: i32,
    channels_last: bool,
) -> ScannerResult<Value> {
    // Convert to RGB
    let rgb = frame.to_rgb()?;

    // Resize to model input size
    let mut resized = Mat::default();
    let size = opencv::core::Size::new(target_width, target_height);
    imgproc::resize(&rgb, &mut resized, size, 0.0, 0.0, imgproc::INTER_LINEAR)?;

    // Convert to float32 and normalize [0, 1]
    let mut float_img = Mat::default();
    resized.convert_to(&mut float_img, opencv::core::CV_32F, 1.0 / 255.0, 0.0)?;

    // Convert OpenCV Mat to ndarray
    let height = float_img.rows() as usize;
    let width = float_img.cols() as usize;
    let channels = float_img.channels() as usize;

    // Extract data from Mat
    let mut data = vec![0.0f32; height * width * channels];
    for y in 0..height {
        for x in 0..width {
            let pixel = float_img.at_2d::<opencv::core::Vec3f>(y as i32, x as i32)?;
            let base_idx = (y * width + x) * channels;
            data[base_idx] = pixel[0]; // R
            data[base_idx + 1] = pixel[1]; // G
            data[base_idx + 2] = pixel[2]; // B
        }
    }

    let final_data = if channels_last {
        // NHWC format: [batch, height, width, channels]
        // Data is already in HWC format, just keep it
        data
    } else {
        // NCHW format: [batch, channels, height, width]
        // Reshape from HWC to CHW
        let mut nchw_data = vec![0.0f32; channels * height * width];
        for c in 0..channels {
            for y in 0..height {
                for x in 0..width {
                    let src_idx = (y * width + x) * channels + c;
                    let dst_idx = c * (height * width) + y * width + x;
                    nchw_data[dst_idx] = data[src_idx];
                }
            }
        }
        nchw_data
    };

    // Convert to ONNX Value with appropriate shape
    // ort 2.0 uses (dimensions, Vec<T>) format instead of ndarray
    // Note: This creates a CPU tensor. For GPU, use CUDAExecutionProvider
    let dimensions = if channels_last {
        // NHWC: [batch, height, width, channels]
        vec![1, height as i64, width as i64, channels as i64]
    } else {
        // NCHW: [batch, channels, height, width]
        vec![1, channels as i64, height as i64, width as i64]
    };
    
    let tensor_value = Value::from_array((dimensions.as_slice(), final_data))
        .map_err(|e| ScannerError::VisionModel(format!("Failed to create ONNX tensor: {}", e)))?;

    Ok(tensor_value.into_dyn())
}

/// Postprocess model outputs to extract features
///
/// Supports common output formats:
/// - Object detection: bounding boxes with class scores
/// - Keypoint detection: (x, y, confidence) tuples
/// - Segmentation: feature maps
pub fn postprocess_model_outputs(
    outputs: ort::session::SessionOutputs,
    image_width: u32,
    image_height: u32,
) -> ScannerResult<Vec<Feature>> {
    let mut features = Vec::new();

    // Extract first output tensor
    // SessionOutputs is an iterator over (name, value) pairs
    let (_name, output) = outputs
        .iter()
        .next()
        .ok_or_else(|| ScannerError::VisionModel("No output tensors from model".to_string()))?;

    // Try to interpret as tensor
    let (shape, data) = output
        .try_extract_tensor::<f32>()
        .map_err(|e| ScannerError::VisionModel(format!("Failed to extract tensor: {}", e)))?;

    // Handle different output formats
    match shape.len() {
        // YOLOv4 format: [batch, grid_h, grid_w, num_anchors, 85]
        // 85 = 4 (bbox: x, y, w, h) + 1 (objectness) + 80 (COCO classes)
        5 if shape[4] == 85 || shape[4] > 80 => {
            log::info!("Detected YOLOv4 output format: {:?}", shape);
            let grid_h = shape[1] as usize;
            let grid_w = shape[2] as usize;
            let num_anchors = shape[3] as usize;
            let num_attrs = shape[4] as usize;
            
            let confidence_threshold = 0.5;
            
            for h in 0..grid_h {
                for w in 0..grid_w {
                    for a in 0..num_anchors {
                        let base_idx = ((h * grid_w + w) * num_anchors + a) * num_attrs;
                        
                        // Extract bbox and objectness
                        let x_center = data[base_idx];
                        let y_center = data[base_idx + 1];
                        let objectness = data[base_idx + 4];
                        
                        // Find best class
                        let mut max_class_score = 0.0f32;
                        let mut best_class = 0;
                        for c in 0..80 {
                            let class_score = data[base_idx + 5 + c];
                            if class_score > max_class_score {
                                max_class_score = class_score;
                                best_class = c;
                            }
                        }
                        
                        // Combined confidence = objectness * class_score
                        let confidence = objectness * max_class_score;
                        
                        if confidence > confidence_threshold {
                            // Convert from grid coordinates to image coordinates
                            let x = (w as f32 + x_center) / grid_w as f32 * image_width as f32;
                            let y = (h as f32 + y_center) / grid_h as f32 * image_height as f32;
                            
                            features.push(Feature::new(
                                x,
                                y,
                                format!("yolo_class_{}", best_class),
                                confidence,
                            ));
                        }
                    }
                }
            }
            
            log::info!("Extracted {} features from YOLOv4 output", features.len());
        }

        // [batch, num_detections, 6] format (x, y, w, h, confidence, class)
        3 if shape[2] >= 5 => {
            let num_detections = shape[1] as usize;
            let detection_size = shape[2] as usize;

            for i in 0..num_detections {
                let offset = i * detection_size;
                let x_center = data[offset] * image_width as f32;
                let y_center = data[offset + 1] * image_height as f32;
                let confidence = data[offset + 4];
                let class_id = if detection_size > 5 {
                    data[offset + 5] as usize
                } else {
                    0
                };

                // Filter by confidence threshold
                if confidence > 0.5 {
                    features.push(Feature::new(
                        x_center,
                        y_center,
                        format!("detection_{}", class_id),
                        confidence,
                    ));
                }
            }
        }

        // [batch, num_keypoints, 3] format (x, y, confidence)
        3 if shape[2] == 3 => {
            let num_keypoints = shape[1] as usize;

            for i in 0..num_keypoints {
                let offset = i * 3;
                let x = data[offset] * image_width as f32;
                let y = data[offset + 1] * image_height as f32;
                let confidence = data[offset + 2];

                if confidence > 0.3 {
                    features.push(Feature::new(x, y, format!("keypoint_{}", i), confidence));
                }
            }
        }

        // Fallback: interpret as flat list of features
        _ => {
            log::warn!(
                "Unexpected output shape: {:?}, attempting fallback parsing",
                shape
            );
            // Create dummy features for testing
            features.push(Feature::new(
                image_width as f32 / 2.0,
                image_height as f32 / 2.0,
                "center".to_string(),
                1.0,
            ));
        }
    }

    // Apply Non-Maximum Suppression to remove overlapping detections
    // Use 5% of image diagonal as threshold for scale-invariance
    let image_diagonal = ((image_width * image_width + image_height * image_height) as f32).sqrt();
    let nms_threshold = image_diagonal * 0.05; // 5% of diagonal
    let features = apply_nms(features, nms_threshold);

    Ok(features)
}

/// Apply Non-Maximum Suppression to remove overlapping features
///
/// Keeps only the feature with highest confidence in overlapping regions
///
/// # Arguments
/// * `features` - Features to filter
/// * `distance_threshold` - Maximum distance (in pixels) for features to be considered overlapping
///
/// # Scale Invariance
/// The threshold should be relative to image dimensions for scale-invariance.
/// For a 1280x720 image, 5% of diagonal ≈ 73 pixels.
/// For a 640x480 image, 5% of diagonal ≈ 40 pixels.
pub fn apply_nms(mut features: Vec<Feature>, distance_threshold: f32) -> Vec<Feature> {
    if features.len() <= 1 {
        return features;
    }

    // Sort by confidence (highest first)
    features.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

    let mut keep = Vec::new();
    let mut suppressed = vec![false; features.len()];

    for i in 0..features.len() {
        if suppressed[i] {
            continue;
        }

        keep.push(features[i].clone());

        // Suppress overlapping features
        for j in (i + 1)..features.len() {
            if suppressed[j] {
                continue;
            }

            // Check if features are close enough to be considered overlapping
            let dist = (features[i].position - features[j].position).norm();
            if dist < distance_threshold {
                suppressed[j] = true;
            }
        }
    }

    keep
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_creation() {
        let feature = Feature::new(100.0, 200.0, "test".to_string(), 0.9);
        assert_eq!(feature.position.x, 100.0);
        assert_eq!(feature.position.y, 200.0);
        assert_eq!(feature.confidence, 0.9);
        assert!(feature.depth.is_none());

        let feature_with_depth = feature.with_depth(50.0);
        assert_eq!(feature_with_depth.depth, Some(50.0));
    }

    #[test]
    fn test_feature_tracker() {
        let mut tracker = FeatureTracker::new();

        let features1 = vec![
            Feature::new(100.0, 100.0, "point1".to_string(), 0.9),
            Feature::new(200.0, 200.0, "point2".to_string(), 0.9),
        ];

        let features2 = vec![
            Feature::new(105.0, 105.0, "point1".to_string(), 0.9),
            Feature::new(205.0, 205.0, "point2".to_string(), 0.9),
        ];

        tracker.update(features1);
        tracker.update(features2);

        assert!(tracker.get_tracks().len() > 0);
    }
}
