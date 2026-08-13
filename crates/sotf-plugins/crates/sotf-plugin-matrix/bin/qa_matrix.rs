use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_host::{CountingAlloc, run_standard_tests};
use sotf_plugin_matrix::MatrixPlugin;
use std::hint::black_box;
use std::time::Instant;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn report_deadline_distribution(
    name: &str,
    plugin: &mut MatrixPlugin,
    frames: usize,
    iterations: usize,
) {
    let input = vec![0.125; frames * plugin.input_channels()];
    let mut output = vec![0.0; frames * plugin.output_channels()];
    let context = ProcessContext::new(48_000, frames);
    for _ in 0..100 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let started = Instant::now();
        plugin
            .process(
                black_box(&input),
                black_box(&mut output),
                black_box(&context),
            )
            .unwrap();
        samples.push(started.elapsed().as_nanos());
    }
    samples.sort_unstable();
    let p50 = samples[samples.len() / 2] as f64 / 1_000.0;
    let p95 = samples[(samples.len() * 95 / 100).min(samples.len() - 1)] as f64 / 1_000.0;
    let worst = samples[samples.len() - 1] as f64 / 1_000.0;
    let deadline = frames as f64 / 48_000.0 * 1_000_000.0;
    println!(
        "  {name:24} {frames:4} frames: p50={p50:8.3} us p95={p95:8.3} us max={worst:8.3} us deadline={deadline:9.3} us"
    );
    assert!(
        p95 < deadline,
        "{name} missed the {frames}-frame p95 callback deadline: {p95:.3} us >= {deadline:.3} us"
    );
}

fn main() {
    let sample_rate = 48000;
    let input_channels = 2;
    let output_channels = 2;

    let mut plugin = MatrixPlugin::new(input_channels, output_channels);
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: Matrix Plugin ===");

    // Test 1: Identity matrix — passthrough
    println!("\n[Test 1] Identity matrix passthrough");
    let num_frames = 4096;
    let mut input = vec![0.0f32; num_frames * input_channels];
    for i in 0..num_frames {
        input[i * input_channels] = 0.5; // L
        input[i * input_channels + 1] = 0.3; // R
    }
    let mut output = vec![0.0f32; num_frames * output_channels];
    let ctx = ProcessContext::new(sample_rate, num_frames);
    plugin.process(&input, &mut output, &ctx).unwrap();

    let out_l = output[(num_frames - 1) * output_channels];
    let out_r = output[(num_frames - 1) * output_channels + 1];
    println!("  L: in=0.50, out={:.4}", out_l);
    println!("  R: in=0.30, out={:.4}", out_r);
    assert!((out_l - 0.5).abs() < 0.05, "L should pass through identity");
    assert!((out_r - 0.3).abs() < 0.05, "R should pass through identity");

    println!("\n[Test 2] Kernel callback-time distributions");
    let mut cases = Vec::new();
    cases.push(("identity_2x2", MatrixPlugin::new(2, 2)));
    let mut permutation = vec![0.0; 8 * 8];
    for output_channel in 0..8 {
        permutation[output_channel * 8 + 7 - output_channel] = 1.0;
    }
    cases.push((
        "permutation_8x8",
        MatrixPlugin::with_matrix(8, 8, permutation).unwrap(),
    ));
    cases.push((
        "mono_sum_8to1",
        MatrixPlugin::with_matrix(8, 1, vec![0.125; 8]).unwrap(),
    ));
    let dense: Vec<f32> = (0..64)
        .map(|index| (index * 17 % 31 + 1) as f32 / 64.0)
        .collect();
    cases.push(("dense_8x8", MatrixPlugin::with_matrix(8, 8, dense).unwrap()));
    let mut sparse = vec![0.0; 16 * 16];
    for channel in [0, 5, 10, 15] {
        sparse[channel * 16 + channel] = 0.75;
    }
    cases.push((
        "sparse_16x16_four_routes",
        MatrixPlugin::with_matrix(16, 16, sparse).unwrap(),
    ));
    cases.push((
        "mapped_sparse_128_width",
        MatrixPlugin::with_sparse_mapping(
            vec![0, 42, 84, 127],
            vec![1, 43, 85, 127],
            vec![
                0.75, 0.0, 0.0, 0.0, 0.0, 0.75, 0.0, 0.0, 0.0, 0.0, 0.75, 0.0, 0.0, 0.0, 0.0, 0.75,
            ],
        )
        .unwrap(),
    ));
    for (name, mut case) in cases {
        case.initialize(sample_rate).unwrap();
        for frames in [16, 256, 4096] {
            report_deadline_distribution(name, &mut case, frames, 1_000);
        }
    }

    // Run standard QA tests
    run_standard_tests(&mut plugin, "MatrixPlugin");

    println!("\n[ALL PASS] Matrix QA Complete.");
}
