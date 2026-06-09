// ============================================================================
// IAMF Codec Wrappers
// ============================================================================
//
// IAMF substreams are encoded with standard codecs (Opus, AAC, FLAC, LPCM).
// This module handles substream reassembly and provides a trait for codec
// decoding that the engine layer implements using Symphonia.

use crate::error::{IamfError, IamfResult};
use crate::types::CodecId;

/// Trait for decoding an individual IAMF audio substream.
///
/// Implementations are provided by the engine layer using Symphonia or
/// other codec libraries.
pub trait SubstreamDecoder: Send {
    /// Decode a single audio frame payload into PCM samples.
    /// Returns interleaved f32 samples normalized to [-1.0, 1.0].
    fn decode_frame(&mut self, payload: &[u8]) -> IamfResult<Vec<f32>>;

    /// Get the number of output channels for this substream.
    fn channels(&self) -> usize;

    /// Reset the decoder state (e.g. after seeking).
    fn reset(&mut self);
}

/// LPCM passthrough decoder (no external codec needed)
pub struct LpcmDecoder {
    channels: usize,
    bit_depth: u16,
    _sample_rate: u32,
}

impl LpcmDecoder {
    pub fn new(channels: usize, bit_depth: u16, sample_rate: u32) -> Self {
        Self {
            channels,
            bit_depth,
            _sample_rate: sample_rate,
        }
    }
}

impl SubstreamDecoder for LpcmDecoder {
    fn decode_frame(&mut self, payload: &[u8]) -> IamfResult<Vec<f32>> {
        match self.bit_depth {
            16 => {
                // IAMF LPCM is big-endian per spec
                let samples = payload
                    .chunks_exact(2)
                    .map(|chunk| {
                        let val = i16::from_be_bytes([chunk[0], chunk[1]]);
                        val as f32 / 32768.0
                    })
                    .collect();
                Ok(samples)
            }
            24 => {
                // IAMF LPCM is big-endian per spec
                let samples = payload
                    .chunks_exact(3)
                    .map(|chunk| {
                        let val = (i32::from(chunk[0]) << 16)
                            | (i32::from(chunk[1]) << 8)
                            | i32::from(chunk[2]);
                        // Sign extend from 24-bit
                        let val = if val & 0x800000 != 0 {
                            val | !0xFF_FFFF
                        } else {
                            val
                        };
                        val as f32 / 8_388_608.0
                    })
                    .collect();
                Ok(samples)
            }
            32 => {
                // IAMF LPCM is big-endian per spec
                let samples = payload
                    .chunks_exact(4)
                    .map(|chunk| f32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect();
                Ok(samples)
            }
            _ => Err(IamfError::CodecError(format!(
                "Unsupported LPCM bit depth: {}",
                self.bit_depth
            ))),
        }
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn reset(&mut self) {
        // Stateless
    }
}

/// Factory function to create a decoder for a given codec.
/// Only LPCM is handled natively. Other codecs need engine-level support.
pub fn create_substream_decoder(
    codec_id: CodecId,
    channels: usize,
    bit_depth: u16,
    sample_rate: u32,
    _decoder_config: &[u8],
) -> IamfResult<Box<dyn SubstreamDecoder>> {
    match codec_id {
        CodecId::Lpcm => Ok(Box::new(LpcmDecoder::new(channels, bit_depth, sample_rate))),
        CodecId::Opus => Err(IamfError::UnsupportedCodec(
            "Opus decoding requires engine-level Symphonia integration".into(),
        )),
        CodecId::AacLc => Err(IamfError::UnsupportedCodec(
            "AAC-LC decoding requires engine-level Symphonia integration".into(),
        )),
        CodecId::Flac => Err(IamfError::UnsupportedCodec(
            "FLAC decoding requires engine-level Symphonia integration".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lpcm_16bit_decode() {
        let mut decoder = LpcmDecoder::new(1, 16, 48000);

        // Silence (16-bit BE)
        let silence = [0u8; 4]; // 2 samples of silence
        let samples = decoder.decode_frame(&silence).unwrap();
        assert_eq!(samples.len(), 2);
        assert!(samples[0].abs() < 1e-6);
        assert!(samples[1].abs() < 1e-6);

        // Full-scale positive (big-endian)
        let full_pos = [0x7F, 0xFF]; // 32767 in BE
        let samples = decoder.decode_frame(&full_pos).unwrap();
        assert!((samples[0] - (32767.0 / 32768.0)).abs() < 1e-4);
    }

    #[test]
    fn test_lpcm_32bit_float_decode() {
        let mut decoder = LpcmDecoder::new(1, 32, 48000);
        let val: f32 = 0.5;
        let bytes = val.to_be_bytes(); // IAMF LPCM is big-endian
        let samples = decoder.decode_frame(&bytes).unwrap();
        assert_eq!(samples.len(), 1);
        assert!((samples[0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_lpcm_24bit_decode() {
        let mut decoder = LpcmDecoder::new(1, 24, 48000);

        // Full-scale positive 24-bit big-endian: 0x7FFFFF = 8388607
        let full_pos = [0x7F, 0xFF, 0xFF];
        let samples = decoder.decode_frame(&full_pos).unwrap();
        assert_eq!(samples.len(), 1);
        assert!((samples[0] - (8_388_607.0 / 8_388_608.0)).abs() < 1e-4);

        // Full-scale negative 24-bit big-endian: 0x800000 = -8388608
        let full_neg = [0x80, 0x00, 0x00];
        let samples = decoder.decode_frame(&full_neg).unwrap();
        assert_eq!(samples.len(), 1);
        assert!((samples[0] - (-1.0)).abs() < 1e-4);
    }

    #[test]
    fn test_lpcm_channels() {
        let decoder = LpcmDecoder::new(6, 24, 48000);
        assert_eq!(decoder.channels(), 6);
    }

    #[test]
    fn test_lpcm_16bit_negative() {
        let mut decoder = LpcmDecoder::new(1, 16, 48000);

        // -1 in big-endian 16-bit: 0xFFFF
        let neg_one = [0xFF, 0xFF];
        let samples = decoder.decode_frame(&neg_one).unwrap();
        assert!((samples[0] - (-1.0 / 32768.0)).abs() < 1e-4);
    }

    #[test]
    fn test_lpcm_unsupported_bit_depth_errors() {
        let mut decoder = LpcmDecoder::new(1, 12, 48000);
        let result = decoder.decode_frame(&[0u8; 4]);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_substream_decoder_lpcm() {
        let decoder = create_substream_decoder(CodecId::Lpcm, 2, 16, 48000, &[]);
        assert!(decoder.is_ok());
        assert_eq!(decoder.unwrap().channels(), 2);
    }

    #[test]
    fn test_create_substream_decoder_opus_errors() {
        let decoder = create_substream_decoder(CodecId::Opus, 2, 32, 48000, &[]);
        assert!(decoder.is_err());
    }

    #[test]
    fn test_create_substream_decoder_aac_errors() {
        let decoder = create_substream_decoder(CodecId::AacLc, 2, 16, 48000, &[]);
        assert!(decoder.is_err());
    }

    #[test]
    fn test_lpcm_empty_payload() {
        let mut decoder = LpcmDecoder::new(1, 16, 48000);
        let samples = decoder.decode_frame(&[]).unwrap();
        assert!(samples.is_empty());
    }

    #[test]
    fn test_lpcm_reset_is_noop() {
        let mut decoder = LpcmDecoder::new(1, 16, 48000);
        decoder.reset(); // should not panic
        let samples = decoder.decode_frame(&[0x7F, 0xFF]).unwrap();
        assert_eq!(samples.len(), 1);
    }

    #[test]
    fn test_lpcm_24bit_negative_sign_extend() {
        let mut decoder = LpcmDecoder::new(1, 24, 48000);

        // -1 as 24-bit big-endian: 0xFFFFFF
        let neg_one = [0xFF, 0xFF, 0xFF];
        let samples = decoder.decode_frame(&neg_one).unwrap();
        assert!((samples[0] - (-1.0 / 8_388_608.0)).abs() < 1e-4);
    }
}
