const RNNOISE_FRAME_SIZE: usize = 480;

/// Monitoring data exposed by the RNNoise speech denoiser.
#[derive(Debug, Clone, Copy, Default)]
pub struct RnnoiseData {
    pub avg_reduction_db: f32,
}

/// RNNoise speech-denoising backend.
///
/// # Constraints
/// - Only supports 48 kHz sample rate (hard-coded by RNNoise / nnnoiseless).
/// - Block sizes passed to `process()` must be exact multiples of 480 samples.
/// - Reports a fixed latency of 480 samples regardless of bypass state.
/// - The first processed 480-sample frame is discarded to avoid RNNoise's
///   documented fade-in artifact; this adds a one-time 480-sample startup delay.
pub struct RnnoiseBackend {
    denoisers: Vec<Box<nnnoiseless::DenoiseState>>,
    channels: usize,
    sample_rate: u32,
    accum_buffers: Vec<Vec<f32>>,
    output_buffers: Vec<Vec<f32>>,
    /// Monotonically increasing write head (wraps modulo ring_size internally
    /// but the absolute count is stored for simplicity in available-sample
    /// arithmetic). Wrapped on every read/write via `% ring_size`.
    output_write_pos: usize,
    output_read_pos: usize,
    accum_fill: usize,
    avg_reduction_db: f32,
    /// Per-channel scratch buffers pre-allocated during `initialize`.
    /// Avoids stack growth inside the real-time audio callback.
    scratch_input: Vec<Vec<f32>>,
    scratch_output: Vec<Vec<f32>>,
    /// True once we have discarded the first 480-sample frame to remove
    /// nnnoiseless's documented fade-in artifact.
    first_frame_discarded: bool,
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
            output_write_pos: 0,
            output_read_pos: 0,
            accum_fill: 0,
            avg_reduction_db: 0.0,
            scratch_input: Vec::new(),
            scratch_output: Vec::new(),
            first_frame_discarded: false,
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
        // Pre-allocate scratch buffers used inside the hot processing loop.
        self.scratch_input = vec![vec![0.0; RNNOISE_FRAME_SIZE]; channels];
        self.scratch_output = vec![vec![0.0; RNNOISE_FRAME_SIZE]; channels];
        self.output_write_pos = 0;
        self.output_read_pos = 0;
        self.accum_fill = 0;
        self.avg_reduction_db = 0.0;
        self.first_frame_discarded = false;
        Ok(())
    }

    pub fn process(
        &mut self,
        buffer: &mut [f32],
        num_frames: usize,
        channels: usize,
        bypass: bool,
    ) -> usize {
        if self.denoisers.is_empty() || channels == 0 {
            return num_frames;
        }

        let ch_count = channels.min(self.channels);

        for frame in 0..num_frames {
            for ch in 0..ch_count {
                self.accum_buffers[ch][self.accum_fill] = buffer[frame * channels + ch];
            }
            self.accum_fill += 1;

            if self.accum_fill == RNNOISE_FRAME_SIZE {
                if bypass {
                    for ch in 0..ch_count {
                        let ring_size = self.output_buffers[ch].len();
                        for (i, &s) in self.accum_buffers[ch].iter().enumerate() {
                            self.output_buffers[ch][(self.output_write_pos + i) % ring_size] = s;
                        }
                    }
                } else {
                    let mut ch0_input_power = 0.0f32;
                    let mut ch0_output_power = 0.0f32;

                    for ch in 0..ch_count {
                        // Copy into pre-allocated scratch and scale to i16 range.
                        self.scratch_input[ch].copy_from_slice(&self.accum_buffers[ch]);
                        for s in &mut self.scratch_input[ch] {
                            *s *= 32767.0;
                        }

                        self.denoisers[ch]
                            .process_frame(&mut self.scratch_output[ch], &self.scratch_input[ch]);

                        for s in &mut self.scratch_output[ch] {
                            *s /= 32767.0;
                        }

                        if ch == 0 {
                            ch0_input_power =
                                self.accum_buffers[0].iter().map(|x| x * x).sum::<f32>()
                                    / RNNOISE_FRAME_SIZE as f32;
                            ch0_output_power =
                                self.scratch_output[0].iter().map(|x| x * x).sum::<f32>()
                                    / RNNOISE_FRAME_SIZE as f32;
                        }

                        let ring_size = self.output_buffers[ch].len();
                        for (i, &s) in self.scratch_output[ch].iter().enumerate() {
                            self.output_buffers[ch][(self.output_write_pos + i) % ring_size] = s;
                        }
                    }

                    if ch0_input_power > 1e-10 {
                        self.avg_reduction_db = 0.9 * self.avg_reduction_db
                            + 0.1 * 10.0 * (ch0_input_power / ch0_output_power.max(1e-10)).log10();
                    }
                }
                if !self.first_frame_discarded {
                    self.first_frame_discarded = true;
                    self.output_read_pos += RNNOISE_FRAME_SIZE;
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
                for ch in 0..ch_count {
                    buffer[frame * channels + ch] = self.output_buffers[ch][read];
                }
            }
            self.output_read_pos += to_write;
        }

        // Zero out any frames that could not be filled from the ring buffer.
        // With correct usage (multiples of 480 after the first-frame warm-up),
        // `to_write` should equal `num_frames` on every call.
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

        to_write
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
        for buf in &mut self.scratch_input {
            buf.fill(0.0);
        }
        for buf in &mut self.scratch_output {
            buf.fill(0.0);
        }
        self.output_write_pos = 0;
        self.output_read_pos = 0;
        self.accum_fill = 0;
        self.avg_reduction_db = 0.0;
        self.first_frame_discarded = false;
    }

    /// Always returns 480 (RNNOISE_FRAME_SIZE) regardless of bypass state.
    ///
    /// Plugin hosts require a fixed, constant latency after initialisation.
    /// Returning 0 when disabled would cause phase misalignment in parallel
    /// processing chains.
    pub fn latency_samples(&self) -> usize {
        RNNOISE_FRAME_SIZE
    }

    pub fn data(&self) -> RnnoiseData {
        RnnoiseData {
            avg_reduction_db: self.avg_reduction_db,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        // Two full frames: the first is discarded (warm-up), the second
        // produces real output — both should be near-silent for silence input.
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

    /// Verify that the first 480 samples output are all zero (the warm-up
    /// frame was discarded and the ring buffer fills up on the second call).
    #[test]
    fn first_frame_is_discarded() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1).unwrap();

        // Feed one frame of non-zero audio.
        let mut first = vec![0.1f32; RNNOISE_FRAME_SIZE];
        backend.process(&mut first, RNNOISE_FRAME_SIZE, 1, false);

        // The first 480 output samples should be zeroed because the warm-up
        // frame is discarded and the ring buffer has no output yet.
        for (i, &s) in first.iter().enumerate() {
            assert_eq!(s, 0.0, "Sample {i} of warm-up frame should be zero");
        }
    }

    /// After warm-up, the second frame should carry non-trivial output.
    #[test]
    fn second_frame_produces_output() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1).unwrap();

        // First frame: warm-up (discarded).
        let mut warmup = vec![0.5f32; RNNOISE_FRAME_SIZE];
        backend.process(&mut warmup, RNNOISE_FRAME_SIZE, 1, false);

        // Second frame: should produce output from the first processed frame.
        let mut second = vec![0.5f32; RNNOISE_FRAME_SIZE];
        backend.process(&mut second, RNNOISE_FRAME_SIZE, 1, false);

        let energy: f32 = second.iter().map(|s| s * s).sum();
        assert!(
            energy > 0.0,
            "Second frame should have non-zero output after warm-up"
        );
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
        assert!(backend.output_write_pos > 0 || backend.first_frame_discarded);

        backend.reset();

        assert_eq!(backend.accum_fill, 0);
        assert_eq!(backend.output_write_pos, 0);
        assert_eq!(backend.output_read_pos, 0);
        assert!(!backend.first_frame_discarded);
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
}
