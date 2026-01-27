//! Time alignment utilities for speaker measurements
//!
//! This module provides functions to analyze WAV files and determine arrival times
//! for time-aligning multiple speakers in a room EQ setup.

use hound::WavReader;
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
pub fn find_arrival_time(wav_path: &Path, threshold_db: Option<f64>) -> Result<ArrivalTimeResult, String> {
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
        hound::SampleFormat::Float => {
            reader
                .samples::<f32>()
                .enumerate()
                .filter(|(i, _)| i % channels == 0)
                .map(|(_, s)| s)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Failed to read samples: {}", e))?
        }
    };

    if samples.is_empty() {
        return Err("WAV file contains no samples".to_string());
    }

    // Find the peak (maximum absolute value) for reference
    let peak_amplitude = samples
        .iter()
        .map(|&s| s.abs())
        .fold(0.0_f32, f32::max);

    if peak_amplitude < 1e-6 {
        return Err("Signal appears to be silent (peak amplitude < -120 dB)".to_string());
    }

    // Estimate noise floor from the first 10ms of silence (or first 1% of signal, whichever is smaller)
    let noise_samples = (sample_rate as usize / 100).min(samples.len() / 100).max(10);
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
    fn test_calculate_alignment_delays_empty() {
        let arrivals = std::collections::HashMap::new();
        let delays = calculate_alignment_delays(&arrivals);
        assert!(delays.is_empty());
    }
}
