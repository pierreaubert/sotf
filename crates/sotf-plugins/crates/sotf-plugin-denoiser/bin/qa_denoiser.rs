use sotf_host::plugin::ProcessContext;
use sotf_host::{
    CountingAlloc, ParameterId, ParameterValue, ParametricInPlacePlugin,
    ParametricInPlacePluginAdapter, assert_no_allocs, run_standard_tests,
};
use sotf_plugin_denoiser::{DenoiserPlugin, DenoiserPluginParams};
use std::time::Instant;

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

    println!("\n[Test 3] Optional-mode allocation/performance matrix");
    for channels in [1_usize, 2, 6, 8] {
        for low_latency in [false, true] {
            for all_modes in [false, true] {
                for block_frames in [64_usize, 257, 4096] {
                    let params = DenoiserPluginParams {
                        low_latency,
                        multi_resolution: all_modes,
                        polyphonic_detection: all_modes,
                        formant_preservation: all_modes,
                        spectral_sub_enabled: all_modes,
                        ..Default::default()
                    };
                    let mut candidate = DenoiserPlugin::try_from_params(channels, params).unwrap();
                    candidate.initialize(sample_rate).unwrap();
                    if all_modes {
                        candidate
                            .parametric_set_parameter(
                                ParameterId::from("harmonic_percussive"),
                                ParameterValue::Bool(true),
                            )
                            .unwrap();
                        candidate
                            .parametric_set_parameter(
                                ParameterId::from("spatial_denoise"),
                                ParameterValue::Bool(true),
                            )
                            .unwrap();
                    }
                    let mut block = (0..block_frames * channels)
                        .map(|i| {
                            let frame = i / channels;
                            let ch = i % channels;
                            (frame as f32 * 0.071 + ch as f32 * 0.37).sin() * 0.1
                                + (frame as f32 * 0.193).cos() * 0.01
                        })
                        .collect::<Vec<_>>();
                    let context = ProcessContext::new(sample_rate, block_frames);
                    for _ in 0..8 {
                        candidate.process_in_place(&mut block, &context).unwrap();
                    }
                    assert_no_allocs("Denoiser optional-mode process", || {
                        candidate.process_in_place(&mut block, &context).unwrap();
                    });
                    let mut timings = Vec::with_capacity(24);
                    for _ in 0..24 {
                        let start = Instant::now();
                        candidate.process_in_place(&mut block, &context).unwrap();
                        timings.push(start.elapsed());
                    }
                    timings.sort_unstable();
                    let p50 = timings[12].as_secs_f64() * 1000.0;
                    let p95 = timings[22].as_secs_f64() * 1000.0;
                    let worst = timings[23].as_secs_f64() * 1000.0;
                    let deadline = block_frames as f64 * 1000.0 / sample_rate as f64;
                    println!(
                        "  {channels}ch fft={} block={block_frames} modes={all_modes}: \
                         p50/p95/max {p50:.3}/{p95:.3}/{worst:.3} ms, deadline {deadline:.3} ms",
                        if low_latency { 512 } else { 2048 }
                    );
                }
            }
        }
    }

    println!("\n[ALL PASS] Denoiser QA Complete.");
}
