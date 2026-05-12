// ============================================================================
// PND Analysis Logic — Windowed Hop-Based Drift Estimation
// ============================================================================

use sotf_host::stft_common::{RealFftProcessor, RingAccumulator, generate_hann_window};

/// Minimum number of matched partials required to record a drift measurement.
/// Lowered from 3 to 2 to improve detection sensitivity on simpler signals
/// (e.g. single-note or two-partial tones).
const MIN_MATCHED_PARTIALS: usize = 2;

/// Weight for log-amplitude distance in the combined matching cost.
/// Frequency remains the primary discriminator; amplitude breaks ties.
const AMPLITUDE_COST_WEIGHT: f32 = 0.1;

pub struct PndAnalyzer {
    fft_size: usize,
    sample_rate: u32,
    window: Vec<f32>,
    fft: RealFftProcessor,
    ring: RingAccumulator,

    // Partial tracking state
    prev_peaks: Vec<(f32, f32)>,    // (frequency_hz, magnitude)
    matched_peaks: Vec<(f32, f32)>, // (frequency_hz, magnitude)

    // Pre-allocated scratch buffers (reused via .clear())
    peak_scratch: Vec<(f32, f32)>,
    ratio_scratch: Vec<f32>,

    // Drift history circular buffer (sized by analysis_window_ms)
    drift_history: Vec<f32>,
    drift_write_pos: usize,
    drift_count: usize,
    drift_history_capacity: usize,

    // Pre-allocated sort buffer for median computation
    median_scratch: Vec<f32>,

    // Cached drift estimate — recomputed only when drift_history changes.
    cached_drift_estimate: f32,
    drift_dirty: bool,

    // Confidence tracking
    last_confidence: f32,
    last_matched_partials: usize,
    last_total_peaks: usize,
}

impl PndAnalyzer {
    pub fn new(fft_size: usize, sample_rate: u32, analysis_window_ms: f32) -> Self {
        let hop_size = fft_size / 4; // 512 for fft_size=2048
        let window = generate_hann_window(fft_size);
        let fft = RealFftProcessor::new_forward_only(fft_size);
        let ring = RingAccumulator::new(fft_size, hop_size);

        let drift_history_capacity =
            compute_drift_history_capacity(analysis_window_ms, sample_rate, hop_size);
        let max_drift_history_capacity =
            compute_drift_history_capacity(500.0, sample_rate, hop_size);
        let spectrum_size = fft_size / 2 + 1;

        Self {
            fft_size,
            sample_rate,
            window,
            fft,
            ring,
            prev_peaks: Vec::with_capacity(spectrum_size),
            matched_peaks: Vec::with_capacity(spectrum_size),
            peak_scratch: Vec::with_capacity(spectrum_size),
            ratio_scratch: Vec::with_capacity(spectrum_size),

            drift_history: vec![0.0; max_drift_history_capacity],
            drift_write_pos: 0,
            drift_count: 0,
            drift_history_capacity,

            median_scratch: vec![0.0; max_drift_history_capacity],

            cached_drift_estimate: 1.0,
            drift_dirty: false,

            last_confidence: 0.0,
            last_matched_partials: 0,
            last_total_peaks: 0,
        }
    }

    /// Feed samples and return the current drift estimate.
    /// Samples are accumulated in a ring buffer; FFT is triggered every hop_size new samples
    /// once the ring buffer has been filled at least once (fft_size samples).
    pub fn analyze(&mut self, samples: &[f32]) -> f32 {
        for &sample in samples {
            if self.ring.push(sample) {
                self.process_fft_frame();
            }
        }

        self.current_drift_estimate()
    }

    fn process_fft_frame(&mut self) {
        // Read ring buffer directly into fft.time_buffer, then apply the window
        // in-place — fusing what was two passes (read_window + multiply) into one.
        self.ring.read_window(&mut self.fft.time_buffer);
        for i in 0..self.fft_size {
            self.fft.time_buffer[i] *= self.window[i];
        }

        self.fft.forward();

        // Peak picking on the real-FFT spectrum (spectrum_size bins)
        let bin_hz = self.sample_rate as f32 / self.fft_size as f32;
        self.peak_scratch.clear();

        let threshold = 0.001; // ~-60dB
        for i in 1..self.fft.spectrum_size - 1 {
            let mag_prev = self.fft.freq_buffer[i - 1].norm();
            let mag_curr = self.fft.freq_buffer[i].norm();
            let mag_next = self.fft.freq_buffer[i + 1].norm();

            if mag_curr > threshold && mag_curr > mag_prev && mag_curr > mag_next {
                // Parabolic interpolation for more accurate frequency
                let alpha = mag_prev;
                let beta = mag_curr;
                let gamma = mag_next;
                let denom = alpha - 2.0 * beta + gamma;
                let p = if denom.abs() > f32::EPSILON {
                    0.5 * (alpha - gamma) / denom
                } else {
                    0.0
                };
                let freq = (i as f32 + p) * bin_hz;
                self.peak_scratch.push((freq, mag_curr));
            }
        }

        // Partial tracking: match current peaks against previous frame
        // using combined frequency + amplitude cost
        self.ratio_scratch.clear();
        self.matched_peaks.clear();
        let mut matched_partials = 0;

        if !self.prev_peaks.is_empty() {
            for &(freq, mag) in &self.peak_scratch {
                let mut min_cost = f32::MAX;
                let mut best_prev_freq = 0.0_f32;

                for &(prev_freq, prev_mag) in &self.prev_peaks {
                    let freq_distance = (freq - prev_freq).abs();

                    // Quick reject: beyond 3% frequency change
                    if freq_distance > prev_freq * 0.03 {
                        continue;
                    }

                    // Combined cost: frequency distance + weighted log-amplitude distance
                    let log_amp_distance = ((mag + 1e-10).ln() - (prev_mag + 1e-10).ln()).abs();
                    let cost = freq_distance + AMPLITUDE_COST_WEIGHT * log_amp_distance;

                    if cost < min_cost {
                        min_cost = cost;
                        best_prev_freq = prev_freq;
                    }
                }

                // Within 50 cents (~3% change) → same partial
                if best_prev_freq > 0.0 && min_cost < f32::MAX {
                    let ratio = freq / best_prev_freq;
                    self.ratio_scratch.push(ratio);
                    self.matched_peaks.push((freq, mag));
                    matched_partials += 1;
                }
            }
        }

        // Update confidence tracking
        self.last_total_peaks = self.peak_scratch.len();
        self.last_matched_partials = matched_partials;
        self.last_confidence = if self.last_total_peaks > 0 {
            matched_partials as f32 / self.last_total_peaks as f32
        } else {
            0.0
        };

        // Swap peaks: reuse prev_peaks capacity
        self.prev_peaks.clear();
        self.prev_peaks.extend_from_slice(&self.peak_scratch);

        // Push median of ratios into drift history only if enough partials matched
        if matched_partials >= MIN_MATCHED_PARTIALS && !self.ratio_scratch.is_empty() {
            let mid = self.ratio_scratch.len() / 2;
            self.ratio_scratch.select_nth_unstable_by(mid, |a, b| {
                a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
            });
            let frame_drift = self.ratio_scratch[mid];

            self.drift_history[self.drift_write_pos] = frame_drift;
            self.drift_write_pos = (self.drift_write_pos + 1) % self.drift_history_capacity;
            if self.drift_count < self.drift_history_capacity {
                self.drift_count += 1;
            }
            self.drift_dirty = true;
        }
    }

    fn current_drift_estimate(&mut self) -> f32 {
        if self.drift_count == 0 {
            return 1.0;
        }

        // Skip the O(n) median if drift_history has not changed since last call.
        if !self.drift_dirty {
            return self.cached_drift_estimate;
        }
        self.drift_dirty = false;

        // Compute median of drift_history using O(n) selection.
        // drift_history is a circular buffer — copy it in wrap order.
        let len = self.drift_count.min(self.drift_history_capacity);
        if self.drift_count < self.drift_history_capacity {
            self.median_scratch[..len].copy_from_slice(&self.drift_history[..len]);
        } else {
            let first = self.drift_history_capacity - self.drift_write_pos;
            self.median_scratch[..first].copy_from_slice(
                &self.drift_history[self.drift_write_pos..self.drift_write_pos + first],
            );
            if self.drift_write_pos > 0 {
                self.median_scratch[first..len]
                    .copy_from_slice(&self.drift_history[..self.drift_write_pos]);
            }
        }

        let mid = len / 2;
        self.median_scratch[..len].select_nth_unstable_by(mid, |a, b| {
            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
        });
        self.cached_drift_estimate = self.median_scratch[mid];
        self.cached_drift_estimate
    }

    pub fn update_analysis_window(&mut self, analysis_window_ms: f32) {
        let hop_size = self.fft_size / 4;
        let new_capacity =
            compute_drift_history_capacity(analysis_window_ms, self.sample_rate, hop_size);
        if new_capacity != self.drift_history_capacity {
            self.drift_history.fill(0.0);
            self.median_scratch.fill(0.0);
            self.drift_write_pos = 0;
            self.drift_count = 0;
            self.drift_history_capacity = new_capacity.min(self.drift_history.len()).max(1);
            self.cached_drift_estimate = 1.0;
            self.drift_dirty = false;
        }
    }

    /// Get the current drift confidence (0.0 to 1.0).
    pub fn confidence(&self) -> f32 {
        self.last_confidence
    }

    /// Get the number of matched partials in the last frame.
    pub fn matched_partials(&self) -> usize {
        self.last_matched_partials
    }

    /// Get the total number of detected peaks in the last frame.
    pub fn total_peaks(&self) -> usize {
        self.last_total_peaks
    }

    /// Get the matched peaks (frequency, magnitude) from the last frame.
    pub fn current_matched_peaks(&self) -> &[(f32, f32)] {
        &self.matched_peaks
    }

    pub fn reset(&mut self) {
        self.ring.reset();

        // Clear peak tracking
        self.prev_peaks.clear();
        self.matched_peaks.clear();
        self.peak_scratch.clear();
        self.ratio_scratch.clear();

        // Reset drift history
        self.drift_history.fill(0.0);
        self.drift_write_pos = 0;
        self.drift_count = 0;
        self.median_scratch.fill(0.0);
        self.cached_drift_estimate = 1.0;
        self.drift_dirty = false;

        // Reset confidence
        self.last_confidence = 0.0;
        self.last_matched_partials = 0;
        self.last_total_peaks = 0;
    }
}

/// Compute drift history capacity: how many FFT frames fit in `analysis_window_ms`.
fn compute_drift_history_capacity(
    analysis_window_ms: f32,
    sample_rate: u32,
    hop_size: usize,
) -> usize {
    let samples_in_window = (analysis_window_ms / 1000.0 * sample_rate as f32) as usize;
    let capacity = samples_in_window / hop_size;
    capacity.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyzer_silence_returns_no_drift() {
        let mut analyzer = PndAnalyzer::new(2048, 44100, 100.0);
        let silence = vec![0.0; 4096];
        let drift = analyzer.analyze(&silence);
        assert!(
            (drift - 1.0).abs() < f32::EPSILON,
            "Silence should produce no drift, got {drift}"
        );
    }

    #[test]
    fn test_analyzer_stable_tone_returns_near_unity() {
        let mut analyzer = PndAnalyzer::new(2048, 44100, 100.0);

        // Generate several blocks of 440 Hz sine (enough to fill ring buffer + multiple hops)
        let num_samples = 44100; // 1 second
        let samples: Vec<f32> = (0..num_samples)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();

        let drift = analyzer.analyze(&samples);

        // Stable tone should produce drift very close to 1.0
        assert!(
            (drift - 1.0).abs() < 0.01,
            "Stable 440Hz tone should produce drift ~1.0, got {drift}"
        );
    }

    #[test]
    fn test_analyzer_processes_small_blocks() {
        let mut analyzer = PndAnalyzer::new(2048, 44100, 100.0);

        // Feed 1024-sample blocks (typical process() call size)
        let block_size = 1024;
        let samples: Vec<f32> = (0..block_size)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();

        // First block: ring not yet filled (need 2048), should return 1.0
        let drift1 = analyzer.analyze(&samples);
        assert!(
            (drift1 - 1.0).abs() < f32::EPSILON,
            "First 1024 samples shouldn't trigger FFT, got {drift1}"
        );

        // Second block: ring fills at sample 2048, then triggers FFT at 2048+hop
        let drift2 = analyzer.analyze(&samples);
        // Should still be ~1.0 (no prev_peaks for first FFT frame, or stable tone)
        assert!(
            (drift2 - 1.0).abs() < 0.01,
            "Second block drift should be ~1.0, got {drift2}"
        );
    }

    #[test]
    fn test_analyzer_reset() {
        let mut analyzer = PndAnalyzer::new(2048, 44100, 100.0);

        // Feed some data
        let samples: Vec<f32> = (0..4096)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();
        analyzer.analyze(&samples);

        // Reset
        analyzer.reset();

        // After reset, should return 1.0 (no drift history)
        let drift = analyzer.analyze(&vec![0.0; 512]);
        assert!(
            (drift - 1.0).abs() < f32::EPSILON,
            "After reset, drift should be 1.0, got {drift}"
        );
    }

    #[test]
    fn test_analyzer_reset_keeps_median_scratch_capacity() {
        let mut analyzer = PndAnalyzer::new(2048, 44100, 100.0);
        analyzer.reset();

        analyzer.drift_history[0] = 1.01;
        analyzer.drift_write_pos = 1;
        analyzer.drift_count = 1;
        analyzer.drift_dirty = true;

        let drift = analyzer.current_drift_estimate();
        assert!((drift - 1.01).abs() < 1e-6);
    }

    #[test]
    fn test_update_analysis_window() {
        let mut analyzer = PndAnalyzer::new(2048, 44100, 100.0);
        let initial_capacity = analyzer.drift_history_capacity;
        let initial_storage = analyzer.drift_history.len();

        analyzer.update_analysis_window(200.0);
        assert!(
            analyzer.drift_history_capacity > initial_capacity,
            "Doubling window should increase capacity"
        );
        assert_eq!(
            analyzer.drift_history.len(),
            initial_storage,
            "Changing analysis window should not reallocate history storage"
        );
        assert_eq!(analyzer.drift_count, 0, "Should reset drift count");
    }

    #[test]
    fn test_drift_history_capacity_computation() {
        // 100ms at 44100 Hz with hop_size 512
        // samples_in_window = 0.1 * 44100 = 4410
        // capacity = 4410 / 512 = 8
        let cap = compute_drift_history_capacity(100.0, 44100, 512);
        assert_eq!(cap, 8);

        // Very small window should clamp to 1
        let cap_min = compute_drift_history_capacity(1.0, 44100, 512);
        assert_eq!(cap_min, 1);
    }
}
