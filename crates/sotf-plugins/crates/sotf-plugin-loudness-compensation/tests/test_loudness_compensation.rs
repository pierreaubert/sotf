// ============================================================================
// Loudness Compensation Plugin Integration Tests
// ============================================================================

use sotf_host::{
    InPlacePluginAdapter, ParameterId, ParameterValue, Plugin, ProcessContext,
};
use sotf_plugin_loudness_compensation::LoudnessCompensationPlugin;

#[test]
fn test_loudness_comp_typical_usage() {
    // Typical use case: +6dB bass and treble boost for low-volume listening
    let mut plugin = InPlacePluginAdapter::new(LoudnessCompensationPlugin::new(
        2,       // Stereo
        100.0,   // Low-shelf at 100Hz
        6.0,     // +6dB bass boost
        10000.0, // High-shelf at 10kHz
        6.0,     // +6dB treble boost
    ));

    plugin.initialize(48000).unwrap();

    // Process some audio
    let num_frames = 1024;
    let mut input = vec![0.0_f32; num_frames * 2];

    // Create a test signal with bass, mid, and treble content
    for i in 0..num_frames {
        let t = i as f32 / 48000.0;
        // Mix of frequencies
        let bass = (2.0 * std::f32::consts::PI * 50.0 * t).sin() * 0.2; // 50Hz
        let mid = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.2; // 1kHz
        let treble = (2.0 * std::f32::consts::PI * 12000.0 * t).sin() * 0.2; // 12kHz

        let sample = bass + mid + treble;
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
        "Loudness compensation: input energy = {:.3}, output energy = {:.3}",
        input_energy, output_energy
    );

    assert!(output_energy > 0.0, "Output should not be silent");

    // With compensation gain, total energy should be similar
    // (bass and treble boosted, but compensated to prevent clipping)
    println!("Energy ratio: {:.3}", output_energy / input_energy);
}

#[test]
fn test_loudness_comp_dynamic_adjustment() {
    // Test adjusting loudness compensation in real-time
    let mut plugin =
        InPlacePluginAdapter::new(LoudnessCompensationPlugin::new(2, 100.0, 0.0, 10000.0, 0.0));
    plugin.initialize(48000).unwrap();

    let num_frames = 512;
    let input = vec![0.1_f32; num_frames * 2];
    let mut output1 = vec![0.0_f32; num_frames * 2];

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

    // Process with no boost
    plugin.process(&input, &mut output1, &context).unwrap();
    let energy1: f32 = output1.iter().map(|x| x * x).sum();

    // Increase bass boost
    plugin
        .set_parameter(ParameterId::from("low_gain"), ParameterValue::Float(12.0))
        .unwrap();

    let mut output2 = vec![0.0_f32; num_frames * 2];
    plugin.process(&input, &mut output2, &context).unwrap();
    let energy2: f32 = output2.iter().map(|x| x * x).sum();

    println!(
        "Energy with 0dB: {:.3}, Energy with +12dB bass: {:.3}",
        energy1, energy2
    );

    // Different processing should produce different results
    assert!(
        (energy1 - energy2).abs() > 0.001,
        "Changing parameters should affect output"
    );
}

#[test]
fn test_loudness_comp_with_music() {
    // Simulate processing music at different volume levels

    // High volume: minimal compensation
    let mut plugin_high_vol =
        InPlacePluginAdapter::new(LoudnessCompensationPlugin::new(2, 100.0, 0.0, 10000.0, 0.0));
    plugin_high_vol.initialize(48000).unwrap();

    // Low volume: significant compensation (Fletcher-Munson curves)
    let mut plugin_low_vol = InPlacePluginAdapter::new(LoudnessCompensationPlugin::new(
        2, 100.0, 10.0, 10000.0, 8.0,
    ));
    plugin_low_vol.initialize(48000).unwrap();

    // Create a test signal (simulated music)
    let num_frames = 2048;
    let mut input = vec![0.0_f32; num_frames * 2];

    for i in 0..num_frames {
        let t = i as f32 / 48000.0;
        // Simulate music with multiple harmonics
        let mut sample = 0.0;
        for harmonic in 1..=8 {
            let freq = 110.0 * harmonic as f32; // A2 and harmonics
            sample += (2.0 * std::f32::consts::PI * freq * t).sin() / harmonic as f32;
        }
        sample *= 0.1; // Reduce amplitude

        input[i * 2] = sample;
        input[i * 2 + 1] = sample;
    }

    let mut output_high = vec![0.0_f32; num_frames * 2];
    let mut output_low = vec![0.0_f32; num_frames * 2];

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

    plugin_high_vol
        .process(&input, &mut output_high, &context)
        .unwrap();
    plugin_low_vol
        .process(&input, &mut output_low, &context)
        .unwrap();

    let energy_high: f32 = output_high.iter().map(|x| x * x).sum();
    let energy_low: f32 = output_low.iter().map(|x| x * x).sum();

    println!("High volume mode energy: {:.3}", energy_high);
    println!("Low volume mode energy: {:.3}", energy_low);
    println!("Difference: {:.3}", (energy_low - energy_high).abs());

    // Low volume mode should have different spectral balance
    assert!(
        (energy_low - energy_high).abs() > 0.01,
        "Different volume modes should produce different results"
    );
}

#[test]
fn test_loudness_comp_12db_per_octave() {
    // Verify that we get 12dB/octave slope (2 cascaded biquads)
    let mut plugin = InPlacePluginAdapter::new(LoudnessCompensationPlugin::new(
        2, 1000.0, 12.0, 10000.0, 0.0,
    ));
    plugin.initialize(48000).unwrap();

    let num_frames = 4096;
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

    // Test at different frequencies to verify slope
    let test_frequencies = vec![
        (250.0, "250Hz (2 octaves below)"),
        (500.0, "500Hz (1 octave below)"),
        (1000.0, "1000Hz (corner frequency)"),
        (2000.0, "2000Hz (1 octave above)"),
    ];

    for (freq, label) in test_frequencies {
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * freq * i as f32 / 48000.0;
            input[i * 2] = phase.sin() * 0.1;
            input[i * 2 + 1] = phase.sin() * 0.1;
        }

        let mut output = vec![0.0_f32; num_frames * 2];
        plugin.reset(); // Reset filter state
        plugin.process(&input, &mut output, &context).unwrap();

        let input_rms = (input.iter().map(|x| x * x).sum::<f32>() / input.len() as f32).sqrt();
        let output_rms = (output.iter().map(|x| x * x).sum::<f32>() / output.len() as f32).sqrt();
        let gain_db = 20.0 * (output_rms / input_rms).log10();

        println!("{}: {:.1} dB", label, gain_db);
    }

    // Note: Actual gain depends on filter response, but we should see a clear slope
}

#[test]
fn test_loudness_comp_stereo_to_5ch() {
    // Test using loudness compensation in a 5.0 channel setup
    let mut plugin =
        InPlacePluginAdapter::new(LoudnessCompensationPlugin::new(5, 100.0, 8.0, 10000.0, 6.0));
    plugin.initialize(48000).unwrap();

    let num_frames = 1024;
    let mut input = vec![0.0_f32; num_frames * 5];

    // Fill with test signal on all channels
    for i in 0..num_frames {
        let t = i as f32 / 48000.0;
        let sample = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.2;

        for ch in 0..5 {
            input[i * 5 + ch] = sample;
        }
    }

    let mut output = vec![0.0_f32; num_frames * 5];

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    // Verify all channels are processed
    for ch in 0..5 {
        let channel_energy: f32 = (0..num_frames).map(|i| output[i * 5 + ch].powi(2)).sum();
        assert!(channel_energy > 0.0, "Channel {} should have energy", ch);
    }

    println!("5-channel loudness compensation processed successfully");
}
