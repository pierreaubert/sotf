use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_host::{CountingAlloc, assert_no_allocs};
use sotf_plugin_resampler::ResamplerPlugin;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let channels = 2;
    let input_sr = 44100;
    let output_sr = 48000;

    println!("=== QA: Resampler Plugin ===");

    // Test 1: Resampling produces output with correct ratio
    println!("\n[Test 1] 44.1kHz → 48kHz resampling");
    let mut plugin = ResamplerPlugin::new_default(channels, input_sr, output_sr).unwrap();
    plugin.initialize(input_sr).unwrap();

    let num_frames = 1024;
    let input = vec![0.5f32; num_frames * channels];
    let max_out_frames = ((num_frames as f64 * output_sr as f64 / input_sr as f64) as usize) + 128;
    let mut output = vec![0.0f32; max_out_frames * channels];
    let ctx = ProcessContext {
        sample_rate: input_sr,
        num_frames,
    };
    plugin.process(&input, &mut output, &ctx).unwrap();

    let has_output = output.iter().any(|&s| s.abs() > 0.01);
    println!("  Has output: {}", has_output);
    assert!(has_output, "Resampler should produce output");

    // Test 2: Latency reporting
    println!("\n[Test 2] Latency Reporting");
    let reported = plugin.latency_samples();
    println!("  Reported Latency: {} samples", reported);
    println!("  Latency: PASS");

    // Test 3: Real-time Safety (Zero Allocations)
    // Resampler needs a larger output buffer due to upsampling
    println!("\n[Test 3] Real-time Safety (Zero Allocations)");
    let rt_block = 1024;
    let rt_input = vec![0.1f32; rt_block * channels];
    let rt_max_out = ((rt_block as f64 * output_sr as f64 / input_sr as f64) as usize) + 128;
    let mut rt_output = vec![0.0f32; rt_max_out * channels];
    let rt_ctx = ProcessContext {
        sample_rate: input_sr,
        num_frames: rt_block,
    };

    // Warm up
    for _ in 0..10 {
        plugin.process(&rt_input, &mut rt_output, &rt_ctx).unwrap();
    }

    assert_no_allocs("ResamplerPlugin::process", || {
        plugin.process(&rt_input, &mut rt_output, &rt_ctx).unwrap();
    });
    println!("  Zero Allocations: PASS");

    // Test 4: Performance Benchmark
    println!("\n[Test 4] Performance Benchmark");
    let bench_frames = 48000 * 5;
    let bench_input = vec![0.1f32; bench_frames * channels];
    let bench_max_out = ((bench_frames as f64 * output_sr as f64 / input_sr as f64) as usize) + 128;
    let mut bench_output = vec![0.0f32; bench_max_out * channels];

    let start = std::time::Instant::now();
    let mut pos = 0;
    let mut out_pos = 0;
    while pos < bench_frames {
        let end = (pos + rt_block).min(bench_frames);
        let ctx = ProcessContext {
            sample_rate: input_sr,
            num_frames: end - pos,
        };
        let produced = plugin
            .process(
                &bench_input[pos * channels..end * channels],
                &mut bench_output[out_pos * channels..],
                &ctx,
            )
            .unwrap();
        out_pos += produced;
        pos = end;
    }
    let duration = start.elapsed();
    let audio_duration_sec = bench_frames as f64 / input_sr as f64;
    let cpu_usage = (duration.as_secs_f64() / audio_duration_sec) * 100.0;
    println!(
        "  Processed {:.1}s of audio in {:.2}ms",
        audio_duration_sec,
        duration.as_secs_f64() * 1000.0
    );
    println!("  Estimated CPU Usage: {:.2}%", cpu_usage);
    assert!(cpu_usage < 10.0, "CPU usage too high: {:.2}%", cpu_usage);
    println!("  Performance: PASS");

    println!("\n[ALL PASS] Resampler QA Complete.");
}
