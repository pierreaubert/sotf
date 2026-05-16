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

/// Maximum number of channels supported by the oversampler.
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
    /// Read cursor into `residual_in`
    residual_in_read: usize,
    /// Number of frames currently in `residual_in`
    residual_frames: usize,
    /// Residual output frames (interleaved) waiting to be consumed by the caller
    residual_out: Vec<f32>,
    /// Number of frames currently ready in `residual_out`
    residual_out_frames: usize,
    /// Read cursor into `residual_out`
    residual_out_read: usize,
    /// Reusable interleaved chunk buffer for full OS_CHUNK_SIZE input blocks.
    chunk_buffer: Vec<f32>,
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

    fn ensure_residual_in_capacity(&mut self, additional_frames: usize) {
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

    fn compact_residual_in(&mut self) {
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

    fn ensure_residual_out_capacity(&mut self, additional_frames: usize) -> usize {
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

    fn compact_residual_out(&mut self) {
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
    fn process_chunk<F>(&mut self, process_fn: &mut F) -> Result<(), String>
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
            let in_adapter = SequentialSliceOfVecs::new(&self.up_in, nc, OS_CHUNK_SIZE)
                .map_err(|e| {
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
                SequentialSliceOfVecs::new_mut(&mut self.down_out, nc, down_out_max)
                    .map_err(|e| {
                        crate::rate_limited_log!(
                            error,
                            5,
                            "oversampling down_out adapter: {e:?}"
                        );
                        format!("down out adapter: {:?}", e)
                    })?;
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

// ============================================================================
// OversampledPlugin — Generic wrapper that oversamples any InPlacePlugin
// ============================================================================

use crate::parameters::{Parameter, ParameterId, ParameterValue};
use crate::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use std::any::Any;
use std::sync::Arc;

/// Wraps any `InPlacePlugin` with transparent oversampling.
///
/// The inner plugin processes audio at `factor × sample_rate`. The wrapper
/// handles upsampling before and downsampling after `process_in_place()`.
///
/// This enables any plugin to be oversampled without modifying its internals:
/// ```ignore
/// let saturator = SaturationPlugin::new(2);
/// let oversampled = OversampledPlugin::new(saturator, 4, 2)?; // 4x, stereo
/// ```
pub struct OversampledPlugin<P: InPlacePlugin> {
    inner: P,
    oversampler: Oversampler,
    factor: u32,
    channels: usize,
    sample_rate: u32,
    /// Pre-allocated interleaved buffer for oversampled processing
    os_interleaved: Vec<f32>,
}

impl<P: InPlacePlugin> OversampledPlugin<P> {
    /// Create a new oversampled plugin wrapper.
    ///
    /// `factor` must be 2 or 4. The inner plugin will be initialized at
    /// `sample_rate * factor` when `initialize()` is called.
    pub fn new(inner: P, factor: u32, channels: usize) -> Result<Self, String> {
        let oversampler = Oversampler::new(factor, channels)?;
        // Pre-allocate for max expected oversampled block: 8192 * factor * channels
        let os_buf_size = 8192 * factor as usize * channels;
        Ok(Self {
            inner,
            oversampler,
            factor,
            channels,
            sample_rate: 48000,
            os_interleaved: vec![0.0; os_buf_size],
        })
    }

    /// Access the inner plugin.
    pub fn inner(&self) -> &P {
        &self.inner
    }

    /// Mutably access the inner plugin.
    pub fn inner_mut(&mut self) -> &mut P {
        &mut self.inner
    }
}

impl<P: InPlacePlugin> InPlacePlugin for OversampledPlugin<P> {
    fn info(&self) -> PluginInfo {
        let mut info = self.inner.info();
        info.name = format!("{}({}x)", info.name, self.factor);
        info
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.inner.parameters()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.inner.set_parameter(id, value)
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        self.inner.get_parameter(id)
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        // Initialize inner plugin at the oversampled rate
        let os_rate = sample_rate * self.factor;
        self.inner.initialize(os_rate)?;
        self.oversampler.reset();
        Ok(())
    }

    fn reset(&mut self) {
        self.inner.reset();
        self.oversampler.reset();
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        let nf = context.num_frames;
        let nc = self.channels;

        // The oversampler's callback receives planar buffers at the OS rate.
        // We need to convert to interleaved, call inner.process_in_place, then
        // convert back to planar.
        let os_context = ProcessContext {
            sample_rate: context.sample_rate * self.factor,
            num_frames: 0, // will be set per-chunk inside callback
        };

        let inner = &mut self.inner;
        let os_interleaved = &mut self.os_interleaved;
        let mut inner_error: Option<String> = None;

        self.oversampler
            .process(buffer, nf, |planar, os_frames| {
                if inner_error.is_some() {
                    return;
                }
                let total_os = os_frames * nc;
                // Ensure buffer is large enough. A grow here means the pre-
                // allocation in `build()` was too small for this block; log so
                // the offending block size is visible. Allocation on the audio
                // thread is acceptable as a one-shot fallback but not as a
                // steady state.
                if os_interleaved.capacity() < total_os {
                    crate::rate_limited_log!(
                        warn,
                        5,
                        "oversampling: os_interleaved grew from {} to {} on hot path",
                        os_interleaved.capacity(),
                        total_os
                    );
                }
                if os_interleaved.len() < total_os {
                    os_interleaved.resize(total_os, 0.0);
                }
                // Convert planar → interleaved
                planar_to_interleaved(planar, &mut os_interleaved[..total_os], os_frames, nc);
                // Process at oversampled rate
                let ctx = ProcessContext {
                    sample_rate: os_context.sample_rate,
                    num_frames: os_frames,
                };
                match inner.process_in_place(&mut os_interleaved[..total_os], &ctx) {
                    Ok(frames) if frames == os_frames => {}
                    Ok(frames) => {
                        inner_error = Some(format!(
                            "oversampled inner processed {frames} frames, expected {os_frames}"
                        ));
                        return;
                    }
                    Err(err) => {
                        inner_error = Some(err);
                        return;
                    }
                }
                // Convert interleaved → planar (back)
                interleaved_to_planar(&os_interleaved[..total_os], planar, os_frames, nc);
            })
            .map_err(|e| e.to_string())?;

        if let Some(err) = inner_error {
            return Err(err);
        }

        Ok(nf)
    }

    fn latency_samples(&self) -> usize {
        // Oversampler latency + inner plugin's latency (scaled to 1x rate)
        let inner_latency_1x = self.inner.latency_samples() / self.factor as usize;
        self.oversampler.latency_samples() + inner_latency_1x
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        self.inner.get_data()
    }

    fn preferred_oversampling(&self) -> Option<u32> {
        None // Already oversampled — don't request more
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
    fn test_oversampler_variable_small_blocks_keep_residual_cursors_valid() {
        let channels = 2;
        let mut os = Oversampler::new(2, channels).unwrap();
        let block_sizes = [17usize, 64, 191, 3, 512, 29, 257, 128];

        for (block_idx, &num_frames) in block_sizes.iter().cycle().take(32).enumerate() {
            let mut buffer: Vec<f32> = (0..num_frames * channels)
                .map(|i| ((block_idx * 31 + i) as f32 * 0.01).sin() * 0.25)
                .collect();

            let processed = os
                .process(&mut buffer, num_frames, |_planar, _frames| {})
                .unwrap();

            assert!(processed <= num_frames);
            assert!(buffer.iter().all(|s| s.is_finite()));
            assert!(os.residual_in_read + os.residual_frames <= os.residual_in.len() / channels);
            assert!(
                os.residual_out_read + os.residual_out_frames <= os.residual_out.len() / channels
            );
        }
    }

    #[test]
    fn test_oversampled_plugin_latency() {
        use crate::parameters::{Parameter, ParameterId, ParameterValue};
        use crate::plugin::{InPlacePlugin, PluginInfo, ProcessContext};

        /// Trivial passthrough plugin for testing
        struct PassthroughPlugin;
        impl InPlacePlugin for PassthroughPlugin {
            fn info(&self) -> PluginInfo {
                PluginInfo::new("Test", "1.0", "Test")
            }
            fn channels(&self) -> usize {
                2
            }
            fn parameters(&self) -> Vec<Parameter> {
                vec![]
            }
            fn set_parameter(
                &mut self,
                _: ParameterId,
                _: ParameterValue,
            ) -> crate::plugin::PluginResult<()> {
                Ok(())
            }
            fn get_parameter(&self, _: &ParameterId) -> Option<ParameterValue> {
                None
            }
            fn process_in_place(
                &mut self,
                _buffer: &mut [f32],
                context: &ProcessContext,
            ) -> crate::plugin::PluginResult<usize> {
                Ok(context.num_frames)
            }
        }

        let os = OversampledPlugin::new(PassthroughPlugin, 2, 2).unwrap();
        // Should have non-zero latency from the oversampler
        assert!(
            os.latency_samples() > 0,
            "Oversampled plugin should have latency"
        );
    }

    #[test]
    fn test_oversampled_plugin_processes_audio() {
        use crate::parameters::{Parameter, ParameterId, ParameterValue};
        use crate::plugin::{InPlacePlugin, PluginInfo, ProcessContext};

        /// Plugin that doubles all samples (to verify processing happens at OS rate)
        struct DoublerPlugin;
        impl InPlacePlugin for DoublerPlugin {
            fn info(&self) -> PluginInfo {
                PluginInfo::new("Doubler", "1.0", "Test")
            }
            fn channels(&self) -> usize {
                1
            }
            fn parameters(&self) -> Vec<Parameter> {
                vec![]
            }
            fn set_parameter(
                &mut self,
                _: ParameterId,
                _: ParameterValue,
            ) -> crate::plugin::PluginResult<()> {
                Ok(())
            }
            fn get_parameter(&self, _: &ParameterId) -> Option<ParameterValue> {
                None
            }
            fn process_in_place(
                &mut self,
                buffer: &mut [f32],
                context: &ProcessContext,
            ) -> crate::plugin::PluginResult<usize> {
                for s in buffer[..context.num_frames].iter_mut() {
                    *s *= 2.0;
                }
                Ok(context.num_frames)
            }
        }

        let mut os = OversampledPlugin::new(DoublerPlugin, 2, 1).unwrap();
        os.initialize(48000).unwrap();

        // Feed a few blocks to prime the oversampler pipeline
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: 256,
        };
        let mut buf = vec![0.5f32; 256];
        os.process_in_place(&mut buf, &ctx).unwrap();

        // After pipeline is primed, output should be ~doubled (accounting for resampler delay)
        let mut buf2 = vec![0.5f32; 256];
        os.process_in_place(&mut buf2, &ctx).unwrap();
        let max = buf2.iter().copied().fold(0.0f32, f32::max);
        assert!(
            max > 0.8,
            "Doubler through oversampler should produce amplified output: max={max}"
        );
    }

    #[test]
    fn test_oversampled_plugin_propagates_inner_process_error() {
        use crate::parameters::{Parameter, ParameterId, ParameterValue};
        use crate::plugin::{InPlacePlugin, PluginInfo, ProcessContext};

        struct ErrorPlugin;
        impl InPlacePlugin for ErrorPlugin {
            fn info(&self) -> PluginInfo {
                PluginInfo::new("Error", "1.0", "Test")
            }
            fn channels(&self) -> usize {
                1
            }
            fn parameters(&self) -> Vec<Parameter> {
                vec![]
            }
            fn set_parameter(
                &mut self,
                _: ParameterId,
                _: ParameterValue,
            ) -> crate::plugin::PluginResult<()> {
                Ok(())
            }
            fn get_parameter(&self, _: &ParameterId) -> Option<ParameterValue> {
                None
            }
            fn process_in_place(
                &mut self,
                _buffer: &mut [f32],
                _context: &ProcessContext,
            ) -> crate::plugin::PluginResult<usize> {
                Err("inner failed".to_string())
            }
        }

        let mut os = OversampledPlugin::new(ErrorPlugin, 2, 1).unwrap();
        os.initialize(48000).unwrap();

        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: 256,
        };
        let mut buf = vec![0.0f32; 256];
        let err = os.process_in_place(&mut buf, &ctx).unwrap_err();

        assert!(err.contains("inner failed"));
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
