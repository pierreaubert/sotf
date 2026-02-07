use sotf_plugins::{LoudnessMonitorPlugin, Plugin, ProcessContext};
use std::time::{Duration, Instant};

#[test]
fn test_loudness_monitor_performance_96khz() {
    let channels = 2;
    let sample_rate = 96000;
    let frame_size = 1024;
    let mut plugin = LoudnessMonitorPlugin::new(channels).unwrap();
    plugin.initialize(sample_rate).unwrap();

    let input = vec![0.1; frame_size * channels];
    let mut output = vec![0.0; frame_size * channels];
    let context = ProcessContext {
        sample_rate,
        num_frames: frame_size,
    };

    // Warm up
    for _ in 0..100 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    let iterations = 1000;
    let start = Instant::now();
    for _ in 0..iterations {
        plugin.process(&input, &mut output, &context).unwrap();
    }
    let duration = start.elapsed();
    let avg_ms = duration.as_secs_f64() * 1000.0 / iterations as f64;
    
    // 1024 frames at 96kHz is ~10.67ms of audio
    let budget_ms = (frame_size as f64 / sample_rate as f64) * 1000.0;
    
    println!("
Performance Results (96kHz, {} channels):", channels);
    println!("Average process time: {:.4} ms", avg_ms);
    println!("Real-time budget:     {:.4} ms", budget_ms);
    println!("CPU usage (estimated): {:.2}%", (avg_ms / budget_ms) * 100.0);

    // If it takes more than 50% of the budget, it's very risky for real-time
    assert!(avg_ms < budget_ms, "Loudness monitor is slower than real-time!");
}
