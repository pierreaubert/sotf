// ============================================================================
// Pure Rust IAMF (Immersive Audio Model and Formats) Decoder
// ============================================================================
//
// Implements IAMF v1.1.0 bitstream parsing and rendering.
// No C/C++ dependencies — reuses SotF's Ambisonics decoder and speaker configs.

pub mod codec;
pub mod error;
pub mod mixer;
pub mod obu;
pub mod renderer;
pub mod types;

use std::io::{Read, Seek};

use error::{IamfError, IamfResult};
use mixer::MixState;
use obu::parser::{IamfDescriptors, parse_descriptors, parse_temporal_unit_with_kinds};
use renderer::ElementRenderer;
use types::*;

use sotf_host::speaker_config::{SpeakerConfig, get_speaker_config};

/// Main IAMF decoder.
///
/// Parses IAMF bitstream, decodes substreams, renders to target speaker layout.
/// All intermediate buffers are pre-allocated during `open()` to avoid heap
/// allocations in the decode hot path.
pub struct IamfDecoder {
    /// Raw IAMF data
    data: Vec<u8>,
    /// Parsed descriptor section
    descriptors: IamfDescriptors,
    /// Byte offset where temporal units begin
    temporal_offset: usize,
    /// Current read position in the data
    position: usize,
    /// Selected mix presentation index
    selected_mix: usize,
    /// Output layout
    output_layout: &'static SpeakerConfig,
    /// Element renderers
    renderers: Vec<Box<dyn ElementRenderer>>,
    /// Mix state
    mix_state: MixState,
    /// Substream decoders
    substream_decoders: Vec<Box<dyn codec::SubstreamDecoder>>,
    /// Per-element substream IDs (parallel to `renderers`)
    element_substream_ids: Vec<Vec<u32>>,
    /// Output spec
    spec: IamfSpec,
    /// Whether we've reached end of stream
    eof: bool,
    /// Frame position (in PCM frames from start)
    frame_position: u64,

    // -- Pre-allocated decode buffers (avoid per-call heap allocations) --
    /// Per-substream decoded PCM scratch slots (indexed by substream ID).
    /// `None` = not yet decoded this temporal unit.
    decoded_bufs: Vec<Option<Vec<f32>>>,
    /// Per-element render output buffer (parallel to `renderers`).
    element_out_bufs: Vec<Vec<f32>>,
}

impl std::fmt::Debug for IamfDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IamfDecoder")
            .field("selected_mix", &self.selected_mix)
            .field("spec", &self.spec)
            .field("eof", &self.eof)
            .field("frame_position", &self.frame_position)
            .finish()
    }
}

impl IamfDecoder {
    /// Open an IAMF stream from a reader.
    pub fn open<R: Read + Seek>(mut reader: R) -> IamfResult<Self> {
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;

        let (descriptors, temporal_offset) = parse_descriptors(&data)?;

        if descriptors.mix_presentations.is_empty() {
            return Err(IamfError::NoMixPresentations);
        }

        // Select first mix presentation by default
        let selected_mix = 0;
        let mix = &descriptors.mix_presentations[selected_mix];
        let sub_mix = mix.sub_mixes.first().ok_or(IamfError::NoMixPresentations)?;

        // Determine output layout
        let layout_id = sub_mix
            .output_layout
            .to_speaker_config_id()
            .unwrap_or("2.0");
        let output_layout = get_speaker_config(layout_id).ok_or_else(|| {
            IamfError::ParseError(format!("No SotF config for layout {layout_id}"))
        })?;

        // Create renderers for each audio element in the sub-mix
        let mut renderers: Vec<Box<dyn ElementRenderer>> = Vec::new();
        let mut substream_decoders: Vec<Box<dyn codec::SubstreamDecoder>> = Vec::new();
        let mut element_substream_ids: Vec<Vec<u32>> = Vec::new();

        for emc in &sub_mix.element_mix_configs {
            let element = descriptors
                .audio_elements
                .iter()
                .find(|ae| ae.audio_element_id == emc.audio_element_id)
                .ok_or(IamfError::UnknownAudioElement(emc.audio_element_id))?;

            let codec_config = descriptors
                .codec_configs
                .iter()
                .find(|cc| cc.codec_config_id == element.codec_config_id)
                .ok_or(IamfError::UnknownCodecConfig(element.codec_config_id))?;

            // Create renderer
            let r = renderer::create_renderer(element, codec_config, output_layout)?;
            renderers.push(r);
            element_substream_ids.push(element.substream_ids.clone());

            // Create substream decoders
            for (local_idx, _ss_id) in element.substream_ids.iter().enumerate() {
                // Determine channels per substream from element config
                // Use element-local index, not global substream ID
                let ss_channels = match &element.element_config {
                    ElementConfig::Channel(config) => {
                        let layer = config.layers.last().unwrap();
                        let coupled_count = layer.coupled_substream_count as usize;
                        if local_idx < coupled_count { 2 } else { 1 }
                    }
                    ElementConfig::Scene(config) => {
                        let coupled = config.coupled_substream_count as usize;
                        if local_idx < coupled { 2 } else { 1 }
                    }
                };

                let decoder = codec::create_substream_decoder(
                    codec_config.codec_id,
                    ss_channels,
                    codec_config.bit_depth,
                    codec_config.sample_rate,
                    &codec_config.decoder_config,
                )?;
                substream_decoders.push(decoder);
            }
        }

        let mix_state = MixState::from_sub_mix(sub_mix);

        // Determine sample rate from first codec config
        let first_codec = descriptors
            .codec_configs
            .first()
            .ok_or(IamfError::ParseError("No codec configs".into()))?;

        let spec = IamfSpec {
            primary_profile: descriptors.primary_profile,
            sample_rate: first_codec.sample_rate,
            bit_depth: first_codec.bit_depth,
            num_samples_per_frame: first_codec.num_samples_per_frame,
            output_channels: output_layout.total_channels as u16,
            output_layout: sub_mix.output_layout,
        };

        // Pre-allocate decode scratch buffers
        let num_decoders = substream_decoders.len();
        let frames_per_block = spec.num_samples_per_frame as usize;
        let out_ch = output_layout.total_channels;
        let out_buf_len = frames_per_block * out_ch;

        let decoded_bufs: Vec<Option<Vec<f32>>> = (0..num_decoders).map(|_| None).collect();
        let element_out_bufs: Vec<Vec<f32>> = (0..renderers.len())
            .map(|_| vec![0.0_f32; out_buf_len])
            .collect();

        Ok(Self {
            data,
            descriptors,
            temporal_offset,
            position: temporal_offset,
            selected_mix,
            output_layout,
            renderers,
            mix_state,
            substream_decoders,
            element_substream_ids,
            spec,
            eof: false,
            frame_position: 0,
            decoded_bufs,
            element_out_bufs,
        })
    }

    /// Get the IAMF stream specification.
    pub fn spec(&self) -> &IamfSpec {
        &self.spec
    }

    /// Get all mix presentations.
    pub fn mix_presentations(&self) -> &[MixPresentation] {
        &self.descriptors.mix_presentations
    }

    /// Get all audio elements.
    pub fn audio_elements(&self) -> &[AudioElement] {
        &self.descriptors.audio_elements
    }

    /// Select a mix presentation by index.
    pub fn select_mix_presentation(&mut self, index: usize) -> IamfResult<()> {
        if index >= self.descriptors.mix_presentations.len() {
            return Err(IamfError::UnknownMixPresentation(index as u32));
        }
        self.selected_mix = index;

        let mix = &self.descriptors.mix_presentations[self.selected_mix];
        let sub_mix = mix.sub_mixes.first().ok_or(IamfError::NoMixPresentations)?;

        self.mix_state = MixState::from_sub_mix(sub_mix);
        Ok(())
    }

    /// Set the output speaker layout.
    pub fn set_output_layout(&mut self, layout_id: &str) -> IamfResult<()> {
        let config = get_speaker_config(layout_id)
            .ok_or_else(|| IamfError::ParseError(format!("Unknown layout: {layout_id}")))?;
        self.output_layout = config;

        // Rebuild renderers and pre-allocated buffers
        self.rebuild_renderers()?;
        self.reallocate_buffers();

        self.spec.output_channels = config.total_channels as u16;
        Ok(())
    }

    fn rebuild_renderers(&mut self) -> IamfResult<()> {
        self.renderers.clear();
        self.element_substream_ids.clear();
        let mix = &self.descriptors.mix_presentations[self.selected_mix];
        let sub_mix = mix.sub_mixes.first().ok_or(IamfError::NoMixPresentations)?;

        for emc in &sub_mix.element_mix_configs {
            let element = self
                .descriptors
                .audio_elements
                .iter()
                .find(|ae| ae.audio_element_id == emc.audio_element_id)
                .ok_or(IamfError::UnknownAudioElement(emc.audio_element_id))?;

            let codec_config = self
                .descriptors
                .codec_configs
                .iter()
                .find(|cc| cc.codec_config_id == element.codec_config_id)
                .ok_or(IamfError::UnknownCodecConfig(element.codec_config_id))?;

            let r = renderer::create_renderer(element, codec_config, self.output_layout)?;
            self.renderers.push(r);
            self.element_substream_ids
                .push(element.substream_ids.clone());
        }
        Ok(())
    }

    fn reallocate_buffers(&mut self) {
        let frames = self.spec.num_samples_per_frame as usize;
        let out_ch = self.output_layout.total_channels;
        let out_buf_len = frames * out_ch;

        self.decoded_bufs
            .resize_with(self.substream_decoders.len(), || None);
        self.element_out_bufs
            .resize_with(self.renderers.len(), || vec![0.0; out_buf_len]);
        for buf in &mut self.element_out_bufs {
            buf.resize(out_buf_len, 0.0);
        }
    }

    /// Decode the next temporal unit into the output buffer.
    /// Returns the number of PCM frames written.
    pub fn decode_next(&mut self, output: &mut [f32]) -> IamfResult<usize> {
        if self.eof || self.position >= self.data.len() {
            return Err(IamfError::EndOfStream);
        }

        let remaining = &self.data[self.position..];
        let kinds = self.descriptors.parameter_kinds();
        let (temporal_unit, consumed) = parse_temporal_unit_with_kinds(remaining, &kinds)?;
        self.position += consumed;

        // Apply parameter blocks
        let mix = &self.descriptors.mix_presentations[self.selected_mix];
        if let Some(sub_mix) = mix.sub_mixes.first() {
            for pb in &temporal_unit.parameter_blocks {
                self.mix_state.apply_parameter_block(pb, sub_mix);
            }
        }

        let frames_per_block = self.spec.num_samples_per_frame as usize;
        let out_ch = self.output_layout.total_channels;

        // Reset decoded buffer slots (no allocation — just sets Options to None)
        for slot in &mut self.decoded_bufs {
            *slot = None;
        }

        // Decode substream audio frames into pre-allocated slots
        for frame_obu in &temporal_unit.audio_frames {
            let ss_id = frame_obu.substream_id as usize;
            if ss_id < self.substream_decoders.len() {
                let pcm = self.substream_decoders[ss_id].decode_frame(&frame_obu.payload)?;
                self.decoded_bufs[ss_id] = Some(pcm);
            }
        }

        // Render each element with only its own substreams.
        // Use `take()` instead of `clone()` — each substream belongs to exactly
        // one element, so we can move the decoded data without copying.
        let num_elements = self.renderers.len();
        for elem_idx in 0..num_elements {
            // Collect this element's substream PCM by taking ownership (zero-copy)
            let elem_pcm: Vec<Vec<f32>> = self.element_substream_ids[elem_idx]
                .iter()
                .map(|&ss_id| self.decoded_bufs[ss_id as usize].take().unwrap_or_default())
                .collect();

            // Reuse pre-allocated output buffer
            let elem_out = &mut self.element_out_bufs[elem_idx];
            let out_len = frames_per_block * out_ch;
            elem_out[..out_len].fill(0.0);
            self.renderers[elem_idx].render(
                &elem_pcm,
                &mut elem_out[..out_len],
                frames_per_block,
            )?;
        }

        // Mix elements using pre-allocated element output buffers
        let out_frames = frames_per_block;
        let out_len = out_frames * out_ch;
        if output.len() < out_len {
            return Err(IamfError::ParseError("Output buffer too small".into()));
        }

        self.mix_state
            .mix_from_bufs(&self.element_out_bufs, output, out_frames)?;

        // Apply trimming
        let trim_start = temporal_unit
            .audio_frames
            .first()
            .map_or(0, |f| f.samples_to_trim_start as usize);
        let trim_end = temporal_unit
            .audio_frames
            .first()
            .map_or(0, |f| f.samples_to_trim_end as usize);

        let actual_frames = out_frames
            .saturating_sub(trim_start)
            .saturating_sub(trim_end);

        // Shift output if trimming from start
        if trim_start > 0 && actual_frames > 0 {
            let src_start = trim_start * out_ch;
            let copy_len = actual_frames * out_ch;
            output.copy_within(src_start..src_start + copy_len, 0);
        }

        self.frame_position += actual_frames as u64;

        if self.position >= self.data.len() {
            self.eof = true;
        }

        Ok(actual_frames)
    }

    /// Seek to a frame position.
    ///
    /// Currently only seeking to the start (position 0) is supported.
    /// IAMF seeking to arbitrary positions requires scanning temporal
    /// delimiters from the beginning of the stream.
    pub fn seek(&mut self, frame_position: u64) -> IamfResult<()> {
        if frame_position != 0 {
            return Err(IamfError::SeekError(format!(
                "IAMF seeking only supports position 0, got {frame_position}"
            )));
        }

        self.position = self.temporal_offset;
        self.frame_position = 0;
        self.eof = false;

        // Reset all substream decoders
        for decoder in &mut self.substream_decoders {
            decoder.reset();
        }

        Ok(())
    }

    /// Get current position in frames.
    pub fn position(&self) -> u64 {
        self.frame_position
    }

    /// Check if end of stream reached.
    pub fn is_eof(&self) -> bool {
        self.eof
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iamf_spec_default() {
        let spec = IamfSpec {
            primary_profile: 0,
            sample_rate: 48000,
            bit_depth: 16,
            num_samples_per_frame: 960,
            output_channels: 2,
            output_layout: IamfChannelLayout::Stereo,
        };
        assert_eq!(spec.sample_rate, 48000);
        assert_eq!(spec.output_channels, 2);
    }
}
