// ============================================================================
// XTC Integration Example
// ============================================================================
//
// Demonstrates how to integrate the head tracker with the XTC plugin
// in the sotf-audio-engine. This example simulates the integration
// without requiring the full audio engine.
//
// For real integration, implement HeadTrackingTarget for AudioEngine.
//
// Run with:
// cargo run --example xtc_integration -p sotf-head-tracker --release

use sotf_head_tracker::{
    HeadPosition, HeadPositionSmoother, HeadTrackingBridge, HeadTrackingTarget, XtcHeadParams,
};
use std::time::Duration;

/// Mock audio engine for demonstration
/// In real usage, this would be sotf_audio_engine::AudioEngine
struct MockAudioEngine {
    xtc_head_offset_x: f32,
    xtc_head_offset_z: f32,
    update_count: usize,
}

impl MockAudioEngine {
    fn new() -> Self {
        Self {
            xtc_head_offset_x: 0.0,
            xtc_head_offset_z: 0.0,
            update_count: 0,
        }
    }
}

impl HeadTrackingTarget for MockAudioEngine {
    fn update_xtc_head_params(
        &mut self,
        plugin_index: usize,
        params: &XtcHeadParams,
    ) -> Result<(), String> {
        // In real implementation, this would call:
        // engine.set_plugin_parameter(plugin_index, "head_offset_x", format!("{}", params.head_offset_x))?;
        // engine.set_plugin_parameter(plugin_index, "head_offset_z", format!("{}", params.head_offset_z))?;

        self.xtc_head_offset_x = params.head_offset_x;
        self.xtc_head_offset_z = params.head_offset_z;
        self.update_count += 1;

        println!(
            "  [XTC Plugin {}] Updated: x={:+.3}m, z={:+.3}m",
            plugin_index, params.head_offset_x, params.head_offset_z
        );

        Ok(())
    }
}

fn main() {
    println!("=== XTC Head Tracking Integration Example ===\n");

    // Create components
    let mut engine = MockAudioEngine::new();
    let mut smoother = HeadPositionSmoother::new(0.1); // 100ms smoothing
    let mut bridge = HeadTrackingBridge::new(0) // XTC at plugin index 0
        .with_threshold(0.01); // 1cm threshold

    println!("Configuration:");
    println!("  XTC plugin index: {}", bridge.plugin_index());
    println!("  Update threshold: 1cm");
    println!("  Smoothing: 100ms");
    println!();

    // Simulate head movement sequence
    println!("--- Simulating Head Movement ---\n");

    let movements = vec![
        // (name, x, z, confidence)
        ("Center", 0.0, 0.0, 0.95),
        ("Slight right", 0.02, 0.0, 0.92),      // Under threshold
        ("More right", 0.08, 0.0, 0.90),        // Above threshold
        ("Right + forward", 0.10, -0.05, 0.88), // Forward movement
        ("Right + back", 0.10, 0.08, 0.85),     // Backward movement
        ("Low confidence", 0.20, 0.15, 0.15),   // Should be filtered
        ("Return center", 0.02, 0.01, 0.90),    // Back near center
        ("Center", 0.0, 0.0, 0.93),
    ];

    let mut timestamp_ms = 0u64;
    let frame_duration_ms = 33; // ~30 FPS

    let num_frames = movements.len();
    for (name, raw_x, raw_z, conf) in movements {
        // Create raw position from "camera"
        let raw_pos = HeadPosition {
            x: raw_x,
            y: 0.0,
            z: raw_z,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            timestamp_ms,
            confidence: conf,
        };

        // Apply smoothing
        let smoothed = smoother.update(raw_pos);

        // Update bridge (which filters and sends to engine)
        print!(
            "Frame {:3}: {:20} raw=({:+.2}m, {:+.2}m) → ",
            timestamp_ms / frame_duration_ms,
            name,
            raw_x,
            raw_z
        );

        match bridge.update(&mut engine, &smoothed) {
            Ok(true) => println!("UPDATE SENT"),
            Ok(false) => println!("filtered (below threshold or low conf)"),
            Err(e) => println!("ERROR: {}", e),
        }

        timestamp_ms += frame_duration_ms;
        std::thread::sleep(Duration::from_millis(50)); // Slow down for visibility
    }

    println!("\n--- Summary ---");
    println!("Total frames: {}", num_frames);
    println!("Updates sent: {}", bridge.updates_sent());
    println!(
        "Final engine state: x={:.3}m, z={:.3}m",
        engine.xtc_head_offset_x, engine.xtc_head_offset_z
    );

    // Demonstrate reset
    println!("\n--- Resetting to Center ---");
    bridge.reset(&mut engine).unwrap();
    println!(
        "Engine reset: x={:.3}m, z={:.3}m",
        engine.xtc_head_offset_x, engine.xtc_head_offset_z
    );

    println!("\n=== Example Complete ===");
    println!();
    println!("To integrate with real AudioEngine, implement HeadTrackingTarget:");
    println!();
    println!("  impl HeadTrackingTarget for AudioEngine {{");
    println!("      fn update_xtc_head_params(");
    println!("          &mut self,");
    println!("          plugin_index: usize,");
    println!("          params: &XtcHeadParams,");
    println!("      ) -> Result<(), String> {{");
    println!("          self.set_plugin_parameter(");
    println!("              plugin_index,");
    println!("              \"head_offset_x\".to_string(),");
    println!("              format!(\"{{}}\", params.head_offset_x),");
    println!("          )?;");
    println!("          self.set_plugin_parameter(");
    println!("              plugin_index,");
    println!("              \"head_offset_z\".to_string(),");
    println!("              format!(\"{{}}\", params.head_offset_z),");
    println!("          )");
    println!("      }}");
    println!("  }}");
}
