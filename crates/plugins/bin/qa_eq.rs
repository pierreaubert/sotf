use sotf_plugins::plugin_eq::{BiquadFilterConfig, EqPlugin, EqPluginParams};
use sotf_plugins::{CountingAlloc, run_standard_tests, generate_dc, measure_peak_db};
use sotf_plugins::{InPlacePlugin, InPlacePluginAdapter, ProcessContext};
use std::f32::consts::PI;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let params = EqPluginParams {
        filters: vec![BiquadFilterConfig {
            filter_type: "peak".to_string(),
            freq: 1000.0,
            q: 1.0,
            db_gain: 6.0,
        }],
        channel_filters: None,
        auto_gain: Default::default(),
    };

    let mut inner = EqPlugin::from_params(channels, sample_rate, params).unwrap();
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: EQ Plugin ===");

    // Test 1: Boost at peak frequency
    println!("\n[Test 1] Peak Boost (+6dB at 1kHz)");
    let num_frames = 4800;
    let mut buffer = generate_sine(sample_rate, 1000.0, -10.0, num_frames);
    let ctx = ProcessContext {
        sample_rate,
        num_frames,
    };
    inner.process_in_place(&mut buffer, &ctx).unwrap();
    let peak = measure_peak_db(&buffer[num_frames - 1000..]); // Measure settled
    println!("  Expected: -4.00dB, Measured: {:.2}dB", peak);
    assert!((peak + 4.0).abs() < 0.5);

    // Test 2: Cut at peak frequency
    println!("\n[Test 2] Peak Cut (-6dB at 1kHz)");
    inner
        .set_parameter(
            "band_0_gain".into(),
            sotf_plugins::ParameterValue::Float(-6.0),
        )
        .unwrap();
    let mut buffer = generate_sine(sample_rate, 1000.0, -10.0, num_frames);
    inner.process_in_place(&mut buffer, &ctx).unwrap();
    let peak = measure_peak_db(&buffer[num_frames - 1000..]);
    println!("  Expected: -16.00dB, Measured: {:.2}dB", peak);
    assert!((peak + 16.0).abs() < 0.5);

    // Run standard QA tests
    let mut plugin = InPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "EqPlugin");

    println!("\n[ALL PASS] EQ QA Complete.");
}

fn generate_sine(sr: u32, freq: f32, db: f32, frames: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    (0..frames)
        .map(|i| (2.0 * PI * freq * i as f32 / sr as f32).sin() * amp)
        .collect()
}

