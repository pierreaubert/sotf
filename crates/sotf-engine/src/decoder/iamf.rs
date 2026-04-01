// ============================================================================
// IAMF Audio Decoder - Engine Integration
// ============================================================================
//
// Adapter between sotf_iamf::IamfDecoder and the engine's AudioDecoder trait.
// The IAMF decoder handles demuxing, codec decoding, and spatial rendering
// internally; this wrapper just provides the AudioDecoder interface.

use crate::decoder::core::{AudioDecoder, AudioSpec, DecodedAudio};
use crate::decoder::error::{AudioDecoderError, AudioDecoderResult};
use crate::decoder::formats::AudioFormat;

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// IAMF audio decoder wrapping the pure Rust sotf-iamf crate.
pub struct IamfAudioDecoder {
    decoder: sotf_iamf::IamfDecoder,
    spec: AudioSpec,
    eof: bool,
}

impl IamfAudioDecoder {
    pub fn new<P: AsRef<Path>>(path: P) -> AudioDecoderResult<Self> {
        let file = File::open(path.as_ref()).map_err(|e| {
            AudioDecoderError::FileNotFound(format!("{}: {}", path.as_ref().display(), e))
        })?;
        let reader = BufReader::new(file);

        let decoder = sotf_iamf::IamfDecoder::open(reader)
            .map_err(|e| AudioDecoderError::InvalidFile(format!("IAMF open failed: {e}")))?;

        let iamf_spec = decoder.spec();

        let spec = AudioSpec {
            sample_rate: iamf_spec.sample_rate,
            channels: iamf_spec.output_channels,
            bits_per_sample: iamf_spec.bit_depth.max(16),
            total_frames: None, // IAMF doesn't expose total frame count upfront
        };

        log::info!(
            "IAMF decoder: {}Hz, {}ch, {}-bit, {} samples/frame",
            spec.sample_rate,
            spec.channels,
            spec.bits_per_sample,
            iamf_spec.num_samples_per_frame,
        );

        Ok(Self {
            decoder,
            spec,
            eof: false,
        })
    }
}

impl AudioDecoder for IamfAudioDecoder {
    fn spec(&self) -> &AudioSpec {
        &self.spec
    }

    fn format(&self) -> AudioFormat {
        AudioFormat::Iamf
    }

    fn decode_into(&mut self, dest: &mut DecodedAudio) -> AudioDecoderResult<usize> {
        if self.eof {
            return Ok(0);
        }

        let iamf_spec = self.decoder.spec();
        let max_frames = iamf_spec.num_samples_per_frame as usize;
        let out_ch = self.spec.channels as usize;
        let buf_size = max_frames * out_ch;

        let mut buf = vec![0.0_f32; buf_size];

        match self.decoder.decode_next(&mut buf) {
            Ok(frames) => {
                if frames == 0 {
                    self.eof = true;
                    return Ok(0);
                }
                dest.frame_position = self.decoder.position().saturating_sub(frames as u64);
                dest.samples.extend_from_slice(&buf[..frames * out_ch]);
                Ok(frames)
            }
            Err(sotf_iamf::error::IamfError::EndOfStream) => {
                self.eof = true;
                Ok(0)
            }
            Err(e) => Err(AudioDecoderError::DecodingFailed(format!(
                "IAMF decode error: {e}"
            ))),
        }
    }

    fn seek(&mut self, frame_position: u64) -> AudioDecoderResult<()> {
        self.decoder
            .seek(frame_position)
            .map_err(|e| AudioDecoderError::SeekFailed(format!("IAMF seek error: {e}")))?;
        self.eof = false;
        Ok(())
    }

    fn position(&self) -> u64 {
        self.decoder.position()
    }

    fn is_eof(&self) -> bool {
        self.eof
    }
}
