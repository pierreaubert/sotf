// ============================================================================
// Oversampling — shared infrastructure for up/downsampling audio
// ============================================================================
//
// Provides a generic `Oversampler` that handles residual buffering, planar
// conversion, and rubato-based resampling. Plugins supply a callback that
// processes audio at the oversampled rate.

use audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{Fft, FixedSync, Resampler};

/// Fixed chunk size for oversampling. Chosen to balance latency (~5ms @ 48kHz)
/// and efficiency.
pub const OS_CHUNK_SIZE: usize = 256;

/// Maximum number of channels supported in the stack-based chunk buffer.
/// 32 channels covers up to 9.1.6 with headroom.
const MAX_OS_CHANNELS: usize = 32;

/// Oversampling processor that handles up/downsampling with residual buffering.
///
/// Usage:
/// 1. Create with `Oversampler::new(factor, channels)`
/// 2. Call `process()` with interleaved audio + a callback that processes at
///    the oversampled rate
/// 3. Query `latency_samples()` for PDC
pub struct Oversampler {
    /// 1x -> Nx resampler (upsample)
    resampler_up: Fft<f32>,
    /// Nx -> 1x resampler (downsample)
    resampler_down: Fft<f32>,
    /// Planar input buffer for up-resampler (one Vec per channel, length = OS_CHUNK_SIZE)
    up_in: Vec<Vec<f32>>,
    /// Planar output buffer for up-resampler (one Vec per channel, length = OS_CHUNK_SIZE * factor)
    up_out: Vec<Vec<f32>>,
    /// Planar input buffer for down-resampler (one Vec per channel, length = OS_CHUNK_SIZE * factor)
    down_in: Vec<Vec<f32>>,
    /// Planar output buffer for down-resampler (one Vec per channel, length = OS_CHUNK_SIZE)
    down_out: Vec<Vec<f32>>,
    /// Residual input frames (interleaved) waiting to fill a full OS_CHUNK_SIZE chunk
    residual_in: Vec<f32>,
    /// Number of frames currently in `residual_in`
    residual_frames: usize,
    /// Residual output frames (interleaved) waiting to be consumed by the caller
    residual_out: Vec<f32>,
    /// Number of frames currently ready in `residual_out`
    residual_out_frames: usize,
    /// Read cursor into `residual_out`
    residual_out_read: usize,
    /// Oversampling factor (2 or 4)
    factor: u32,
    /// Number of audio channels
    channels: usize,
    /// Total latency in samples (at 1x rate) from the resampler pair
    latency: usize,
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
        let resampler_up =
            Fft::<f32>::new(1, f, OS_CHUNK_SIZE, 1, channels, FixedSync::Input)
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
            residual_frames: 0,
            residual_out: vec![0.0f32; (OS_CHUNK_SIZE + latency) * channels * 4],
            residual_out_frames: 0,
            residual_out_read: 0,
            factor,
            channels,
            latency,
        })
    }

    /// Reset all internal state (resamplers, residual buffers).
    pub fn reset(&mut self) {
        self.resampler_up.reset();
        self.resampler_down.reset();
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

        // 1. Grow residual_in if needed
        {
            let needed = (self.residual_frames + num_frames) * nc;
            if needed > self.residual_in.len() {
                self.residual_in.resize(needed + OS_CHUNK_SIZE * nc, 0.0);
            }
            // Append incoming frames to residual_in
            let write_start = self.residual_frames * nc;
            self.residual_in[write_start..write_start + total_in_samples]
                .copy_from_slice(&buffer[..total_in_samples]);
            self.residual_frames += num_frames;
        }

        // 2. Process all full chunks from the residual input
        while self.residual_frames >= OS_CHUNK_SIZE {
            let chunk_len = OS_CHUNK_SIZE * nc;
            let mut chunk_buf = [0.0f32; OS_CHUNK_SIZE * MAX_OS_CHANNELS];
            chunk_buf[..chunk_len].copy_from_slice(&self.residual_in[..chunk_len]);

            // Shift residual_in left by OS_CHUNK_SIZE frames
            let remaining = (self.residual_frames - OS_CHUNK_SIZE) * nc;
            self.residual_in.copy_within(chunk_len..chunk_len + remaining, 0);
            self.residual_frames -= OS_CHUNK_SIZE;

            // Process the chunk
            self.process_chunk(&chunk_buf[..chunk_len], &mut process_fn)?;
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

            // Compact the residual_out buffer if it was fully consumed
            if frames_to_copy == frames_ready {
                self.residual_out_read = 0;
                self.residual_out_frames = 0;
            } else {
                self.residual_out_read += frames_to_copy;
                self.residual_out_frames -= frames_to_copy;
            }
            frames_written += frames_to_copy;
        }

        Ok(frames_written)
    }

    /// Process one OS_CHUNK_SIZE chunk of interleaved input through
    /// upsample -> callback -> downsample.
    fn process_chunk<F>(
        &mut self,
        input_chunk: &[f32],
        process_fn: &mut F,
    ) -> Result<(), String>
    where
        F: FnMut(&mut [Vec<f32>], usize),
    {
        let nc = self.channels;
        let factor = self.factor as usize;

        // Step 1: interleaved -> planar into up_in
        interleaved_to_planar(input_chunk, &mut self.up_in, OS_CHUNK_SIZE, nc);

        // Step 2: upsample
        let up_out_max = self.resampler_up.output_frames_max();
        {
            let in_adapter = SequentialSliceOfVecs::new(&self.up_in, nc, OS_CHUNK_SIZE)
                .map_err(|e| format!("up in adapter: {:?}", e))?;
            let mut out_adapter =
                SequentialSliceOfVecs::new_mut(&mut self.up_out, nc, up_out_max)
                    .map_err(|e| format!("up out adapter: {:?}", e))?;
            self.resampler_up
                .process_into_buffer(&in_adapter, &mut out_adapter, None)
                .map_err(|e| format!("upsample: {:?}", e))?;
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
            let in_adapter =
                SequentialSliceOfVecs::new(&self.down_in, nc, OS_CHUNK_SIZE * factor)
                    .map_err(|e| format!("down in adapter: {:?}", e))?;
            let mut out_adapter =
                SequentialSliceOfVecs::new_mut(&mut self.down_out, nc, down_out_max)
                    .map_err(|e| format!("down out adapter: {:?}", e))?;
            let (_, out_frames) = self
                .resampler_down
                .process_into_buffer(&in_adapter, &mut out_adapter, None)
                .map_err(|e| format!("downsample: {:?}", e))?;
            out_frames
        };

        // Step 6: planar -> interleaved into residual_out
        let write_offset = self.residual_out_frames * nc;
        let needed = write_offset + down_frames * nc;
        if needed > self.residual_out.len() {
            self.residual_out.resize(needed + OS_CHUNK_SIZE * nc, 0.0);
        }
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

/// Convert interleaved audio to planar format.
///
/// `interleaved` is `[ch0_f0, ch1_f0, ch0_f1, ch1_f1, ...]`.
/// `planar[ch][frame]` is the output.
pub fn interleaved_to_planar(
    interleaved: &[f32],
    planar: &mut [Vec<f32>],
    num_frames: usize,
    num_channels: usize,
) {
    for ch in 0..num_channels {
        for frame in 0..num_frames {
            planar[ch][frame] = interleaved[frame * num_channels + ch];
        }
    }
}

/// Convert planar audio to interleaved format.
///
/// `planar[ch][frame]` is the input.
/// `interleaved` is `[ch0_f0, ch1_f0, ch0_f1, ch1_f1, ...]`.
pub fn planar_to_interleaved(
    planar: &[Vec<f32>],
    interleaved: &mut [f32],
    num_frames: usize,
    num_channels: usize,
) {
    for frame in 0..num_frames {
        for ch in 0..num_channels {
            interleaved[frame * num_channels + ch] = planar[ch][frame];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oversampler_2x_passthrough() {
        let channels = 2;
        let mut os = Oversampler::new(2, channels).unwrap();

        // Process silence through a passthrough callback
        let num_frames = 512;
        let mut buffer = vec![0.0f32; num_frames * channels];

        // Process several blocks to fill the pipeline
        for _ in 0..10 {
            os.process(&mut buffer, num_frames, |_planar, _frames| {
                // passthrough: do nothing
            })
            .unwrap();
        }

        // Output should be silence (within float tolerance)
        for (i, &s) in buffer.iter().enumerate() {
            assert!(
                s.abs() < 1e-6,
                "2x passthrough sample {} not silent: {}",
                i,
                s
            );
        }
    }

    #[test]
    fn test_oversampler_4x_passthrough() {
        let channels = 2;
        let mut os = Oversampler::new(4, channels).unwrap();

        let num_frames = 512;
        let mut buffer = vec![0.0f32; num_frames * channels];

        for _ in 0..10 {
            os.process(&mut buffer, num_frames, |_planar, _frames| {
                // passthrough: do nothing
            })
            .unwrap();
        }

        for (i, &s) in buffer.iter().enumerate() {
            assert!(
                s.abs() < 1e-6,
                "4x passthrough sample {} not silent: {}",
                i,
                s
            );
        }
    }

    #[test]
    fn test_oversampler_latency() {
        let os_2x = Oversampler::new(2, 2).unwrap();
        assert!(
            os_2x.latency_samples() > 0,
            "2x oversampler should have nonzero latency"
        );
        // Latency should be reasonable: at least OS_CHUNK_SIZE and less than
        // several thousand samples
        assert!(os_2x.latency_samples() >= OS_CHUNK_SIZE);
        assert!(os_2x.latency_samples() < 4096);

        let os_4x = Oversampler::new(4, 2).unwrap();
        assert!(
            os_4x.latency_samples() > 0,
            "4x oversampler should have nonzero latency"
        );
        assert!(os_4x.latency_samples() >= OS_CHUNK_SIZE);
        assert!(os_4x.latency_samples() < 4096);
    }

    #[test]
    fn test_oversampler_preserves_signal() {
        // Process a known sine wave through a passthrough callback and verify
        // the output has the same frequency content. After the pipeline fills,
        // a passthrough should reproduce the input with only resampler delay.
        let channels = 1;
        let mut os = Oversampler::new(2, channels).unwrap();

        let num_frames = 512;
        let freq = 1000.0f32;
        let sample_rate = 48000.0f32;

        // Warm up the pipeline with the sine
        for block in 0..20 {
            let mut buffer: Vec<f32> = (0..num_frames)
                .map(|i| {
                    let t = (block * num_frames + i) as f32 / sample_rate;
                    (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5
                })
                .collect();

            os.process(&mut buffer, num_frames, |_planar, _frames| {
                // passthrough
            })
            .unwrap();
        }

        // Now capture one more block
        let block = 20;
        let mut output: Vec<f32> = (0..num_frames)
            .map(|i| {
                let t = (block * num_frames + i) as f32 / sample_rate;
                (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5
            })
            .collect();

        os.process(&mut output, num_frames, |_planar, _frames| {
            // passthrough
        })
        .unwrap();

        // The output should be a sine wave with similar amplitude (within
        // resampler attenuation tolerance). Check that peak is > 0.3
        // (input peak is 0.5, some attenuation from the anti-aliasing filter
        // is expected).
        let peak = output.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            peak > 0.3,
            "Output peak {} is too low, signal was not preserved",
            peak
        );

        // All samples should be finite
        for (i, &s) in output.iter().enumerate() {
            assert!(s.is_finite(), "sample {} not finite: {}", i, s);
        }
    }

    #[test]
    fn test_oversampler_reset() {
        let channels = 2;
        let mut os = Oversampler::new(2, channels).unwrap();

        // Process some audio
        let num_frames = 512;
        let mut buffer = vec![0.5f32; num_frames * channels];
        os.process(&mut buffer, num_frames, |_planar, _frames| {})
            .unwrap();

        // Reset should clear all residual state
        os.reset();
        assert_eq!(os.residual_frames, 0);
        assert_eq!(os.residual_out_frames, 0);
        assert_eq!(os.residual_out_read, 0);
    }

    #[test]
    fn test_oversampler_invalid_factor() {
        assert!(Oversampler::new(1, 2).is_err());
        assert!(Oversampler::new(3, 2).is_err());
        assert!(Oversampler::new(0, 2).is_err());
        assert!(Oversampler::new(8, 2).is_err());
    }

    #[test]
    fn test_oversampler_invalid_channels() {
        assert!(Oversampler::new(2, 0).is_err());
        assert!(Oversampler::new(2, 33).is_err());
    }

    #[test]
    fn test_interleaved_to_planar_roundtrip() {
        let channels = 3;
        let frames = 4;
        let interleaved: Vec<f32> = (0..channels * frames).map(|i| i as f32).collect();

        let mut planar = vec![vec![0.0f32; frames]; channels];
        interleaved_to_planar(&interleaved, &mut planar, frames, channels);

        // Verify: planar[ch][frame] == interleaved[frame * channels + ch]
        for ch in 0..channels {
            for frame in 0..frames {
                assert_eq!(planar[ch][frame], interleaved[frame * channels + ch]);
            }
        }

        // Roundtrip back
        let mut result = vec![0.0f32; channels * frames];
        planar_to_interleaved(&planar, &mut result, frames, channels);
        assert_eq!(result, interleaved);
    }
}
