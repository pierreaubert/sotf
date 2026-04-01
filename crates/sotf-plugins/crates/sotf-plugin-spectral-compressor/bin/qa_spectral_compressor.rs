use sotf_host::plugin::{InPlacePlugin, InPlacePluginAdapter, ProcessContext};
use sotf_host::{CountingAlloc, run_standard_tests};
use sotf_plugin_spectral_compressor::{SpectralCompressorPlugin, SpectralCompressorPluginParams};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let params = SpectralCompressorPluginParams::default();

    let mut inner = SpectralCompressorPlugin::from_params(channels, params);
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: SpectralCompressor Plugin ===");

    // Test 1: Process audio through STFT pipeline
    println!("\n[Test 1] STFT processing passthrough");
    let num_frames = 48000; // 1 second for STFT convergence
    let mut buffer = vec![0.5f32; num_frames * channels];
    let ctx = ProcessContext {
        sample_rate,
        num_frames,
    };
    inner.process_in_place(&mut buffer, &ctx).unwrap();

    // Output should be finite
    assert!(
        buffer.iter().all(|s| s.is_finite()),
        "All output samples should be finite"
    );
    println!("  All samples finite: PASS");

    // Run standard QA tests
    let mut plugin = InPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "SpectralCompressorPlugin");

    println!("\n[ALL PASS] SpectralCompressor QA Complete.");
}
