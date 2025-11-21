// Integration test for upmixer plugin

use sotf_audio::plugins::Host;
use sotf_audio::{PluginHost, UpmixerPlugin};

#[test]
fn test_upmixer_stereo_to_5ch() {
    // Create a 2→5 channel plugin host
    let mut host = PluginHost::new(2, 44100);

    // Add upmixer plugin (5.1 configuration outputs 6 channels)
    let upmixer = UpmixerPlugin::new(2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5);
    host.add_plugin(Box::new(upmixer)).unwrap();

    // Verify channel counts
    assert_eq!(host.input_channels(), 2);
    assert_eq!(host.output_channels(), 6);

    // Create stereo input with sine waves
    let mut input_stereo = vec![0.0; 2048 * 2];
    for i in 0..2048 {
        let t = i as f32 / 44100.0;
        input_stereo[i * 2] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5; // 440 Hz left
        input_stereo[i * 2 + 1] = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.3; // 880 Hz right
    }

    let mut output_6ch = vec![0.0; 2048 * 6];

    // Process
    host.process(&input_stereo, &mut output_6ch).unwrap();

    // Verify we got output
    let total_energy: f32 = output_6ch.iter().map(|x| x * x).sum();
    assert!(total_energy > 0.0, "Should have non-zero output");

    // Check individual channels
    let mut channel_energies = vec![0.0; 6];
    for i in 0..2048 {
        for ch in 0..6 {
            channel_energies[ch] += output_6ch[i * 6 + ch].powi(2);
        }
    }

    println!("Channel energies:");
    println!("  Front Left:  {:.4}", channel_energies[0]);
    println!("  Front Right: {:.4}", channel_energies[1]);
    println!("  Center:      {:.4}", channel_energies[2]);
    println!("  LFE:         {:.4}", channel_energies[3]);
    println!("  Rear Left:   {:.4}", channel_energies[4]);
    println!("  Rear Right:  {:.4}", channel_energies[5]);

    // Front channels should have most energy
    assert!(
        channel_energies[0] > 0.0 || channel_energies[1] > 0.0,
        "Front channels should have content"
    );
}

#[test]
fn test_upmixer_chain_with_gain() {
    use sotf_audio::{GainPlugin, InPlacePluginAdapter};

    // Create a processing chain: stereo → upmix to 5ch → gain on 5ch
    let mut host = PluginHost::new(2, 44100);

    // Add upmixer (2→6, 5.1 configuration)
    let upmixer = UpmixerPlugin::new(1024, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5); // Smaller FFT for this test
    host.add_plugin(Box::new(upmixer)).unwrap();

    // Add gain to the 6-channel output
    let gain = GainPlugin::new(6, -6.0); // -6dB on all 6 channels
    host.add_plugin(Box::new(InPlacePluginAdapter::new(gain)))
        .unwrap();

    // Verify final configuration
    assert_eq!(host.input_channels(), 2);
    assert_eq!(host.output_channels(), 6);

    // Process with varying input
    let mut input = vec![0.0; 1024 * 2];
    for i in 0..1024 {
        input[i * 2] = (i as f32 * 0.01).sin() * 0.5;
        input[i * 2 + 1] = (i as f32 * 0.015).cos() * 0.5;
    }
    let mut output = vec![0.0; 1024 * 6];

    host.process(&input, &mut output).unwrap();

    // Output should be non-zero and attenuated
    let sum: f32 = output.iter().map(|x| x.abs()).sum();
    println!("Chain output sum: {}", sum);
    assert!(sum > 0.0, "Should have output after upmixer + gain");
}

#[test]
fn test_upmixer_parameter_adjustment() {
    use sotf_audio::{ParameterId, ParameterValue, Plugin};

    let mut plugin = UpmixerPlugin::new(2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5);
    plugin.initialize(44100).unwrap();

    // Test parameter queries
    let params = plugin.parameters();
    assert_eq!(params.len(), 11); // speaker_config, gain_front_direct, gain_front_ambient, gain_rear_ambient, height_gain, lfe_gain, lfe_cutoff_hz, stereo_width, bandpass_hz, enable_subharmonic_synth, subharmonic_gain

    // Modify gains
    plugin
        .set_parameter(
            ParameterId::from("gain_front_direct"),
            ParameterValue::Float(0.8),
        )
        .unwrap();

    plugin
        .set_parameter(
            ParameterId::from("gain_rear_ambient"),
            ParameterValue::Float(1.5),
        )
        .unwrap();

    // Verify changes
    let front_direct = plugin.get_parameter(&ParameterId::from("gain_front_direct"));
    assert_eq!(front_direct, Some(ParameterValue::Float(0.8)));

    let rear_ambient = plugin.get_parameter(&ParameterId::from("gain_rear_ambient"));
    assert_eq!(rear_ambient, Some(ParameterValue::Float(1.5)));
}

#[test]
fn test_upmixer_synthesis_windowing_no_crackling() {
    use sotf_audio::Plugin;

    // This test verifies that synthesis windowing is properly applied to prevent
    // crackling/clicking artifacts at block boundaries during overlap-add reconstruction

    let fft_size = 2048;
    let hop_size = fft_size / 2; // 50% overlap
    let sample_rate = 44100;

    let mut plugin = UpmixerPlugin::new(
        fft_size, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5
    );
    plugin.initialize(sample_rate).unwrap();

    // Create continuous sine wave input (1kHz stereo)
    let num_blocks = 4;
    let total_samples = hop_size * (num_blocks + 1);
    let mut continuous_input = vec![0.0; total_samples * 2];

    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        continuous_input[i * 2] = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.3;
        continuous_input[i * 2 + 1] = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.3;
    }

    // Process using Plugin trait (which handles overlap-add internally)
    let output_channels = 6;
    let mut output = vec![0.0; total_samples * output_channels];
    let context = sotf_audio::ProcessContext {
        sample_rate,
        num_frames: total_samples,
    };
    plugin.process(&continuous_input, &mut output, &context).unwrap();

    // Check for discontinuities at expected block boundaries
    // Measure the maximum derivative (difference between consecutive samples)
    let mut max_derivative = 0.0_f32;
    let mut max_derivative_position = 0;

    for ch in 0..output_channels {
        for i in 1..(total_samples - 1) {
            let idx = i * output_channels + ch;
            let derivative = (output[idx] - output[idx - output_channels]).abs();

            if derivative > max_derivative {
                max_derivative = derivative;
                max_derivative_position = i;
            }
        }
    }

    println!("Max derivative: {:.6} at sample {}", max_derivative, max_derivative_position);

    // With proper synthesis windowing, the maximum derivative should be reasonable
    // For a 1kHz sine at 44.1kHz, max derivative ≈ 2π*1000*0.3/44100 ≈ 0.043
    // Allow 10x margin for upmixer processing
    assert!(
        max_derivative < 0.5,
        "Excessive discontinuity detected: {:.6}. This indicates crackling artifacts from improper windowing.",
        max_derivative
    );

    // Verify output is not silent
    // Note: After channel normalization (0.9/sqrt(2) ≈ 0.636), energy is reduced
    // Energy scales with amplitude squared, so we expect ~0.4x of original energy
    let total_energy: f32 = output.iter().map(|x| x * x).sum();
    assert!(total_energy > 20.0, "Output should have significant energy (got {:.2})", total_energy);

    println!("✓ Synthesis windowing test passed: no crackling detected");
}

#[test]
fn test_upmixer_channel_normalization_no_clipping() {
    use sotf_audio::Plugin;

    // This test verifies that channel normalization prevents clipping when summing
    // multiple signal components (direct + ambient) across output speakers

    let fft_size = 2048;
    let sample_rate = 44100;

    let mut plugin = UpmixerPlugin::new(
        fft_size, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5
    );
    plugin.initialize(sample_rate).unwrap();

    // Create high-amplitude input signal (0.95 amplitude to avoid input clipping)
    // Use complex waveform (multiple frequencies) to stress-test the upmixer
    let num_samples = 8192;
    let mut input = vec![0.0; num_samples * 2];

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        // Mix of frequencies with high amplitude
        let signal = 0.95 * (
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.4 +
            (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.3 +
            (2.0 * std::f32::consts::PI * 1320.0 * t).sin() * 0.2 +
            (2.0 * std::f32::consts::PI * 220.0 * t).sin() * 0.1
        );
        input[i * 2] = signal;
        input[i * 2 + 1] = signal;
    }

    let output_channels = 6;
    let mut output = vec![0.0; num_samples * output_channels];
    let context = sotf_audio::ProcessContext {
        sample_rate,
        num_frames: num_samples,
    };
    plugin.process(&input, &mut output, &context).unwrap();

    // Check for clipping (values exceeding [-1.0, 1.0])
    let mut max_sample = 0.0_f32;
    let mut min_sample = 0.0_f32;
    let mut clipped_samples = 0;

    for &sample in output.iter() {
        max_sample = max_sample.max(sample);
        min_sample = min_sample.min(sample);

        if sample.abs() > 1.0 {
            clipped_samples += 1;
        }
    }

    println!("Output range: [{:.6}, {:.6}]", min_sample, max_sample);
    println!("Clipped samples: {}/{}", clipped_samples, output.len());

    // With proper channel normalization, output should stay within [-1.0, 1.0]
    assert!(
        max_sample <= 1.0,
        "Positive clipping detected: max sample = {:.6}. Channel normalization is insufficient.",
        max_sample
    );

    assert!(
        min_sample >= -1.0,
        "Negative clipping detected: min sample = {:.6}. Channel normalization is insufficient.",
        min_sample
    );

    // Verify output has reasonable amplitude (not overly attenuated)
    let rms: f32 = (output.iter().map(|x| x * x).sum::<f32>() / output.len() as f32).sqrt();
    println!("Output RMS: {:.6}", rms);

    assert!(
        rms > 0.01,
        "Output RMS too low: {:.6}. Signal may be overly attenuated.",
        rms
    );

    println!("✓ Channel normalization test passed: no clipping detected");
}

#[test]
fn test_upmixer_denormal_flushing() {
    use sotf_audio::Plugin;

    // This test verifies that denormal numbers (very small floats) are flushed to zero
    // to prevent CPU performance spikes and numerical instability

    let fft_size = 2048;
    let sample_rate = 44100;

    let mut plugin = UpmixerPlugin::new(
        fft_size, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5
    );
    plugin.initialize(sample_rate).unwrap();

    // Create very low amplitude input (below denormal threshold)
    let num_samples = 8192;
    let mut input = vec![0.0; num_samples * 2];

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        // Extremely low amplitude signal (1e-35 is in denormal range)
        let signal = 1e-35 * (2.0 * std::f32::consts::PI * 1000.0 * t).sin();
        input[i * 2] = signal;
        input[i * 2 + 1] = signal;
    }

    let output_channels = 6;
    let mut output = vec![0.0; num_samples * output_channels];
    let context = sotf_audio::ProcessContext {
        sample_rate,
        num_frames: num_samples,
    };
    plugin.process(&input, &mut output, &context).unwrap();

    // Count denormal samples (between 0 and 1e-30)
    let mut denormal_count = 0;
    let mut zero_count = 0;
    let mut normal_count = 0;

    for &sample in output.iter() {
        let abs_sample = sample.abs();
        if abs_sample == 0.0 {
            zero_count += 1;
        } else if abs_sample < 1e-30 {
            denormal_count += 1;
        } else {
            normal_count += 1;
        }
    }

    println!("Zero samples: {}", zero_count);
    println!("Denormal samples (< 1e-30): {}", denormal_count);
    println!("Normal samples (>= 1e-30): {}", normal_count);

    // With proper denormal flushing, there should be NO denormal samples
    assert_eq!(
        denormal_count, 0,
        "Found {} denormal samples. Denormal flushing is not working correctly.",
        denormal_count
    );

    // Most samples should be flushed to zero given the tiny input
    let zero_percentage = (zero_count as f32 / output.len() as f32) * 100.0;
    println!("Zero samples: {:.2}%", zero_percentage);

    assert!(
        zero_percentage > 90.0,
        "Only {:.2}% samples are zero. Expected >90% for denormal input.",
        zero_percentage
    );

    println!("✓ Denormal flushing test passed: no denormals detected");
}

#[test]
fn test_upmixer_subharmonic_smoothing() {
    use sotf_audio::Plugin;

    // This test verifies that sub-harmonic synthesis uses smooth attack/release
    // envelopes to prevent clicks and pops when turning on/off

    let fft_size = 2048;
    let sample_rate = 44100;

    // Enable sub-harmonic synthesis
    let mut plugin = UpmixerPlugin::new(
        fft_size, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, true, 0.5
    );
    plugin.initialize(sample_rate).unwrap();

    // Create input with sharp amplitude transition: silence -> signal -> silence
    // This tests both attack (turn-on) and release (turn-off) smoothing
    let silence_samples = 2048;
    let signal_samples = 2048;
    let total_samples = silence_samples + signal_samples + silence_samples;

    let mut input = vec![0.0; total_samples * 2];

    // Add low-frequency signal in the middle section (to trigger LFE)
    for i in silence_samples..(silence_samples + signal_samples) {
        let t = i as f32 / sample_rate as f32;
        let signal = (2.0 * std::f32::consts::PI * 60.0 * t).sin() * 0.5; // 60Hz (in LFE range)
        input[i * 2] = signal;
        input[i * 2 + 1] = signal;
    }

    let output_channels = 6;
    let mut output = vec![0.0; total_samples * output_channels];
    let context = sotf_audio::ProcessContext {
        sample_rate,
        num_frames: total_samples,
    };
    plugin.process(&input, &mut output, &context).unwrap();

    // Extract LFE channel (channel 3 in 5.1 configuration)
    let lfe_channel_idx = 3;
    let mut lfe_samples = Vec::new();
    for i in 0..total_samples {
        lfe_samples.push(output[i * output_channels + lfe_channel_idx]);
    }

    // Check for discontinuities at the transitions (silence -> signal, signal -> silence)
    // Measure maximum derivative in the transition regions
    let transition_attack_start = silence_samples;
    let transition_release_start = silence_samples + signal_samples;
    let check_window = 100; // Check 100 samples around transition

    let mut max_derivative_attack = 0.0_f32;
    let mut max_derivative_release = 0.0_f32;

    // Attack region (first 100 samples after signal starts)
    for i in transition_attack_start..(transition_attack_start + check_window).min(lfe_samples.len() - 1) {
        let derivative = (lfe_samples[i + 1] - lfe_samples[i]).abs();
        max_derivative_attack = max_derivative_attack.max(derivative);
    }

    // Release region (first 100 samples after signal ends)
    for i in transition_release_start..(transition_release_start + check_window).min(lfe_samples.len() - 1) {
        let derivative = (lfe_samples[i + 1] - lfe_samples[i]).abs();
        max_derivative_release = max_derivative_release.max(derivative);
    }

    println!("Max derivative at attack: {:.6}", max_derivative_attack);
    println!("Max derivative at release: {:.6}", max_derivative_release);

    // With smooth envelope, derivatives should be small
    // For a 60Hz sine at 44.1kHz, max derivative ≈ 2π*60*0.5/44100 ≈ 0.0043
    // Allow 20x margin for sub-harmonic synthesis processing
    assert!(
        max_derivative_attack < 0.1,
        "Excessive discontinuity at attack: {:.6}. Sub-harmonic smoothing not working.",
        max_derivative_attack
    );

    assert!(
        max_derivative_release < 0.1,
        "Excessive discontinuity at release: {:.6}. Sub-harmonic smoothing not working.",
        max_derivative_release
    );

    // Verify LFE channel has content in the signal region
    let lfe_signal_energy: f32 = lfe_samples[silence_samples..(silence_samples + signal_samples)]
        .iter()
        .map(|x| x * x)
        .sum();

    assert!(
        lfe_signal_energy > 10.0,
        "LFE channel should have significant energy in signal region"
    );

    println!("✓ Sub-harmonic smoothing test passed: smooth attack/release detected");
}
