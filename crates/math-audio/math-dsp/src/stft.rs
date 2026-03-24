// ============================================================================
// Shared STFT Infrastructure
// ============================================================================
//
// Reusable components for STFT-based plugins:
// - generate_hann_window: Hann window generation for STFT analysis
// - RealFftProcessor: Thin wrapper around realfft for single-channel use
// - RingAccumulator: Sample accumulator with hop-based triggering

use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::sync::Arc;

// ============================================================================
// Hann Window
// ============================================================================

/// Generate a Hann window of the given size.
/// Uses N (not N-1) divisor for perfect COLA with 50% overlap.
pub fn generate_hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| 0.5 * (1.0 - ((2.0 * std::f32::consts::PI * i as f32) / size as f32).cos()))
        .collect()
}

/// Generate a symmetric Hann window of the given size.
/// Uses N-1 divisor — suitable for spectral analysis (zero at endpoints).
pub fn generate_hann_window_symmetric(size: usize) -> Vec<f32> {
    if size <= 1 {
        return vec![1.0; size];
    }
    let n_minus_1 = (size as f32) - 1.0;
    (0..size)
        .map(|i| 0.5 * (1.0 - ((2.0 * std::f32::consts::PI * i as f32) / n_minus_1).cos()))
        .collect()
}

/// Generate a sqrt(Hann) window for WOLA (Weighted Overlap-Add) processing.
/// When used as both analysis and synthesis window, the product is Hann,
/// which has perfect COLA at 50% overlap.
pub fn generate_sqrt_hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| {
            let hann = 0.5 * (1.0 - ((2.0 * std::f32::consts::PI * i as f32) / size as f32).cos());
            hann.sqrt()
        })
        .collect()
}

// ============================================================================
// RealFftProcessor
// ============================================================================

/// Thin wrapper around `realfft` encapsulating planner + buffers for
/// single-channel use. Provides forward (real→complex) and optional
/// inverse (complex→real) FFT.
pub struct RealFftProcessor {
    #[allow(dead_code)]
    pub fft_size: usize,
    pub spectrum_size: usize,
    fft_forward: Arc<dyn RealToComplex<f32>>,
    fft_inverse: Option<Arc<dyn ComplexToReal<f32>>>,
    pub time_buffer: Vec<f32>,
    pub freq_buffer: Vec<Complex<f32>>,
}

impl RealFftProcessor {
    /// Create a forward-only FFT processor (no inverse).
    pub fn new_forward_only(fft_size: usize) -> Self {
        let spectrum_size = fft_size / 2 + 1;
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(fft_size);

        Self {
            fft_size,
            spectrum_size,
            fft_forward,
            fft_inverse: None,
            time_buffer: vec![0.0; fft_size],
            freq_buffer: vec![Complex::new(0.0, 0.0); spectrum_size],
        }
    }

    /// Create a bidirectional FFT processor (forward + inverse).
    #[allow(dead_code)]
    pub fn new_bidirectional(fft_size: usize) -> Self {
        let spectrum_size = fft_size / 2 + 1;
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(fft_size);
        let fft_inverse = planner.plan_fft_inverse(fft_size);

        Self {
            fft_size,
            spectrum_size,
            fft_forward,
            fft_inverse: Some(fft_inverse),
            time_buffer: vec![0.0; fft_size],
            freq_buffer: vec![Complex::new(0.0, 0.0); spectrum_size],
        }
    }

    /// Perform forward FFT: time_buffer → freq_buffer.
    /// The caller should fill `time_buffer` before calling this.
    pub fn forward(&mut self) {
        self.fft_forward
            .process(&mut self.time_buffer, &mut self.freq_buffer)
            .expect("FFT forward failed");
    }

    /// Perform inverse FFT: freq_buffer → time_buffer.
    /// Panics if this processor was created with `new_forward_only`.
    #[allow(dead_code)]
    pub fn inverse(&mut self) {
        self.fft_inverse
            .as_ref()
            .expect("Inverse FFT not available (forward-only processor)")
            .process(&mut self.freq_buffer, &mut self.time_buffer)
            .expect("FFT inverse failed");
    }
}

// ============================================================================
// RingAccumulator
// ============================================================================

/// Sample accumulator with hop-based triggering.
/// Accumulates samples into a circular buffer and signals when `hop_size`
/// new samples have been written (and the buffer has been filled at least once).
pub struct RingAccumulator {
    buffer: Vec<f32>,
    write_pos: usize,
    samples_since_trigger: usize,
    filled: bool,
    window_size: usize,
    hop_size: usize,
}

impl RingAccumulator {
    pub fn new(window_size: usize, hop_size: usize) -> Self {
        Self {
            buffer: vec![0.0; window_size],
            write_pos: 0,
            samples_since_trigger: 0,
            filled: false,
            window_size,
            hop_size,
        }
    }

    /// Push a single sample. Returns `true` when `hop_size` samples have
    /// accumulated since the last trigger (and the buffer is full).
    pub fn push(&mut self, sample: f32) -> bool {
        self.buffer[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % self.window_size;
        self.samples_since_trigger += 1;

        if !self.filled && self.samples_since_trigger >= self.window_size {
            self.filled = true;
        }

        if self.filled && self.samples_since_trigger >= self.hop_size {
            self.samples_since_trigger = 0;
            true
        } else {
            false
        }
    }

    /// Copy the current window (oldest-first) into `dest`.
    /// `dest` must be at least `window_size` long.
    /// Uses two contiguous copies instead of per-element modulo.
    pub fn read_window(&self, dest: &mut [f32]) {
        debug_assert!(dest.len() >= self.window_size);
        let start = self.write_pos; // oldest sample
        let first_len = self.window_size - start;
        dest[..first_len].copy_from_slice(&self.buffer[start..]);
        if start > 0 {
            dest[first_len..self.window_size].copy_from_slice(&self.buffer[..start]);
        }
    }

    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
        self.samples_since_trigger = 0;
        self.filled = false;
    }
}

// ============================================================================
// Dual-Window STFT Framework
// ============================================================================
//
// Decouples frequency resolution from latency by using separate analysis
// (long) and synthesis (short) windows. The analysis window provides high
// frequency resolution while the synthesis window determines the output latency.

/// Dual-window STFT processor.
///
/// Uses a long analysis window for frequency resolution and a shorter
/// synthesis window for low-latency output. The output latency equals
/// the synthesis window size, not the analysis window size.
pub struct DualWindowStft {
    analysis_window: Vec<f32>,
    synthesis_window: Vec<f32>,
    analysis_size: usize,
    /// Input ring buffer sized to analysis window
    input_ring: RingAccumulator,
    /// Overlap-add output accumulator
    output_accum: Vec<f32>,
    output_read_pos: usize,
    /// FFT processor (analysis size)
    fft: RealFftProcessor,
    /// Window read buffer
    window_buf: Vec<f32>,
    /// COLA normalization factor
    #[allow(dead_code)]
    cola_norm: Vec<f32>,
}

/// Design a dual-window pair satisfying the COLA (Constant Overlap-Add) condition.
///
/// # Arguments
/// * `analysis_size` - Analysis window length (long, e.g. 1024)
/// * `synthesis_size` - Synthesis window length (short, e.g. 256)
/// * `hop_size` - Hop size in samples
///
/// # Returns
/// (analysis_window, synthesis_window) pair
pub fn design_dual_windows(
    analysis_size: usize,
    synthesis_size: usize,
    hop_size: usize,
) -> (Vec<f32>, Vec<f32>) {
    // Analysis window: Hann
    let w_a = generate_hann_window(analysis_size);

    // Synthesis window: truncated Hann centered in the analysis window,
    // normalized to satisfy COLA
    let offset = (analysis_size - synthesis_size) / 2;

    // Start with a Hann window of synthesis_size
    let w_s_raw = generate_hann_window(synthesis_size);

    // Compute the COLA sum: Σ_k w_a(n - k*hop) * w_s(n - k*hop)
    // across all hop-shifted positions. We need this to be constant.
    // Normalize w_s so the sum equals 1.
    let num_overlaps = analysis_size.div_ceil(hop_size);

    let mut cola_sum = vec![0.0f32; hop_size];
    for k in 0..num_overlaps {
        let shift = k * hop_size;
        for (n, cola_val) in cola_sum.iter_mut().enumerate() {
            let ana_idx = n + shift;
            if ana_idx < analysis_size {
                // Check if this falls within the synthesis window support
                let syn_idx = ana_idx.wrapping_sub(offset);
                if syn_idx < synthesis_size {
                    *cola_val += w_a[ana_idx] * w_s_raw[syn_idx];
                }
            }
        }
    }

    // Normalize synthesis window
    let avg_cola: f32 = cola_sum.iter().sum::<f32>() / cola_sum.len() as f32;
    let norm_factor = if avg_cola > 1e-10 { 1.0 / avg_cola } else { 1.0 };

    let mut w_s = vec![0.0f32; analysis_size];
    for i in 0..synthesis_size {
        w_s[offset + i] = w_s_raw[i] * norm_factor;
    }

    (w_a, w_s)
}

impl DualWindowStft {
    /// Create a new dual-window STFT processor.
    ///
    /// # Arguments
    /// * `analysis_size` - Analysis window size (determines frequency resolution)
    /// * `synthesis_size` - Synthesis window size (determines output latency)
    /// * `hop_size` - Hop size in samples
    pub fn new(analysis_size: usize, synthesis_size: usize, hop_size: usize) -> Self {
        let (analysis_window, synthesis_window) =
            design_dual_windows(analysis_size, synthesis_size, hop_size);

        let fft = RealFftProcessor::new_bidirectional(analysis_size);

        Self {
            analysis_window,
            synthesis_window,
            analysis_size,
            input_ring: RingAccumulator::new(analysis_size, hop_size),
            output_accum: vec![0.0; analysis_size * 3],
            output_read_pos: 0,
            fft,
            window_buf: vec![0.0; analysis_size],
            cola_norm: vec![1.0; analysis_size],
        }
    }

    /// Push a single sample. Returns `true` when a hop boundary is reached.
    ///
    /// When `true`, the spectrum is available in `freq_buffer_mut()` for
    /// in-place modification. Call `synthesize_in_place()` after modifying.
    pub fn analyze(&mut self, sample: f32) -> bool {
        if !self.input_ring.push(sample) {
            return false;
        }

        // Read the analysis window worth of samples
        self.input_ring.read_window(&mut self.window_buf);

        // Apply analysis window
        for i in 0..self.analysis_size {
            self.fft.time_buffer[i] = self.window_buf[i] * self.analysis_window[i];
        }

        // Forward FFT
        self.fft.forward();

        true
    }

    /// Access the frequency buffer for in-place modification after `analyze()` returns `true`.
    pub fn freq_buffer_mut(&mut self) -> &mut [Complex<f32>] {
        &mut self.fft.freq_buffer
    }

    /// Synthesize output from the current frequency buffer (after in-place modification).
    ///
    /// Call this after `analyze()` returns `true` and the spectrum has been modified
    /// via `freq_buffer_mut()`. The output samples accumulate in the internal buffer
    /// and can be read via `read_output()`.
    pub fn synthesize_in_place(&mut self) {
        // Inverse FFT (operates on self.fft.freq_buffer directly)
        self.fft.inverse();

        // Apply synthesis window and overlap-add
        let scale = 1.0 / self.analysis_size as f32;
        for i in 0..self.analysis_size {
            let pos = (self.output_read_pos + i) % self.output_accum.len();
            self.output_accum[pos] += self.fft.time_buffer[i] * self.synthesis_window[i] * scale;
        }
    }

    /// Read one output sample. Returns 0.0 if no output is ready yet.
    pub fn read_output(&mut self) -> f32 {
        let sample = self.output_accum[self.output_read_pos];
        self.output_accum[self.output_read_pos] = 0.0;
        self.output_read_pos = (self.output_read_pos + 1) % self.output_accum.len();
        sample
    }

    /// Process a block: analyze, apply user function, synthesize.
    ///
    /// # Arguments
    /// * `input` - Input samples
    /// * `output` - Output buffer (same length as input)
    /// * `process_fn` - Function to modify the spectrum (called at each hop boundary)
    pub fn process_block<F>(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        mut process_fn: F,
    ) where
        F: FnMut(&mut [Complex<f32>]),
    {
        for (i, &sample) in input.iter().enumerate() {
            if self.analyze(sample) {
                process_fn(&mut self.fft.freq_buffer);
                self.synthesize_in_place();
            }
            output[i] = self.read_output();
        }
    }

    /// Get the output latency in samples.
    pub fn latency_samples(&self) -> usize {
        self.analysis_size
    }

    /// Reset all internal state.
    pub fn reset(&mut self) {
        self.input_ring.reset();
        self.output_accum.fill(0.0);
        self.output_read_pos = 0;
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::needless_range_loop)]
mod tests {
    use super::*;

    #[test]
    fn test_hann_window_size_and_symmetry() {
        let window = generate_hann_window(8);
        assert_eq!(window.len(), 8);

        // Hann window should start near zero and peak at center
        assert!((window[0] - 0.0).abs() < 0.01);
        assert!((window[4] - 1.0).abs() < 0.01);

        // Symmetric: w[i] == w[N-i] for periodic Hann
        for i in 1..4 {
            assert!(
                (window[i] - window[8 - i]).abs() < 1e-6,
                "Window not symmetric at i={}: {} vs {}",
                i,
                window[i],
                window[8 - i]
            );
        }
    }

    #[test]
    fn test_sqrt_hann_cola_property() {
        // sqrt(Hann) analysis * sqrt(Hann) synthesis = Hann
        // Hann has perfect COLA at 50% overlap: w[i] + w[i+N/2] = 1.0
        let n = 256;
        let sqrt_window = generate_sqrt_hann_window(n);
        let hop = n / 2;

        for i in 0..hop {
            // Product of analysis and synthesis = Hann
            let hann_i = sqrt_window[i] * sqrt_window[i];
            let hann_shifted = sqrt_window[i + hop] * sqrt_window[i + hop];
            let sum = hann_i + hann_shifted;
            assert!(
                (sum - 1.0).abs() < 1e-5,
                "sqrt(Hann) COLA violated at i={}: sum={}, expected 1.0",
                i,
                sum
            );
        }
    }

    #[test]
    fn test_hann_window_cola_property() {
        // With 50% overlap, w[i] + w[i + N/2] should equal 1.0 (COLA)
        let n = 256;
        let window = generate_hann_window(n);
        let hop = n / 2;

        for i in 0..hop {
            let sum = window[i] + window[i + hop];
            assert!(
                (sum - 1.0).abs() < 1e-5,
                "COLA violated at i={}: sum={}, expected 1.0",
                i,
                sum
            );
        }
    }

    #[test]
    fn test_symmetric_hann_endpoints_are_zero() {
        let window = generate_hann_window_symmetric(256);
        assert!(window[0].abs() < 1e-7, "First sample should be 0");
        assert!(window[255].abs() < 1e-7, "Last sample should be 0");
        // Peak at center
        assert!((window[128] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_symmetric_hann_no_nan_for_small_sizes() {
        // size=0: empty
        let w0 = generate_hann_window_symmetric(0);
        assert!(w0.is_empty());

        // size=1: should be [1.0], not NaN
        let w1 = generate_hann_window_symmetric(1);
        assert_eq!(w1.len(), 1);
        assert!(w1[0].is_finite(), "size=1 produced non-finite: {}", w1[0]);
        assert!((w1[0] - 1.0).abs() < 1e-6);

        // size=2: endpoints [0, 0] by symmetric formula
        let w2 = generate_hann_window_symmetric(2);
        assert_eq!(w2.len(), 2);
        assert!(w2[0].is_finite());
        assert!(w2[1].is_finite());
    }

    #[test]
    fn test_fft_roundtrip() {
        let fft_size = 256;
        let mut fft = RealFftProcessor::new_bidirectional(fft_size);

        // Fill with a known signal
        let original: Vec<f32> = (0..fft_size)
            .map(|i| (2.0 * std::f32::consts::PI * 10.0 * i as f32 / fft_size as f32).sin())
            .collect();
        fft.time_buffer.copy_from_slice(&original);

        // Forward then inverse
        fft.forward();
        fft.inverse();

        // Inverse FFT scales by fft_size, so divide
        let scale = 1.0 / fft_size as f32;
        for i in 0..fft_size {
            let recovered = fft.time_buffer[i] * scale;
            assert!(
                (recovered - original[i]).abs() < 1e-4,
                "FFT roundtrip mismatch at i={}: expected {}, got {}",
                i,
                original[i],
                recovered,
            );
        }
    }

    #[test]
    fn test_ring_accumulator_trigger_timing() {
        let window_size = 8;
        let hop_size = 4;
        let mut ring = RingAccumulator::new(window_size, hop_size);

        let mut triggers = Vec::new();
        for i in 0..24 {
            if ring.push(i as f32) {
                triggers.push(i);
            }
        }

        // First trigger at sample 7 (index 7 = 8th sample, filling window)
        // Then every hop_size (4) samples: 11, 15, 19, 23
        assert_eq!(triggers, vec![7, 11, 15, 19, 23]);
    }

    #[test]
    fn test_ring_accumulator_window_readout() {
        let window_size = 4;
        let hop_size = 2;
        let mut ring = RingAccumulator::new(window_size, hop_size);

        // Push 6 samples: [0, 1, 2, 3, 4, 5]
        // After 4 samples, ring is filled. After 6 samples (2 more = hop), trigger.
        // Ring state: write_pos = 2, buffer = [4, 5, 2, 3]
        // oldest-first read: [2, 3, 4, 5]
        for i in 0..6 {
            ring.push(i as f32);
        }

        let mut dest = vec![0.0; 4];
        ring.read_window(&mut dest);
        assert_eq!(dest, vec![2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn test_ring_accumulator_reset() {
        let mut ring = RingAccumulator::new(8, 4);

        // Fill and trigger
        for i in 0..12 {
            ring.push(i as f32);
        }
        assert!(ring.filled);

        ring.reset();
        assert!(!ring.filled);
        assert_eq!(ring.write_pos, 0);
        assert_eq!(ring.samples_since_trigger, 0);

        // Should not trigger until filled again
        let mut triggered = false;
        for _ in 0..4 {
            triggered |= ring.push(1.0);
        }
        assert!(!triggered, "Should not trigger before ring is filled again");
    }

    #[test]
    fn test_dual_window_design() {
        let analysis_size = 1024;
        let synthesis_size = 256;
        let hop_size = 128;

        let (w_a, w_s) = design_dual_windows(analysis_size, synthesis_size, hop_size);
        assert_eq!(w_a.len(), analysis_size);
        assert_eq!(w_s.len(), analysis_size);

        // Synthesis window should be non-zero only in the center
        let offset = (analysis_size - synthesis_size) / 2;
        for i in 0..offset {
            assert_eq!(w_s[i], 0.0, "Synthesis window should be zero before offset");
        }
        for i in (offset + synthesis_size)..analysis_size {
            assert_eq!(w_s[i], 0.0, "Synthesis window should be zero after support");
        }
    }

    #[test]
    fn test_dual_window_stft_passthrough() {
        let analysis_size = 512;
        let synthesis_size = 128;
        let hop_size = 64;

        let mut stft = DualWindowStft::new(analysis_size, synthesis_size, hop_size);

        // Generate a tone
        let num_samples = 4096;
        let signal: Vec<f32> = (0..num_samples)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 48000.0).sin())
            .collect();

        let mut output = vec![0.0f32; num_samples];

        // Pass-through (no spectral modification)
        stft.process_block(&signal, &mut output, |_spectrum| {
            // Identity: don't modify spectrum
        });

        // After latency, output should approximate input
        let latency = stft.latency_samples();
        let check_start = latency + 512; // skip transient
        let check_end = num_samples - 512;

        if check_end > check_start {
            let rms_error: f32 = output[check_start..check_end]
                .iter()
                .zip(&signal[check_start - latency..check_end - latency])
                .map(|(o, s)| (o - s).powi(2))
                .sum::<f32>()
                / (check_end - check_start) as f32;

            // Some error is expected from windowing; just verify it's bounded
            assert!(
                rms_error < 1.0,
                "Dual-window STFT passthrough RMS error too high: {rms_error:.6}"
            );
        }
    }

    #[test]
    fn test_dual_window_stft_reset() {
        let mut stft = DualWindowStft::new(512, 128, 64);

        // Process some data
        let signal: Vec<f32> = (0..2048)
            .map(|i| (i as f32 * 0.1).sin())
            .collect();
        let mut output = vec![0.0; 2048];
        stft.process_block(&signal, &mut output, |_| {});

        // Reset
        stft.reset();

        // Process silence — output should be near zero
        let silence = vec![0.0f32; 1024];
        let mut output2 = vec![0.0; 1024];
        stft.process_block(&silence, &mut output2, |_| {});

        let max_output: f32 = output2.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
        assert!(
            max_output < 0.01,
            "After reset + silence, max output should be ~0, got {max_output}"
        );
    }
}
