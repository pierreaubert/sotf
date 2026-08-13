use sotf_plugins::{LoudnessMonitorPlugin, Plugin, ProcessContext};
use std::time::Instant;

#[sotf_test::slow]
#[test]
fn test_loudness_monitor_performance_96khz() {
    let channels = 2;
    let sample_rate = 96000;
    let frame_size = 1024;
    let mut plugin = LoudnessMonitorPlugin::new(channels).unwrap();
    plugin.initialize(sample_rate).unwrap();

    let input = vec![0.1; frame_size * channels];
    let mut output = vec![0.0; frame_size * channels];
    let context = ProcessContext::new(sample_rate, frame_size);

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

    println!(
        "
Performance Results (96kHz, {} channels):",
        channels
    );
    println!("Average process time: {:.4} ms", avg_ms);
    println!("Real-time budget:     {:.4} ms", budget_ms);
    println!(
        "CPU usage (estimated): {:.2}%",
        (avg_ms / budget_ms) * 100.0
    );

    // If it takes more than 50% of the budget, it's very risky for real-time
    assert!(
        avg_ms < budget_ms,
        "Loudness monitor is slower than real-time!"
    );
}

#[sotf_test::slow]
#[test]
fn loudness_monitor_performance_matrix_scales_with_explicit_spatial_mode() {
    for sample_rate in [48_000_u32, 96_000, 192_000] {
        for channels in [2_usize, 8, 16, 32, 40] {
            for spatial in [false, true] {
                if spatial && channels > 16 {
                    continue;
                }
                for frame_size in [64_usize, 512, 2_048] {
                    let mut plugin = if spatial {
                        LoudnessMonitorPlugin::new(channels).unwrap().with_spatial()
                    } else {
                        LoudnessMonitorPlugin::new(channels).unwrap()
                    };
                    plugin.initialize(sample_rate).unwrap();
                    let input = vec![0.1; frame_size * channels];
                    let mut output = vec![0.0; input.len()];
                    let context = ProcessContext::new(sample_rate, frame_size);
                    for _ in 0..4 {
                        plugin.process(&input, &mut output, &context).unwrap();
                    }
                    let iterations = 20;
                    let start = Instant::now();
                    for _ in 0..iterations {
                        plugin.process(&input, &mut output, &context).unwrap();
                    }
                    let average = start.elapsed().as_secs_f64() / iterations as f64;
                    let budget = frame_size as f64 / sample_rate as f64;
                    assert!(
                        average < budget,
                        "loudness monitor missed realtime: {sample_rate} Hz, {channels}ch, spatial={spatial}, frames={frame_size}, avg={average:.6}s budget={budget:.6}s"
                    );
                }
            }
        }
    }
}
