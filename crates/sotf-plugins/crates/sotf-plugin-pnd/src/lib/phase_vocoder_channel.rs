use super::consts::PV_FFT_SIZE;
use super::consts::PV_HOP_SIZE;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use std::sync::Arc;

/// Per-channel phase vocoder state for pitch shifting without changing duration.
pub(super) struct PhaseVocoderChannel {
    pub(super) fft_forward: Arc<dyn rustfft::Fft<f32>>,
    pub(super) fft_inverse: Arc<dyn rustfft::Fft<f32>>,
    pub(super) analysis_window: Vec<f32>,
    /// Input accumulation buffer
    pub(super) input_buf: Vec<f32>,
    pub(super) input_fill: usize,
    /// Output overlap-add buffer
    pub(super) output_accum: Vec<f32>,
    pub(super) output_read: usize,
    pub(super) output_fill: usize,
    /// Previous frame analysis phases for phase accumulation
    pub(super) prev_phase: Vec<f32>,
    /// Accumulated synthesis phases
    pub(super) synth_phase: Vec<f32>,
    /// Scratch buffers
    pub(super) fft_buf: Vec<Complex<f32>>,
    pub(super) fft_scratch: Vec<Complex<f32>>,
    pub(super) ifft_buf: Vec<Complex<f32>>,
}

impl PhaseVocoderChannel {
    pub(super) fn new() -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(PV_FFT_SIZE);
        let fft_inverse = planner.plan_fft_inverse(PV_FFT_SIZE);
        let scratch_len = fft_forward
            .get_inplace_scratch_len()
            .max(fft_inverse.get_inplace_scratch_len());

        let analysis_window: Vec<f32> = (0..PV_FFT_SIZE)
            .map(|i| {
                let x = i as f32 / PV_FFT_SIZE as f32;
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * x).cos())
            })
            .collect();

        Self {
            fft_forward,
            fft_inverse,
            analysis_window,
            input_buf: vec![0.0; PV_FFT_SIZE],
            input_fill: 0,
            output_accum: vec![0.0; PV_FFT_SIZE * 4],
            output_read: 0,
            output_fill: 0,
            prev_phase: vec![0.0; PV_FFT_SIZE],
            synth_phase: vec![0.0; PV_FFT_SIZE],
            fft_buf: vec![Complex::new(0.0, 0.0); PV_FFT_SIZE],
            fft_scratch: vec![Complex::new(0.0, 0.0); scratch_len],
            ifft_buf: vec![Complex::new(0.0, 0.0); PV_FFT_SIZE],
        }
    }

    pub(super) fn reset(&mut self) {
        self.input_buf.fill(0.0);
        self.input_fill = 0;
        self.output_accum.fill(0.0);
        self.output_read = 0;
        self.output_fill = 0;
        self.prev_phase.fill(0.0);
        self.synth_phase.fill(0.0);
    }

    /// Process a hop of samples with the given pitch shift ratio.
    /// pitch_shift > 1.0 shifts up, < 1.0 shifts down.
    pub(super) fn process_hop(&mut self, pitch_shift: f32) {
        let n = PV_FFT_SIZE;
        let hop = PV_HOP_SIZE;
        let expected_phase_advance = 2.0 * std::f32::consts::PI * hop as f32 / n as f32;
        let inv_n = 1.0 / n as f32;

        // Window and FFT
        for i in 0..n {
            self.fft_buf[i] = Complex::new(self.input_buf[i] * self.analysis_window[i], 0.0);
        }
        self.fft_forward
            .process_with_scratch(&mut self.fft_buf, &mut self.fft_scratch);

        // Analysis: extract magnitude and phase, compute instantaneous frequency
        for bin in 0..n {
            let mag = self.fft_buf[bin].norm();
            let phase = self.fft_buf[bin].arg();

            // Phase difference from previous frame
            let phase_diff = phase - self.prev_phase[bin];
            self.prev_phase[bin] = phase;

            // Remove expected phase advance
            let deviation = phase_diff - bin as f32 * expected_phase_advance;

            // Wrap to [-pi, pi]
            let wrapped = deviation
                - (deviation / (2.0 * std::f32::consts::PI)).round() * 2.0 * std::f32::consts::PI;

            // True frequency (in bins)
            let true_freq = bin as f32 + wrapped / expected_phase_advance;

            // Synthesis: apply pitch shift to frequency
            let shifted_freq = true_freq * pitch_shift;

            // Accumulate synthesis phase at the shifted frequency
            self.synth_phase[bin] += shifted_freq * expected_phase_advance;

            // Reconstruct complex spectrum with original magnitude and shifted phase
            self.ifft_buf[bin] = Complex::new(
                mag * self.synth_phase[bin].cos(),
                mag * self.synth_phase[bin].sin(),
            );
        }

        // Restore conjugate symmetry for correct real-valued IFFT
        let n = PV_FFT_SIZE;
        self.ifft_buf[0].im = 0.0;
        if n > 1 {
            self.ifft_buf[n / 2].im = 0.0;
        }
        for bin in 1..n / 2 {
            self.ifft_buf[n - bin] = self.ifft_buf[bin].conj();
        }

        // IFFT
        self.fft_inverse
            .process_with_scratch(&mut self.ifft_buf, &mut self.fft_scratch);

        // Overlap-add with synthesis window and normalization
        let scale = inv_n / 1.5; // Hann window with 75% overlap: sum(w^2) normalization
        let accum_len = self.output_accum.len();
        for i in 0..n {
            let idx = (self.output_read + self.output_fill + i) % accum_len;
            self.output_accum[idx] += self.ifft_buf[i].re * self.analysis_window[i] * scale;
        }
        self.output_fill += hop;

        // Shift input buffer by hop
        self.input_buf.copy_within(hop..n, 0);
        self.input_fill = n - hop;
    }
}
