// ============================================================================
// LR4 Crossover — Linkwitz-Riley 4th-order crossover utility
// ============================================================================

use crate::traits::{FilterFloat, lit};
use crate::{Biquad, BiquadFilterType};

/// Standard multiband crossover presets: (freq1, freq2, freq3, freq4).
/// Shared by multiband compressor and expander.
pub const CROSSOVER_PRESETS: &[(f32, f32, f32, f32)] = &[
    (200.0, 2000.0, 8000.0, 12000.0),
    (100.0, 3000.0, 8000.0, 12000.0),
    (250.0, 4000.0, 10000.0, 14000.0),
];

/// A single Linkwitz-Riley 4th-order crossover point.
///
/// Generic over [`FilterFloat`] (`f32` or `f64`, default `f64`).
///
/// Splits a signal into low and high bands with -24 dB/octave slopes.
/// The two outputs sum to unity (flat magnitude response) with zero
/// phase shift at the crossover frequency.
#[derive(Debug, Clone)]
pub struct Lr4Crossover<T: FilterFloat = f64> {
    /// Two cascaded lowpass biquads per channel.
    lowpass: Vec<[Biquad<T>; 2]>,
    /// Two cascaded highpass biquads per channel.
    highpass: Vec<[Biquad<T>; 2]>,
    freq: T,
    sample_rate: T,
    channels: usize,
}

impl<T: FilterFloat> Lr4Crossover<T> {
    /// Create a new LR4 crossover at the given frequency.
    pub fn new(freq: T, sample_rate: T, channels: usize) -> Self {
        let butterworth_q: T = T::FRAC_1_SQRT_2();

        let make_pair = |filter_type: BiquadFilterType| -> [Biquad<T>; 2] {
            [
                Biquad::new(filter_type, freq, sample_rate, butterworth_q, T::zero()),
                Biquad::new(filter_type, freq, sample_rate, butterworth_q, T::zero()),
            ]
        };

        let lowpass: Vec<[Biquad<T>; 2]> = (0..channels)
            .map(|_| make_pair(BiquadFilterType::Lowpass))
            .collect();
        let highpass: Vec<[Biquad<T>; 2]> = (0..channels)
            .map(|_| make_pair(BiquadFilterType::Highpass))
            .collect();

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
    pub fn process(&mut self, sample: T, channel: usize) -> (T, T) {
        // Cascade two lowpass stages
        let lp1 = self.lowpass[channel][0].process(sample);
        let low = self.lowpass[channel][1].process(lp1);

        // Cascade two highpass stages
        let hp1 = self.highpass[channel][0].process(sample);
        let high = self.highpass[channel][1].process(hp1);

        (low, high)
    }

    /// Process one interleaved frame. `input` has `channels` samples.
    /// `low` and `high` outputs each have `channels` samples.
    #[inline]
    pub fn process_frame(&mut self, input: &[T], low: &mut [T], high: &mut [T]) {
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
    pub fn set_frequency(&mut self, freq: T) {
        if (freq - self.freq).abs() < lit(0.001) {
            return;
        }
        self.freq = freq;
        let butterworth_q: T = T::FRAC_1_SQRT_2();

        for ch in 0..self.channels {
            for stage in 0..2 {
                self.lowpass[ch][stage].update_params(
                    BiquadFilterType::Lowpass,
                    freq,
                    self.sample_rate,
                    butterworth_q,
                    T::zero(),
                );
                self.highpass[ch][stage].update_params(
                    BiquadFilterType::Highpass,
                    freq,
                    self.sample_rate,
                    butterworth_q,
                    T::zero(),
                );
            }
        }
    }

    /// Reset all filter states to zero by reinitializing filters.
    pub fn reset(&mut self) {
        let butterworth_q: T = T::FRAC_1_SQRT_2();
        for ch in 0..self.channels {
            for stage in 0..2 {
                self.lowpass[ch][stage] = Biquad::new(
                    BiquadFilterType::Lowpass,
                    self.freq,
                    self.sample_rate,
                    butterworth_q,
                    T::zero(),
                );
                self.highpass[ch][stage] = Biquad::new(
                    BiquadFilterType::Highpass,
                    self.freq,
                    self.sample_rate,
                    butterworth_q,
                    T::zero(),
                );
            }
        }
    }

    /// Re-initialize for a new sample rate and/or channel count.
    pub fn reinit(&mut self, freq: T, sample_rate: T, channels: usize) {
        *self = Self::new(freq, sample_rate, channels);
    }

    /// Get the crossover frequency.
    pub fn frequency(&self) -> T {
        self.freq
    }

    /// Get the number of channels.
    pub fn channels(&self) -> usize {
        self.channels
    }
}

/// A multi-band LR4 crossover that splits a signal into N bands.
///
/// Generic over [`FilterFloat`] (`f32` or `f64`, default `f64`).
/// Uses N-1 crossover points to create N frequency bands.
///
/// Note that cascaded LR4 bands are amplitude-complementary, but their group
/// delays are not identical: lower bands pass through different pole sets than
/// upper bands. Summing the bands is suitable for broad crossover work, but it
/// is not a phase-perfect linear-phase split.
#[derive(Debug, Clone)]
pub struct MultibandLr4Crossover<T: FilterFloat = f64> {
    crossovers: Vec<Lr4Crossover<T>>,
    /// Scratch buffer to hold current high-pass carry.
    scratch: Vec<T>,
    /// Second scratch buffer to avoid per-frame heap allocation.
    carry: Vec<T>,
}

impl<T: FilterFloat> MultibandLr4Crossover<T> {
    /// Create a multiband crossover with the given crossover frequencies.
    ///
    /// Frequencies must be sorted in ascending order. Creates `freqs.len() + 1` bands.
    pub fn new(freqs: &[T], sample_rate: T, channels: usize) -> Self {
        let crossovers = freqs
            .iter()
            .map(|&f| Lr4Crossover::new(f, sample_rate, channels))
            .collect();
        Self {
            crossovers,
            scratch: vec![T::zero(); channels],
            carry: vec![T::zero(); channels],
        }
    }

    /// Process one interleaved frame into bands.
    ///
    /// `input` has `channels` samples.
    /// `bands` is a slice of band outputs, each having `channels` samples.
    /// `bands.len()` must equal `num_bands()`.
    pub fn process_frame(&mut self, input: &[T], bands: &mut [&mut [T]]) {
        debug_assert_eq!(bands.len(), self.num_bands());

        if self.crossovers.is_empty() {
            // Single band — pass through
            bands[0].copy_from_slice(input);
            return;
        }

        // First split: input → low (band 0) + high (carry forward)
        self.crossovers[0].process_frame(input, bands[0], &mut self.scratch);

        // Middle splits: carry → low (band i) + high (carry forward)
        #[allow(clippy::needless_range_loop)] // Can't use iterators: multiple &mut self fields
        for i in 1..self.crossovers.len() {
            std::mem::swap(&mut self.carry, &mut self.scratch);
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
    pub fn set_frequency(&mut self, index: usize, freq: T) {
        if index < self.crossovers.len() {
            self.crossovers[index].set_frequency(freq);
        }
    }

    /// Reset all crossover filter states.
    pub fn reset(&mut self) {
        for xo in &mut self.crossovers {
            xo.reset();
        }
    }

    /// Re-initialize for new frequencies, sample rate, and/or channel count.
    pub fn reinit(&mut self, freqs: &[T], sample_rate: T, channels: usize) {
        *self = Self::new(freqs, sample_rate, channels);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lr4_basic() {
        let mut xo = Lr4Crossover::new(1000.0, 48000.0, 1);
        for i in 0..1000 {
            let sample = (i as f64 * 0.1).sin();
            let (low, high) = xo.process(sample, 0);
            assert!(low.is_finite());
            assert!(high.is_finite());
        }
    }

    #[test]
    fn test_lr4_unity_sum_dc() {
        let mut xo = Lr4Crossover::new(1000.0, 48000.0, 1);
        let mut sum_low = 0.0f64;
        let mut sum_high = 0.0f64;
        let n = 10000;
        for _ in 0..n {
            let (low, high) = xo.process(1.0, 0);
            sum_low += low;
            sum_high += high;
        }
        let avg_low = sum_low / n as f64;
        let avg_high = sum_high / n as f64;
        assert!(avg_low > 0.9, "avg_low: {}", avg_low);
        assert!(avg_high.abs() < 0.1, "avg_high: {}", avg_high);
    }

    #[test]
    fn test_multiband_three_bands() {
        let mut mb = MultibandLr4Crossover::new(&[500.0, 5000.0], 48000.0, 1);
        assert_eq!(mb.num_bands(), 3);

        let mut band0 = [0.0f64; 1];
        let mut band1 = [0.0f64; 1];
        let mut band2 = [0.0f64; 1];

        for i in 0..10000 {
            let sample = (i as f64 * 0.01).sin();
            let input = [sample];
            mb.process_frame(
                &input,
                &mut [&mut band0[..], &mut band1[..], &mut band2[..]],
            );
        }
        assert!(band0[0].is_finite());
        assert!(band1[0].is_finite());
        assert!(band2[0].is_finite());
    }

    #[test]
    fn test_set_frequency() {
        let mut xo = Lr4Crossover::new(1000.0_f64, 48000.0, 2);
        xo.set_frequency(2000.0);
        assert!((xo.frequency() - 2000.0).abs() < 0.1);
    }

    #[test]
    fn test_set_frequency_small_changes_are_not_ignored() {
        let mut xo = Lr4Crossover::new(1000.0_f64, 48000.0, 2);
        xo.set_frequency(1000.05);
        assert!(
            (xo.frequency() - 1000.05).abs() < 0.001,
            "small frequency changes (< 0.1 Hz) should not be ignored, got freq={}",
            xo.frequency()
        );
    }

    #[test]
    fn test_reset() {
        let mut xo = Lr4Crossover::new(1000.0_f64, 48000.0, 1);
        for _ in 0..100 {
            xo.process(1.0, 0);
        }
        xo.reset();
        let (low, high) = xo.process(0.0, 0);
        assert!(low.abs() < 0.001);
        assert!(high.abs() < 0.001);
    }

    #[test]
    fn test_lr4_f32() {
        let mut xo = Lr4Crossover::<f32>::new(1000.0, 48000.0, 1);
        let (low, high) = xo.process(1.0f32, 0);
        assert!(low.is_finite());
        assert!(high.is_finite());
    }

    #[test]
    fn test_set_frequency_no_op_small_delta() {
        let mut xo = Lr4Crossover::new(1000.0_f64, 48000.0, 2);
        let freq_before = xo.frequency();
        xo.set_frequency(1000.0005);
        // Difference is < 0.001, so should be a no-op
        assert!((xo.frequency() - freq_before).abs() < 1e-9);
    }

    #[test]
    fn test_set_frequency_updates_coefficients() {
        let mut xo = Lr4Crossover::new(1000.0_f64, 48000.0, 1);
        xo.set_frequency(2000.0);
        // After settling with DC, lowpass should still pass DC
        let mut low = 0.0;
        for _ in 0..2000 {
            let (l, _h) = xo.process(1.0, 0);
            low = l;
        }
        assert!(
            low > 0.9,
            "lowpass should still pass DC after frequency change, got {}",
            low
        );
    }

    #[test]
    fn test_multiband_set_frequency() {
        let mut mb = MultibandLr4Crossover::new(&[500.0, 5000.0], 48000.0, 1);
        mb.set_frequency(0, 800.0);
        mb.set_frequency(1, 6000.0);
        // Process some frames to verify no panic
        let mut band0 = [0.0f64; 1];
        let mut band1 = [0.0f64; 1];
        let mut band2 = [0.0f64; 1];
        for i in 0..100 {
            let sample = (i as f64 * 0.01).sin();
            mb.process_frame(
                &[sample],
                &mut [&mut band0[..], &mut band1[..], &mut band2[..]],
            );
        }
        assert!(band0[0].is_finite());
        assert!(band1[0].is_finite());
        assert!(band2[0].is_finite());
    }

    #[test]
    fn test_set_frequency_out_of_range_index() {
        let mut mb = MultibandLr4Crossover::new(&[500.0], 48000.0, 1);
        // Index out of range should be silently ignored
        mb.set_frequency(5, 1000.0);
        let mut band0 = [0.0f64; 1];
        let mut band1 = [0.0f64; 1];
        mb.process_frame(&[1.0], &mut [&mut band0[..], &mut band1[..]]);
        assert!(band0[0].is_finite());
        assert!(band1[0].is_finite());
    }
}
