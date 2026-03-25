use sotf_plugins::{DawHost, InPlacePluginAdapter, LoudnessMonitorPlugin};
use std::time::Instant;

#[test]
fn test_plugin_chain_performance_with_monitor_96khz() {
    let channels = 2;
    let sample_rate = 96000;
    let frame_size = 1024;

    let mut host = DawHost::new(channels, sample_rate);

    // EQ
    let eq = sotf_plugins::EqPlugin::new(channels, vec![]);
    host.add_plugin(Box::new(InPlacePluginAdapter::new(eq)))
        .unwrap();

    // Compressor
    let comp = sotf_plugins::CompressorPlugin::new(channels);
    host.add_plugin(Box::new(InPlacePluginAdapter::new(comp)))
        .unwrap();

    // Monitor
    let monitor = LoudnessMonitorPlugin::new(channels).unwrap();
    host.add_plugin(Box::new(monitor)).unwrap();

    host.build().unwrap();

    let input = vec![0.1; frame_size * channels];
    let mut output = vec![0.0; frame_size * channels];

    // Warm up
    for _ in 0..100 {
        host.process(&input, &mut output).unwrap();
    }

    let iterations = 1000;
    let start = Instant::now();
    for _ in 0..iterations {
        host.process(&input, &mut output).unwrap();
    }
    let duration = start.elapsed();
    let avg_ms = duration.as_secs_f64() * 1000.0 / iterations as f64;
    let budget_ms = (frame_size as f64 / sample_rate as f64) * 1000.0;

    println!(
        "\nChain Performance Results (96kHz, {} channels):",
        channels
    );
    println!("Average process time: {:.4} ms", avg_ms);
    println!("Real-time budget:     {:.4} ms", budget_ms);
    println!(
        "CPU usage (estimated): {:.2}%",
        (avg_ms / budget_ms) * 100.0
    );
}
