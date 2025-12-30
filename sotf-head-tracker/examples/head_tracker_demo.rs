// ============================================================================
// Head Tracker Demo
// ============================================================================
//
// Demonstrates the head tracking system and how it could be integrated
// with the XTC plugin for real-time crosstalk cancellation adjustment.
//
// Run with:
// cargo run --example head_tracker_demo -p sotf-head-tracker --release

use sotf_head_tracker::{HeadPosition, HeadPositionSmoother, HeadTracker, HeadTrackerConfig};
use std::time::Duration;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== Head Tracker Demo ===\n");

    // Create head tracker with custom configuration
    let config = HeadTrackerConfig {
        enabled: true,
        target_fps: 30,
        smoothing_time_s: 0.1,
        min_confidence: 0.4,
        camera_index: 0,
        camera_distance_m: 0.6, // Typical laptop distance
        camera_fov_deg: 60.0,
        position_threshold_m: 0.01, // 1cm
        angle_threshold_deg: 1.0,
        lost_face_hold_frames: 10,
    };

    println!("Configuration:");
    println!("  Target FPS: {}", config.target_fps);
    println!("  Smoothing: {}s", config.smoothing_time_s);
    println!("  Camera distance: {}m", config.camera_distance_m);
    println!("  Camera FOV: {}°", config.camera_fov_deg);
    println!();

    // Demonstrate the smoother
    println!("--- Testing Position Smoother ---\n");
    demo_smoother();

    // Demonstrate the tracker (if camera is available)
    println!("\n--- Testing Head Tracker ---\n");
    demo_tracker(config);

    println!("\n=== Demo Complete ===");
}

fn demo_smoother() {
    let mut smoother = HeadPositionSmoother::new(0.1); // 100ms time constant

    println!("Simulating jittery input being smoothed:");

    // Simulate jittery position data
    let jitter_positions = vec![
        HeadPosition { x: 0.10, z: 0.00, timestamp_ms: 0, confidence: 0.9, ..Default::default() },
        HeadPosition { x: 0.12, z: 0.01, timestamp_ms: 33, confidence: 0.9, ..Default::default() },
        HeadPosition { x: 0.09, z: -0.01, timestamp_ms: 66, confidence: 0.9, ..Default::default() },
        HeadPosition { x: 0.11, z: 0.02, timestamp_ms: 100, confidence: 0.9, ..Default::default() },
        HeadPosition { x: 0.08, z: -0.02, timestamp_ms: 133, confidence: 0.9, ..Default::default() },
        HeadPosition { x: 0.10, z: 0.00, timestamp_ms: 166, confidence: 0.9, ..Default::default() },
    ];

    for (i, raw) in jitter_positions.iter().enumerate() {
        let smoothed = smoother.update(*raw);
        println!(
            "  Frame {}: raw=({:+.3}m, {:+.3}m) → smoothed=({:+.3}m, {:+.3}m)",
            i, raw.x, raw.z, smoothed.x, smoothed.z
        );
    }

    // Show convergence to a new position
    println!("\nSimulating head movement to new position:");
    let target_x = 0.15;
    let target_z = 0.05;

    for i in 0..10 {
        let pos = HeadPosition {
            x: target_x,
            z: target_z,
            timestamp_ms: 200 + i * 33,
            confidence: 0.95,
            ..Default::default()
        };
        let smoothed = smoother.update(pos);
        let error = ((smoothed.x - target_x).powi(2) + (smoothed.z - target_z).powi(2)).sqrt();
        println!(
            "  Frame {}: target=({:+.3}m, {:+.3}m), smoothed=({:+.3}m, {:+.3}m), error={:.4}m",
            i + 6, target_x, target_z, smoothed.x, smoothed.z, error
        );
    }
}

fn demo_tracker(config: HeadTrackerConfig) {
    let mut tracker = HeadTracker::with_config(config);

    // Try to start the tracker
    println!("Attempting to start head tracker...");
    println!("(This requires camera access - may prompt for permission)\n");

    match tracker.start() {
        Ok(()) => {
            println!("Head tracker started successfully!");
            println!("Tracking for 3 seconds...\n");

            // Track for a few seconds
            for i in 0..30 {
                std::thread::sleep(Duration::from_millis(100));

                if let Some(pos) = tracker.get_position() {
                    println!(
                        "  [{:>2}] Position: x={:+.3}m, z={:+.3}m, yaw={:+.1}° (conf={:.2})",
                        i, pos.x, pos.z, pos.yaw, pos.confidence
                    );

                    // Show how this would map to XTC plugin parameters
                    if i % 5 == 0 {
                        println!(
                            "       → XTC: head_offset_x={:.3}, head_offset_z={:.3}",
                            pos.x, pos.z
                        );
                    }
                } else {
                    println!("  [{:>2}] No position available", i);
                }
            }

            tracker.stop();
            println!("\nHead tracker stopped.");
        }
        Err(e) => {
            println!("Could not start head tracker: {}", e);
            println!("\nThis is expected if:");
            println!("  - No camera is available");
            println!("  - Camera permission was denied");
            println!("  - Another app is using the camera");
            println!("\nThe smoother demo above shows the filtering algorithm works correctly.");
        }
    }
}

/// Example of how to integrate head tracking with XTC plugin parameters
///
/// This shows the pattern for wiring the head tracker to the audio engine:
///
/// ```rust,ignore
/// use sotf_head_tracker::HeadTracker;
/// use sotf_plugins::{XtcPlugin, ParameterId, ParameterValue};
///
/// fn update_xtc_from_head_tracker(
///     tracker: &HeadTracker,
///     xtc_plugin: &mut XtcPlugin,
///     last_position: &mut HeadPosition,
/// ) {
///     if let Some(pos) = tracker.get_position() {
///         // Only update if position changed significantly
///         if pos.significantly_different(last_position, 0.01, 1.0) {
///             // Update XTC plugin parameters
///             xtc_plugin.set_parameter(
///                 ParameterId::from("head_offset_x"),
///                 ParameterValue::Float(pos.x),
///             ).ok();
///
///             xtc_plugin.set_parameter(
///                 ParameterId::from("head_offset_z"),
///                 ParameterValue::Float(pos.z),
///             ).ok();
///
///             *last_position = pos;
///         }
///     }
/// }
/// ```
#[allow(dead_code)]
fn integration_example() {}
