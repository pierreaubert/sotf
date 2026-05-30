use crate::config::SsirConfig;
use rayon::prelude::*;

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
/// 1. Compute log magnitude: L_x = 20 · log10(|x|)
/// 2. Find the global maximum.
/// 3. Direct sound = the earliest local maximum within 11 dB of that maximum.
///
/// The direct sound search intentionally does **not** use minimum-distance
/// peak suppression: magnitude-greedy suppression can discard an earlier,
/// physically correct direct arrival when a stronger reflection occurs inside
/// the suppression radius.
pub(crate) fn find_direct_sound_toa(rir: &[f32], _config: &SsirConfig) -> Option<usize> {
    if rir.is_empty() {
        return None;
    }
    if rir.iter().all(|&x| x.abs() < 1e-12) {
        return None;
    }
    if rir.len() < 3 {
        return earliest_abs_max(rir);
    }

    // Step 1: Compute log magnitude (20 * log10(|x|))
    let log_mag: Vec<f64> = rir
        .iter()
        .map(|&x| {
            let abs = (x as f64).abs().max(1e-20); // avoid log(0)
            20.0 * abs.log10()
        })
        .collect();

    let global_max = log_mag.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let threshold = global_max - 11.0;

    local_peak_indices(&log_mag)
        .into_iter()
        .find(|&i| log_mag[i] >= threshold)
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

    let mut raw_detections: Vec<(usize, f64)> = (0..num_windows)
        .into_par_iter()
        .flat_map(|i| {
            let win_start = i * window_len;
            let win_end = ((i + 1) * window_len).min(rir.len());
            if win_start >= rir.len() {
                return Vec::new();
            }

            // Compute energies in this window
            let mut energies: Vec<f64> = (win_start..win_end)
                .filter(|&j| j < ds_start || j >= ds_end)
                .map(|j| {
                    let s = rir[j] as f64;
                    s * s
                })
                .collect();

            if energies.is_empty() {
                return Vec::new();
            }

            // Compute median energy
            let median = median_of(&mut energies);

            // Threshold: energy must exceed `energy_threshold` times the median
            let threshold = config.energy_threshold * median;

            let mut detections = Vec::new();
            for (j, &sample) in rir.iter().enumerate().take(win_end).skip(win_start) {
                if j >= ds_start && j < ds_end {
                    continue;
                }
                let e = (sample as f64) * (sample as f64);
                if e > threshold && is_local_energy_peak(rir, j, win_start, win_end) {
                    detections.push((j, e));
                }
            }

            detections
        })
        .collect();

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
            // Re-check both the next pair and, if replacement moved a later
            // event leftward, the previous neighbor.
            i = i.saturating_sub(1);
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

fn earliest_abs_max(rir: &[f32]) -> Option<usize> {
    let mut best_idx = None;
    let mut best = f32::NEG_INFINITY;
    for (i, &sample) in rir.iter().enumerate() {
        let abs = sample.abs();
        if abs > best {
            best = abs;
            best_idx = Some(i);
        }
    }
    best_idx
}

/// Find local maxima in a signal, returning the first sample of a plateau.
fn local_peak_indices(signal: &[f64]) -> Vec<usize> {
    let mut peaks = Vec::new();
    let len = signal.len();
    if len < 2 {
        return peaks;
    }

    let mut i = 0;
    while i < len {
        let plateau_start = i;
        let mut plateau_end = i;
        while plateau_end + 1 < len && signal[plateau_end + 1] == signal[plateau_start] {
            plateau_end += 1;
        }

        let left_lower = plateau_start == 0 || signal[plateau_start] > signal[plateau_start - 1];
        let right_lower = plateau_end + 1 == len || signal[plateau_end] > signal[plateau_end + 1];
        let has_neighbor = plateau_start > 0 || plateau_end + 1 < len;

        if has_neighbor && left_lower && right_lower {
            peaks.push(plateau_start);
        }

        i = plateau_end + 1;
    }

    peaks
}

fn is_local_energy_peak(rir: &[f32], idx: usize, start: usize, end: usize) -> bool {
    if idx >= rir.len() || idx < start || idx >= end {
        return false;
    }

    let e = (rir[idx] as f64) * (rir[idx] as f64);
    let left = idx.checked_sub(1).filter(|&i| i >= start);
    let right = (idx + 1 < end && idx + 1 < rir.len()).then_some(idx + 1);

    if let Some(left) = left {
        let left_e = (rir[left] as f64) * (rir[left] as f64);
        if e <= left_e {
            return false;
        }
    }

    if let Some(right) = right {
        let right_e = (rir[right] as f64) * (rir[right] as f64);
        if e < right_e {
            return false;
        }
    }

    true
}

/// Compute the median of a mutable slice (partially sorts in place).
fn median_of(values: &mut [f64]) -> f64 {
    let len = values.len();
    if len == 0 {
        return 0.0;
    }
    // Partition non-finite values to the end so they don't corrupt the median.
    let mut write = 0;
    for read in 0..len {
        if values[read].is_finite() {
            values.swap(write, read);
            write += 1;
        }
    }
    let valid_len = write;
    if valid_len == 0 {
        return f64::NAN;
    }
    values[..valid_len].sort_by(|a, b| a.total_cmp(b));
    if valid_len.is_multiple_of(2) {
        (values[valid_len / 2 - 1] + values[valid_len / 2]) / 2.0
    } else {
        values[valid_len / 2]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_peaks_basic() {
        let signal = vec![0.0, 1.0, 0.0, 2.0, 0.0, 3.0, 0.0];
        let peaks = local_peak_indices(&signal);
        assert_eq!(peaks, vec![1, 3, 5]);
    }

    #[test]
    fn test_find_peaks_plateau_uses_first_sample() {
        let signal = vec![0.0, 5.0, 5.0, 5.0, 0.0];
        let peaks = local_peak_indices(&signal);
        assert_eq!(peaks, vec![1]);
    }

    #[test]
    fn test_find_peaks_accepts_edge_plateau() {
        let signal = vec![5.0, 5.0, 0.0, 1.0, 0.0];
        let peaks = local_peak_indices(&signal);
        assert_eq!(peaks, vec![0, 3]);
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

    #[test]
    fn test_find_direct_sound_toa_short_rir() {
        // Very short RIRs (< 3 samples) have no local maxima in the traditional sense,
        // but the global maximum should still be found as the direct sound.
        let rir = vec![0.0f32, 1.0f32];
        let config = SsirConfig::new(48000.0);
        let toa = find_direct_sound_toa(&rir, &config);
        assert_eq!(toa, Some(1), "should find the global max in a 2-sample RIR");
    }

    #[test]
    fn test_find_direct_sound_toa_short_rir_tie_prefers_earliest() {
        let rir = vec![1.0f32, -1.0f32];
        let config = SsirConfig::new(48000.0);
        let toa = find_direct_sound_toa(&rir, &config);
        assert_eq!(toa, Some(0), "ties should prefer the earliest arrival");
    }

    #[test]
    fn test_find_direct_sound_earliest_above_11db_threshold() {
        // Algorithm 1 uses the first local maximum inside the 11 dB envelope,
        // not the strongest peak in a narrower tie band.
        let mut rir = vec![0.0001f32; 500];
        rir[50] = 0.30;
        rir[100] = 1.0;
        rir[300] = 0.18;

        let config = SsirConfig::new(48000.0);
        let toa = find_direct_sound_toa(&rir, &config);
        assert_eq!(toa, Some(50));
    }

    #[test]
    fn test_find_direct_sound_ignores_min_distance_suppression_order() {
        let mut rir = vec![0.0001f32; 500];
        rir[100] = 0.5; // direct sound, within 11 dB of the reflection
        rir[130] = 1.0; // stronger floor reflection inside min-distance radius

        let config = SsirConfig {
            sample_rate: 48000.0,
            min_peak_distance_ms: 1.0,
            ..SsirConfig::default()
        };

        let toa = find_direct_sound_toa(&rir, &config);
        assert_eq!(toa, Some(100));
    }

    #[test]
    fn test_find_direct_sound_earliest_in_tie_band() {
        // When several peaks satisfy the 11 dB direct-sound envelope, return
        // the earliest qualifying local maximum.
        let mut rir = vec![0.0001f32; 500];
        rir[100] = 1.0;
        rir[200] = 1.0;

        let config = SsirConfig::new(48000.0);
        let toa = find_direct_sound_toa(&rir, &config);
        assert_eq!(
            toa,
            Some(100),
            "earliest peak in the direct-sound envelope should be returned"
        );
    }

    #[test]
    fn test_median_of_with_nan() {
        // NaN values should not corrupt the median — they should be ignored
        // and the median computed from the finite values only.
        let mut values = [3.0, f64::NAN, 1.0, 2.0];
        let m = median_of(&mut values);
        assert!(
            m.is_finite(),
            "median should be finite when finite values exist, got {}",
            m
        );
        assert_eq!(m, 2.0, "median of [1, 2, 3] should be 2.0");
    }

    #[test]
    fn test_detect_reflections_keeps_multiple_peaks_in_ler_window() {
        let mut rir = vec![0.0001f32; 2400];
        rir[48] = 1.0;
        rir[288] = 0.5;
        rir[320] = 0.4;

        let config = SsirConfig {
            sample_rate: 48000.0,
            mixing_time_ms: Some(40.0),
            ..SsirConfig::default()
        };

        let reflections = detect_reflections(&rir, 48, None, &config);
        let toas: Vec<usize> = reflections.iter().map(|r| r.toa_sample).collect();
        assert!(
            toas.contains(&288) && toas.contains(&320),
            "expected both same-window reflections, got {toas:?}"
        );
    }

    #[test]
    fn test_validate_and_merge_rechecks_previous_neighbor_after_replacement() {
        let mut reflections = vec![
            DetectedReflection {
                toa_sample: 100,
                peak_energy: 1.0,
                doa: Some([1.0, 0.0, 0.0]),
            },
            DetectedReflection {
                toa_sample: 140,
                peak_energy: 0.1,
                doa: Some([0.0, 1.0, 0.0]),
            },
            DetectedReflection {
                toa_sample: 150,
                peak_energy: 2.0,
                doa: Some([1.0, 0.0, 0.0]),
            },
        ];
        let config = SsirConfig::new(48000.0);

        validate_and_merge(&mut reflections, &config);

        assert_eq!(reflections.len(), 1);
        assert_eq!(reflections[0].toa_sample, 150);
    }
}
