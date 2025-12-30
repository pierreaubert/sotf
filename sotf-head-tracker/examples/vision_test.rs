// ============================================================================
// Vision Framework Test
// ============================================================================
//
// Tests the Vision face detection with a generated test image.
// This bypasses camera capture to verify the Vision integration works.
//
// Run with:
// cargo run --example vision_test -p sotf-head-tracker --release

use sotf_head_tracker::backend::{FaceDetection, MacOSVisionBackend};
use sotf_head_tracker::camera::CameraFrame;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    println!("=== Vision Framework Test ===\n");

    // Create a simple test frame (solid gray image)
    // Vision won't find faces in this, but it will test the pipeline
    let width = 640u32;
    let height = 480u32;
    let data: Vec<u8> = vec![128u8; (width * height * 3) as usize]; // Gray RGB image

    let frame = CameraFrame {
        data,
        width,
        height,
        timestamp_ms: 0,
    };

    println!("Created test frame: {}x{} ({} bytes)", width, height, frame.data.len());

    // Create Vision backend
    let backend = MacOSVisionBackend::new(0.5);
    println!("Created Vision backend\n");

    // Try to detect faces
    println!("Running face detection...");
    match backend.detect_faces(&frame) {
        Ok(faces) => {
            println!("✓ Vision pipeline works! Found {} faces", faces.len());
            for (i, face) in faces.iter().enumerate() {
                println!(
                    "  Face {}: bbox=({:.2}, {:.2}, {:.2}x{:.2}), conf={:.2}",
                    i,
                    face.bounding_box.x,
                    face.bounding_box.y,
                    face.bounding_box.width,
                    face.bounding_box.height,
                    face.confidence
                );
            }
            if faces.is_empty() {
                println!("  (No faces expected in solid gray test image)");
            }
        }
        Err(e) => {
            println!("✗ Vision error: {}", e);
        }
    }

    println!("\n=== Test Complete ===");
}
