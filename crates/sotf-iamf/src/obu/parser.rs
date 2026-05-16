// ============================================================================
// IAMF OBU Bitstream Parser
// ============================================================================
//
// Parses OBU headers and descriptor OBUs from an IAMF bitstream.
// Based on IAMF v1.1.0 specification.

use std::collections::HashMap;

use crate::error::{IamfError, IamfResult};
use crate::obu::bitreader::BitReader;
use crate::types::*;

/// Hard upper bound on any `Vec::with_capacity(leb128)` allocation. 64 MiB
/// (capacity is in *elements*, so this is a per-vector ceiling — adversarial
/// leb128 sizes like `0xFFFF_FFFF` would otherwise request multi-GiB).
pub const MAX_LEB128_CAPACITY: usize = 64 * 1024 * 1024;

/// Validate that a leb128-derived count is plausible:
///   - <= `MAX_LEB128_CAPACITY` (64M),
///   - <= `remaining_bytes` (every element consumes at least one byte).
///
/// Returns the count as `usize` on success.
pub fn bounded_capacity(count: u32, remaining_bytes: usize) -> IamfResult<usize> {
    let n = count as usize;
    if n > MAX_LEB128_CAPACITY {
        return Err(IamfError::ParseError(format!(
            "Refusing leb128 capacity {n} > MAX_LEB128_CAPACITY ({MAX_LEB128_CAPACITY})"
        )));
    }
    if n > remaining_bytes {
        return Err(IamfError::ParseError(format!(
            "Refusing leb128 capacity {n} > remaining bytes {remaining_bytes}"
        )));
    }
    Ok(n)
}

/// OBU type identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObuType {
    CodecConfig,
    AudioElement,
    MixPresentation,
    ParameterBlock,
    TemporalDelimiter,
    AudioFrame,
    /// Audio frame with implicit substream ID (0-17)
    AudioFrameId(u8),
    SequenceHeader,
}

impl ObuType {
    pub fn from_u8(val: u8) -> IamfResult<Self> {
        match val {
            0 => Ok(Self::CodecConfig),
            1 => Ok(Self::AudioElement),
            2 => Ok(Self::MixPresentation),
            3 => Ok(Self::ParameterBlock),
            4 => Ok(Self::TemporalDelimiter),
            5 => Ok(Self::AudioFrame),
            6..=23 => Ok(Self::AudioFrameId(val - 6)),
            31 => Ok(Self::SequenceHeader),
            _ => Err(IamfError::InvalidObuType(val)),
        }
    }
}

/// Parsed OBU header
#[derive(Debug, Clone)]
pub struct ObuHeader {
    pub obu_type: ObuType,
    pub redundant_copy: bool,
    pub trimming_status: bool,
    pub extension_flag: bool,
    pub payload_size: usize,
    pub trim_start: u32,
    pub trim_end: u32,
}

// ============================================================================
// LEB128 (Little Endian Base 128) variable-length integer encoding
// ============================================================================

/// Read a leb128-encoded unsigned integer from a byte slice.
/// Returns (value, bytes_consumed).
pub fn read_leb128(data: &[u8]) -> IamfResult<(u64, usize)> {
    let mut result: u64 = 0;
    let mut shift = 0;
    for (i, &byte) in data.iter().enumerate() {
        if shift >= 64 {
            return Err(IamfError::ParseError("leb128 overflow".into()));
        }
        result |= (byte as u64 & 0x7F) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            return Ok((result, i + 1));
        }
    }
    Err(IamfError::ParseError("Truncated leb128".into()))
}

/// Read a leb128 u32 from cursor position, advancing the cursor.
fn read_leb128_u32(data: &[u8], pos: &mut usize) -> IamfResult<u32> {
    let (val, consumed) = read_leb128(&data[*pos..])?;
    *pos += consumed;
    Ok(val as u32)
}

fn read_u8(data: &[u8], pos: &mut usize) -> IamfResult<u8> {
    if *pos >= data.len() {
        return Err(IamfError::ParseError("Unexpected end of data".into()));
    }
    let val = data[*pos];
    *pos += 1;
    Ok(val)
}

fn read_u16_be(data: &[u8], pos: &mut usize) -> IamfResult<u16> {
    if *pos + 2 > data.len() {
        return Err(IamfError::ParseError("Unexpected end of data".into()));
    }
    let val = u16::from_be_bytes([data[*pos], data[*pos + 1]]);
    *pos += 2;
    Ok(val)
}

fn read_i16_be(data: &[u8], pos: &mut usize) -> IamfResult<i16> {
    if *pos + 2 > data.len() {
        return Err(IamfError::ParseError("Unexpected end of data".into()));
    }
    let val = i16::from_be_bytes([data[*pos], data[*pos + 1]]);
    *pos += 2;
    Ok(val)
}

fn read_u32_be(data: &[u8], pos: &mut usize) -> IamfResult<u32> {
    if *pos + 4 > data.len() {
        return Err(IamfError::ParseError("Unexpected end of data".into()));
    }
    let val = u32::from_be_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]);
    *pos += 4;
    Ok(val)
}

fn read_bytes<'a>(data: &'a [u8], pos: &mut usize, n: usize) -> IamfResult<&'a [u8]> {
    if *pos + n > data.len() {
        return Err(IamfError::ParseError("Unexpected end of data".into()));
    }
    let slice = &data[*pos..*pos + n];
    *pos += n;
    Ok(slice)
}

fn read_string(data: &[u8], pos: &mut usize) -> IamfResult<String> {
    // IAMF strings are null-terminated
    let start = *pos;
    while *pos < data.len() && data[*pos] != 0 {
        *pos += 1;
    }
    let s = String::from_utf8_lossy(&data[start..*pos]).to_string();
    if *pos < data.len() {
        *pos += 1; // skip null terminator
    }
    Ok(s)
}

// ============================================================================
// OBU Header Parsing
// ============================================================================

/// Parse an OBU header from a byte stream.
/// Returns the header and total bytes consumed (header + payload = full OBU).
pub fn parse_obu_header(data: &[u8]) -> IamfResult<(ObuHeader, usize)> {
    if data.is_empty() {
        return Err(IamfError::EndOfStream);
    }

    let mut pos = 0;
    let first_byte = read_u8(data, &mut pos)?;

    let obu_type_val = (first_byte >> 3) & 0x1F;
    let obu_type = ObuType::from_u8(obu_type_val)?;
    let redundant_copy = (first_byte >> 2) & 1 != 0;
    let trimming_status = (first_byte >> 1) & 1 != 0;
    let extension_flag = first_byte & 1 != 0;

    let payload_size = read_leb128_u32(data, &mut pos)? as usize;

    let mut trim_end = 0u32;
    let mut trim_start = 0u32;

    if trimming_status {
        trim_end = read_leb128_u32(data, &mut pos)?;
        trim_start = read_leb128_u32(data, &mut pos)?;
    }

    if extension_flag {
        let ext_size = read_leb128_u32(data, &mut pos)? as usize;
        pos += ext_size; // skip extension bytes
    }

    let header_size = pos;
    let total_size = header_size + payload_size;

    if total_size > data.len() {
        return Err(IamfError::TruncatedObu {
            expected: total_size,
            available: data.len(),
        });
    }

    Ok((
        ObuHeader {
            obu_type,
            redundant_copy,
            trimming_status,
            extension_flag,
            payload_size,
            trim_start,
            trim_end,
        },
        header_size,
    ))
}

// ============================================================================
// Descriptor OBU Parsing
// ============================================================================

/// Parse sequence header OBU payload.
pub fn parse_sequence_header(data: &[u8]) -> IamfResult<(u8, u8)> {
    let mut pos = 0;
    let ia_code = read_u32_be(data, &mut pos)?;
    if ia_code != u32::from_be_bytes(*b"iamf") {
        return Err(IamfError::InvalidMagic);
    }
    let primary_profile = read_u8(data, &mut pos)?;
    let additional_profile = read_u8(data, &mut pos)?;
    Ok((primary_profile, additional_profile))
}

/// Parse codec_config OBU payload.
pub fn parse_codec_config(data: &[u8]) -> IamfResult<CodecConfig> {
    let mut pos = 0;
    let codec_config_id = read_leb128_u32(data, &mut pos)?;

    let codec_id_bytes: [u8; 4] = read_bytes(data, &mut pos, 4)?.try_into().unwrap();
    let codec_id = CodecId::from_bytes(codec_id_bytes).ok_or_else(|| {
        IamfError::UnsupportedCodec(String::from_utf8_lossy(&codec_id_bytes).to_string())
    })?;

    let num_samples_per_frame = read_leb128_u32(data, &mut pos)?;
    let audio_roll_distance = read_i16_be(data, &mut pos)?;

    // Parse decoder_config based on codec type
    let (sample_rate, bit_depth) = match codec_id {
        CodecId::Opus => {
            // Opus decoder config: version(1), output_channel_count(1), pre_skip(2),
            // input_sample_rate(4), output_gain(2), mapping_family(1)
            let _version = read_u8(data, &mut pos)?;
            let _ch_count = read_u8(data, &mut pos)?;
            let _pre_skip = read_u16_be(data, &mut pos)?;
            let sr = read_u32_be(data, &mut pos)?;
            let _output_gain = read_i16_be(data, &mut pos)?;
            let _mapping_family = read_u8(data, &mut pos)?;
            (sr, 32) // Opus always outputs float32
        }
        CodecId::Flac => {
            // FLAC: read STREAMINFO metadata block
            // For now, extract sample rate and bit depth from minimum fields
            if data.len() > pos + 18 {
                let sr = (u32::from(data[pos + 10]) << 12)
                    | (u32::from(data[pos + 11]) << 4)
                    | (u32::from(data[pos + 12]) >> 4);
                let bps =
                    (u16::from(data[pos + 12] & 0x01) << 4) | (u16::from(data[pos + 13]) >> 4);
                pos = data.len(); // consume rest
                (sr, bps + 1)
            } else {
                (48000, 24) // fallback
            }
        }
        CodecId::AacLc => {
            // AAC-LC: AudioSpecificConfig
            // sample rate and channel info encoded in first 2+ bytes
            // Simplified: use common defaults
            pos = data.len();
            (48000, 16)
        }
        CodecId::Lpcm => {
            // LPCM config: sample_format_flags(1), sample_size(1), sample_rate(4)
            let _format_flags = read_u8(data, &mut pos)?;
            let sample_size = read_u8(data, &mut pos)?;
            let sr = read_u32_be(data, &mut pos)?;
            (sr, sample_size as u16)
        }
    };

    let decoder_config = data[pos..].to_vec();

    Ok(CodecConfig {
        codec_config_id,
        codec_id,
        num_samples_per_frame,
        audio_roll_distance,
        sample_rate,
        bit_depth,
        decoder_config,
    })
}

/// Parse audio_element OBU payload.
pub fn parse_audio_element(data: &[u8]) -> IamfResult<AudioElement> {
    let mut pos = 0;
    let audio_element_id = read_leb128_u32(data, &mut pos)?;

    // type byte: audio_element_type (3 bits) + reserved (5 bits) — per IAMF
    // §3.6.2. We use the bit reader so reserved bits are skipped explicitly
    // rather than incidentally trimmed by a `>> 6` shift.
    let type_byte_slice = read_bytes(data, &mut pos, 1)?;
    let element_type_val = {
        let mut br = BitReader::new(type_byte_slice);
        br.read_bits(3)? as u8
    };
    let element_type = match element_type_val {
        0 => AudioElementType::Channel,
        1 => AudioElementType::Scene,
        other => return Err(IamfError::UnsupportedElementType(other)),
    };

    let codec_config_id = read_leb128_u32(data, &mut pos)?;
    let num_substreams = read_leb128_u32(data, &mut pos)?;

    let cap = bounded_capacity(num_substreams, data.len().saturating_sub(pos))?;
    let mut substream_ids = Vec::with_capacity(cap);
    for _ in 0..num_substreams {
        substream_ids.push(read_leb128_u32(data, &mut pos)?);
    }

    // Parse num_parameters and parameter definitions
    let num_parameters = read_leb128_u32(data, &mut pos)?;
    let cap = bounded_capacity(num_parameters, data.len().saturating_sub(pos))?;
    let mut parameter_definitions = Vec::with_capacity(cap);
    for _ in 0..num_parameters {
        // parameter_definition_type is leb128 per IAMF §3.6.4. We dispatch on
        // it later when parsing parameter blocks.
        let pdt_raw = read_leb128_u32(data, &mut pos)?;
        let parameter_kind = ParameterDataKind::from_u32(pdt_raw).ok_or_else(|| {
            IamfError::ParseError(format!("Unknown parameter_definition_type: {pdt_raw}"))
        })?;
        let parameter_id = read_leb128_u32(data, &mut pos)?;
        let parameter_rate = read_leb128_u32(data, &mut pos)?;
        let mode_byte = read_u8(data, &mut pos)?;
        let param_definition_mode = (mode_byte >> 7) & 1 != 0;
        let mut duration = 0;
        let mut constant_subblock_duration = 0;
        if !param_definition_mode {
            duration = read_leb128_u32(data, &mut pos)?;
            constant_subblock_duration = read_leb128_u32(data, &mut pos)?;
            if constant_subblock_duration == 0 {
                let num_subblocks = read_leb128_u32(data, &mut pos)?;
                for _ in 0..num_subblocks.saturating_sub(1) {
                    let _subblock_dur = read_leb128_u32(data, &mut pos)?;
                }
            }
        }
        parameter_definitions.push(ParameterDefinition {
            parameter_id,
            parameter_rate,
            param_definition_mode,
            duration,
            constant_subblock_duration,
            parameter_kind,
        });
    }

    // Parse element-specific config
    let element_config = match element_type {
        AudioElementType::Channel => {
            let config = parse_scalable_channel_config(data, &mut pos)?;
            ElementConfig::Channel(config)
        }
        AudioElementType::Scene => {
            let config = parse_ambisonics_config(data, &mut pos)?;
            ElementConfig::Scene(config)
        }
    };

    Ok(AudioElement {
        audio_element_id,
        element_type,
        codec_config_id,
        num_substreams,
        substream_ids,
        element_config,
        parameter_definitions,
    })
}

fn parse_scalable_channel_config(
    data: &[u8],
    pos: &mut usize,
) -> IamfResult<ScalableChannelConfig> {
    // First byte: num_layers (3 bits) + reserved (5 bits)
    let header_byte = read_bytes(data, pos, 1)?;
    let num_layers = {
        let mut br = BitReader::new(header_byte);
        let n = br.read_bits(3)? as u8;
        // Remaining 5 reserved bits intentionally ignored.
        n
    };

    let mut layers = Vec::with_capacity(num_layers as usize);
    for _ in 0..num_layers {
        // Per-layer header (16 bits total):
        //   loudspeaker_layout (4) + output_gain_is_present (1)
        //   + recon_gain_is_present (1) + reserved (2)
        //   + substream_count (8) + coupled_substream_count (8)
        let layer_bytes = read_bytes(data, pos, 3)?;
        let mut br = BitReader::new(layer_bytes);
        let layout_idx = br.read_bits(4)? as u8;
        let loudspeaker_layout = IamfChannelLayout::from_layout_index(layout_idx)
            .ok_or_else(|| IamfError::ParseError(format!("Unknown layout index: {layout_idx}")))?;
        let output_gain_is_present = br.read_bool()?;
        let recon_gain_is_present = br.read_bool()?;
        br.skip_bits(2)?; // reserved
        let substream_count = br.read_bits(8)? as u8;
        let coupled_substream_count = br.read_bits(8)? as u8;

        let output_gain_db = if output_gain_is_present {
            // output_gain_flags (6 bits per-channel mask) + reserved (2)
            // + output_gain (i16 Q7.8)
            let og_bytes = read_bytes(data, pos, 3)?;
            let mut br = BitReader::new(og_bytes);
            let _flags = br.read_bits(6)?;
            br.skip_bits(2)?;
            let raw = br.read_bits(16)? as i16;
            raw as f32 / 256.0
        } else {
            0.0
        };

        layers.push(ChannelLayer {
            loudspeaker_layout,
            output_gain_is_present,
            recon_gain_is_present,
            substream_count,
            coupled_substream_count,
            output_gain_db,
        });
    }

    Ok(ScalableChannelConfig { num_layers, layers })
}

fn parse_ambisonics_config(data: &[u8], pos: &mut usize) -> IamfResult<AmbisonicsConfig> {
    // ambisonics_mode fits in a byte. Use the bit reader for consistency.
    let mode_bytes = read_bytes(data, pos, 1)?;
    let mode_val = {
        let mut br = BitReader::new(mode_bytes);
        br.read_bits(8)? as u8
    };
    let ambisonics_mode = match mode_val {
        0 => AmbisonicsMode::Mono,
        1 => AmbisonicsMode::Projection,
        _ => {
            return Err(IamfError::ParseError(format!(
                "Unknown ambisonics mode: {mode_val}"
            )));
        }
    };

    let cfg_bytes = read_bytes(data, pos, 3)?;
    let (output_channel_count, substream_count, coupled_substream_count) = {
        let mut br = BitReader::new(cfg_bytes);
        (
            br.read_bits(8)? as u8,
            br.read_bits(8)? as u8,
            br.read_bits(8)? as u8,
        )
    };

    let mapping_len = output_channel_count as usize;
    if mapping_len > data.len().saturating_sub(*pos) {
        return Err(IamfError::ParseError(format!(
            "ambisonics mapping length {mapping_len} > remaining bytes"
        )));
    }
    let mut channel_mapping = Vec::with_capacity(mapping_len);
    for _ in 0..output_channel_count {
        channel_mapping.push(read_u8(data, pos)?);
    }

    let demixing_matrix = if ambisonics_mode == AmbisonicsMode::Projection {
        let coupled = coupled_substream_count as usize;
        let uncoupled = (substream_count as usize).saturating_sub(coupled);
        let substream_channels = coupled * 2 + uncoupled;
        let matrix_size = output_channel_count as usize * substream_channels;
        if matrix_size.saturating_mul(2) > data.len().saturating_sub(*pos) {
            return Err(IamfError::ParseError(format!(
                "ambisonics matrix size {matrix_size} exceeds remaining bytes"
            )));
        }
        let mut matrix = Vec::with_capacity(matrix_size);
        for _ in 0..matrix_size {
            let val = read_i16_be(data, pos)?;
            matrix.push(val as f32 / 32768.0); // Q15 to float
        }
        matrix
    } else {
        Vec::new()
    };

    Ok(AmbisonicsConfig {
        ambisonics_mode,
        output_channel_count,
        substream_count,
        coupled_substream_count,
        channel_mapping,
        demixing_matrix,
    })
}

/// Parse mix_presentation OBU payload.
pub fn parse_mix_presentation(data: &[u8]) -> IamfResult<MixPresentation> {
    let mut pos = 0;
    let mix_presentation_id = read_leb128_u32(data, &mut pos)?;

    let count_label = read_leb128_u32(data, &mut pos)?;
    let cap = bounded_capacity(count_label, data.len().saturating_sub(pos))?;
    let mut annotations = Vec::with_capacity(cap);

    // Read language tags first
    let mut languages = Vec::with_capacity(cap);
    for _ in 0..count_label {
        languages.push(read_string(data, &mut pos)?);
    }

    // Read labels for each language
    for lang in &languages {
        let label = read_string(data, &mut pos)?;
        annotations.push(MixAnnotation {
            language: lang.clone(),
            label,
        });
    }

    let num_sub_mixes = read_leb128_u32(data, &mut pos)?;
    let cap = bounded_capacity(num_sub_mixes, data.len().saturating_sub(pos))?;
    let mut sub_mixes = Vec::with_capacity(cap);

    for _ in 0..num_sub_mixes {
        let num_audio_elements = read_leb128_u32(data, &mut pos)?;
        let cap = bounded_capacity(num_audio_elements, data.len().saturating_sub(pos))?;
        let mut element_mix_configs = Vec::with_capacity(cap);

        for _ in 0..num_audio_elements {
            let audio_element_id = read_leb128_u32(data, &mut pos)?;

            // Element annotations (skip for now)
            for _ in 0..count_label {
                let _label = read_string(data, &mut pos)?;
            }

            // rendering_config
            let _headphones_rendering_mode_byte = read_u8(data, &mut pos)?;
            let rendering_config_extension_size = read_leb128_u32(data, &mut pos)? as usize;
            pos += rendering_config_extension_size;

            // element mix gain
            let mix_gain = parse_mix_gain_config(data, &mut pos)?;

            element_mix_configs.push(ElementMixConfig {
                audio_element_id,
                mix_gain,
            });
        }

        // output mix gain
        let output_mix_gain = parse_mix_gain_config(data, &mut pos)?;

        // output layout
        let layout_byte = read_u8(data, &mut pos)?;
        let layout_type = (layout_byte >> 6) & 0x03;
        let output_layout = if layout_type == 0 {
            // Loudspeaker layout
            let sound_system_byte = read_u8(data, &mut pos)?;
            let sound_system = (sound_system_byte >> 4) & 0x0F;
            IamfChannelLayout::from_layout_index(sound_system).unwrap_or(IamfChannelLayout::Stereo)
        } else {
            // Binaural or reserved
            IamfChannelLayout::Binaural
        };

        // Loudness info
        let loudness = parse_loudness_info(data, &mut pos)?;

        sub_mixes.push(SubMix {
            num_audio_elements,
            element_mix_configs,
            output_mix_gain,
            output_layout,
            loudness,
        });
    }

    Ok(MixPresentation {
        mix_presentation_id,
        annotations,
        sub_mixes,
    })
}

fn parse_mix_gain_config(data: &[u8], pos: &mut usize) -> IamfResult<MixGainConfig> {
    let parameter_id = read_leb128_u32(data, pos)?;
    let _parameter_rate = read_leb128_u32(data, pos)?;
    let mode_byte = read_u8(data, pos)?;
    let param_definition_mode = (mode_byte >> 7) & 1 != 0;
    if !param_definition_mode {
        let _duration = read_leb128_u32(data, pos)?;
        let constant_subblock_duration = read_leb128_u32(data, pos)?;
        if constant_subblock_duration == 0 {
            let num_subblocks = read_leb128_u32(data, pos)?;
            for _ in 0..num_subblocks.saturating_sub(1) {
                let _dur = read_leb128_u32(data, pos)?;
            }
        }
    }
    let gain_raw = read_i16_be(data, pos)?;
    let default_mix_gain_db = gain_raw as f32 / 256.0; // Q7.8

    Ok(MixGainConfig {
        parameter_id,
        default_mix_gain_db,
    })
}

fn parse_loudness_info(data: &[u8], pos: &mut usize) -> IamfResult<LoudnessInfo> {
    let info_type = read_u8(data, pos)?;
    let integrated_raw = read_i16_be(data, pos)?;
    let peak_raw = read_i16_be(data, pos)?;

    let integrated_loudness = integrated_raw as f32 / 256.0; // Q7.8
    let digital_peak = peak_raw as f32 / 256.0;

    let true_peak = if info_type & 1 != 0 {
        let tp_raw = read_i16_be(data, pos)?;
        Some(tp_raw as f32 / 256.0)
    } else {
        None
    };

    // Skip anchored loudness if present (info_type bit 1)
    if info_type & 2 != 0 {
        let num_anchored = read_u8(data, pos)?;
        for _ in 0..num_anchored {
            let _anchor_element = read_u8(data, pos)?;
            let _anchored_loudness = read_i16_be(data, pos)?;
        }
    }

    // Skip layout extension if present (info_type bit 2)
    if info_type & 4 != 0 {
        let ext_size = read_leb128_u32(data, pos)? as usize;
        *pos += ext_size;
    }

    Ok(LoudnessInfo {
        info_type,
        integrated_loudness,
        digital_peak,
        true_peak,
    })
}

/// Parse parameter_block OBU payload, dispatching on the parameter's kind.
///
/// `kind_lookup` maps `parameter_id -> ParameterDataKind` and is built from
/// the descriptor section (audio_element OBUs declare each parameter's
/// `parameter_definition_type`). Parameter ids that aren't in the lookup
/// fall back to MixGain (mix-presentation parameters live outside the
/// audio_element kind map and are always MixGain in v1.1.0).
///
/// On a typed match the payload is parsed with the spec-correct shape:
/// DemixingInfo → `dmixp_mode (3 bits) + reserved (5)`, ReconGain → empty
/// (we emit a typed variant with no values rather than coercing it into
/// random MixGain bytes).
pub fn parse_parameter_block_with_kind(
    data: &[u8],
    kind_lookup: &HashMap<u32, ParameterDataKind>,
) -> IamfResult<ParameterBlock> {
    let mut pos = 0;
    let parameter_id = read_leb128_u32(data, &mut pos)?;
    let duration = read_leb128_u32(data, &mut pos)?;
    let constant_subblock_duration = read_leb128_u32(data, &mut pos)?;

    let num_subblocks = if constant_subblock_duration == 0 {
        read_leb128_u32(data, &mut pos)?
    } else if constant_subblock_duration > 0 && duration > 0 {
        duration.div_ceil(constant_subblock_duration)
    } else {
        1
    };

    let kind = kind_lookup
        .get(&parameter_id)
        .copied()
        .unwrap_or(ParameterDataKind::MixGain);

    // Subblocks may legitimately consume zero bytes per element (e.g. our
    // simplified ReconGain), so we only cap against the absolute element
    // ceiling here rather than `remaining_bytes`.
    let cap_n = num_subblocks as usize;
    if cap_n > MAX_LEB128_CAPACITY {
        return Err(IamfError::ParseError(format!(
            "Refusing num_subblocks {cap_n} > MAX_LEB128_CAPACITY"
        )));
    }
    let mut subblocks = Vec::with_capacity(cap_n);
    for i in 0..num_subblocks {
        let subblock_duration = if constant_subblock_duration != 0 {
            constant_subblock_duration
        } else if i + 1 < num_subblocks {
            read_leb128_u32(data, &mut pos)?
        } else {
            let used: u32 = subblocks
                .iter()
                .map(|sb: &ParameterSubblock| sb.subblock_duration)
                .sum();
            duration.saturating_sub(used)
        };

        let param_data = match kind {
            ParameterDataKind::MixGain => parse_mix_gain_payload(data, &mut pos)?,
            ParameterDataKind::DemixingInfo => {
                // dmixp_mode (3 bits) + reserved (5 bits)
                let byte = read_bytes(data, &mut pos, 1)?;
                let mut br = BitReader::new(byte);
                let dmixp_mode = br.read_bits(3)? as u8;
                ParameterData::DemixingInfo { dmixp_mode }
            }
            ParameterDataKind::ReconGain => {
                // Emit a typed variant with no gains so the mixer can't
                // silently apply this as MixGain. A spec-accurate parse
                // requires the owning audio_element's `recon_gain_is_present`
                // flags to know how many channels carry a recon gain byte.
                ParameterData::ReconGain {
                    recon_gains: Vec::new(),
                }
            }
        };

        subblocks.push(ParameterSubblock {
            subblock_duration,
            param_data,
        });
    }

    Ok(ParameterBlock {
        parameter_id,
        duration,
        constant_subblock_duration,
        subblocks,
    })
}

/// Backwards-compatible wrapper: every parameter is decoded as MixGain.
/// Prefer `parse_parameter_block_with_kind` whenever descriptors are
/// available.
pub fn parse_parameter_block(data: &[u8]) -> IamfResult<ParameterBlock> {
    let empty = HashMap::new();
    parse_parameter_block_with_kind(data, &empty)
}

fn parse_mix_gain_payload(data: &[u8], pos: &mut usize) -> IamfResult<ParameterData> {
    let animation_byte = read_u8(data, pos)?;
    let animation_type =
        AnimationType::from_u8(animation_byte & 0x07).unwrap_or(AnimationType::Step);

    let start_raw = read_i16_be(data, pos)?;
    let start_point_value = start_raw as f32 / 256.0;

    let (end_point_value, control_point_value, control_point_relative_time) = match animation_type {
        AnimationType::Step => (start_point_value, 0.0, 0.0),
        AnimationType::Linear => {
            let end_raw = read_i16_be(data, pos)?;
            (end_raw as f32 / 256.0, 0.0, 0.0)
        }
        AnimationType::Bezier => {
            let end_raw = read_i16_be(data, pos)?;
            let ctrl_raw = read_i16_be(data, pos)?;
            let ctrl_time_raw = read_u8(data, pos)?;
            (
                end_raw as f32 / 256.0,
                ctrl_raw as f32 / 256.0,
                ctrl_time_raw as f32 / 255.0,
            )
        }
    };

    Ok(ParameterData::MixGain {
        animation_type,
        start_point_value,
        end_point_value,
        control_point_value,
        control_point_relative_time,
    })
}

// ============================================================================
// High-level Parsing
// ============================================================================

/// Parsed descriptor section of an IAMF stream
#[derive(Debug, Clone)]
pub struct IamfDescriptors {
    pub primary_profile: u8,
    pub additional_profile: u8,
    pub codec_configs: Vec<CodecConfig>,
    pub audio_elements: Vec<AudioElement>,
    pub mix_presentations: Vec<MixPresentation>,
}

impl IamfDescriptors {
    /// Build a `parameter_id -> kind` map from all audio_element parameter
    /// definitions. Parameter blocks in temporal units reference these IDs;
    /// the kind drives `parse_parameter_block_with_kind` payload dispatch.
    pub fn parameter_kinds(&self) -> HashMap<u32, ParameterDataKind> {
        let mut map = HashMap::new();
        for ae in &self.audio_elements {
            for pd in &ae.parameter_definitions {
                map.insert(pd.parameter_id, pd.parameter_kind);
            }
        }
        map
    }
}

/// Parse all descriptor OBUs from the beginning of an IAMF stream.
/// Returns the descriptors and the byte offset where temporal units begin.
pub fn parse_descriptors(data: &[u8]) -> IamfResult<(IamfDescriptors, usize)> {
    let mut pos = 0;
    let mut primary_profile = 0u8;
    let mut additional_profile = 0u8;
    let mut codec_configs = Vec::new();
    let mut audio_elements = Vec::new();
    let mut mix_presentations = Vec::new();
    let mut found_header = false;

    while pos < data.len() {
        let remaining = &data[pos..];
        let (header, header_size) = match parse_obu_header(remaining) {
            Ok(h) => h,
            Err(IamfError::EndOfStream) => break,
            Err(e) => return Err(e),
        };

        let payload_start = pos + header_size;
        let payload_end = payload_start + header.payload_size;
        if payload_end > data.len() {
            return Err(IamfError::TruncatedObu {
                expected: payload_end,
                available: data.len(),
            });
        }
        let payload = &data[payload_start..payload_end];

        match header.obu_type {
            ObuType::SequenceHeader => {
                let (pp, ap) = parse_sequence_header(payload)?;
                primary_profile = pp;
                additional_profile = ap;
                found_header = true;
            }
            ObuType::CodecConfig => {
                codec_configs.push(parse_codec_config(payload)?);
            }
            ObuType::AudioElement => {
                audio_elements.push(parse_audio_element(payload)?);
            }
            ObuType::MixPresentation => {
                mix_presentations.push(parse_mix_presentation(payload)?);
            }
            ObuType::TemporalDelimiter
            | ObuType::AudioFrame
            | ObuType::AudioFrameId(_)
            | ObuType::ParameterBlock => {
                // Reached temporal unit section — stop descriptor parsing
                break;
            }
        }

        pos = payload_end;
    }

    if !found_header {
        return Err(IamfError::InvalidMagic);
    }

    Ok((
        IamfDescriptors {
            primary_profile,
            additional_profile,
            codec_configs,
            audio_elements,
            mix_presentations,
        },
        pos,
    ))
}

/// Parsed temporal unit: audio frames + parameter blocks for one time step.
#[derive(Debug)]
pub struct TemporalUnit {
    pub parameter_blocks: Vec<ParameterBlock>,
    pub audio_frames: Vec<AudioFrameObu>,
}

/// Parse a single temporal unit starting at the given offset.
/// Returns the temporal unit and the byte offset after it.
///
/// `parameter_kinds` dispatches parameter-block payloads by kind. Build it
/// from descriptors via [`IamfDescriptors::parameter_kinds`]; an empty map
/// degrades to MixGain-only parsing.
pub fn parse_temporal_unit_with_kinds(
    data: &[u8],
    parameter_kinds: &HashMap<u32, ParameterDataKind>,
) -> IamfResult<(TemporalUnit, usize)> {
    let mut pos = 0;
    let mut parameter_blocks = Vec::new();
    let mut audio_frames = Vec::new();
    let mut first_obu = true;

    while pos < data.len() {
        let remaining = &data[pos..];
        let (header, header_size) = match parse_obu_header(remaining) {
            Ok(h) => h,
            Err(IamfError::EndOfStream) => break,
            Err(e) => return Err(e),
        };

        // A temporal delimiter marks the start of a new temporal unit
        if header.obu_type == ObuType::TemporalDelimiter {
            if first_obu {
                // This is our delimiter — consume it and continue
                pos += header_size + header.payload_size;
                first_obu = false;
                continue;
            }
            // Next temporal unit's delimiter — stop
            break;
        }
        first_obu = false;

        let payload_start = pos + header_size;
        let payload_end = payload_start + header.payload_size;
        if payload_end > data.len() {
            break;
        }
        let payload = &data[payload_start..payload_end];

        match header.obu_type {
            ObuType::ParameterBlock => {
                if let Ok(pb) = parse_parameter_block_with_kind(payload, parameter_kinds) {
                    parameter_blocks.push(pb);
                }
            }
            ObuType::AudioFrame => {
                let mut frame_pos = 0;
                let substream_id = read_leb128_u32(payload, &mut frame_pos)?;
                audio_frames.push(AudioFrameObu {
                    substream_id,
                    samples_to_trim_start: header.trim_start,
                    samples_to_trim_end: header.trim_end,
                    payload: payload[frame_pos..].to_vec(),
                });
            }
            ObuType::AudioFrameId(id) => {
                audio_frames.push(AudioFrameObu {
                    substream_id: id as u32,
                    samples_to_trim_start: header.trim_start,
                    samples_to_trim_end: header.trim_end,
                    payload: payload.to_vec(),
                });
            }
            _ => {
                // Skip descriptor OBUs that appear in temporal units (redundant copies)
            }
        }

        pos = payload_end;
    }

    if audio_frames.is_empty() && pos >= data.len() {
        return Err(IamfError::EndOfStream);
    }

    Ok((
        TemporalUnit {
            parameter_blocks,
            audio_frames,
        },
        pos,
    ))
}

/// Legacy wrapper: parse a temporal unit with no parameter-kind hints.
pub fn parse_temporal_unit(data: &[u8]) -> IamfResult<(TemporalUnit, usize)> {
    let empty: HashMap<u32, ParameterDataKind> = HashMap::new();
    parse_temporal_unit_with_kinds(data, &empty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leb128_single_byte() {
        let (val, consumed) = read_leb128(&[42]).unwrap();
        assert_eq!(val, 42);
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_leb128_multi_byte() {
        // 300 = 0b100101100 -> leb128: [0xAC, 0x02]
        let (val, consumed) = read_leb128(&[0xAC, 0x02]).unwrap();
        assert_eq!(val, 300);
        assert_eq!(consumed, 2);
    }

    #[test]
    fn test_leb128_zero() {
        let (val, consumed) = read_leb128(&[0]).unwrap();
        assert_eq!(val, 0);
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_obu_type_from_u8() {
        assert_eq!(ObuType::from_u8(0).unwrap(), ObuType::CodecConfig);
        assert_eq!(ObuType::from_u8(1).unwrap(), ObuType::AudioElement);
        assert_eq!(ObuType::from_u8(2).unwrap(), ObuType::MixPresentation);
        assert_eq!(ObuType::from_u8(4).unwrap(), ObuType::TemporalDelimiter);
        assert_eq!(ObuType::from_u8(5).unwrap(), ObuType::AudioFrame);
        assert_eq!(ObuType::from_u8(6).unwrap(), ObuType::AudioFrameId(0));
        assert_eq!(ObuType::from_u8(31).unwrap(), ObuType::SequenceHeader);
        assert!(ObuType::from_u8(25).is_err());
    }

    #[test]
    fn test_parse_sequence_header() {
        let data = [b'i', b'a', b'm', b'f', 0, 0]; // Simple profile
        let (primary, additional) = parse_sequence_header(&data).unwrap();
        assert_eq!(primary, 0);
        assert_eq!(additional, 0);
    }

    #[test]
    fn test_parse_sequence_header_invalid_magic() {
        let data = [b'x', b'x', b'x', b'x', 0, 0];
        assert!(matches!(
            parse_sequence_header(&data),
            Err(IamfError::InvalidMagic)
        ));
    }

    #[test]
    fn test_obu_header_minimal() {
        // OBU type = 31 (SequenceHeader), no flags, size = 6
        let obu_type_byte = 31 << 3; // 0xF8
        let data = [obu_type_byte, 6, b'i', b'a', b'm', b'f', 0, 0];
        let (header, header_size) = parse_obu_header(&data).unwrap();
        assert_eq!(header.obu_type, ObuType::SequenceHeader);
        assert!(!header.redundant_copy);
        assert!(!header.trimming_status);
        assert!(!header.extension_flag);
        assert_eq!(header.payload_size, 6);
        assert_eq!(header_size, 2);
    }

    /// Build a minimal MixGain-style parameter_block payload:
    /// parameter_id (leb128) + duration (leb128) + constant_subblock_duration
    /// (leb128) + 1 subblock with animation_type=Step + start_value (Q7.8).
    fn build_param_block_payload(parameter_id: u8) -> Vec<u8> {
        vec![
            parameter_id, // parameter_id (leb128, <128 so 1 byte)
            10,           // duration
            10,           // constant_subblock_duration (==duration => 1 subblock)
            0,            // animation_type=Step (low 3 bits)
            0x10, 0x00,   // start_point_value = 0x1000 i16 BE = 4096/256 = 16.0 dB
        ]
    }

    /// DemixingInfo block carries `dmixp_mode (3) + reserved (5)` — only 1 byte
    /// of payload. If the parser silently treats it as MixGain it consumes
    /// 3 extra bytes (animation_byte + i16 start) and corrupts the gain.
    /// The fix dispatches on the parameter kind from the descriptor.
    #[test]
    fn parameter_block_demixing_info_not_silently_mix_gain() {
        // Build a demixing-info payload: parameter_id=7, duration=10, csd=10,
        // then one byte: dmixp_mode=2 (top 3 bits = 010_00000 = 0x40).
        let payload: Vec<u8> = vec![
            7,    // parameter_id
            10,   // duration
            10,   // constant_subblock_duration
            0x40, // dmixp_mode=2, reserved=0
        ];

        // With kind=DemixingInfo, the parse succeeds and emits DemixingInfo.
        let mut kinds = HashMap::new();
        kinds.insert(7u32, ParameterDataKind::DemixingInfo);
        let pb = parse_parameter_block_with_kind(&payload, &kinds)
            .expect("demixing-info parameter block must parse");
        assert_eq!(pb.parameter_id, 7);
        assert_eq!(pb.subblocks.len(), 1);
        match &pb.subblocks[0].param_data {
            ParameterData::DemixingInfo { dmixp_mode } => {
                assert_eq!(*dmixp_mode, 2, "dmixp_mode should be 2");
            }
            other => panic!("expected DemixingInfo, got {other:?}"),
        }

        // With no kind hint, the legacy parser would try to read 3 extra
        // bytes (animation + i16) — the payload is too short, so it errors
        // out instead of silently producing a garbage MixGain.
        let legacy = parse_parameter_block(&payload);
        assert!(
            legacy.is_err(),
            "DemixingInfo payload must NOT silently decode as MixGain"
        );
    }

    /// MixGain payload still parses correctly under the new dispatch.
    #[test]
    fn parameter_block_mix_gain_still_parses() {
        let payload = build_param_block_payload(3);
        let mut kinds = HashMap::new();
        kinds.insert(3u32, ParameterDataKind::MixGain);
        let pb = parse_parameter_block_with_kind(&payload, &kinds).unwrap();
        assert_eq!(pb.parameter_id, 3);
        assert_eq!(pb.subblocks.len(), 1);
        match &pb.subblocks[0].param_data {
            ParameterData::MixGain {
                start_point_value, ..
            } => {
                assert!((start_point_value - 16.0).abs() < 1e-3);
            }
            other => panic!("expected MixGain, got {other:?}"),
        }
    }

    /// ReconGain dispatch emits a typed variant rather than coercing the
    /// payload into MixGain.
    #[test]
    fn parameter_block_recon_gain_emits_typed_variant() {
        // payload: parameter_id=9, duration=10, csd=10, then 1 subblock with
        // no recon-gain bytes (our simplified parse skips them).
        let payload = vec![9u8, 10, 10];
        let mut kinds = HashMap::new();
        kinds.insert(9u32, ParameterDataKind::ReconGain);
        let pb = parse_parameter_block_with_kind(&payload, &kinds).unwrap();
        assert_eq!(pb.subblocks.len(), 1);
        assert!(matches!(
            pb.subblocks[0].param_data,
            ParameterData::ReconGain { .. }
        ));
    }

    /// `bounded_capacity` must reject leb128 counts greater than the byte
    /// ceiling, the absolute max, or both.
    #[test]
    fn bounded_capacity_rejects_unbounded_values() {
        // 100 elements but only 5 bytes remain.
        assert!(bounded_capacity(100, 5).is_err());
        // Above MAX_LEB128_CAPACITY.
        assert!(bounded_capacity(u32::MAX, 1024 * 1024 * 1024).is_err());
        // Reasonable counts pass.
        assert_eq!(bounded_capacity(10, 1024).unwrap(), 10);
        assert_eq!(bounded_capacity(0, 0).unwrap(), 0);
    }

    /// Adversarial audio_element with a huge `num_substreams` leb128 must be
    /// rejected before the allocator is asked for gigabytes.
    #[test]
    fn parse_audio_element_rejects_unbounded_leb128_substreams() {
        // audio_element_id=0, type_byte=Channel(0), codec_config_id=0,
        // then num_substreams = 0xFFFFFFFF (leb128 5-byte encoding).
        let mut payload = vec![
            0u8, // audio_element_id
            0,   // type byte (top 3 bits = element_type = 0 = Channel)
            0,   // codec_config_id
        ];
        // 0xFFFFFFFF leb128 = 0xff,0xff,0xff,0xff,0x0f
        payload.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0x0f]);
        // A few trailing bytes — far less than 4 billion.
        payload.extend_from_slice(&[0u8; 16]);

        let err = parse_audio_element(&payload)
            .expect_err("must reject 4G substreams against tiny payload");
        match err {
            IamfError::ParseError(msg) => {
                assert!(
                    msg.contains("leb128 capacity") || msg.contains("remaining"),
                    "unexpected error message: {msg}"
                );
            }
            other => panic!("expected ParseError, got {other:?}"),
        }
    }
}
