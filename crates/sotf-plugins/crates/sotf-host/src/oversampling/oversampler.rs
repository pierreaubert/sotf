use super::misc::MAX_OS_CHANNELS;
use super::misc::OS_CHUNK_SIZE;
use super::misc::interleaved_to_planar;
use super::misc::planar_to_interleaved;
use audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{Fft, FixedSync, Resampler};

/// Oversampling processor that handles up/downsampling with residual buffering.
///
/// Usage:
/// 1. Create with `Oversampler::new(factor, channels)`
/// 2. Call `process()` with interleaved audio + a callback that processes at
///    the oversampled rate
/// 3. Query `latency_samples()` for PDC
pub struct Oversampler {
    /// 1x -> Nx resampler (upsample)
    pub(super) resampler_up: Fft<f32>,
    /// Nx -> 1x resampler (downsample)
    pub(super) resampler_down: Fft<f32>,
    /// Planar input buffer for up-resampler (one Vec per channel, length = OS_CHUNK_SIZE)
    pub(super) up_in: Vec<Vec<f32>>,
    /// Planar output buffer for up-resampler (one Vec per channel, length = OS_CHUNK_SIZE * factor)
    pub(super) up_out: Vec<Vec<f32>>,
    /// Planar input buffer for down-resampler (one Vec per channel, length = OS_CHUNK_SIZE * factor)
    pub(super) down_in: Vec<Vec<f32>>,
    /// Planar output buffer for down-resampler (one Vec per channel, length = OS_CHUNK_SIZE)
    pub(super) down_out: Vec<Vec<f32>>,
    /// Residual input frames (interleaved) waiting to fill a full OS_CHUNK_SIZE chunk
    pub(super) residual_in: Vec<f32>,
    /// Read cursor into `residual_in`
    pub(super) residual_in_read: usize,
    /// Number of frames currently in `residual_in`
    pub(super) residual_frames: usize,
    /// Residual output frames (interleaved) waiting to be consumed by the caller
    pub(super) residual_out: Vec<f32>,
    /// Number of frames currently ready in `residual_out`
    pub(super) residual_out_frames: usize,
    /// Read cursor into `residual_out`
    pub(super) residual_out_read: usize,
    /// Reusable interleaved chunk buffer for full OS_CHUNK_SIZE input blocks.
    pub(super) chunk_buffer: Vec<f32>,
    /// Oversampling factor (2 or 4)
    pub(super) factor: u32,
    /// Number of audio channels
    pub(super) channels: usize,
    /// Total latency in samples (at 1x rate) from the resampler pair
    pub(super) latency: usize,
}

impl Oversampler {
    /// Create a new oversampler. `factor` must be 2 or 4. `channels` >= 1.
    pub fn new(factor: u32, channels: usize) -> Result<Self, String> {
        if factor != 2 && factor != 4 {
            return Err(format!(
                "Invalid oversampling factor {}: must be 2 or 4",
                factor
            ));
        }
        if channels == 0 {
            return Err("channels must be >= 1".to_string());
        }
        if channels > MAX_OS_CHANNELS {
            return Err(format!(
                "Oversampler supports at most {} channels, got {}",
                MAX_OS_CHANNELS, channels
            ));
        }

        let f = factor as usize;

        // Up-resampler: input sample_rate 1, output sample_rate factor
        // chunk_size = OS_CHUNK_SIZE (fixed input)
        let resampler_up = Fft::<f32>::new(1, f, OS_CHUNK_SIZE, 1, channels, FixedSync::Input)
            .map_err(|e| format!("Failed to create up-resampler: {:?}", e))?;

        // Down-resampler: input sample_rate factor, output sample_rate 1
        // chunk_size = OS_CHUNK_SIZE * factor (fixed input, produces OS_CHUNK_SIZE output)
        let resampler_down =
            Fft::<f32>::new(f, 1, OS_CHUNK_SIZE * f, 1, channels, FixedSync::Input)
                .map_err(|e| format!("Failed to create down-resampler: {:?}", e))?;

        let up_out_frames = resampler_up.output_frames_max();
        let down_out_frames = resampler_down.output_frames_max();

        // Latency: up-resampler delay (in output frames at Nx rate) converted to 1x frames,
        // plus down-resampler delay (already in 1x output frames).
        // Both delays are reported as output frames. We add them in 1x units.
        let up_delay_1x = resampler_up.output_delay() / f; // Nx -> 1x
        let down_delay_1x = resampler_down.output_delay();
        // Add one chunk of input buffering latency
        let latency = up_delay_1x + down_delay_1x + OS_CHUNK_SIZE;

        Ok(Self {
            resampler_up,
            resampler_down,
            up_in: vec![vec![0.0f32; OS_CHUNK_SIZE]; channels],
            up_out: vec![vec![0.0f32; up_out_frames]; channels],
            down_in: vec![vec![0.0f32; OS_CHUNK_SIZE * f]; channels],
            down_out: vec![vec![0.0f32; down_out_frames]; channels],
            // Residual I/O buffers pre-allocated for max expected frame size (4096)
            // to avoid hot-path resize. The resize guards remain as safety nets.
            residual_in: vec![0.0f32; (4096 + OS_CHUNK_SIZE) * channels],
            residual_in_read: 0,
            residual_frames: 0,
            residual_out: vec![0.0f32; (OS_CHUNK_SIZE + latency) * channels * 4],
            residual_out_frames: 0,
            residual_out_read: 0,
            chunk_buffer: vec![0.0f32; OS_CHUNK_SIZE * channels],
            factor,
            channels,
            latency,
        })
    }

    /// Reset all internal state (resamplers, residual buffers).
    pub fn reset(&mut self) {
        self.resampler_up.reset();
        self.resampler_down.reset();
        self.residual_in_read = 0;
        self.residual_frames = 0;
        self.residual_out_frames = 0;
        self.residual_out_read = 0;
        for ch_buf in &mut self.up_in {
            ch_buf.fill(0.0);
        }
        for ch_buf in &mut self.up_out {
            ch_buf.fill(0.0);
        }
        for ch_buf in &mut self.down_in {
            ch_buf.fill(0.0);
        }
        for ch_buf in &mut self.down_out {
            ch_buf.fill(0.0);
        }
    }

    /// Total latency in samples (at the original sample rate).
    pub fn latency_samples(&self) -> usize {
        self.latency
    }

    /// Oversampling factor (2 or 4).
    pub fn factor(&self) -> u32 {
        self.factor
    }

    /// Process interleaved audio through the oversampling pipeline.
    ///
    /// `buffer` contains interleaved audio `[ch0_f0, ch1_f0, ch0_f1, ch1_f1, ...]`.
    /// `num_frames` is the number of frames in the buffer.
    /// `process_fn` is called with `(planar_buffers, oversampled_frames)` to process
    /// the audio at the oversampled rate. The callback processes in-place on planar
    /// buffers.
    ///
    /// Returns the number of output frames written to `buffer`.
    pub fn process<F>(
        &mut self,
        buffer: &mut [f32],
        num_frames: usize,
        mut process_fn: F,
    ) -> Result<usize, String>
    where
        F: FnMut(&mut [Vec<f32>], usize),
    {
        let nc = self.channels;
        let total_in_samples = num_frames * nc;

        // 1. Append incoming frames to residual_in. The read cursor allows full
        // chunks to be consumed without shifting residual data every iteration.
        self.ensure_residual_in_capacity(num_frames);
        let write_start = (self.residual_in_read + self.residual_frames) * nc;
        self.residual_in[write_start..write_start + total_in_samples]
            .copy_from_slice(&buffer[..total_in_samples]);
        self.residual_frames += num_frames;

        if self.chunk_buffer.len() < OS_CHUNK_SIZE * nc {
            self.chunk_buffer.resize(OS_CHUNK_SIZE * nc, 0.0);
        }

        // 2. Process all full chunks from the residual input
        while self.residual_frames >= OS_CHUNK_SIZE {
            let chunk_len = OS_CHUNK_SIZE * nc;
            let chunk_start = self.residual_in_read * nc;
            self.chunk_buffer[..chunk_len]
                .copy_from_slice(&self.residual_in[chunk_start..chunk_start + chunk_len]);
            self.residual_in_read += OS_CHUNK_SIZE;
            self.residual_frames -= OS_CHUNK_SIZE;
            if self.residual_frames == 0 {
                self.residual_in_read = 0;
            }

            self.process_chunk(&mut process_fn)?;
        }

        // 3. Drain residual_out into buffer
        let mut frames_written = 0usize;
        while frames_written < num_frames {
            let frames_ready = self.residual_out_frames;
            let frames_needed = num_frames - frames_written;

            if frames_ready == 0 {
                // Not enough output ready (latency fill with zeros)
                let fill_start = frames_written * nc;
                buffer[fill_start..fill_start + frames_needed * nc].fill(0.0);
                break;
            }

            let frames_to_copy = frames_ready.min(frames_needed);
            let src_start = self.residual_out_read * nc;
            let dst_start = frames_written * nc;
            buffer[dst_start..dst_start + frames_to_copy * nc]
                .copy_from_slice(&self.residual_out[src_start..src_start + frames_to_copy * nc]);

            self.residual_out_read += frames_to_copy;
            self.residual_out_frames -= frames_to_copy;
            if self.residual_out_frames == 0 {
                self.residual_out_read = 0;
            }
            frames_written += frames_to_copy;
        }

        Ok(frames_written)
    }

    pub(super) fn ensure_residual_in_capacity(&mut self, additional_frames: usize) {
        let nc = self.channels;
        let needed_end = (self.residual_in_read + self.residual_frames + additional_frames) * nc;
        if needed_end <= self.residual_in.len() {
            return;
        }

        self.compact_residual_in();
        let needed = (self.residual_frames + additional_frames) * nc;
        if needed > self.residual_in.len() {
            self.residual_in.resize(needed + OS_CHUNK_SIZE * nc, 0.0);
        }
    }

    pub(super) fn compact_residual_in(&mut self) {
        if self.residual_in_read == 0 {
            return;
        }
        let nc = self.channels;
        let remaining = self.residual_frames * nc;
        if remaining > 0 {
            let src_start = self.residual_in_read * nc;
            self.residual_in
                .copy_within(src_start..src_start + remaining, 0);
        }
        self.residual_in_read = 0;
    }

    pub(super) fn ensure_residual_out_capacity(&mut self, additional_frames: usize) -> usize {
        let nc = self.channels;
        let mut write_frame = self.residual_out_read + self.residual_out_frames;
        let needed_end = (write_frame + additional_frames) * nc;
        if needed_end <= self.residual_out.len() {
            return write_frame;
        }

        self.compact_residual_out();
        write_frame = self.residual_out_frames;
        let needed = (write_frame + additional_frames) * nc;
        if needed > self.residual_out.len() {
            self.residual_out.resize(needed + OS_CHUNK_SIZE * nc, 0.0);
        }
        write_frame
    }

    pub(super) fn compact_residual_out(&mut self) {
        if self.residual_out_read == 0 {
            return;
        }
        let nc = self.channels;
        let remaining = self.residual_out_frames * nc;
        if remaining > 0 {
            let src_start = self.residual_out_read * nc;
            self.residual_out
                .copy_within(src_start..src_start + remaining, 0);
        }
        self.residual_out_read = 0;
    }

    /// Process one OS_CHUNK_SIZE chunk of interleaved input through
    /// upsample -> callback -> downsample.
    pub(super) fn process_chunk<F>(&mut self, process_fn: &mut F) -> Result<(), String>
    where
        F: FnMut(&mut [Vec<f32>], usize),
    {
        let nc = self.channels;
        let factor = self.factor as usize;

        // Step 1: interleaved -> planar into up_in
        interleaved_to_planar(
            &self.chunk_buffer[..OS_CHUNK_SIZE * nc],
            &mut self.up_in,
            OS_CHUNK_SIZE,
            nc,
        );

        // Step 2: upsample
        let up_out_max = self.resampler_up.output_frames_max();
        {
            let in_adapter =
                SequentialSliceOfVecs::new(&self.up_in, nc, OS_CHUNK_SIZE).map_err(|e| {
                    crate::rate_limited_log!(error, 5, "oversampling up_in adapter: {e:?}");
                    format!("up in adapter: {:?}", e)
                })?;
            let mut out_adapter = SequentialSliceOfVecs::new_mut(&mut self.up_out, nc, up_out_max)
                .map_err(|e| {
                    crate::rate_limited_log!(error, 5, "oversampling up_out adapter: {e:?}");
                    format!("up out adapter: {:?}", e)
                })?;
            self.resampler_up
                .process_into_buffer(&in_adapter, &mut out_adapter, None)
                .map_err(|e| {
                    crate::rate_limited_log!(error, 5, "oversampling upsample failed: {e:?}");
                    format!("upsample: {:?}", e)
                })?;
        }

        // The upsampled frame count is OS_CHUNK_SIZE * factor
        let up_frames = OS_CHUNK_SIZE * factor;

        // Step 3: call the process callback on upsampled data
        process_fn(&mut self.up_out, up_frames);

        // Step 4: copy upsampled data to down_in (they are different buffers)
        for ch in 0..nc {
            self.down_in[ch][..up_frames].copy_from_slice(&self.up_out[ch][..up_frames]);
        }

        // Step 5: downsample
        let down_out_max = self.resampler_down.output_frames_max();
        let down_frames = {
            let in_adapter = SequentialSliceOfVecs::new(&self.down_in, nc, OS_CHUNK_SIZE * factor)
                .map_err(|e| {
                    crate::rate_limited_log!(error, 5, "oversampling down_in adapter: {e:?}");
                    format!("down in adapter: {:?}", e)
                })?;
            let mut out_adapter =
                SequentialSliceOfVecs::new_mut(&mut self.down_out, nc, down_out_max).map_err(
                    |e| {
                        crate::rate_limited_log!(error, 5, "oversampling down_out adapter: {e:?}");
                        format!("down out adapter: {:?}", e)
                    },
                )?;
            let (_, out_frames) = self
                .resampler_down
                .process_into_buffer(&in_adapter, &mut out_adapter, None)
                .map_err(|e| {
                    crate::rate_limited_log!(error, 5, "oversampling downsample failed: {e:?}");
                    format!("downsample: {:?}", e)
                })?;
            out_frames
        };

        // Step 6: planar -> interleaved into residual_out
        let write_frame = self.ensure_residual_out_capacity(down_frames);
        let write_offset = write_frame * nc;
        planar_to_interleaved(
            &self.down_out,
            &mut self.residual_out[write_offset..],
            down_frames,
            nc,
        );
        self.residual_out_frames += down_frames;

        Ok(())
    }
}
