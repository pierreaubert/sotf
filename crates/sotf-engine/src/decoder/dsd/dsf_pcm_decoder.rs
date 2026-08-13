use super::consts::DSD_DECODE_CHUNK_FRAMES;
use super::consts::DSD_TO_PCM_DECIMATION;
use super::decimator::{ALIGNMENT_OUTPUTS, DsdPcmDecimator, SEEK_PREROLL_FRAMES};
#[cfg(test)]
use super::parse::parse_dsf;
use super::source::DsdDataSource;
use super::stream_parse::parse_dsf_file;
use crate::decoder::core::{AudioDecoder, AudioSpec, DecodedAudio};
use crate::decoder::error::{AudioDecoderError, AudioDecoderResult};
use crate::decoder::formats::AudioFormat;
use std::fs::File;
use std::path::Path;

/// DSF-to-PCM decoder used when the engine is configured to decode DSD as PCM.
///
/// The decoder preserves channel layout and outputs one f32 PCM frame per 64 DSD
/// one-bit samples. This is intentionally a decoder path, not DoP/native output:
/// bitstream modes require a bit-perfect output transport that the current cpal
/// playback path does not expose.
pub struct DsfPcmDecoder {
    pub(super) spec: AudioSpec,
    pub(super) data: DsdDataSource,
    pub(super) channels: usize,
    pub(super) dsd_sample_count: u64,
    pub(super) block_size_per_channel: usize,
    pub(super) lsb_first: bool,
    pub(super) source_bit_position: u64,
    pub(super) pcm_position: u64,
    pub(super) decimator: DsdPcmDecimator,
    pub(super) input_scratch: Vec<f64>,
    pub(super) pcm_scratch: Vec<f32>,
    source_byte_position: Option<u64>,
    source_byte_scratch: Vec<u8>,
}

impl DsfPcmDecoder {
    pub fn new<P: AsRef<Path>>(path: P) -> AudioDecoderResult<Self> {
        let mut file = File::open(path)?;
        let fmt = parse_dsf_file(&mut file)?;
        Self::from_parts(
            fmt.sample_rate,
            fmt.channels,
            fmt.sample_count,
            fmt.block_size_per_channel,
            fmt.lsb_first,
            DsdDataSource::file(file, fmt.data_offset, fmt.data_len),
        )
    }

    #[cfg(test)]
    pub(super) fn from_bytes(bytes: Vec<u8>) -> AudioDecoderResult<Self> {
        let fmt = parse_dsf(&bytes)?;
        Self::from_parts(
            fmt.sample_rate,
            fmt.channels,
            fmt.sample_count,
            fmt.block_size_per_channel,
            fmt.lsb_first,
            DsdDataSource::memory(fmt.data),
        )
    }

    fn from_parts(
        sample_rate: u32,
        channel_count: u16,
        sample_count: u64,
        block_size_per_channel: usize,
        lsb_first: bool,
        data: DsdDataSource,
    ) -> AudioDecoderResult<Self> {
        let pcm_sample_rate = sample_rate / DSD_TO_PCM_DECIMATION as u32;
        let total_frames = Some(sample_count / DSD_TO_PCM_DECIMATION);

        let channels = channel_count as usize;
        let mut decoder = Self {
            spec: AudioSpec {
                sample_rate: pcm_sample_rate,
                channels: channel_count,
                bits_per_sample: 32,
                total_frames,
            },
            data,
            channels,
            dsd_sample_count: sample_count,
            block_size_per_channel,
            lsb_first,
            source_bit_position: 0,
            pcm_position: 0,
            decimator: DsdPcmDecimator::new(channels),
            input_scratch: vec![0.0; channels],
            pcm_scratch: vec![0.0; channels],
            source_byte_position: None,
            source_byte_scratch: vec![0; channels],
        };
        decoder.prepare_position(0)?;
        Ok(decoder)
    }

    pub(super) fn total_pcm_frames(&self) -> u64 {
        self.dsd_sample_count / DSD_TO_PCM_DECIMATION
    }

    fn load_source_bytes(&mut self, byte_position: u64) -> AudioDecoderResult<()> {
        if self.source_byte_position == Some(byte_position) {
            return Ok(());
        }
        let byte_index_per_channel = usize::try_from(byte_position).map_err(|_| {
            AudioDecoderError::DecodingFailed("DSF byte position is too large".to_string())
        })?;
        let block = byte_index_per_channel / self.block_size_per_channel;
        let in_block = byte_index_per_channel % self.block_size_per_channel;
        for channel in 0..self.channels {
            let offset = block
                .checked_mul(self.block_size_per_channel.saturating_mul(self.channels))
                .and_then(|offset| {
                    offset.checked_add(channel.saturating_mul(self.block_size_per_channel))
                })
                .and_then(|offset| offset.checked_add(in_block))
                .ok_or_else(|| {
                    AudioDecoderError::DecodingFailed("DSF data offset overflow".to_string())
                })?;
            self.data.read_exact_at(
                u64::try_from(offset).map_err(|_| {
                    AudioDecoderError::DecodingFailed("DSF data offset is too large".to_string())
                })?,
                &mut self.source_byte_scratch[channel..=channel],
            )?;
        }
        self.source_byte_position = Some(byte_position);
        Ok(())
    }

    fn produce_frame(&mut self) -> AudioDecoderResult<()> {
        loop {
            if self.source_bit_position < self.dsd_sample_count {
                let byte_position = self.source_bit_position / 8;
                self.load_source_bytes(byte_position)?;
                let bit = (self.source_bit_position % 8) as u32;
                let shift = if self.lsb_first { bit } else { 7 - bit };
                for channel in 0..self.channels {
                    self.input_scratch[channel] =
                        if self.source_byte_scratch[channel] & (1 << shift) != 0 {
                            1.0
                        } else {
                            -1.0
                        };
                }
            } else {
                let padding = if self.source_bit_position.is_multiple_of(2) {
                    1.0
                } else {
                    -1.0
                };
                self.input_scratch.fill(padding);
            }
            self.source_bit_position = self.source_bit_position.saturating_add(1);
            if let Some(frame) = self.decimator.push(&self.input_scratch) {
                self.pcm_scratch.copy_from_slice(frame);
                return Ok(());
            }
        }
    }

    fn prepare_position(&mut self, frame_position: u64) -> AudioDecoderResult<()> {
        let start_frame = frame_position.saturating_sub(SEEK_PREROLL_FRAMES);
        self.source_bit_position = start_frame
            .checked_mul(DSD_TO_PCM_DECIMATION)
            .ok_or_else(|| AudioDecoderError::SeekFailed("DSF seek offset overflow".into()))?;
        self.decimator.reset();
        self.source_byte_position = None;

        let discard = frame_position
            .checked_add(ALIGNMENT_OUTPUTS)
            .and_then(|position| position.checked_sub(start_frame))
            .ok_or_else(|| AudioDecoderError::SeekFailed("DSF seek pre-roll overflow".into()))?;
        for _ in 0..discard {
            self.produce_frame()?;
        }
        self.pcm_position = frame_position;
        Ok(())
    }
}

#[cfg(test)]
pub(super) fn dsf_sample(
    data: &[u8],
    channels: usize,
    block_size_per_channel: usize,
    lsb_first: bool,
    channel: usize,
    bit_position: u64,
    sample_count: u64,
) -> f64 {
    if bit_position >= sample_count {
        return if bit_position.is_multiple_of(2) {
            1.0
        } else {
            -1.0
        };
    }
    let byte_position = bit_position / 8;
    let Ok(byte_index_per_channel) = usize::try_from(byte_position) else {
        return 0.0;
    };
    let block = byte_index_per_channel / block_size_per_channel;
    let in_block = byte_index_per_channel % block_size_per_channel;
    let Some(offset) = block
        .checked_mul(block_size_per_channel.saturating_mul(channels))
        .and_then(|offset| offset.checked_add(channel.saturating_mul(block_size_per_channel)))
        .and_then(|offset| offset.checked_add(in_block))
    else {
        return 0.0;
    };
    let byte = data.get(offset).copied().unwrap_or(0);
    let bit = (bit_position % 8) as u32;
    let shift = if lsb_first { bit } else { 7 - bit };
    if byte & (1 << shift) != 0 { 1.0 } else { -1.0 }
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
            let dst = frame_offset as usize * self.channels;
            self.produce_frame()?;
            dest.samples[dst..dst + self.channels].copy_from_slice(&self.pcm_scratch);
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
        self.prepare_position(frame_position)
    }

    fn position(&self) -> u64 {
        self.pcm_position
    }

    fn is_eof(&self) -> bool {
        self.pcm_position >= self.total_pcm_frames()
    }
}
