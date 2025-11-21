// Integration tests for binaural decoder with SOFA files
//
// These tests verify end-to-end functionality with actual HRTF data

use sotf_audio::plugins::BinauralDecoderPlugin;
use sotf_audio::{Plugin, ProcessContext};
use std::f32::consts::PI;
use std::fs;
use std::path::PathBuf;

mod fixtures;
use fixtures::create_test_sofa_file;

/// Test basic processing with a minimal SOFA file
#[test]
fn test_binaural_with_minimal_sofa() {
    let sofa_path = create_test_sofa_file("test_minimal.sofa", 5, 256, 44100.0);

    let mut decoder = BinauralDecoderPlugin::new(
        5,    // 5.0 surround
        2048, // FFT size
        Some(sofa_path.clone()),
        true, // Optimization on
        0.0,  // No externalization
        0.0,  // No near-field
    );

    decoder.initialize(44100).unwrap();

    // Process a sine wave
    let block_size = 512;
    let mut input = vec![0.0f32; block_size * 5];
    let mut output = vec![0.0f32; block_size * 2];

    // Generate 440Hz sine wave on front left channel
    for i in 0..block_size {
        let t = i as f32 / 44100.0;
        input[i * 5] = (2.0 * PI * 440.0 * t).sin() * 0.3;
    }

    let context = ProcessContext {
        num_frames: block_size,
        sample_rate: 44100,
    };

    decoder.process(&input, &mut output, &context).unwrap();

    // Verify output is non-zero (actual spatialization happened)
    let rms_left: f32 = output.iter().step_by(2).map(|x| x * x).sum::<f32>() / (block_size as f32);
    let rms_right: f32 = output.iter().skip(1).step_by(2).map(|x| x * x).sum::<f32>() / (block_size as f32);

    let rms_left = rms_left.sqrt();
    let rms_right = rms_right.sqrt();

    println!("RMS Left: {:.6}, RMS Right: {:.6}", rms_left, rms_right);

    // Output should be non-trivial
    assert!(rms_left > 0.001, "Left channel has no output");
    assert!(rms_right > 0.001, "Right channel has no output");

    // Clean up
    fs::remove_file(sofa_path).ok();
}

/// Test sample rate resampling
#[test]
fn test_binaural_sample_rate_resampling() {
    // Create SOFA at 48kHz
    let sofa_path = create_test_sofa_file("test_resample.sofa", 5, 256, 48000.0);

    // Initialize decoder at 44.1kHz (different rate)
    let mut decoder = BinauralDecoderPlugin::new(
        5,
        2048,
        Some(sofa_path.clone()),
        true,
        0.0,
        0.0,
    );

    // This should trigger automatic resampling
    decoder.initialize(44100).unwrap();

    let block_size = 256;
    let input = vec![0.1f32; block_size * 5];
    let mut output = vec![0.0f32; block_size * 2];

    let context = ProcessContext {
        num_frames: block_size,
        sample_rate: 44100,
    };

    // Should process without errors despite sample rate mismatch
    decoder.process(&input, &mut output, &context).unwrap();

    // Verify output
    let rms: f32 = (output.iter().map(|x| x * x).sum::<f32>() / output.len() as f32).sqrt();
    assert!(rms > 0.001, "Resampled processing produced no output");

    fs::remove_file(sofa_path).ok();
}

/// Test LFE channel handling
#[test]
fn test_binaural_lfe_handling() {
    let sofa_path = create_test_sofa_file("test_lfe.sofa", 6, 256, 48000.0);

    // 5.1 configuration has LFE at channel 3
    let mut decoder = BinauralDecoderPlugin::new(
        6, // 5.1 surround
        2048,
        Some(sofa_path.clone()),
        true,
        0.0,
        0.0,
    );

    decoder.initialize(48000).unwrap();

    let block_size = 512;
    let mut input = vec![0.0f32; block_size * 6];
    let mut output = vec![0.0f32; block_size * 2];

    // Put signal only on LFE channel (channel 3)
    for i in 0..block_size {
        input[i * 6 + 3] = 0.5;
    }

    let context = ProcessContext {
        num_frames: block_size,
        sample_rate: 48000,
    };

    decoder.process(&input, &mut output, &context).unwrap();

    // LFE should be mixed equally to both ears
    let rms_left: f32 = output.iter().step_by(2).map(|x| x * x).sum::<f32>() / (block_size as f32);
    let rms_right: f32 = output.iter().skip(1).step_by(2).map(|x| x * x).sum::<f32>() / (block_size as f32);

    let rms_left = rms_left.sqrt();
    let rms_right = rms_right.sqrt();

    // Both channels should have similar energy (within 10%)
    let ratio = rms_left / rms_right;
    assert!(
        (0.9..=1.1).contains(&ratio),
        "LFE not mixed equally: L/R ratio = {}",
        ratio
    );

    fs::remove_file(sofa_path).ok();
}

/// Test externalization effect
#[test]
fn test_binaural_externalization() {
    let sofa_path = create_test_sofa_file("test_ext.sofa", 5, 256, 48000.0);

    // Test with externalization off
    let mut decoder_no_ext = BinauralDecoderPlugin::new(
        5,
        2048,
        Some(sofa_path.clone()),
        true,
        0.0, // No externalization
        0.0,
    );
    decoder_no_ext.initialize(48000).unwrap();

    // Test with externalization on
    let mut decoder_with_ext = BinauralDecoderPlugin::new(
        5,
        2048,
        Some(sofa_path.clone()),
        true,
        0.8, // High externalization
        0.0,
    );
    decoder_with_ext.initialize(48000).unwrap();

    let block_size = 1024;
    let mut input = vec![0.0f32; block_size * 5];

    // Create impulse on front left
    input[0] = 1.0;

    let mut output_no_ext = vec![0.0f32; block_size * 2];
    let mut output_with_ext = vec![0.0f32; block_size * 2];

    let context = ProcessContext {
        num_frames: block_size,
        sample_rate: 48000,
    };

    decoder_no_ext.process(&input, &mut output_no_ext, &context).unwrap();
    decoder_with_ext.process(&input, &mut output_with_ext, &context).unwrap();

    // With externalization, the impulse response should be longer (early reflections)
    // Find the effective length (90% energy threshold)
    let energy_no_ext: f32 = output_no_ext.iter().map(|x| x * x).sum();
    let energy_with_ext: f32 = output_with_ext.iter().map(|x| x * x).sum();

    // Externalization should add energy (reflections)
    assert!(
        energy_with_ext > energy_no_ext * 1.05,
        "Externalization should add energy"
    );

    fs::remove_file(sofa_path).ok();
}

/// Test optimization enabled vs disabled (should produce same results)
#[test]
fn test_binaural_optimization_equivalence() {
    let sofa_path = create_test_sofa_file("test_opt.sofa", 5, 256, 48000.0);

    let mut decoder_standard = BinauralDecoderPlugin::new(
        5,
        2048,
        Some(sofa_path.clone()),
        false, // Standard mode
        0.0,
        0.0,
    );
    decoder_standard.initialize(48000).unwrap();

    let mut decoder_optimized = BinauralDecoderPlugin::new(
        5,
        2048,
        Some(sofa_path.clone()),
        true, // Optimized mode
        0.0,
        0.0,
    );
    decoder_optimized.initialize(48000).unwrap();

    let block_size = 512;
    let input = vec![0.3f32; block_size * 5];
    let mut output_standard = vec![0.0f32; block_size * 2];
    let mut output_optimized = vec![0.0f32; block_size * 2];

    let context = ProcessContext {
        num_frames: block_size,
        sample_rate: 48000,
    };

    decoder_standard.process(&input, &mut output_standard, &context).unwrap();
    decoder_optimized.process(&input, &mut output_optimized, &context).unwrap();

    // Outputs should be very similar (within numerical precision)
    let max_diff = output_standard
        .iter()
        .zip(output_optimized.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);

    println!("Max difference between standard and optimized: {:.6e}", max_diff);

    assert!(
        max_diff < 1e-5,
        "Optimization changed results too much: max_diff = {}",
        max_diff
    );

    fs::remove_file(sofa_path).ok();
}

/// Test Dolby Atmos 7.1.4 configuration
#[test]
fn test_binaural_atmos_7_1_4() {
    let sofa_path = create_test_sofa_file("test_atmos.sofa", 12, 256, 48000.0);

    let mut decoder = BinauralDecoderPlugin::new(
        12, // 7.1.4 Atmos
        2048,
        Some(sofa_path.clone()),
        true,
        0.3, // Some externalization
        0.2, // Some near-field
    );

    decoder.initialize(48000).unwrap();

    let block_size = 512;
    let mut input = vec![0.0f32; block_size * 12];

    // Put signal on height channels (last 4 channels)
    for i in 0..block_size {
        input[i * 12 + 8] = 0.2;  // Top Front Left
        input[i * 12 + 9] = 0.2;  // Top Front Right
        input[i * 12 + 10] = 0.2; // Top Back Left
        input[i * 12 + 11] = 0.2; // Top Back Right
    }

    let mut output = vec![0.0f32; block_size * 2];

    let context = ProcessContext {
        num_frames: block_size,
        sample_rate: 48000,
    };

    decoder.process(&input, &mut output, &context).unwrap();

    let rms: f32 = (output.iter().map(|x| x * x).sum::<f32>() / output.len() as f32).sqrt();
    assert!(rms > 0.01, "Atmos processing produced insufficient output");

    fs::remove_file(sofa_path).ok();
}

/// Test continuous processing (multiple blocks)
#[test]
fn test_binaural_continuous_processing() {
    let sofa_path = create_test_sofa_file("test_continuous.sofa", 5, 256, 48000.0);

    let mut decoder = BinauralDecoderPlugin::new(
        5,
        2048,
        Some(sofa_path.clone()),
        true,
        0.0,
        0.0,
    );

    decoder.initialize(48000).unwrap();

    let block_size = 256;
    let num_blocks = 100;

    for block in 0..num_blocks {
        let input = vec![0.1f32; block_size * 5];
        let mut output = vec![0.0f32; block_size * 2];

        let context = ProcessContext {
            num_frames: block_size,
            sample_rate: 48000,
        };

        decoder.process(&input, &mut output, &context).unwrap();

        // Verify each block produces valid output
        let has_output = output.iter().any(|&x| x.abs() > 1e-6);
        assert!(has_output, "Block {} produced no output", block);
    }

    fs::remove_file(sofa_path).ok();
}
