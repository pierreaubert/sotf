use super::consts::DSD_DECODE_CHUNK_FRAMES;
use super::consts::DSD_TO_PCM_DECIMATION;
use super::decimator::{ALIGNMENT_OUTPUTS, DsdPcmDecimator, SEEK_PREROLL_FRAMES};
use super::parse::parse_dff;
use crate::decoder::core::{AudioDecoder, AudioSpec, DecodedAudio};
use crate::decoder::error::{AudioDecoderError, AudioDecoderResult};
use crate::decoder::formats::AudioFormat;
use std::fs;
use std::path::Path;

/// Uncompressed DFF/DSDIFF-to-PCM decoder used by the same PCM fallback path as DSF.
pub struct DffPcmDecoder {
    pub(super) spec: AudioSpec,
    pub(super) data: Vec<u8>,
    pub(super) channels: usize,
    pub(super) dsd_sample_count: u64,
    pub(super) source_bit_position: u64,
    pub(super) pcm_position: u64,
    pub(super) decimator: DsdPcmDecimator,
    pub(super) input_scratch: Vec<f64>,
    pub(super) pcm_scratch: Vec<f32>,
}

impl DffPcmDecoder {
    pub fn new<P: AsRef<Path>>(path: P) -> AudioDecoderResult<Self> {
        let bytes = fs::read(path)?;
        Self::from_bytes(bytes)
    }

    pub(super) fn from_bytes(bytes: Vec<u8>) -> AudioDecoderResult<Self> {
        let fmt = parse_dff(&bytes)?;
        let pcm_sample_rate = fmt.sample_rate / DSD_TO_PCM_DECIMATION as u32;
        let total_frames = Some(fmt.sample_count / DSD_TO_PCM_DECIMATION);

        let channels = fmt.channels as usize;
        let mut decoder = Self {
            spec: AudioSpec {
                sample_rate: pcm_sample_rate,
                channels: fmt.channels,
                bits_per_sample: 32,
                total_frames,
            },
            data: fmt.data,
            channels,
            dsd_sample_count: fmt.sample_count,
            source_bit_position: 0,
            pcm_position: 0,
            decimator: DsdPcmDecimator::new(channels),
            input_scratch: vec![0.0; channels],
            pcm_scratch: vec![0.0; channels],
        };
        decoder.prepare_position(0)?;
        Ok(decoder)
    }

    pub(super) fn total_pcm_frames(&self) -> u64 {
        self.dsd_sample_count / DSD_TO_PCM_DECIMATION
    }

    fn channel_sample(&self, channel: usize, bit_position: u64) -> f64 {
        dff_sample(
            &self.data,
            self.channels,
            channel,
            bit_position,
            self.dsd_sample_count,
        )
    }

    fn produce_frame(&mut self) {
        loop {
            for channel in 0..self.channels {
                self.input_scratch[channel] =
                    self.channel_sample(channel, self.source_bit_position);
            }
            self.source_bit_position = self.source_bit_position.saturating_add(1);
            if let Some(frame) = self.decimator.push(&self.input_scratch) {
                self.pcm_scratch.copy_from_slice(frame);
                return;
            }
        }
    }

    fn prepare_position(&mut self, frame_position: u64) -> AudioDecoderResult<()> {
        let start_frame = frame_position.saturating_sub(SEEK_PREROLL_FRAMES);
        self.source_bit_position = start_frame
            .checked_mul(DSD_TO_PCM_DECIMATION)
            .ok_or_else(|| AudioDecoderError::SeekFailed("DFF seek offset overflow".into()))?;
        self.decimator.reset();

        let discard = frame_position
            .checked_add(ALIGNMENT_OUTPUTS)
            .and_then(|position| position.checked_sub(start_frame))
            .ok_or_else(|| AudioDecoderError::SeekFailed("DFF seek pre-roll overflow".into()))?;
        for _ in 0..discard {
            self.produce_frame();
        }
        self.pcm_position = frame_position;
        Ok(())
    }
}

fn dff_sample(
    data: &[u8],
    channels: usize,
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
    let Ok(byte_position) = usize::try_from(bit_position / 8) else {
        return 0.0;
    };
    let Some(offset) = byte_position
        .checked_mul(channels)
        .and_then(|offset| offset.checked_add(channel))
    else {
        return 0.0;
    };
    let byte = data.get(offset).copied().unwrap_or(0);
    let shift = 7 - (bit_position % 8) as u32;
    if byte & (1 << shift) != 0 { 1.0 } else { -1.0 }
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
            self.produce_frame();
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
