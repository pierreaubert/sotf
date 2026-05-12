pub const RNNOISE_FRAME_SIZE: usize = 480;

/// Monitoring data exposed by the RNNoise speech denoiser.
#[derive(Debug, Clone, Copy, Default)]
pub struct RnnoiseData {
    pub avg_reduction_db: f32,
}

/// RNNoise speech-denoising backend.
pub struct RnnoiseBackend {
    denoisers: Vec<Box<nnnoiseless::DenoiseState>>,
    denoiser_pool: Vec<Box<nnnoiseless::DenoiseState>>,
    channels: usize,
    sample_rate: u32,
    accum_buffers: Vec<Vec<f32>>,
    output_buffers: Vec<Vec<f32>>,
    output_write_pos: usize,
    output_read_pos: usize,
    accum_fill: usize,
    avg_reduction_db: f32,
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
            denoiser_pool: Vec::new(),
            channels: 0,
            sample_rate: 48000,
            accum_buffers: Vec::new(),
            output_buffers: Vec::new(),
            output_write_pos: 0,
            output_read_pos: 0,
            accum_fill: 0,
            avg_reduction_db: 0.0,
            first_frame_discarded: false,
        }
    }

    pub fn initialize(&mut self, sample_rate: u32, channels: usize) -> Result<(), String> {
        if sample_rate != 48000 {
            return Err("RNNoise only supports 48 kHz sample rate".into());
        }
        self.sample_rate = sample_rate;
        self.channels = channels;
        self.denoisers = (0..channels)
            .map(|_| nnnoiseless::DenoiseState::new())
            .collect::<Vec<_>>();
        self.denoiser_pool = (0..channels)
            .map(|_| nnnoiseless::DenoiseState::new())
            .collect::<Vec<_>>();
        self.accum_buffers = vec![vec![0.0; RNNOISE_FRAME_SIZE]; channels];
        let ring_size = RNNOISE_FRAME_SIZE * 4;
        self.output_buffers = vec![vec![0.0; ring_size]; channels];
        self.output_write_pos = 0;
        self.output_read_pos = 0;
        self.accum_fill = 0;
        self.avg_reduction_db = 0.0;
        self.first_frame_discarded = false;
        Ok(())
    }

    pub fn max_in_place_frames(&self) -> usize {
        self.output_buffers
            .first()
            .map(|buffer| buffer.len())
            .unwrap_or(RNNOISE_FRAME_SIZE * 4)
    }

    pub fn process(
        &mut self,
        buffer: &mut [f32],
        num_frames: usize,
        channels: usize,
        bypass: bool,
    ) -> usize {
        let ch_count = channels.min(self.channels);
        if self.denoisers.is_empty() || ch_count == 0 {
            return num_frames;
        }

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
                } else if self.channels == 2 && ch_count == 2 {
                    // Stereo: downmix to mid, process with single denoiser, apply delta to both
                    let mut mid = [0.0f32; RNNOISE_FRAME_SIZE];
                    for i in 0..RNNOISE_FRAME_SIZE {
                        mid[i] = (self.accum_buffers[0][i] + self.accum_buffers[1][i]) * 0.5;
                    }

                    let mut mid_scaled = mid;
                    for s in &mut mid_scaled {
                        *s *= 32767.0;
                    }

                    let mut mid_denoised = [0.0f32; RNNOISE_FRAME_SIZE];
                    self.denoisers[0].process_frame(&mut mid_denoised, &mid_scaled);

                    for s in &mut mid_denoised {
                        *s /= 32767.0;
                    }

                    let ring_size = self.output_buffers[0].len();
                    for i in 0..RNNOISE_FRAME_SIZE {
                        let delta = mid_denoised[i] - mid[i];
                        let l_out = self.accum_buffers[0][i] + delta;
                        let r_out = self.accum_buffers[1][i] + delta;
                        self.output_buffers[0][(self.output_write_pos + i) % ring_size] = l_out;
                        self.output_buffers[1][(self.output_write_pos + i) % ring_size] = r_out;
                    }

                    // Reduction metering on mid channel
                    let output_power =
                        mid_denoised.iter().map(|x| x * x).sum::<f32>() / RNNOISE_FRAME_SIZE as f32;
                    let input_power =
                        mid.iter().map(|x| x * x).sum::<f32>() / RNNOISE_FRAME_SIZE as f32;
                    if input_power > 1e-10 {
                        self.avg_reduction_db = 0.9 * self.avg_reduction_db
                            + 0.1 * 10.0 * (input_power / output_power.max(1e-10)).log10();
                    }
                } else {
                    let mut ch0_output_power = 0.0f32;
                    for ch in 0..ch_count {
                        let mut input_buf = [0.0f32; RNNOISE_FRAME_SIZE];
                        let mut output_buf = [0.0f32; RNNOISE_FRAME_SIZE];
                        input_buf.copy_from_slice(&self.accum_buffers[ch]);

                        for s in &mut input_buf {
                            *s *= 32767.0;
                        }

                        self.denoisers[ch].process_frame(&mut output_buf, &input_buf);

                        for s in &mut output_buf {
                            *s /= 32767.0;
                        }

                        if ch == 0 {
                            ch0_output_power = output_buf.iter().map(|x| x * x).sum::<f32>()
                                / RNNOISE_FRAME_SIZE as f32;
                        }

                        let ring_size = self.output_buffers[ch].len();
                        for (i, &s) in output_buf.iter().enumerate() {
                            self.output_buffers[ch][(self.output_write_pos + i) % ring_size] = s;
                        }
                    }

                    let input_power = self.accum_buffers[0].iter().map(|x| x * x).sum::<f32>()
                        / RNNOISE_FRAME_SIZE as f32;
                    if input_power > 1e-10 {
                        self.avg_reduction_db = 0.9 * self.avg_reduction_db
                            + 0.1 * 10.0 * (input_power / ch0_output_power.max(1e-10)).log10();
                    }
                }

                // Discard the first processed frame to avoid fade-in artifacts.
                // Do this in both denoising and bypass modes so latency stays constant.
                if !self.first_frame_discarded {
                    self.first_frame_discarded = true;
                    self.output_read_pos += RNNOISE_FRAME_SIZE;
                }

                self.output_write_pos += RNNOISE_FRAME_SIZE;
                self.accum_fill = 0;
            }
        }

        let ring_size = self.output_buffers[0].len();
        let available = self.output_write_pos.saturating_sub(self.output_read_pos);
        let to_write = num_frames.min(available);

        if ch_count > 0 {
            for frame in 0..to_write {
                for ch in 0..ch_count {
                    buffer[frame * channels + ch] =
                        self.output_buffers[ch][(self.output_read_pos + frame) % ring_size];
                }
            }
            self.output_read_pos += to_write;
        }

        // Wrap pointers to prevent overflow on long-running sessions.
        if self.output_write_pos >= ring_size * 2 {
            let delta = self.output_write_pos - self.output_read_pos;
            self.output_write_pos = delta;
            self.output_read_pos = 0;
        }

        to_write
    }

    pub fn reset(&mut self) {
        // Swap with pre-allocated pool to avoid heap allocation in the audio thread.
        for (denoiser, pool) in self.denoisers.iter_mut().zip(self.denoiser_pool.iter_mut()) {
            std::mem::swap(denoiser, pool);
        }
        for buf in &mut self.accum_buffers {
            buf.fill(0.0);
        }
        for buf in &mut self.output_buffers {
            buf.fill(0.0);
        }
        self.output_write_pos = 0;
        self.output_read_pos = 0;
        self.accum_fill = 0;
        self.avg_reduction_db = 0.0;
        self.first_frame_discarded = false;
    }

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
    fn rejects_non_48khz() {
        let mut backend = RnnoiseBackend::new();
        let result = backend.initialize(44100, 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("48 kHz"));
    }

    #[test]
    fn silence_stays_near_silent() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1).unwrap();

        // First 480 frames are discarded; process two frames.
        let mut buffer = vec![0.0f32; 960];
        let written = backend.process(&mut buffer, 960, 1, false);
        // After first-frame discard, only 480 samples are available.
        assert_eq!(written, 480);

        for (i, &sample) in buffer[..written].iter().enumerate() {
            assert!(
                sample.abs() < 0.01,
                "Sample {i} should be near zero, got {sample}"
            );
        }
    }

    #[test]
    fn first_frame_is_discarded() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1).unwrap();

        // First frame: silence. Second frame: 440 Hz sine wave.
        let mut buffer = vec![0.0f32; 960];
        for i in 480..960 {
            buffer[i] = ((i - 480) as f32 * 440.0 * 2.0 * std::f32::consts::PI / 48000.0).sin();
        }

        let written = backend.process(&mut buffer, 960, 1, false);
        // First frame discarded, so only second frame is output.
        assert_eq!(written, 480);

        // Compute RMS of the output; it should be clearly above silence
        // because the second frame contained a sine wave.
        let rms = (buffer[..written].iter().map(|x| x * x).sum::<f32>() / written as f32).sqrt();
        assert!(
            rms > 0.01,
            "Output should contain the sine wave from the second frame, got rms={}",
            rms
        );
    }

    #[test]
    fn first_frame_is_discarded_in_bypass() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1).unwrap();

        // First frame: 0.5. Second frame: -0.5.
        let mut buffer = vec![0.0f32; 960];
        for i in 0..480 {
            buffer[i] = 0.5;
        }
        for i in 480..960 {
            buffer[i] = -0.5;
        }

        let written = backend.process(&mut buffer, 960, 1, true);
        // First frame discarded even in bypass to keep latency constant.
        assert_eq!(written, 480);

        // In bypass the second frame passes through unchanged.
        assert!(
            buffer[..written].iter().all(|&x| x < 0.0),
            "Output should be the bypassed second frame (-0.5)"
        );
    }

    #[test]
    fn stereo_preserves_image() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 2).unwrap();

        // First 480-frame block (discarded).
        let mut buffer1 = vec![0.0f32; 960];
        for i in 0..480 {
            let sample = (i as f32 * 440.0 * 2.0 * std::f32::consts::PI / 48000.0).sin();
            buffer1[i * 2] = sample;
            buffer1[i * 2 + 1] = sample;
        }
        backend.process(&mut buffer1, 480, 2, false);

        // Second 480-frame block (output).
        let mut buffer2 = vec![0.0f32; 960];
        for i in 0..480 {
            let sample = (i as f32 * 440.0 * 2.0 * std::f32::consts::PI / 48000.0).sin();
            buffer2[i * 2] = sample;
            buffer2[i * 2 + 1] = sample;
        }
        let written = backend.process(&mut buffer2, 480, 2, false);
        assert_eq!(written, 480);

        // L and R should remain identical (perfect stereo image preservation).
        for i in 0..written {
            let l = buffer2[i * 2];
            let r = buffer2[i * 2 + 1];
            assert!(
                (l - r).abs() < 0.001,
                "Stereo image should be preserved at sample {}, got L={} R={}",
                i,
                l,
                r
            );
        }
    }

    #[test]
    fn reset_does_not_allocate() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 2).unwrap();
        // reset() swaps with pre-allocated pool; no new allocations.
        backend.reset();
        assert_eq!(backend.denoisers.len(), 2);
        assert_eq!(backend.denoiser_pool.len(), 2);
    }
}
