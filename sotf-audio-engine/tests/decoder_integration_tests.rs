//! Decoder Integration Tests
//!
//! Tests for the audio decoder module including:
//! - File format support (WAV, FLAC, MP3, etc.)
//! - Audio specification parsing
//! - Decoding and sample conversion
//! - Seeking functionality
//! - Error handling

use hound::{SampleFormat, WavSpec, WavWriter};
use sotf_audio::decoder::{AudioDecoder, AudioFormat, DecodedAudio, create_decoder, probe_file};
use sotf_audio::AudioSpec;
use std::path::Path;
use tempfile::NamedTempFile;

// ============================================================================
// Test Helpers
// ============================================================================

/// Create a test WAV file with specific parameters
fn create_wav_file(
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    duration_secs: f32,
    frequency: f32,
) -> NamedTempFile {
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample,
        sample_format: if bits_per_sample == 32 {
            SampleFormat::Float
        } else {
            SampleFormat::Int
        },
    };

    let temp_file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    let mut writer = WavWriter::create(temp_file.path(), spec).unwrap();

    let num_frames = (duration_secs * sample_rate as f32) as usize;

    for frame in 0..num_frames {
        let t = frame as f32 / sample_rate as f32;
        let sample = (t * frequency * 2.0 * std::f32::consts::PI).sin();

        for _ in 0..channels {
            match bits_per_sample {
                16 => {
                    let amplitude = (sample * i16::MAX as f32 * 0.8) as i16;
                    writer.write_sample(amplitude).unwrap();
                }
                24 => {
                    let amplitude = (sample * 8388607.0 * 0.8) as i32;
                    writer.write_sample(amplitude).unwrap();
                }
                32 => {
                    writer.write_sample(sample * 0.8).unwrap();
                }
                _ => panic!("Unsupported bits per sample"),
            }
        }
    }

    writer.finalize().unwrap();
    temp_file
}

/// Create a stereo WAV with different frequencies per channel
fn create_stereo_wav_distinct_channels(
    sample_rate: u32,
    duration_secs: f32,
    left_freq: f32,
    right_freq: f32,
) -> NamedTempFile {
    let spec = WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let temp_file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    let mut writer = WavWriter::create(temp_file.path(), spec).unwrap();

    let num_frames = (duration_secs * sample_rate as f32) as usize;

    for frame in 0..num_frames {
        let t = frame as f32 / sample_rate as f32;
        let left = (t * left_freq * 2.0 * std::f32::consts::PI).sin();
        let right = (t * right_freq * 2.0 * std::f32::consts::PI).sin();

        writer
            .write_sample((left * i16::MAX as f32 * 0.8) as i16)
            .unwrap();
        writer
            .write_sample((right * i16::MAX as f32 * 0.8) as i16)
            .unwrap();
    }

    writer.finalize().unwrap();
    temp_file
}

// ============================================================================
// AudioSpec Tests
// ============================================================================

#[test]
fn test_audio_spec_duration_calculation() {
    let spec = AudioSpec {
        sample_rate: 48000,
        channels: 2,
        bits_per_sample: 16,
        total_frames: Some(48000), // 1 second
    };

    let duration = spec.duration().unwrap();
    assert_eq!(duration.as_secs(), 1);
    assert!(duration.as_millis() >= 999 && duration.as_millis() <= 1001);
}

#[test]
fn test_audio_spec_duration_none_when_unknown() {
    let spec = AudioSpec {
        sample_rate: 48000,
        channels: 2,
        bits_per_sample: 16,
        total_frames: None,
    };

    assert!(spec.duration().is_none());
}

#[test]
fn test_audio_spec_bytes_per_frame() {
    // Stereo 16-bit: 2 channels * 2 bytes = 4 bytes per frame
    let spec_16bit = AudioSpec {
        sample_rate: 48000,
        channels: 2,
        bits_per_sample: 16,
        total_frames: None,
    };
    assert_eq!(spec_16bit.bytes_per_frame(), 4);

    // Stereo 24-bit: 2 channels * 3 bytes = 6 bytes per frame
    let spec_24bit = AudioSpec {
        sample_rate: 96000,
        channels: 2,
        bits_per_sample: 24,
        total_frames: None,
    };
    assert_eq!(spec_24bit.bytes_per_frame(), 6);

    // 5.1 surround 32-bit: 6 channels * 4 bytes = 24 bytes per frame
    let spec_surround = AudioSpec {
        sample_rate: 48000,
        channels: 6,
        bits_per_sample: 32,
        total_frames: None,
    };
    assert_eq!(spec_surround.bytes_per_frame(), 24);
}

// ============================================================================
// DecodedAudio Tests
// ============================================================================

#[test]
fn test_decoded_audio_frame_count() {
    let spec = AudioSpec {
        sample_rate: 48000,
        channels: 2,
        bits_per_sample: 16,
        total_frames: None,
    };

    let mut audio = DecodedAudio::new(spec);
    assert_eq!(audio.frame_count(), 0);
    assert!(audio.is_empty());

    // Add 100 frames of stereo audio (200 samples)
    audio.samples = vec![0.0; 200];
    assert_eq!(audio.frame_count(), 100);
    assert!(!audio.is_empty());
}

#[test]
fn test_decoded_audio_clear() {
    let spec = AudioSpec {
        sample_rate: 48000,
        channels: 2,
        bits_per_sample: 16,
        total_frames: None,
    };

    let mut audio = DecodedAudio::new(spec);
    audio.samples = vec![1.0; 100];
    assert!(!audio.is_empty());

    audio.clear();
    assert!(audio.is_empty());
}

#[test]
fn test_decoded_audio_to_bytes() {
    let spec = AudioSpec {
        sample_rate: 48000,
        channels: 1,
        bits_per_sample: 32,
        total_frames: None,
    };

    let mut audio = DecodedAudio::new(spec);
    audio.samples = vec![0.5, -0.5, 1.0, -1.0];

    let bytes = audio.to_bytes_f32_le();
    assert_eq!(bytes.len(), 16); // 4 samples * 4 bytes each

    // Verify first sample (0.5)
    let first_sample = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    assert!((first_sample - 0.5).abs() < 1e-6);
}

// ============================================================================
// File Probing Tests
// ============================================================================

#[test]
fn test_probe_wav_file() {
    let wav_file = create_wav_file(48000, 2, 16, 1.0, 440.0);

    let result = probe_file(wav_file.path());
    assert!(result.is_ok(), "Failed to probe WAV file: {:?}", result.err());

    let (format, spec) = result.unwrap();
    assert_eq!(format, AudioFormat::Wav);
    assert_eq!(spec.sample_rate, 48000);
    assert_eq!(spec.channels, 2);
    assert!(spec.total_frames.is_some());
}

#[test]
fn test_probe_wav_various_sample_rates() {
    let sample_rates = [22050, 44100, 48000, 96000];

    for &sr in &sample_rates {
        let wav_file = create_wav_file(sr, 2, 16, 0.5, 440.0);
        let result = probe_file(wav_file.path());

        assert!(
            result.is_ok(),
            "Failed to probe WAV at {}Hz: {:?}",
            sr,
            result.err()
        );
        let (_format, spec) = result.unwrap();
        assert_eq!(spec.sample_rate, sr);
    }
}

#[test]
fn test_probe_wav_various_channel_counts() {
    let channel_counts = [1, 2, 4, 6, 8];

    for &ch in &channel_counts {
        let wav_file = create_wav_file(48000, ch, 16, 0.5, 440.0);
        let result = probe_file(wav_file.path());

        assert!(
            result.is_ok(),
            "Failed to probe WAV with {} channels: {:?}",
            ch,
            result.err()
        );
        let (_format, spec) = result.unwrap();
        assert_eq!(spec.channels, ch);
    }
}

#[test]
fn test_probe_nonexistent_file() {
    let result = probe_file(Path::new("/nonexistent/path/to/file.wav"));
    assert!(result.is_err());
}

// ============================================================================
// Decoder Creation Tests
// ============================================================================

#[test]
fn test_create_decoder_wav() {
    let wav_file = create_wav_file(48000, 2, 16, 1.0, 440.0);

    let result = create_decoder(wav_file.path());
    assert!(
        result.is_ok(),
        "Failed to create WAV decoder: {:?}",
        result.err()
    );

    let decoder = result.unwrap();
    assert_eq!(decoder.format(), AudioFormat::Wav);
    assert_eq!(decoder.spec().sample_rate, 48000);
    assert_eq!(decoder.spec().channels, 2);
}

#[test]
fn test_create_decoder_invalid_file() {
    // Create a file with invalid content
    let temp_file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    std::fs::write(temp_file.path(), b"not a valid audio file").unwrap();

    let result = create_decoder(temp_file.path());
    assert!(result.is_err());
}

// ============================================================================
// Decoding Tests
// ============================================================================

#[test]
fn test_decode_wav_16bit() {
    let wav_file = create_wav_file(48000, 2, 16, 0.5, 440.0);
    let mut decoder = create_decoder(wav_file.path()).unwrap();

    let mut total_frames = 0;
    let mut iterations = 0;
    const MAX_ITERATIONS: usize = 1000;

    while iterations < MAX_ITERATIONS {
        match decoder.decode_next() {
            Ok(Some(audio)) => {
                total_frames += audio.frame_count();
                // Verify samples are in valid range
                for &sample in &audio.samples {
                    assert!(
                        sample >= -1.0 && sample <= 1.0,
                        "Sample out of range: {}",
                        sample
                    );
                }
            }
            Ok(None) => break, // End of stream
            Err(e) => panic!("Decode error: {:?}", e),
        }
        iterations += 1;
    }

    // 0.5 seconds at 48000 Hz = 24000 frames
    assert!(
        total_frames >= 23000 && total_frames <= 25000,
        "Expected ~24000 frames, got {}",
        total_frames
    );
}

#[test]
fn test_decode_wav_24bit() {
    let wav_file = create_wav_file(48000, 2, 24, 0.5, 440.0);
    let mut decoder = create_decoder(wav_file.path()).unwrap();

    let mut total_frames = 0;

    while let Ok(Some(audio)) = decoder.decode_next() {
        total_frames += audio.frame_count();
        // Verify samples are normalized
        for &sample in &audio.samples {
            assert!(sample >= -1.0 && sample <= 1.0);
        }
    }

    assert!(total_frames > 0, "No frames decoded from 24-bit WAV");
}

#[test]
fn test_decode_wav_32bit_float() {
    let wav_file = create_wav_file(48000, 2, 32, 0.5, 440.0);
    let mut decoder = create_decoder(wav_file.path()).unwrap();

    let mut total_frames = 0;

    while let Ok(Some(audio)) = decoder.decode_next() {
        total_frames += audio.frame_count();
    }

    assert!(total_frames > 0, "No frames decoded from 32-bit float WAV");
}

#[test]
fn test_decode_mono_wav() {
    let wav_file = create_wav_file(48000, 1, 16, 0.5, 440.0);
    let mut decoder = create_decoder(wav_file.path()).unwrap();

    assert_eq!(decoder.spec().channels, 1);

    let audio = decoder.decode_next().unwrap().unwrap();
    // Mono: samples per frame = 1
    assert_eq!(audio.samples.len(), audio.frame_count());
}

#[test]
fn test_decode_multichannel_wav() {
    let wav_file = create_wav_file(48000, 6, 16, 0.5, 440.0);
    let mut decoder = create_decoder(wav_file.path()).unwrap();

    assert_eq!(decoder.spec().channels, 6);

    let audio = decoder.decode_next().unwrap().unwrap();
    // 6 channels: samples per frame = 6
    assert_eq!(audio.samples.len(), audio.frame_count() * 6);
}

#[test]
fn test_decode_stereo_channel_separation() {
    // Create stereo file with different frequencies per channel
    let wav_file = create_stereo_wav_distinct_channels(48000, 0.5, 440.0, 880.0);
    let mut decoder = create_decoder(wav_file.path()).unwrap();

    let audio = decoder.decode_next().unwrap().unwrap();

    // Extract left and right channels
    let left: Vec<f32> = audio.samples.iter().step_by(2).copied().collect();
    let right: Vec<f32> = audio.samples.iter().skip(1).step_by(2).copied().collect();

    assert_eq!(left.len(), right.len());
    assert!(!left.is_empty());

    // Channels should have different content (different frequencies)
    let mut differences = 0;
    for (l, r) in left.iter().zip(right.iter()) {
        if (l - r).abs() > 0.01 {
            differences += 1;
        }
    }

    // Most samples should be different
    assert!(
        differences > left.len() / 2,
        "Left and right channels should differ"
    );
}

// ============================================================================
// Sample Rate Tests
// ============================================================================

#[test]
fn test_decode_various_sample_rates() {
    let sample_rates = [22050, 44100, 48000, 96000];

    for &sr in &sample_rates {
        let wav_file = create_wav_file(sr, 2, 16, 0.1, 440.0);
        let mut decoder = create_decoder(wav_file.path()).unwrap();

        assert_eq!(decoder.spec().sample_rate, sr);

        let audio = decoder.decode_next().unwrap().unwrap();
        assert!(!audio.is_empty(), "No audio decoded at {}Hz", sr);
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_decode_very_short_file() {
    // Create a very short file (10ms)
    let wav_file = create_wav_file(48000, 2, 16, 0.01, 440.0);
    let mut decoder = create_decoder(wav_file.path()).unwrap();

    let mut total_frames = 0;
    while let Ok(Some(audio)) = decoder.decode_next() {
        total_frames += audio.frame_count();
    }

    // 10ms at 48kHz = 480 frames
    assert!(
        total_frames >= 400 && total_frames <= 600,
        "Expected ~480 frames, got {}",
        total_frames
    );
}

#[test]
fn test_decode_empty_after_eos() {
    let wav_file = create_wav_file(48000, 2, 16, 0.1, 440.0);
    let mut decoder = create_decoder(wav_file.path()).unwrap();

    // Decode until end
    while let Ok(Some(_)) = decoder.decode_next() {}

    // Further decode attempts should return None
    assert!(decoder.decode_next().unwrap().is_none());
    assert!(decoder.decode_next().unwrap().is_none());
}
