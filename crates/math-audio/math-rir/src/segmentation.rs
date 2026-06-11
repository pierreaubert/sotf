use crate::config::SsirConfig;
use crate::detection::DetectedReflection;
use crate::types::RirSegment;
use rayon::prelude::*;

/// Build segments from detected reflections with onset refinement.
///
/// For each detected reflection, the onset is refined by searching for the
/// earliest detectable transient within [TOA - onset_window, TOA].
/// Segments shorter than `min_segment_ms` are merged with the predecessor.
///
/// The direct sound segment starts at sample 0 (or the onset of the RIR)
/// and ends at the onset of the first reflection.
pub(crate) fn build_segments(
    rir: &[f32],
    direct_sound_toa: usize,
    direct_sound_doa: Option<[f32; 3]>,
    reflections: &[DetectedReflection],
    mixing_time_samples: usize,
    config: &SsirConfig,
) -> Vec<RirSegment> {
    if rir.is_empty() || direct_sound_toa >= rir.len() {
        return Vec::new();
    }

    let onset_window = config.onset_window_samples();
    let min_segment = config.min_segment_samples();
    let final_segment = config.final_segment_samples();

    // Build the combined sound event list: direct sound + reflections
    let mut events: Vec<(usize, f64, Option<[f32; 3]>, bool)> = Vec::new();

    // Direct sound
    let ds_energy = (rir[direct_sound_toa] as f64).powi(2);
    events.push((direct_sound_toa, ds_energy, direct_sound_doa, true));

    // Early reflections
    for r in reflections {
        events.push((r.toa_sample, r.peak_energy, r.doa, false));
    }

    // Compute onsets for each event
    let mut onsets: Vec<usize> = Vec::with_capacity(events.len() + 1);

    // The RIR starts at sample 0
    onsets.push(0);

    // For each reflection (skip direct sound — its onset is 0), find the onset
    onsets.extend(
        events[1..]
            .par_iter()
            .map(|event| find_onset(rir, event.0, onset_window))
            .collect::<Vec<_>>(),
    );

    // Enforce minimum segment duration by merging short segments with predecessors
    let mut refined_onsets: Vec<usize> = vec![onsets[0]];
    let mut refined_events: Vec<usize> = vec![0]; // indices into events

    for (i, &onset) in onsets.iter().enumerate().skip(1) {
        let prev_onset = *refined_onsets.last().unwrap();
        let duration = onset.saturating_sub(prev_onset);

        // Direct sound (event index 0) is always kept; only reflections are subject to merging
        if i > 0 && duration < min_segment {
            // Too short: merge with predecessor (skip this onset)
            continue;
        }

        refined_onsets.push(onsets[i]);
        refined_events.push(i);
    }

    // Build RirSegment list
    let num_segments = refined_onsets.len();
    let mut segments: Vec<RirSegment> = Vec::with_capacity(num_segments);

    for seg_idx in 0..num_segments {
        let event_idx = refined_events[seg_idx];
        let (toa, peak_energy, doa, is_direct) = &events[event_idx];

        let onset_sample = refined_onsets[seg_idx];

        // End sample: next segment's onset, or mixing_time + final_segment for the last
        let end_sample = if seg_idx + 1 < num_segments {
            refined_onsets[seg_idx + 1]
        } else {
            // Last segment: extend by final_segment_ms past the last onset,
            // but don't exceed mixing time + final segment allowance
            (onset_sample + final_segment)
                .min(mixing_time_samples + final_segment)
                .min(rir.len())
        };

        segments.push(RirSegment {
            onset_sample,
            end_sample,
            toa_sample: *toa,
            doa: *doa,
            peak_energy: *peak_energy,
            is_direct_sound: *is_direct,
        });
    }

    segments
}

/// Find the onset of a sound event within a pre-onset window.
///
/// Uses a simple peak-relative energy-rise heuristic: finds the earliest
/// sample in `[toa - window, toa]` whose energy reaches 10% of the TOA peak.
/// This is intentionally conservative for hard RIR transients and is not a
/// full Defrance noise-floor-relative onset detector.
fn find_onset(rir: &[f32], toa: usize, onset_window: usize) -> usize {
    let start = toa.saturating_sub(onset_window);
    if start >= toa || toa >= rir.len() {
        return toa;
    }

    // Compute energy in the onset search window
    let window: Vec<f64> = (start..=toa.min(rir.len() - 1))
        .map(|i| (rir[i] as f64).powi(2))
        .collect();

    if window.is_empty() {
        return toa;
    }

    // The peak energy is at the TOA (end of window)
    let peak_energy = *window.last().unwrap();
    if peak_energy < 1e-20 {
        return toa;
    }

    // Onset = first sample where energy rises above 10% of the peak
    // (a common heuristic for transient onset detection)
    let threshold = peak_energy * 0.1;

    for (i, &e) in window.iter().enumerate() {
        if e >= threshold {
            return start + i;
        }
    }

    toa
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_onset_places_before_peak() {
        // Ramp up to a peak at sample 100
        let mut rir = vec![0.0001f32; 200];
        rir[95] = 0.05;
        rir[96] = 0.1;
        rir[97] = 0.3;
        rir[98] = 0.6;
        rir[99] = 0.9;
        rir[100] = 1.0;

        let onset = find_onset(&rir, 100, 24); // 0.5ms @ 48kHz
        assert!(onset <= 100, "onset should be at or before TOA");
        assert!(
            onset >= 95,
            "onset should be within the ramp-up, got {onset}"
        );
    }

    #[test]
    fn test_build_segments_basic() {
        let mut rir = vec![0.0001f32; 2400]; // 50ms @ 48kHz
        rir[48] = 1.0; // direct sound at 1ms
        rir[288] = 0.5; // reflection at 6ms
        rir[480] = 0.3; // reflection at 10ms

        let reflections = vec![
            DetectedReflection {
                toa_sample: 288,
                peak_energy: 0.25,
                doa: None,
            },
            DetectedReflection {
                toa_sample: 480,
                peak_energy: 0.09,
                doa: None,
            },
        ];

        let config = SsirConfig {
            sample_rate: 48000.0,
            mixing_time_ms: Some(40.0),
            ..SsirConfig::default()
        };

        let segments = build_segments(
            &rir,
            48,
            None,
            &reflections,
            config.mixing_time_samples(),
            &config,
        );

        // Should have 3 segments: direct + 2 reflections
        assert_eq!(segments.len(), 3);
        assert!(segments[0].is_direct_sound);
        assert!(!segments[1].is_direct_sound);
        assert!(!segments[2].is_direct_sound);

        // Segments should be consecutive
        assert_eq!(segments[0].end_sample, segments[1].onset_sample);
        assert_eq!(segments[1].end_sample, segments[2].onset_sample);
    }

    #[test]
    fn test_short_segments_are_merged() {
        let mut rir = vec![0.0001f32; 2400];
        rir[48] = 1.0;
        rir[288] = 0.5;
        rir[290] = 0.4; // very close to previous — should be merged

        let reflections = vec![
            DetectedReflection {
                toa_sample: 288,
                peak_energy: 0.25,
                doa: None,
            },
            DetectedReflection {
                toa_sample: 290,
                peak_energy: 0.16,
                doa: None,
            },
        ];

        let config = SsirConfig {
            sample_rate: 48000.0,
            min_segment_ms: 1.0, // 48 samples — the gap between 288 and 290 is only 2
            mixing_time_ms: Some(40.0),
            ..SsirConfig::default()
        };

        let segments = build_segments(
            &rir,
            48,
            None,
            &reflections,
            config.mixing_time_samples(),
            &config,
        );

        // The second reflection should be merged, leaving only 2 segments
        assert_eq!(segments.len(), 2);
    }

    #[test]
    fn test_first_reflection_merged_when_too_short() {
        // The first reflection (event index 1) should also be subject to merging
        // if its segment is shorter than min_segment. The comment says
        // "only reflections are subject to merging" — the direct sound is exempt.
        let mut rir = vec![0.0001f32; 2400];
        rir[100] = 1.0; // direct sound
        rir[110] = 0.5; // first reflection very close to DS

        let reflections = vec![DetectedReflection {
            toa_sample: 110,
            peak_energy: 0.25,
            doa: None,
        }];

        // min_segment = 48 samples (1ms). The gap from DS onset (0) to R1 onset (110)
        // is 110 samples, which is > 48, so it would NOT be merged. We need a larger
        // min_segment to trigger merging. Use 6ms = 288 samples.
        let config = SsirConfig {
            sample_rate: 48000.0,
            direct_sound_window_ms: (0.1, 0.1), // tiny DS window so R1 is detected
            min_segment_ms: 6.0,                // 288 samples — gap 110 < 288
            mixing_time_ms: Some(40.0),
            ..SsirConfig::default()
        };

        let segments = build_segments(
            &rir,
            100,
            None,
            &reflections,
            config.mixing_time_samples(),
            &config,
        );

        // The first reflection should be merged into the direct sound,
        // leaving only 1 segment (the direct sound).
        assert_eq!(
            segments.len(),
            1,
            "first reflection should be merged when its segment is shorter than min_segment"
        );
    }

    #[test]
    fn test_find_onset_edge_cases() {
        // Empty RIR
        assert_eq!(find_onset(&[], 0, 10), 0);

        // toa out of bounds
        let rir = vec![0.5f32; 10];
        assert_eq!(find_onset(&rir, 20, 5), 20);

        // onset_window = 0
        assert_eq!(find_onset(&rir, 5, 0), 5);

        // start >= toa (window too small to move back)
        assert_eq!(find_onset(&rir, 0, 0), 0);

        // Zero-energy peak (all zeros)
        let zeros = vec![0.0f32; 100];
        assert_eq!(find_onset(&zeros, 50, 10), 50);

        // Very low energy peak
        let mut low = vec![0.0f32; 100];
        low[50] = 1e-12;
        assert_eq!(find_onset(&low, 50, 10), 50);

        // Threshold not reached in window (energy stays below 10% of peak)
        let mut ramp = vec![0.0f32; 100];
        ramp[90] = 0.01;
        ramp[95] = 0.05;
        ramp[99] = 1.0;
        let onset = find_onset(&ramp, 99, 10);
        assert!(onset <= 99 && onset >= 90);
    }

    #[test]
    fn test_build_segments_empty_and_invalid() {
        let config = SsirConfig::new(48000.0);

        // Empty RIR
        let segs = build_segments(&[], 0, None, &[], 1000, &config);
        assert!(segs.is_empty());

        // Direct sound TOA beyond RIR length
        let rir = vec![0.5f32; 10];
        let segs = build_segments(&rir, 20, None, &[], 1000, &config);
        assert!(segs.is_empty());
    }

    #[test]
    fn test_build_segments_no_reflections() {
        let mut rir = vec![0.0001f32; 2400];
        rir[48] = 1.0;

        let config = SsirConfig {
            sample_rate: 48000.0,
            mixing_time_ms: Some(40.0),
            ..SsirConfig::default()
        };

        let segs = build_segments(&rir, 48, None, &[], config.mixing_time_samples(), &config);
        assert_eq!(segs.len(), 1);
        assert!(segs[0].is_direct_sound);
        assert_eq!(segs[0].onset_sample, 0);
        assert_eq!(segs[0].toa_sample, 48);
        // Last segment end should be clamped to mixing_time + final_segment
        assert!(
            segs[0].end_sample <= config.mixing_time_samples() + config.final_segment_samples()
        );
    }

    #[test]
    fn test_build_segments_last_segment_end_clamping() {
        let mut rir = vec![0.0001f32; 4800]; // 100ms
        rir[48] = 1.0;
        rir[288] = 0.5;

        let reflections = vec![DetectedReflection {
            toa_sample: 288,
            peak_energy: 0.25,
            doa: None,
        }];

        let config = SsirConfig {
            sample_rate: 48000.0,
            mixing_time_ms: Some(10.0), // very short mixing time
            final_segment_ms: 2.0,
            ..SsirConfig::default()
        };

        let segs = build_segments(
            &rir,
            48,
            None,
            &reflections,
            config.mixing_time_samples(),
            &config,
        );

        assert_eq!(segs.len(), 2);
        // Last segment end should be clamped to mixing_time + final_segment and rir.len()
        let last = segs.last().unwrap();
        let expected_max = (last.onset_sample + config.final_segment_samples())
            .min(config.mixing_time_samples() + config.final_segment_samples())
            .min(rir.len());
        assert_eq!(last.end_sample, expected_max);
    }
}
