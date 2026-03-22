// ============================================================================
// PcmDecoder — Pass-through decoder for pre-decoded PCM streams
// ============================================================================
//
// Used when a streaming service (e.g. Spotify via librespot) provides raw PCM
// samples instead of an encoded audio file. The decoder simply reads f32
// samples from the provided reader without any Symphonia decoding.

use crate::decoder::core::{AudioDecoder, AudioSpec, DecodedAudio};
use crate::decoder::error::{AudioDecoderError, AudioDecoderResult};
use crate::decoder::formats::AudioFormat;
use std::io::Read;

/// Decoder for raw interleaved f32 PCM streams.
///
/// The reader must produce f32 little-endian samples (4 bytes per sample).
/// Channels are interleaved: L0 R0 L1 R1 ...
pub struct PcmDecoder {
    spec: AudioSpec,
    reader: Box<dyn Read + Send>,
    position: u64,
    eof: bool,
    /// Temporary byte buffer for reading from the stream
    read_buf: Vec<u8>,
}

impl PcmDecoder {
    /// Create a new PCM decoder.
    ///
    /// - `sample_rate`: e.g. 44100
    /// - `channels`: e.g. 2
    /// - `bits_per_sample`: for metadata only (always reads f32)
    /// - `total_frames`: None for live/infinite streams
    /// - `reader`: produces interleaved f32 little-endian bytes
    pub fn new(
        sample_rate: u32,
        channels: u16,
        bits_per_sample: u16,
        total_frames: Option<u64>,
        reader: Box<dyn Read + Send>,
    ) -> Self {
        let spec = AudioSpec {
            sample_rate,
            channels,
            bits_per_sample,
            total_frames,
        };

        // Pre-allocate read buffer for one "chunk" (~1024 frames)
        let chunk_bytes = 1024 * channels as usize * 4; // 4 bytes per f32
        Self {
            spec,
            reader,
            position: 0,
            eof: false,
            read_buf: vec![0u8; chunk_bytes],
        }
    }
}

impl AudioDecoder for PcmDecoder {
    fn spec(&self) -> &AudioSpec {
        &self.spec
    }

    fn format(&self) -> AudioFormat {
        AudioFormat::Wav // Closest match for raw PCM
    }

    fn decode_into(&mut self, dest: &mut DecodedAudio) -> AudioDecoderResult<usize> {
        if self.eof {
            return Ok(0);
        }

        dest.samples.clear();
        dest.frame_position = self.position;
        dest.spec = self.spec.clone();

        // Read a chunk of raw bytes
        let bytes_to_read = self.read_buf.len();
        let mut total_read = 0;

        while total_read < bytes_to_read {
            match self.reader.read(&mut self.read_buf[total_read..]) {
                Ok(0) => {
                    self.eof = true;
                    break;
                }
                Ok(n) => {
                    total_read += n;
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    self.eof = true;
                    return Err(AudioDecoderError::IoError(e.to_string()));
                }
            }
        }

        if total_read == 0 {
            return Ok(0);
        }

        // Ensure we have a multiple of 4 bytes (one f32)
        let usable_bytes = total_read - (total_read % 4);
        let num_samples = usable_bytes / 4;

        // Convert bytes to f32 samples
        dest.samples.reserve(num_samples);
        for i in 0..num_samples {
            let offset = i * 4;
            let bytes = [
                self.read_buf[offset],
                self.read_buf[offset + 1],
                self.read_buf[offset + 2],
                self.read_buf[offset + 3],
            ];
            dest.samples.push(f32::from_le_bytes(bytes));
        }

        let frames = num_samples / self.spec.channels as usize;
        self.position += frames as u64;

        Ok(frames)
    }

    fn seek(&mut self, _frame_position: u64) -> AudioDecoderResult<()> {
        Err(AudioDecoderError::SeekFailed(
            "Seeking not supported for service PCM streams".to_string(),
        ))
    }

    fn position(&self) -> u64 {
        self.position
    }

    fn is_eof(&self) -> bool {
        self.eof
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_pcm_decoder_reads_f32_samples() {
        // Create a buffer with 4 stereo frames of f32 data
        let samples: Vec<f32> = vec![
            0.5, -0.5, // frame 0: L, R
            0.25, -0.25, // frame 1
            0.75, -0.75, // frame 2
            1.0, -1.0, // frame 3
        ];
        let mut bytes = Vec::new();
        for s in &samples {
            bytes.extend_from_slice(&s.to_le_bytes());
        }

        let reader = Box::new(Cursor::new(bytes));
        let mut decoder = PcmDecoder::new(44100, 2, 16, Some(4), reader);

        assert_eq!(decoder.spec().sample_rate, 44100);
        assert_eq!(decoder.spec().channels, 2);
        assert!(!decoder.is_eof());

        let mut dest = DecodedAudio::new(decoder.spec().clone());
        let frames = decoder.decode_into(&mut dest).unwrap();

        assert_eq!(frames, 4);
        assert_eq!(dest.samples.len(), 8);
        assert!((dest.samples[0] - 0.5).abs() < 1e-6);
        assert!((dest.samples[1] - (-0.5)).abs() < 1e-6);
        assert_eq!(decoder.position(), 4);
    }

    #[test]
    fn test_pcm_decoder_eof() {
        let reader = Box::new(Cursor::new(Vec::<u8>::new()));
        let mut decoder = PcmDecoder::new(44100, 2, 16, None, reader);

        let mut dest = DecodedAudio::new(decoder.spec().clone());
        let frames = decoder.decode_into(&mut dest).unwrap();

        assert_eq!(frames, 0);
        assert!(decoder.is_eof());
    }

    #[test]
    fn test_pcm_decoder_seek_unsupported() {
        let reader = Box::new(Cursor::new(Vec::<u8>::new()));
        let mut decoder = PcmDecoder::new(44100, 2, 16, None, reader);

        assert!(decoder.seek(100).is_err());
    }
}
