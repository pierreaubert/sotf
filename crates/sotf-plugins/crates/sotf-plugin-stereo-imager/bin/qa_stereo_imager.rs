use sotf_host::{CountingAlloc, run_standard_tests};
use sotf_host::plugin::{InPlacePlugin, InPlacePluginAdapter, ProcessContext};
use sotf_plugin_stereo_imager::{StereoImagerPlugin, StereoImagerPluginParams};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 2;
    let params = StereoImagerPluginParams {
        width: 1.0,
        low_mid_freq: 250.0,
        mid_high_freq: 4000.0,
        low_width: 1.0,
        mid_width: 1.0,
        high_width: 1.0,
        mono_bass: false,
        mix: 1.0,
    };

    let mut inner = StereoImagerPlugin::from_params(channels, params);
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: StereoImager Plugin ===");

    // Test 1: Unity width should pass through unchanged
    println!("\n[Test 1] Unity width passthrough");
    let num_frames = 24000;
    let mut buffer = vec![0.0f32; num_frames * channels];
    for i in 0..num_frames {
        buffer[i * channels] = 0.5;     // L
        buffer[i * channels + 1] = 0.3; // R
    }
    let ctx = ProcessContext {
        sample_rate,
        num_frames,
    };
    inner.process_in_place(&mut buffer, &ctx).unwrap();

    let out_l = buffer[(num_frames - 1) * channels];
    let out_r = buffer[(num_frames - 1) * channels + 1];
    println!("  L: in=0.50, out={:.4}", out_l);
    println!("  R: in=0.30, out={:.4}", out_r);
    // At unity width the signal should be close to original
    assert!(
        (out_l - 0.5).abs() < 0.1,
        "Unity width L should be close to input"
    );
    assert!(
        (out_r - 0.3).abs() < 0.1,
        "Unity width R should be close to input"
    );

    // Run standard QA tests
    let mut plugin = InPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "StereoImagerPlugin");

    println!("\n[ALL PASS] StereoImager QA Complete.");
}
