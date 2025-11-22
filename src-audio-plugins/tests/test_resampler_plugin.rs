// ============================================================================
// Resampler Plugin Integration Tests
// ============================================================================
//
// Demonstrates how to use the resampler plugin for sample rate conversion.

use sotf_plugins::{Plugin, ProcessContext, ResamplerPlugin};

#[test]
fn test_resampler_basic_usage() {
    // Create a resampler: 44.1kHz -> 48kHz, stereo
    let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
    resampler.initialize(44100).unwrap();

    // Generate input audio: 440Hz tone at 44.1kHz
    let num_frames = 1024;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let t = i as f32 / 44100.0;
        let sample = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
        input[i * 2] = sample; // Left
        input[i * 2 + 1] = sample; // Right
    }

    // Allocate output buffer (max possible size)
    let max_output_frames = resampler.output_frames_for_input(num_frames);
    let mut output = vec![0.0_f32; max_output_frames * 2];

    // Process
    let context = ProcessContext {
        sample_rate: 44100,
        num_frames,
    };
    resampler.process(&input, &mut output, &context).unwrap();

    println!(
        "Resampled {} frames to max {} frames",
        num_frames, max_output_frames
    );
    println!("Ratio: {:.4}", resampler.ratio());

    // Verify output has signal
    let expected_frames = (num_frames as f64 * resampler.ratio()) as usize;
    let rms: f32 = output[..expected_frames * 2]
        .iter()
        .map(|x| x * x)
        .sum::<f32>()
        / (expected_frames * 2) as f32;
    assert!(rms.sqrt() > 0.1, "Output should contain signal");
}

#[test]
fn test_resampler_downsampling() {
    // Downsample: 48kHz -> 44.1kHz
    let mut resampler = ResamplerPlugin::new(2, 48000, 44100, 1024).unwrap();
    resampler.initialize(48000).unwrap();

    let num_frames = 1024;
    let mut input = vec![0.0_f32; num_frames * 2];

    // Generate 1kHz tone at 48kHz
    for i in 0..num_frames {
        let t = i as f32 / 48000.0;
        let sample = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.3;
        input[i * 2] = sample;
        input[i * 2 + 1] = sample;
    }

    let max_output_frames = resampler.output_frames_for_input(num_frames);
    let mut output = vec![0.0_f32; max_output_frames * 2];

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };
    resampler.process(&input, &mut output, &context).unwrap();

    println!(
        "Downsampling: {} -> max {} frames",
        num_frames, max_output_frames
    );

    // Check output
    let expected_frames = (num_frames as f64 * resampler.ratio()) as usize;
    let rms: f32 = output[..expected_frames * 2]
        .iter()
        .map(|x| x * x)
        .sum::<f32>()
        / (expected_frames * 2) as f32;
    assert!(rms.sqrt() > 0.1);
}

#[test]
fn test_resampler_surround_sound() {
    // Resample 5.1 surround audio
    let mut resampler = ResamplerPlugin::new(6, 44100, 48000, 1024).unwrap();
    resampler.initialize(44100).unwrap();

    let num_frames = 1024;
    let mut input = vec![0.0_f32; num_frames * 6];

    // Generate different content per channel
    for i in 0..num_frames {
        let t = i as f32 / 44100.0;
        input[i * 6 + 0] = (2.0 * std::f32::consts::PI * 220.0 * t).sin() * 0.2; // FL
        input[i * 6 + 1] = (2.0 * std::f32::consts::PI * 247.0 * t).sin() * 0.2; // FR
        input[i * 6 + 2] = (2.0 * std::f32::consts::PI * 277.0 * t).sin() * 0.2; // C
        input[i * 6 + 3] = (2.0 * std::f32::consts::PI * 110.0 * t).sin() * 0.15; // LFE
        input[i * 6 + 4] = (2.0 * std::f32::consts::PI * 185.0 * t).sin() * 0.15; // RL
        input[i * 6 + 5] = (2.0 * std::f32::consts::PI * 196.0 * t).sin() * 0.15; // RR
    }

    let max_output_frames = resampler.output_frames_for_input(num_frames);
    let mut output = vec![0.0_f32; max_output_frames * 6];

    let context = ProcessContext {
        sample_rate: 44100,
        num_frames,
    };
    resampler.process(&input, &mut output, &context).unwrap();

    println!(
        "5.1 Surround: {} -> max {} frames",
        num_frames, max_output_frames
    );

    // Verify each channel
    let expected_frames = (num_frames as f64 * resampler.ratio()) as usize;
    for ch in 0..6 {
        let channel_samples: Vec<f32> = (0..expected_frames).map(|i| output[i * 6 + ch]).collect();
        let rms: f32 =
            channel_samples.iter().map(|x| x * x).sum::<f32>() / channel_samples.len() as f32;
        println!("Channel {} RMS: {:.4}", ch, rms.sqrt());
        assert!(rms.sqrt() > 0.05, "Channel {} should have signal", ch);
    }
}

#[test]
fn test_resampler_multiple_blocks() {
    // Process multiple consecutive blocks
    let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
    resampler.initialize(44100).unwrap();

    let num_frames = 1024;
    let max_output_frames = resampler.output_frames_for_input(num_frames);

    // Process 3 blocks
    for block in 0..3 {
        let mut input = vec![0.0_f32; num_frames * 2];

        // Generate continuous tone across blocks
        for i in 0..num_frames {
            let frame_idx = block * num_frames + i;
            let t = frame_idx as f32 / 44100.0;
            let sample = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }

        let mut output = vec![0.0_f32; max_output_frames * 2];
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames,
        };
        resampler.process(&input, &mut output, &context).unwrap();

        println!("Block {}: processed successfully", block);
    }

    // All blocks should process without errors
    assert!(true);
}

#[test]
fn test_resampler_extreme_ratios() {
    // Test with more extreme sample rate conversions

    // 2x upsampling (22.05kHz -> 44.1kHz)
    let mut resampler1 = ResamplerPlugin::new(2, 22050, 44100, 512).unwrap();
    resampler1.initialize(22050).unwrap();
    println!("2x upsampling ratio: {:.4}", resampler1.ratio());
    assert!((resampler1.ratio() - 2.0).abs() < 0.001);

    // 2x downsampling (96kHz -> 48kHz)
    let mut resampler2 = ResamplerPlugin::new(2, 96000, 48000, 2048).unwrap();
    resampler2.initialize(96000).unwrap();
    println!("2x downsampling ratio: {:.4}", resampler2.ratio());
    assert!((resampler2.ratio() - 0.5).abs() < 0.001);

    // Arbitrary ratio (32kHz -> 48kHz)
    let mut resampler3 = ResamplerPlugin::new(2, 32000, 48000, 1024).unwrap();
    resampler3.initialize(32000).unwrap();
    println!("1.5x upsampling ratio: {:.4}", resampler3.ratio());
    assert!((resampler3.ratio() - 1.5).abs() < 0.001);
}

#[test]
fn test_resampler_reset_functionality() {
    let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
    resampler.initialize(44100).unwrap();

    let num_frames = 1024;
    let input = vec![0.5_f32; num_frames * 2];
    let max_output_frames = resampler.output_frames_for_input(num_frames);
    let mut output = vec![0.0_f32; max_output_frames * 2];

    let context = ProcessContext {
        sample_rate: 44100,
        num_frames,
    };

    // Process once
    resampler.process(&input, &mut output, &context).unwrap();

    // Reset
    resampler.reset();

    // Process again - should work
    resampler.process(&input, &mut output, &context).unwrap();

    // Verify output
    let expected_frames = (num_frames as f64 * resampler.ratio()) as usize;
    let rms: f32 = output[..expected_frames * 2]
        .iter()
        .map(|x| x * x)
        .sum::<f32>()
        / (expected_frames * 2) as f32;
    assert!(rms.sqrt() > 0.1);

    println!("Reset test passed");
}

#[test]
fn test_resampler_with_default_chunk_size() {
    // Test using the default constructor
    let mut resampler = ResamplerPlugin::new_default(2, 44100, 48000).unwrap();
    resampler.initialize(44100).unwrap();

    let num_frames = 1024; // Default chunk size
    let input = vec![0.3_f32; num_frames * 2];
    let max_output_frames = resampler.output_frames_for_input(num_frames);
    let mut output = vec![0.0_f32; max_output_frames * 2];

    let context = ProcessContext {
        sample_rate: 44100,
        num_frames,
    };

    resampler.process(&input, &mut output, &context).unwrap();

    println!("Default chunk size test passed");
}
