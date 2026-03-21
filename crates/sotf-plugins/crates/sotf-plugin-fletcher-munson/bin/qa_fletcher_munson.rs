use sotf_host::{CountingAlloc, measure_peak_db, run_standard_tests};
use sotf_host::{InPlacePlugin, InPlacePluginAdapter, ProcessContext};
use sotf_plugin_fletcher_munson::{FletcherMunsonPlugin, FletcherMunsonPluginParams};
use std::f32::consts::PI;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000u32;
    let channels = 1usize;
    let num_frames = 4800usize;

    println!("=== QA: Fletcher-Munson Plugin ===");

    // -------------------------------------------------------------------------
    // Test 1: At reference level, the plugin must be near-unity (no-op).
    //
    // When playback_volume_db == reference_level_db, delta = 0 so all band
    // gains are 0 and the compensation smoother targets 1.0 — pass-through.
    // We allow ±1 dB tolerance to cover filter ringing and smoother lag.
    // -------------------------------------------------------------------------
    println!("\n[Test 1] Near-unity at reference level");
    let params = FletcherMunsonPluginParams {
        playback_volume_db: 0.0,
        reference_level_db: 0.0,
        enabled: true,
        auto_gain_enabled: false,
        band1: None,
        band2: None,
        band3: None,
        band4: None,
        smoothing_ms: 0.0,
    };
    let mut inner = FletcherMunsonPlugin::from_params(channels, params).unwrap();
    inner.initialize(sample_rate).unwrap();

    let mut buffer = generate_sine(sample_rate, 1000.0, -10.0, num_frames);
    let ctx = ProcessContext {
        sample_rate,
        num_frames,
    };
    inner.process_in_place(&mut buffer, &ctx).unwrap();
    let peak = measure_peak_db(&buffer[num_frames - 1000..]);
    println!("  Expected: ~-10.00 dB  Measured: {:.2} dB", peak);
    assert!(
        (peak + 10.0).abs() < 1.0,
        "Test 1 FAILED: expected ~-10 dB at unity, got {:.2} dB",
        peak
    );

    // -------------------------------------------------------------------------
    // Test 2: Below reference level, bass and treble boosted relative to mids.
    //
    // With playback=-20 dB and reference=0 dB, delta=20 dB.
    // Default slopes: band1(60Hz)=0.6, band2(250Hz)=0.4, band4(12kHz)=0.3.
    // Max band gain = 0.6 * 20 = 12 dB (band1), so compensation = -12 dB.
    //
    // Net level at each frequency:
    //   50 Hz  → boosted by band1 lowshelf  (~+12 dB) - 12 dB comp ≈  0 dB rel
    //   1 kHz  → no band boost              (  0 dB) - 12 dB comp ≈ -12 dB rel
    //   10 kHz → boosted by band4 highshelf (~+6 dB)  - 12 dB comp ≈  -6 dB rel
    //
    // All inputs start at -10 dB. We verify:
    //   peak_50Hz  > peak_1kHz  (bass boosted vs mids)
    //   peak_10kHz > peak_1kHz  (treble boosted vs mids)
    // -------------------------------------------------------------------------
    println!("\n[Test 2] Bass and treble boosted relative to mids below reference");
    let params_below = FletcherMunsonPluginParams {
        playback_volume_db: -20.0,
        reference_level_db: 0.0,
        enabled: true,
        auto_gain_enabled: false,
        band1: None,
        band2: None,
        band3: None,
        band4: None,
        smoothing_ms: 0.0,
    };
    let mut plugin_below = FletcherMunsonPlugin::from_params(channels, params_below).unwrap();
    plugin_below.initialize(sample_rate).unwrap();

    // Process a long buffer (10× normal) so the gain smoothers converge fully.
    let settle_frames = num_frames * 10;

    // Measure 50 Hz (sub-bass)
    let mut buf_50 = generate_sine(sample_rate, 50.0, -10.0, settle_frames);
    let ctx_settle = ProcessContext {
        sample_rate,
        num_frames: settle_frames,
    };
    plugin_below.process_in_place(&mut buf_50, &ctx_settle).unwrap();
    let peak_50 = measure_peak_db(&buf_50[settle_frames - 2000..]);

    // Re-initialize between frequency tests to reset filter state.
    plugin_below.initialize(sample_rate).unwrap();

    // Measure 1 kHz (mids — reference frequency, minimal boost)
    let mut buf_1k = generate_sine(sample_rate, 1000.0, -10.0, settle_frames);
    plugin_below.process_in_place(&mut buf_1k, &ctx_settle).unwrap();
    let peak_1k = measure_peak_db(&buf_1k[settle_frames - 2000..]);

    plugin_below.initialize(sample_rate).unwrap();

    // Measure 10 kHz (air/treble)
    let mut buf_10k = generate_sine(sample_rate, 10000.0, -10.0, settle_frames);
    plugin_below.process_in_place(&mut buf_10k, &ctx_settle).unwrap();
    let peak_10k = measure_peak_db(&buf_10k[settle_frames - 2000..]);

    println!("  50 Hz  peak: {:.2} dB", peak_50);
    println!("  1 kHz  peak: {:.2} dB  (reference, should be lowest)", peak_1k);
    println!("  10 kHz peak: {:.2} dB", peak_10k);

    assert!(
        peak_50 > peak_1k,
        "Test 2 FAILED: 50 Hz ({:.2} dB) should be louder than 1 kHz ({:.2} dB)",
        peak_50,
        peak_1k
    );
    assert!(
        peak_10k > peak_1k,
        "Test 2 FAILED: 10 kHz ({:.2} dB) should be louder than 1 kHz ({:.2} dB)",
        peak_10k,
        peak_1k
    );

    // -------------------------------------------------------------------------
    // Standard RT-safety and latency tests
    // -------------------------------------------------------------------------
    let params_rt = FletcherMunsonPluginParams {
        playback_volume_db: -10.0,
        reference_level_db: 0.0,
        enabled: true,
        auto_gain_enabled: false,
        band1: None,
        band2: None,
        band3: None,
        band4: None,
        smoothing_ms: 0.0,
    };
    let inner_rt = FletcherMunsonPlugin::from_params(channels, params_rt).unwrap();
    let mut plugin_rt = InPlacePluginAdapter::new(inner_rt);
    run_standard_tests(&mut plugin_rt, "FletcherMunsonPlugin");

    println!("\n[ALL PASS] Fletcher-Munson QA Complete.");
}

fn generate_sine(sr: u32, freq: f32, db: f32, frames: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    (0..frames)
        .map(|i| (2.0 * PI * freq * i as f32 / sr as f32).sin() * amp)
        .collect()
}
