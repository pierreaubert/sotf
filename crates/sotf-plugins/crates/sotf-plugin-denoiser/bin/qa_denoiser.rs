use sotf_host::plugin::{InPlacePlugin, ProcessContext};
use sotf_host::ParametricInPlacePluginAdapter;
use sotf_host::{ParametricInPlacePluginAdapter, ParametricInPlacePlugin, CountingAlloc, run_standard_tests};
use sotf_plugin_denoiser::{DenoiserPlugin, DenoiserPluginParams};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let params = DenoiserPluginParams::default();

    let mut inner = DenoiserPlugin::from_params(channels, params);
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: Denoiser Plugin ===");

    // Test 1: Silence should remain silent
    println!("\n[Test 1] Silence passthrough");
    let num_frames = 48000; // 1 second for MCRA to converge
    let mut buffer = vec![0.0f32; num_frames * channels];
    let block_frames = 2048;
    for chunk in buffer.chunks_mut(block_frames * channels) {
        let frames = chunk.len() / channels;
        let ctx = ProcessContext::new(sample_rate, frames);
        inner.process_in_place(chunk, &ctx).unwrap();
    }

    let peak = buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    println!("  Peak after silence: {:.6}", peak);
    assert!(peak < 0.01, "Silence should remain near-silent");

    // Run standard QA tests
    let mut plugin = ParametricInPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "DenoiserPlugin");

    println!("\n[ALL PASS] Denoiser QA Complete.");
}
