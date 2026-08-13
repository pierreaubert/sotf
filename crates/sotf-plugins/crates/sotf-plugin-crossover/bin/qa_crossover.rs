use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_host::{CountingAlloc, assert_no_allocs, run_standard_tests};
use sotf_plugin_crossover::{CrossoverPlugin, CrossoverPluginParams, PerChannelOpMode};
use std::f32::consts::PI;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let params = CrossoverPluginParams {
        crossover_type: "LR24".to_string(),
        frequency: 1000.0,
        output: "both".to_string(),
        extra_frequencies: vec![],
        fir_taps: None,
        channel_frequencies_hz: vec![],
        channel_modes: vec![],
    };

    let mut plugin = CrossoverPlugin::from_params(channels, &params).unwrap();
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: Crossover Plugin ===");

    // Test 1: Low frequency stays in low band
    // 2-way "both" with 1 channel → output is 2 channels: [low, high] per frame
    println!("\n[Test 1] 100Hz signal → low band");
    let num_frames = 4096;
    let input = generate_sine(sample_rate, 100.0, -10.0, num_frames);
    let out_ch = 2; // 2 bands * 1 channel
    let mut output = vec![0.0f32; num_frames * out_ch];
    let ctx = ProcessContext::new(sample_rate, num_frames);
    plugin.process(&input, &mut output, &ctx).unwrap();

    // Low band = output[frame * 2 + 0], high band = output[frame * 2 + 1]
    let low_energy: f32 = (0..num_frames)
        .map(|f| {
            let s = output[f * out_ch];
            s * s
        })
        .sum::<f32>()
        / num_frames as f32;
    let high_energy: f32 = (0..num_frames)
        .map(|f| {
            let s = output[f * out_ch + 1];
            s * s
        })
        .sum::<f32>()
        / num_frames as f32;
    println!(
        "  Low band energy: {:.6}, High band energy: {:.6}",
        low_energy, high_energy
    );
    assert!(
        low_energy > high_energy * 10.0,
        "100Hz should be mostly in low band"
    );

    // Run standard QA tests
    run_standard_tests(&mut plugin, "CrossoverPlugin");

    println!("\n[Test 2] Complex steady topologies");
    for mut topology in [
        CrossoverPlugin::new_multiway(2, "LR24", 300.0, "both", &[1_200.0, 5_000.0]).unwrap(),
        CrossoverPlugin::new_multiway(2, "FIR", 300.0, "both", &[1_200.0, 5_000.0]).unwrap(),
        CrossoverPlugin::new_per_channel(
            "LR24",
            vec![120.0, 2_500.0],
            vec![PerChannelOpMode::Lowpass, PerChannelOpMode::Highpass],
        )
        .unwrap(),
    ] {
        topology.initialize(sample_rate).unwrap();
        let input = vec![0.1; 256 * topology.input_channels()];
        let mut output = vec![0.0; 256 * topology.output_channels()];
        assert_no_allocs("complex crossover topology", || {
            topology
                .process(&input, &mut output, &ProcessContext::new(sample_rate, 256))
                .unwrap();
        });
        assert!(output.iter().all(|sample| sample.is_finite()));
    }
    println!("  LR four-way, FIR four-way, and per-channel: PASS");

    println!("\n[ALL PASS] Crossover QA Complete.");
}

fn generate_sine(sr: u32, freq: f32, db: f32, frames: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    (0..frames)
        .map(|i| (2.0 * PI * freq * i as f32 / sr as f32).sin() * amp)
        .collect()
}
