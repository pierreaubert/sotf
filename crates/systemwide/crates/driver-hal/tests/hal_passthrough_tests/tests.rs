use super::misc::generate_test_audio;
use super::types::create_mock_shared_memory;
use driver_hal::SharedAudioBuffer;
use sotf_plugins::param_specs::gain::default_smoothing_ms;
use sotf_plugins::{
    BiquadFilterConfig, EqPlugin, EqPluginParams, ParametricPluginAdapter, Plugin, ProcessContext,
};
use sotf_plugins::{GainPlugin, GainPluginParams};

#[test]
fn test_hal_shared_memory_passthrough_bit_exact() {
    let sample_rate = 48000;
    let buffer_frames = 512;
    let channel_count = 2;

    let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);
    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

    // Generate test audio
    let input_audio =
        generate_test_audio(buffer_frames as usize, channel_count as usize, sample_rate);

    // Write to shared memory
    let frames_written = buffer.write_audio(&input_audio);
    assert_eq!(frames_written, buffer_frames as usize);

    // Read back
    let mut output_audio = vec![0.0f32; input_audio.len()];
    let frames_read = buffer.read_audio(&mut output_audio);
    assert_eq!(frames_read, buffer_frames as usize);

    // Verify bit-for-bit accuracy
    let mut mismatches = 0;
    for (i, (input, output)) in input_audio.iter().zip(output_audio.iter()).enumerate() {
        if input.to_bits() != output.to_bits() {
            mismatches += 1;
            if mismatches <= 5 {
                eprintln!(
                    "Sample {}: input={:.10} (bits={:#010x}), output={:.10} (bits={:#010x})",
                    i,
                    input,
                    input.to_bits(),
                    output,
                    output.to_bits()
                );
            }
        }
    }
    assert_eq!(mismatches, 0, "Found {} bit-for-bit mismatches", mismatches);
}

#[test]
fn test_hal_encrypted_shared_memory_passthrough_bit_exact() {
    let sample_rate = 48000;
    let buffer_frames = 512;
    let channel_count = 2;

    let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);
    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");
    let key = driver_hal::generate_key();
    let cipher = driver_hal::AudioCipher::new(&key);
    buffer.set_key_fingerprint(*cipher.fingerprint());
    buffer.set_encrypted(true);

    let input_audio =
        generate_test_audio(buffer_frames as usize, channel_count as usize, sample_rate);

    let frames_written = buffer.write_audio_encrypted(&input_audio, &cipher);
    assert_eq!(frames_written, buffer_frames as usize);

    let mut output_audio = vec![0.0f32; input_audio.len()];
    let frames_read = buffer.read_audio_encrypted(&mut output_audio, &cipher);
    assert_eq!(frames_read, buffer_frames as usize);

    for (i, (input, output)) in input_audio.iter().zip(output_audio.iter()).enumerate() {
        assert_eq!(
            input.to_bits(),
            output.to_bits(),
            "encrypted passthrough mismatch at sample {i}"
        );
    }
}

#[test]
fn swift_hal_encryption_tests_are_part_of_cross_language_passthrough_suite() {
    let swift_tests = include_str!("../../swift/Sources/Tests.swift");
    assert!(swift_tests.contains("testEncryptionRoundTrip"));
    assert!(swift_tests.contains("AudioCipher(keyBytes:"));
    assert!(swift_tests.contains("cipher.encrypt(samples:"));
    assert!(swift_tests.contains("cipher.decrypt(ciphertext:"));
}

#[test]
fn test_hal_with_eq_zero_gain_passthrough() {
    // This test simulates the full pipeline:
    // 1. Audio data in shared memory
    // 2. Read from shared memory
    // 3. Process through EQ with zero-gain filters
    // 4. Write back to shared memory
    // 5. Read final output
    // 6. Verify bit-for-bit accuracy with original

    let sample_rate = 48000;
    let buffer_frames = 512;
    let channel_count = 2;

    // Create mock shared memory
    let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);
    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

    // Create EQ plugin with zero-gain filters
    let zero_gain_filters = vec![
        BiquadFilterConfig {
            filter_type: "peak".to_string(),
            freq: 1000.0,
            q: 1.0,
            db_gain: 0.0,
            order: 2,
            topology: Default::default(),
            lambda: None,
            kautz_sections: Vec::new(),
        },
        BiquadFilterConfig {
            filter_type: "lowshelf".to_string(),
            freq: 100.0,
            q: 0.707,
            db_gain: 0.0,
            order: 2,
            topology: Default::default(),
            lambda: None,
            kautz_sections: Vec::new(),
        },
        BiquadFilterConfig {
            filter_type: "highshelf".to_string(),
            freq: 10000.0,
            q: 0.707,
            db_gain: 0.0,
            order: 2,
            topology: Default::default(),
            lambda: None,
            kautz_sections: Vec::new(),
        },
    ];

    let params = EqPluginParams {
        filters: zero_gain_filters,
        channel_filters: None,
        auto_gain: Default::default(),
    };

    let mut plugin = ParametricPluginAdapter::new(
        EqPlugin::from_params(channel_count as usize, sample_rate, params)
            .expect("Failed to create EQ plugin"),
    );
    plugin
        .initialize(sample_rate)
        .expect("Failed to initialize");

    // Generate original audio
    let original_audio =
        generate_test_audio(buffer_frames as usize, channel_count as usize, sample_rate);

    // Step 1: Write original audio to shared memory
    buffer.write_audio(&original_audio);

    // Step 2: Read from shared memory (simulating engine reading from HAL)
    let mut read_audio = vec![0.0f32; original_audio.len()];
    buffer.read_audio(&mut read_audio);

    // Verify first read is bit-exact
    for (i, (orig, read)) in original_audio.iter().zip(read_audio.iter()).enumerate() {
        assert_eq!(
            orig.to_bits(),
            read.to_bits(),
            "First read mismatch at sample {}",
            i
        );
    }

    // Step 3: Process through EQ
    let mut processed_audio = vec![0.0f32; read_audio.len()];
    let context = ProcessContext::new(sample_rate, buffer_frames as usize);
    plugin
        .process(&read_audio, &mut processed_audio, &context)
        .expect("Failed to process");

    // Verify EQ processing is bit-exact
    for (i, (read, processed)) in read_audio.iter().zip(processed_audio.iter()).enumerate() {
        assert_eq!(
            read.to_bits(),
            processed.to_bits(),
            "EQ processing mismatch at sample {}",
            i
        );
    }

    // Step 4: Write processed audio back to shared memory
    buffer.write_audio(&processed_audio);

    // Step 5: Read final output
    let mut final_audio = vec![0.0f32; processed_audio.len()];
    buffer.read_audio(&mut final_audio);

    // Step 6: Verify final output matches original (full pipeline is bit-exact)
    let mut mismatches = 0;
    for (i, (orig, final_sample)) in original_audio.iter().zip(final_audio.iter()).enumerate() {
        if orig.to_bits() != final_sample.to_bits() {
            mismatches += 1;
            if mismatches <= 5 {
                eprintln!(
                    "Pipeline sample {}: original={:.10}, final={:.10}",
                    i, orig, final_sample
                );
            }
        }
    }

    assert_eq!(
        mismatches, 0,
        "Full pipeline (HAL read -> EQ zero-gain -> HAL write -> read) should be bit-exact, found {} mismatches",
        mismatches
    );
}

#[test]
fn test_eq_zero_gain_with_silence() {
    // Ensure silence remains silence (no DC offset or noise introduced)
    let params = EqPluginParams {
        filters: vec![BiquadFilterConfig {
            filter_type: "peak".to_string(),
            freq: 1000.0,
            q: 1.0,
            db_gain: 0.0,
            order: 2,
            topology: Default::default(),
            lambda: None,
            kautz_sections: Vec::new(),
        }],
        channel_filters: None,
        auto_gain: Default::default(),
    };

    let sample_rate = 48000;
    let num_channels = 2;
    let mut plugin = ParametricPluginAdapter::new(
        EqPlugin::from_params(num_channels, sample_rate, params)
            .expect("Failed to create EQ plugin"),
    );
    plugin
        .initialize(sample_rate)
        .expect("Failed to initialize");

    let num_frames = 1024;
    let input_audio = vec![0.0f32; num_frames * num_channels];
    let mut output_audio = vec![0.0f32; input_audio.len()];

    let context = ProcessContext::new(sample_rate, num_frames);

    plugin
        .process(&input_audio, &mut output_audio, &context)
        .expect("Failed to process");

    // All samples should be exactly zero
    for (i, &sample) in output_audio.iter().enumerate() {
        assert_eq!(
            sample, 0.0,
            "Silence should remain silence, sample {} = {}",
            i, sample
        );
        assert_eq!(
            sample.to_bits(),
            0.0f32.to_bits(),
            "Silence should be positive zero"
        );
    }
}

#[test]
fn test_volume_control_global_gain() {
    // Test global volume control (same gain on all channels)
    let sample_rate = 48000;
    let num_channels = 2;
    let num_frames = 256;

    // Create GainPlugin with -6dB (approximately 0.5x)
    let mut plugin = ParametricPluginAdapter::new(GainPlugin::new(num_channels, -6.0));
    plugin
        .initialize(sample_rate)
        .expect("Failed to initialize");

    // Generate test audio
    let input: Vec<f32> = (0..num_frames * num_channels)
        .map(|i| {
            let t = (i / num_channels) as f32 / sample_rate as f32;
            (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.8
        })
        .collect();

    let mut output = vec![0.0f32; input.len()];
    let context = ProcessContext::new(sample_rate, num_frames);

    plugin
        .process(&input, &mut output, &context)
        .expect("Failed to process");

    // Verify attenuation (should be approximately half amplitude)
    // -6dB ≈ 0.501x linear gain
    let expected_gain = 10.0_f32.powf(-6.0 / 20.0);
    for (i, (orig, processed)) in input.iter().zip(output.iter()).enumerate() {
        let expected = orig * expected_gain;
        assert!(
            (processed - expected).abs() < 0.001,
            "Sample {}: expected {}, got {}",
            i,
            expected,
            processed
        );
    }
}

#[test]
fn test_volume_control_per_channel() {
    // Test per-channel volume control
    let sample_rate = 48000;
    let num_channels = 2;
    let num_frames = 256;

    // Left channel: 0dB (unity), Right channel: -6dB (half)
    let params = GainPluginParams {
        gain_db: 0.0,
        smoothing_ms: default_smoothing_ms(),
        channel_gains: vec![0.0, -6.0],
    };
    let mut plugin = ParametricPluginAdapter::new(
        GainPlugin::from_params(num_channels, params).expect("Failed to create plugin"),
    );
    plugin
        .initialize(sample_rate)
        .expect("Failed to initialize");

    // Generate identical audio on both channels
    let input: Vec<f32> = (0..num_frames)
        .flat_map(|i| {
            let t = i as f32 / sample_rate as f32;
            let sample = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.8;
            [sample, sample] // Same sample on L and R
        })
        .collect();
    let mut buffer = vec![0.0f32; input.len()];

    let context = ProcessContext::new(sample_rate, num_frames);

    plugin
        .process(&input, &mut buffer, &context)
        .expect("Failed to process");

    // Verify per-channel gains
    let gain_r = 10.0_f32.powf(-6.0 / 20.0);
    for i in 0..num_frames {
        let left = buffer[i * 2];
        let right = buffer[i * 2 + 1];

        // Left should be nearly unchanged (0dB)
        // Right should be attenuated by ~0.5x (-6dB)
        // Note: They started equal, so right should be ~half of left
        assert!(
            (right - left * gain_r).abs() < 0.01,
            "Frame {}: right channel should be ~{:.1}x of left, got L={}, R={}",
            i,
            gain_r,
            left,
            right
        );
    }
}

#[test]
fn test_volume_control_multichannel() {
    // Test per-channel volume on 6-channel (5.1) configuration
    let sample_rate = 48000;
    let num_channels = 6;
    let num_frames = 256;

    // Different gain for each channel
    // FL=0dB, FR=-3dB, C=-6dB, LFE=-12dB, SL=-9dB, SR=-9dB
    let channel_gains = vec![0.0, -3.0, -6.0, -12.0, -9.0, -9.0];
    let params = GainPluginParams {
        gain_db: 0.0,
        smoothing_ms: default_smoothing_ms(),
        channel_gains: channel_gains.clone(),
    };
    let mut plugin = ParametricPluginAdapter::new(
        GainPlugin::from_params(num_channels, params).expect("Failed to create plugin"),
    );
    plugin
        .initialize(sample_rate)
        .expect("Failed to initialize");

    // Generate audio with same amplitude on all channels
    let amplitude = 0.8;
    let input: Vec<f32> = (0..num_frames)
        .flat_map(|i| {
            let t = i as f32 / sample_rate as f32;
            let sample = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * amplitude;
            vec![sample; num_channels]
        })
        .collect();
    let mut buffer = vec![0.0f32; input.len()];

    let context = ProcessContext::new(sample_rate, num_frames);

    plugin
        .process(&input, &mut buffer, &context)
        .expect("Failed to process");

    // Verify each channel has correct gain applied
    let linear_gains: Vec<f32> = channel_gains
        .iter()
        .map(|&db| 10.0_f32.powf(db / 20.0))
        .collect();

    for frame in 0..num_frames {
        for (ch, &gain) in linear_gains.iter().enumerate().take(num_channels) {
            let idx = frame * num_channels + ch;
            let t = frame as f32 / sample_rate as f32;
            let original = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * amplitude;
            let expected = original * gain;

            assert!(
                (buffer[idx] - expected).abs() < 0.01,
                "Frame {} Ch {}: expected {:.4}, got {:.4}",
                frame,
                ch,
                expected,
                buffer[idx]
            );
        }
    }
}

#[test]
fn test_eq_zero_gain_preserves_full_scale() {
    // Test with full-scale values to ensure no clipping or modification
    let params = EqPluginParams {
        filters: vec![BiquadFilterConfig {
            filter_type: "peak".to_string(),
            freq: 1000.0,
            q: 1.0,
            db_gain: 0.0,
            order: 2,
            topology: Default::default(),
            lambda: None,
            kautz_sections: Vec::new(),
        }],
        channel_filters: None,
        auto_gain: Default::default(),
    };

    let sample_rate = 48000;
    let num_channels = 2;
    let mut plugin = ParametricPluginAdapter::new(
        EqPlugin::from_params(num_channels, sample_rate, params)
            .expect("Failed to create EQ plugin"),
    );
    plugin
        .initialize(sample_rate)
        .expect("Failed to initialize");

    let num_frames = 256;
    // Create full-scale square wave
    let input_audio: Vec<f32> = (0..num_frames * num_channels)
        .map(|i| {
            if (i / num_channels) % 2 == 0 {
                1.0
            } else {
                -1.0
            }
        })
        .collect();
    let mut output_audio = vec![0.0f32; input_audio.len()];

    let context = ProcessContext::new(sample_rate, num_frames);

    plugin
        .process(&input_audio, &mut output_audio, &context)
        .expect("Failed to process");

    // Verify bit-for-bit accuracy
    for (i, (input, output)) in input_audio.iter().zip(output_audio.iter()).enumerate() {
        assert_eq!(
            input.to_bits(),
            output.to_bits(),
            "Full-scale sample {} should be preserved exactly",
            i
        );
    }
}
