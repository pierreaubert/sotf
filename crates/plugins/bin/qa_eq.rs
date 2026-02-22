use sotf_plugins::plugin_eq::{EqPlugin, EqPluginParams, BiquadFilterConfig};
use sotf_plugins::{InPlacePlugin, ProcessContext};
use std::f32::consts::PI;

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let params = EqPluginParams {
        filters: vec![
            BiquadFilterConfig {
                filter_type: "peak".to_string(),
                freq: 1000.0,
                q: 1.0,
                db_gain: 6.0,
            }
        ],
        channel_filters: None,
        auto_gain: Default::default(),
    };

    let mut plugin = EqPlugin::from_params(channels, sample_rate, params).unwrap();
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: EQ Plugin ===");

    // Test 1: Boost at peak frequency
    println!("
[Test 1] Peak Boost (+6dB at 1kHz)");
    let num_frames = 4800;
    let mut buffer = generate_sine(sample_rate, 1000.0, -10.0, num_frames);
    let ctx = ProcessContext { sample_rate, num_frames };
    plugin.process_in_place(&mut buffer, &ctx).unwrap();
    let peak = measure_peak_db(&buffer[num_frames-1000..]); // Measure settled
    println!("  Expected: -4.00dB, Measured: {:.2}dB", peak);
    assert!((peak + 4.0).abs() < 0.5);

    // Test 2: Cut at peak frequency
    println!("
[Test 2] Peak Cut (-6dB at 1kHz)");
    plugin.set_parameter("band_0_gain".into(), sotf_plugins::ParameterValue::Float(-6.0)).unwrap();
    let mut buffer = generate_sine(sample_rate, 1000.0, -10.0, num_frames);
    plugin.process_in_place(&mut buffer, &ctx).unwrap();
    let peak = measure_peak_db(&buffer[num_frames-1000..]);
    println!("  Expected: -16.00dB, Measured: {:.2}dB", peak);
    assert!((peak + 16.0).abs() < 0.5);

    println!("
[PASS] EQ QA Complete.");
}

fn generate_sine(sr: u32, freq: f32, db: f32, frames: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    (0..frames).map(|i| (2.0 * PI * freq * i as f32 / sr as f32).sin() * amp).collect()
}

fn measure_peak_db(buffer: &[f32]) -> f32 {
    let peak = buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    20.0 * peak.max(1e-10).log10()
}
