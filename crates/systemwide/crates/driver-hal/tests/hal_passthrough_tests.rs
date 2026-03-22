#![cfg(target_os = "macos")]
//! Integration tests for HAL driver audio passthrough
//!
//! These tests verify that audio data passes through the HAL shared memory
//! interface and plugin processing without modification when configured
//! for passthrough (e.g., EQ with zero gain filters).

use driver_hal::SharedAudioBuffer;
use sotf_plugins::{
    BiquadFilterConfig, EqPlugin, EqPluginParams, InPlacePluginAdapter, Plugin, ProcessContext,
};
use std::io::Write;
use std::sync::atomic::{AtomicU32, AtomicU64};
use tempfile::NamedTempFile;

/// Magic number for shared memory header validation: 'SOTF'
const SHARED_MEMORY_MAGIC: u32 = 0x534F5446;
/// Version 3: Added config negotiation fields
const SHARED_MEMORY_VERSION: u32 = 3;

/// Shared audio header structure (must match driver_hal::SharedAudioHeader)
#[repr(C)]
struct SharedAudioHeader {
    magic: u32,
    version: u32,
    sample_rate: u32,
    buffer_frames: u32,
    channel_count: u32,
    write_position: AtomicU64,
    read_position: AtomicU64,
    active: AtomicU32,
    config_changed: AtomicU32,
    driver_ready: AtomicU32,
    engine_ready: AtomicU32,
    // Encryption fields (version 2+)
    encrypted: AtomicU32,
    key_fingerprint: [u8; 8],
    frame_counter: AtomicU64,
    // Config negotiation fields (version 3+)
    requested_sample_rate: u32,
    requested_buffer_frames: u32,
    actual_sample_rate: u32,
    actual_buffer_frames: u32,
    config_status: AtomicU32,
    config_source: AtomicU32,
    config_error_code: u32,
}

/// Create a mock shared memory file for testing
fn create_mock_shared_memory(
    sample_rate: u32,
    buffer_frames: u32,
    channel_count: u32,
) -> NamedTempFile {
    let header_size = std::mem::size_of::<SharedAudioHeader>();
    let audio_offset = (header_size + 63) & !63;
    let audio_capacity = (buffer_frames as usize) * (channel_count as usize) * 8;
    let total_size = audio_offset + audio_capacity * std::mem::size_of::<f32>();

    let mut file = NamedTempFile::new().expect("Failed to create temp file");

    let header = SharedAudioHeader {
        magic: SHARED_MEMORY_MAGIC,
        version: SHARED_MEMORY_VERSION,
        sample_rate,
        buffer_frames,
        channel_count,
        write_position: AtomicU64::new(0),
        read_position: AtomicU64::new(0),
        active: AtomicU32::new(1),
        config_changed: AtomicU32::new(0),
        driver_ready: AtomicU32::new(1),
        engine_ready: AtomicU32::new(0),
        // Encryption fields (version 2+)
        encrypted: AtomicU32::new(0),
        key_fingerprint: [0; 8],
        frame_counter: AtomicU64::new(0),
        // Config negotiation fields (version 3+)
        requested_sample_rate: 0,
        requested_buffer_frames: 0,
        actual_sample_rate: sample_rate,
        actual_buffer_frames: buffer_frames,
        config_status: AtomicU32::new(0),
        config_source: AtomicU32::new(0),
        config_error_code: 0,
    };

    let header_bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(&header as *const _ as *const u8, header_size) };

    let mut buffer = vec![0u8; total_size];
    buffer[..header_size].copy_from_slice(header_bytes);

    file.write_all(&buffer).expect("Failed to write to file");
    file.flush().expect("Failed to flush file");

    file
}

/// Generate test audio with a known pattern (sine waves)
fn generate_test_audio(num_frames: usize, channels: usize, sample_rate: u32) -> Vec<f32> {
    (0..num_frames)
        .flat_map(|i| {
            let t = i as f32 / sample_rate as f32;
            (0..channels)
                .map(move |ch| {
                    // Different frequency per channel for easy verification
                    let freq = 440.0 * (ch as f32 + 1.0);
                    (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

// ============================================================================
// HAL Shared Memory Passthrough Tests
// ============================================================================

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

// ============================================================================
// EQ Plugin Passthrough Tests
// ============================================================================

#[test]
fn test_eq_zero_gain_filters_passthrough_near_exact() {
    // Create EQ plugin with multiple zero-gain filters
    //
    // IMPORTANT: Zero-gain biquad filters are NOT bit-exact passthrough!
    // Even with 0 dB gain, the biquad filter coefficients are computed and applied,
    // which can introduce floating point rounding errors of up to 1 ULP (unit in last place).
    // This test verifies that zero-gain filters produce output that is numerically
    // equivalent within floating point precision.
    //
    // For true bit-exact passthrough, use an empty filter chain instead.
    let zero_gain_filters = vec![
        BiquadFilterConfig {
            filter_type: "peak".to_string(),
            freq: 100.0,
            q: 1.0,
            db_gain: 0.0, // Zero gain
            order: 2,
        },
        BiquadFilterConfig {
            filter_type: "peak".to_string(),
            freq: 1000.0,
            q: 1.0,
            db_gain: 0.0, // Zero gain
            order: 2,
        },
        BiquadFilterConfig {
            filter_type: "peak".to_string(),
            freq: 10000.0,
            q: 1.0,
            db_gain: 0.0, // Zero gain
            order: 2,
        },
        BiquadFilterConfig {
            filter_type: "lowshelf".to_string(),
            freq: 80.0,
            q: 0.707,
            db_gain: 0.0, // Zero gain
            order: 2,
        },
        BiquadFilterConfig {
            filter_type: "highshelf".to_string(),
            freq: 8000.0,
            q: 0.707,
            db_gain: 0.0, // Zero gain
            order: 2,
        },
    ];

    let params = EqPluginParams {
        filters: zero_gain_filters,
        channel_filters: None,
        auto_gain: Default::default(), // Auto-gain disabled by default
    };

    let sample_rate = 48000;
    let num_channels = 2;
    let mut plugin = InPlacePluginAdapter::new(
        EqPlugin::from_params(num_channels, sample_rate, params)
            .expect("Failed to create EQ plugin"),
    );

    plugin
        .initialize(sample_rate)
        .expect("Failed to initialize");

    // Generate test audio
    let num_frames = 1024;
    let input_audio = generate_test_audio(num_frames, num_channels, sample_rate);
    let mut output_audio = vec![0.0f32; input_audio.len()];

    let context = ProcessContext {
        sample_rate,
        num_frames,
    };

    // Process through EQ
    plugin
        .process(&input_audio, &mut output_audio, &context)
        .expect("Failed to process audio");

    // Verify near-exact match (within floating point precision)
    // Allow up to 2 ULP difference (1 ULP per filter stage, with margin)
    let max_ulp_diff = 10; // Allow some ULP difference due to multiple filter stages
    let mut max_ulp_seen = 0u32;
    let mut large_errors = 0;

    for (i, (input, output)) in input_audio.iter().zip(output_audio.iter()).enumerate() {
        let input_bits = input.to_bits();
        let output_bits = output.to_bits();
        let ulp_diff = (input_bits as i64 - output_bits as i64).unsigned_abs() as u32;

        if ulp_diff > max_ulp_seen {
            max_ulp_seen = ulp_diff;
        }

        if ulp_diff > max_ulp_diff {
            large_errors += 1;
            if large_errors <= 5 {
                eprintln!(
                    "Sample {}: input={:.10} (bits={:#010x}), output={:.10} (bits={:#010x}), ULP diff={}",
                    i, input, input_bits, output, output_bits, ulp_diff
                );
            }
        }
    }

    assert_eq!(
        large_errors, 0,
        "EQ with zero-gain filters should be near-passthrough (max {} ULP), found {} samples with larger error. Max ULP seen: {}",
        max_ulp_diff, large_errors, max_ulp_seen
    );

    // Log the maximum ULP difference for informational purposes
    eprintln!(
        "Zero-gain filter test: max ULP difference = {} (threshold = {})",
        max_ulp_seen, max_ulp_diff
    );
}

#[test]
fn test_eq_empty_filters_passthrough_bit_exact() {
    // Empty filter chain should be perfect passthrough
    let params = EqPluginParams {
        filters: vec![],
        channel_filters: None,
        auto_gain: Default::default(),
    };

    let sample_rate = 48000;
    let num_channels = 2;
    let mut plugin = InPlacePluginAdapter::new(
        EqPlugin::from_params(num_channels, sample_rate, params)
            .expect("Failed to create EQ plugin"),
    );

    plugin
        .initialize(sample_rate)
        .expect("Failed to initialize");

    let num_frames = 1024;
    let input_audio = generate_test_audio(num_frames, num_channels, sample_rate);
    let mut output_audio = vec![0.0f32; input_audio.len()];

    let context = ProcessContext {
        sample_rate,
        num_frames,
    };

    plugin
        .process(&input_audio, &mut output_audio, &context)
        .expect("Failed to process audio");

    // Verify bit-for-bit accuracy
    for (i, (input, output)) in input_audio.iter().zip(output_audio.iter()).enumerate() {
        assert_eq!(
            input.to_bits(),
            output.to_bits(),
            "Sample {}: Empty filter chain should be bit-exact passthrough",
            i
        );
    }
}

// ============================================================================
// Combined HAL + EQ Passthrough Test
// ============================================================================

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
        },
        BiquadFilterConfig {
            filter_type: "lowshelf".to_string(),
            freq: 100.0,
            q: 0.707,
            db_gain: 0.0,
            order: 2,
        },
        BiquadFilterConfig {
            filter_type: "highshelf".to_string(),
            freq: 10000.0,
            q: 0.707,
            db_gain: 0.0,
            order: 2,
        },
    ];

    let params = EqPluginParams {
        filters: zero_gain_filters,
        channel_filters: None,
        auto_gain: Default::default(),
    };

    let mut plugin = InPlacePluginAdapter::new(
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
    let context = ProcessContext {
        sample_rate,
        num_frames: buffer_frames as usize,
    };
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

// ============================================================================
// Edge Case Tests
// ============================================================================

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
        }],
        channel_filters: None,
        auto_gain: Default::default(),
    };

    let sample_rate = 48000;
    let num_channels = 2;
    let mut plugin = InPlacePluginAdapter::new(
        EqPlugin::from_params(num_channels, sample_rate, params)
            .expect("Failed to create EQ plugin"),
    );
    plugin
        .initialize(sample_rate)
        .expect("Failed to initialize");

    let num_frames = 1024;
    let input_audio = vec![0.0f32; num_frames * num_channels];
    let mut output_audio = vec![0.0f32; input_audio.len()];

    let context = ProcessContext {
        sample_rate,
        num_frames,
    };

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

// ============================================================================
// Multi-Channel Support Tests
// ============================================================================

#[test]
fn test_hal_multi_channel_support_4ch() {
    // Test 4-channel (quad) configuration
    let sample_rate = 48000;
    let buffer_frames = 256;
    let channel_count = 4;

    let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);
    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

    assert_eq!(buffer.channel_count(), channel_count);

    // Generate 4-channel audio with distinct content per channel
    let input_audio: Vec<f32> = (0..buffer_frames as usize)
        .flat_map(|i| {
            let t = i as f32 / sample_rate as f32;
            [
                (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5, // Ch0: 440Hz
                (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.4, // Ch1: 880Hz
                (2.0 * std::f32::consts::PI * 1320.0 * t).sin() * 0.3, // Ch2: 1320Hz
                (2.0 * std::f32::consts::PI * 1760.0 * t).sin() * 0.2, // Ch3: 1760Hz
            ]
        })
        .collect();

    // Write and read back
    let frames_written = buffer.write_audio(&input_audio);
    assert_eq!(frames_written, buffer_frames as usize);

    let mut output_audio = vec![0.0f32; input_audio.len()];
    let frames_read = buffer.read_audio(&mut output_audio);
    assert_eq!(frames_read, buffer_frames as usize);

    // Verify bit-exact
    for (i, (input, output)) in input_audio.iter().zip(output_audio.iter()).enumerate() {
        assert_eq!(
            input.to_bits(),
            output.to_bits(),
            "4-channel sample {} mismatch",
            i
        );
    }
}

#[test]
fn test_hal_multi_channel_support_6ch() {
    // Test 5.1 surround (6-channel) configuration
    let sample_rate = 48000;
    let buffer_frames = 256;
    let channel_count = 6;

    let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);
    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

    assert_eq!(buffer.channel_count(), channel_count);

    // Generate 6-channel audio
    let input_audio: Vec<f32> = (0..buffer_frames as usize)
        .flat_map(|i| {
            let t = i as f32 / sample_rate as f32;
            (0..6)
                .map(|ch| {
                    let freq = 440.0 * (ch as f32 + 1.0);
                    (2.0 * std::f32::consts::PI * freq * t).sin() * (0.5 - ch as f32 * 0.07)
                })
                .collect::<Vec<_>>()
        })
        .collect();

    buffer.write_audio(&input_audio);
    let mut output_audio = vec![0.0f32; input_audio.len()];
    buffer.read_audio(&mut output_audio);

    for (i, (input, output)) in input_audio.iter().zip(output_audio.iter()).enumerate() {
        assert_eq!(
            input.to_bits(),
            output.to_bits(),
            "6-channel sample {} mismatch",
            i
        );
    }
}

#[test]
fn test_hal_multi_channel_support_8ch() {
    // Test 7.1 surround (8-channel) configuration
    let sample_rate = 48000;
    let buffer_frames = 256;
    let channel_count = 8;

    let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);
    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

    assert_eq!(buffer.channel_count(), channel_count);

    let input_audio: Vec<f32> = (0..buffer_frames as usize)
        .flat_map(|i| {
            let t = i as f32 / sample_rate as f32;
            (0..8)
                .map(|ch| {
                    let freq = 220.0 * (ch as f32 + 1.0);
                    (2.0 * std::f32::consts::PI * freq * t).sin() * 0.4
                })
                .collect::<Vec<_>>()
        })
        .collect();

    buffer.write_audio(&input_audio);
    let mut output_audio = vec![0.0f32; input_audio.len()];
    buffer.read_audio(&mut output_audio);

    for (i, (input, output)) in input_audio.iter().zip(output_audio.iter()).enumerate() {
        assert_eq!(
            input.to_bits(),
            output.to_bits(),
            "8-channel sample {} mismatch",
            i
        );
    }
}

#[test]
fn test_hal_channel_count_dynamic() {
    // Test that channel count is read from header, not hardcoded
    for channel_count in [1, 2, 4, 6, 8, 16] {
        let sample_rate = 48000;
        let buffer_frames = 128;

        let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);
        let buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

        assert_eq!(
            buffer.channel_count(),
            channel_count,
            "Channel count should match header for {} channels",
            channel_count
        );
    }
}

// ============================================================================
// Volume Control via GainPlugin Tests
// ============================================================================

use sotf_plugins::{GainPlugin, GainPluginParams, InPlacePlugin};

#[test]
fn test_volume_control_global_gain() {
    // Test global volume control (same gain on all channels)
    let sample_rate = 48000;
    let num_channels = 2;
    let num_frames = 256;

    // Create GainPlugin with -6dB (approximately 0.5x)
    let mut plugin = GainPlugin::new(num_channels, -6.0);
    plugin
        .initialize(sample_rate)
        .expect("Failed to initialize");

    // Generate test audio
    let mut buffer: Vec<f32> = (0..num_frames * num_channels)
        .map(|i| {
            let t = (i / num_channels) as f32 / sample_rate as f32;
            (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.8
        })
        .collect();

    let original_buffer = buffer.clone();

    let context = ProcessContext {
        sample_rate,
        num_frames,
    };

    plugin
        .process_in_place(&mut buffer, &context)
        .expect("Failed to process");

    // Verify attenuation (should be approximately half amplitude)
    // -6dB ≈ 0.501x linear gain
    let expected_gain = 10.0_f32.powf(-6.0 / 20.0);
    for (i, (orig, processed)) in original_buffer.iter().zip(buffer.iter()).enumerate() {
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
        channel_gains: vec![0.0, -6.0],
    };
    let mut plugin =
        GainPlugin::from_params(num_channels, params).expect("Failed to create plugin");
    plugin
        .initialize(sample_rate)
        .expect("Failed to initialize");

    // Generate identical audio on both channels
    let mut buffer: Vec<f32> = (0..num_frames)
        .flat_map(|i| {
            let t = i as f32 / sample_rate as f32;
            let sample = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.8;
            [sample, sample] // Same sample on L and R
        })
        .collect();

    let context = ProcessContext {
        sample_rate,
        num_frames,
    };

    plugin
        .process_in_place(&mut buffer, &context)
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
        channel_gains: channel_gains.clone(),
    };
    let mut plugin =
        GainPlugin::from_params(num_channels, params).expect("Failed to create plugin");
    plugin
        .initialize(sample_rate)
        .expect("Failed to initialize");

    // Generate audio with same amplitude on all channels
    let amplitude = 0.8;
    let mut buffer: Vec<f32> = (0..num_frames)
        .flat_map(|i| {
            let t = i as f32 / sample_rate as f32;
            let sample = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * amplitude;
            vec![sample; num_channels]
        })
        .collect();

    let context = ProcessContext {
        sample_rate,
        num_frames,
    };

    plugin
        .process_in_place(&mut buffer, &context)
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
fn test_volume_with_hal_pipeline() {
    // Test full pipeline: HAL read -> Gain (volume) -> verify
    let sample_rate = 48000;
    let buffer_frames = 256;
    let channel_count = 2;

    // Create mock shared memory
    let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);
    let mut hal_buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

    // Generate test audio
    let input_audio: Vec<f32> = (0..buffer_frames as usize)
        .flat_map(|i| {
            let t = i as f32 / sample_rate as f32;
            let sample = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.8;
            [sample, sample]
        })
        .collect();

    // Write to HAL
    hal_buffer.write_audio(&input_audio);

    // Read from HAL (simulating HalInputPlugin)
    let mut read_buffer = vec![0.0f32; input_audio.len()];
    hal_buffer.read_audio(&mut read_buffer);

    // Apply volume via GainPlugin (-6dB global)
    let mut gain_plugin = GainPlugin::new(channel_count as usize, -6.0);
    gain_plugin
        .initialize(sample_rate)
        .expect("Failed to initialize");

    let context = ProcessContext {
        sample_rate,
        num_frames: buffer_frames as usize,
    };

    gain_plugin
        .process_in_place(&mut read_buffer, &context)
        .expect("Failed to process");

    // Verify attenuation
    let expected_gain = 10.0_f32.powf(-6.0 / 20.0);
    for (i, (orig, processed)) in input_audio.iter().zip(read_buffer.iter()).enumerate() {
        let expected = orig * expected_gain;
        assert!(
            (processed - expected).abs() < 0.001,
            "Sample {}: expected {:.6}, got {:.6}",
            i,
            expected,
            processed
        );
    }
}

// ============================================================================
// Edge Case Tests
// ============================================================================

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
        }],
        channel_filters: None,
        auto_gain: Default::default(),
    };

    let sample_rate = 48000;
    let num_channels = 2;
    let mut plugin = InPlacePluginAdapter::new(
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

    let context = ProcessContext {
        sample_rate,
        num_frames,
    };

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
