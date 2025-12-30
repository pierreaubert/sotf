// ============================================================================
// macOS Vision Framework Backend
// ============================================================================
//
// Uses Apple's Vision framework for face detection and landmark extraction.
// This provides low-latency, on-device face detection optimized for Apple Silicon.

use crate::camera::CameraFrame;
use crate::types::{FaceRect, HeadPosition, HeadTrackerError};
use log::{debug, info, trace, warn};
use objc2::rc::Retained;
use objc2::AnyThread;
use objc2_core_foundation::CGRect;
use objc2_foundation::{NSArray, NSDictionary, NSData};
use objc2_vision::{
    VNDetectFaceRectanglesRequest, VNFaceObservation, VNImageRequestHandler,
    VNRequest, VNImageOption,
};

/// Face detection result
#[derive(Debug, Clone)]
pub struct FaceDetection {
    /// Bounding box (normalized 0-1 coordinates, origin at top-left)
    pub bounding_box: FaceRect,

    /// Detection confidence (0-1)
    pub confidence: f32,

    /// Yaw angle in degrees (+ = looking right)
    pub yaw: Option<f32>,

    /// Pitch angle in degrees (+ = looking up)
    pub pitch: Option<f32>,

    /// Roll angle in degrees (+ = tilting right)
    pub roll: Option<f32>,
}

impl FaceDetection {
    /// Convert to head position given calibration data
    pub fn to_head_position(
        &self,
        calibration_center_x: f32,
        calibration_center_y: f32,
        calibration_face_size: f32,
        camera_distance_m: f32,
        camera_fov_deg: f32,
        timestamp_ms: u64,
    ) -> HeadPosition {
        // Calculate lateral offset from center
        let face_center_x = self.bounding_box.center_x();
        let face_center_y = self.bounding_box.center_y();

        // Normalize offset from calibration center (-0.5 to 0.5)
        let offset_x = face_center_x - calibration_center_x;
        let offset_y = face_center_y - calibration_center_y;

        // Convert to meters using FOV
        let fov_rad = camera_fov_deg.to_radians();
        let view_width_at_distance = 2.0 * camera_distance_m * (fov_rad / 2.0).tan();

        let x_meters = offset_x * view_width_at_distance;
        let y_meters = -offset_y * view_width_at_distance; // Y is inverted in image coords

        // Estimate Z from face size change
        let current_face_size = self.bounding_box.area().sqrt();
        let z_meters = if calibration_face_size > 0.0 && current_face_size > 0.0 {
            // Face appears larger when closer
            let size_ratio = calibration_face_size / current_face_size;
            camera_distance_m * (size_ratio - 1.0)
        } else {
            0.0
        };

        HeadPosition {
            x: x_meters,
            y: y_meters,
            z: z_meters.clamp(-0.5, 0.5), // Clamp to reasonable range
            yaw: self.yaw.unwrap_or(0.0),
            pitch: self.pitch.unwrap_or(0.0),
            roll: self.roll.unwrap_or(0.0),
            timestamp_ms,
            confidence: self.confidence,
        }
    }
}

/// macOS Vision framework backend for face detection
pub struct MacOSVisionBackend {
    /// Minimum confidence threshold
    min_confidence: f32,
}

impl std::fmt::Debug for MacOSVisionBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MacOSVisionBackend")
            .field("min_confidence", &self.min_confidence)
            .finish()
    }
}

impl MacOSVisionBackend {
    /// Create a new Vision backend
    pub fn new(min_confidence: f32) -> Self {
        info!("Creating macOS Vision backend");
        Self {
            min_confidence: min_confidence.clamp(0.0, 1.0),
        }
    }

    /// Detect faces in a camera frame using Apple Vision framework
    ///
    /// Returns detected faces sorted by size (largest first)
    pub fn detect_faces(&self, frame: &CameraFrame) -> Result<Vec<FaceDetection>, HeadTrackerError> {
        trace!(
            "Detecting faces in {}x{} frame ({} bytes, expected {} bytes for RGB)",
            frame.width,
            frame.height,
            frame.data.len(),
            frame.width as usize * frame.height as usize * 3
        );

        // Convert RGB to JPEG for Vision framework
        // Vision works better with compressed image formats
        let jpeg_data = self.rgb_to_jpeg(frame)?;
        trace!("JPEG data size: {} bytes", jpeg_data.len());

        // Create NSData from JPEG bytes
        let ns_data = NSData::with_bytes(&jpeg_data);

        // Create empty options dictionary
        let options: Retained<NSDictionary<VNImageOption, objc2::runtime::AnyObject>> =
            NSDictionary::new();

        // Create image request handler from data
        let handler: Retained<VNImageRequestHandler> =
            VNImageRequestHandler::initWithData_options(
                VNImageRequestHandler::alloc(),
                &ns_data,
                &options,
            );

        // Create face detection request
        // SAFETY: VNDetectFaceRectanglesRequest::new() is safe to call, it creates a new request object
        let request = unsafe { VNDetectFaceRectanglesRequest::new() };

        // Create array of requests
        let requests: Retained<NSArray<VNRequest>> = unsafe {
            // Cast the specific request to VNRequest
            let request_as_vn: &VNRequest = std::mem::transmute(&*request);
            NSArray::from_slice(&[request_as_vn])
        };

        // Perform the request
        match handler.performRequests_error(&requests) {
            Ok(()) => {
                debug!("Vision request completed successfully");
            }
            Err(error) => {
                let description = error.localizedDescription();
                warn!("Vision request failed: {}", description);
                return Err(HeadTrackerError::Vision(description.to_string()));
            }
        }

        // Get results
        // SAFETY: results() is safe to call after performRequests_error succeeds
        let observations = unsafe { request.results() };

        let mut detections: Vec<FaceDetection> = Vec::new();

        if let Some(faces) = observations {
            trace!("Vision found {} face(s)", faces.len());
            for face_obj in faces.iter() {
                // Get the VNFaceObservation
                let face: &VNFaceObservation = unsafe { std::mem::transmute(face_obj) };

                // Get bounding box (Vision uses bottom-left origin, we need top-left)
                let bbox: CGRect = unsafe { objc2::msg_send![face, boundingBox] };

                // Get confidence
                let confidence: f32 = unsafe { objc2::msg_send![face, confidence] };

                if confidence < self.min_confidence {
                    continue;
                }

                // Convert coordinate system (flip Y)
                let bounding_box = FaceRect {
                    x: bbox.origin.x as f32,
                    y: 1.0 - (bbox.origin.y as f32 + bbox.size.height as f32),
                    width: bbox.size.width as f32,
                    height: bbox.size.height as f32,
                };

                // Try to get yaw/pitch/roll (available in newer macOS versions)
                let yaw = Self::get_face_angle(face, "yaw");
                let pitch = Self::get_face_angle(face, "pitch");
                let roll = Self::get_face_angle(face, "roll");

                detections.push(FaceDetection {
                    bounding_box,
                    confidence,
                    yaw,
                    pitch,
                    roll,
                });

                debug!(
                    "Face detected: bbox=({:.2}, {:.2}, {:.2}x{:.2}), conf={:.2}, yaw={:?}",
                    bounding_box.x, bounding_box.y, bounding_box.width, bounding_box.height,
                    confidence, yaw
                );
            }
        } else {
            trace!("Vision returned no observations (results() returned None)");
        }

        // Sort by face size (largest first)
        detections.sort_by(|a, b| {
            b.bounding_box
                .area()
                .partial_cmp(&a.bounding_box.area())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        debug!("Detected {} faces", detections.len());
        Ok(detections)
    }

    /// Get face angle property (yaw, pitch, or roll) in degrees
    fn get_face_angle(face: &VNFaceObservation, property: &str) -> Option<f32> {
        unsafe {
            let sel = match property {
                "yaw" => objc2::sel!(yaw),
                "pitch" => objc2::sel!(pitch),
                "roll" => objc2::sel!(roll),
                _ => return None,
            };

            let number: *const objc2_foundation::NSNumber = objc2::msg_send![face, performSelector: sel];
            if number.is_null() {
                None
            } else {
                // Convert from radians to degrees
                let radians: f64 = (*number).doubleValue();
                Some((radians * 180.0 / std::f64::consts::PI) as f32)
            }
        }
    }

    /// Convert RGB frame to JPEG data
    ///
    /// Vision framework works better with JPEG than raw RGB
    fn rgb_to_jpeg(&self, frame: &CameraFrame) -> Result<Vec<u8>, HeadTrackerError> {
        use std::io::Cursor;

        // Create image buffer
        let img = image::RgbImage::from_raw(
            frame.width,
            frame.height,
            frame.data.clone(),
        ).ok_or_else(|| HeadTrackerError::Vision("Invalid RGB data".to_string()))?;

        // Encode as JPEG
        let mut jpeg_data = Cursor::new(Vec::new());
        img.write_to(&mut jpeg_data, image::ImageFormat::Jpeg)
            .map_err(|e| HeadTrackerError::Vision(format!("JPEG encoding failed: {}", e)))?;

        Ok(jpeg_data.into_inner())
    }

    /// Set minimum confidence threshold
    pub fn set_min_confidence(&mut self, confidence: f32) {
        self.min_confidence = confidence.clamp(0.0, 1.0);
    }
}

impl Default for MacOSVisionBackend {
    fn default() -> Self {
        Self::new(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_face_detection_to_head_position() {
        let detection = FaceDetection {
            bounding_box: FaceRect {
                x: 0.4,
                y: 0.4,
                width: 0.2,
                height: 0.2,
            },
            confidence: 0.95,
            yaw: Some(5.0),
            pitch: Some(-3.0),
            roll: Some(0.0),
        };

        let pos = detection.to_head_position(
            0.5, 0.5, 0.2, 0.6, 60.0, 1000,
        );

        assert!(pos.x.abs() < 0.1, "X offset should be near zero");
        assert!(pos.confidence > 0.9);
        assert_eq!(pos.yaw, 5.0);
    }

    #[test]
    fn test_backend_creation() {
        let backend = MacOSVisionBackend::new(0.5);
        assert_eq!(backend.min_confidence, 0.5);
    }

    #[test]
    fn test_face_rect_methods() {
        let rect = FaceRect {
            x: 0.25,
            y: 0.25,
            width: 0.5,
            height: 0.5,
        };
        assert!((rect.center_x() - 0.5).abs() < 0.001);
        assert!((rect.center_y() - 0.5).abs() < 0.001);
        assert!((rect.area() - 0.25).abs() < 0.001);
    }
}
