//! Time alignment utilities for speaker measurements
//!
//! This module provides functions to analyze WAV files and determine arrival times
//! for time-aligning multiple speakers in a room EQ setup.

use hound::WavReader;
use math_audio_dsp::analysis::cross_correlate_envelope;
use std::path::Path;

/// Result of arrival time analysis
#[derive(Debug, Clone)]
pub struct ArrivalTimeResult {
    /// Arrival time in samples from the start of the recording
    pub arrival_samples: usize,
    /// Arrival time in milliseconds
    pub arrival_ms: f64,
    /// Sample rate of the WAV file
    pub sample_rate: u32,
    /// Peak amplitude (for validation)
    pub peak_amplitude: f32,
}

/// Find the arrival time (signal onset) in a WAV file
///
/// This function loads a WAV file and finds the first point where the signal
/// exceeds a threshold above the noise floor, representing the arrival of sound.
/// Works with both impulse responses and log sweep recordings.
///
/// # Arguments
/// * `wav_path` - Path to the WAV file (impulse response or sweep recording)
/// * `threshold_db` - Threshold above noise floor to consider as "arrival" (default: -40 dB)
///
/// # Returns
/// * `ArrivalTimeResult` containing arrival time in samples and milliseconds
pub fn find_arrival_time(
    wav_path: &Path,
    threshold_db: Option<f64>,
) -> Result<ArrivalTimeResult, String> {
    let threshold_db = threshold_db.unwrap_or(-40.0);

    // Load WAV file
    let mut reader = WavReader::open(wav_path)
        .map_err(|e| format!("Failed to open WAV file {:?}: {}", wav_path, e))?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels as usize;

    // Read samples (convert to f32, take first channel if stereo)
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample;
            let max_val = (1u32 << (bits - 1)) as f32;
            reader
                .samples::<i32>()
                .enumerate()
                .filter(|(i, _)| i % channels == 0) // Take first channel only
                .map(|(_, s)| s.map(|v| v as f32 / max_val))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Failed to read samples: {}", e))?
        }
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .enumerate()
            .filter(|(i, _)| i % channels == 0)
            .map(|(_, s)| s)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to read samples: {}", e))?,
    };

    if samples.is_empty() {
        return Err("WAV file contains no samples".to_string());
    }

    // Find the peak (maximum absolute value) for reference
    let peak_amplitude = samples.iter().map(|&s| s.abs()).fold(0.0_f32, f32::max);

    if peak_amplitude < 1e-6 {
        return Err("Signal appears to be silent (peak amplitude < -120 dB)".to_string());
    }

    // Estimate noise floor from the first 10ms of silence (or first 1% of signal, whichever is smaller)
    let noise_samples = (sample_rate as usize / 100)
        .min(samples.len() / 100)
        .max(10);
    let noise_floor: f32 = samples[..noise_samples]
        .iter()
        .map(|&s| s.abs())
        .fold(0.0_f32, f32::max);

    // Use the larger of: noise_floor * 10 (20dB above noise) or peak * threshold_db
    let threshold_from_peak = peak_amplitude * 10.0_f32.powf(threshold_db as f32 / 20.0);
    let threshold_from_noise = noise_floor * 10.0; // 20 dB above noise floor
    let threshold_linear = threshold_from_peak.max(threshold_from_noise).max(1e-5);

    // Find the first sample that exceeds the threshold (signal onset)
    // This works for both impulse responses and sweep recordings
    let mut arrival_idx = 0;
    for (i, &sample) in samples.iter().enumerate() {
        if sample.abs() >= threshold_linear {
            arrival_idx = i;
            break;
        }
    }

    let arrival_ms = arrival_idx as f64 * 1000.0 / sample_rate as f64;

    Ok(ArrivalTimeResult {
        arrival_samples: arrival_idx,
        arrival_ms,
        sample_rate,
        peak_amplitude,
    })
}

/// Estimate speaker propagation delay from a frequency-domain phase measurement.
///
/// Uses linear regression on the unwrapped phase in the [min_freq, max_freq] band:
///   φ(f) ≈ φ₀ - 2π·τ·f  →  τ = -slope / (2π)
///
/// Returns the estimated arrival time in milliseconds, or None if phase data
/// is absent, no points fall in the band, or the estimate is implausible.
pub fn estimate_arrival_from_phase(
    curve: &crate::Curve,
    min_freq: f64,
    max_freq: f64,
) -> Option<f64> {
    use std::f64::consts::PI;

    let phase = curve.phase.as_ref()?;

    // Unwrap phase to remove discontinuities
    let unwrapped = super::phase_utils::unwrap_phase_degrees(phase);

    // Filter to the [min_freq, max_freq] band
    let points: Vec<(f64, f64)> = curve
        .freq
        .iter()
        .zip(unwrapped.iter())
        .filter(|&(&f, _)| f >= min_freq && f <= max_freq)
        .map(|(&f, &p)| (f, p))
        .collect();

    if points.len() < 5 {
        return None;
    }

    // Linear regression in radians: φ_rad = φ₀ - 2π·τ·f  →  slope = dφ/df
    let n = points.len() as f64;
    let sum_f: f64 = points.iter().map(|(f, _)| f).sum();
    let sum_phi: f64 = points.iter().map(|(_, p)| p.to_radians()).sum();
    let sum_f2: f64 = points.iter().map(|(f, _)| f * f).sum();
    let sum_f_phi: f64 = points.iter().map(|(f, p)| f * p.to_radians()).sum();

    let denom = n * sum_f2 - sum_f * sum_f;
    if denom.abs() < 1e-12 {
        return None;
    }

    let slope = (n * sum_f_phi - sum_f * sum_phi) / denom;

    // τ = -slope / (2π), convert seconds → milliseconds
    let delay_ms = -slope / (2.0 * PI) * 1000.0;

    // Sanity check: plausible acoustic propagation time (0–500 ms)
    if delay_ms > 0.0 && delay_ms < 500.0 {
        Some(delay_ms)
    } else {
        None
    }
}

/// Calculate time alignment delays for multiple channels
///
/// Given arrival times for multiple channels, calculates the delays needed
/// to align all channels to the slowest one (longest arrival time).
///
/// # Arguments
/// * `arrival_times` - Map of channel name to arrival time in milliseconds
///
/// # Returns
/// * Map of channel name to delay in milliseconds (to be added)
pub fn calculate_alignment_delays(
    arrival_times: &std::collections::HashMap<String, f64>,
) -> std::collections::HashMap<String, f64> {
    if arrival_times.is_empty() {
        return std::collections::HashMap::new();
    }

    // Find the maximum (slowest) arrival time - this is our reference
    let max_arrival = arrival_times
        .values()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);

    // Calculate delays: delay = max_arrival - channel_arrival
    arrival_times
        .iter()
        .map(|(name, &arrival)| (name.clone(), max_arrival - arrival))
        .collect()
}

/// Result of probe-based delay detection for one channel.
///
/// More accurate than threshold-based detection (`ArrivalTimeResult`) because
/// the matched filter (cross-correlation + analytic envelope) rejects noise
/// and provides sub-sample precision.
#[derive(Debug, Clone)]
pub struct ProbeDelayResult {
    /// Arrival time in milliseconds (sub-sample precision)
    pub arrival_ms: f64,
    /// Arrival time in samples (integer)
    pub arrival_samples: usize,
    /// Relative gain (linear) derived from envelope peak
    pub gain_linear: f64,
    /// Gain in dB relative to the probe's self-correlation peak
    pub gain_db: f64,
    /// Signal-to-noise ratio of the detection (peak / median envelope, in dB)
    pub detection_snr_db: f64,
}

/// Detect arrival time and gain for a single channel using a narrowband probe.
///
/// Cross-correlates the recorded signal with the known probe, computes the
/// analytic envelope, and finds the peak. The peak position is the arrival
/// time; the peak value (normalized against probe autocorrelation) gives gain.
pub fn detect_delay_with_probe(
    probe: &[f32],
    recorded: &[f32],
    sample_rate: u32,
) -> Result<ProbeDelayResult, String> {
    // Compute probe autocorrelation peak for gain normalization
    let auto_result = cross_correlate_envelope(probe, probe, sample_rate)?;
    let auto_peak = auto_result.peak_value as f64;

    detect_delay_with_probe_inner(probe, recorded, sample_rate, auto_peak)
}

/// Inner implementation that accepts a precomputed autocorrelation peak,
/// avoiding redundant FFT computation when called in a loop.
fn detect_delay_with_probe_inner(
    probe: &[f32],
    recorded: &[f32],
    sample_rate: u32,
    auto_peak: f64,
) -> Result<ProbeDelayResult, String> {
    let result = cross_correlate_envelope(probe, recorded, sample_rate)?;

    let gain_linear = if auto_peak > 1e-10 {
        result.peak_value as f64 / auto_peak
    } else {
        0.0
    };

    let gain_db = if gain_linear > 1e-10 {
        20.0 * gain_linear.log10()
    } else {
        -120.0
    };

    // SNR: peak / median of envelope
    let mut sorted_env = result.envelope.to_vec();
    sorted_env.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = if sorted_env.is_empty() {
        1e-10
    } else {
        sorted_env[sorted_env.len() / 2].max(1e-10) as f64
    };
    let detection_snr_db = 20.0 * (result.peak_value as f64 / median).log10();

    Ok(ProbeDelayResult {
        arrival_ms: result.arrival_ms,
        arrival_samples: result.peak_sample,
        gain_linear,
        gain_db,
        detection_snr_db,
    })
}

/// Detect delays for multiple channels from a single sequential recording.
///
/// The recording contains probes played one at a time on each channel,
/// separated by silence gaps. This function extracts each channel's
/// segment and runs probe detection on it.
///
/// # Arguments
/// * `probe` - The narrowband probe used for all channels
/// * `recorded` - Full recording containing all channels sequentially
/// * `channel_offsets` - Start sample of each channel's probe in the playback signal
/// * `segment_length` - Expected length of each probe+silence segment in samples
/// * `sample_rate` - Sample rate
pub fn detect_delays_multi_channel(
    probe: &[f32],
    recorded: &[f32],
    channel_offsets: &[usize],
    segment_length: usize,
    sample_rate: u32,
) -> Result<Vec<ProbeDelayResult>, String> {
    let mut results = Vec::with_capacity(channel_offsets.len());

    // Precompute autocorrelation once (same probe for all channels)
    let auto_result = cross_correlate_envelope(probe, probe, sample_rate)?;
    let auto_peak = auto_result.peak_value as f64;

    for (i, &offset) in channel_offsets.iter().enumerate() {
        // Bounds check before arithmetic to avoid overflow
        if offset >= recorded.len() {
            return Err(format!(
                "Channel {} offset {} exceeds recording length {}",
                i,
                offset,
                recorded.len()
            ));
        }
        let end = (offset.saturating_add(segment_length)).min(recorded.len());

        let segment = &recorded[offset..end];
        let channel_result =
            detect_delay_with_probe_inner(probe, segment, sample_rate, auto_peak)?;

        log::debug!(
            "[detect_delays_multi_channel] Ch {}: arrival={:.3}ms, gain={:.1}dB, SNR={:.1}dB",
            i,
            channel_result.arrival_ms,
            channel_result.gain_db,
            channel_result.detection_snr_db
        );

        results.push(channel_result);
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_alignment_delays() {
        let mut arrivals = std::collections::HashMap::new();
        arrivals.insert("L".to_string(), 10.0);
        arrivals.insert("R".to_string(), 12.0);
        arrivals.insert("C".to_string(), 8.0);

        let delays = calculate_alignment_delays(&arrivals);

        // R is slowest (12ms), so it gets 0 delay
        // L needs 2ms delay to match R
        // C needs 4ms delay to match R
        assert!((delays["R"] - 0.0).abs() < 0.001);
        assert!((delays["L"] - 2.0).abs() < 0.001);
        assert!((delays["C"] - 4.0).abs() < 0.001);
    }

    #[test]
    fn test_estimate_arrival_from_phase() {
        use ndarray::Array1;

        // Synthesize φ(f) = -2π·τ·f for τ = 5 ms
        let tau_ms = 5.0_f64;
        let tau_s = tau_ms / 1000.0;
        let freqs: Vec<f64> = (20..=2000).step_by(10).map(|f| f as f64).collect();
        let phase_deg: Vec<f64> = freqs.iter().map(|&f| -360.0 * f * tau_s).collect();

        let curve = crate::Curve {
            freq: Array1::from_vec(freqs),
            spl: Array1::zeros(phase_deg.len()),
            phase: Some(Array1::from_vec(phase_deg)),
        };

        let estimated = estimate_arrival_from_phase(&curve, 200.0, 2000.0);
        assert!(
            estimated.is_some(),
            "Should recover arrival time from phase"
        );
        let estimated = estimated.unwrap();
        assert!(
            (estimated - tau_ms).abs() < 0.1,
            "Expected ~{} ms, got {} ms",
            tau_ms,
            estimated
        );
    }

    #[test]
    fn test_estimate_arrival_from_phase_no_phase() {
        use ndarray::Array1;

        let curve = crate::Curve {
            freq: Array1::linspace(20.0, 2000.0, 100),
            spl: Array1::zeros(100),
            phase: None,
        };
        assert!(estimate_arrival_from_phase(&curve, 200.0, 2000.0).is_none());
    }

    #[test]
    fn test_calculate_alignment_delays_empty() {
        let arrivals = std::collections::HashMap::new();
        let delays = calculate_alignment_delays(&arrivals);
        assert!(delays.is_empty());
    }

    #[test]
    fn test_alignment_delays_three_speakers() {
        // Arrivals: [0, 2, 5] ms → delays: [5, 3, 0] ms
        let mut arrivals = std::collections::HashMap::new();
        arrivals.insert("A".to_string(), 0.0);
        arrivals.insert("B".to_string(), 2.0);
        arrivals.insert("C".to_string(), 5.0);

        let delays = calculate_alignment_delays(&arrivals);

        assert!(
            (delays["A"] - 5.0).abs() < 0.001,
            "A should get 5ms delay, got {}",
            delays["A"]
        );
        assert!(
            (delays["B"] - 3.0).abs() < 0.001,
            "B should get 3ms delay, got {}",
            delays["B"]
        );
        assert!(
            (delays["C"] - 0.0).abs() < 0.001,
            "C should get 0ms delay, got {}",
            delays["C"]
        );
    }

    #[test]
    fn test_estimate_arrival_linear_phase() {
        use ndarray::Array1;

        // Construct a curve with linear phase corresponding to 3ms delay
        let tau_ms = 3.0;
        let tau_s = tau_ms / 1000.0;
        let freqs: Vec<f64> = (100..=5000).step_by(20).map(|f| f as f64).collect();
        let phase_deg: Vec<f64> = freqs.iter().map(|&f| -360.0 * f * tau_s).collect();

        let curve = crate::Curve {
            freq: Array1::from_vec(freqs),
            spl: Array1::zeros(phase_deg.len()),
            phase: Some(Array1::from_vec(phase_deg)),
        };

        let estimated = estimate_arrival_from_phase(&curve, 200.0, 4000.0);
        assert!(
            estimated.is_some(),
            "Should recover arrival time from linear phase"
        );
        let estimated = estimated.unwrap();
        assert!(
            (estimated - tau_ms).abs() < 0.1,
            "Expected ~{} ms, got {} ms (error {:.3} ms)",
            tau_ms,
            estimated,
            (estimated - tau_ms).abs()
        );
    }

    #[test]
    fn test_detect_delay_with_probe() {
        let sr = 48000_u32;
        let n = 4096;
        let probe = math_audio_dsp::signals::gen_narrowband_probe(n, sr, 0.5, 42, 800.0, 2000.0);

        // Simulate: delay 480 samples (10ms), attenuate by 0.4
        let delay = 480_usize;
        let atten = 0.4_f32;
        let mut recorded = vec![0.0_f32; n + delay + 500];
        for (i, &s) in probe.iter().enumerate() {
            recorded[i + delay] += s * atten;
        }

        let result = detect_delay_with_probe(&probe, &recorded, sr).unwrap();

        assert!(
            (result.arrival_ms - 10.0).abs() < 0.2,
            "Expected ~10ms arrival, got {:.3}ms",
            result.arrival_ms
        );
        assert!(
            result.detection_snr_db > 10.0,
            "SNR should be high for clean signal, got {:.1}dB",
            result.detection_snr_db
        );
        // Gain should be close to the applied attenuation
        let expected_gain_db = 20.0 * (atten as f64).log10(); // -7.96 dB
        assert!(
            (result.gain_db - expected_gain_db).abs() < 3.0,
            "Expected gain ~{:.1}dB, got {:.1}dB",
            expected_gain_db,
            result.gain_db
        );
    }

    #[test]
    fn test_detect_delays_multi_channel() {
        let sr = 48000_u32;
        let n = 2048;
        let probe = math_audio_dsp::signals::gen_narrowband_probe(n, sr, 0.5, 42, 800.0, 2000.0);

        // Build a sequential recording with 3 channels at different delays
        let segment_len = n + 1000; // probe + some room tail
        let silence_len = 1000;
        let delays = [240_usize, 480, 120]; // 5ms, 10ms, 2.5ms
        let attens = [0.5_f32, 0.3, 0.7];

        let total_len = delays.len() * (segment_len + silence_len) + silence_len;
        let mut recorded = vec![0.0_f32; total_len];
        let mut offsets = Vec::new();

        for (ch, (&d, &a)) in delays.iter().zip(attens.iter()).enumerate() {
            let offset = silence_len + ch * (segment_len + silence_len);
            offsets.push(offset);
            for (i, &s) in probe.iter().enumerate() {
                let idx = offset + d + i;
                if idx < recorded.len() {
                    recorded[idx] += s * a;
                }
            }
        }

        let results =
            detect_delays_multi_channel(&probe, &recorded, &offsets, segment_len, sr).unwrap();

        assert_eq!(results.len(), 3);

        let expected_ms = [5.0, 10.0, 2.5];
        for (i, (result, &expected)) in results.iter().zip(expected_ms.iter()).enumerate() {
            assert!(
                (result.arrival_ms - expected).abs() < 0.5,
                "Channel {}: expected ~{:.1}ms, got {:.3}ms",
                i,
                expected,
                result.arrival_ms
            );
        }
    }
}
