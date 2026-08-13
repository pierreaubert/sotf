const RNNOISE_FRAME_SIZE: usize = 480;
const LINKED_STEREO_COHERENCE_THRESHOLD: f32 = 0.75;
const BYPASS_CROSSFADE_SAMPLES: usize = RNNOISE_FRAME_SIZE;
/// Maximum fraction of a new detector gain applied at one 10 ms model frame.
/// Keeping this frame-level (rather than sample-level) avoids image-changing
/// gain modulation while preventing cancellation-prone stereo from pumping.
const LINKED_STEREO_GAIN_SMOOTHING: f32 = 0.2;

/// RNNoise speech-denoising backend.
///
/// # Constraints
/// - Only supports 48 kHz sample rate (hard-coded by RNNoise / nnnoiseless).
/// - Host block sizes may be arbitrary; input is framed internally in 480-sample quanta.
/// - Reports a fixed latency of 480 samples regardless of bypass state.
/// - A pre-seeded 480-sample output queue provides a constant startup delay.
pub struct RnnoiseBackend {
    denoisers: Vec<Box<nnnoiseless::DenoiseState>>,
    channels: usize,
    sample_rate: u32,
    accum_buffers: Vec<Vec<f32>>,
    /// Processed signal, delayed by one model frame.
    output_buffers: Vec<Vec<f32>>,
    /// Unprocessed signal with the identical delay used for click-free bypass.
    dry_output_buffers: Vec<Vec<f32>>,
    /// Monotonically increasing write head (wraps modulo ring_size internally
    /// but the absolute count is stored for simplicity in available-sample
    /// arithmetic). Wrapped on every read/write via `% ring_size`.
    output_write_pos: usize,
    output_read_pos: usize,
    accum_fill: usize,
    /// Per-channel scratch buffers pre-allocated during `initialize`.
    /// Avoids stack growth inside the real-time audio callback.
    scratch_input: Vec<Vec<f32>>,
    scratch_output: Vec<Vec<f32>>,
    /// Common gain used for linked stereo.  It is smoothed across model frames
    /// so detector changes near L/R cancellation cannot create hard amplitude
    /// steps on both channels.
    linked_stereo_gain: f32,
    bypass_mix: f32,
    bypass_target: f32,
    bypass_initialized: bool,
}

impl Default for RnnoiseBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl RnnoiseBackend {
    pub fn new() -> Self {
        Self {
            denoisers: Vec::new(),
            channels: 0,
            sample_rate: 48000,
            accum_buffers: Vec::new(),
            output_buffers: Vec::new(),
            dry_output_buffers: Vec::new(),
            output_write_pos: 0,
            output_read_pos: 0,
            accum_fill: 0,
            scratch_input: Vec::new(),
            scratch_output: Vec::new(),
            linked_stereo_gain: 1.0,
            bypass_mix: 0.0,
            bypass_target: 0.0,
            bypass_initialized: false,
        }
    }

    /// Initialise for the given sample rate and channel count.
    ///
    /// Returns `Err` if `sample_rate != 48000`; RNNoise band edges and FFT
    /// sizes are hard-coded for 48 kHz.
    pub fn initialize(&mut self, sample_rate: u32, channels: usize) -> Result<(), String> {
        if sample_rate != 48000 {
            return Err(format!(
                "RNNoise only supports 48 kHz; got {} Hz",
                sample_rate
            ));
        }
        if !(1..=2).contains(&channels) {
            return Err(format!(
                "RNNoise supports mono or stereo only; got {channels} channels"
            ));
        }
        nnnoiseless::prepare();
        self.sample_rate = sample_rate;
        self.channels = channels;
        self.denoisers = (0..channels)
            .map(|_| nnnoiseless::DenoiseState::new())
            .collect::<Vec<_>>();
        self.accum_buffers = vec![vec![0.0; RNNOISE_FRAME_SIZE]; channels];
        // Ring buffer: 4× frame size so even back-to-back full frames never
        // wrap back onto unread data before the reader catches up.
        let ring_size = RNNOISE_FRAME_SIZE * 4;
        self.output_buffers = vec![vec![0.0; ring_size]; channels];
        self.dry_output_buffers = vec![vec![0.0; ring_size]; channels];
        // Pre-allocate scratch buffers used inside the hot processing loop.
        self.scratch_input = vec![vec![0.0; RNNOISE_FRAME_SIZE]; channels];
        self.scratch_output = vec![vec![0.0; RNNOISE_FRAME_SIZE]; channels];
        // Keep one model frame of zeroes ahead of the write head. This gives
        // every input sample the same declared latency, including startup.
        self.output_write_pos = RNNOISE_FRAME_SIZE;
        self.output_read_pos = 0;
        self.accum_fill = 0;
        self.linked_stereo_gain = 1.0;
        self.bypass_mix = 0.0;
        self.bypass_target = 0.0;
        self.bypass_initialized = false;
        Ok(())
    }

    pub fn process(
        &mut self,
        buffer: &mut [f32],
        num_frames: usize,
        channels: usize,
        bypass: bool,
    ) -> usize {
        if self.denoisers.is_empty() || channels != self.channels {
            return 0;
        }
        let Some(required_samples) = num_frames.checked_mul(channels) else {
            return 0;
        };
        if required_samples > buffer.len() {
            return 0;
        }

        let ch_count = channels;
        self.bypass_target = if bypass { 1.0 } else { 0.0 };
        if !self.bypass_initialized {
            self.bypass_mix = self.bypass_target;
            self.bypass_initialized = true;
        }

        for frame in 0..num_frames {
            for ch in 0..ch_count {
                let sample = buffer[frame * channels + ch];
                self.accum_buffers[ch][self.accum_fill] = if sample.is_finite() {
                    sample.clamp(-1.0, 1.0)
                } else {
                    0.0
                };
            }
            self.accum_fill += 1;

            if self.accum_fill == RNNOISE_FRAME_SIZE {
                // The dry stream is always queued and the model is always
                // advanced. Bypass is a latency-aligned output crossfade, not
                // a frozen-state alternate topology.
                for ch in 0..ch_count {
                    let ring_size = self.dry_output_buffers[ch].len();
                    for (i, &sample) in self.accum_buffers[ch].iter().enumerate() {
                        self.dry_output_buffers[ch][(self.output_write_pos + i) % ring_size] =
                            sample;
                    }
                }

                if ch_count == 2 {
                    let mut input_power_sum = 0.0f32;
                    let mut output_power_sum = 0.0f32;
                    // Use a mono detector only when the stereo signal is
                    // sufficiently coherent.  The normalized coherence is
                    // one for identical channels, zero for anti-phase, and
                    // one half for a hard-panned signal.
                    let mut mono_input_power_sum = 0.0f32;
                    let mut stereo_input_power_sum = 0.0f32;
                    for i in 0..RNNOISE_FRAME_SIZE {
                        let mono = (self.accum_buffers[0][i] + self.accum_buffers[1][i]) * 0.5;
                        self.scratch_input[0][i] = mono * 32768.0;
                        mono_input_power_sum += mono * mono;
                        stereo_input_power_sum += self.accum_buffers[0][i]
                            * self.accum_buffers[0][i]
                            + self.accum_buffers[1][i] * self.accum_buffers[1][i];
                    }

                    let coherence = if stereo_input_power_sum > 1e-10 {
                        (2.0 * mono_input_power_sum / stereo_input_power_sum).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };

                    if coherence >= LINKED_STEREO_COHERENCE_THRESHOLD {
                        // Coherent stereo: process one detector and apply
                        // its frame-level gain to both channels.
                        self.denoisers[0]
                            .process_frame(&mut self.scratch_output[0], &self.scratch_input[0]);
                        for i in 0..RNNOISE_FRAME_SIZE {
                            let mono_out = self.scratch_output[0][i] / 32768.0;
                            let ring_size = self.output_buffers[0].len();
                            self.output_buffers[0][(self.output_write_pos + i) % ring_size] =
                                self.accum_buffers[0][i];
                            self.output_buffers[1][(self.output_write_pos + i) % ring_size] =
                                self.accum_buffers[1][i];
                            output_power_sum += mono_out * mono_out;
                        }
                        let gain = self.update_linked_stereo_gain(linked_stereo_gain(
                            mono_input_power_sum,
                            output_power_sum,
                        ));
                        for i in 0..RNNOISE_FRAME_SIZE {
                            let ring_size = self.output_buffers[0].len();
                            self.output_buffers[0][(self.output_write_pos + i) % ring_size] *= gain;
                            self.output_buffers[1][(self.output_write_pos + i) % ring_size] *= gain;
                        }
                    } else {
                        // Anti-phase, hard-panned, and otherwise wide
                        // material can cancel in a mono detector.  Run
                        // independent detectors to obtain a stable shared
                        // frame gain, then apply that gain to the original
                        // channels so the stereo image is not altered.
                        for ch in 0..2 {
                            self.scratch_input[ch].copy_from_slice(&self.accum_buffers[ch]);
                            for sample in &mut self.scratch_input[ch] {
                                *sample *= 32768.0;
                            }
                            self.denoisers[ch].process_frame(
                                &mut self.scratch_output[ch],
                                &self.scratch_input[ch],
                            );
                            for sample in &mut self.scratch_output[ch] {
                                *sample /= 32768.0;
                            }
                            input_power_sum += self.accum_buffers[ch]
                                .iter()
                                .map(|sample| sample * sample)
                                .sum::<f32>();
                            output_power_sum += self.scratch_output[ch]
                                .iter()
                                .map(|sample| sample * sample)
                                .sum::<f32>();
                        }
                        let gain = self.update_linked_stereo_gain(linked_stereo_gain(
                            input_power_sum,
                            output_power_sum,
                        ));
                        for i in 0..RNNOISE_FRAME_SIZE {
                            let ring_size = self.output_buffers[0].len();
                            self.output_buffers[0][(self.output_write_pos + i) % ring_size] =
                                self.accum_buffers[0][i] * gain;
                            self.output_buffers[1][(self.output_write_pos + i) % ring_size] =
                                self.accum_buffers[1][i] * gain;
                        }
                    }
                } else {
                    // Mono processing.
                    for ch in 0..ch_count {
                        self.scratch_input[ch].copy_from_slice(&self.accum_buffers[ch]);
                        for s in &mut self.scratch_input[ch] {
                            *s *= 32768.0;
                        }

                        self.denoisers[ch]
                            .process_frame(&mut self.scratch_output[ch], &self.scratch_input[ch]);

                        for s in &mut self.scratch_output[ch] {
                            *s /= 32768.0;
                        }
                        let ring_size = self.output_buffers[ch].len();
                        for (i, &sample) in self.scratch_output[ch].iter().enumerate() {
                            self.output_buffers[ch][(self.output_write_pos + i) % ring_size] =
                                sample;
                        }
                    }
                }
                self.output_write_pos += RNNOISE_FRAME_SIZE;

                self.accum_fill = 0;
            }
        }

        let available = self.output_write_pos.saturating_sub(self.output_read_pos);
        let to_write = num_frames.min(available);

        if ch_count > 0 {
            let ring_size = self.output_buffers[0].len();
            for frame in 0..to_write {
                let read = (self.output_read_pos + frame) % ring_size;
                self.advance_bypass_mix();
                for ch in 0..ch_count {
                    let wet = self.output_buffers[ch][read];
                    let dry = self.dry_output_buffers[ch][read];
                    buffer[frame * channels + ch] = wet + self.bypass_mix * (dry - wet);
                }
            }
            self.output_read_pos += to_write;
        }

        // The pre-seeded delay queue guarantees a full output block for every
        // valid call, including partial model frames.
        for frame in to_write..num_frames {
            for ch in 0..channels {
                buffer[frame * channels + ch] = 0.0;
            }
        }
        let ring_size = self.output_buffers[0].len();
        if self.output_write_pos >= ring_size * 2 {
            let delta = self.output_write_pos - self.output_read_pos;
            self.output_write_pos = delta;
            self.output_read_pos = 0;
        }

        num_frames
    }

    /// Reset processing state in place without heap allocation.
    pub fn reset(&mut self) {
        for denoiser in &mut self.denoisers {
            denoiser.reset();
        }
        for buf in &mut self.accum_buffers {
            buf.fill(0.0);
        }
        for buf in &mut self.output_buffers {
            buf.fill(0.0);
        }
        for buf in &mut self.dry_output_buffers {
            buf.fill(0.0);
        }
        for buf in &mut self.scratch_input {
            buf.fill(0.0);
        }
        for buf in &mut self.scratch_output {
            buf.fill(0.0);
        }
        self.output_write_pos = RNNOISE_FRAME_SIZE;
        self.output_read_pos = 0;
        self.accum_fill = 0;
        self.linked_stereo_gain = 1.0;
        self.bypass_mix = 0.0;
        self.bypass_target = 0.0;
        self.bypass_initialized = false;
    }

    /// Always returns 480 (RNNOISE_FRAME_SIZE) regardless of bypass state.
    ///
    /// Plugin hosts require a fixed, constant latency after initialisation.
    /// Returning 0 when disabled would cause phase misalignment in parallel
    /// processing chains.
    pub fn latency_samples(&self) -> usize {
        RNNOISE_FRAME_SIZE
    }

    fn update_linked_stereo_gain(&mut self, target: f32) -> f32 {
        let target = if target.is_finite() {
            target.clamp(0.0, 1.0)
        } else {
            self.linked_stereo_gain
        };
        self.linked_stereo_gain +=
            LINKED_STEREO_GAIN_SMOOTHING * (target - self.linked_stereo_gain);
        self.linked_stereo_gain = self.linked_stereo_gain.clamp(0.0, 1.0);
        self.linked_stereo_gain
    }

    fn advance_bypass_mix(&mut self) {
        let step = 1.0 / BYPASS_CROSSFADE_SAMPLES as f32;
        if self.bypass_mix < self.bypass_target {
            self.bypass_mix = (self.bypass_mix + step).min(self.bypass_target);
        } else if self.bypass_mix > self.bypass_target {
            self.bypass_mix = (self.bypass_mix - step).max(self.bypass_target);
        }
    }
}

fn linked_stereo_gain(mono_in: f32, mono_out: f32) -> f32 {
    if mono_in <= 1e-10 || mono_out <= 0.0 || !mono_in.is_finite() || !mono_out.is_finite() {
        return 0.0;
    }
    (mono_out / mono_in).sqrt().clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_common_gain_preserves_stereo(input: &[f32]) {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 2).unwrap();
        let mut first = input.to_vec();
        backend.process(&mut first, RNNOISE_FRAME_SIZE, 2, false);
        let mut output = vec![0.0; RNNOISE_FRAME_SIZE * 2];
        backend.process(&mut output, RNNOISE_FRAME_SIZE, 2, false);

        let input_energy: f32 = input.iter().map(|sample| sample * sample).sum();
        let gain = input
            .iter()
            .zip(&output)
            .map(|(dry, wet)| dry * wet)
            .sum::<f32>()
            / input_energy;
        assert!((0.0..=1.0).contains(&gain), "invalid linked gain: {gain}");
        let max_residual = input
            .iter()
            .zip(&output)
            .map(|(dry, wet)| (wet - gain * dry).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            max_residual < 1e-5,
            "linked processing changed the stereo image: residual={max_residual}"
        );
    }

    #[test]
    fn vendored_model_matches_reference_after_workspace_reuse_refactor() {
        let input_bytes = include_bytes!("../../../../3rdparties/nnnoiseless/tests/testing.raw");
        let output_bytes =
            include_bytes!("../../../../3rdparties/nnnoiseless/tests/reference_output.raw");
        let input: Vec<f32> = input_bytes
            .chunks_exact(2)
            .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]) as f32)
            .collect();
        let reference: Vec<i16> = output_bytes
            .chunks_exact(2)
            .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]))
            .collect();
        let mut state = nnnoiseless::DenoiseState::new();
        let mut frame = [0.0; RNNOISE_FRAME_SIZE];
        let mut actual = Vec::with_capacity(reference.len());
        for (index, input_frame) in input.chunks_exact(RNNOISE_FRAME_SIZE).enumerate() {
            state.process_frame(&mut frame, input_frame);
            if index > 0 {
                actual.extend(frame.iter().map(|sample| *sample as i16));
            }
        }
        assert_eq!(actual.len(), reference.len());
        let xx: f64 = reference
            .iter()
            .map(|sample| *sample as f64 * *sample as f64)
            .sum();
        let yy: f64 = actual
            .iter()
            .map(|sample| *sample as f64 * *sample as f64)
            .sum();
        let xy: f64 = reference
            .iter()
            .zip(&actual)
            .map(|(a, b)| *a as f64 * *b as f64)
            .sum();
        let correlation = xy / (xx.sqrt() * yy.sqrt());
        assert!(
            (correlation - 1.0).abs() < 1e-4,
            "reference correlation={correlation}"
        );
    }

    #[test]
    fn model_processes_on_a_bounded_audio_thread_stack() {
        std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(|| {
                let mut state = nnnoiseless::DenoiseState::new();
                let input = [0.1; RNNOISE_FRAME_SIZE];
                let mut output = [0.0; RNNOISE_FRAME_SIZE];
                for _ in 0..8 {
                    state.process_frame(&mut output, &input);
                }
                assert!(output.iter().all(|sample| sample.is_finite()));
            })
            .unwrap()
            .join()
            .expect("RNNoise exceeded the bounded callback stack");
    }

    #[test]
    fn creates_state_per_channel() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 2).unwrap();
        assert_eq!(backend.channels, 2);
        assert_eq!(backend.latency_samples(), 480);
    }

    #[test]
    fn initialize_rejects_non_48khz() {
        let mut backend = RnnoiseBackend::new();
        assert!(backend.initialize(44100, 1).is_err());
        assert!(backend.initialize(96000, 1).is_err());
        assert!(backend.initialize(48000, 1).is_ok());
    }

    #[test]
    fn silence_stays_near_silent() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1).unwrap();

        // Two full frames: the first is the declared zero-valued latency
        // region and the second contains the delayed processed first frame.
        let mut buffer = vec![0.0f32; 960];
        backend.process(&mut buffer, 960, 1, false);

        for (i, &sample) in buffer.iter().enumerate() {
            assert!(
                sample.abs() < 0.01,
                "Sample {i} should be near zero, got {sample}"
            );
        }
    }

    #[test]
    fn latency_is_fixed_at_480() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1).unwrap();
        assert_eq!(backend.latency_samples(), RNNOISE_FRAME_SIZE);
    }

    /// Verify that the declared 480-sample startup latency is emitted as
    /// zeroes without discarding the first processed input frame.
    #[test]
    fn startup_emits_zero_valued_latency_region() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1).unwrap();

        // Feed one frame of non-zero audio.
        let mut first = vec![0.1f32; RNNOISE_FRAME_SIZE];
        backend.process(&mut first, RNNOISE_FRAME_SIZE, 1, false);

        // The first 480 output samples are the pre-seeded latency queue.
        for (i, &s) in first.iter().enumerate() {
            assert_eq!(s, 0.0, "Sample {i} of startup latency should be zero");
        }
    }

    /// After the latency region, the second frame carries the processed first
    /// input frame rather than deleting it.
    #[test]
    fn second_frame_produces_output() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1).unwrap();

        let mut first = vec![0.5f32; RNNOISE_FRAME_SIZE];
        backend.process(&mut first, RNNOISE_FRAME_SIZE, 1, false);

        // Second frame: should produce output from the first processed frame.
        let mut second = vec![0.5f32; RNNOISE_FRAME_SIZE];
        backend.process(&mut second, RNNOISE_FRAME_SIZE, 1, false);

        let energy: f32 = second.iter().map(|s| s * s).sum();
        assert!(
            energy > 0.0,
            "Second frame should contain the delayed first processed frame"
        );
    }

    #[test]
    fn process_rejects_undersized_buffer() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 2).unwrap();
        let mut buffer = vec![0.0f32; 3];

        assert_eq!(backend.process(&mut buffer, 2, 2, false), 0);
    }

    #[test]
    fn linked_stereo_gain_is_finite_and_bounded() {
        assert_eq!(linked_stereo_gain(0.0, 1.0), 0.0);
        assert_eq!(linked_stereo_gain(1e-12, 1.0), 0.0);
        assert_eq!(linked_stereo_gain(1.0, 4.0), 1.0);
        assert_eq!(linked_stereo_gain(4.0, 1.0), 0.5);
        assert_eq!(linked_stereo_gain(1.0, f32::NAN), 0.0);
    }

    #[test]
    fn linked_stereo_gain_changes_are_smoothed_between_frames() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 2).unwrap();

        // A near-cancelled stereo frame can make the detector's target gain
        // change substantially from one 10 ms model frame to the next.  The
        // common gain must ramp rather than imposing a full-scale step on both
        // channels, which would turn the cancellation-prone signal into
        // audible amplitude modulation.
        let first = backend.update_linked_stereo_gain(0.0);
        assert!(first > 0.0 && first < 1.0);
        let second = backend.update_linked_stereo_gain(1.0);
        assert!(second > first && second < 1.0);
        assert!((second - first) <= LINKED_STEREO_GAIN_SMOOTHING + 1e-6);
    }

    #[test]
    fn anti_phase_stereo_does_not_collapse_to_silence() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 2).unwrap();

        // A linked mono detector must not treat a wide stereo signal as
        // silence.  The second call reads the first processed frame after the
        // fixed startup latency.
        let mut warmup = vec![0.0f32; RNNOISE_FRAME_SIZE * 2];
        for i in 0..RNNOISE_FRAME_SIZE {
            let sample = (i as f32 * 0.071).sin() * 0.35;
            warmup[2 * i] = sample;
            warmup[2 * i + 1] = -sample;
        }
        backend.process(&mut warmup, RNNOISE_FRAME_SIZE, 2, false);

        let mut output = vec![0.0f32; RNNOISE_FRAME_SIZE * 2];
        for i in 0..RNNOISE_FRAME_SIZE {
            let sample = (i as f32 * 0.071).sin() * 0.35;
            output[2 * i] = sample;
            output[2 * i + 1] = -sample;
        }
        backend.process(&mut output, RNNOISE_FRAME_SIZE, 2, false);

        let left_power: f32 = output.iter().step_by(2).map(|s| s * s).sum();
        let right_power: f32 = output.iter().skip(1).step_by(2).map(|s| s * s).sum();
        let cross: f32 = output
            .chunks_exact(2)
            .map(|stereo| stereo[0] * stereo[1])
            .sum();
        assert!(left_power > 1e-5, "anti-phase left channel was collapsed");
        assert!(right_power > 1e-5, "anti-phase right channel was collapsed");
        assert!(
            cross < -0.9 * (left_power * right_power).sqrt(),
            "anti-phase image was not preserved: cross={cross}, L={left_power}, R={right_power}"
        );
    }

    #[test]
    fn quadrature_stereo_preserves_phase_relationship() {
        let mut input = vec![0.0; RNNOISE_FRAME_SIZE * 2];
        for frame in 0..RNNOISE_FRAME_SIZE {
            let phase = frame as f32 * 0.071;
            input[2 * frame] = phase.sin() * 0.35;
            input[2 * frame + 1] = phase.cos() * 0.35;
        }
        assert_common_gain_preserves_stereo(&input);
    }

    #[test]
    fn uncorrelated_stereo_preserves_each_channel() {
        let mut input = vec![0.0; RNNOISE_FRAME_SIZE * 2];
        let mut left_state = 0x1234_5678_u32;
        let mut right_state = 0x9abc_def0_u32;
        for frame in 0..RNNOISE_FRAME_SIZE {
            left_state = left_state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            right_state = right_state.wrapping_mul(22_695_477).wrapping_add(1);
            input[2 * frame] = (left_state as f32 / u32::MAX as f32 - 0.5) * 0.7;
            input[2 * frame + 1] = (right_state as f32 / u32::MAX as f32 - 0.5) * 0.7;
        }
        assert_common_gain_preserves_stereo(&input);
    }

    #[test]
    fn hard_panned_stereo_keeps_active_channel_and_silence() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 2).unwrap();

        let mut warmup = vec![0.0f32; RNNOISE_FRAME_SIZE * 2];
        for i in 0..RNNOISE_FRAME_SIZE {
            warmup[2 * i] = (i as f32 * 0.083).sin() * 0.35;
        }
        backend.process(&mut warmup, RNNOISE_FRAME_SIZE, 2, false);

        let mut output = vec![0.0f32; RNNOISE_FRAME_SIZE * 2];
        for i in 0..RNNOISE_FRAME_SIZE {
            output[2 * i] = (i as f32 * 0.083).sin() * 0.35;
        }
        backend.process(&mut output, RNNOISE_FRAME_SIZE, 2, false);

        let left_power: f32 = output.iter().step_by(2).map(|s| s * s).sum();
        let right_power: f32 = output.iter().skip(1).step_by(2).map(|s| s * s).sum();
        assert!(
            left_power > 1e-5,
            "hard-panned active channel was collapsed"
        );
        assert!(
            right_power < 1e-12,
            "hard-panned silent channel was contaminated"
        );
    }

    #[test]
    fn unequal_stereo_levels_keep_their_image_ratio() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 2).unwrap();

        let mut warmup = vec![0.0f32; RNNOISE_FRAME_SIZE * 2];
        for i in 0..RNNOISE_FRAME_SIZE {
            let sample = (i as f32 * 0.067).sin() * 0.35;
            warmup[2 * i] = sample;
            warmup[2 * i + 1] = sample / 6.0;
        }
        backend.process(&mut warmup, RNNOISE_FRAME_SIZE, 2, false);

        let mut output = vec![0.0f32; RNNOISE_FRAME_SIZE * 2];
        for i in 0..RNNOISE_FRAME_SIZE {
            let sample = (i as f32 * 0.067).sin() * 0.35;
            output[2 * i] = sample;
            output[2 * i + 1] = sample / 6.0;
        }
        backend.process(&mut output, RNNOISE_FRAME_SIZE, 2, false);

        let left_power: f32 = output.iter().step_by(2).map(|s| s * s).sum();
        let right_power: f32 = output.iter().skip(1).step_by(2).map(|s| s * s).sum();
        assert!(
            left_power > 1e-5,
            "unequal stereo active channel was collapsed"
        );
        assert!(
            right_power > 1e-7,
            "unequal stereo quiet channel was collapsed"
        );
        let ratio = (left_power / right_power).sqrt();
        assert!(
            (ratio - 6.0).abs() < 0.01,
            "stereo level ratio changed: {ratio}"
        );
    }

    #[test]
    fn arbitrary_blocks_return_full_length_with_constant_startup_delay() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1).unwrap();
        for size in [1usize, 16, 63, 64, 127, 128, 256, 479, 480, 481, 512, 1024] {
            let mut buffer = vec![0.1; size];
            assert_eq!(backend.process(&mut buffer, size, 1, true), size);
            assert!(buffer.iter().all(|sample| sample.is_finite()));
        }
    }

    #[test]
    fn scratch_buffers_pre_allocated_after_initialize() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 2).unwrap();
        assert_eq!(backend.scratch_input.len(), 2);
        assert_eq!(backend.scratch_output.len(), 2);
        assert_eq!(backend.scratch_input[0].len(), RNNOISE_FRAME_SIZE);
    }

    #[test]
    fn reset_clears_state_without_reinitialize() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1).unwrap();

        // Process some audio to advance pointers.
        let mut buf = vec![0.5f32; 960];
        backend.process(&mut buf, 960, 1, false);
        assert!(backend.output_write_pos > RNNOISE_FRAME_SIZE);

        backend.reset();

        assert_eq!(backend.accum_fill, 0);
        assert_eq!(backend.output_write_pos, RNNOISE_FRAME_SIZE);
        assert_eq!(backend.output_read_pos, 0);
    }

    /// Regression test: reset() must not heap-allocate new DenoiseState objects.
    #[test]
    fn reset_does_not_reallocate_denoisers() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 2).unwrap();

        let ptrs_before: Vec<*const nnnoiseless::DenoiseState> = backend
            .denoisers
            .iter()
            .map(|d| d.as_ref() as *const _)
            .collect();

        backend.reset();

        let ptrs_after: Vec<*const nnnoiseless::DenoiseState> = backend
            .denoisers
            .iter()
            .map(|d| d.as_ref() as *const _)
            .collect();

        assert_eq!(
            ptrs_before, ptrs_after,
            "reset() must reuse existing DenoiseState objects"
        );
    }

    /// Regression: stereo channels must be processed with linked gain so that
    /// the stereo image does not collapse or shift randomly during noise-only
    /// passages.  With independent per-channel denoisers the suppression gain
    /// differs per channel; after the fix both channels receive the same gain.
    #[test]
    fn test_stereo_linked_gain_preserves_image() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 2).unwrap();

        // Warm-up: two frames to get past the first-frame discard.
        let mut warmup = vec![0.0f32; RNNOISE_FRAME_SIZE * 2 * 2]; // 2 frames, stereo
        for f in 0..(RNNOISE_FRAME_SIZE * 2) {
            // Correlated sine wave in both channels (same phase, same amplitude)
            let s = (f as f32 * 0.1).sin() * 0.3;
            warmup[f * 2] = s;
            warmup[f * 2 + 1] = s;
        }
        backend.process(&mut warmup, RNNOISE_FRAME_SIZE * 2, 2, false);

        // Now process a third frame where L and R have identical content.
        let mut frame = vec![0.0f32; RNNOISE_FRAME_SIZE * 2];
        for f in 0..RNNOISE_FRAME_SIZE {
            let s = (f as f32 * 0.1).sin() * 0.3;
            frame[f * 2] = s;
            frame[f * 2 + 1] = s;
        }
        backend.process(&mut frame, RNNOISE_FRAME_SIZE, 2, false);

        // With linked gain, L and R outputs should be almost identical.
        let mut max_diff = 0.0f32;
        for f in 0..RNNOISE_FRAME_SIZE {
            let diff = (frame[f * 2] - frame[f * 2 + 1]).abs();
            max_diff = max_diff.max(diff);
        }
        assert!(
            max_diff < 1e-4,
            "Stereo image broken: max(L-R) diff = {max_diff} (should be ~0 with linked gain)"
        );
    }

    #[test]
    fn bypass_copies_input_after_warmup() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1).unwrap();

        // Startup emits the pre-seeded latency queue.
        let mut first = vec![0.5f32; RNNOISE_FRAME_SIZE];
        backend.process(&mut first, RNNOISE_FRAME_SIZE, 1, true);
        for (i, &s) in first.iter().enumerate() {
            assert_eq!(
                s, 0.0,
                "startup-latency sample {i} should be zero in bypass"
            );
        }

        // The next block carries the first frame through unchanged (bypass).
        let mut second = vec![0.3f32; RNNOISE_FRAME_SIZE];
        backend.process(&mut second, RNNOISE_FRAME_SIZE, 1, true);
        for (i, &s) in second.iter().enumerate() {
            assert!(
                (s - 0.5).abs() < 1e-6,
                "bypass sample {i} should equal delayed input (0.5), got {s}"
            );
        }
    }

    #[test]
    fn bypass_keeps_model_state_warm_and_crossfades_back_to_wet() {
        let mut always_wet = RnnoiseBackend::new();
        let mut toggled = RnnoiseBackend::new();
        always_wet.initialize(48000, 1).unwrap();
        toggled.initialize(48000, 1).unwrap();

        for frame_index in 0..6 {
            let mut reference = vec![0.0; RNNOISE_FRAME_SIZE];
            for (index, sample) in reference.iter_mut().enumerate() {
                *sample = ((frame_index * RNNOISE_FRAME_SIZE + index) as f32 * 0.071).sin() * 0.3;
            }
            let mut actual = reference.clone();
            always_wet.process(&mut reference, RNNOISE_FRAME_SIZE, 1, false);
            toggled.process(&mut actual, RNNOISE_FRAME_SIZE, 1, frame_index < 3);

            if frame_index == 5 {
                let max_error = actual
                    .iter()
                    .zip(&reference)
                    .map(|(a, b)| (a - b).abs())
                    .fold(0.0_f32, f32::max);
                assert!(
                    max_error < 1e-6,
                    "re-enabled model resumed stale state: {max_error}"
                );
            }
        }
    }

    #[test]
    fn bypass_transition_is_bounded_and_latency_constant() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1).unwrap();
        let mut previous = 0.0_f32;
        let mut max_step = 0.0_f32;
        for bypass in [false, false, true, true, false, false] {
            let mut frame = vec![0.2; RNNOISE_FRAME_SIZE];
            backend.process(&mut frame, RNNOISE_FRAME_SIZE, 1, bypass);
            for sample in frame {
                max_step = max_step.max((sample - previous).abs());
                previous = sample;
            }
            assert_eq!(backend.latency_samples(), RNNOISE_FRAME_SIZE);
        }
        assert!(max_step < 0.25, "bypass transition clicked: {max_step}");
    }

    #[test]
    fn model_domain_is_sanitized_and_clamped_for_each_stereo_channel() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 2).unwrap();
        let mut buffer = vec![0.0; RNNOISE_FRAME_SIZE * 2 * 2];
        for frame in 0..RNNOISE_FRAME_SIZE {
            buffer[frame * 2] = if frame % 3 == 0 { f32::NAN } else { 1.0e30 };
            buffer[frame * 2 + 1] = if frame % 5 == 0 {
                f32::NEG_INFINITY
            } else {
                -1.0e30
            };
        }
        backend.process(&mut buffer, RNNOISE_FRAME_SIZE * 2, 2, true);
        assert!(buffer.iter().all(|sample| sample.is_finite()));
        assert!(buffer.iter().all(|sample| sample.abs() <= 1.0));
        let delayed = &buffer[RNNOISE_FRAME_SIZE * 2..];
        assert!(delayed.iter().step_by(2).any(|sample| *sample == 1.0));
        assert!(
            delayed
                .iter()
                .skip(1)
                .step_by(2)
                .any(|sample| *sample == -1.0)
        );
    }

    #[test]
    fn uninitialized_process_returns_zero_frames() {
        let mut backend = RnnoiseBackend::new();
        // Denoisers vector is empty because initialize() was never called.
        let mut buffer = vec![0.5f32; RNNOISE_FRAME_SIZE];
        assert_eq!(
            backend.process(&mut buffer, RNNOISE_FRAME_SIZE, 1, false),
            0
        );
    }

    #[test]
    fn channels_zero_is_rejected() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1).unwrap();
        let mut buffer = vec![0.5f32; RNNOISE_FRAME_SIZE];
        assert_eq!(
            backend.process(&mut buffer, RNNOISE_FRAME_SIZE, 0, false),
            0
        );
    }

    #[test]
    fn mono_processing_produces_output_after_latency() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1).unwrap();

        let mut first = vec![0.5f32; RNNOISE_FRAME_SIZE];
        backend.process(&mut first, RNNOISE_FRAME_SIZE, 1, false);

        // Second frame: should produce output from the mono path.
        let mut frame = vec![0.5f32; RNNOISE_FRAME_SIZE];
        backend.process(&mut frame, RNNOISE_FRAME_SIZE, 1, false);

        let energy: f32 = frame.iter().map(|s| s * s).sum();
        assert!(
            energy > 0.0,
            "Mono second frame should contain delayed processed output"
        );
    }

    #[test]
    fn initialize_rejects_undefined_multichannel_layouts() {
        let mut backend = RnnoiseBackend::new();
        assert!(backend.initialize(48000, 0).is_err());
        assert!(backend.initialize(48000, 3).is_err());
        assert!(backend.initialize(48000, 6).is_err());
        assert!(backend.initialize(48000, 12).is_err());
    }

    #[test]
    fn ring_buffer_wraps_after_many_frames() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1).unwrap();

        // Process enough 480-sample frames to exceed the ring buffer size (4×480 = 1920).
        // Each call reads a full frame, starting with the pre-seeded latency region.
        // We need 10 calls to push write_pos past 3840 (ring_size * 2).
        for i in 0..10 {
            let val = 0.1 * (i + 1) as f32;
            let mut frame = vec![val; RNNOISE_FRAME_SIZE];
            let written = backend.process(&mut frame, RNNOISE_FRAME_SIZE, 1, false);
            assert_eq!(written, RNNOISE_FRAME_SIZE);
        }

        // The wrap should have happened transparently; pointers must stay consistent.
        assert!(backend.output_write_pos >= backend.output_read_pos);
        let delta = backend.output_write_pos - backend.output_read_pos;
        assert!(delta <= backend.output_buffers[0].len());
    }

    #[test]
    fn sequential_calls_produce_continuous_output() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1).unwrap();

        let mut first = vec![0.5f32; RNNOISE_FRAME_SIZE];
        backend.process(&mut first, RNNOISE_FRAME_SIZE, 1, false);

        // Two sequential frames should both produce full output.
        let mut frame1 = vec![0.4f32; RNNOISE_FRAME_SIZE];
        let mut frame2 = vec![0.6f32; RNNOISE_FRAME_SIZE];

        let w1 = backend.process(&mut frame1, RNNOISE_FRAME_SIZE, 1, false);
        let w2 = backend.process(&mut frame2, RNNOISE_FRAME_SIZE, 1, false);

        assert_eq!(w1, RNNOISE_FRAME_SIZE);
        assert_eq!(w2, RNNOISE_FRAME_SIZE);

        let energy1: f32 = frame1.iter().map(|s| s * s).sum();
        let energy2: f32 = frame2.iter().map(|s| s * s).sum();
        assert!(energy1 > 0.0, "Frame 1 should have output");
        assert!(energy2 > 0.0, "Frame 2 should have output");
    }

    #[test]
    fn first_processed_frame_is_emitted_after_latency() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1).unwrap();

        // First call emits only the pre-seeded latency region.
        let mut first = vec![0.5f32; RNNOISE_FRAME_SIZE];
        backend.process(&mut first, RNNOISE_FRAME_SIZE, 1, false);
        assert!(first.iter().all(|sample| *sample == 0.0));

        // The next call emits the processed first input frame.
        let mut second = vec![0.3f32; RNNOISE_FRAME_SIZE];
        backend.process(&mut second, RNNOISE_FRAME_SIZE, 1, false);

        // The second call should have returned the first frame's processed audio.
        let energy: f32 = second.iter().map(|s| s * s).sum();
        assert!(energy > 0.0, "Second call should produce non-zero output");
    }
}
