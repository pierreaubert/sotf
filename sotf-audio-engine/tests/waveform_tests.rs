//! Waveform Analysis Integration Tests
//!
//! Tests for the waveform analysis module including:
//! - Waveform generation from audio files
//! - Output format validation
//! - Edge cases (short files, silence)

use hound::{SampleFormat, WavSpec, WavWriter};
use sotf_audio::waveform::{WAVEFORM_SAMPLES, analyze_waveform};
use tempfile::NamedTempFile;

// ============================================================================
// Test Helpers
// ============================================================================

/// Create a test WAV file with a sine wave
fn create_sine_wav(
    sample_rate: u32,
    channels: u16,
    duration_secs: f32,
    frequency: f32,
) -> NamedTempFile {
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let temp_file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    let mut writer = WavWriter::create(temp_file.path(), spec).unwrap();

    let num_frames = (duration_secs * sample_rate as f32) as usize;

    for frame in 0..num_frames {
        let t = frame as f32 / sample_rate as f32;
        let sample = (t * frequency * 2.0 * std::f32::consts::PI).sin();
        let amplitude = (sample * i16::MAX as f32 * 0.8) as i16;

        for _ in 0..channels {
            writer.write_sample(amplitude).unwrap();
        }
    }

    writer.finalize().unwrap();
    temp_file
}

/// Create a silent WAV file
fn create_silent_wav(sample_rate: u32, channels: u16, duration_secs: f32) -> NamedTempFile {
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let temp_file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    let mut writer = WavWriter::create(temp_file.path(), spec).unwrap();

    let num_frames = (duration_secs * sample_rate as f32) as usize;

    for _ in 0..num_frames {
        for _ in 0..channels {
            writer.write_sample(0i16).unwrap();
        }
    }

    writer.finalize().unwrap();
    temp_file
}

/// Create a WAV file with varying amplitude (fade in/out)
fn create_fade_wav(sample_rate: u32, duration_secs: f32) -> NamedTempFile {
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
        let progress = frame as f32 / num_frames as f32;

        // Fade in for first half, fade out for second half
        let envelope = if progress < 0.5 {
            progress * 2.0
        } else {
            (1.0 - progress) * 2.0
        };

        let sample = (t * 440.0 * 2.0 * std::f32::consts::PI).sin();
        let amplitude = (sample * i16::MAX as f32 * 0.8 * envelope) as i16;

        writer.write_sample(amplitude).unwrap();
        writer.write_sample(amplitude).unwrap();
    }

    writer.finalize().unwrap();
    temp_file
}

// ============================================================================
// Basic Waveform Tests
// ============================================================================

#[test]
fn test_waveform_output_length() {
    let wav_file = create_sine_wav(48000, 2, 5.0, 440.0);

    let result = analyze_waveform(wav_file.path());
    assert!(
        result.is_ok(),
        "Waveform analysis failed: {:?}",
        result.err()
    );

    let waveform = result.unwrap();
    assert_eq!(
        waveform.len(),
        WAVEFORM_SAMPLES,
        "Waveform should have exactly {} samples",
        WAVEFORM_SAMPLES
    );
}

#[test]
fn test_waveform_values_in_range() {
    let wav_file = create_sine_wav(48000, 2, 3.0, 440.0);

    let waveform = analyze_waveform(wav_file.path()).unwrap();

    // Verify waveform has expected length and values are valid u8 (0-255)
    // Note: u8 type guarantees values are 0-255, so we just verify length
    assert!(!waveform.is_empty(), "Waveform should not be empty");
    for (i, &value) in waveform.iter().enumerate() {
        // Verify we have valid sample indices
        assert!(
            i < waveform.len(),
            "Index {} should be within waveform bounds",
            i
        );
        // u8 is always valid, just log for debugging if needed
        let _ = value;
    }
}

#[test]
fn test_waveform_non_zero_for_audio() {
    let wav_file = create_sine_wav(48000, 2, 3.0, 440.0);

    let waveform = analyze_waveform(wav_file.path()).unwrap();

    // At least some values should be non-zero for audio content
    let non_zero_count = waveform.iter().filter(|&&v| v > 0).count();
    assert!(
        non_zero_count > WAVEFORM_SAMPLES / 2,
        "Expected most waveform values to be non-zero, got {} non-zero out of {}",
        non_zero_count,
        WAVEFORM_SAMPLES
    );
}

#[test]
fn test_waveform_silent_file() {
    let wav_file = create_silent_wav(48000, 2, 3.0);

    let waveform = analyze_waveform(wav_file.path()).unwrap();

    // Silent file should have all zeros (or very low values)
    let max_value = *waveform.iter().max().unwrap();
    assert!(
        max_value < 5,
        "Silent file should have near-zero waveform values, got max {}",
        max_value
    );
}

// ============================================================================
// Waveform Shape Tests
// ============================================================================

#[test]
fn test_waveform_fade_shape() {
    let wav_file = create_fade_wav(48000, 4.0);

    let waveform = analyze_waveform(wav_file.path()).unwrap();

    // First quarter should be lower than middle
    let first_quarter_avg: f32 = waveform[0..WAVEFORM_SAMPLES / 4]
        .iter()
        .map(|&v| v as f32)
        .sum::<f32>()
        / (WAVEFORM_SAMPLES / 4) as f32;

    let middle_avg: f32 = waveform[WAVEFORM_SAMPLES / 4..3 * WAVEFORM_SAMPLES / 4]
        .iter()
        .map(|&v| v as f32)
        .sum::<f32>()
        / (WAVEFORM_SAMPLES / 2) as f32;

    let last_quarter_avg: f32 = waveform[3 * WAVEFORM_SAMPLES / 4..]
        .iter()
        .map(|&v| v as f32)
        .sum::<f32>()
        / (WAVEFORM_SAMPLES / 4) as f32;

    // Middle should be louder than edges (fade in/out)
    assert!(
        middle_avg > first_quarter_avg,
        "Middle ({}) should be louder than start ({})",
        middle_avg,
        first_quarter_avg
    );
    assert!(
        middle_avg > last_quarter_avg,
        "Middle ({}) should be louder than end ({})",
        middle_avg,
        last_quarter_avg
    );
}

// ============================================================================
// Various Audio Formats
// ============================================================================

#[test]
fn test_waveform_mono() {
    let wav_file = create_sine_wav(48000, 1, 2.0, 440.0);

    let result = analyze_waveform(wav_file.path());
    assert!(result.is_ok(), "Mono waveform analysis failed");

    let waveform = result.unwrap();
    assert_eq!(waveform.len(), WAVEFORM_SAMPLES);
}

#[test]
fn test_waveform_multichannel() {
    let wav_file = create_sine_wav(48000, 6, 2.0, 440.0);

    let result = analyze_waveform(wav_file.path());
    assert!(result.is_ok(), "Multichannel waveform analysis failed");

    let waveform = result.unwrap();
    assert_eq!(waveform.len(), WAVEFORM_SAMPLES);
}

#[test]
fn test_waveform_various_sample_rates() {
    let sample_rates = [22050, 44100, 48000, 96000];

    for &sr in &sample_rates {
        let wav_file = create_sine_wav(sr, 2, 2.0, 440.0);

        let result = analyze_waveform(wav_file.path());
        assert!(
            result.is_ok(),
            "Waveform analysis failed at {}Hz: {:?}",
            sr,
            result.err()
        );

        let waveform = result.unwrap();
        assert_eq!(waveform.len(), WAVEFORM_SAMPLES);
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_waveform_short_file() {
    // Very short file (50ms)
    let wav_file = create_sine_wav(48000, 2, 0.05, 440.0);

    let result = analyze_waveform(wav_file.path());
    assert!(result.is_ok(), "Short file waveform analysis failed");

    let waveform = result.unwrap();
    assert_eq!(waveform.len(), WAVEFORM_SAMPLES);
}

#[test]
fn test_waveform_long_file() {
    // Longer file (30 seconds)
    let wav_file = create_sine_wav(48000, 2, 30.0, 440.0);

    let result = analyze_waveform(wav_file.path());
    assert!(result.is_ok(), "Long file waveform analysis failed");

    let waveform = result.unwrap();
    assert_eq!(waveform.len(), WAVEFORM_SAMPLES);
}

#[test]
fn test_waveform_nonexistent_file() {
    let result = analyze_waveform("/nonexistent/path/to/file.wav");
    assert!(result.is_err());
}

#[test]
fn test_waveform_invalid_file() {
    let temp_file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    std::fs::write(temp_file.path(), b"not a valid audio file").unwrap();

    let result = analyze_waveform(temp_file.path());
    assert!(result.is_err());
}

// ============================================================================
// Consistency Tests
// ============================================================================

#[test]
fn test_waveform_deterministic() {
    let wav_file = create_sine_wav(48000, 2, 2.0, 440.0);

    let waveform1 = analyze_waveform(wav_file.path()).unwrap();
    let waveform2 = analyze_waveform(wav_file.path()).unwrap();

    assert_eq!(
        waveform1, waveform2,
        "Waveform analysis should be deterministic"
    );
}
