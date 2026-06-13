//! Deterministic audio signal generators and WAV fixture helpers.

use hound::{WavReader, WavSpec, WavWriter, SampleFormat};
use std::f32::consts::PI;
use std::path::Path;

/// Generate a mono sine wave at the given frequency.
pub fn sine(samples: usize, sample_rate: u32, freq_hz: f32) -> Vec<f32> {
    (0..samples)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            (t * freq_hz * 2.0 * PI).sin()
        })
        .collect()
}

/// Generate a mono logarithmic sine sweep from `freq_start` to `freq_end`.
pub fn log_sweep(samples: usize, sample_rate: u32, freq_start: f32, freq_end: f32) -> Vec<f32> {
    let dt = 1.0 / sample_rate as f32;
    let mut phase = 0.0f32;
    let log_ratio = (freq_end / freq_start).ln();
    (0..samples)
        .map(|i| {
            let t = i as f32 / samples as f32;
            let freq = freq_start * (log_ratio * t).exp();
            phase += 2.0 * PI * freq * dt;
            phase.sin()
        })
        .collect()
}

/// Generate a mono impulse (delta) signal.
pub fn impulse(samples: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; samples];
    if !out.is_empty() {
        out[0] = 1.0;
    }
    out
}

/// Generate silence.
pub fn silence(samples: usize) -> Vec<f32> {
    vec![0.0f32; samples]
}

/// Generate white noise with a deterministic seed.
pub fn white_noise(samples: usize, seed: u64) -> Vec<f32> {
    use rand::SeedableRng;
    use rand::distr::{Distribution, Uniform};
    use rand_chacha::ChaCha8Rng;

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let dist = Uniform::new_inclusive(-1.0f32, 1.0).unwrap();
    (0..samples).map(|_| dist.sample(&mut rng)).collect()
}

/// Interleave `channels` mono signals into a planar `[L, R, L, R, ...]` buffer.
pub fn interleave(channels: &[Vec<f32>]) -> Vec<f32> {
    if channels.is_empty() {
        return Vec::new();
    }
    let frames = channels[0].len();
    debug_assert!(channels.iter().all(|c| c.len() == frames));
    let mut out = Vec::with_capacity(frames * channels.len());
    for frame in 0..frames {
        for ch in channels {
            out.push(ch[frame]);
        }
    }
    out
}

/// Compute RMS of a buffer in dBFS (full scale).
pub fn rms_dbfs(buffer: &[f32]) -> f32 {
    if buffer.is_empty() {
        return f32::NEG_INFINITY;
    }
    let sum_sq: f32 = buffer.iter().map(|s| s * s).sum();
    let rms = (sum_sq / buffer.len() as f32).sqrt();
    if rms <= 0.0 {
        f32::NEG_INFINITY
    } else {
        20.0 * rms.log10()
    }
}

/// Compute peak amplitude in dBFS.
pub fn peak_dbfs(buffer: &[f32]) -> f32 {
    let peak = buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    if peak <= 0.0 {
        f32::NEG_INFINITY
    } else {
        20.0 * peak.log10()
    }
}

/// Write a mono or multi-channel WAV file from an interleaved f32 buffer.
pub fn write_wav<P: AsRef<Path>>(
    path: P,
    sample_rate: u32,
    channels: u16,
    interleaved: &[f32],
) -> Result<(), hound::Error> {
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    let mut writer = WavWriter::create(path, spec)?;
    for &sample in interleaved {
        writer.write_sample(sample)?;
    }
    writer.finalize()
}

/// Read the first channel of a WAV file as f32 samples.
pub fn read_first_channel_f32<P: AsRef<Path>>(path: P) -> Result<(u32, u16, Vec<f32>), hound::Error> {
    let reader = WavReader::open(path)?;
    let spec = reader.spec();
    let channels = spec.channels as usize;
    let sample_rate = spec.sample_rate;
    let bits = spec.bits_per_sample;
    let format = spec.sample_format;

    let samples: Vec<f32> = match format {
        SampleFormat::Float => reader
            .into_samples::<f32>()
            .filter_map(Result::ok)
            .step_by(channels)
            .collect(),
        SampleFormat::Int => {
            let max = ((1u64 << (bits - 1)) as f32) - 1.0;
            reader
                .into_samples::<i32>()
                .filter_map(Result::ok)
                .step_by(channels)
                .map(|s| s as f32 / max)
                .collect()
        }
    };

    Ok((sample_rate, spec.channels, samples))
}

/// Create a temporary WAV file containing a sine wave and return its path.
pub fn temp_sine_wav(
    duration_secs: f32,
    sample_rate: u32,
    channels: u16,
    freq_hz: f32,
) -> Result<(tempfile::NamedTempFile, Vec<f32>), Box<dyn std::error::Error>> {
    let samples = (duration_secs * sample_rate as f32) as usize;
    let mono = sine(samples, sample_rate, freq_hz);
    let interleaved: Vec<f32> = (0..samples)
        .flat_map(|i| std::iter::repeat_n(mono[i], channels as usize))
        .collect();

    let temp = tempfile::Builder::new().suffix(".wav").tempfile()?;
    write_wav(temp.path(), sample_rate, channels, &interleaved)?;
    Ok((temp, mono))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_has_expected_peak() {
        let buf = sine(1024, 48000, 1000.0);
        assert!(peak_dbfs(&buf).abs() < 0.01);
    }

    #[test]
    fn silence_has_neg_inf_db() {
        assert_eq!(peak_dbfs(&silence(64)), f32::NEG_INFINITY);
        assert_eq!(rms_dbfs(&silence(64)), f32::NEG_INFINITY);
    }

    #[test]
    fn white_noise_is_deterministic() {
        let a = white_noise(100, 42);
        let b = white_noise(100, 42);
        assert_eq!(a, b);
    }

    #[test]
    fn round_trip_wav_first_channel() {
        let (temp, original) = temp_sine_wav(0.1, 48000, 2, 440.0).unwrap();
        let (sr, ch, read) = read_first_channel_f32(temp.path()).unwrap();
        assert_eq!(sr, 48000);
        assert_eq!(ch, 2);
        assert_eq!(read.len(), original.len());
        let max_err: f32 = read
            .iter()
            .zip(original.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(max_err < 1e-5, "max_err={}", max_err);
    }
}
