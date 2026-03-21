// ============================================================================
// LR4 Crossover — Linkwitz-Riley 4th-order crossover utility
// ============================================================================

use math_audio_iir_fir::{Biquad, BiquadFilterType};

/// A single Linkwitz-Riley 4th-order crossover point.
///
/// Splits a signal into low and high bands with -24 dB/octave slopes.
/// The two outputs sum to unity (flat magnitude response) with zero
/// phase shift at the crossover frequency.
///
/// Extracted from the multiband-compressor's crossover implementation
/// for reuse in crossover, band-split, crossfeed, and other plugins.
#[derive(Debug, Clone)]
pub struct Lr4Crossover {
    /// Two cascaded lowpass biquads per channel.
    lowpass: Vec<[Biquad; 2]>,
    /// Two cascaded highpass biquads per channel.
    highpass: Vec<[Biquad; 2]>,
    freq: f32,
    sample_rate: u32,
    channels: usize,
}

const BUTTERWORTH_Q: f64 = std::f64::consts::FRAC_1_SQRT_2; // 1/√2 ≈ 0.7071

impl Lr4Crossover {
    /// Create a new LR4 crossover at the given frequency.
    pub fn new(freq: f32, sample_rate: u32, channels: usize) -> Self {
        let sr = sample_rate as f64;
        let f = freq as f64;

        let make_pair = |filter_type: BiquadFilterType| -> [Biquad; 2] {
            [
                Biquad::new(filter_type, f, sr, BUTTERWORTH_Q, 0.0),
                Biquad::new(filter_type, f, sr, BUTTERWORTH_Q, 0.0),
            ]
        };

        let lowpass: Vec<[Biquad; 2]> =
            (0..channels).map(|_| make_pair(BiquadFilterType::Lowpass)).collect();
        let highpass: Vec<[Biquad; 2]> =
            (0..channels).map(|_| make_pair(BiquadFilterType::Highpass)).collect();

        Self {
            lowpass,
            highpass,
            freq,
            sample_rate,
            channels,
        }
    }

    /// Process one sample for a given channel. Returns `(low, high)`.
    #[inline]
    pub fn process(&mut self, sample: f32, channel: usize) -> (f32, f32) {
        let s = sample as f64;

        // Cascade two lowpass stages
        let lp1 = self.lowpass[channel][0].process(s);
        let low = self.lowpass[channel][1].process(lp1) as f32;

        // Cascade two highpass stages
        let hp1 = self.highpass[channel][0].process(s);
        let high = self.highpass[channel][1].process(hp1) as f32;

        (low, high)
    }

    /// Process one interleaved frame. `input` has `channels` samples.
    /// `low` and `high` outputs each have `channels` samples.
    #[inline]
    pub fn process_frame(&mut self, input: &[f32], low: &mut [f32], high: &mut [f32]) {
        debug_assert_eq!(input.len(), self.channels);
        debug_assert_eq!(low.len(), self.channels);
        debug_assert_eq!(high.len(), self.channels);
        for ch in 0..self.channels {
            let (l, h) = self.process(input[ch], ch);
            low[ch] = l;
            high[ch] = h;
        }
    }

    /// Update the crossover frequency. Recomputes all filter coefficients
    /// without resetting filter state (click-free).
    pub fn set_frequency(&mut self, freq: f32) {
        if (freq - self.freq).abs() < 0.1 {
            return;
        }
        self.freq = freq;
        let f = freq as f64;
        let sr = self.sample_rate as f64;

        for ch in 0..self.channels {
            for stage in 0..2 {
                self.lowpass[ch][stage].update_params(
                    BiquadFilterType::Lowpass,
                    f,
                    sr,
                    BUTTERWORTH_Q,
                    0.0,
                );
                self.highpass[ch][stage].update_params(
                    BiquadFilterType::Highpass,
                    f,
                    sr,
                    BUTTERWORTH_Q,
                    0.0,
                );
            }
        }
    }

    /// Reset all filter states to zero by reinitializing filters.
    pub fn reset(&mut self) {
        let f = self.freq as f64;
        let sr = self.sample_rate as f64;
        for ch in 0..self.channels {
            for stage in 0..2 {
                self.lowpass[ch][stage] =
                    Biquad::new(BiquadFilterType::Lowpass, f, sr, BUTTERWORTH_Q, 0.0);
                self.highpass[ch][stage] =
                    Biquad::new(BiquadFilterType::Highpass, f, sr, BUTTERWORTH_Q, 0.0);
            }
        }
    }

    /// Re-initialize for a new sample rate and/or channel count.
    pub fn reinit(&mut self, freq: f32, sample_rate: u32, channels: usize) {
        *self = Self::new(freq, sample_rate, channels);
    }

    pub fn frequency(&self) -> f32 {
        self.freq
    }

    pub fn channels(&self) -> usize {
        self.channels
    }
}

/// A multi-band LR4 crossover that splits a signal into N bands.
///
/// Uses N-1 crossover points to create N frequency bands.
#[derive(Debug, Clone)]
pub struct MultibandLr4Crossover {
    crossovers: Vec<Lr4Crossover>,
    /// Scratch buffer to hold current high-pass carry.
    scratch: Vec<f32>,
    /// Second scratch buffer to avoid per-frame heap allocation.
    carry: Vec<f32>,
}


impl MultibandLr4Crossover {
    /// Create a multiband crossover with the given crossover frequencies.
    ///
    /// Frequencies must be sorted in ascending order. Creates `freqs.len() + 1` bands.
    pub fn new(freqs: &[f32], sample_rate: u32, channels: usize) -> Self {
        let crossovers = freqs
            .iter()
            .map(|&f| Lr4Crossover::new(f, sample_rate, channels))
            .collect();
        Self {
            crossovers,
            scratch: vec![0.0; channels],
            carry: vec![0.0; channels],
        }
    }

    /// Process one interleaved frame into bands.
    ///
    /// `input` has `channels` samples.
    /// `bands` is a slice of band outputs, each having `channels` samples.
    /// `bands.len()` must equal `num_bands()`.
    pub fn process_frame(&mut self, input: &[f32], bands: &mut [&mut [f32]]) {
        debug_assert_eq!(bands.len(), self.num_bands());

        if self.crossovers.is_empty() {
            // Single band — pass through
            bands[0].copy_from_slice(input);
            return;
        }

        // First split: input → low (band 0) + high (carry forward)
        self.crossovers[0].process_frame(input, bands[0], &mut self.scratch);

        // Middle splits: carry → low (band i) + high (carry forward)
        for i in 1..self.crossovers.len() {
            self.carry.copy_from_slice(&self.scratch);
            self.crossovers[i].process_frame(&self.carry, bands[i], &mut self.scratch);
        }

        // Last band is the remaining high
        let last = bands.len() - 1;
        bands[last].copy_from_slice(&self.scratch);
    }

    /// Number of output bands.
    pub fn num_bands(&self) -> usize {
        self.crossovers.len() + 1
    }

    /// Update a crossover frequency by index.
    pub fn set_frequency(&mut self, index: usize, freq: f32) {
        if index < self.crossovers.len() {
            self.crossovers[index].set_frequency(freq);
        }
    }

    pub fn reset(&mut self) {
        for xo in &mut self.crossovers {
            xo.reset();
        }
    }

    pub fn reinit(&mut self, freqs: &[f32], sample_rate: u32, channels: usize) {
        *self = Self::new(freqs, sample_rate, channels);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lr4_basic() {
        let mut xo = Lr4Crossover::new(1000.0, 48000, 1);
        // Process some samples — should not panic
        for i in 0..1000 {
            let sample = (i as f32 * 0.1).sin();
            let (low, high) = xo.process(sample, 0);
            assert!(low.is_finite());
            assert!(high.is_finite());
        }
    }

    #[test]
    fn test_lr4_unity_sum_dc() {
        let mut xo = Lr4Crossover::new(1000.0, 48000, 1);
        // DC signal: should pass entirely through lowpass
        let mut sum_low = 0.0f32;
        let mut sum_high = 0.0f32;
        let n = 10000;
        for _ in 0..n {
            let (low, high) = xo.process(1.0, 0);
            sum_low += low;
            sum_high += high;
        }
        // After settling, low should be near 1.0 and high near 0.0
        let avg_low = sum_low / n as f32;
        let avg_high = sum_high / n as f32;
        assert!(avg_low > 0.9, "avg_low: {}", avg_low);
        assert!(avg_high.abs() < 0.1, "avg_high: {}", avg_high);
    }

    #[test]
    fn test_multiband_three_bands() {
        let mut mb = MultibandLr4Crossover::new(&[500.0, 5000.0], 48000, 1);
        assert_eq!(mb.num_bands(), 3);

        let mut band0 = vec![0.0f32; 1];
        let mut band1 = vec![0.0f32; 1];
        let mut band2 = vec![0.0f32; 1];

        for i in 0..10000 {
            let sample = (i as f32 * 0.01).sin();
            let input = [sample];
            mb.process_frame(
                &input,
                &mut [&mut band0[..], &mut band1[..], &mut band2[..]],
            );
        }
        // Just verify no NaN/Inf
        assert!(band0[0].is_finite());
        assert!(band1[0].is_finite());
        assert!(band2[0].is_finite());
    }

    #[test]
    fn test_set_frequency() {
        let mut xo = Lr4Crossover::new(1000.0, 48000, 2);
        xo.set_frequency(2000.0);
        assert!((xo.frequency() - 2000.0).abs() < 0.1);
    }

    #[test]
    fn test_reset() {
        let mut xo = Lr4Crossover::new(1000.0, 48000, 1);
        for _ in 0..100 {
            xo.process(1.0, 0);
        }
        xo.reset();
        // After reset, first output should be near zero (filter state cleared)
        let (low, high) = xo.process(0.0, 0);
        assert!(low.abs() < 0.001);
        assert!(high.abs() < 0.001);
    }
}
