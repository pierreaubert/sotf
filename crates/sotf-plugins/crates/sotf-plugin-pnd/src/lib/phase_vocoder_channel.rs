use super::consts::PV_FFT_SIZE;
use super::consts::PV_HOP_SIZE;
use super::consts::PV_PREFILL_FRAMES;
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
    /// Previous magnitudes used by the normalized spectral-flux onset detector.
    pub(super) prev_magnitude: Vec<f32>,
    /// Current positive-frequency analysis phases and instantaneous frequencies.
    pub(super) analysis_magnitude: Vec<f32>,
    /// Smoothed log-magnitude envelope for optional formant transport.
    pub(super) analysis_envelope: Vec<f32>,
    pub(super) analysis_phase: Vec<f32>,
    pub(super) source_frequency: Vec<f32>,
    /// Source-bin peak regions and dominant contributors after spectral remapping.
    pub(super) source_peak_owner: Vec<usize>,
    pub(super) dominant_source: Vec<usize>,
    pub(super) dominant_magnitude: Vec<f32>,
    /// Source peak assigned to each remapped target peak (`usize::MAX` if absent).
    pub(super) target_peak_source: Vec<usize>,
    pub(super) previous_target_peak_source: Vec<usize>,
    /// Within-frame position of an onset while it remains in overlapping frames.
    pub(super) transient_position: Option<usize>,
    pub(super) processed_hops: usize,
    /// Accumulated synthesis phases
    pub(super) synth_phase: Vec<f32>,
    pub(super) synth_phase_initialized: Vec<bool>,
    /// Remapped positive-frequency magnitudes for the current synthesis frame.
    pub(super) synth_magnitude: Vec<f32>,
    /// Magnitude-weighted instantaneous target-bin frequency.
    pub(super) synth_frequency_sum: Vec<f32>,
    /// Scratch buffers
    pub(super) fft_buf: Vec<Complex<f32>>,
    pub(super) fft_scratch: Vec<Complex<f32>>,
    pub(super) ifft_buf: Vec<Complex<f32>>,
    #[cfg(test)]
    pub(super) last_frame_was_transient: bool,
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
            input_fill: PV_PREFILL_FRAMES,
            output_accum: vec![0.0; PV_FFT_SIZE * 4],
            output_read: 0,
            output_fill: 0,
            prev_phase: vec![0.0; PV_FFT_SIZE],
            prev_magnitude: vec![0.0; PV_FFT_SIZE / 2 + 1],
            analysis_magnitude: vec![0.0; PV_FFT_SIZE / 2 + 1],
            analysis_envelope: vec![0.0; PV_FFT_SIZE / 2 + 1],
            analysis_phase: vec![0.0; PV_FFT_SIZE / 2 + 1],
            source_frequency: vec![0.0; PV_FFT_SIZE / 2 + 1],
            source_peak_owner: vec![0; PV_FFT_SIZE / 2 + 1],
            dominant_source: vec![usize::MAX; PV_FFT_SIZE / 2 + 1],
            dominant_magnitude: vec![0.0; PV_FFT_SIZE / 2 + 1],
            target_peak_source: vec![usize::MAX; PV_FFT_SIZE / 2 + 1],
            previous_target_peak_source: vec![usize::MAX; PV_FFT_SIZE / 2 + 1],
            transient_position: None,
            processed_hops: 0,
            synth_phase: vec![0.0; PV_FFT_SIZE],
            synth_phase_initialized: vec![false; PV_FFT_SIZE / 2 + 1],
            synth_magnitude: vec![0.0; PV_FFT_SIZE / 2 + 1],
            synth_frequency_sum: vec![0.0; PV_FFT_SIZE / 2 + 1],
            fft_buf: vec![Complex::new(0.0, 0.0); PV_FFT_SIZE],
            fft_scratch: vec![Complex::new(0.0, 0.0); scratch_len],
            ifft_buf: vec![Complex::new(0.0, 0.0); PV_FFT_SIZE],
            #[cfg(test)]
            last_frame_was_transient: false,
        }
    }

    pub(super) fn reset(&mut self) {
        self.input_buf.fill(0.0);
        self.input_fill = PV_PREFILL_FRAMES;
        self.output_accum.fill(0.0);
        self.output_read = 0;
        self.output_fill = 0;
        self.prev_phase.fill(0.0);
        self.prev_magnitude.fill(0.0);
        self.analysis_magnitude.fill(0.0);
        self.analysis_envelope.fill(0.0);
        self.analysis_phase.fill(0.0);
        self.source_frequency.fill(0.0);
        self.source_peak_owner.fill(0);
        self.dominant_source.fill(usize::MAX);
        self.dominant_magnitude.fill(0.0);
        self.target_peak_source.fill(usize::MAX);
        self.previous_target_peak_source.fill(usize::MAX);
        self.transient_position = None;
        self.processed_hops = 0;
        self.synth_phase.fill(0.0);
        self.synth_phase_initialized.fill(false);
        self.synth_magnitude.fill(0.0);
        self.synth_frequency_sum.fill(0.0);
        #[cfg(test)]
        {
            self.last_frame_was_transient = false;
        }
    }

    /// Process a hop of samples with the given pitch shift ratio.
    /// pitch_shift > 1.0 shifts up, < 1.0 shifts down.
    #[cfg(test)]
    pub(super) fn process_hop(&mut self, pitch_shift: f32) {
        self.process_hop_with_formant_strength(pitch_shift, 0.0);
    }

    /// Process a hop with optional formant-preserving spectral-envelope
    /// transport.  `formant_strength` is in [0, 1]; zero is exactly the legacy
    /// uniform pitch-shift path.  The envelope is estimated from the current
    /// positive-frequency magnitude frame and all work uses setup-allocated
    /// buffers.
    pub(super) fn process_hop_with_formant_strength(
        &mut self,
        pitch_shift: f32,
        formant_strength: f32,
    ) {
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

        // Analysis: estimate instantaneous frequency and normalized positive
        // spectral flux before changing the previous-frame state. A large
        // positive flux marks an onset. Onsets reset synthesis peak phases to
        // their analysis phases instead of extrapolating stale steady-state
        // phase through a transient.
        let positive_bins = n / 2 + 1;
        let mut positive_flux = 0.0_f32;
        let mut magnitude_sum = 0.0_f32;
        let mut max_magnitude = 0.0_f32;
        for bin in 0..positive_bins {
            let magnitude = self.fft_buf[bin].norm();
            let phase = self.fft_buf[bin].arg();
            let phase_diff = phase - self.prev_phase[bin];
            let deviation = phase_diff - bin as f32 * expected_phase_advance;
            let wrapped = wrap_phase(deviation);

            self.analysis_magnitude[bin] = magnitude;
            self.analysis_phase[bin] = phase;
            self.source_frequency[bin] = bin as f32 + wrapped / expected_phase_advance;
            positive_flux += (magnitude - self.prev_magnitude[bin]).max(0.0);
            magnitude_sum += magnitude;
            max_magnitude = max_magnitude.max(magnitude);
        }
        // Estimate the attack's position inside this analysis frame. Remapping
        // a source bin k to target k' must also remap the linear phase ramp
        // caused by a time offset tau: phi(k') = phi(k) - 2*pi*(k'-k)*tau/N.
        // Copying phi(k) directly shifts an off-origin attack by the pitch
        // ratio. Since frames advance by one hop, genuinely new material lies
        // in the newest hop. Select its largest time-domain novelty rather than
        // the first absolute threshold crossing in the whole frame: the latter
        // incorrectly chooses an older sustained tone before a new attack.
        let new_hop_start = n - hop;
        let mut transient_sample = new_hop_start;
        let mut maximum_novelty = 0.0_f32;
        let mut novelty_sum = 0.0_f32;
        let mut hop_energy = 0.0_f32;
        for sample in new_hop_start..n {
            let previous = self.input_buf[sample.saturating_sub(1)];
            let novelty = (self.input_buf[sample] - previous).abs();
            novelty_sum += novelty;
            hop_energy += self.input_buf[sample] * self.input_buf[sample];
            if novelty > maximum_novelty {
                maximum_novelty = novelty;
                transient_sample = sample;
            }
        }
        let mean_novelty = novelty_sum / hop as f32;
        let hop_rms = (hop_energy / hop as f32).sqrt();
        let spectral_onset = magnitude_sum > f32::EPSILON
            && positive_flux / magnitude_sum.max(f32::MIN_POSITIVE) >= 0.35;
        let time_domain_onset = maximum_novelty > f32::EPSILON
            && maximum_novelty >= mean_novelty * 6.0
            && maximum_novelty >= hop_rms * 4.0;
        // A sharp attack near the right edge is strongly attenuated by the
        // Hann window and may not produce spectral flux until the next overlap.
        // Arm its time origin immediately from the unwindowed newest hop; the
        // tracked position then follows it through all overlapping frames.
        let mut new_transient =
            time_domain_onset && (spectral_onset || maximum_novelty >= mean_novelty * 12.0);
        // The first external sample always begins at the known prefill
        // boundary for every channel. Use that shared time origin even when a
        // phase-shifted tone starts near a zero crossing in one channel and at
        // full amplitude in another.
        if self.processed_hops == 0 && !new_transient {
            new_transient = true;
            transient_sample = PV_PREFILL_FRAMES;
        }
        self.transient_position = self
            .transient_position
            .and_then(|position| position.checked_sub(hop));
        if new_transient {
            self.transient_position = Some(transient_sample);
        }
        let transient = self.transient_position.is_some();
        let transient_sample = self.transient_position.unwrap_or(0);
        #[cfg(test)]
        {
            self.last_frame_was_transient = transient;
        }

        // Assign each source bin to the strongest nearby spectral peak. This
        // compact peak-region representation is deterministic, allocation-free,
        // and sufficient for identity phase locking over PND's narrow ±5%
        // correction range.
        const PEAK_REGION_RADIUS: usize = 4;
        for bin in 0..positive_bins {
            let start = bin.saturating_sub(PEAK_REGION_RADIUS);
            let end = (bin + PEAK_REGION_RADIUS + 1).min(positive_bins);
            let mut owner = bin;
            let mut owner_magnitude = self.analysis_magnitude[bin];
            for candidate in start..end {
                let candidate_magnitude = self.analysis_magnitude[candidate];
                if candidate_magnitude > owner_magnitude {
                    owner = candidate;
                    owner_magnitude = candidate_magnitude;
                }
            }
            self.source_peak_owner[bin] = owner;
        }

        // A short log-frequency smoothing kernel suppresses individual
        // harmonics while retaining the broad spectral peaks that carry vowel
        // and instrument-body identity.  Use the source frame for both source
        // and target lookup: gain E(target)/E(source) keeps the envelope at
        // its original absolute frequency after the harmonic remap.
        let formant_strength = formant_strength.clamp(0.0, 1.0);
        if formant_strength > 0.0 {
            const ENVELOPE_RADIUS: usize = 8;
            for bin in 0..positive_bins {
                let start = bin.saturating_sub(ENVELOPE_RADIUS);
                let end = (bin + ENVELOPE_RADIUS + 1).min(positive_bins);
                let mut log_sum = 0.0_f32;
                for sample in start..end {
                    log_sum += (self.analysis_magnitude[sample] + 1.0e-6).ln();
                }
                self.analysis_envelope[bin] = (log_sum / (end - start) as f32).exp();
            }
        }

        // Identify the strongest source peak contributing to each remapped
        // target peak. Weak numerical sidelobes are not allowed to become phase
        // anchors.
        self.target_peak_source.fill(usize::MAX);
        let peak_floor = max_magnitude * 1.0e-5;
        for source_bin in 0..positive_bins {
            if self.source_peak_owner[source_bin] != source_bin
                || self.analysis_magnitude[source_bin] <= peak_floor
            {
                continue;
            }
            let target_peak = (source_bin as f32 * pitch_shift).round() as usize;
            if target_peak >= positive_bins {
                continue;
            }
            let previous = self.target_peak_source[target_peak];
            if previous == usize::MAX
                || self.analysis_magnitude[source_bin] > self.analysis_magnitude[previous]
            {
                self.target_peak_source[target_peak] = source_bin;
            }
        }

        // Move both magnitude and instantaneous frequency to the requested
        // target bin. Keep the strongest contributor for phase-region
        // ownership when multiple source bins collide.
        self.synth_magnitude.fill(0.0);
        self.synth_frequency_sum.fill(0.0);
        self.dominant_source.fill(usize::MAX);
        self.dominant_magnitude.fill(0.0);
        for bin in 0..positive_bins {
            let mag = self.analysis_magnitude[bin];
            let target_bin = (bin as f32 * pitch_shift).round() as usize;
            if target_bin <= n / 2 {
                let formant_gain = if formant_strength > 0.0 {
                    let source_envelope = self.analysis_envelope[bin].max(1.0e-6);
                    let target_envelope = self.analysis_envelope[target_bin].max(1.0e-6);
                    ((target_envelope / source_envelope).ln() * formant_strength)
                        .clamp(-1.386_294_4, 1.386_294_4)
                        .exp()
                } else {
                    1.0
                };
                let transported_magnitude = mag * formant_gain;
                let shifted_frequency = self.source_frequency[bin] * pitch_shift;
                self.synth_magnitude[target_bin] += transported_magnitude;
                self.synth_frequency_sum[target_bin] += transported_magnitude * shifted_frequency;
                if transported_magnitude > self.dominant_magnitude[target_bin] {
                    self.dominant_magnitude[target_bin] = transported_magnitude;
                    self.dominant_source[target_bin] = bin;
                }
            }
            self.prev_phase[bin] = self.analysis_phase[bin];
            self.prev_magnitude[bin] = mag;
        }

        // Advance (or transient-reset) phase only at remapped spectral peaks.
        // The remaining bins are locked to their source peak's relative phase
        // below, preserving waveform shape and inter-channel phase offsets.
        for target_peak in 0..positive_bins {
            let source_peak = self.target_peak_source[target_peak];
            if source_peak == usize::MAX {
                continue;
            }
            if transient
                || !self.synth_phase_initialized[target_peak]
                || self.previous_target_peak_source[target_peak] != source_peak
            {
                self.synth_phase[target_peak] = if transient {
                    remap_transient_phase(
                        self.analysis_phase[source_peak],
                        source_peak,
                        target_peak,
                        transient_sample,
                        n,
                    )
                } else {
                    self.analysis_phase[source_peak]
                };
                self.synth_phase_initialized[target_peak] = true;
            } else {
                self.synth_phase[target_peak] +=
                    self.source_frequency[source_peak] * pitch_shift * expected_phase_advance;
            }
        }
        self.previous_target_peak_source
            .copy_from_slice(&self.target_peak_source);

        self.ifft_buf.fill(Complex::new(0.0, 0.0));
        for bin in 0..positive_bins {
            let magnitude = self.synth_magnitude[bin];
            if magnitude <= f32::EPSILON {
                continue;
            }
            let shifted_frequency = self.synth_frequency_sum[bin] / magnitude;
            let source_bin = self.dominant_source[bin];
            let source_peak = self.source_peak_owner[source_bin];
            let target_peak = (source_peak as f32 * pitch_shift).round() as usize;
            if transient {
                self.synth_phase[bin] = remap_transient_phase(
                    self.analysis_phase[source_bin],
                    source_bin,
                    bin,
                    transient_sample,
                    n,
                );
                self.synth_phase_initialized[bin] = true;
            } else if target_peak < positive_bins
                && self.target_peak_source[target_peak] == source_peak
                && self.synth_phase_initialized[target_peak]
            {
                self.synth_phase[bin] = self.synth_phase[target_peak]
                    + wrap_phase(
                        self.analysis_phase[source_bin] - self.analysis_phase[source_peak],
                    );
            } else if !self.synth_phase_initialized[bin] {
                self.synth_phase[bin] = self.analysis_phase[source_bin];
                self.synth_phase_initialized[bin] = true;
            } else {
                self.synth_phase[bin] += shifted_frequency * expected_phase_advance;
            }
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
        self.processed_hops = self.processed_hops.saturating_add(1);
    }
}

#[inline]
fn wrap_phase(phase: f32) -> f32 {
    phase - (phase / (2.0 * std::f32::consts::PI)).round() * 2.0 * std::f32::consts::PI
}

#[inline]
fn remap_transient_phase(
    source_phase: f32,
    source_bin: usize,
    target_bin: usize,
    transient_sample: usize,
    fft_size: usize,
) -> f32 {
    let bin_delta = target_bin as f32 - source_bin as f32;
    wrap_phase(
        source_phase
            - 2.0 * std::f32::consts::PI * bin_delta * transient_sample as f32 / fft_size as f32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_tone(input_hz: f32, ratio: f32, sample_rate: f32, frames: usize) -> Vec<f32> {
        render_tone_with_phase(input_hz, ratio, sample_rate, frames, 0.0)
    }

    fn render_tone_with_phase(
        input_hz: f32,
        ratio: f32,
        sample_rate: f32,
        frames: usize,
        initial_phase: f32,
    ) -> Vec<f32> {
        let mut channel = PhaseVocoderChannel::new();
        let mut output = Vec::with_capacity(frames);

        for frame in 0..frames {
            let phase =
                initial_phase + 2.0 * std::f32::consts::PI * input_hz * frame as f32 / sample_rate;
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

    fn render_signal(input: &[f32], ratio: f32) -> Vec<f32> {
        render_signal_with_formant_strength(input, ratio, 0.0)
    }

    fn render_signal_with_formant_strength(
        input: &[f32],
        ratio: f32,
        formant_strength: f32,
    ) -> Vec<f32> {
        let mut channel = PhaseVocoderChannel::new();
        let mut output = vec![0.0; input.len()];
        for (&input_sample, output_sample) in input.iter().zip(&mut output) {
            channel.input_buf[channel.input_fill] = input_sample;
            channel.input_fill += 1;
            if channel.input_fill >= PV_FFT_SIZE {
                channel.process_hop_with_formant_strength(ratio, formant_strength);
            }
            if channel.output_fill > 0 {
                let index = channel.output_read % channel.output_accum.len();
                *output_sample = channel.output_accum[index];
                channel.output_accum[index] = 0.0;
                channel.output_read += 1;
                channel.output_fill -= 1;
            }
        }
        output
    }

    fn spectral_amplitude(samples: &[f32], frequency: f32, sample_rate: f32) -> f32 {
        let mut real = 0.0_f64;
        let mut imaginary = 0.0_f64;
        for (frame, &sample) in samples.iter().enumerate() {
            let angle = 2.0 * std::f64::consts::PI * f64::from(frequency) * frame as f64
                / f64::from(sample_rate);
            real += f64::from(sample) * angle.cos();
            imaginary -= f64::from(sample) * angle.sin();
        }
        (2.0 * real.hypot(imaginary) / samples.len() as f64) as f32
    }

    fn measured_phase(samples: &[f32], frequency: f32, sample_rate: f32) -> f32 {
        let mut real = 0.0_f64;
        let mut imaginary = 0.0_f64;
        for (frame, &sample) in samples.iter().enumerate() {
            let angle = 2.0 * std::f64::consts::PI * f64::from(frequency) * frame as f64
                / f64::from(sample_rate);
            real += f64::from(sample) * angle.cos();
            imaginary -= f64::from(sample) * angle.sin();
        }
        imaginary.atan2(real) as f32
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

    #[test]
    fn unity_ratio_preserves_tone_amplitude_and_snr_after_fixed_latency() {
        use super::super::consts::PV_LATENCY_FRAMES;

        let sample_rate = 48_000.0;
        let frequency = 997.0;
        let frames = 96_000;
        let rendered = render_tone(frequency, 1.0, sample_rate, frames);
        let start = PV_LATENCY_FRAMES + PV_FFT_SIZE * 4;
        let mut signal_energy = 0.0_f64;
        let mut error_energy = 0.0_f64;
        for (output_index, &actual) in rendered.iter().enumerate().skip(start) {
            let input_index = output_index - PV_LATENCY_FRAMES;
            let expected = 0.5
                * (2.0 * std::f32::consts::PI * frequency * input_index as f32 / sample_rate).sin();
            signal_energy += f64::from(expected * expected);
            let error = actual - expected;
            error_energy += f64::from(error * error);
        }
        let snr_db = 10.0 * (signal_energy / error_energy.max(f64::MIN_POSITIVE)).log10();
        let rms = (signal_energy / (frames - start) as f64).sqrt();
        let output_rms = (rendered[start..]
            .iter()
            .map(|sample| f64::from(sample * sample))
            .sum::<f64>()
            / (frames - start) as f64)
            .sqrt();
        assert!((output_rms - rms).abs() < 0.01, "RMS {output_rms} vs {rms}");
        assert!(snr_db > 35.0, "unity phase-vocoder SNR was {snr_db:.1} dB");
    }

    #[test]
    fn impulse_transient_energy_remains_localized_without_a_formant_claim() {
        use super::super::consts::PV_LATENCY_FRAMES;

        let mut channel = PhaseVocoderChannel::new();
        let frames = PV_LATENCY_FRAMES + PV_FFT_SIZE * 3;
        let mut output = vec![0.0; frames];
        for (frame, output_sample) in output.iter_mut().enumerate() {
            channel.input_buf[channel.input_fill] = if frame == 0 { 1.0 } else { 0.0 };
            channel.input_fill += 1;
            if channel.input_fill >= PV_FFT_SIZE {
                channel.process_hop(1.0);
            }
            if channel.output_fill > 0 {
                let index = channel.output_read % channel.output_accum.len();
                *output_sample = channel.output_accum[index];
                channel.output_accum[index] = 0.0;
                channel.output_read += 1;
                channel.output_fill -= 1;
            }
        }
        let total_energy: f32 = output.iter().map(|sample| sample * sample).sum();
        let local_start = PV_LATENCY_FRAMES.saturating_sub(PV_HOP_SIZE);
        let local_end = (PV_LATENCY_FRAMES + PV_HOP_SIZE + 1).min(output.len());
        let local_energy: f32 = output[local_start..local_end]
            .iter()
            .map(|sample| sample * sample)
            .sum();
        assert!(total_energy > 0.5);
        assert!(local_energy / total_energy > 0.95);
    }

    #[test]
    fn onset_flux_resets_peak_phase_and_shifted_impulse_stays_localized() {
        use super::super::consts::PV_LATENCY_FRAMES;

        let mut channel = PhaseVocoderChannel::new();
        channel.input_buf[channel.input_fill] = 1.0;
        channel.input_fill += 1;
        while channel.input_fill < PV_FFT_SIZE {
            channel.input_buf[channel.input_fill] = 0.0;
            channel.input_fill += 1;
        }
        channel.process_hop(1.05);
        assert!(
            channel.last_frame_was_transient,
            "the normalized positive spectral flux must identify an isolated attack"
        );

        let frames = PV_LATENCY_FRAMES + PV_FFT_SIZE * 3;
        let mut shifted = PhaseVocoderChannel::new();
        let mut output = vec![0.0; frames];
        for (frame, output_sample) in output.iter_mut().enumerate() {
            shifted.input_buf[shifted.input_fill] = if frame == 0 { 1.0 } else { 0.0 };
            shifted.input_fill += 1;
            if shifted.input_fill >= PV_FFT_SIZE {
                shifted.process_hop(1.05);
            }
            if shifted.output_fill > 0 {
                let index = shifted.output_read % shifted.output_accum.len();
                *output_sample = shifted.output_accum[index];
                shifted.output_accum[index] = 0.0;
                shifted.output_read += 1;
                shifted.output_fill -= 1;
            }
        }
        let total_energy: f32 = output.iter().map(|sample| sample * sample).sum();
        let local_start = PV_LATENCY_FRAMES.saturating_sub(PV_HOP_SIZE);
        let local_end = (PV_LATENCY_FRAMES + PV_HOP_SIZE + 1).min(output.len());
        let local_energy: f32 = output[local_start..local_end]
            .iter()
            .map(|sample| sample * sample)
            .sum();
        assert!(total_energy > 0.1, "shifted transient was lost");
        assert!(
            local_energy / total_energy > 0.9,
            "shifted transient smeared outside one hop: local/total={}",
            local_energy / total_energy
        );
    }

    #[test]
    fn remapped_transients_preserve_arbitrary_within_frame_time_origins() {
        use super::super::consts::PV_LATENCY_FRAMES;

        for ratio in [0.95_f32, 1.05] {
            for offset in [1usize, 127, 255, 511, 549, 777, 1023, 1537] {
                let frames = offset + PV_LATENCY_FRAMES + PV_FFT_SIZE * 3;
                let mut input = vec![0.0_f32; frames];
                input[offset] = 1.0;
                let output = render_signal(&input, ratio);
                let (peak_index, peak) = output
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| {
                        a.abs()
                            .partial_cmp(&b.abs())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .unwrap();
                let expected = offset + PV_LATENCY_FRAMES;
                assert!(
                    peak_index.abs_diff(expected) <= 1,
                    "ratio {ratio}, input offset {offset}: attack peak moved to {peak_index}, \
                     expected {expected}"
                );
                assert!(
                    peak.abs() > 0.1,
                    "ratio {ratio}, input offset {offset}: attack peak was lost ({peak})"
                );
                let total_energy: f32 = output.iter().map(|sample| sample * sample).sum();
                let start = expected.saturating_sub(PV_HOP_SIZE);
                let end = (expected + PV_HOP_SIZE + 1).min(output.len());
                let local_energy: f32 = output[start..end]
                    .iter()
                    .map(|sample| sample * sample)
                    .sum();
                assert!(
                    local_energy / total_energy > 0.9,
                    "ratio {ratio}, input offset {offset}: attack smeared, local/total={}",
                    local_energy / total_energy
                );
            }
        }
    }

    #[test]
    fn remapped_attack_on_a_sustained_harmonic_bed_keeps_its_time_origin() {
        use super::super::consts::PV_LATENCY_FRAMES;

        let sample_rate = 48_000.0_f32;
        for ratio in [0.95_f32, 1.05] {
            for offset in [
                PV_FFT_SIZE * 2 + 37,
                PV_FFT_SIZE * 2 + 255,
                PV_FFT_SIZE * 2 + 499,
            ] {
                let frames = offset + PV_LATENCY_FRAMES + PV_FFT_SIZE * 3;
                let bed = (0..frames)
                    .map(|frame| {
                        let time = frame as f32 / sample_rate;
                        0.12 * (2.0 * std::f32::consts::PI * 311.0 * time).sin()
                            + 0.06 * (2.0 * std::f32::consts::PI * 622.0 * time + 0.4).sin()
                            + 0.03 * (2.0 * std::f32::consts::PI * 933.0 * time - 0.2).sin()
                    })
                    .collect::<Vec<_>>();
                let mut attacked = bed.clone();
                attacked[offset] += 1.0;
                // A short asymmetric tail exercises an attack rather than only
                // a mathematically isolated delta.
                attacked[offset + 1] += 0.35;
                attacked[offset + 2] -= 0.15;

                let bed_output = render_signal(&bed, ratio);
                let attacked_output = render_signal(&attacked, ratio);
                let residual = attacked_output
                    .iter()
                    .zip(&bed_output)
                    .map(|(attacked, bed)| attacked - bed)
                    .collect::<Vec<_>>();
                // The phase reset deliberately changes the continuing tonal
                // bed's phase, so raw subtraction contains a long low-frequency
                // residual unrelated to attack timing. A second difference
                // rejects that known smooth bed while preserving the added
                // broadband attack at the same sample index.
                let mut attack_residual = vec![0.0_f32; residual.len()];
                for sample in 1..residual.len().saturating_sub(1) {
                    attack_residual[sample] =
                        residual[sample - 1] - 2.0 * residual[sample] + residual[sample + 1];
                }
                let (peak_index, peak) = attack_residual
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| {
                        a.abs()
                            .partial_cmp(&b.abs())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .unwrap();
                let expected = offset + PV_LATENCY_FRAMES;
                assert!(
                    peak_index.abs_diff(expected) <= 2,
                    "ratio {ratio}, bed attack offset {offset}: residual peak moved to \
                     {peak_index}, expected {expected}"
                );
                assert!(
                    peak.abs() > 0.08,
                    "ratio {ratio}, bed attack offset {offset}: residual attack was lost ({peak})"
                );
                let start = expected.saturating_sub(PV_HOP_SIZE);
                let end = (expected + PV_HOP_SIZE + 1).min(residual.len());
                let local_energy: f32 = attack_residual[start..end]
                    .iter()
                    .map(|sample| sample * sample)
                    .sum();
                // Compare the one-hop attack neighborhood with a bounded
                // four-hop transient region around the expected onset.
                let region_start = expected.saturating_sub(PV_HOP_SIZE * 2);
                let region_end = (expected + PV_HOP_SIZE * 2 + 1).min(attack_residual.len());
                let transient_region_energy: f32 = attack_residual[region_start..region_end]
                    .iter()
                    .map(|sample| sample * sample)
                    .sum();
                assert!(
                    local_energy / transient_region_energy > 0.5,
                    "ratio {ratio}, bed attack offset {offset}: residual smeared, \
                     local/region={}",
                    local_energy / transient_region_energy
                );
            }
        }
    }

    #[test]
    fn identity_phase_locking_preserves_interchannel_phase_offset() {
        let sample_rate = 48_000.0;
        let input_hz = 997.0;
        let frames = 96_000;
        for ratio in [0.95, 1.0, 1.05] {
            for expected_offset in [std::f32::consts::FRAC_PI_2, std::f32::consts::PI] {
                let left = render_tone_with_phase(input_hz, ratio, sample_rate, frames, 0.0);
                let right =
                    render_tone_with_phase(input_hz, ratio, sample_rate, frames, expected_offset);
                let start = PV_FFT_SIZE * 6;
                let output_hz = input_hz * ratio;
                let left_phase = measured_phase(&left[start..], output_hz, sample_rate);
                let right_phase = measured_phase(&right[start..], output_hz, sample_rate);
                let phase_error = wrap_phase((right_phase - left_phase) - expected_offset);
                assert!(
                    phase_error.abs() < 0.12,
                    "ratio {ratio}, offset {expected_offset}: independent channels lost their \
                     phase relationship: error={phase_error} rad"
                );
            }
        }
    }

    #[test]
    fn peak_locked_voiced_harmonic_stack_retains_resolved_partials() {
        let sample_rate = 48_000.0;
        let fundamental = 173.0_f32;
        let ratio = 1.04_f32;
        let frames = 96_000;
        let input = (0..frames)
            .map(|frame| {
                let time = frame as f32 / sample_rate;
                (1..=8)
                    .map(|harmonic| {
                        let harmonic = harmonic as f32;
                        0.3 / harmonic
                            * (2.0 * std::f32::consts::PI * fundamental * harmonic * time).sin()
                    })
                    .sum::<f32>()
            })
            .collect::<Vec<_>>();
        let output = render_signal(&input, ratio);
        let steady = &output[PV_FFT_SIZE * 6..];
        for harmonic in 1..=6 {
            let harmonic = harmonic as f32;
            let expected = fundamental * harmonic * ratio;
            let on_partial = spectral_amplitude(steady, expected, sample_rate);
            let between_partials =
                spectral_amplitude(steady, expected + fundamental * ratio * 0.5, sample_rate);
            assert!(
                on_partial > 0.025 / harmonic,
                "shifted voiced harmonic {harmonic} was lost: amplitude={on_partial}"
            );
            assert!(
                on_partial > between_partials * 4.0,
                "harmonic {harmonic} smeared between peaks: on={on_partial}, off={between_partials}"
            );
        }
    }

    #[test]
    fn formant_transport_keeps_broad_peaks_at_absolute_frequencies() {
        let sample_rate = 48_000.0_f32;
        let fundamental = 100.0_f32;
        let frames = 96_000;
        let input = (0..frames)
            .map(|frame| {
                let time = frame as f32 / sample_rate;
                (1..=40)
                    .map(|harmonic| {
                        let frequency = fundamental * harmonic as f32;
                        let first_formant =
                            (-(frequency - 700.0).powi(2) / (2.0 * 180.0_f32.powi(2))).exp();
                        let second_formant =
                            (-(frequency - 1_800.0).powi(2) / (2.0 * 240.0_f32.powi(2))).exp();
                        (0.015 + 0.12 * first_formant + 0.08 * second_formant)
                            * (2.0 * std::f32::consts::PI * frequency * time).sin()
                    })
                    .sum::<f32>()
            })
            .collect::<Vec<_>>();
        let uniform = render_signal_with_formant_strength(&input, 1.2, 0.0);
        let preserved = render_signal_with_formant_strength(&input, 1.2, 1.0);
        let start = PV_FFT_SIZE * 6;
        let uniform_first = spectral_amplitude(&uniform[start..], 700.0, sample_rate);
        let preserved_first = spectral_amplitude(&preserved[start..], 700.0, sample_rate);
        let uniform_shifted = spectral_amplitude(&uniform[start..], 840.0, sample_rate);
        let preserved_shifted = spectral_amplitude(&preserved[start..], 840.0, sample_rate);
        assert!(uniform_first.is_finite() && preserved_first.is_finite());
        assert!(
            preserved_first > uniform_first * 1.15,
            "formant at 700 Hz was not restored: uniform={uniform_first}, preserved={preserved_first}"
        );
        assert!(
            preserved_shifted < uniform_shifted,
            "shifted formant was not attenuated: uniform={uniform_shifted}, preserved={preserved_shifted}"
        );
    }

    #[test]
    fn repeated_percussive_attacks_remain_localized_after_correction() {
        use super::super::consts::PV_LATENCY_FRAMES;

        let spacing = PV_FFT_SIZE * 6;
        let attacks = 6usize;
        let frames = PV_LATENCY_FRAMES + spacing * (attacks + 1);
        let mut input = vec![0.0_f32; frames];
        for attack in 0..attacks {
            input[attack * spacing] = if attack % 2 == 0 { 1.0 } else { -1.0 };
        }
        let output = render_signal(&input, 0.95);
        let total_energy: f32 = output.iter().map(|sample| sample * sample).sum();
        let mut localized_energy = 0.0_f32;
        for attack in 0..attacks {
            let expected = attack * spacing + PV_LATENCY_FRAMES;
            let start = expected.saturating_sub(PV_HOP_SIZE);
            let end = (expected + PV_HOP_SIZE + 1).min(output.len());
            localized_energy += output[start..end]
                .iter()
                .map(|sample| sample * sample)
                .sum::<f32>();
            let peak = output[start..end]
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| {
                    a.abs()
                        .partial_cmp(&b.abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap();
            assert!(
                peak.1.abs() > 0.1,
                "percussive attack {attack} lost at corrected output"
            );
        }
        assert!(total_energy > 0.5);
        assert!(
            localized_energy / total_energy > 0.85,
            "percussive energy smeared between attacks: localized/total={}",
            localized_energy / total_energy
        );
    }
}
