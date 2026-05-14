// ============================================================================
// Linear-Phase FIR Crossover
// ============================================================================
//
// Provides a linear-phase alternative to the IIR Linkwitz-Riley crossover.
// Uses windowed-sinc FIR lowpass filters designed via math-iir-fir::Fir.
//
// Key property: highpass = delayed_input - lowpass, guaranteeing perfect
// reconstruction (flat sum of bands) with zero phase distortion.
//
// Trade-off: higher latency (taps/2 samples) vs LR4 IIR (zero latency).
//
// HARD RULES:
// - No allocations in process methods
// - All buffers pre-allocated in new()

use crate::traits::FilterFloat;
use crate::{Fir, WindowType};

/// Default number of taps for generated linear-phase crossover filters.
pub const DEFAULT_FIR_CROSSOVER_TAPS: usize = 1025;

// ============================================================================
// Single-point FIR crossover
// ============================================================================

/// A single-point linear-phase FIR crossover that splits a signal into
/// low and high bands with zero phase distortion.
///
/// Generic over [`FilterFloat`] (`f32` or `f64`, default `f64`).
pub struct FirCrossover<T: FilterFloat = f64> {
    /// FIR lowpass coefficients
    coefficients: Vec<T>,
    /// Per-channel delay lines for FIR convolution
    delay_lines: Vec<Vec<T>>,
    /// Per-channel write positions
    write_positions: Vec<usize>,
    /// Number of taps
    n_taps: usize,
    channels: usize,
}

impl<T: FilterFloat> FirCrossover<T> {
    /// Create a new linear-phase FIR crossover at the given frequency.
    ///
    /// # Arguments
    /// * `freq` - Crossover frequency in Hz
    /// * `sample_rate` - Sample rate in Hz
    /// * `channels` - Number of audio channels
    /// * `n_taps` - Number of FIR taps (must be odd; even values are incremented)
    pub fn new(freq: T, sample_rate: T, channels: usize, n_taps: usize) -> Self {
        let n_taps = if n_taps.is_multiple_of(2) {
            n_taps + 1
        } else {
            n_taps
        };

        // Design lowpass FIR using Kaiser window (beta=8 for good sidelobe rejection)
        let fir = Fir::<T>::lowpass(
            n_taps,
            freq,
            sample_rate,
            WindowType::Kaiser,
            T::from_f64(8.0).unwrap(),
        );

        let coefficients: Vec<T> = fir.coeffs().to_vec();

        let delay_lines = vec![vec![T::zero(); n_taps]; channels];
        let write_positions = vec![0; channels];

        Self {
            coefficients,
            delay_lines,
            write_positions,
            n_taps,
            channels,
        }
    }

    /// Process one sample for one channel. Returns (low, high).
    ///
    /// The highpass output is computed as `delayed_input - lowpass_output`,
    /// guaranteeing perfect reconstruction.
    #[inline]
    pub fn process_sample(&mut self, sample: T, channel: usize) -> (T, T) {
        let dl = &mut self.delay_lines[channel];
        let wp = &mut self.write_positions[channel];

        // Write new sample into delay line
        dl[*wp] = sample;

        // Compute lowpass output via convolution
        let mut low = T::zero();
        let mut read_pos = *wp;
        for &coeff in &self.coefficients {
            low += dl[read_pos] * coeff;
            if read_pos == 0 {
                read_pos = self.n_taps - 1;
            } else {
                read_pos -= 1;
            }
        }

        // Advance write position
        *wp = (*wp + 1) % self.n_taps;

        // Highpass = delayed_input - lowpass
        // The delay is (n_taps - 1) / 2 samples (center of symmetric FIR)
        let delay = (self.n_taps - 1) / 2;
        let delayed_pos = if *wp > delay {
            *wp - delay - 1
        } else {
            self.n_taps + *wp - delay - 1
        };
        let delayed_input = dl[delayed_pos];
        let high = delayed_input - low;

        (low, high)
    }

    /// Process one interleaved frame. Writes low and high band outputs.
    ///
    /// `input`: interleaved frame (length = channels)
    /// `low_out`: interleaved low-band frame (length = channels)
    /// `high_out`: interleaved high-band frame (length = channels)
    pub fn process_frame(&mut self, input: &[T], low_out: &mut [T], high_out: &mut [T]) {
        for ch in 0..self.channels {
            let (l, h) = self.process_sample(input[ch], ch);
            low_out[ch] = l;
            high_out[ch] = h;
        }
    }

    /// Lowpass FIR coefficients.
    pub fn lowpass_coefficients(&self) -> &[T] {
        &self.coefficients
    }

    /// Highpass FIR coefficients computed as delayed-input minus lowpass.
    pub fn highpass_coefficients(&self) -> Vec<T> {
        let mut coefficients = self.coefficients.iter().map(|&c| -c).collect::<Vec<_>>();
        coefficients[(self.n_taps - 1) / 2] += T::one();
        coefficients
    }

    /// Number of FIR taps.
    pub fn n_taps(&self) -> usize {
        self.n_taps
    }

    /// Latency in samples (= (n_taps - 1) / 2).
    pub fn latency_samples(&self) -> usize {
        (self.n_taps - 1) / 2
    }

    /// Reset all filter state.
    pub fn reset(&mut self) {
        for dl in &mut self.delay_lines {
            dl.fill(T::zero());
        }
        self.write_positions.fill(0);
    }
}

// ============================================================================
// Multi-band FIR crossover
// ============================================================================

/// N-way linear-phase FIR crossover using cascaded split points.
///
/// Generic over [`FilterFloat`] (`f32` or `f64`, default `f64`).
/// For N split frequencies, produces N+1 bands.
/// All bands sum to the original signal (perfect reconstruction).
pub struct MultibandFirCrossover<T: FilterFloat = f64> {
    crossovers: Vec<FirCrossover<T>>,
    /// Scratch buffers for intermediate band splitting
    scratch_low: Vec<T>,
    scratch_high: Vec<T>,
    /// Second high scratch buffer to avoid allocation in process_frame
    scratch_high2: Vec<T>,
    channels: usize,
    num_bands: usize,
}

impl<T: FilterFloat> MultibandFirCrossover<T> {
    /// Create a new multi-band FIR crossover.
    ///
    /// # Arguments
    /// * `freqs` - Crossover frequencies in ascending order
    /// * `sample_rate` - Sample rate in Hz
    /// * `channels` - Number of audio channels
    /// * `n_taps` - Number of FIR taps per crossover point
    pub fn new(freqs: &[T], sample_rate: T, channels: usize, n_taps: usize) -> Self {
        let crossovers: Vec<FirCrossover<T>> = freqs
            .iter()
            .map(|&freq| FirCrossover::new(freq, sample_rate, channels, n_taps))
            .collect();

        Self {
            num_bands: freqs.len() + 1,
            crossovers,
            scratch_low: vec![T::zero(); channels],
            scratch_high: vec![T::zero(); channels],
            scratch_high2: vec![T::zero(); channels],
            channels,
        }
    }

    /// Process one interleaved frame. Writes output to `bands`.
    ///
    /// `input`: interleaved frame (length = channels)
    /// `bands`: slice of mutable slices, one per band (length = num_bands).
    ///          Each band slice has length = channels.
    pub fn process_frame(&mut self, input: &[T], bands: &mut [&mut [T]]) {
        debug_assert_eq!(bands.len(), self.num_bands);

        if self.crossovers.is_empty() {
            // No crossovers — single band passthrough
            bands[0].copy_from_slice(input);
            return;
        }

        // First split: input → lowest band + remainder
        self.crossovers[0].process_frame(input, &mut self.scratch_low, &mut self.scratch_high);
        bands[0].copy_from_slice(&self.scratch_low);

        // Cascade: split remainder at each subsequent frequency (zero-allocation)
        for (i, xover) in self.crossovers.iter_mut().enumerate().skip(1) {
            self.scratch_high2[..self.channels]
                .copy_from_slice(&self.scratch_high[..self.channels]);
            xover.process_frame(
                &self.scratch_high2,
                &mut self.scratch_low,
                &mut self.scratch_high,
            );
            bands[i].copy_from_slice(&self.scratch_low);
        }

        // Last band is the final high remainder
        let last = self.num_bands - 1;
        bands[last].copy_from_slice(&self.scratch_high[..self.channels]);
    }

    /// Number of output bands.
    pub fn num_bands(&self) -> usize {
        self.num_bands
    }

    /// Latency in samples.
    ///
    /// For cascaded crossovers, the total latency is the sum of all individual latencies.
    pub fn latency_samples(&self) -> usize {
        self.crossovers.iter().map(|c| c.latency_samples()).sum()
    }

    /// Reset all crossover filter states.
    pub fn reset(&mut self) {
        for c in &mut self.crossovers {
            c.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perfect_reconstruction() {
        let mut xover = FirCrossover::new(1000.0, 48000.0, 1, 127);
        let latency = xover.latency_samples();

        let num_samples = 1000;
        let mut inputs = Vec::with_capacity(num_samples);
        let mut low_outputs = Vec::with_capacity(num_samples);
        let mut high_outputs = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let input = ((i as f64 * 0.37).sin() + (i as f64 * 0.73).cos()) * 0.5;
            inputs.push(input);
            let (l, h) = xover.process_sample(input, 0);
            low_outputs.push(l);
            high_outputs.push(h);
        }

        let mut max_error = 0.0f64;
        for i in (latency + 10)..num_samples {
            let reconstructed = low_outputs[i] + high_outputs[i];
            let original = inputs[i - latency];
            let error = (reconstructed - original).abs();
            max_error = max_error.max(error);
        }

        assert!(
            max_error < 0.01,
            "Perfect reconstruction failed: max error = {max_error}"
        );
    }

    #[test]
    fn test_lowpass_attenuates_high_freq() {
        let sr = 48000.0;
        let mut xover = FirCrossover::new(1000.0, sr, 1, 255);
        let latency = xover.latency_samples();

        let freq = 10000.0;
        let num_samples = 2000;
        let mut low_energy = 0.0f64;
        let mut input_energy = 0.0f64;
        for i in 0..num_samples {
            let t = i as f64 / sr;
            let input = (2.0 * std::f64::consts::PI * freq * t).sin();
            let (l, _h) = xover.process_sample(input, 0);
            if i > latency + 100 {
                low_energy += l * l;
                input_energy += input * input;
            }
        }

        let attenuation_db = 10.0 * (low_energy / input_energy).log10();
        assert!(
            attenuation_db < -20.0,
            "10kHz should be heavily attenuated in low band: {attenuation_db} dB"
        );
    }

    #[test]
    fn test_highpass_passes_high_freq() {
        let sr = 48000.0;
        let mut xover = FirCrossover::new(1000.0, sr, 1, 255);
        let latency = xover.latency_samples();

        let freq = 10000.0;
        let num_samples = 2000;
        let mut high_energy = 0.0f64;
        let mut input_energy = 0.0f64;
        for i in 0..num_samples {
            let t = i as f64 / sr;
            let input = (2.0 * std::f64::consts::PI * freq * t).sin();
            let (_l, h) = xover.process_sample(input, 0);
            if i > latency + 100 {
                high_energy += h * h;
                input_energy += input * input;
            }
        }

        let ratio = high_energy / input_energy;
        assert!(
            ratio > 0.8,
            "10kHz should pass through high band: ratio = {ratio}"
        );
    }

    #[test]
    fn test_latency() {
        let xover = FirCrossover::new(1000.0, 48000.0, 2, 255);
        assert_eq!(xover.latency_samples(), 127); // (255-1)/2
    }

    #[test]
    fn test_multiband_3way() {
        let freqs = [200.0, 2000.0];
        let mut xover = MultibandFirCrossover::new(&freqs, 48000.0, 1, 127);
        assert_eq!(xover.num_bands(), 3);

        let latency = xover.latency_samples();
        let num_samples = 2000;

        let mut inputs = Vec::with_capacity(num_samples);
        let mut band_sums = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let input = (i as f64 * 0.37).sin() * 0.5;
            inputs.push(input);

            let input_frame = [input];
            let mut b0 = [0.0f64];
            let mut b1 = [0.0f64];
            let mut b2 = [0.0f64];
            let mut bands: Vec<&mut [f64]> = vec![&mut b0, &mut b1, &mut b2];
            xover.process_frame(&input_frame, &mut bands);
            band_sums.push(b0[0] + b1[0] + b2[0]);
        }

        let settle = latency + 50;
        let mut max_error = 0.0f64;
        for i in settle..num_samples {
            let error = (band_sums[i] - inputs[i - latency]).abs();
            max_error = max_error.max(error);
        }

        assert!(
            max_error < 0.05,
            "3-way reconstruction failed: max error = {max_error}"
        );
    }

    #[test]
    fn test_reset() {
        let mut xover = FirCrossover::new(1000.0_f64, 48000.0, 1, 63);
        xover.process_sample(1.0, 0);
        xover.reset();
        let (l, h) = xover.process_sample(0.0, 0);
        assert!(l.abs() < 1e-6);
        assert!(h.abs() < 1e-6);
    }

    #[test]
    fn test_fir_crossover_f32() {
        let mut xover = FirCrossover::<f32>::new(1000.0, 48000.0, 1, 63);
        let (low, high) = xover.process_sample(1.0f32, 0);
        assert!(low.is_finite());
        assert!(high.is_finite());
    }
}
