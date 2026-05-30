use crate::decoder::core::{AudioDecoder, AudioSpec, DecodedAudio};
use crate::decoder::error::{AudioDecoderError, AudioDecoderResult};
use crate::decoder::formats::AudioFormat;
use std::fs;
use std::path::Path;

const DSF_HEADER_LEN: usize = 12;
const DSF_ROOT_CHUNK_SIZE: usize = 28;
const DSF_FMT_CHUNK_SIZE: usize = 52;
const DFF_HEADER_LEN: usize = 12;
const DSD_TO_PCM_DECIMATION: u64 = 64;
const DSD_DECODE_CHUNK_FRAMES: u64 = 4096;

/// DSF-to-PCM decoder used when the engine is configured to decode DSD as PCM.
///
/// The decoder preserves channel layout and outputs one f32 PCM frame per 64 DSD
/// one-bit samples. This is intentionally a decoder path, not DoP/native output:
/// bitstream modes require a bit-perfect output transport that the current cpal
/// playback path does not expose.
pub struct DsfPcmDecoder {
    spec: AudioSpec,
    data: Vec<u8>,
    channels: usize,
    dsd_sample_count: u64,
    block_size_per_channel: usize,
    pcm_position: u64,
}

impl DsfPcmDecoder {
    pub fn new<P: AsRef<Path>>(path: P) -> AudioDecoderResult<Self> {
        let bytes = fs::read(path)?;
        Self::from_bytes(bytes)
    }

    fn from_bytes(bytes: Vec<u8>) -> AudioDecoderResult<Self> {
        let fmt = parse_dsf(&bytes)?;
        let pcm_sample_rate = fmt.sample_rate / DSD_TO_PCM_DECIMATION as u32;
        let total_frames = Some(fmt.sample_count / DSD_TO_PCM_DECIMATION);

        Ok(Self {
            spec: AudioSpec {
                sample_rate: pcm_sample_rate,
                channels: fmt.channels,
                bits_per_sample: 32,
                total_frames,
            },
            data: fmt.data,
            channels: fmt.channels as usize,
            dsd_sample_count: fmt.sample_count,
            block_size_per_channel: fmt.block_size_per_channel,
            pcm_position: 0,
        })
    }

    fn total_pcm_frames(&self) -> u64 {
        self.dsd_sample_count / DSD_TO_PCM_DECIMATION
    }

    fn channel_byte(&self, channel: usize, byte_index_per_channel: u64) -> u8 {
        let Ok(byte_index_per_channel) = usize::try_from(byte_index_per_channel) else {
            return 0;
        };
        let block = byte_index_per_channel / self.block_size_per_channel;
        let in_block = byte_index_per_channel % self.block_size_per_channel;
        let Some(block_stride) = self.block_size_per_channel.checked_mul(self.channels) else {
            return 0;
        };
        let Some(block_offset) = block.checked_mul(block_stride) else {
            return 0;
        };
        let Some(channel_offset) = channel.checked_mul(self.block_size_per_channel) else {
            return 0;
        };
        let Some(offset) = block_offset
            .checked_add(channel_offset)
            .and_then(|offset| offset.checked_add(in_block))
        else {
            return 0;
        };
        self.data.get(offset).copied().unwrap_or(0)
    }

    fn decode_sample(&self, channel: usize, pcm_frame: u64) -> f32 {
        let first_byte = pcm_frame * (DSD_TO_PCM_DECIMATION / 8);
        let mut ones = 0u32;
        for byte_offset in 0..(DSD_TO_PCM_DECIMATION / 8) {
            ones += self
                .channel_byte(channel, first_byte + byte_offset)
                .count_ones();
        }
        pcm_sample_from_ones(ones)
    }
}

impl AudioDecoder for DsfPcmDecoder {
    fn spec(&self) -> &AudioSpec {
        &self.spec
    }

    fn format(&self) -> AudioFormat {
        AudioFormat::DsdDsf
    }

    fn decode_into(&mut self, dest: &mut DecodedAudio) -> AudioDecoderResult<usize> {
        dest.clear();
        dest.spec = self.spec.clone();
        dest.frame_position = self.pcm_position;

        let remaining = self.total_pcm_frames().saturating_sub(self.pcm_position);
        if remaining == 0 {
            return Ok(0);
        }

        let frames = remaining.min(DSD_DECODE_CHUNK_FRAMES);
        let sample_len = usize::try_from(frames)
            .ok()
            .and_then(|frames| frames.checked_mul(self.channels))
            .ok_or_else(|| {
                AudioDecoderError::DecodingFailed(
                    "DSF decode chunk is too large to address".to_string(),
                )
            })?;

        dest.samples.resize(sample_len, 0.0);
        for frame_offset in 0..frames {
            let pcm_frame = self.pcm_position + frame_offset;
            let dst = frame_offset as usize * self.channels;
            for channel in 0..self.channels {
                dest.samples[dst + channel] = self.decode_sample(channel, pcm_frame);
            }
        }

        self.pcm_position += frames;
        Ok(frames as usize)
    }

    fn seek(&mut self, frame_position: u64) -> AudioDecoderResult<()> {
        if frame_position > self.total_pcm_frames() {
            return Err(AudioDecoderError::SeekFailed(format!(
                "DSF seek target {} is past end of stream {}",
                frame_position,
                self.total_pcm_frames()
            )));
        }
        self.pcm_position = frame_position;
        Ok(())
    }

    fn position(&self) -> u64 {
        self.pcm_position
    }

    fn is_eof(&self) -> bool {
        self.pcm_position >= self.total_pcm_frames()
    }
}

/// Uncompressed DFF/DSDIFF-to-PCM decoder used by the same PCM fallback path as DSF.
pub struct DffPcmDecoder {
    spec: AudioSpec,
    data: Vec<u8>,
    channels: usize,
    dsd_sample_count: u64,
    pcm_position: u64,
}

impl DffPcmDecoder {
    pub fn new<P: AsRef<Path>>(path: P) -> AudioDecoderResult<Self> {
        let bytes = fs::read(path)?;
        Self::from_bytes(bytes)
    }

    fn from_bytes(bytes: Vec<u8>) -> AudioDecoderResult<Self> {
        let fmt = parse_dff(&bytes)?;
        let pcm_sample_rate = fmt.sample_rate / DSD_TO_PCM_DECIMATION as u32;
        let total_frames = Some(fmt.sample_count / DSD_TO_PCM_DECIMATION);

        Ok(Self {
            spec: AudioSpec {
                sample_rate: pcm_sample_rate,
                channels: fmt.channels,
                bits_per_sample: 32,
                total_frames,
            },
            data: fmt.data,
            channels: fmt.channels as usize,
            dsd_sample_count: fmt.sample_count,
            pcm_position: 0,
        })
    }

    fn total_pcm_frames(&self) -> u64 {
        self.dsd_sample_count / DSD_TO_PCM_DECIMATION
    }

    fn channel_byte(&self, channel: usize, byte_index_per_channel: u64) -> u8 {
        let Ok(byte_index_per_channel) = usize::try_from(byte_index_per_channel) else {
            return 0;
        };
        let Some(frame_offset) = byte_index_per_channel.checked_mul(self.channels) else {
            return 0;
        };
        let Some(offset) = frame_offset.checked_add(channel) else {
            return 0;
        };
        self.data.get(offset).copied().unwrap_or(0)
    }

    fn decode_sample(&self, channel: usize, pcm_frame: u64) -> f32 {
        let first_byte = pcm_frame * (DSD_TO_PCM_DECIMATION / 8);
        let mut ones = 0u32;
        for byte_offset in 0..(DSD_TO_PCM_DECIMATION / 8) {
            ones += self
                .channel_byte(channel, first_byte + byte_offset)
                .count_ones();
        }
        pcm_sample_from_ones(ones)
    }
}

impl AudioDecoder for DffPcmDecoder {
    fn spec(&self) -> &AudioSpec {
        &self.spec
    }

    fn format(&self) -> AudioFormat {
        AudioFormat::DsdDff
    }

    fn decode_into(&mut self, dest: &mut DecodedAudio) -> AudioDecoderResult<usize> {
        dest.clear();
        dest.spec = self.spec.clone();
        dest.frame_position = self.pcm_position;

        let remaining = self.total_pcm_frames().saturating_sub(self.pcm_position);
        if remaining == 0 {
            return Ok(0);
        }

        let frames = remaining.min(DSD_DECODE_CHUNK_FRAMES);
        let sample_len = usize::try_from(frames)
            .ok()
            .and_then(|frames| frames.checked_mul(self.channels))
            .ok_or_else(|| {
                AudioDecoderError::DecodingFailed(
                    "DFF decode chunk is too large to address".to_string(),
                )
            })?;

        dest.samples.resize(sample_len, 0.0);
        for frame_offset in 0..frames {
            let pcm_frame = self.pcm_position + frame_offset;
            let dst = frame_offset as usize * self.channels;
            for channel in 0..self.channels {
                dest.samples[dst + channel] = self.decode_sample(channel, pcm_frame);
            }
        }

        self.pcm_position += frames;
        Ok(frames as usize)
    }

    fn seek(&mut self, frame_position: u64) -> AudioDecoderResult<()> {
        if frame_position > self.total_pcm_frames() {
            return Err(AudioDecoderError::SeekFailed(format!(
                "DFF seek target {} is past end of stream {}",
                frame_position,
                self.total_pcm_frames()
            )));
        }
        self.pcm_position = frame_position;
        Ok(())
    }

    fn position(&self) -> u64 {
        self.pcm_position
    }

    fn is_eof(&self) -> bool {
        self.pcm_position >= self.total_pcm_frames()
    }
}

fn pcm_sample_from_ones(ones: u32) -> f32 {
    ((ones as f32 * 2.0) - DSD_TO_PCM_DECIMATION as f32) / DSD_TO_PCM_DECIMATION as f32
}

struct ParsedDsf {
    sample_rate: u32,
    channels: u16,
    sample_count: u64,
    block_size_per_channel: usize,
    data: Vec<u8>,
}

struct ParsedDff {
    sample_rate: u32,
    channels: u16,
    sample_count: u64,
    data: Vec<u8>,
}

fn parse_dsf(bytes: &[u8]) -> AudioDecoderResult<ParsedDsf> {
    if bytes.len() < DSF_ROOT_CHUNK_SIZE || &bytes[0..4] != b"DSD " {
        return Err(AudioDecoderError::InvalidFile(
            "DSF file must start with a DSD chunk".to_string(),
        ));
    }

    let root_size = read_u64_le(bytes, 4)?;
    if root_size != DSF_ROOT_CHUNK_SIZE as u64 {
        return Err(AudioDecoderError::InvalidFile(format!(
            "Unexpected DSF root chunk size {}",
            root_size
        )));
    }

    let mut offset = DSF_ROOT_CHUNK_SIZE;
    let mut sample_rate = None;
    let mut channels = None;
    let mut sample_count = None;
    let mut block_size_per_channel = None;
    let mut data = None;

    while offset + DSF_HEADER_LEN <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let chunk_size = checked_chunk_size(read_u64_le(bytes, offset + 4)?)?;
        if chunk_size < DSF_HEADER_LEN {
            return Err(AudioDecoderError::InvalidFile(format!(
                "DSF chunk {:?} has invalid size {}",
                String::from_utf8_lossy(id),
                chunk_size
            )));
        }
        let chunk_end = offset
            .checked_add(chunk_size)
            .ok_or_else(|| AudioDecoderError::InvalidFile("DSF chunk offset overflow".into()))?;
        if chunk_end > bytes.len() {
            return Err(AudioDecoderError::InvalidFile(format!(
                "DSF chunk {:?} extends past end of file",
                String::from_utf8_lossy(id)
            )));
        }
        let payload_start = offset + DSF_HEADER_LEN;

        match id {
            b"fmt " => {
                if chunk_size < DSF_FMT_CHUNK_SIZE {
                    return Err(AudioDecoderError::InvalidFile(format!(
                        "DSF fmt chunk too small: {}",
                        chunk_size
                    )));
                }
                let format_id = read_u32_le(bytes, payload_start + 4)?;
                if format_id != 0 {
                    return Err(AudioDecoderError::UnsupportedFormat(format!(
                        "Unsupported DSF format id {}",
                        format_id
                    )));
                }
                let channel_count = read_u32_le(bytes, payload_start + 12)?;
                let bits_per_sample = read_u32_le(bytes, payload_start + 20)?;
                if !matches!(bits_per_sample, 1 | 8) {
                    return Err(AudioDecoderError::UnsupportedFormat(format!(
                        "Unsupported DSF bits-per-sample {}",
                        bits_per_sample
                    )));
                }

                sample_rate = Some(read_u32_le(bytes, payload_start + 16)?);
                channels = Some(u16::try_from(channel_count).map_err(|_| {
                    AudioDecoderError::UnsupportedFormat(format!(
                        "Unsupported DSF channel count {}",
                        channel_count
                    ))
                })?);
                sample_count = Some(read_u64_le(bytes, payload_start + 24)?);
                block_size_per_channel = Some(
                    usize::try_from(read_u32_le(bytes, payload_start + 32)?).map_err(|_| {
                        AudioDecoderError::UnsupportedFormat(
                            "DSF block size is too large".to_string(),
                        )
                    })?,
                );
            }
            b"data" => {
                data = Some(bytes[payload_start..chunk_end].to_vec());
            }
            _ => {}
        }

        offset = chunk_end;
    }

    let sample_rate = sample_rate
        .ok_or_else(|| AudioDecoderError::InvalidFile("Missing DSF fmt chunk".into()))?;
    let channels =
        channels.ok_or_else(|| AudioDecoderError::InvalidFile("Missing DSF channels".into()))?;
    let sample_count = sample_count
        .ok_or_else(|| AudioDecoderError::InvalidFile("Missing DSF sample count".into()))?;
    let block_size_per_channel = block_size_per_channel
        .ok_or_else(|| AudioDecoderError::InvalidFile("Missing DSF block size".into()))?;
    let data =
        data.ok_or_else(|| AudioDecoderError::InvalidFile("Missing DSF data chunk".into()))?;

    if channels == 0 {
        return Err(AudioDecoderError::InvalidFile(
            "DSF file has zero channels".to_string(),
        ));
    }
    if sample_rate < DSD_TO_PCM_DECIMATION as u32 || sample_rate % DSD_TO_PCM_DECIMATION as u32 != 0
    {
        return Err(AudioDecoderError::UnsupportedFormat(format!(
            "Unsupported DSF sample rate {}",
            sample_rate
        )));
    }
    if block_size_per_channel == 0 {
        return Err(AudioDecoderError::InvalidFile(
            "DSF file has zero block size".to_string(),
        ));
    }

    Ok(ParsedDsf {
        sample_rate,
        channels,
        sample_count,
        block_size_per_channel,
        data,
    })
}

fn parse_dff(bytes: &[u8]) -> AudioDecoderResult<ParsedDff> {
    if bytes.len() < 16 || &bytes[0..4] != b"FRM8" || &bytes[12..16] != b"DSD " {
        return Err(AudioDecoderError::InvalidFile(
            "DFF file must start with an FRM8 DSD form".to_string(),
        ));
    }

    let form_size = checked_chunk_size(read_u64_be(bytes, 4)?)?;
    let form_end = 12usize
        .checked_add(form_size)
        .ok_or_else(|| AudioDecoderError::InvalidFile("DFF form offset overflow".into()))?
        .min(bytes.len());
    let mut offset = 16;
    let mut sample_rate = None;
    let mut channels = None;
    let mut compression = None;
    let mut data = None;

    while offset + DFF_HEADER_LEN <= form_end {
        let id = &bytes[offset..offset + 4];
        let payload_size = checked_chunk_size(read_u64_be(bytes, offset + 4)?)?;
        let payload_start = offset + DFF_HEADER_LEN;
        let payload_end = payload_start.checked_add(payload_size).ok_or_else(|| {
            AudioDecoderError::InvalidFile("DFF chunk offset overflow".to_string())
        })?;
        if payload_end > form_end {
            return Err(AudioDecoderError::InvalidFile(format!(
                "DFF chunk {:?} extends past end of form",
                String::from_utf8_lossy(id)
            )));
        }

        match id {
            b"PROP" => {
                let props = parse_dff_sound_properties(&bytes[payload_start..payload_end])?;
                sample_rate = props.sample_rate.or(sample_rate);
                channels = props.channels.or(channels);
                compression = props.compression.or(compression);
            }
            b"DSD " => {
                data = Some(bytes[payload_start..payload_end].to_vec());
            }
            _ => {}
        }

        offset = payload_end + (payload_size & 1);
    }

    let sample_rate = sample_rate
        .ok_or_else(|| AudioDecoderError::InvalidFile("Missing DFF sample rate".into()))?;
    let channels =
        channels.ok_or_else(|| AudioDecoderError::InvalidFile("Missing DFF channels".into()))?;
    let compression = compression
        .ok_or_else(|| AudioDecoderError::InvalidFile("Missing DFF compression".into()))?;
    let data =
        data.ok_or_else(|| AudioDecoderError::InvalidFile("Missing DFF DSD data chunk".into()))?;

    if compression != *b"DSD " {
        return Err(AudioDecoderError::UnsupportedFormat(format!(
            "Unsupported DFF compression {}",
            String::from_utf8_lossy(&compression)
        )));
    }
    if channels == 0 {
        return Err(AudioDecoderError::InvalidFile(
            "DFF file has zero channels".to_string(),
        ));
    }
    if sample_rate < DSD_TO_PCM_DECIMATION as u32 || sample_rate % DSD_TO_PCM_DECIMATION as u32 != 0
    {
        return Err(AudioDecoderError::UnsupportedFormat(format!(
            "Unsupported DFF sample rate {}",
            sample_rate
        )));
    }

    let sample_count = (data.len() as u64 * 8) / channels as u64;
    Ok(ParsedDff {
        sample_rate,
        channels,
        sample_count,
        data,
    })
}

struct DffSoundProperties {
    sample_rate: Option<u32>,
    channels: Option<u16>,
    compression: Option<[u8; 4]>,
}

fn parse_dff_sound_properties(payload: &[u8]) -> AudioDecoderResult<DffSoundProperties> {
    if payload.len() < 4 || &payload[0..4] != b"SND " {
        return Ok(DffSoundProperties {
            sample_rate: None,
            channels: None,
            compression: None,
        });
    }

    let mut offset = 4;
    let mut props = DffSoundProperties {
        sample_rate: None,
        channels: None,
        compression: None,
    };

    while offset + DFF_HEADER_LEN <= payload.len() {
        let id = &payload[offset..offset + 4];
        let payload_size = checked_chunk_size(read_u64_be(payload, offset + 4)?)?;
        let sub_payload_start = offset + DFF_HEADER_LEN;
        let sub_payload_end = sub_payload_start.checked_add(payload_size).ok_or_else(|| {
            AudioDecoderError::InvalidFile("DFF PROP chunk offset overflow".to_string())
        })?;
        if sub_payload_end > payload.len() {
            return Err(AudioDecoderError::InvalidFile(format!(
                "DFF PROP subchunk {:?} extends past end of chunk",
                String::from_utf8_lossy(id)
            )));
        }

        match id {
            b"FS  " if payload_size >= 4 => {
                props.sample_rate = Some(read_u32_be(payload, sub_payload_start)?);
            }
            b"CHNL" if payload_size >= 2 => {
                props.channels = Some(read_u16_be(payload, sub_payload_start)?);
            }
            b"CMPR" if payload_size >= 4 => {
                let compression = payload[sub_payload_start..sub_payload_start + 4]
                    .try_into()
                    .unwrap();
                props.compression = Some(compression);
            }
            _ => {}
        }

        offset = sub_payload_end + (payload_size & 1);
    }

    Ok(props)
}

fn checked_chunk_size(size: u64) -> AudioDecoderResult<usize> {
    usize::try_from(size).map_err(|_| {
        AudioDecoderError::InvalidFile(format!("DSF chunk size {} is too large", size))
    })
}

fn read_u16_be(bytes: &[u8], offset: usize) -> AudioDecoderResult<u16> {
    let slice = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| AudioDecoderError::InvalidFile("Unexpected end of DFF file".to_string()))?;
    Ok(u16::from_be_bytes(slice.try_into().unwrap()))
}

fn read_u32_be(bytes: &[u8], offset: usize) -> AudioDecoderResult<u32> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| AudioDecoderError::InvalidFile("Unexpected end of DFF file".to_string()))?;
    Ok(u32::from_be_bytes(slice.try_into().unwrap()))
}

fn read_u64_be(bytes: &[u8], offset: usize) -> AudioDecoderResult<u64> {
    let slice = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| AudioDecoderError::InvalidFile("Unexpected end of DFF file".to_string()))?;
    Ok(u64::from_be_bytes(slice.try_into().unwrap()))
}

fn read_u32_le(bytes: &[u8], offset: usize) -> AudioDecoderResult<u32> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| AudioDecoderError::InvalidFile("Unexpected end of DSF file".to_string()))?;
    Ok(u32::from_le_bytes(slice.try_into().unwrap()))
}

fn read_u64_le(bytes: &[u8], offset: usize) -> AudioDecoderResult<u64> {
    let slice = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| AudioDecoderError::InvalidFile("Unexpected end of DSF file".to_string()))?;
    Ok(u64::from_le_bytes(slice.try_into().unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(DSF_HEADER_LEN + payload.len());
        bytes.extend_from_slice(id);
        bytes.extend_from_slice(&(DSF_HEADER_LEN as u64 + payload.len() as u64).to_le_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }

    fn chunk_be(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(DFF_HEADER_LEN + payload.len() + (payload.len() & 1));
        bytes.extend_from_slice(id);
        bytes.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        bytes.extend_from_slice(payload);
        if payload.len() % 2 != 0 {
            bytes.push(0);
        }
        bytes
    }

    fn minimal_dsf(channels: u32, sample_count: u64, block_size: u32, payload: &[u8]) -> Vec<u8> {
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&1u32.to_le_bytes());
        fmt.extend_from_slice(&0u32.to_le_bytes());
        fmt.extend_from_slice(&2u32.to_le_bytes());
        fmt.extend_from_slice(&channels.to_le_bytes());
        fmt.extend_from_slice(&2_822_400u32.to_le_bytes());
        fmt.extend_from_slice(&1u32.to_le_bytes());
        fmt.extend_from_slice(&sample_count.to_le_bytes());
        fmt.extend_from_slice(&block_size.to_le_bytes());
        fmt.extend_from_slice(&0u32.to_le_bytes());

        let fmt_chunk = chunk(b"fmt ", &fmt);
        let data_chunk = chunk(b"data", payload);
        let file_size = DSF_ROOT_CHUNK_SIZE + fmt_chunk.len() + data_chunk.len();

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"DSD ");
        bytes.extend_from_slice(&(DSF_ROOT_CHUNK_SIZE as u64).to_le_bytes());
        bytes.extend_from_slice(&(file_size as u64).to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.extend_from_slice(&fmt_chunk);
        bytes.extend_from_slice(&data_chunk);
        bytes
    }

    fn minimal_dff(channels: u16, payload: &[u8]) -> Vec<u8> {
        let fs = chunk_be(b"FS  ", &2_822_400u32.to_be_bytes());

        let mut chnl = Vec::new();
        chnl.extend_from_slice(&channels.to_be_bytes());
        for channel in 0..channels {
            let id = if channel == 0 {
                *b"SLFT"
            } else if channel == 1 {
                *b"SRGT"
            } else {
                *b"C___"
            };
            chnl.extend_from_slice(&id);
        }
        let chnl = chunk_be(b"CHNL", &chnl);

        let mut cmpr = Vec::new();
        cmpr.extend_from_slice(b"DSD ");
        cmpr.push(0);
        let cmpr = chunk_be(b"CMPR", &cmpr);

        let mut prop_payload = Vec::new();
        prop_payload.extend_from_slice(b"SND ");
        prop_payload.extend_from_slice(&fs);
        prop_payload.extend_from_slice(&chnl);
        prop_payload.extend_from_slice(&cmpr);
        let prop = chunk_be(b"PROP", &prop_payload);
        let dsd = chunk_be(b"DSD ", payload);
        let form_size = 4 + prop.len() + dsd.len();

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"FRM8");
        bytes.extend_from_slice(&(form_size as u64).to_be_bytes());
        bytes.extend_from_slice(b"DSD ");
        bytes.extend_from_slice(&prop);
        bytes.extend_from_slice(&dsd);
        bytes
    }

    #[test]
    fn dsf_pcm_decoder_converts_one_bit_samples_to_pcm_frames() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&[0xff; 8]);
        payload.extend_from_slice(&[0x00; 8]);
        let bytes = minimal_dsf(2, 64, 8, &payload);
        let mut decoder = DsfPcmDecoder::from_bytes(bytes).unwrap();
        let mut dest = DecodedAudio::new(decoder.spec().clone());

        let frames = decoder.decode_into(&mut dest).unwrap();

        assert_eq!(frames, 1);
        assert_eq!(dest.spec.sample_rate, 44_100);
        assert_eq!(dest.spec.channels, 2);
        assert_eq!(dest.samples, vec![1.0, -1.0]);
        assert_eq!(decoder.position(), 1);
    }

    #[test]
    fn dsf_pcm_decoder_seek_and_eof_are_pcm_frame_based() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&[0xff; 16]);
        let bytes = minimal_dsf(1, 128, 16, &payload);
        let mut decoder = DsfPcmDecoder::from_bytes(bytes).unwrap();
        let mut dest = DecodedAudio::new(decoder.spec().clone());

        decoder.seek(1).unwrap();
        let frames = decoder.decode_into(&mut dest).unwrap();
        assert_eq!(frames, 1);
        assert_eq!(dest.frame_position, 1);
        assert_eq!(dest.samples, vec![1.0]);
        assert!(decoder.is_eof());
        assert!(decoder.seek(3).is_err());
    }

    #[test]
    fn dff_pcm_decoder_converts_uncompressed_interleaved_dsd_bytes() {
        let mut payload = Vec::new();
        for _ in 0..8 {
            payload.push(0xff);
            payload.push(0x00);
        }
        let bytes = minimal_dff(2, &payload);
        let mut decoder = DffPcmDecoder::from_bytes(bytes).unwrap();
        let mut dest = DecodedAudio::new(decoder.spec().clone());

        let frames = decoder.decode_into(&mut dest).unwrap();

        assert_eq!(frames, 1);
        assert_eq!(dest.spec.sample_rate, 44_100);
        assert_eq!(dest.spec.channels, 2);
        assert_eq!(dest.samples, vec![1.0, -1.0]);
        assert_eq!(decoder.format(), AudioFormat::DsdDff);
    }

    #[test]
    fn dff_pcm_decoder_rejects_dst_compression() {
        let mut bytes = minimal_dff(1, &[0xff; 8]);
        let pos = bytes
            .windows(4)
            .position(|window| window == b"CMPR")
            .expect("compression marker should exist");
        bytes[pos + DFF_HEADER_LEN..pos + DFF_HEADER_LEN + 4].copy_from_slice(b"DST ");

        let err = match DffPcmDecoder::from_bytes(bytes) {
            Ok(_) => panic!("DST-compressed DFF should be rejected"),
            Err(err) => err,
        };
        assert!(
            matches!(err, AudioDecoderError::UnsupportedFormat(message) if message.contains("DST"))
        );
    }
}
