use sotf_host::plugin::ProcessContext;
use sotf_host::{
    CountingAlloc, ParametricInPlacePlugin, ParametricInPlacePluginAdapter, measure_peak_db,
    run_standard_tests,
};
use sotf_plugin_loudness_compensation::{
    LoudnessCompensationPlugin, LoudnessCompensationPluginParams,
};
use std::f32::consts::PI;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let params = LoudnessCompensationPluginParams {
        low_freq: 100.0,
        low_gain: 6.0,
        high_freq: 10000.0,
        high_gain: 6.0,
        mid_enabled: false,
        mid_freq: 1000.0,
        mid_gain: 0.0,
        mid_q: 1.0,
        channel_params: vec![],
        auto_gain_enabled: false,
        auto_gain_max_db: 12.0,
        auto_gain_smoothing_ms: 100.0,
        auto_gain_position: "post".to_string(),
        mode: 0,
        playback_level_db: 70.0,
        playback_volume_db: 0.0,
        reference_level_db: 83.0,
    };

    let mut inner = LoudnessCompensationPlugin::from_params(channels, params).unwrap();
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: Loudness Compensation Plugin ===");

    // Test 1: Low Frequency Boost
    println!("\n[Test 1] Low Boost (+6dB at 50Hz)");
    let num_frames = 4800;
    let mut buffer = generate_sine(sample_rate, 50.0, -10.0, num_frames);
    let ctx = ProcessContext::new(sample_rate, num_frames);
    inner.process_in_place(&mut buffer, &ctx).unwrap();
    let peak = measure_peak_db(&buffer[num_frames - 1000..]);

    // LoudnessComp uses 2 cascaded biquads, so 6dB total boost
    // But it also applies compensation gain to keep peak near unity relative to boost
    // Initial: -10dB. Boost: +6dB. Compensation: -6dB (max boost). Net: -10dB.
    println!("  Initial: -10.00dB, Measured: {:.2}dB", peak);
    assert!((peak + 10.0).abs() < 1.0);

    // Test 2: Mid Frequency Neutrality
    println!("\n[Test 2] Mid Frequency (-6dB attenuation due to compensation)");
    let mut buffer = generate_sine(sample_rate, 1000.0, -10.0, num_frames);
    inner.process_in_place(&mut buffer, &ctx).unwrap();
    let peak = measure_peak_db(&buffer[num_frames - 1000..]);
    println!("  Expected: ~ -16.00dB, Measured: {:.2}dB", peak);
    assert!((peak + 16.0).abs() < 1.0);

    // Run standard QA tests
    let mut plugin = ParametricInPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "LoudnessCompensationPlugin");

    println!("\n[ALL PASS] Loudness Compensation QA Complete.");
}

fn generate_sine(sr: u32, freq: f32, db: f32, frames: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    (0..frames)
        .map(|i| (2.0 * PI * freq * i as f32 / sr as f32).sin() * amp)
        .collect()
}
