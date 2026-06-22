use super::ChannelMeasurement;
use super::read_first_wav_channel_f32;
use crate::recording_types::RecordingResult;
use std::path::Path;

fn write_wav_bytes(
    path: &Path,
    format: u16,
    bits: u16,
    channels: u16,
    sample_rate: u32,
    samples: &[i32],
) {
    let bytes_per_sample = (bits / 8) as usize;
    let _frame_size = bytes_per_sample * channels as usize;
    let data_len = samples.len() * bytes_per_sample;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&format.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(
        &(sample_rate * channels as u32 * bytes_per_sample as u32).to_le_bytes(),
    );
    bytes.extend_from_slice(&(channels * bytes_per_sample as u16).to_le_bytes());
    bytes.extend_from_slice(&bits.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_len as u32).to_le_bytes());

    for &sample in samples {
        let bytes_sample = match bits {
            16 => sample as i16 as i32 as u32,
            24 => (sample & 0x00ff_ffff) as u32,
            32 => sample as u32,
            _ => panic!("unsupported bits"),
        };
        for i in 0..bytes_per_sample {
            bytes.push(((bytes_sample >> (i * 8)) & 0xff) as u8);
        }
    }

    std::fs::write(path, bytes).unwrap();
}

#[test]
fn read_first_wav_channel_handles_pcm16() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pcm16.wav");
    // Stereo: left = max, right = half max
    write_wav_bytes(
        &path,
        1,
        16,
        2,
        48000,
        &[i16::MAX as i32, (i16::MAX / 2) as i32],
    );

    let (samples, sr) = read_first_wav_channel_f32(&path).unwrap();
    assert_eq!(sr, 48000);
    assert_eq!(samples.len(), 1);
    assert!((samples[0] - 1.0).abs() < 1e-5);
}

#[test]
fn read_first_wav_channel_handles_pcm24() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pcm24.wav");
    // 24-bit max value
    let max24 = 8_388_607i32;
    write_wav_bytes(&path, 1, 24, 1, 48000, &[max24]);

    let (samples, sr) = read_first_wav_channel_f32(&path).unwrap();
    assert_eq!(sr, 48000);
    assert_eq!(samples.len(), 1);
    assert!((samples[0] - 1.0).abs() < 1e-5);
}

#[test]
fn read_first_wav_channel_handles_float32() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("float32.wav");
    let sample = 0.5f32;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&40u32.to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&48000u32.to_le_bytes());
    bytes.extend_from_slice(&(48000u32 * 4).to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&32u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&4u32.to_le_bytes());
    bytes.extend_from_slice(&sample.to_le_bytes());

    std::fs::write(&path, bytes).unwrap();

    let (samples, sr) = read_first_wav_channel_f32(&path).unwrap();
    assert_eq!(sr, 48000);
    assert_eq!(samples.len(), 1);
    assert!((samples[0] - 0.5).abs() < 1e-6);
}

#[test]
fn read_first_wav_channel_rejects_non_wav() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("not_a_wav.txt");
    std::fs::write(&path, b"hello world").unwrap();

    let result = read_first_wav_channel_f32(&path);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not a RIFF/WAVE file"));
}

#[test]
fn read_first_wav_channel_rejects_truncated_chunk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("truncated.wav");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&100u32.to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&48000u32.to_le_bytes());
    bytes.extend_from_slice(&(48000u32 * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&50u32.to_le_bytes()); // claims 50 bytes but file ends
    std::fs::write(&path, bytes).unwrap();

    let result = read_first_wav_channel_f32(&path);
    assert!(result.is_err());
}

#[test]
fn compute_lr_slope_returns_none_for_empty_measurements() {
    assert!(super::compute_lr_slope(&[]).is_none());
}

#[test]
fn compute_lr_slope_computes_negative_slope() {
    // LR measurements: magnitude falls with log frequency → negative slope
    let freqs: Vec<f32> = (1..=50).map(|i| 200.0 + i as f32 * 396.0).collect();
    let mags: Vec<f32> = freqs.iter().map(|f| 20.0 - 3.0 * f.log10()).collect();
    let measurements = vec![ChannelMeasurement {
        channel_name: "L".to_string(),
        measurement: RecordingResult {
            channel: 0,
            wav_path: None,
            csv_path: None,
            frequencies: freqs.clone(),
            magnitude_db: mags.clone(),
            phase_deg: vec![0.0; freqs.len()],
            impulse_response: None,
            impulse_time_ms: None,
            excess_group_delay_ms: None,
            thd_percent: None,
            harmonic_distortion_db: None,
            rt60_ms: None,
            clarity_c50_db: None,
            clarity_c80_db: None,
            spectrogram_db: None,
        },
        is_group: false,
        group_drivers: Vec::new(),
        multi_mic_measurements: Vec::new(),
    }];

    let (slope, min, max) = super::compute_lr_slope(&measurements).unwrap();
    assert!(slope < 0.0, "expected negative slope, got {slope}");
    // Recommendation brackets slope by 0.8× and 1.1×; for a negative slope
    // the 1.1× bound is more negative and the 0.8× bound is less negative.
    assert!(max < slope, "max {max} should be below slope {slope}");
    assert!(slope < min, "min {min} should be above slope {slope}");
}
