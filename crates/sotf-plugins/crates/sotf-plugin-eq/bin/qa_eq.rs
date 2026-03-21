use sotf_host::{CountingAlloc, measure_peak_db, run_standard_tests};
use sotf_host::{InPlacePlugin, InPlacePluginAdapter, ProcessContext};
use sotf_plugin_eq::{BiquadFilterConfig, EqPlugin, EqPluginParams};
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
            order: 2,
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
        .set_parameter("band_0_gain".into(), sotf_host::ParameterValue::Float(-6.0))
        .unwrap();
    let mut buffer = generate_sine(sample_rate, 1000.0, -10.0, num_frames);
    inner.process_in_place(&mut buffer, &ctx).unwrap();
    let peak = measure_peak_db(&buffer[num_frames - 1000..]);
    println!("  Expected: -16.00dB, Measured: {:.2}dB", peak);
    assert!((peak + 16.0).abs() < 0.5);

    // Test 3: Orfanidis low shelf — boost below shelf frequency
    println!("\n[Test 3] LowshelfOrf (+6dB at 200Hz): 50Hz sine should be ~6dB louder");
    let params_lowshelf_orf = EqPluginParams {
        filters: vec![BiquadFilterConfig {
            filter_type: "lowshelf_orf".to_string(),
            freq: 200.0,
            q: 1.0,
            db_gain: 6.0,
            order: 2,
        }],
        channel_filters: None,
        auto_gain: Default::default(),
    };
    let mut inner_ls_orf = EqPlugin::from_params(channels, sample_rate, params_lowshelf_orf).unwrap();
    inner_ls_orf.initialize(sample_rate).unwrap();

    let mut buf_50hz = generate_sine(sample_rate, 50.0, -20.0, num_frames);
    inner_ls_orf.process_in_place(&mut buf_50hz, &ctx).unwrap();
    let peak_50hz = measure_peak_db(&buf_50hz[num_frames - 1000..]);
    println!("  Expected: ~-14.00dB, Measured: {:.2}dB", peak_50hz);
    assert!((peak_50hz + 14.0).abs() < 1.0, "50Hz should be boosted ~6dB, got {:.2}dB", peak_50hz);

    println!("\n[Test 3b] LowshelfOrf (+6dB at 200Hz): 5kHz sine should be near-unity");
    let mut buf_5khz_ls = generate_sine(sample_rate, 5000.0, -20.0, num_frames);
    inner_ls_orf.reset();
    inner_ls_orf.process_in_place(&mut buf_5khz_ls, &ctx).unwrap();
    let peak_5khz_ls = measure_peak_db(&buf_5khz_ls[num_frames - 1000..]);
    println!("  Expected: ~-20.00dB, Measured: {:.2}dB", peak_5khz_ls);
    assert!((peak_5khz_ls + 20.0).abs() < 0.5, "5kHz should be unaffected by low shelf, got {:.2}dB", peak_5khz_ls);

    // Test 4: Orfanidis high shelf — boost above shelf frequency
    println!("\n[Test 4] HighshelfOrf (+6dB at 5kHz): 10kHz sine should be ~6dB louder");
    let params_highshelf_orf = EqPluginParams {
        filters: vec![BiquadFilterConfig {
            filter_type: "highshelf_orf".to_string(),
            freq: 5000.0,
            q: 1.0,
            db_gain: 6.0,
            order: 2,
        }],
        channel_filters: None,
        auto_gain: Default::default(),
    };
    let mut inner_hs_orf = EqPlugin::from_params(channels, sample_rate, params_highshelf_orf).unwrap();
    inner_hs_orf.initialize(sample_rate).unwrap();

    let mut buf_10khz = generate_sine(sample_rate, 10000.0, -20.0, num_frames);
    inner_hs_orf.process_in_place(&mut buf_10khz, &ctx).unwrap();
    let peak_10khz = measure_peak_db(&buf_10khz[num_frames - 1000..]);
    println!("  Expected: ~-14.00dB, Measured: {:.2}dB", peak_10khz);
    assert!((peak_10khz + 14.0).abs() < 1.0, "10kHz should be boosted ~6dB, got {:.2}dB", peak_10khz);

    println!("\n[Test 4b] HighshelfOrf (+6dB at 5kHz): 200Hz sine should be near-unity");
    let mut buf_200hz_hs = generate_sine(sample_rate, 200.0, -20.0, num_frames);
    inner_hs_orf.reset();
    inner_hs_orf.process_in_place(&mut buf_200hz_hs, &ctx).unwrap();
    let peak_200hz_hs = measure_peak_db(&buf_200hz_hs[num_frames - 1000..]);
    println!("  Expected: ~-20.00dB, Measured: {:.2}dB", peak_200hz_hs);
    assert!((peak_200hz_hs + 20.0).abs() < 0.5, "200Hz should be unaffected by high shelf, got {:.2}dB", peak_200hz_hs);

    // Test 5: Vicanek matched peak — boost at center frequency
    println!("\n[Test 5] PeakMatched (+6dB at 1kHz, Q=2.0): 1kHz sine should be ~6dB louder");
    let params_peak_matched = EqPluginParams {
        filters: vec![BiquadFilterConfig {
            filter_type: "peak_matched".to_string(),
            freq: 1000.0,
            q: 2.0,
            db_gain: 6.0,
            order: 2,
        }],
        channel_filters: None,
        auto_gain: Default::default(),
    };
    let mut inner_pm = EqPlugin::from_params(channels, sample_rate, params_peak_matched).unwrap();
    inner_pm.initialize(sample_rate).unwrap();

    let mut buf_1khz_pm = generate_sine(sample_rate, 1000.0, -20.0, num_frames);
    inner_pm.process_in_place(&mut buf_1khz_pm, &ctx).unwrap();
    let peak_1khz_pm = measure_peak_db(&buf_1khz_pm[num_frames - 1000..]);
    println!("  Expected: ~-14.00dB, Measured: {:.2}dB", peak_1khz_pm);
    assert!((peak_1khz_pm + 14.0).abs() < 1.0, "1kHz should be boosted ~6dB by PeakMatched, got {:.2}dB", peak_1khz_pm);

    println!("\n[Test 5b] PeakMatched (+6dB at 1kHz, Q=2.0): 100Hz sine should be near-unity");
    let mut buf_100hz_pm = generate_sine(sample_rate, 100.0, -20.0, num_frames);
    inner_pm.reset();
    inner_pm.process_in_place(&mut buf_100hz_pm, &ctx).unwrap();
    let peak_100hz_pm = measure_peak_db(&buf_100hz_pm[num_frames - 1000..]);
    println!("  Expected: ~-20.00dB, Measured: {:.2}dB", peak_100hz_pm);
    assert!((peak_100hz_pm + 20.0).abs() < 0.5, "100Hz should be unaffected by 1kHz PeakMatched, got {:.2}dB", peak_100hz_pm);

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
