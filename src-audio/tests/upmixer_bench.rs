use rand::Rng;
use sotf_audio::plugins::{UpmixerPlugin, UpmixerPluginParams};
use std::time::Instant;

#[test]
fn benchmark_upmixer_process() {
    // Setup
    let fft_size = 2048;
    let sample_rate = 44100;
    let mut plugin = UpmixerPlugin::new(
        fft_size, "5.1", 1.0,   // gain_front_direct
        0.5,   // gain_front_ambient
        1.0,   // gain_rear_ambient
        120.0, // lfe_cutoff_hz
        0.5,   // stereo_width
        300.0, // bandpass_hz
        0.2,   // height_gain
        1.0,   // lfe_gain
    );

    // Generate random input
    let mut rng = rand::rng();
    let input_len = fft_size * 2; // Stereo
    let input: Vec<f32> = (0..input_len)
        .map(|_| rng.random::<f32>() * 2.0 - 1.0)
        .collect();

    // Output buffer (5.1 = 6 channels)
    let num_output_channels = 6;
    let mut output = vec![0.0; fft_size * num_output_channels];

    // Warmup
    for _ in 0..100 {
        plugin.process_fft_block(&input, &mut output);
    }

    // Benchmark
    let iterations = 1000;
    let start = Instant::now();
    for _ in 0..iterations {
        plugin.process_fft_block(&input, &mut output);
    }
    let duration = start.elapsed();

    println!("Total time for {} iterations: {:?}", iterations, duration);
    println!("Average time per block: {:?}", duration / iterations as u32);
    println!(
        "Throughput: {:.2} blocks/sec",
        iterations as f64 / duration.as_secs_f64()
    );
}
