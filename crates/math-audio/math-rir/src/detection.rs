use crate::config::SsirConfig;

/// A detected reflection candidate before segmentation.
#[derive(Debug, Clone)]
pub(crate) struct DetectedReflection {
    /// Sample index of the reflection's peak (TOA)
    pub toa_sample: usize,
    /// Peak energy (squared amplitude) at the TOA
    pub peak_energy: f64,
    /// DOA unit vector, if available from multi-channel input
    pub doa: Option<[f32; 3]>,
}

/// Find the direct sound arrival time in a RIR.
///
/// Implements Algorithm 1 from Pawlak & Lee (2026):
/// 1. Compute log magnitude: L_x = 20 * log10(|x|)
/// 2. Find peaks with minimum spacing delta_min
/// 3. Direct sound = first peak within 11 dB of the global maximum
pub(crate) fn find_direct_sound_toa(rir: &[f32], config: &SsirConfig) -> Option<usize> {
    if rir.is_empty() {
        return None;
    }

    let min_distance = config.min_peak_distance_samples();

    // Step 1: Compute log magnitude (20 * log10(|x|))
    let log_mag: Vec<f64> = rir
        .iter()
        .map(|&x| {
            let abs = (x as f64).abs().max(1e-20); // avoid log(0)
            20.0 * abs.log10()
        })
        .collect();

    // Step 2: Find peaks in log magnitude with minimum distance
    let peaks = find_peaks_with_min_distance(&log_mag, min_distance);
    if peaks.is_empty() {
        return None;
    }

    // Step 3: Find the global maximum of log magnitude
    let global_max = peaks
        .iter()
        .map(|&i| log_mag[i])
        .fold(f64::NEG_INFINITY, f64::max);

    // Step 4: Direct sound = first peak within 11 dB of global max
    let threshold = global_max - 11.0;
    peaks.into_iter().find(|&i| log_mag[i] >= threshold)
}

/// Detect early reflections using Local Energy Ratio (LER).
///
/// The RIR is divided into consecutive windows of `ler_window_ms` length.
/// Within each window, the median energy is computed. A reflection is detected
/// at the sample with maximum energy if that energy exceeds `energy_threshold`
/// times the median.
///
/// Detections within the direct sound window are discarded.
/// Consecutive pairs that are too close in both DOA and TOA are merged.
pub(crate) fn detect_reflections(
    rir: &[f32],
    direct_sound_toa: usize,
    doa_vectors: Option<&[[f32; 3]]>,
    config: &SsirConfig,
) -> Vec<DetectedReflection> {
    let window_len = config.ler_window_samples();
    let mixing_time = config.mixing_time_samples().min(rir.len());
    let (ds_pre, ds_post) = config.direct_sound_window_samples();

    // Direct sound exclusion zone
    let ds_start = direct_sound_toa.saturating_sub(ds_pre);
    let ds_end = (direct_sound_toa + ds_post).min(rir.len());

    // Number of analysis windows up to mixing time
    let num_windows = if window_len > 0 {
        mixing_time.div_ceil(window_len)
    } else {
        return Vec::new();
    };

    let mut raw_detections: Vec<(usize, f64)> = Vec::new();

    for i in 0..num_windows {
        let win_start = i * window_len;
        let win_end = ((i + 1) * window_len).min(rir.len());
        if win_start >= rir.len() {
            break;
        }

        // Compute energies in this window
        let mut energies: Vec<f64> = (win_start..win_end)
            .map(|j| {
                let s = rir[j] as f64;
                s * s
            })
            .collect();

        if energies.is_empty() {
            continue;
        }

        // Compute median energy
        let median = median_of(&mut energies);

        // Threshold: energy must exceed `energy_threshold` times the median
        let threshold = config.energy_threshold * median;

        // Find the sample with maximum energy above threshold
        let mut best_idx = None;
        let mut best_energy = 0.0;

        for (j, &sample) in rir.iter().enumerate().take(win_end).skip(win_start) {
            let e = (sample as f64) * (sample as f64);
            if e > threshold && e > best_energy {
                best_energy = e;
                best_idx = Some(j);
            }
        }

        if let Some(idx) = best_idx {
            // Skip if within direct sound window
            if idx >= ds_start && idx < ds_end {
                continue;
            }
            raw_detections.push((idx, best_energy));
        }
    }

    // Sort by sample index
    raw_detections.sort_by_key(|&(idx, _)| idx);

    // Assign DOA vectors and build DetectedReflection list
    let mut reflections: Vec<DetectedReflection> = raw_detections
        .iter()
        .map(|&(toa, energy)| DetectedReflection {
            toa_sample: toa,
            peak_energy: energy,
            doa: doa_vectors.and_then(|doas| doas.get(toa).copied()),
        })
        .collect();

    // Validate consecutive pairs and merge if too close
    validate_and_merge(&mut reflections, config);

    reflections
}

/// Validate consecutive reflection pairs using DOA and TOA thresholds.
///
/// From Eq. (3) in the paper: a reflection R_{j+1} is retained only if
///   DELTA_DOA(j) >= lambda_DOA  AND  DELTA_TOA(j) > lambda_TOA
/// Otherwise it is merged with the previous reflection (keeping the one with higher energy).
fn validate_and_merge(reflections: &mut Vec<DetectedReflection>, config: &SsirConfig) {
    if reflections.len() < 2 {
        return;
    }

    let toa_threshold = config.toa_threshold_samples();
    let doa_threshold_rad = config.doa_threshold_deg.to_radians();

    let mut i = 0;
    while i + 1 < reflections.len() {
        let toa_diff = reflections[i + 1]
            .toa_sample
            .saturating_sub(reflections[i].toa_sample);

        let doa_diff = match (&reflections[i].doa, &reflections[i + 1].doa) {
            (Some(a), Some(b)) => angular_distance(a, b),
            // Without DOA data, skip the DOA check (only use TOA)
            _ => f64::MAX,
        };

        // Keep the pair separate if BOTH conditions are met:
        // angular distance >= threshold AND temporal distance > threshold
        let spatially_distinct = doa_diff >= doa_threshold_rad;
        let temporally_distinct = toa_diff > toa_threshold;

        if spatially_distinct && temporally_distinct {
            i += 1;
        } else {
            // Merge: keep the one with higher energy
            if reflections[i + 1].peak_energy > reflections[i].peak_energy {
                reflections[i] = reflections[i + 1].clone();
            }
            reflections.remove(i + 1);
            // Don't advance i — re-check with the next element
        }
    }
}

/// Compute angular distance between two DOA unit vectors in radians.
fn angular_distance(a: &[f32; 3], b: &[f32; 3]) -> f64 {
    let dot = (a[0] as f64) * (b[0] as f64)
        + (a[1] as f64) * (b[1] as f64)
        + (a[2] as f64) * (b[2] as f64);

    let norm_a = ((a[0] as f64).powi(2) + (a[1] as f64).powi(2) + (a[2] as f64).powi(2)).sqrt();
    let norm_b = ((b[0] as f64).powi(2) + (b[1] as f64).powi(2) + (b[2] as f64).powi(2)).sqrt();

    let denom = norm_a * norm_b;
    if denom < 1e-12 {
        return 0.0;
    }

    let cos_angle = (dot / denom).clamp(-1.0, 1.0);
    cos_angle.acos()
}

/// Find local maxima in a signal with minimum spacing.
fn find_peaks_with_min_distance(signal: &[f64], min_distance: usize) -> Vec<usize> {
    let mut peaks = Vec::new();
    let len = signal.len();
    if len < 3 {
        return peaks;
    }

    // Find all local maxima
    for i in 1..len - 1 {
        if signal[i] > signal[i - 1] && signal[i] >= signal[i + 1] {
            peaks.push(i);
        }
    }

    if min_distance <= 1 {
        return peaks;
    }

    // Enforce minimum distance: greedily keep peaks by descending magnitude
    let mut indexed: Vec<(usize, f64)> = peaks.iter().map(|&i| (i, signal[i])).collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut kept = Vec::new();
    let mut suppressed = vec![false; len];

    for (idx, _) in indexed {
        if suppressed[idx] {
            continue;
        }
        kept.push(idx);
        // Suppress nearby peaks
        let start = idx.saturating_sub(min_distance);
        let end = (idx + min_distance + 1).min(len);
        for (j, flag) in suppressed.iter_mut().enumerate().take(end).skip(start) {
            if j != idx {
                *flag = true;
            }
        }
    }

    kept.sort();
    kept
}

/// Compute the median of a mutable slice (partially sorts in place).
fn median_of(values: &mut [f64]) -> f64 {
    let len = values.len();
    if len == 0 {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if len.is_multiple_of(2) {
        (values[len / 2 - 1] + values[len / 2]) / 2.0
    } else {
        values[len / 2]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_peaks_basic() {
        let signal = vec![0.0, 1.0, 0.0, 2.0, 0.0, 3.0, 0.0];
        let peaks = find_peaks_with_min_distance(&signal, 1);
        assert_eq!(peaks, vec![1, 3, 5]);
    }

    #[test]
    fn test_find_peaks_min_distance() {
        let signal = vec![0.0, 1.0, 0.5, 2.0, 0.0, 0.5, 3.0, 0.0];
        let peaks = find_peaks_with_min_distance(&signal, 3);
        // Should keep 6 (val=3.0) and suppress 3 (val=2.0, within distance 3 of 6)
        // Then keep 1 (val=1.0, distance 5 from 6)
        assert!(peaks.contains(&6));
        assert!(peaks.contains(&1));
    }

    #[test]
    fn test_median_of() {
        assert_eq!(median_of(&mut [3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median_of(&mut [4.0, 1.0, 3.0, 2.0]), 2.5);
        assert_eq!(median_of(&mut [1.0]), 1.0);
    }

    #[test]
    fn test_angular_distance() {
        let front = [1.0, 0.0, 0.0];
        let left = [0.0, 1.0, 0.0];
        let dist = angular_distance(&front, &left);
        assert!((dist - std::f64::consts::FRAC_PI_2).abs() < 1e-6);
    }

    #[test]
    fn test_direct_sound_detection() {
        // Synthetic RIR: silence, then a strong impulse at sample 100
        let mut rir = vec![0.001f32; 500];
        rir[100] = 1.0;
        rir[200] = 0.3; // weaker reflection
        rir[300] = 0.2; // weaker reflection

        let config = SsirConfig::new(48000.0);
        let toa = find_direct_sound_toa(&rir, &config);
        assert_eq!(toa, Some(100));
    }

    #[test]
    fn test_detect_reflections_basic() {
        // Synthetic RIR at 48kHz: direct sound at sample 48, reflections at 288 and 480
        let mut rir = vec![0.0001f32; 2400]; // 50ms
        rir[48] = 1.0; // direct sound at 1ms
        rir[288] = 0.5; // reflection at 6ms
        rir[480] = 0.3; // reflection at 10ms

        let config = SsirConfig {
            sample_rate: 48000.0,
            mixing_time_ms: Some(40.0),
            ..SsirConfig::default()
        };

        let reflections = detect_reflections(&rir, 48, None, &config);
        assert!(
            reflections.len() >= 2,
            "expected at least 2 reflections, got {}",
            reflections.len()
        );

        // Reflections should be near samples 288 and 480
        let toas: Vec<usize> = reflections.iter().map(|r| r.toa_sample).collect();
        assert!(toas.iter().any(|&t| (t as i64 - 288).unsigned_abs() < 48));
        assert!(toas.iter().any(|&t| (t as i64 - 480).unsigned_abs() < 48));
    }
}
