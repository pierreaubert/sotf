use sotf_host::plugin::ProcessContext;
use sotf_host::{
    CountingAlloc, ParameterId, ParameterValue, ParametricInPlacePlugin,
    ParametricInPlacePluginAdapter, measure_peak_db, run_standard_tests,
};
use sotf_plugin_loudness_compensation::{
    LoudnessCompensationPlugin, LoudnessCompensationPluginParams,
};
use std::f32::consts::PI;
use std::time::Instant;

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
        headroom_normalized: false,
        auto_calibrated: false,
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

    // Preserve-reference is the default policy: the requested contour remains
    // visible and downstream headroom/limiting is the caller's responsibility.
    println!("  Initial: -10.00dB, Measured: {:.2}dB", peak);
    assert!((peak + 4.0).abs() < 1.0);

    // Test 2: Mid Frequency Neutrality
    println!("\n[Test 2] Mid Frequency (preserved reference)");
    let mut buffer = generate_sine(sample_rate, 1000.0, -10.0, num_frames);
    inner.process_in_place(&mut buffer, &ctx).unwrap();
    let peak = measure_peak_db(&buffer[num_frames - 1000..]);
    println!("  Expected: ~ -10.00dB, Measured: {:.2}dB", peak);
    assert!((peak + 10.0).abs() < 1.0);

    println!("\n[Test 3] ISO/Auto high-channel finite output and tail latency");
    let mut auto = LoudnessCompensationPlugin::new(32, 100.0, 6.0, 8000.0, 6.0);
    auto.initialize(sample_rate).unwrap();
    auto.set_parameter(
        ParameterId::from("auto_calibrated"),
        ParameterValue::Bool(true),
    )
    .unwrap();
    auto.set_parameter(ParameterId::from("mode"), ParameterValue::Int(2))
        .unwrap();
    auto.set_parameter(
        ParameterId::from("playback_volume_db"),
        ParameterValue::Float(-20.0),
    )
    .unwrap();
    let context = ProcessContext::new(sample_rate, 128);
    let mut high_channel = vec![0.1; 128 * 32];
    let mut timings = Vec::with_capacity(200);
    for _ in 0..200 {
        high_channel.fill(0.1);
        let start = Instant::now();
        auto.process_in_place(&mut high_channel, &context).unwrap();
        timings.push(start.elapsed());
    }
    timings.sort_unstable();
    let p99 = timings[timings.len() * 99 / 100];
    let maximum = *timings.last().unwrap();
    println!("  32ch/128f p99={p99:?}, max={maximum:?}");
    assert!(high_channel.iter().all(|sample| sample.is_finite()));
    assert!(maximum.as_secs_f64() < 128.0 / sample_rate as f64);

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
