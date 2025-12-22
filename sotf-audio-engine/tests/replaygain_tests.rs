//! ReplayGain Analysis Integration Tests
//!
//! Tests for the ReplayGain analysis module including:
//! - Loudness measurement (EBU R128)
//! - Peak detection
//! - Gain calculation
//! - Various audio formats and configurations

use hound::{SampleFormat, WavSpec, WavWriter};
use sotf_audio::replaygain::{ReplayGainInfo, analyze_file};
use tempfile::NamedTempFile;

// ============================================================================
// Test Helpers
// ============================================================================

/// Create a test WAV file with a sine wave at specified amplitude
fn create_wav_with_amplitude(
    sample_rate: u32,
    channels: u16,
    duration_secs: f32,
    frequency: f32,
    amplitude: f32, // 0.0 to 1.0
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
        let scaled = (sample * i16::MAX as f32 * amplitude) as i16;

        for _ in 0..channels {
            writer.write_sample(scaled).unwrap();
        }
    }

    writer.finalize().unwrap();
    temp_file
}

/// Create a silent WAV file
fn create_silent_wav(sample_rate: u32, channels: u16, duration_secs: f32) -> NamedTempFile {
    create_wav_with_amplitude(sample_rate, channels, duration_secs, 440.0, 0.0)
}

/// Create a loud WAV file (near full scale)
fn create_loud_wav(sample_rate: u32, channels: u16, duration_secs: f32) -> NamedTempFile {
    create_wav_with_amplitude(sample_rate, channels, duration_secs, 440.0, 0.95)
}

/// Create a quiet WAV file
fn create_quiet_wav(sample_rate: u32, channels: u16, duration_secs: f32) -> NamedTempFile {
    create_wav_with_amplitude(sample_rate, channels, duration_secs, 440.0, 0.1)
}

// ============================================================================
// Basic ReplayGain Tests
// ============================================================================

#[test]
fn test_replaygain_returns_valid_info() {
    let wav_file = create_wav_with_amplitude(48000, 2, 3.0, 440.0, 0.5);

    let result = analyze_file(wav_file.path());
    assert!(
        result.is_ok(),
        "ReplayGain analysis failed: {:?}",
        result.err()
    );

    let info = result.unwrap();

    // Gain should be a finite number
    assert!(info.gain.is_finite(), "Gain should be finite");

    // Peak should be positive and reasonable
    assert!(info.peak >= 0.0, "Peak should be non-negative");
    assert!(info.peak <= 2.0, "Peak should be reasonable (< 2.0)");
}

#[test]
fn test_replaygain_info_serialization() {
    let info = ReplayGainInfo {
        gain: -6.5,
        peak: 0.95,
    };

    // Test JSON serialization
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("-6.5"));
    assert!(json.contains("0.95"));

    // Test deserialization
    let deserialized: ReplayGainInfo = serde_json::from_str(&json).unwrap();
    assert!((deserialized.gain - info.gain).abs() < 0.001);
    assert!((deserialized.peak - info.peak).abs() < 0.001);
}

// ============================================================================
// Loudness Level Tests
// ============================================================================

#[test]
fn test_replaygain_loud_vs_quiet() {
    let loud_file = create_loud_wav(48000, 2, 3.0);
    let quiet_file = create_quiet_wav(48000, 2, 3.0);

    let loud_info = analyze_file(loud_file.path()).unwrap();
    let quiet_info = analyze_file(quiet_file.path()).unwrap();

    // Loud file should need more negative gain (reduce volume)
    // Quiet file should need more positive gain (increase volume)
    assert!(
        loud_info.gain < quiet_info.gain,
        "Loud file gain ({}) should be less than quiet file gain ({})",
        loud_info.gain,
        quiet_info.gain
    );
}

#[test]
fn test_replaygain_peak_detection() {
    let loud_file = create_loud_wav(48000, 2, 2.0);
    let quiet_file = create_quiet_wav(48000, 2, 2.0);

    let loud_info = analyze_file(loud_file.path()).unwrap();
    let quiet_info = analyze_file(quiet_file.path()).unwrap();

    // Loud file should have higher peak
    assert!(
        loud_info.peak > quiet_info.peak,
        "Loud file peak ({}) should be greater than quiet file peak ({})",
        loud_info.peak,
        quiet_info.peak
    );

    // Loud file peak should be close to 0.95 (our amplitude)
    assert!(
        loud_info.peak > 0.8 && loud_info.peak < 1.1,
        "Loud file peak should be ~0.95, got {}",
        loud_info.peak
    );

    // Quiet file peak should be close to 0.1 (our amplitude)
    assert!(
        quiet_info.peak > 0.05 && quiet_info.peak < 0.2,
        "Quiet file peak should be ~0.1, got {}",
        quiet_info.peak
    );
}

// ============================================================================
// Various Audio Configurations
// ============================================================================

#[test]
fn test_replaygain_mono() {
    let wav_file = create_wav_with_amplitude(48000, 1, 2.0, 440.0, 0.5);

    let result = analyze_file(wav_file.path());
    assert!(result.is_ok(), "Mono ReplayGain analysis failed");

    let info = result.unwrap();
    assert!(info.gain.is_finite());
    assert!(info.peak > 0.0);
}

#[test]
fn test_replaygain_stereo() {
    let wav_file = create_wav_with_amplitude(48000, 2, 2.0, 440.0, 0.5);

    let result = analyze_file(wav_file.path());
    assert!(result.is_ok(), "Stereo ReplayGain analysis failed");

    let info = result.unwrap();
    assert!(info.gain.is_finite());
    assert!(info.peak > 0.0);
}

#[test]
fn test_replaygain_multichannel() {
    let wav_file = create_wav_with_amplitude(48000, 6, 2.0, 440.0, 0.5);

    let result = analyze_file(wav_file.path());
    assert!(result.is_ok(), "Multichannel ReplayGain analysis failed");

    let info = result.unwrap();
    assert!(info.gain.is_finite());
    assert!(info.peak > 0.0);
}

#[test]
fn test_replaygain_various_sample_rates() {
    let sample_rates = [44100, 48000, 96000];

    for &sr in &sample_rates {
        let wav_file = create_wav_with_amplitude(sr, 2, 2.0, 440.0, 0.5);

        let result = analyze_file(wav_file.path());
        assert!(
            result.is_ok(),
            "ReplayGain analysis failed at {}Hz: {:?}",
            sr,
            result.err()
        );

        let info = result.unwrap();
        assert!(info.gain.is_finite(), "Gain should be finite at {}Hz", sr);
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_replaygain_silent_file() {
    let wav_file = create_silent_wav(48000, 2, 2.0);

    let result = analyze_file(wav_file.path());
    // Silent files may return an error or very high gain
    // Either is acceptable behavior
    if let Ok(info) = result {
        // If it succeeds, gain should be very high (need to boost a lot)
        // or the peak should be very low
        assert!(
            info.peak < 0.01 || info.gain > 10.0,
            "Silent file should have very low peak or very high gain"
        );
    }
}

#[test]
fn test_replaygain_short_file() {
    // Very short file (100ms) - EBU R128 requires at least 400ms for accurate measurement
    // so we use a slightly longer file
    let wav_file = create_wav_with_amplitude(48000, 2, 0.5, 440.0, 0.5);

    let result = analyze_file(wav_file.path());
    // Short files may not have enough data for accurate loudness measurement
    // but should not crash
    if let Ok(info) = result {
        // Gain may be infinite for very short files, which is acceptable
        // Just verify peak is valid
        assert!(info.peak >= 0.0, "Peak should be non-negative");
    }
}

#[test]
fn test_replaygain_nonexistent_file() {
    let result = analyze_file("/nonexistent/path/to/file.wav");
    assert!(result.is_err());
}

#[test]
fn test_replaygain_invalid_file() {
    let temp_file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    std::fs::write(temp_file.path(), b"not a valid audio file").unwrap();

    let result = analyze_file(temp_file.path());
    assert!(result.is_err());
}

// ============================================================================
// Consistency Tests
// ============================================================================

#[test]
fn test_replaygain_deterministic() {
    let wav_file = create_wav_with_amplitude(48000, 2, 2.0, 440.0, 0.5);

    let info1 = analyze_file(wav_file.path()).unwrap();
    let info2 = analyze_file(wav_file.path()).unwrap();

    assert!(
        (info1.gain - info2.gain).abs() < 0.001,
        "ReplayGain should be deterministic"
    );
    assert!(
        (info1.peak - info2.peak).abs() < 0.001,
        "Peak should be deterministic"
    );
}

#[test]
fn test_replaygain_same_content_same_result() {
    // Create two identical files
    let wav_file1 = create_wav_with_amplitude(48000, 2, 2.0, 440.0, 0.5);
    let wav_file2 = create_wav_with_amplitude(48000, 2, 2.0, 440.0, 0.5);

    let info1 = analyze_file(wav_file1.path()).unwrap();
    let info2 = analyze_file(wav_file2.path()).unwrap();

    // Same content should produce same results
    assert!(
        (info1.gain - info2.gain).abs() < 0.1,
        "Same content should produce similar gain: {} vs {}",
        info1.gain,
        info2.gain
    );
    assert!(
        (info1.peak - info2.peak).abs() < 0.01,
        "Same content should produce same peak: {} vs {}",
        info1.peak,
        info2.peak
    );
}
