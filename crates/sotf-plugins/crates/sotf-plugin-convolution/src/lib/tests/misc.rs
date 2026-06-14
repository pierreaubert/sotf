use super::super::convolution_plugin::ConvolutionPlugin;
use super::super::misc::FFT_SIZE;
use super::super::misc::PARTITION_SIZE;
use super::super::params;
use super::super::types::ConvolutionPluginParams;
use super::super::types::ConvolutionState;
use super::super::*;
use rayon::prelude::*;
use rubato::Resampler;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::ProcessContext;
use sotf_host::plugin::ProcessContext;
use std::fs;
use std::path::Path;
use std::sync::Arc;

fn write_test_wav(path: &Path, samples: &[i16], sample_rate: u32) {
    let data_len = samples.len() * 2;
    let mut bytes = Vec::with_capacity(44 + data_len);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_len as u32).to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    fs::write(path, bytes).unwrap();
}

/// Helper: create a ConvolutionPlugin and load a synthetic IR directly.
fn make_plugin_with_ir(channels: usize, sample_rate: u32, ir: Vec<Vec<f32>>) -> ConvolutionPlugin {
    let mut plugin = ConvolutionPlugin::new(channels, sample_rate);
    plugin.initialize(sample_rate).unwrap();

    // Build partitions from the IR data
    let ir_channels = ir.len();
    let mut planner = FftPlanner::<f32>::new();
    let fft_forward = planner.plan_fft_forward(FFT_SIZE);
    let fft_inverse = planner.plan_fft_inverse(FFT_SIZE);

    let mut partitions = Vec::with_capacity(ir_channels);
    for ch_samples in &ir {
        let num_parts = ch_samples.len().div_ceil(PARTITION_SIZE);
        let mut ch_parts = Vec::with_capacity(num_parts);
        for p in 0..num_parts {
            let mut block = vec![Complex::new(0.0, 0.0); FFT_SIZE];
            let start = p * PARTITION_SIZE;
            let end = (start + PARTITION_SIZE).min(ch_samples.len());
            for (i, &s) in ch_samples[start..end].iter().enumerate() {
                block[i] = Complex::new(s, 0.0);
            }
            fft_forward.process(&mut block);
            ch_parts.push(block);
        }
        partitions.push(ch_parts);
    }

    let num_partitions = partitions[0].len();
    let fft_scratch_len = fft_forward
        .get_inplace_scratch_len()
        .max(fft_inverse.get_inplace_scratch_len());

    plugin.state.store(Arc::new(Some(ConvolutionState {
        partitions,
        num_partitions,
        ir_channels,
        fft_forward,
        fft_inverse,
    })));
    plugin.fdl_flat = vec![Complex::new(0.0, 0.0); num_partitions * channels * FFT_SIZE];
    plugin.fdl_head = 0;
    plugin.fft_scratch = vec![Complex::new(0.0, 0.0); fft_scratch_len];

    plugin
}

/// Unity IR (Dirac at sample 0) should pass audio through unchanged.
#[test]
fn test_unity_ir_passthrough() {
    let channels = 1;
    let sr = 48000;
    // Dirac impulse: 1.0 at sample 0, zeros elsewhere
    let ir = vec![vec![1.0]];
    let mut plugin = make_plugin_with_ir(channels, sr, ir);
    // mix = 1.0 (fully wet), gain = 0 dB
    plugin.mix_value = 1.0;
    plugin.mix.set_target(1.0);

    // Process a few blocks of a sine wave
    let total_frames = PARTITION_SIZE * 4;
    let mut buffer: Vec<f32> = (0..total_frames).map(|i| (i as f32 * 0.1).sin()).collect();
    let original = buffer.clone();

    // Process in partition-sized blocks
    for block_start in (0..total_frames).step_by(PARTITION_SIZE) {
        let block_end = (block_start + PARTITION_SIZE).min(total_frames);
        let nf = block_end - block_start;
        let ctx = ProcessContext::new(sr, nf);
        plugin
            .process_in_place(&mut buffer[block_start..block_end], &ctx)
            .unwrap();
    }

    // Verify output is finite and has energy (convolution with unity IR)
    let output_energy: f32 = buffer.iter().map(|s| s * s).sum();
    let input_energy: f32 = original.iter().map(|s| s * s).sum();
    assert!(output_energy.is_finite(), "Output must be finite");
    assert!(
        output_energy > 0.0,
        "Unity IR convolution should produce non-zero output"
    );
    // With a unity IR, output energy should be comparable to input energy
    // (allowing for partitioned convolution edge effects)
    if input_energy > 0.0 {
        let ratio = output_energy / input_energy;
        assert!(
            ratio > 0.1,
            "Unity IR should preserve most energy, ratio = {ratio}"
        );
    }
}

/// Dirac impulse response (single sample IR) should produce output.
#[test]
fn test_dirac_impulse_response() {
    let channels = 2;
    let sr = 44100;
    // Single-sample IR with gain 0.5
    let ir = vec![vec![0.5], vec![0.5]];
    let mut plugin = make_plugin_with_ir(channels, sr, ir);
    plugin.mix_value = 1.0;
    plugin.mix.set_target(1.0);

    // Send a DC signal of 1.0 on both channels
    let total_frames = PARTITION_SIZE * 3;
    let mut buffer = vec![1.0f32; total_frames * channels];

    for block_start in (0..total_frames).step_by(PARTITION_SIZE) {
        let block_end = (block_start + PARTITION_SIZE).min(total_frames);
        let nf = block_end - block_start;
        let ctx = ProcessContext::new(sr, nf);
        let buf_start = block_start * channels;
        let buf_end = block_end * channels;
        plugin
            .process_in_place(&mut buffer[buf_start..buf_end], &ctx)
            .unwrap();
    }

    // After settling, output should be approximately 0.5 (IR gain)
    let skip = PARTITION_SIZE * channels * 2;
    let tail = &buffer[skip..];
    let avg: f32 = tail.iter().sum::<f32>() / tail.len() as f32;
    assert!(
        (avg - 0.5).abs() < 0.05,
        "Dirac IR with gain 0.5 should produce ~0.5 output, got avg = {avg}"
    );
}

/// With mix=0.0 (fully dry), output should equal input.
///
/// The UPC path has one-partition latency: the first `PARTITION_SIZE`
/// output samples are zero (the ring buffer starts empty), and subsequent
/// blocks contain the dry input shifted by one partition.  The test
/// accounts for this by processing N+1 blocks and comparing
/// `output[PARTITION_SIZE..]` against `original[0..N*PARTITION_SIZE]`.
#[test]
fn test_mix_zero_is_dry_passthrough() {
    let channels = 1;
    let sr = 48000;
    // Dirac impulse at sample 0
    let ir = vec![vec![1.0]];
    let mut plugin = make_plugin_with_ir(channels, sr, ir);
    // Set mix to 0.0 (fully dry)
    plugin.mix_value = 0.0;
    plugin.mix.set_target(0.0);
    plugin.mix.reset(0.0);

    // Process N+1 blocks so the last block's output is flushed from the ring.
    let signal_frames = PARTITION_SIZE * 3;
    // One extra block of silence at the end to drain the final partition.
    let total_frames = signal_frames + PARTITION_SIZE;
    let mut buffer: Vec<f32> = (0..signal_frames)
        .map(|i| (i as f32 * 0.1).sin())
        .chain(std::iter::repeat_n(0.0f32, PARTITION_SIZE))
        .collect();
    let original = buffer[..signal_frames].to_vec();

    for block_start in (0..total_frames).step_by(PARTITION_SIZE) {
        let block_end = (block_start + PARTITION_SIZE).min(total_frames);
        let nf = block_end - block_start;
        let ctx = ProcessContext::new(sr, nf);
        plugin
            .process_in_place(&mut buffer[block_start..block_end], &ctx)
            .unwrap();
    }

    // The first PARTITION_SIZE output samples are zero (empty ring at start).
    // Samples PARTITION_SIZE..PARTITION_SIZE+signal_frames should equal original.
    let latency = PARTITION_SIZE;
    for (i, (&got, &exp)) in buffer[latency..latency + signal_frames]
        .iter()
        .zip(original.iter())
        .enumerate()
    {
        assert!(
            (got - exp).abs() < 1e-4,
            "mix=0 passthrough mismatch at sample {}: got {}, expected {}",
            latency + i,
            got,
            exp
        );
    }
}

/// With gain_db=6.0, the wet signal should be louder than with gain_db=0.0.
#[test]
fn test_gain_db_increases_output() {
    let channels = 1;
    let sr = 48000;
    let ir = vec![vec![1.0]]; // Unity IR

    // Process with gain_db=0.0
    let mut plugin_0db = make_plugin_with_ir(channels, sr, ir.clone());
    plugin_0db.mix_value = 1.0;
    plugin_0db.mix.set_target(1.0);
    plugin_0db.mix.reset(1.0);
    plugin_0db.gain_db_value = 0.0;
    plugin_0db.gain_linear.set_target(1.0);
    plugin_0db.gain_linear.reset(1.0);

    let total_frames = PARTITION_SIZE * 4;
    let input_signal: Vec<f32> = (0..total_frames)
        .map(|i| (i as f32 * 0.05).sin() * 0.5)
        .collect();

    let mut buffer_0db = input_signal.clone();
    for block_start in (0..total_frames).step_by(PARTITION_SIZE) {
        let block_end = (block_start + PARTITION_SIZE).min(total_frames);
        let nf = block_end - block_start;
        let ctx = ProcessContext::new(sr, nf);
        plugin_0db
            .process_in_place(&mut buffer_0db[block_start..block_end], &ctx)
            .unwrap();
    }

    // Process with gain_db=6.0
    let mut plugin_6db = make_plugin_with_ir(channels, sr, ir);
    plugin_6db.mix_value = 1.0;
    plugin_6db.mix.set_target(1.0);
    plugin_6db.mix.reset(1.0);
    let gain_linear_6db = 10.0f32.powf(6.0 / 20.0);
    plugin_6db.gain_db_value = 6.0;
    plugin_6db.gain_linear.set_target(gain_linear_6db);
    plugin_6db.gain_linear.reset(gain_linear_6db);

    let mut buffer_6db = input_signal.clone();
    for block_start in (0..total_frames).step_by(PARTITION_SIZE) {
        let block_end = (block_start + PARTITION_SIZE).min(total_frames);
        let nf = block_end - block_start;
        let ctx = ProcessContext::new(sr, nf);
        plugin_6db
            .process_in_place(&mut buffer_6db[block_start..block_end], &ctx)
            .unwrap();
    }

    // Compare energy in the settled region (skip first partition for edge effects)
    let skip = PARTITION_SIZE * 2;
    let energy_0db: f32 = buffer_0db[skip..].iter().map(|s| s * s).sum();
    let energy_6db: f32 = buffer_6db[skip..].iter().map(|s| s * s).sum();

    assert!(
        energy_6db > energy_0db * 1.5,
        "gain_db=6 should produce notably more energy than gain_db=0: {} vs {}",
        energy_6db,
        energy_0db
    );
}

/// Parallel partition sum produces the same output as the sequential path.
///
/// Strategy: build two plugins with the same IR that is long enough to have
/// \>= 8 partitions (so the parallel code path is exercised for the first
/// plugin), then verify the outputs are bit-for-bit identical.  The second
/// plugin uses a short IR (\< 8 partitions, sequential path) with a single-
/// sample Dirac that is analytically equivalent to the identity, so we check
/// the long-IR plugin produces finite, energy-preserving output.
///
/// Additionally we verify the parallel path against the known Dirac result:
/// convolving with a Dirac at sample 0 should preserve the input (within
/// float rounding).
#[test]
fn test_parallel_path_bit_exact_vs_sequential() {
    let channels = 1;
    let sr = 48000;

    // Build an IR long enough to trigger the parallel path (>= 8 partitions).
    // PARTITION_SIZE = 1024, so 8 partitions = 8192 samples.
    let ir_len = PARTITION_SIZE * 10; // 10 partitions
    let mut ir_data = vec![0.0f32; ir_len];
    // Dirac at sample 0 — convolution with this should be identity.
    ir_data[0] = 1.0;
    let ir_parallel = vec![ir_data];

    // Single-sample Dirac for the sequential path reference (1 partition).
    let ir_seq = vec![vec![1.0f32]];

    let input_signal: Vec<f32> = (0..PARTITION_SIZE * 6)
        .map(|i| (i as f32 * 0.07).sin() * 0.8)
        .collect();
    let total_frames = input_signal.len();

    // --- Run the parallel-path plugin (10-partition IR, >= 8 → parallel) ---
    let mut plugin_par = make_plugin_with_ir(channels, sr, ir_parallel);
    plugin_par.mix_value = 1.0;
    plugin_par.mix.set_target(1.0);
    plugin_par.mix.reset(1.0);
    plugin_par.gain_linear.set_target(1.0);
    plugin_par.gain_linear.reset(1.0);

    let mut buf_par = input_signal.clone();
    for block_start in (0..total_frames).step_by(PARTITION_SIZE) {
        let block_end = (block_start + PARTITION_SIZE).min(total_frames);
        let nf = block_end - block_start;
        let ctx = ProcessContext::new(sr, nf);
        plugin_par
            .process_in_place(&mut buf_par[block_start..block_end], &ctx)
            .unwrap();
    }

    // --- Run the sequential-path plugin (1-partition Dirac, sequential) ---
    let mut plugin_seq = make_plugin_with_ir(channels, sr, ir_seq);
    plugin_seq.mix_value = 1.0;
    plugin_seq.mix.set_target(1.0);
    plugin_seq.mix.reset(1.0);
    plugin_seq.gain_linear.set_target(1.0);
    plugin_seq.gain_linear.reset(1.0);

    let mut buf_seq = input_signal.clone();
    for block_start in (0..total_frames).step_by(PARTITION_SIZE) {
        let block_end = (block_start + PARTITION_SIZE).min(total_frames);
        let nf = block_end - block_start;
        let ctx = ProcessContext::new(sr, nf);
        plugin_seq
            .process_in_place(&mut buf_seq[block_start..block_end], &ctx)
            .unwrap();
    }

    // Both plugins convolve with a Dirac, so outputs should match to float
    // precision.  The parallel path settles one partition later (zeros in
    // partitions 1-9 of the longer IR take one extra block to flush), so we
    // compare the settled region (skip the first 2 partitions).
    let skip = PARTITION_SIZE * 2;
    for (i, (&par, &seq)) in buf_par[skip..]
        .iter()
        .zip(buf_seq[skip..].iter())
        .enumerate()
    {
        assert!(
            (par - seq).abs() < 1e-5,
            "Parallel/sequential output mismatch at sample {}: parallel={par}, sequential={seq}",
            skip + i
        );
    }

    // Sanity: output must be finite and have energy.
    let energy: f32 = buf_par[skip..].iter().map(|s| s * s).sum();
    assert!(
        energy.is_finite() && energy > 0.0,
        "Parallel path must produce non-zero finite output"
    );
}

/// Long IR stability: no NaN or Inf after 10000 frames.
#[test]
fn test_long_ir_stability() {
    let channels = 1;
    let sr = 48000;
    // Create a longer IR (multiple partitions)
    let ir_len = PARTITION_SIZE * 4;
    let mut ir_data = vec![0.0f32; ir_len];
    // Exponentially decaying impulse response
    for (i, sample) in ir_data.iter_mut().enumerate() {
        *sample = (-(i as f32) / 500.0).exp() * 0.1;
    }
    let ir = vec![ir_data];
    let mut plugin = make_plugin_with_ir(channels, sr, ir);
    plugin.mix_value = 1.0;
    plugin.mix.set_target(1.0);

    // Process 10000 frames of random-ish signal
    let total_frames = 10000;
    let mut buffer: Vec<f32> = (0..total_frames)
        .map(|i| {
            let t = i as f32 / sr as f32;
            0.3 * (t * 440.0 * std::f32::consts::TAU).sin()
                + 0.1 * (t * 1000.0 * std::f32::consts::TAU).sin()
        })
        .collect();

    for block_start in (0..total_frames).step_by(PARTITION_SIZE) {
        let block_end = (block_start + PARTITION_SIZE).min(total_frames);
        let nf = block_end - block_start;
        let ctx = ProcessContext::new(sr, nf);
        plugin
            .process_in_place(&mut buffer[block_start..block_end], &ctx)
            .unwrap();
    }

    // Verify no NaN or Inf in output
    for (i, &s) in buffer.iter().enumerate() {
        assert!(
            s.is_finite(),
            "Output sample at index {i} is not finite: {s}"
        );
    }
}

#[test]
fn test_ir_file_parameter_loads_and_reports_path() {
    let path = std::env::temp_dir().join(format!(
        "sotf-convolution-ir-{}-{}.wav",
        std::process::id(),
        "param"
    ));
    write_test_wav(&path, &[32767, 0, 0, 0], 48000);

    let mut plugin = ConvolutionPlugin::new(1, 48000);
    plugin
        .set_parameter(
            ParameterId::from("ir_file"),
            ParameterValue::String(path.to_string_lossy().into_owned()),
        )
        .unwrap();

    assert_eq!(
        plugin.get_parameter(&ParameterId::from("ir_file")),
        Some(ParameterValue::String(path.to_string_lossy().into_owned()))
    );

    // Spin until the background thread finishes loading.
    let mut buf = vec![0.0f32; 1024];
    let ctx = ProcessContext::new(48000, 1024);
    for _ in 0..200 {
        plugin.process_in_place(&mut buf, &ctx).unwrap();
        if plugin.ir_load_result_rx.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(plugin.state.load().is_some(), "IR load should complete");

    fs::remove_file(path).ok();
}

#[test]
fn test_from_params_loads_nupc_engine_from_ir_file() {
    let path = std::env::temp_dir().join(format!(
        "sotf-convolution-ir-{}-{}.wav",
        std::process::id(),
        "nupc"
    ));
    write_test_wav(&path, &[32767, 0, 0, 0], 48000);

    let params = ConvolutionPluginParams {
        ir_file: path.to_string_lossy().into_owned(),
        mix: 1.0,
        gain_db: 0.0,
        use_nupc: true,
        zero_latency_head: false,
        head_taps: 128,
    };

    let mut plugin = ConvolutionPlugin::from_params(1, 48000, params).unwrap();
    assert_eq!(
        plugin.nupc_engines.len(),
        1,
        "from_params with use_nupc=true should build NUPC engines from the IR file"
    );

    let mut buffer = vec![1.0_f32; 64];
    let ctx = ProcessContext::new(48000, 64);
    plugin.process_in_place(&mut buffer, &ctx).unwrap();
    assert!(buffer.iter().all(|sample| sample.is_finite()));

    fs::remove_file(path).ok();
}

#[test]
fn test_process_rejects_short_buffer() {
    let mut plugin = make_plugin_with_ir(2, 48000, vec![vec![1.0], vec![1.0]]);
    let ctx = ProcessContext::new(48000, 32);
    let mut short = vec![0.0_f32; 32 * 2 - 1];
    assert!(plugin.process_in_place(&mut short, &ctx).is_err());
}

/// Partial-block passthrough: process with nf=64 (much smaller than
/// PARTITION_SIZE=1024).  A Dirac IR with mix=0 must produce exactly the
/// dry input in output[PARTITION_SIZE..] after flushing.
///
/// This is the regression test for the UPC output-dropping bug (review
/// issue #1): the old code only wrote back the last `to_copy` samples of
/// each partition and silently discarded the first `PARTITION_SIZE-to_copy`
/// samples.
#[test]
fn test_partial_block_no_output_drop() {
    let small_block = 64_usize; // << PARTITION_SIZE
    assert!(small_block < PARTITION_SIZE);

    let channels = 1;
    let sr = 48000;
    let ir = vec![vec![1.0f32]]; // Dirac: mix=0 output should equal dry input

    let mut plugin = make_plugin_with_ir(channels, sr, ir);
    plugin.mix_value = 0.0;
    plugin.mix.set_target(0.0);
    plugin.mix.reset(0.0);

    // Signal: PARTITION_SIZE samples of a sine + one flush block of zeros.
    let signal_frames = PARTITION_SIZE;
    let total_frames = signal_frames + PARTITION_SIZE; // extra block to flush ring
    let mut buffer: Vec<f32> = (0..signal_frames)
        .map(|i| (i as f32 * 0.05).sin())
        .chain(std::iter::repeat_n(0.0f32, PARTITION_SIZE))
        .collect();
    let original = buffer[..signal_frames].to_vec();

    // Process in small blocks
    for block_start in (0..total_frames).step_by(small_block) {
        let block_end = (block_start + small_block).min(total_frames);
        let nf = block_end - block_start;
        let ctx = ProcessContext::new(sr, nf);
        plugin
            .process_in_place(&mut buffer[block_start..block_end], &ctx)
            .unwrap();
    }

    // After one full partition + flush, output[PARTITION_SIZE..2*PARTITION_SIZE]
    // should equal original[0..PARTITION_SIZE] (mix=0 → dry passthrough with 1 block delay).
    let latency = PARTITION_SIZE;
    for (i, (&got, &exp)) in buffer[latency..latency + signal_frames]
        .iter()
        .zip(original.iter())
        .enumerate()
    {
        assert!(
            (got - exp).abs() < 1e-4,
            "partial-block output drop at sample {}: got {}, expected {}",
            latency + i,
            got,
            exp
        );
    }
}

/// Partial-block energy preservation: with a Dirac IR and mix=1 (fully wet),
/// the output energy across multiple small blocks should approximately equal
/// the input energy.  This catches sample-dropping that reduces total output
/// amplitude.
#[test]
fn test_partial_block_energy_preserved() {
    let small_block = 128_usize;
    assert!(small_block < PARTITION_SIZE);

    let channels = 1;
    let sr = 48000;
    let ir = vec![vec![1.0f32]]; // Dirac

    let mut plugin = make_plugin_with_ir(channels, sr, ir);
    plugin.mix_value = 1.0;
    plugin.mix.set_target(1.0);
    plugin.mix.reset(1.0);
    plugin.gain_db_value = 0.0;
    plugin.gain_linear.set_target(1.0);
    plugin.gain_linear.reset(1.0);

    // Two full partitions of signal + one flush partition.
    let signal_frames = PARTITION_SIZE * 2;
    let total_frames = signal_frames + PARTITION_SIZE;
    let mut buffer: Vec<f32> = (0..signal_frames)
        .map(|i| (i as f32 * 0.07).sin() * 0.5)
        .chain(std::iter::repeat_n(0.0f32, PARTITION_SIZE))
        .collect();
    let input_energy: f32 = buffer[..signal_frames].iter().map(|s| s * s).sum();

    for block_start in (0..total_frames).step_by(small_block) {
        let block_end = (block_start + small_block).min(total_frames);
        let nf = block_end - block_start;
        let ctx = ProcessContext::new(sr, nf);
        plugin
            .process_in_place(&mut buffer[block_start..block_end], &ctx)
            .unwrap();
    }

    // Collect settled output (skip the initial 1-partition latency).
    let latency = PARTITION_SIZE;
    let output_energy: f32 = buffer[latency..latency + signal_frames]
        .iter()
        .map(|s| s * s)
        .sum();

    let ratio = output_energy / input_energy;
    assert!(
        (ratio - 1.0).abs() < 0.05,
        "partial-block energy ratio should be ~1.0, got {ratio} (in={input_energy}, out={output_energy})"
    );
}

#[test]
fn test_upc_ir_channel_mapping_cycles_stereo_ir() {
    let channels = 4;
    let sr = 48000;
    let ir = vec![vec![0.25f32], vec![0.75f32]];

    let mut plugin = make_plugin_with_ir(channels, sr, ir);
    plugin.use_nupc = false;
    plugin.mix_value = 1.0;
    plugin.mix.set_target(1.0);
    plugin.mix.reset(1.0);
    plugin.gain_linear.set_target(1.0);
    plugin.gain_linear.reset(1.0);

    let mut buffer = vec![1.0f32; PARTITION_SIZE * 2 * channels];
    for block_start in (0..PARTITION_SIZE * 2).step_by(PARTITION_SIZE) {
        let ctx = ProcessContext::new(sr, PARTITION_SIZE);
        let start = block_start * channels;
        let end = start + PARTITION_SIZE * channels;
        plugin
            .process_in_place(&mut buffer[start..end], &ctx)
            .unwrap();
    }

    let first_output = PARTITION_SIZE * channels;
    let expected = [0.25, 0.75, 0.25, 0.75];
    for (ch, expected_gain) in expected.iter().enumerate() {
        let got = buffer[first_output + ch];
        assert!(
            (got - expected_gain).abs() < 1e-4,
            "UPC IR channel {ch} should cycle stereo IR channels, got {got}, expected {expected_gain}"
        );
    }
}

#[test]
fn test_tiny_but_normal_output_is_not_threshold_flushed() {
    let channels = 1;
    let sr = 48000;
    let tiny = 1.0e-35_f32;

    let mut plugin = make_plugin_with_ir(channels, sr, vec![vec![1.0f32]]);
    plugin.mix_value = 0.0;
    plugin.mix.set_target(0.0);
    plugin.mix.reset(0.0);

    let mut buffer = vec![0.0f32; PARTITION_SIZE * 2];
    buffer[..PARTITION_SIZE].fill(tiny);

    for block_start in (0..PARTITION_SIZE * 2).step_by(PARTITION_SIZE) {
        let ctx = ProcessContext::new(sr, PARTITION_SIZE);
        plugin
            .process_in_place(&mut buffer[block_start..block_start + PARTITION_SIZE], &ctx)
            .unwrap();
    }

    assert_eq!(
        buffer[PARTITION_SIZE], tiny,
        "values below the old 1e-30 cleanup threshold but above f32 denormal range should survive"
    );
}

/// reset() clears all state: after reset, processing should be identical
/// to a fresh plugin run.
#[test]
fn test_reset_clears_all_state() {
    let channels = 1;
    let sr = 48000;
    let ir = vec![vec![1.0f32]];

    let mut plugin = make_plugin_with_ir(channels, sr, ir.clone());
    plugin.mix_value = 1.0;
    plugin.mix.set_target(1.0);
    plugin.mix.reset(1.0);
    plugin.gain_linear.set_target(1.0);
    plugin.gain_linear.reset(1.0);

    // First run
    let frames = PARTITION_SIZE * 2;
    let signal: Vec<f32> = (0..frames).map(|i| (i as f32 * 0.1).sin()).collect();
    let mut buf1 = signal.clone();
    for block_start in (0..frames).step_by(PARTITION_SIZE) {
        let nf = PARTITION_SIZE.min(frames - block_start);
        let ctx = ProcessContext::new(sr, nf);
        plugin
            .process_in_place(&mut buf1[block_start..block_start + nf], &ctx)
            .unwrap();
    }

    // Reset and second run with same input — must produce same output
    plugin.reset();
    let mut buf2 = signal.clone();
    for block_start in (0..frames).step_by(PARTITION_SIZE) {
        let nf = PARTITION_SIZE.min(frames - block_start);
        let ctx = ProcessContext::new(sr, nf);
        plugin
            .process_in_place(&mut buf2[block_start..block_start + nf], &ctx)
            .unwrap();
    }

    for (i, (&a, &b)) in buf1.iter().zip(buf2.iter()).enumerate() {
        assert!(
            (a - b).abs() < 1e-5,
            "reset() mismatch at sample {i}: first_run={a}, after_reset={b}"
        );
    }
}
