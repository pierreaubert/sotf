//! Stereo depth estimation example
//!
//! Demonstrates how to use stereo cameras for better depth accuracy.
//!
//! Run with: cargo run --example stereo_depth

use head_scanner::*;
use nalgebra::Point2;
use stereo::{StereoConfig, StereoDepthEstimator};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("Stereo Depth Estimation Example");
    println!("================================\n");

    // 1. Create stereo configuration
    println!("1. Configuring stereo camera system...");
    let baseline_cm = 6.0; // 6cm between cameras (typical for webcams)
    let config = StereoConfig::default_webcam_stereo(1280, 720, baseline_cm);
    println!("   Baseline: {} cm", config.baseline);
    println!("   Focal length: {:.1} px", config.left_intrinsics.fx);

    // 2. Create stereo depth estimator
    let estimator = StereoDepthEstimator::new(config);

    // 3. Simulate matching features between left and right cameras
    println!("\n2. Matching features between stereo pair...");

    let left_features = vec![
        Point2::new(640.0, 360.0), // Center of image
        Point2::new(400.0, 300.0),
        Point2::new(800.0, 400.0),
    ];

    let right_features = vec![
        Point2::new(630.0, 360.0), // Disparity of 10 pixels
        Point2::new(390.0, 300.0), // Disparity of 10 pixels
        Point2::new(790.0, 400.0), // Disparity of 10 pixels
    ];

    println!("   Left features: {} points", left_features.len());
    println!("   Right features: {} points", right_features.len());

    // 4. Match features using epipolar constraints
    println!("\n3. Finding stereo correspondences...");
    let matches = estimator.match_stereo_features(&left_features, &right_features);
    println!("   Found {} valid matches", matches.len());

    // 5. Triangulate 3D points
    println!("\n4. Triangulating 3D points...");
    let points_3d = estimator.triangulate_points(&left_features, &right_features)?;
    println!("   Reconstructed {} 3D points", points_3d.len());

    for (i, point) in points_3d.iter().enumerate() {
        println!(
            "   Point {}: ({:.1}, {:.1}, {:.1}) cm",
            i, point.x, point.y, point.z
        );
    }

    println!("\n✓ Stereo depth estimation complete!");
    println!("\nIn a real application:");
    println!("- Would use actual stereo camera images");
    println!("- Would compute dense depth maps");
    println!("- Would perform stereo calibration for accuracy");

    Ok(())
}
