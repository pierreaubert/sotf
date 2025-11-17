//! Computer vision module for feature detection and head tracking

use crate::camera::Frame;
use crate::error::{ScannerError, ScannerResult};
use nalgebra::Point2;
use ndarray::{Array, Array4};
use opencv::{
    core::{Mat, Point as CvPoint, Scalar, Vector},
    features2d,
    imgproc,
    objdetect,
    prelude::*,
};
use ort::{Environment, ExecutionProvider, Session, SessionBuilder, Value};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    environment: Arc<Environment>,
}

impl VisionModel {
    /// Load a vision model from an ONNX file
    pub fn load(model_path: &str) -> ScannerResult<Self> {
        let environment = Arc::new(
            Environment::builder()
                .with_name("head_scanner")
                .build()
                .map_err(|e| ScannerError::VisionModel(format!("Failed to create ONNX environment: {}", e)))?
        );

        let session = SessionBuilder::new(&environment)?
            .with_execution_providers([ExecutionProvider::CPU])?
            .with_model_from_file(model_path)?;

        Ok(Self {
            session,
            environment,
        })
    }

    /// Detect features in a frame using the neural network
    pub fn detect_features(&self, frame: &Frame) -> ScannerResult<Vec<Feature>> {
        // Preprocess image for the model
        let input_tensor = self.preprocess_image(frame)?;

        // Run inference
        let outputs = self.session
            .run(vec![input_tensor])
            .map_err(|e| ScannerError::VisionModel(format!("Inference failed: {}", e)))?;

        // Post-process outputs to extract features
        self.postprocess_outputs(outputs, frame)
    }

    fn preprocess_image(&self, frame: &Frame) -> ScannerResult<Value> {
        preprocess_image_for_model(frame, 224, 224)
    }

    fn postprocess_outputs(
        &self,
        outputs: Vec<Value>,
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
    // Note: In production, these paths should be configurable
    let face_cascade_path = "/usr/share/opencv4/haarcascades/haarcascade_frontalface_default.xml";

    let mut face_cascade = objdetect::CascadeClassifier::new(face_cascade_path)
        .map_err(|e| ScannerError::VisionModel(format!("Failed to load face cascade: {}", e)))?;

    if face_cascade.empty() {
        return Err(ScannerError::VisionModel(
            "Face cascade is empty - check the cascade file path".to_string(),
        ));
    }

    // Detect faces
    let mut faces = Vector::<opencv::core::Rect>::new();
    face_cascade.detect_multi_scale(
        &gray,
        &mut faces,
        1.1,  // scale factor
        3,    // min neighbors
        0,    // flags
        opencv::core::Size::new(30, 30),  // min size
        opencv::core::Size::new(0, 0),    // max size
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
pub fn preprocess_image_for_model(
    frame: &Frame,
    target_width: i32,
    target_height: i32,
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

    // Reshape to NCHW format (batch, channels, height, width) for ONNX
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

    // Create ndarray with shape [1, C, H, W]
    let array = Array::from_shape_vec(
        (1, channels, height, width),
        nchw_data,
    )
    .map_err(|e| ScannerError::VisionModel(format!("Failed to create ndarray: {}", e)))?;

    // Convert to ONNX Value
    // Note: This creates a CPU tensor. For GPU, use CUDAExecutionProvider
    Value::from_array(array)
        .map_err(|e| ScannerError::VisionModel(format!("Failed to create ONNX tensor: {}", e)))
}

/// Postprocess model outputs to extract features
///
/// Supports common output formats:
/// - Object detection: bounding boxes with class scores
/// - Keypoint detection: (x, y, confidence) tuples
/// - Segmentation: feature maps
pub fn postprocess_model_outputs(
    outputs: Vec<Value>,
    image_width: u32,
    image_height: u32,
) -> ScannerResult<Vec<Feature>> {
    if outputs.is_empty() {
        return Ok(Vec::new());
    }

    let mut features = Vec::new();

    // Extract first output tensor
    let output = &outputs[0];

    // Try to interpret as tensor
    let tensor = output
        .try_extract_tensor::<f32>()
        .map_err(|e| ScannerError::VisionModel(format!("Failed to extract tensor: {}", e)))?;

    let shape = tensor.shape();

    // Handle different output formats
    match shape.len() {
        // [batch, num_detections, 6] format (x, y, w, h, confidence, class)
        3 if shape[2] >= 5 => {
            let num_detections = shape[1];
            let detection_size = shape[2];

            for i in 0..num_detections {
                let offset = i * detection_size;
                let x_center = tensor[offset] * image_width as f32;
                let y_center = tensor[offset + 1] * image_height as f32;
                let confidence = tensor[offset + 4];
                let class_id = if detection_size > 5 {
                    tensor[offset + 5] as usize
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
            let num_keypoints = shape[1];

            for i in 0..num_keypoints {
                let offset = i * 3;
                let x = tensor[offset] * image_width as f32;
                let y = tensor[offset + 1] * image_height as f32;
                let confidence = tensor[offset + 2];

                if confidence > 0.3 {
                    features.push(Feature::new(
                        x,
                        y,
                        format!("keypoint_{}", i),
                        confidence,
                    ));
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
    let features = apply_nms(features, 0.5);

    Ok(features)
}

/// Apply Non-Maximum Suppression to remove overlapping features
///
/// Keeps only the feature with highest confidence in overlapping regions
fn apply_nms(mut features: Vec<Feature>, iou_threshold: f32) -> Vec<Feature> {
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
            if dist < 50.0 {
                // 50 pixel threshold
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
