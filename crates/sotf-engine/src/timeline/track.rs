// ============================================================================
// Track — An audio channel on the timeline with regions and a plugin chain
// ============================================================================

use super::clip::Region;
use crate::decoder::core::{AudioDecoder, AudioSpec, DecodedAudio};
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
    offset_in_block: usize,
    reverse: bool,
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
            volume: 1.0,
            pan: 0.0,
            muted: false,
            solo: false,
            channels,
            decoders: HashMap::new(),
            decode_buf: DecodedAudio::new(spec),
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
        self.chain.build()
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
            // Apply time-stretch: map clip position to source position
            let stretched_pos =
                if region.clip.time_stretch_ratio != 1.0 && region.clip.time_stretch_ratio > 0.0 {
                    (clip_position as f64 * region.clip.time_stretch_ratio) as u64
                } else {
                    clip_position
                };
            // Apply reverse: read from end of source instead of start
            let source_position = if region.clip.reverse {
                let end = region.clip.source_offset_samples + region.clip.duration_samples;
                end.saturating_sub(stretched_pos + 1)
            } else {
                region.clip.source_offset_samples + stretched_pos
            };
            let offset_in_block = (overlap_start - block_start) as usize;

            self.overlap_work.push(OverlapWork {
                region_idx,
                overlap_frames,
                clip_position,
                source_position,
                offset_in_block,
                reverse: region.clip.reverse,
            });
        }

        // Remove decoders for regions no longer overlapping
        let active_indices: Vec<usize> = self.overlap_work.iter().map(|w| w.region_idx).collect();
        self.decoders.retain(|k, _| active_indices.contains(k));

        // Phase 2: Decode and mix (mutable access to decoders and buffers)
        // Use take() to move the Vec without allocating (put back after loop)
        let mut work_items = std::mem::take(&mut self.overlap_work);
        for work in &work_items {
            // Ensure decoder exists and is seeked
            if !self.decoders.contains_key(&work.region_idx) {
                let region = &self.regions[work.region_idx];
                let mut decoder =
                    crate::decoder::core::create_decoder_from_source(&region.clip.source)
                        .map_err(|e| format!("Failed to open source: {e}"))?;
                if work.source_position > 0 {
                    if let Err(e) = decoder.seek(work.source_position) {
                        return Err(format!(
                            "Seek failed for region {} on track '{}': {e}",
                            work.region_idx, self.name
                        ));
                    }
                }
                self.decoders.insert(
                    work.region_idx,
                    ActiveDecoder {
                        decoder,
                        source_position: work.source_position,
                    },
                );
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

            // Decode
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

            // Reverse decoded samples if clip is reversed
            if work.reverse && usable_frames > 1 {
                let samples = &mut self.decode_buf.samples;
                for f in 0..usable_frames / 2 {
                    let f2 = usable_frames - 1 - f;
                    for ch in 0..src_channels {
                        samples.swap(f * src_channels + ch, f2 * src_channels + ch);
                    }
                }
            }

            // Mix decoded audio into mix_buf with per-clip gain/fade
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

            // Update decoder position to match where the decoder actually is
            // (it advanced by `decoded` frames, which may be more than `usable_frames`)
            dec.source_position = work.source_position + decoded as u64;
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
        self.decoders.clear();
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
}
