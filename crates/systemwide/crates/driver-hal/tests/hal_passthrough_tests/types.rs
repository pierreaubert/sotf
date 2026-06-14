use super::SharedAudioHeader;
use super::misc::SHARED_MEMORY_MAGIC;
use super::misc::SHARED_MEMORY_VERSION;
use driver_hal::SharedAudioBuffer;
use sotf_plugins::GainPlugin;
use sotf_plugins::InPlacePlugin;
use sotf_plugins::ProcessContext;
use std::io::Write;
use std::sync::atomic::{AtomicU32, AtomicU64};
use tempfile::NamedTempFile;

/// Create a mock shared memory file for testing
pub(super) fn create_mock_shared_memory(
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
        magic: AtomicU32::new(SHARED_MEMORY_MAGIC),
        version: AtomicU32::new(SHARED_MEMORY_VERSION),
        sample_rate: AtomicU32::new(sample_rate),
        buffer_frames: AtomicU32::new(buffer_frames),
        channel_count: AtomicU32::new(channel_count),
        write_position: AtomicU64::new(0),
        read_position: AtomicU64::new(0),
        active: AtomicU32::new(1),
        config_changed: AtomicU32::new(0),
        driver_ready: AtomicU32::new(1),
        engine_ready: AtomicU32::new(0),
        // Encryption fields (version 2+)
        encrypted: AtomicU32::new(0),
        key_fingerprint: AtomicU64::new(0),
        frame_counter: AtomicU64::new(0),
        // Config negotiation fields (version 3+)
        requested_sample_rate: AtomicU32::new(0),
        requested_buffer_frames: AtomicU32::new(0),
        actual_sample_rate: AtomicU32::new(sample_rate),
        actual_buffer_frames: AtomicU32::new(buffer_frames),
        config_status: AtomicU32::new(0),
        config_source: AtomicU32::new(0),
        config_error_code: AtomicU32::new(0),
        // Statistics
        encryption_overflow_count: AtomicU64::new(0),
        daemon_heartbeat_ms: AtomicU64::new(0),
        configuring: AtomicU32::new(0),
    };

    let header_bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(&header as *const _ as *const u8, header_size) };

    let mut buffer = vec![0u8; total_size];
    buffer[..header_size].copy_from_slice(header_bytes);

    file.write_all(&buffer).expect("Failed to write to file");
    file.flush().expect("Failed to flush file");

    file
}

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
    for channel_count in [1, 2, 4, 6, 8, 16, 32] {
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

    let context = ProcessContext::new(sample_rate, buffer_frames as usize);

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
