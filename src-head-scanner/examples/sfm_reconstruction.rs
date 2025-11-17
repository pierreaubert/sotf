//! Structure-from-Motion reconstruction example
//!
//! Demonstrates how to use SfM to build a 3D model from multiple views.
//!
//! Run with: cargo run --example sfm_reconstruction

use head_scanner::*;
use reconstruction::{CameraIntrinsics, SfMReconstructor};
use vision::Feature;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("Structure-from-Motion Reconstruction Example");
    println!("============================================\n");

    // 1. Initialize SfM reconstructor
    println!("1. Initializing SfM reconstructor...");
    let intrinsics = CameraIntrinsics::default_webcam(1280, 720);
    let mut sfm = SfMReconstructor::new(intrinsics);

    // 2. Simulate capturing multiple frames with features
    println!("\n2. Processing frames...");
    let num_frames = 30;

    for frame_idx in 0..num_frames {
        // Simulate detected features that move slightly between frames
        let features = generate_synthetic_features(frame_idx);

        sfm.add_frame(features)?;

        if (frame_idx + 1) % 10 == 0 {
            println!("   Processed {} frames", frame_idx + 1);
        }
    }

    // 3. Get reconstructed 3D points
    println!("\n3. Retrieving reconstructed 3D points...");
    let points_3d = sfm.get_points();
    println!("   Reconstructed {} 3D points", points_3d.len());

    // 4. Optional: Apply bundle adjustment for refinement
    println!("\n4. Applying bundle adjustment (optional)...");
    println!("   Bundle adjustment would refine camera poses and point positions");
    println!("   (Skipped in this example for simplicity)");

    println!("\n✓ SfM reconstruction complete!");
    println!("\nIn a real application:");
    println!("- Features would come from actual camera frames");
    println!("- Bundle adjustment would improve accuracy");
    println!("- The 3D points would form a dense point cloud");

    Ok(())
}

/// Generate synthetic features for testing
fn generate_synthetic_features(frame_idx: usize) -> Vec<Feature> {
    vec![
        Feature::new(
            640.0 + (frame_idx as f32 * 2.0).cos() * 20.0,
            360.0,
            "nose".to_string(),
            0.9,
        ),
        Feature::new(
            600.0 + (frame_idx as f32 * 2.0).cos() * 15.0,
            340.0 + (frame_idx as f32 * 2.0).sin() * 5.0,
            "left_eye".to_string(),
            0.85,
        ),
        Feature::new(
            680.0 + (frame_idx as f32 * 2.0).cos() * 15.0,
            340.0 + (frame_idx as f32 * 2.0).sin() * 5.0,
            "right_eye".to_string(),
            0.85,
        ),
        Feature::new(
            620.0 + (frame_idx as f32 * 2.0).cos() * 18.0,
            360.0 + (frame_idx as f32 * 2.0).sin() * 10.0,
            "left_ear".to_string(),
            0.75,
        ),
        Feature::new(
            660.0 + (frame_idx as f32 * 2.0).cos() * 18.0,
            360.0 + (frame_idx as f32 * 2.0).sin() * 10.0,
            "right_ear".to_string(),
            0.75,
        ),
        Feature::new(
            640.0 + (frame_idx as f32 * 2.0).cos() * 20.0,
            400.0 + (frame_idx as f32 * 2.0).sin() * 8.0,
            "mouth".to_string(),
            0.88,
        ),
    ]
}
