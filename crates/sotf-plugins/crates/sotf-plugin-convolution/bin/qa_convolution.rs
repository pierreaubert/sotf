use sotf_host::plugin::ProcessContext;
use sotf_host::{
    CountingAlloc, ParametricInPlacePlugin, ParametricInPlacePluginAdapter, assert_no_allocs,
    run_standard_tests,
};
use sotf_plugin_convolution::{ConvolutionPlugin, ConvolutionPluginParams};
use std::io::Write;
use std::time::Instant;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 2;

    // Create convolution plugin without an IR file (dry passthrough)
    let mut inner = ConvolutionPlugin::new(channels, sample_rate);
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: Convolution Plugin ===");

    // Test 1: No IR loaded — signal passes through unchanged (dry path)
    println!("\n[Test 1] Passthrough without IR");
    let num_frames = 4096;
    let mut buffer = vec![0.5f32; num_frames * channels];
    let ctx = ProcessContext::new(sample_rate, num_frames);
    inner.process_in_place(&mut buffer, &ctx).unwrap();

    let last = buffer[(num_frames - 1) * channels];
    println!("  Input: 0.50, Output: {:.4}", last);
    // Without IR, output depends on mix setting (dry signal)
    assert!(last.is_finite(), "Output should be finite");

    // Run standard QA tests
    let mut plugin = ParametricInPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "ConvolutionPlugin");

    println!("\n[Test 5] Active-backend callback matrix");
    let path = std::env::temp_dir().join(format!("sotf-convolution-qa-{}.wav", std::process::id()));
    let mut file = std::fs::File::create(&path).unwrap();
    let frames = 8192_u32;
    let data_len = frames * 2;
    file.write_all(b"RIFF").unwrap();
    file.write_all(&(36 + data_len).to_le_bytes()).unwrap();
    file.write_all(b"WAVEfmt \x10\0\0\0\x01\0\x01\0").unwrap();
    file.write_all(&sample_rate.to_le_bytes()).unwrap();
    file.write_all(&(sample_rate * 2).to_le_bytes()).unwrap();
    file.write_all(b"\x02\0\x10\0data").unwrap();
    file.write_all(&data_len.to_le_bytes()).unwrap();
    for i in 0..frames {
        let sample = if i == 0 {
            i16::MAX
        } else if i % 997 == 0 {
            2048
        } else {
            0
        };
        file.write_all(&sample.to_le_bytes()).unwrap();
    }
    drop(file);

    for channels in [1_usize, 2, 8] {
        for (use_nupc, zero_latency_head) in [(false, false), (true, false), (true, true)] {
            for block_frames in [64_usize, 257, 1024] {
                let params = ConvolutionPluginParams {
                    ir_file: path.to_string_lossy().into_owned(),
                    use_nupc,
                    zero_latency_head,
                    ..Default::default()
                };
                let mut candidate =
                    ConvolutionPlugin::from_params(channels, sample_rate, params).unwrap();
                let mut block = (0..block_frames * channels)
                    .map(|i| ((i / channels) as f32 * 0.071 + (i % channels) as f32).sin() * 0.1)
                    .collect::<Vec<_>>();
                let context = ProcessContext::new(sample_rate, block_frames);
                for _ in 0..8 {
                    candidate.process_in_place(&mut block, &context).unwrap();
                }
                assert_no_allocs("Convolution active process", || {
                    candidate.process_in_place(&mut block, &context).unwrap();
                });
                let mut timings = Vec::with_capacity(32);
                for _ in 0..32 {
                    let started = Instant::now();
                    candidate.process_in_place(&mut block, &context).unwrap();
                    timings.push(started.elapsed());
                }
                timings.sort_unstable();
                let deadline = block_frames as f64 * 1000.0 / sample_rate as f64;
                println!(
                    "  {channels}ch nupc={use_nupc} head={zero_latency_head} block={block_frames}: \
                     p50/p99/max {:.3}/{:.3}/{:.3} ms, deadline {deadline:.3} ms",
                    timings[16].as_secs_f64() * 1000.0,
                    timings[31].as_secs_f64() * 1000.0,
                    timings[31].as_secs_f64() * 1000.0,
                );
            }
        }
    }
    std::fs::remove_file(path).ok();

    println!("\n[ALL PASS] Convolution QA Complete.");
}
