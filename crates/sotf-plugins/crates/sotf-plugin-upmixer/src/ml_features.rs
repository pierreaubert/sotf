// ============================================================================
// MFCC Feature Extraction for ML Vocal Detection
// ============================================================================
//
// Extracts 40-element feature vectors (20 MFCCs + 20 deltas) from existing
// FFT frequency-domain data. Zero allocation in the compute() hot path.

use rustfft::num_complex::Complex;

/// Number of mel filterbank bands
const NUM_MEL_BANDS: usize = 40;

/// Number of MFCC coefficients to keep
const NUM_MFCCS: usize = 20;

/// Total feature vector size: MFCCs + delta MFCCs
pub const FEATURE_SIZE: usize = NUM_MFCCS + NUM_MFCCS;

/// Sparse triangular mel filter: (bin_index, weight) pairs for each mel band
struct MelFilter {
    /// Start index in the flat weights array
    offset: usize,
    /// Number of (bin, weight) pairs
    len: usize,
}

/// MFCC feature extractor that reuses existing FFT data.
///
/// Pre-computes mel filterbank and DCT-II matrix at construction time.
/// The `compute()` method produces a 40-element feature vector with zero allocations.
pub struct MfccExtractor {
    /// Sparse mel filterbank: flat array of (bin_index, weight)
    filter_weights: Vec<(usize, f32)>,
    /// Per-band metadata into filter_weights
    mel_filters: Vec<MelFilter>,
    /// DCT-II matrix [NUM_MFCCS x NUM_MEL_BANDS], row-major
    dct_matrix: Vec<f32>,
    /// Intermediate: mel band energies
    mel_energies: Vec<f32>,
    /// Intermediate: log mel energies
    log_mel_energies: Vec<f32>,
    /// Output feature buffer [FEATURE_SIZE]
    features: Vec<f32>,
    /// Previous frame MFCCs for delta computation
    prev_mfccs: Vec<f32>,
    /// Whether we have a previous frame (for delta computation)
    has_prev: bool,
}

impl MfccExtractor {
    /// Create a new MFCC extractor.
    ///
    /// Pre-computes the mel filterbank (HTK scale, triangular filters) and
    /// DCT-II matrix. All intermediate buffers are pre-allocated.
    pub fn new(sample_rate: u32, fft_size: usize) -> Self {
        let spectrum_size = fft_size / 2 + 1;
        let nyquist = sample_rate as f32 / 2.0;

        // HTK mel scale
        let mel_low = hz_to_mel(0.0);
        let mel_high = hz_to_mel(nyquist);

        // Equally spaced mel points (NUM_MEL_BANDS + 2 for triangular filter edges)
        let num_points = NUM_MEL_BANDS + 2;
        let mel_points: Vec<f32> = (0..num_points)
            .map(|i| mel_low + (mel_high - mel_low) * i as f32 / (num_points - 1) as f32)
            .collect();

        let hz_points: Vec<f32> = mel_points.iter().map(|&m| mel_to_hz(m)).collect();

        // Convert Hz to FFT bin indices (fractional)
        let bin_points: Vec<f32> = hz_points
            .iter()
            .map(|&f| f * fft_size as f32 / sample_rate as f32)
            .collect();

        // Build sparse triangular filters
        let mut filter_weights = Vec::new();
        let mut mel_filters = Vec::with_capacity(NUM_MEL_BANDS);

        for band in 0..NUM_MEL_BANDS {
            let left = bin_points[band];
            let center = bin_points[band + 1];
            let right = bin_points[band + 2];

            let bin_start = left.floor() as usize;
            let bin_end = (right.ceil() as usize).min(spectrum_size - 1);

            let offset = filter_weights.len();
            let mut count = 0;

            for bin in bin_start..=bin_end {
                let bin_f = bin as f32;
                let weight = if bin_f <= left {
                    0.0
                } else if bin_f <= center {
                    (bin_f - left) / (center - left)
                } else if bin_f <= right {
                    (right - bin_f) / (right - center)
                } else {
                    0.0
                };
                if weight > 0.0 {
                    filter_weights.push((bin, weight));
                    count += 1;
                }
            }

            mel_filters.push(MelFilter { offset, len: count });
        }

        // Pre-compute DCT-II matrix: dct[k][n] = cos(PI * k * (n + 0.5) / N)
        let mut dct_matrix = vec![0.0_f32; NUM_MFCCS * NUM_MEL_BANDS];
        for k in 0..NUM_MFCCS {
            for n in 0..NUM_MEL_BANDS {
                dct_matrix[k * NUM_MEL_BANDS + n] =
                    (std::f32::consts::PI * k as f32 * (n as f32 + 0.5) / NUM_MEL_BANDS as f32)
                        .cos();
            }
        }

        Self {
            filter_weights,
            mel_filters,
            dct_matrix,
            mel_energies: vec![0.0; NUM_MEL_BANDS],
            log_mel_energies: vec![0.0; NUM_MEL_BANDS],
            features: vec![0.0; FEATURE_SIZE],
            prev_mfccs: vec![0.0; NUM_MFCCS],
            has_prev: false,
        }
    }

    /// Compute MFCC features from existing FFT frequency-domain data.
    ///
    /// Takes the left and right channel complex spectra (from the forward FFT)
    /// and produces a 40-element feature vector: 20 MFCCs + 20 delta MFCCs.
    ///
    /// Zero allocation — all buffers are pre-allocated.
    #[inline]
    pub fn compute(
        &mut self,
        freq_left: &[Complex<f32>],
        freq_right: &[Complex<f32>],
    ) -> &[f32; FEATURE_SIZE] {
        // Step 1: Compute mono power spectrum from existing complex bins
        // P[i] = 0.5 * (|L[i]|^2 + |R[i]|^2)

        // Step 2: Apply mel filterbank (sparse dot products)
        for (band_idx, filter) in self.mel_filters.iter().enumerate() {
            let mut energy = 0.0_f32;
            let pairs = &self.filter_weights[filter.offset..filter.offset + filter.len];
            for &(bin, weight) in pairs {
                let power = (freq_left[bin].norm_sqr() + freq_right[bin].norm_sqr()) * 0.5;
                energy += power * weight;
            }
            self.mel_energies[band_idx] = energy;
        }

        // Step 3: Log compression (with floor to avoid log(0))
        const LOG_FLOOR: f32 = 1e-10;
        for i in 0..NUM_MEL_BANDS {
            self.log_mel_energies[i] = (self.mel_energies[i] + LOG_FLOOR).ln();
        }

        // Step 4: DCT-II to get MFCCs
        for k in 0..NUM_MFCCS {
            let row = &self.dct_matrix[k * NUM_MEL_BANDS..(k + 1) * NUM_MEL_BANDS];
            let mut sum = 0.0_f32;
            for (n, &dct_coeff) in row.iter().enumerate() {
                sum += dct_coeff * self.log_mel_energies[n];
            }
            self.features[k] = sum;
        }

        // Step 5: Delta MFCCs (first-order difference with previous frame)
        if self.has_prev {
            for k in 0..NUM_MFCCS {
                self.features[NUM_MFCCS + k] = self.features[k] - self.prev_mfccs[k];
            }
        } else {
            // First frame: deltas are zero
            for k in 0..NUM_MFCCS {
                self.features[NUM_MFCCS + k] = 0.0;
            }
        }

        // Save current MFCCs for next frame's delta computation
        self.prev_mfccs.copy_from_slice(&self.features[..NUM_MFCCS]);
        self.has_prev = true;

        // SAFETY: features is always FEATURE_SIZE elements
        self.features[..FEATURE_SIZE].try_into().unwrap()
    }

    /// Reset state (e.g., on stream discontinuity)
    pub fn reset(&mut self) {
        self.has_prev = false;
        self.prev_mfccs.fill(0.0);
        self.features.fill(0.0);
    }
}

/// Convert frequency in Hz to mel scale (HTK formula)
#[inline]
fn hz_to_mel(f: f32) -> f32 {
    2595.0 * (1.0 + f / 700.0).log10()
}

/// Convert mel scale back to Hz
#[inline]
fn mel_to_hz(m: f32) -> f32 {
    700.0 * (10.0_f32.powf(m / 2595.0) - 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mfcc_extractor_basic() {
        let sample_rate = 44100;
        let fft_size = 2048;
        let spectrum_size = fft_size / 2 + 1;
        let mut extractor = MfccExtractor::new(sample_rate, fft_size);

        // Create a simple test signal: energy concentrated in voice band (~1kHz)
        let mut freq_left = vec![Complex::new(0.0, 0.0); spectrum_size];
        let mut freq_right = vec![Complex::new(0.0, 0.0); spectrum_size];

        // Put energy around 1kHz (bin ~47 at 44100/2048)
        let target_bin = (1000.0 * fft_size as f32 / sample_rate as f32) as usize;
        for i in target_bin.saturating_sub(5)..=(target_bin + 5).min(spectrum_size - 1) {
            freq_left[i] = Complex::new(1.0, 0.0);
            freq_right[i] = Complex::new(1.0, 0.0);
        }

        let features = extractor.compute(&freq_left, &freq_right);
        assert_eq!(features.len(), FEATURE_SIZE);

        // First frame: deltas should be zero
        for &feat in &features[NUM_MFCCS..FEATURE_SIZE] {
            assert_eq!(feat, 0.0, "First frame deltas should be zero");
        }

        // MFCCs should be non-zero (we have energy in the signal)
        let mfcc_energy: f32 = features[..NUM_MFCCS].iter().map(|x| x * x).sum();
        assert!(
            mfcc_energy > 0.0,
            "MFCCs should be non-zero for non-silent signal"
        );

        // Second frame: deltas should be non-zero if input differs
        freq_left[target_bin] = Complex::new(2.0, 0.0);
        freq_right[target_bin] = Complex::new(2.0, 0.0);
        let features2 = extractor.compute(&freq_left, &freq_right);
        let delta_energy: f32 = features2[NUM_MFCCS..].iter().map(|x| x * x).sum();
        assert!(
            delta_energy > 0.0,
            "Deltas should be non-zero when input changes"
        );
    }

    #[test]
    fn test_mfcc_extractor_silent_input() {
        let mut extractor = MfccExtractor::new(44100, 2048);
        let spectrum_size = 2048 / 2 + 1;
        let freq = vec![Complex::new(0.0, 0.0); spectrum_size];

        let features = extractor.compute(&freq, &freq);
        // All features should be finite
        for &f in features.iter() {
            assert!(f.is_finite(), "Feature must be finite, got {}", f);
        }
    }

    #[test]
    fn test_mfcc_extractor_reset() {
        let mut extractor = MfccExtractor::new(44100, 2048);
        let spectrum_size = 2048 / 2 + 1;
        let freq = vec![Complex::new(1.0, 0.0); spectrum_size];

        // Compute one frame to set has_prev
        let _ = extractor.compute(&freq, &freq);
        assert!(extractor.has_prev);

        extractor.reset();
        assert!(!extractor.has_prev);

        // After reset, deltas should be zero again
        let features = extractor.compute(&freq, &freq);
        for &feat in &features[NUM_MFCCS..FEATURE_SIZE] {
            assert_eq!(feat, 0.0, "Post-reset deltas should be zero");
        }
    }

    #[test]
    fn test_mel_scale_roundtrip() {
        for &freq in &[0.0, 100.0, 1000.0, 5000.0, 10000.0, 20000.0] {
            let mel = hz_to_mel(freq);
            let hz = mel_to_hz(mel);
            assert!(
                (hz - freq).abs() < 0.01,
                "Mel roundtrip failed for {} Hz: got {} Hz",
                freq,
                hz
            );
        }
    }

    #[test]
    fn test_mel_filterbank_coverage() {
        let extractor = MfccExtractor::new(44100, 2048);

        // All filters should have non-zero weights
        for (i, filter) in extractor.mel_filters.iter().enumerate() {
            assert!(filter.len > 0, "Mel filter band {} has no weights", i);
        }
    }
}
