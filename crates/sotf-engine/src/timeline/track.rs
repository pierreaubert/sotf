// ============================================================================
// Track — An audio channel on the timeline with regions and a plugin chain
// ============================================================================

use super::clip::Region;
use crate::decoder::core::{AudioDecoder, AudioSpec, DecodedAudio};
use crate::engine::PluginConfig;
use sotf_plugins::DawHost;
use std::collections::HashMap;

/// An audio track on the timeline.
///
/// Each track contains a list of regions (clips placed at positions), a per-track
/// plugin chain (DawHost), and volume/pan/mute/solo controls.
pub struct Track {
    /// Track name (for display)
    pub name: String,
    /// Regions placed on this track
    pub regions: Vec<Region>,
    /// Per-track plugin chain
    pub chain: DawHost,
    /// Serializable plugin configs used to rebuild `chain`.
    pub plugin_configs: Vec<PluginConfig>,
    /// Track volume (linear, 1.0 = unity)
    pub volume: f32,
    /// Track pan (-1.0 = full left, 0.0 = center, 1.0 = full right)
    pub pan: f32,
    /// Muted (track produces silence)
    pub muted: bool,
    /// Soloed (only soloed tracks produce audio when any track is soloed)
    pub solo: bool,
    /// Number of output channels for this track
    pub channels: usize,
    /// Decoders for active clips, keyed by region index
    decoders: HashMap<usize, ActiveDecoder>,
    /// Pre-allocated decode buffer
    decode_buf: DecodedAudio,
    /// Reused source-span buffer for fractional varispeed reads.
    stretch_buf: Vec<f32>,
    /// Pre-allocated mix buffer for summing clips
    mix_buf: Vec<f32>,
    /// Pre-allocated chain output buffer
    chain_output: Vec<f32>,
    /// Scratch vec for collecting overlap work items (avoids allocation)
    overlap_work: Vec<OverlapWork>,
}

/// Pre-computed overlap info for one region, collected before decoding.
struct OverlapWork {
    region_idx: usize,
    overlap_frames: usize,
    clip_position: u64,
    source_position: u64,
    source_start: f64,
    source_step: f64,
    source_frames_needed: usize,
    offset_in_block: usize,
}

/// A decoder that is currently active for a region.
struct ActiveDecoder {
    decoder: Box<dyn AudioDecoder>,
    /// Current read position in source samples
    source_position: u64,
}

impl Track {
    pub fn new(name: impl Into<String>, channels: usize, sample_rate: u32) -> Self {
        let spec = AudioSpec {
            sample_rate,
            channels: channels as u16,
            bits_per_sample: 32,
            total_frames: None,
        };
        Self {
            name: name.into(),
            regions: Vec::new(),
            chain: DawHost::new(channels, sample_rate),
            plugin_configs: Vec::new(),
            volume: 1.0,
            pan: 0.0,
            muted: false,
            solo: false,
            channels,
            decoders: HashMap::new(),
            decode_buf: DecodedAudio::new(spec),
            stretch_buf: Vec::new(),
            mix_buf: Vec::new(),
            chain_output: Vec::new(),
            overlap_work: Vec::new(),
        }
    }

    /// Add a region to this track.
    pub fn add_region(&mut self, region: Region) {
        self.regions.push(region);
    }

    /// Build the track's plugin chain. Must be called after adding plugins.
    pub fn build(&mut self) -> Result<(), String> {
        self.prepare_decoders()?;
        self.chain.build()
    }

    /// Pre-allocate scratch used by `render_block` for the timeline's fixed
    /// processing block size. The timeline calls this during `build()` so
    /// fractional and reverse playback cannot grow a Vec on the audio thread.
    pub(crate) fn prepare_render_buffers(&mut self, max_frames: usize) {
        let total_samples = max_frames.saturating_mul(self.channels);
        self.mix_buf.resize(total_samples, 0.0);
        self.chain_output
            .resize(max_frames.saturating_mul(self.chain.output_channels()), 0.0);
        if self.overlap_work.capacity() < self.regions.len() {
            self.overlap_work
                .reserve(self.regions.len().saturating_sub(self.overlap_work.len()));
        }

        let max_source_channels = self
            .decoders
            .values()
            .map(|active| active.decoder.spec().channels as usize)
            .max()
            .unwrap_or(self.channels);
        let max_source_frames = self
            .regions
            .iter()
            .map(|region| {
                let ratio = if region.clip.time_stretch_ratio.is_finite()
                    && region.clip.time_stretch_ratio > 0.0
                {
                    region.clip.time_stretch_ratio
                } else {
                    1.0
                };
                (((max_frames.saturating_sub(1) as f64 * ratio).ceil() as usize) + 2)
                    .min(region.clip.duration_samples as usize)
            })
            .max()
            .unwrap_or(0);
        let required_samples = max_source_frames.saturating_mul(max_source_channels);
        self.stretch_buf.clear();
        if self.stretch_buf.capacity() < required_samples {
            self.stretch_buf.reserve(required_samples);
        }
    }

    fn prepare_decoders(&mut self) -> Result<(), String> {
        for (region_idx, region) in self.regions.iter().enumerate() {
            if self.decoders.contains_key(&region_idx) {
                continue;
            }

            let decoder = crate::decoder::core::create_decoder_from_source(&region.clip.source)
                .map_err(|e| format!("Failed to open source: {e}"))?;
            self.decoders.insert(
                region_idx,
                ActiveDecoder {
                    decoder,
                    source_position: 0,
                },
            );
        }

        Ok(())
    }

    /// Render one block of audio at the given timeline position.
    ///
    /// Decodes all overlapping clips, applies per-clip fades/gain, sums them,
    /// and processes through the track's plugin chain.
    pub fn render_block(
        &mut self,
        position_samples: u64,
        num_frames: usize,
        output: &mut [f32],
    ) -> Result<usize, String> {
        let total_samples = num_frames * self.channels;

        // Ensure buffers are large enough
        if self.mix_buf.len() < total_samples {
            self.mix_buf.resize(total_samples, 0.0);
        }
        self.mix_buf[..total_samples].fill(0.0);

        // Phase 1: Collect overlap info (immutable borrow of self.regions only)
        self.overlap_work.clear();
        let block_start = position_samples;
        let block_end = position_samples + num_frames as u64;
        for (region_idx, region) in self.regions.iter().enumerate() {
            if !region.overlaps(position_samples, num_frames as u64) {
                continue;
            }
            let region_start = region.position_samples;
            let region_end = region.end_samples();
            let overlap_start = block_start.max(region_start);
            let overlap_end = block_end.min(region_end);
            let overlap_frames = (overlap_end - overlap_start) as usize;
            if overlap_frames == 0 {
                continue;
            }
            let clip_position = overlap_start - region_start;
            let ratio = if region.clip.time_stretch_ratio > 0.0 {
                region.clip.time_stretch_ratio
            } else {
                1.0
            };
            let source_offset = region.clip.source_offset_samples as f64;
            let source_end =
                (region.clip.source_offset_samples + region.clip.duration_samples) as f64;
            let relative_start = clip_position as f64 * ratio;
            let source_start = if region.clip.reverse {
                source_end - 1.0 - relative_start
            } else {
                source_offset + relative_start
            };
            let source_step = if region.clip.reverse { -ratio } else { ratio };
            let source_last = source_start + source_step * overlap_frames.saturating_sub(1) as f64;
            let source_min = source_start.min(source_last).max(source_offset);
            let source_max = source_start.max(source_last).min(source_end - 1.0);
            let source_position = source_min.floor() as u64;
            let source_frames_needed = if source_max >= source_min {
                (source_max.floor() as u64)
                    .saturating_sub(source_position)
                    .saturating_add(2)
                    .min(
                        region.clip.source_offset_samples + region.clip.duration_samples
                            - source_position,
                    ) as usize
            } else {
                0
            };
            let offset_in_block = (overlap_start - block_start) as usize;

            self.overlap_work.push(OverlapWork {
                region_idx,
                overlap_frames,
                clip_position,
                source_position,
                source_start,
                source_step,
                source_frames_needed,
                offset_in_block,
            });
        }

        // Phase 2: Decode and mix (mutable access to decoders and buffers)
        // Use take() to move the Vec without allocating (put back after loop)
        let work_items = std::mem::take(&mut self.overlap_work);
        for work in &work_items {
            // Ensure decoder exists and is seeked
            if !self.decoders.contains_key(&work.region_idx) {
                return Err(format!(
                    "Decoder for region {} on track '{}' is not prepared; call build() before rendering",
                    work.region_idx, self.name
                ));
            } else {
                let dec = self.decoders.get_mut(&work.region_idx).ok_or_else(|| {
                    format!(
                        "Missing active decoder for region {} on track '{}'",
                        work.region_idx, self.name
                    )
                })?;
                if dec.source_position != work.source_position {
                    dec.decoder.seek(work.source_position).map_err(|e| {
                        format!(
                            "Seek failed for region {} on track '{}': {e}",
                            work.region_idx, self.name
                        )
                    })?;
                    dec.source_position = work.source_position;
                }
            }

            // Preserve the allocation-free common path for ordinary forward
            // playback. Fractional/reverse reads use the span buffer below.
            if work.source_step == 1.0 && work.source_start.fract() == 0.0 {
                self.decode_buf.clear();
                let dec = self.decoders.get_mut(&work.region_idx).ok_or_else(|| {
                    format!(
                        "Missing active decoder for region {} on track '{}'",
                        work.region_idx, self.name
                    )
                })?;
                let decoded = dec
                    .decoder
                    .decode_into(&mut self.decode_buf)
                    .map_err(|e| format!("Decode error on track '{}': {e}", self.name))?;
                if decoded == 0 {
                    continue;
                }

                let usable_frames = decoded.min(work.overlap_frames);
                let src_channels = self.decode_buf.spec.channels as usize;
                let region = &self.regions[work.region_idx];
                let clip_gain = region.clip.linear_gain();
                for frame in 0..usable_frames {
                    let gain =
                        clip_gain * region.clip.fade_gain_at(work.clip_position + frame as u64);
                    for ch in 0..self.channels.min(src_channels) {
                        let src_idx = frame * src_channels + ch;
                        let dst_idx = (work.offset_in_block + frame) * self.channels + ch;
                        if src_idx < self.decode_buf.samples.len() && dst_idx < total_samples {
                            self.mix_buf[dst_idx] += self.decode_buf.samples[src_idx] * gain;
                        }
                    }
                }
                dec.source_position = work.source_position + decoded as u64;
                continue;
            }

            // Decode the complete source span needed for this output block.
            self.stretch_buf.clear();
            let mut source_frames = 0usize;
            let mut decoder_advanced = 0usize;
            let mut src_channels = 0usize;
            while source_frames < work.source_frames_needed {
                self.decode_buf.clear();
                let dec = self.decoders.get_mut(&work.region_idx).ok_or_else(|| {
                    format!(
                        "Missing active decoder for region {} on track '{}'",
                        work.region_idx, self.name
                    )
                })?;
                let decoded = dec
                    .decoder
                    .decode_into(&mut self.decode_buf)
                    .map_err(|e| format!("Decode error on track '{}': {e}", self.name))?;
                if decoded == 0 {
                    break;
                }
                decoder_advanced += decoded;
                src_channels = self.decode_buf.spec.channels as usize;
                let take_frames = decoded.min(work.source_frames_needed - source_frames);
                let take_samples = take_frames.saturating_mul(src_channels);
                if self.stretch_buf.len().saturating_add(take_samples) > self.stretch_buf.capacity()
                {
                    return Err(format!(
                        "Stretch scratch for track '{}' is too small for {} frames; rebuild the timeline for the configured block size",
                        self.name, work.overlap_frames
                    ));
                }
                self.stretch_buf
                    .extend_from_slice(&self.decode_buf.samples[..take_samples]);
                source_frames += take_frames;
            }
            if source_frames == 0 || src_channels == 0 {
                continue;
            }

            // Mix decoded audio into mix_buf with per-clip gain/fade
            let region = &self.regions[work.region_idx];
            let clip_gain = region.clip.linear_gain();
            for frame in 0..work.overlap_frames {
                let source_position = (work.source_start + work.source_step * frame as f64).clamp(
                    region.clip.source_offset_samples as f64,
                    (region.clip.source_offset_samples + region.clip.duration_samples - 1) as f64,
                );
                let local_position = source_position - work.source_position as f64;
                if local_position < 0.0 {
                    continue;
                }
                let source_frame = local_position.floor() as usize;
                if source_frame >= source_frames {
                    continue;
                }
                let fraction = (local_position - source_frame as f64) as f32;
                let next_source_frame = (source_frame + 1).min(source_frames - 1);
                let gain = clip_gain * region.clip.fade_gain_at(work.clip_position + frame as u64);
                for ch in 0..self.channels.min(src_channels) {
                    let src_idx = source_frame * src_channels + ch;
                    let next_src_idx = next_source_frame * src_channels + ch;
                    let dst_idx = (work.offset_in_block + frame) * self.channels + ch;
                    if next_src_idx < self.stretch_buf.len() && dst_idx < total_samples {
                        let sample = self.stretch_buf[src_idx]
                            + (self.stretch_buf[next_src_idx] - self.stretch_buf[src_idx])
                                * fraction;
                        self.mix_buf[dst_idx] += sample * gain;
                    }
                }
            }

            // Update decoder position to match where the decoder actually is
            // (it may have advanced past the exact source span in its final chunk).
            let dec = self.decoders.get_mut(&work.region_idx).ok_or_else(|| {
                format!(
                    "Missing active decoder for region {} on track '{}'",
                    work.region_idx, self.name
                )
            })?;
            dec.source_position = work.source_position + decoder_advanced as u64;
        }
        // Return work_items Vec for reuse (avoids allocation on next call)
        self.overlap_work = work_items;

        // Phase 3: Process through track plugin chain
        if self.chain.plugin_count() > 0 {
            let out_ch = self.chain.output_channels();
            let out_len = num_frames * out_ch;
            if self.chain_output.len() < out_len {
                self.chain_output.resize(out_len, 0.0);
            }
            let frames = self.chain.process(
                &self.mix_buf[..total_samples],
                &mut self.chain_output[..out_len],
            )?;

            let copy_len = (frames * out_ch).min(output.len());
            output[..copy_len].copy_from_slice(&self.chain_output[..copy_len]);
            Self::apply_volume_pan(&mut output[..copy_len], out_ch, self.volume, self.pan);
            Ok(frames)
        } else {
            let copy_len = total_samples.min(output.len());
            output[..copy_len].copy_from_slice(&self.mix_buf[..copy_len]);
            Self::apply_volume_pan(
                &mut output[..copy_len],
                self.channels,
                self.volume,
                self.pan,
            );
            Ok(num_frames)
        }
    }

    /// Reset all decoders (e.g., after a seek).
    pub fn reset_decoders(&mut self) {
        for active in self.decoders.values_mut() {
            active.decoder.seek(0).ok();
            active.source_position = 0;
        }
    }

    /// Apply volume and pan to interleaved audio in-place.
    /// Uses linear pan law for stereo: center (pan=0) = unity per channel,
    /// hard left (pan=-1) = L at full / R silent, hard right (pan=1) = opposite.
    pub(crate) fn apply_volume_pan(output: &mut [f32], channels: usize, volume: f32, pan: f32) {
        if channels == 2 {
            // Linear pan law: gain_l = volume * (1 - pan) / 2, gain_r = volume * (1 + pan) / 2
            // At center (pan=0): both get volume * 0.5... no, that's -6dB.
            // Better: gain_l = volume * min(1, 1 - pan), gain_r = volume * min(1, 1 + pan)
            // At center: both = volume * 1.0 (unity). At hard L: L=volume, R=0.
            let gain_l = volume * (1.0 - pan).min(1.0);
            let gain_r = volume * (1.0 + pan).min(1.0);
            let num_frames = output.len() / 2;
            for f in 0..num_frames {
                output[f * 2] *= gain_l;
                output[f * 2 + 1] *= gain_r;
            }
        } else {
            for s in output.iter_mut() {
                *s *= volume;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::error::{AudioDecoderError, AudioDecoderResult};
    use crate::decoder::formats::AudioFormat;
    use crate::decoder::source::AudioSource;
    use crate::timeline::clip::{Clip, Region};

    struct FailingSeekDecoder {
        spec: AudioSpec,
    }

    struct RampDecoder {
        spec: AudioSpec,
        position: u64,
        total_frames: u64,
    }

    impl AudioDecoder for RampDecoder {
        fn spec(&self) -> &AudioSpec {
            &self.spec
        }

        fn format(&self) -> AudioFormat {
            AudioFormat::Wav
        }

        fn decode_into(&mut self, dest: &mut DecodedAudio) -> AudioDecoderResult<usize> {
            dest.clear();
            dest.spec = self.spec.clone();
            let frames = (self.total_frames - self.position).min(256) as usize;
            dest.samples
                .extend((0..frames).map(|i| (self.position + i as u64) as f32));
            self.position += frames as u64;
            Ok(frames)
        }

        fn seek(&mut self, frame_position: u64) -> AudioDecoderResult<()> {
            self.position = frame_position.min(self.total_frames);
            Ok(())
        }

        fn position(&self) -> u64 {
            self.position
        }

        fn is_eof(&self) -> bool {
            self.position >= self.total_frames
        }
    }

    impl AudioDecoder for FailingSeekDecoder {
        fn spec(&self) -> &AudioSpec {
            &self.spec
        }

        fn format(&self) -> AudioFormat {
            AudioFormat::Wav
        }

        fn decode_into(&mut self, dest: &mut DecodedAudio) -> AudioDecoderResult<usize> {
            dest.samples.clear();
            dest.spec = self.spec.clone();
            dest.samples.resize(self.spec.channels as usize, 0.5);
            Ok(1)
        }

        fn seek(&mut self, _frame_position: u64) -> AudioDecoderResult<()> {
            Err(AudioDecoderError::SeekFailed("synthetic failure".into()))
        }

        fn position(&self) -> u64 {
            0
        }

        fn is_eof(&self) -> bool {
            false
        }
    }

    #[test]
    fn render_block_keeps_decoder_position_when_seek_fails() {
        let mut track = Track::new("seek-test", 1, 48_000);
        track.add_region(Region::new(Clip::new(AudioSource::Driver, 1_000), 0));
        track.decoders.insert(
            0,
            ActiveDecoder {
                decoder: Box::new(FailingSeekDecoder {
                    spec: AudioSpec {
                        sample_rate: 48_000,
                        channels: 1,
                        bits_per_sample: 32,
                        total_frames: None,
                    },
                }),
                source_position: 0,
            },
        );

        let mut output = vec![0.0; 16];
        let err = track.render_block(10, 16, &mut output).unwrap_err();

        assert!(err.contains("Seek failed for region 0"));
        assert_eq!(track.decoders.get(&0).unwrap().source_position, 0);
    }

    #[test]
    fn time_stretch_ratio_is_applied_continuously_within_a_block() {
        let spec = AudioSpec {
            sample_rate: 48_000,
            channels: 1,
            bits_per_sample: 32,
            total_frames: Some(1_000),
        };
        let mut clip = Clip::new(AudioSource::Driver, 100);
        clip.time_stretch_ratio = 2.0;
        let mut track = Track::new("stretch-test", 1, 48_000);
        track.add_region(Region::new(clip, 0));
        track.decoders.insert(
            0,
            ActiveDecoder {
                decoder: Box::new(RampDecoder {
                    spec,
                    position: 0,
                    total_frames: 1_000,
                }),
                source_position: 0,
            },
        );

        track.prepare_render_buffers(8);
        let stretch_capacity = track.stretch_buf.capacity();
        let mut output = vec![0.0; 8];
        track.render_block(0, 8, &mut output).unwrap();
        assert_eq!(output, vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0]);
        assert_eq!(
            track.stretch_buf.capacity(),
            stretch_capacity,
            "prepared stretch rendering must not grow audio-thread scratch storage"
        );
    }

    #[test]
    fn fractional_reverse_clamps_the_source_end_symmetrically() {
        let spec = AudioSpec {
            sample_rate: 48_000,
            channels: 1,
            bits_per_sample: 32,
            total_frames: Some(100),
        };
        let mut clip = Clip::new(AudioSource::Driver, 100);
        clip.source_offset_samples = 10;
        clip.time_stretch_ratio = 0.5;
        clip.reverse = true;
        let mut track = Track::new("reverse-stretch-test", 1, 48_000);
        track.add_region(Region::new(clip, 0));
        track.decoders.insert(
            0,
            ActiveDecoder {
                decoder: Box::new(RampDecoder {
                    spec,
                    position: 0,
                    total_frames: 200,
                }),
                source_position: 0,
            },
        );

        track.prepare_render_buffers(4);
        let mut output = vec![0.0; 4];
        track.render_block(196, 4, &mut output).unwrap();
        assert_eq!(output, vec![11.0, 10.5, 10.0, 10.0]);
    }

    #[test]
    fn render_block_does_not_allocate_active_region_index_vec() {
        let source = include_str!("track.rs");

        assert!(
            !source.contains(concat!("let active_indices: Vec", "<usize>")),
            "audio track render must retain active decoders without allocating an index Vec per block"
        );
    }

    #[test]
    fn render_block_does_not_open_decoders_in_render_path() {
        let source = include_str!("track.rs");
        let render_start = source
            .find("pub fn render_block")
            .expect("render_block should exist");
        let render_body = &source[render_start..];
        let decode_factory = concat!("create_decoder", "_from_source");

        assert!(
            !render_body.contains(decode_factory),
            "audio track render must not open/create decoders from the render path"
        );
    }
}
