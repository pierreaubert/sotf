use crate::decoder::error::{AudioDecoderError, AudioDecoderResult};
use crate::decoder::formats::{AudioFormat, SymphoniaDecoder};
use crate::decoder::source::AudioSource;
use std::path::Path;
use std::time::Duration;

/// Audio sample information
#[derive(Debug, Clone, PartialEq)]
pub struct AudioSpec {
    /// Sample rate in Hz (e.g., 44100, 48000, 96000)
    pub sample_rate: u32,
    /// Number of channels (1 = mono, 2 = stereo, etc.)
    pub channels: u16,
    /// Bits per sample (16, 24, 32)
    pub bits_per_sample: u16,
    /// Total number of frames in the audio file (if known)
    pub total_frames: Option<u64>,
}

impl AudioSpec {
    /// Calculate the duration of the audio file
    pub fn duration(&self) -> Option<Duration> {
        self.total_frames.map(|frames| {
            let seconds = frames as f64 / self.sample_rate as f64;
            Duration::from_secs_f64(seconds)
        })
    }

    /// Calculate bytes per frame (all channels)
    pub fn bytes_per_frame(&self) -> u32 {
        (self.channels as u32) * (self.bits_per_sample as u32) / 8
    }
}

/// Decoded audio data buffer
#[derive(Debug, Clone)]
pub struct DecodedAudio {
    /// Audio specification
    pub spec: AudioSpec,
    /// PCM audio samples as f32 values (interleaved if multi-channel)
    /// Values are normalized to [-1.0, 1.0] range
    pub samples: Vec<f32>,
    /// Current frame position in the stream
    pub frame_position: u64,
}

impl DecodedAudio {
    pub fn new(spec: AudioSpec) -> Self {
        Self {
            spec,
            samples: Vec::new(),
            frame_position: 0,
        }
    }

    /// Get the number of frames in this buffer
    pub fn frame_count(&self) -> usize {
        self.samples.len() / self.spec.channels as usize
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.samples.clear();
    }

    /// Convert samples to bytes for streaming to external processes
    pub fn to_bytes_f32_le(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.samples.len() * 4);
        for sample in &self.samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    }
}

/// Main audio decoder trait
pub trait AudioDecoder {
    /// Get the audio specification (sample rate, channels, etc.)
    fn spec(&self) -> &AudioSpec;

    /// Get the audio format
    fn format(&self) -> AudioFormat;

    /// Decode the next chunk of audio data into the provided buffer.
    ///
    /// Implementations must overwrite `dest`: clear `dest.samples` before
    /// appending decoded data, update `dest.spec`, and set
    /// `dest.frame_position` to the first decoded frame. Callers may reuse the
    /// same `DecodedAudio` allocation across calls.
    ///
    /// Returns the number of frames decoded (0 indicates end of stream).
    fn decode_into(&mut self, dest: &mut DecodedAudio) -> AudioDecoderResult<usize>;

    /// Decode the next chunk of audio data (allocates new buffer)
    /// Returns None when the stream ends
    fn decode_next(&mut self) -> AudioDecoderResult<Option<DecodedAudio>> {
        let mut dest = DecodedAudio::new(self.spec().clone());
        let frames = self.decode_into(&mut dest)?;
        if frames == 0 {
            Ok(None)
        } else {
            Ok(Some(dest))
        }
    }

    /// Seek to a specific frame position
    fn seek(&mut self, frame_position: u64) -> AudioDecoderResult<()>;

    /// Get current playback position in frames
    fn position(&self) -> u64;

    /// Reset decoder to beginning
    fn reset(&mut self) -> AudioDecoderResult<()> {
        self.seek(0)
    }

    /// Check if decoder has reached end of stream
    fn is_eof(&self) -> bool;
}

/// Create a decoder for the given audio file
pub fn create_decoder<P: AsRef<Path>>(path: P) -> AudioDecoderResult<Box<dyn AudioDecoder>> {
    let path = path.as_ref();

    // First, validate the file extension to detect unsupported formats early
    if let Err(err) = AudioFormat::from_path(path) {
        // If the error is specifically UnsupportedFormat, return it now even if the file
        // doesn't exist. This matches test expectations where extension validation is prioritized.
        if matches!(err, AudioDecoderError::UnsupportedFormat(_)) {
            return Err(err);
        }
    }

    // Validate file exists (for supported extensions)
    if !path.exists() {
        return Err(AudioDecoderError::FileNotFound(
            path.to_string_lossy().to_string(),
        ));
    }

    // Route IAMF files to the dedicated IAMF decoder
    #[cfg(feature = "iamf")]
    if let Ok(AudioFormat::Iamf) = AudioFormat::from_path(path) {
        let decoder = crate::decoder::iamf::IamfAudioDecoder::new(path)?;
        return Ok(Box::new(decoder));
    }

    // Create unified Symphonia decoder that handles format detection internally
    let decoder = SymphoniaDecoder::new(path)?;
    Ok(Box::new(decoder))
}

/// Probe an audio file to get basic information without creating a full decoder
pub fn probe_file<P: AsRef<Path>>(path: P) -> AudioDecoderResult<(AudioFormat, AudioSpec)> {
    let path = path.as_ref();

    // Validate file exists
    if !path.exists() {
        return Err(AudioDecoderError::FileNotFound(
            path.to_string_lossy().to_string(),
        ));
    }

    // Detect format
    let format = AudioFormat::from_path(path)?;

    // Create temporary decoder to get spec
    let decoder = create_decoder(path)?;
    let spec = decoder.spec().clone();

    Ok((format, spec))
}

/// Create a decoder from an `AudioSource` (file, URL, or service stream).
///
/// This is the main entry point for the decoder thread to create decoders
/// from any supported source type.
pub fn create_decoder_from_source(
    source: &AudioSource,
) -> AudioDecoderResult<Box<dyn AudioDecoder>> {
    match source {
        AudioSource::File(path) => create_decoder(path),
        #[cfg(feature = "streaming")]
        AudioSource::Url {
            url,
            format_hint,
            seekable: _,
        } if url.starts_with("mpd-stream://") => {
            use symphonia::core::formats::probe::Hint;

            let (mpd_source, _metadata_rx) = sotf_streaming::MpdStreamSource::open(url)
                .map_err(AudioDecoderError::NetworkError)?;

            let mut hint = Hint::new();
            if let Some(fh) = format_hint {
                hint.with_extension(fh);
            } else if let Some(detected) = mpd_source.format_hint() {
                hint.with_extension(&detected);
            }

            let decoder = SymphoniaDecoder::from_media_source(Box::new(mpd_source), hint, url)?;
            Ok(Box::new(decoder))
        }
        #[cfg(feature = "streaming")]
        AudioSource::Url {
            url,
            format_hint,
            seekable: _,
        } => {
            use symphonia::core::formats::probe::Hint;

            let (http_source, _metadata_rx) = sotf_streaming::HttpMediaSource::open(url)
                .map_err(|e| AudioDecoderError::NetworkError(e.to_string()))?;

            // Build hint from explicit format_hint or from URL/content-type detection
            let mut hint = Hint::new();
            if let Some(fh) = format_hint {
                hint.with_extension(fh);
            } else if let Some(detected) = http_source.format_hint() {
                hint.with_extension(&detected);
            }

            let decoder = SymphoniaDecoder::from_media_source(Box::new(http_source), hint, url)?;
            Ok(Box::new(decoder))
        }
        #[cfg(not(feature = "streaming"))]
        AudioSource::Url { url, .. } => Err(AudioDecoderError::UnsupportedFormat(format!(
            "HTTP streaming not available (compile with 'streaming' feature): {}",
            url
        ))),
        AudioSource::ServiceStream { service, track_id } => {
            // Service streams are resolved at a higher level (player/manager) into
            // either AudioSource::Url (Tidal) or a pre-configured PcmDecoder (Spotify).
            // If we reach here, the service wasn't configured.
            Err(AudioDecoderError::ServiceError(format!(
                "Service {} not configured — cannot stream {}",
                service, track_id
            )))
        }
        AudioSource::Driver => Err(AudioDecoderError::ConfigError(
            "Driver source does not use a decoder".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_audio_spec() {
        let spec = AudioSpec {
            sample_rate: 48000,
            channels: 2,
            bits_per_sample: 24,
            total_frames: Some(240000), // 5 seconds at 48kHz
        };

        assert_eq!(spec.duration(), Some(Duration::from_secs(5)));
        assert_eq!(spec.bytes_per_frame(), 6); // 2 channels * 24 bits / 8 = 6 bytes
    }

    #[test]
    fn test_decoded_audio() {
        let spec = AudioSpec {
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: 16,
            total_frames: Some(1000),
        };

        let mut decoded = DecodedAudio::new(spec);
        assert!(decoded.is_empty());
        assert_eq!(decoded.frame_count(), 0);

        // Add some stereo samples (L, R, L, R)
        decoded.samples = vec![0.5, -0.5, 0.25, -0.25];
        assert_eq!(decoded.frame_count(), 2); // 4 samples / 2 channels = 2 frames
        assert!(!decoded.is_empty());

        // Test byte conversion
        let bytes = decoded.to_bytes_f32_le();
        assert_eq!(bytes.len(), 16); // 4 samples * 4 bytes each = 16 bytes
    }

    #[test]
    fn test_create_decoder_nonexistent_file() {
        let result = create_decoder("nonexistent.flac");
        assert!(matches!(result, Err(AudioDecoderError::FileNotFound(_))));
    }

    #[test]
    fn test_create_decoder_unsupported_format() {
        // This will fail at format detection, not file existence
        let result = create_decoder("test.unsupported");
        assert!(matches!(
            result,
            Err(AudioDecoderError::UnsupportedFormat(_))
        ));
    }
}
