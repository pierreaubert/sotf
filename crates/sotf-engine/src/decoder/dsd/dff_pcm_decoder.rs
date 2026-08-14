use super::consts::DSD_DECODE_CHUNK_FRAMES;
use super::consts::DSD_TO_PCM_DECIMATION;
use super::decimator::{ALIGNMENT_OUTPUTS, DsdPcmDecimator, SEEK_PREROLL_FRAMES};
#[cfg(test)]
use super::parse::parse_dff;
use super::source::DsdDataSource;
use super::stream_parse::parse_dff_file;
use crate::decoder::core::{AudioDecoder, AudioSpec, DecodedAudio};
use crate::decoder::error::{AudioDecoderError, AudioDecoderResult};
use crate::decoder::formats::AudioFormat;
use std::fs::File;
use std::path::Path;

/// Uncompressed DFF/DSDIFF-to-PCM decoder used by the same PCM fallback path as DSF.
pub struct DffPcmDecoder {
    pub(super) spec: AudioSpec,
    pub(super) data: DsdDataSource,
    pub(super) channels: usize,
    pub(super) dsd_sample_count: u64,
    pub(super) source_bit_position: u64,
    pub(super) pcm_position: u64,
    pub(super) decimator: DsdPcmDecimator,
    pub(super) input_scratch: Vec<f64>,
    pub(super) pcm_scratch: Vec<f32>,
    source_byte_position: Option<u64>,
    source_byte_scratch: Vec<u8>,
}

impl DffPcmDecoder {
    pub fn new<P: AsRef<Path>>(path: P) -> AudioDecoderResult<Self> {
        let mut file = File::open(path)?;
        let fmt = parse_dff_file(&mut file)?;
        Self::from_parts(
            fmt.sample_rate,
            fmt.channels,
            fmt.sample_count,
            DsdDataSource::file(file, fmt.data_offset, fmt.data_len),
        )
    }

    #[cfg(test)]
    pub(super) fn from_bytes(bytes: Vec<u8>) -> AudioDecoderResult<Self> {
        let fmt = parse_dff(&bytes)?;
        Self::from_parts(
            fmt.sample_rate,
            fmt.channels,
            fmt.sample_count,
            DsdDataSource::memory(fmt.data),
        )
    }

    fn from_parts(
        sample_rate: u32,
        channel_count: u16,
        sample_count: u64,
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
        let offset = byte_position
            .checked_mul(self.channels as u64)
            .ok_or_else(|| {
                AudioDecoderError::DecodingFailed("DFF data offset overflow".to_string())
            })?;
        self.data
            .read_exact_at(offset, &mut self.source_byte_scratch)?;
        self.source_byte_position = Some(byte_position);
        Ok(())
    }

    fn produce_frame(&mut self) -> AudioDecoderResult<()> {
        loop {
            if self.source_bit_position < self.dsd_sample_count {
                self.load_source_bytes(self.source_bit_position / 8)?;
                let shift = 7 - (self.source_bit_position % 8) as u32;
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
            .ok_or_else(|| AudioDecoderError::SeekFailed("DFF seek offset overflow".into()))?;
        self.decimator.reset();
        self.source_byte_position = None;

        let discard = frame_position
            .checked_add(ALIGNMENT_OUTPUTS)
            .and_then(|position| position.checked_sub(start_frame))
            .ok_or_else(|| AudioDecoderError::SeekFailed("DFF seek pre-roll overflow".into()))?;
        for _ in 0..discard {
            self.produce_frame()?;
        }
        self.pcm_position = frame_position;
        Ok(())
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
                "DFF seek target {} is past end of stream {}",
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
