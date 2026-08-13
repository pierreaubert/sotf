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
    /// Remapped positive-frequency magnitudes for the current synthesis frame.
    pub(super) synth_magnitude: Vec<f32>,
    /// Magnitude-weighted instantaneous target-bin frequency.
    pub(super) synth_frequency_sum: Vec<f32>,
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
            synth_magnitude: vec![0.0; PV_FFT_SIZE / 2 + 1],
            synth_frequency_sum: vec![0.0; PV_FFT_SIZE / 2 + 1],
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
        self.synth_magnitude.fill(0.0);
        self.synth_frequency_sum.fill(0.0);
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

        // Analysis: estimate each positive-frequency bin's instantaneous
        // frequency, then move both magnitude and frequency to the requested
        // target bin. Merely changing a bin's phase advance while leaving its
        // magnitude at the source bin does not shift pitch.
        self.synth_magnitude.fill(0.0);
        self.synth_frequency_sum.fill(0.0);
        for bin in 0..=n / 2 {
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

            let target_bin = (bin as f32 * pitch_shift).round() as usize;
            if target_bin <= n / 2 {
                let shifted_frequency = true_freq * pitch_shift;
                self.synth_magnitude[target_bin] += mag;
                self.synth_frequency_sum[target_bin] += mag * shifted_frequency;
            }
        }

        self.ifft_buf.fill(Complex::new(0.0, 0.0));
        for bin in 0..=n / 2 {
            let magnitude = self.synth_magnitude[bin];
            if magnitude <= f32::EPSILON {
                continue;
            }
            let shifted_frequency = self.synth_frequency_sum[bin] / magnitude;
            self.synth_phase[bin] += shifted_frequency * expected_phase_advance;
            self.ifft_buf[bin] = Complex::new(
                magnitude * self.synth_phase[bin].cos(),
                magnitude * self.synth_phase[bin].sin(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn render_tone(input_hz: f32, ratio: f32, sample_rate: f32, frames: usize) -> Vec<f32> {
        let mut channel = PhaseVocoderChannel::new();
        let mut output = Vec::with_capacity(frames);

        for frame in 0..frames {
            let phase = 2.0 * std::f32::consts::PI * input_hz * frame as f32 / sample_rate;
            channel.input_buf[channel.input_fill] = 0.5 * phase.sin();
            channel.input_fill += 1;
            if channel.input_fill >= PV_FFT_SIZE {
                channel.process_hop(ratio);
            }

            if channel.output_fill > 0 {
                let index = channel.output_read % channel.output_accum.len();
                output.push(channel.output_accum[index]);
                channel.output_accum[index] = 0.0;
                channel.output_read += 1;
                channel.output_fill -= 1;
            } else {
                output.push(0.0);
            }
        }

        output
    }

    fn dominant_frequency(samples: &[f32], sample_rate: f32) -> f32 {
        let mut crossings = 0usize;
        for pair in samples.windows(2) {
            if pair[0] <= 0.0 && pair[1] > 0.0 {
                crossings += 1;
            }
        }
        crossings as f32 * sample_rate / samples.len() as f32
    }

    #[test]
    fn phase_vocoder_moves_spectral_energy_to_the_requested_pitch() {
        let sample_rate = 48_000.0;
        let rendered = render_tone(440.0, 2.0, sample_rate, 96_000);
        let steady = &rendered[PV_FFT_SIZE * 4..];
        let frequency = dominant_frequency(steady, sample_rate);
        let rms =
            (steady.iter().map(|sample| sample * sample).sum::<f32>() / steady.len() as f32).sqrt();

        assert!(
            rms > 0.05,
            "pitch-shifted output is unexpectedly quiet: {rms}"
        );
        assert!(
            (frequency - 880.0).abs() < 12.0,
            "2x pitch shift should move 440 Hz near 880 Hz, got {frequency} Hz"
        );
    }

    #[test]
    fn phase_vocoder_shifts_down_and_preserves_unity_pitch() {
        let sample_rate = 48_000.0;
        for (ratio, expected_hz) in [(0.5, 220.0), (1.0, 440.0)] {
            let rendered = render_tone(440.0, ratio, sample_rate, 96_000);
            let steady = &rendered[PV_FFT_SIZE * 4..];
            let frequency = dominant_frequency(steady, sample_rate);
            let rms = (steady.iter().map(|sample| sample * sample).sum::<f32>()
                / steady.len() as f32)
                .sqrt();

            assert!(
                rms > 0.05,
                "ratio {ratio} output is unexpectedly quiet: {rms}"
            );
            assert!(
                (frequency - expected_hz).abs() < 12.0,
                "ratio {ratio} should produce {expected_hz} Hz, got {frequency} Hz"
            );
        }
    }
}
