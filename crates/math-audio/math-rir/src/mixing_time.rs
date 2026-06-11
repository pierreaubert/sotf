use rayon::prelude::*;

/// Estimate the mixing time of a room impulse response.
///
/// Uses the Abel & Huang (2006) echo density method: the mixing time is
/// where the RIR transitions from sparse, discrete reflections to a dense
/// reverberant tail.
///
/// The algorithm computes the normalized echo density in sliding windows.
/// When the density exceeds a threshold (indicating the transition from
/// sparse early reflections to dense reverberant tail), that time is
/// returned as the mixing time.
///
/// Returns mixing time in samples, or a fallback default if estimation fails.
pub(crate) fn estimate_mixing_time(rir: &[f32], sample_rate: f64) -> usize {
    if rir.is_empty() {
        return default_mixing_time_samples(sample_rate);
    }

    // Window size for echo density computation: ~5ms
    let window_samples = (0.005 * sample_rate).round() as usize;
    if window_samples < 4 {
        return default_mixing_time_samples(sample_rate);
    }

    // Maximum search range: 100ms (well beyond typical mixing times of 30-50ms)
    let max_search = (0.100 * sample_rate).round() as usize;
    let search_end = max_search.min(rir.len());

    // Find direct sound peak to start after it
    let direct_peak = rir[..search_end.min(rir.len())]
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            a.abs()
                .partial_cmp(&b.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
        .unwrap_or(0);

    // Start analysis a few ms after direct sound
    let analysis_start = direct_peak + (0.003 * sample_rate).round() as usize;
    if analysis_start + window_samples >= search_end {
        return default_mixing_time_samples(sample_rate);
    }

    // Compute echo density in sliding windows.
    // Echo density = ratio of samples whose magnitude exceeds the local RMS.
    // In sparse early reflections, density is low.
    // In the diffuse tail, density approaches ~0.67 (Gaussian distribution).
    let hop = window_samples / 2;
    let density_threshold = 0.55; // Transition threshold

    let positions: Vec<usize> = std::iter::successors(Some(analysis_start), |pos| {
        let next = pos.saturating_add(hop);
        (next + window_samples <= search_end).then_some(next)
    })
    .collect();

    let densities_above: Vec<bool> = positions
        .par_iter()
        .map(|&pos| {
            let window = &rir[pos..pos + window_samples];

            // Compute RMS of window
            let rms = {
                let sum_sq: f64 = window.iter().map(|&x| (x as f64).powi(2)).sum();
                (sum_sq / window_samples as f64).sqrt()
            };

            if rms < 1e-12 {
                return false;
            }

            // Count samples above RMS (echo density)
            let above_count = window.iter().filter(|&&x| (x as f64).abs() > rms).count();
            let density = above_count as f64 / window_samples as f64;

            density >= density_threshold
        })
        .collect();

    // Running count of windows above threshold
    let mut consecutive_above = 0;
    let required_consecutive = 3;

    for (&pos, &above) in positions.iter().zip(densities_above.iter()) {
        if above {
            consecutive_above += 1;
            if consecutive_above >= required_consecutive {
                // Mixing time = start of the first window in the streak
                return pos.saturating_sub((required_consecutive - 1) * hop);
            }
        } else {
            consecutive_above = 0;
        }
    }

    // If estimation failed (e.g., very dry room), use default
    default_mixing_time_samples(sample_rate)
}

/// Default mixing time: 38ms (median across rooms in the SSIR paper).
fn default_mixing_time_samples(sample_rate: f64) -> usize {
    (0.038 * sample_rate).round() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_mixing_time_returns_reasonable_value() {
        // Synthetic RIR: direct sound + sparse reflections + dense tail
        let sample_rate = 48000.0;
        let len = (0.200 * sample_rate) as usize; // 200ms
        let mut rir = vec![0.0f32; len];

        // Direct sound at 1ms
        rir[48] = 1.0;

        // Sparse early reflections at 5ms, 10ms, 15ms, 20ms
        rir[240] = 0.5;
        rir[480] = 0.3;
        rir[720] = 0.2;
        rir[960] = 0.15;

        // Dense reverberant tail starting around 30ms
        // Fill with decaying noise
        let mixing_start = (0.030 * sample_rate) as usize;
        let mut amplitude = 0.1f32;
        let decay = 0.9997f32;
        let mut rng_state: u32 = 42;
        for sample in rir.iter_mut().take(len).skip(mixing_start) {
            // Simple LCG pseudo-random
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            let noise = ((rng_state >> 16) as f32 / 32768.0) - 1.0;
            *sample = noise * amplitude;
            amplitude *= decay;
        }

        let mt = estimate_mixing_time(&rir, sample_rate);
        let mt_ms = mt as f64 / sample_rate * 1000.0;

        // Should be in the range 20-60ms for this synthetic room
        assert!(
            (15.0..=80.0).contains(&mt_ms),
            "mixing time {mt_ms:.1}ms outside expected range 15-80ms"
        );
    }

    #[test]
    fn test_empty_rir_returns_default() {
        let mt = estimate_mixing_time(&[], 48000.0);
        assert_eq!(mt, 1824); // 38ms * 48000 = 1824
    }

    #[test]
    fn test_dry_rir_returns_default() {
        // RIR with only direct sound, no reverb
        let mut rir = vec![0.0f32; 4800];
        rir[48] = 1.0;

        let mt = estimate_mixing_time(&rir, 48000.0);
        let mt_ms = mt as f64 / 48000.0 * 1000.0;
        // Should fall back to default 38ms
        assert!(
            (mt_ms - 38.0).abs() < 1.0,
            "dry room should return default, got {mt_ms:.1}ms"
        );
    }

    #[test]
    fn test_estimate_mixing_time_very_low_sample_rate() {
        // At 400 Hz, 5ms window = 2 samples → falls back to default (< 4)
        let rir = vec![0.5f32; 100];
        let mt = estimate_mixing_time(&rir, 400.0);
        assert_eq!(mt, default_mixing_time_samples(400.0));
    }

    #[test]
    fn test_estimate_mixing_time_short_rir_no_room() {
        // RIR so short that analysis_start + window >= search_end
        let mut rir = vec![0.0f32; 10];
        rir[0] = 1.0;
        let mt = estimate_mixing_time(&rir, 48000.0);
        assert_eq!(mt, default_mixing_time_samples(48000.0));
    }

    #[test]
    fn test_estimate_mixing_time_all_zeros() {
        // All-zero RIR (not empty) — direct_peak will be sample 0,
        // but RMS in every window is zero, so density never exceeds threshold.
        let rir = vec![0.0f32; 4800];
        let mt = estimate_mixing_time(&rir, 48000.0);
        assert_eq!(mt, default_mixing_time_samples(48000.0));
    }

    #[test]
    fn test_estimate_mixing_time_early_dense_tail() {
        // Dense tail starts immediately after direct sound → mixing time should be early
        let sample_rate = 48000.0;
        let len = (0.100 * sample_rate) as usize;
        let mut rir = vec![0.0f32; len];
        rir[48] = 1.0;

        // Fill everything after direct sound with dense noise
        let mut amp = 0.1f32;
        let mut rng: u32 = 12345;
        for sample in rir.iter_mut().skip(49) {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let noise = ((rng >> 16) as f32 / 32768.0) - 1.0;
            *sample = noise * amp;
            amp *= 0.9997;
        }

        let mt = estimate_mixing_time(&rir, sample_rate);
        let mt_ms = mt as f64 / sample_rate * 1000.0;
        // Should detect an early mixing time (before default 38ms)
        assert!(
            mt_ms < 50.0,
            "early dense tail should yield early mixing time, got {mt_ms:.1}ms"
        );
    }

    #[test]
    fn test_default_mixing_time_samples_exact() {
        assert_eq!(default_mixing_time_samples(48000.0), 1824); // 0.038 * 48000 = 1824
        assert_eq!(default_mixing_time_samples(44100.0), 1676); // 0.038 * 44100 = 1675.8 ≈ 1676
        assert_eq!(default_mixing_time_samples(96000.0), 3648);
    }
}
