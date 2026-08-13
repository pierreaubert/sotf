use super::nupc_engine::NupcEngine;
use super::types::plan_partitions;

#[test]
fn test_partition_planning() {
    let specs = plan_partitions(8192, 256);
    assert!(!specs.is_empty());

    // First spec should use min_block
    assert_eq!(specs[0].block_size, 256);

    // Verify coverage: total samples covered >= IR length
    let total: usize = specs.iter().map(|s| s.count * s.block_size).sum();
    assert!(total >= 8192, "Partitions cover {total} samples, need 8192");

    // Verify doubling pattern
    let mut prev_size = 0;
    for spec in &specs {
        assert!(
            spec.block_size >= prev_size,
            "Block sizes should be non-decreasing"
        );
        prev_size = spec.block_size;
    }
}

#[test]
fn test_partition_planning_short_ir() {
    let specs = plan_partitions(100, 256);
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].count, 1);
}

#[test]
fn test_partition_planning_empty() {
    let specs = plan_partitions(0, 256);
    assert!(specs.is_empty());
}

#[test]
fn test_nupc_impulse_response() {
    // Create a simple IR: [1, 0, 0, 0, ...]
    let mut ir = vec![0.0f32; 512];
    ir[0] = 1.0;

    let mut engine = NupcEngine::new(&ir, 256);

    // Process a known signal through
    let input: Vec<f32> = (0..1024)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 48000.0).sin())
        .collect();

    let mut output = vec![0.0f32; 1024];
    engine.process_block(&input, &mut output);

    // With unit impulse IR, output should match input (after initial latency)
    let latency = engine.latency_samples();
    for i in latency..1024 {
        let error = (output[i] - input[i - latency]).abs();
        assert!(
            error < 0.01,
            "Sample {i}: expected {:.4}, got {:.4} (error {error:.6})",
            input[i - latency],
            output[i]
        );
    }
}

#[test]
fn test_nupc_vs_upc_simple() {
    // Use a short decaying IR
    let ir_len = 2048;
    let ir: Vec<f32> = (0..ir_len)
        .map(|i| (-i as f32 / 500.0).exp() * (i as f32 * 0.1).sin() * 0.5)
        .collect();

    let mut nupc = NupcEngine::new(&ir, 256);

    // Process a signal
    let sig_len = 4096;
    let input: Vec<f32> = (0..sig_len)
        .map(|i| (i as f32 * 0.05).sin() * 0.3)
        .collect();

    let mut output = vec![0.0f32; sig_len];
    nupc.process_block(&input, &mut output);

    // Verify output is finite and non-zero after latency
    let latency = nupc.latency_samples();
    let post_latency = &output[latency + 256..];
    let has_signal = post_latency.iter().any(|&x| x.abs() > 1e-6);
    assert!(has_signal, "NUPC should produce non-zero output");

    for (i, &x) in output.iter().enumerate() {
        assert!(x.is_finite(), "Sample {i} is not finite: {x}");
    }
}

#[test]
fn test_nupc_reset() {
    let ir = vec![1.0f32; 512];
    let mut engine = NupcEngine::new(&ir, 256);

    let input = vec![1.0f32; 512];
    let mut output = vec![0.0; 512];
    engine.process_block(&input, &mut output);

    engine.reset();

    // After reset, processing silence should give silence
    let silence = vec![0.0f32; 512];
    let mut output2 = vec![0.0; 512];
    engine.process_block(&silence, &mut output2);

    for (i, &x) in output2.iter().enumerate() {
        assert!(
            x.abs() < 0.01,
            "After reset, sample {i} should be near zero, got {x}"
        );
    }
}

#[test]
fn instantiated_channels_share_immutable_ir_kernels() {
    let kernel = super::NupcKernel::new(&vec![0.25; 4096], 256);
    let left = kernel.instantiate();
    let right = kernel.instantiate();
    assert!(left.shares_ir_kernel_with(&right));
}

fn direct_convolution(input: &[f32], ir: &[f32]) -> Vec<f32> {
    let mut output = vec![0.0; input.len() + ir.len() - 1];
    for (n, &sample) in input.iter().enumerate() {
        for (k, &tap) in ir.iter().enumerate() {
            output[n + k] += sample * tap;
        }
    }
    output
}

fn assert_matches_delayed_oracle(
    mut engine: NupcEngine,
    input: &[f32],
    ir: &[f32],
    latency: usize,
) {
    let oracle = direct_convolution(input, ir);
    let mut actual = vec![0.0; latency + oracle.len() + 512];
    for (i, sample) in actual.iter_mut().enumerate() {
        *sample = engine.process_sample(input.get(i).copied().unwrap_or(0.0));
    }
    for (i, expected) in oracle.iter().enumerate() {
        let got = actual[latency + i];
        assert!(
            (got - expected).abs() < 2.0e-4,
            "oracle mismatch at convolution sample {i}: got {got}, expected {expected}"
        );
    }
}

#[test]
fn nupc_preserves_absolute_offsets_across_partition_levels() {
    let min_block = 64;
    let mut ir = vec![0.0; 1025];
    for (index, gain) in [
        (0, 0.5),
        (127, -0.25),
        (128, 0.75),
        (511, 0.2),
        (1024, -0.1),
    ] {
        ir[index] = gain;
    }
    let input: Vec<f32> = (0..257)
        .map(|i| ((i * 17 % 31) as f32 - 15.0) / 31.0)
        .collect();
    assert_matches_delayed_oracle(NupcEngine::new(&ir, min_block), &input, &ir, min_block);
}

#[test]
fn zero_latency_head_preserves_tail_offset() {
    let min_block = 64;
    let head_taps = 17;
    let mut ir = vec![0.0; 513];
    for (index, gain) in [(0, 0.5), (16, -0.25), (17, 0.75), (64, 0.2), (512, -0.1)] {
        ir[index] = gain;
    }
    let input: Vec<f32> = (0..129)
        .map(|i| ((i * 11 % 23) as f32 - 11.0) / 23.0)
        .collect();
    assert_matches_delayed_oracle(
        NupcEngine::new_with_head(&ir, min_block, head_taps),
        &input,
        &ir,
        0,
    );
}
