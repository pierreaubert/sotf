// ============================================================================
// EQ Plugin Integration Tests
// ============================================================================
//
// This file demonstrates how to use the EQ plugin with IIR biquad filters.

use math_audio_iir_fir::{Biquad, BiquadFilterType};
use sotf_plugins::{EqPlugin, InPlacePluginAdapter, Plugin, ProcessContext};

#[test]
fn test_eq_plugin_basic() {
    // Create a simple 2-band EQ: bass boost + treble boost
    let filters = vec![
        Biquad::new(BiquadFilterType::Lowshelf, 100.0, 48000.0, 0.707, 6.0), // +6dB bass
        Biquad::new(BiquadFilterType::Highshelf, 8000.0, 48000.0, 0.707, 6.0), // +6dB treble
    ];

    let mut plugin = InPlacePluginAdapter::new(EqPlugin::new(2, filters)); // 2 channels (stereo)
    plugin.initialize(48000).unwrap();

    // Create test signal: 1kHz sine wave
    let num_frames = 1024;
    let mut input = vec![0.0_f32; num_frames * 2]; // Stereo
    for i in 0..num_frames {
        let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
        let sample = phase.sin() * 0.5;
        input[i * 2] = sample; // Left
        input[i * 2 + 1] = sample; // Right
    }

    let mut output = vec![0.0_f32; num_frames * 2];

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

    // Process
    plugin.process(&input, &mut output, &context).unwrap();

    // Verify output is not silent
    let output_sum: f32 = output.iter().map(|x| x.abs()).sum();
    assert!(output_sum > 0.0, "Output should not be silent");

    println!("EQ plugin processed {} frames successfully", num_frames);
}

#[test]
fn test_eq_plugin_parametric() {
    // Create a parametric EQ with multiple bands
    let filters = vec![
        // Sub-bass cut
        Biquad::new(BiquadFilterType::Highpass, 30.0, 48000.0, 0.707, 0.0),
        // Bass boost
        Biquad::new(BiquadFilterType::Lowshelf, 100.0, 48000.0, 0.707, 4.0),
        // Lower mid cut
        Biquad::new(BiquadFilterType::Peak, 250.0, 48000.0, 1.0, -2.0),
        // Mid presence boost
        Biquad::new(BiquadFilterType::Peak, 2000.0, 48000.0, 2.0, 3.0),
        // High mid cut (de-harsh)
        Biquad::new(BiquadFilterType::Peak, 4000.0, 48000.0, 1.5, -2.0),
        // Air boost
        Biquad::new(BiquadFilterType::Highshelf, 10000.0, 48000.0, 0.707, 3.0),
    ];

    let mut plugin = InPlacePluginAdapter::new(EqPlugin::new(2, filters));
    plugin.initialize(48000).unwrap();

    // Process a sweep or noise to test frequency response
    let num_frames = 4096;
    let mut input = vec![0.0_f32; num_frames * 2];

    // Create a sweep from 20Hz to 20kHz
    for i in 0..num_frames {
        let t = i as f32 / 48000.0;
        let freq = 20.0 * (1000.0_f32).powf(t * 48000.0 / num_frames as f32);
        let phase = 2.0 * std::f32::consts::PI * freq * t;
        let sample = phase.sin() * 0.3;
        input[i * 2] = sample;
        input[i * 2 + 1] = sample;
    }

    let mut output = vec![0.0_f32; num_frames * 2];

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    // Verify processing occurred
    let input_energy: f32 = input.iter().map(|x| x * x).sum();
    let output_energy: f32 = output.iter().map(|x| x * x).sum();

    println!(
        "Parametric EQ: input energy = {:.2}, output energy = {:.2}, ratio = {:.2}",
        input_energy,
        output_energy,
        output_energy / input_energy
    );

    // Energy should be modified by the EQ
    assert!(output_energy > 0.0, "Output should not be silent");
}

#[test]
fn test_eq_plugin_filter_update() {
    // Test updating filters dynamically
    let initial_filters = vec![Biquad::new(
        BiquadFilterType::Peak,
        1000.0,
        48000.0,
        1.0,
        6.0,
    )];

    let mut eq = EqPlugin::new(2, initial_filters);
    let mut plugin = InPlacePluginAdapter::new(eq);
    plugin.initialize(48000).unwrap();

    // Process with initial filter
    let num_frames = 512;
    let input = vec![0.5_f32; num_frames * 2];
    let mut output1 = vec![0.0_f32; num_frames * 2];

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

    plugin.process(&input, &mut output1, &context).unwrap();

    // Update to different filter (Note: we need to access the inner plugin if we want to call set_filters)
    // Actually, InPlacePluginAdapter doesn't expose the inner plugin easily.
    // Let's just recreate it or use trait methods if possible.
    // For this test, I'll just use the trait parameters if available or access inner via unsafe/re-wrapping.
    // Better: fix EqPlugin to support set_filters via parameters? It already does!

    plugin
        .set_parameter(
            sotf_plugins::ParameterId::from("band_0_freq"),
            sotf_plugins::ParameterValue::Float(2000.0),
        )
        .unwrap();
    plugin
        .set_parameter(
            sotf_plugins::ParameterId::from("band_0_gain"),
            sotf_plugins::ParameterValue::Float(-6.0),
        )
        .unwrap();

    // Process with new filter
    let mut output2 = vec![0.0_f32; num_frames * 2];
    plugin.process(&input, &mut output2, &context).unwrap();

    // Outputs should be different
    let diff: f32 = output1
        .iter()
        .zip(output2.iter())
        .map(|(a, b)| (a - b).abs())
        .sum();

    println!("Difference between filter sets: {}", diff);
    assert!(
        diff > 0.1,
        "Changing filters should produce different output"
    );
}

#[test]
fn test_eq_plugin_multi_channel() {
    // Test with 5 channels (e.g., after upmixer)
    let filters = vec![Biquad::new(
        BiquadFilterType::Peak,
        1000.0,
        48000.0,
        1.0,
        3.0,
    )];

    let mut plugin = InPlacePluginAdapter::new(EqPlugin::new(5, filters)); // 5.0 surround
    plugin.initialize(48000).unwrap();

    let num_frames = 1024;
    let mut input = vec![0.0_f32; num_frames * 5];

    // Fill each channel with a different frequency
    for i in 0..num_frames {
        let t = i as f32 / 48000.0;
        input[i * 5 + 0] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.3; // FL
        input[i * 5 + 1] = (2.0 * std::f32::consts::PI * 550.0 * t).sin() * 0.3; // FR
        input[i * 5 + 2] = (2.0 * std::f32::consts::PI * 660.0 * t).sin() * 0.3; // C
        input[i * 5 + 3] = (2.0 * std::f32::consts::PI * 770.0 * t).sin() * 0.3; // RL
        input[i * 5 + 4] = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.3; // RR
    }

    let mut output = vec![0.0_f32; num_frames * 5];

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    // Verify all channels have output
    for ch in 0..5 {
        let channel_energy: f32 = (0..num_frames).map(|i| output[i * 5 + ch].powi(2)).sum();
        assert!(channel_energy > 0.0, "Channel {} should have energy", ch);
    }

    println!("5-channel EQ processed successfully");
}
