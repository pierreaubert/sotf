use crate::DsdOutputMode;
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
    create_decoder_with_dsd_mode(path, DsdOutputMode::Disabled)
}

/// Create a decoder for the given audio file using the requested DSD policy.
pub fn create_decoder_with_dsd_mode<P: AsRef<Path>>(
    path: P,
    dsd_output: DsdOutputMode,
) -> AudioDecoderResult<Box<dyn AudioDecoder>> {
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

    if let Ok(format) = AudioFormat::from_path(path)
        && format.is_dsd()
    {
        return create_dsd_decoder(path, format, dsd_output);
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

fn create_dsd_decoder(
    path: &Path,
    format: AudioFormat,
    dsd_output: DsdOutputMode,
) -> AudioDecoderResult<Box<dyn AudioDecoder>> {
    match dsd_output {
        DsdOutputMode::Disabled => Err(AudioDecoderError::UnsupportedFormat(format!(
            "{} is recognized, but DSD output is disabled in the engine config",
            format
        ))),
        DsdOutputMode::PcmDecode | DsdOutputMode::DopPreferred | DsdOutputMode::NativePreferred => {
            match format {
                AudioFormat::DsdDsf => Ok(Box::new(crate::decoder::dsd::DsfPcmDecoder::new(path)?)),
                AudioFormat::DsdDff => Ok(Box::new(crate::decoder::dsd::DffPcmDecoder::new(path)?)),
                AudioFormat::SacdIso => Err(AudioDecoderError::UnsupportedFormat(
                    "SACD ISO decoding is not available yet; extract DSF tracks or convert to PCM"
                        .to_string(),
                )),
                _ => unreachable!("create_dsd_decoder called for non-DSD format"),
            }
        }
        DsdOutputMode::DopRequired => Err(AudioDecoderError::UnsupportedFormat(format!(
            "{} requires DoP output, but the current playback backend cannot carry bit-perfect DoP frames",
            format
        ))),
        DsdOutputMode::NativeRequired => Err(AudioDecoderError::UnsupportedFormat(format!(
            "{} requires native DSD output, but the current playback backend cannot carry native DSD frames",
            format
        ))),
    }
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
    create_decoder_from_source_with_dsd_mode(source, DsdOutputMode::Disabled)
}

pub fn create_decoder_from_source_with_dsd_mode(
    source: &AudioSource,
    dsd_output: DsdOutputMode,
) -> AudioDecoderResult<Box<dyn AudioDecoder>> {
    let (decoder, _metadata_rx) =
        create_decoder_from_source_with_dsd_mode_and_metadata(source, dsd_output)?;
    Ok(decoder)
}

#[cfg(feature = "streaming")]
pub type SourceMetadataReceiver = std::sync::mpsc::Receiver<sotf_streaming::StreamMetadata>;
#[cfg(not(feature = "streaming"))]
pub type SourceMetadataReceiver = ();

/// Create a decoder from an `AudioSource`, optionally returning live stream metadata updates.
///
/// For local files and non-streaming sources, metadata updates are `None`.
pub fn create_decoder_from_source_with_dsd_mode_and_metadata(
    source: &AudioSource,
    dsd_output: DsdOutputMode,
) -> AudioDecoderResult<(Box<dyn AudioDecoder>, Option<SourceMetadataReceiver>)> {
    match source {
        AudioSource::File(path) => Ok((create_decoder_with_dsd_mode(path, dsd_output)?, None)),
        #[cfg(feature = "streaming")]
        AudioSource::Url {
            url,
            format_hint,
            seekable: _,
        } if url.starts_with("mpd-stream://") => {
            use symphonia::core::formats::probe::Hint;

            let (mpd_source, metadata_rx) = sotf_streaming::MpdStreamSource::open(url)
                .map_err(AudioDecoderError::NetworkError)?;

            let mut hint = Hint::new();
            if let Some(fh) = format_hint {
                hint.with_extension(fh);
            } else if let Some(detected) = mpd_source.format_hint() {
                hint.with_extension(&detected);
            }

            let decoder = SymphoniaDecoder::from_media_source(Box::new(mpd_source), hint, url)?;
            Ok((Box::new(decoder), Some(metadata_rx)))
        }
        #[cfg(all(feature = "streaming", feature = "hls"))]
        AudioSource::Url {
            url,
            format_hint,
            seekable: _,
        } if is_hls_source(url, format_hint.as_deref()) => {
            use symphonia::core::formats::probe::Hint;

            let hls_source = sotf_streaming::HlsSource::open(url)
                .map_err(|e| AudioDecoderError::NetworkError(e.to_string()))?;

            let mut hint = Hint::new();
            if let Some(detected) = hls_source.format_hint() {
                hint.with_extension(&detected);
            } else if let Some(fh) = format_hint
                && !is_hls_format_hint(fh)
            {
                hint.with_extension(fh);
            }

            let decoder = SymphoniaDecoder::from_media_source(Box::new(hls_source), hint, url)?;
            Ok((Box::new(decoder), None))
        }
        #[cfg(all(feature = "streaming", not(feature = "hls")))]
        AudioSource::Url {
            url,
            format_hint,
            seekable: _,
        } if is_hls_source(url, format_hint.as_deref()) => {
            Err(AudioDecoderError::UnsupportedFormat(format!(
                "HLS streaming not available (compile with 'hls' feature): {}",
                url
            )))
        }
        #[cfg(feature = "streaming")]
        AudioSource::Url {
            url,
            format_hint,
            seekable: _,
        } => {
            use symphonia::core::formats::probe::Hint;

            let (http_source, metadata_rx) = sotf_streaming::HttpMediaSource::open(url)
                .map_err(|e| AudioDecoderError::NetworkError(e.to_string()))?;

            // Build hint from explicit format_hint or from URL/content-type detection
            let mut hint = Hint::new();
            if let Some(fh) = format_hint {
                hint.with_extension(fh);
            } else if let Some(detected) = http_source.format_hint() {
                hint.with_extension(&detected);
            }

            let decoder = SymphoniaDecoder::from_media_source(Box::new(http_source), hint, url)?;
            Ok((Box::new(decoder), Some(metadata_rx)))
        }
        #[cfg(not(feature = "streaming"))]
        AudioSource::Url { url, .. } => Err(AudioDecoderError::UnsupportedFormat(format!(
            "HTTP streaming not available (compile with 'streaming' feature): {}",
            url
        ))),
        AudioSource::ServiceStream { service, track_id } => {
            use crate::decoder::service_resolver::{ResolvedServiceStream, resolve_service_stream};

            match resolve_service_stream(*service, track_id) {
                // The resolver handed back a plain URL (e.g. Tidal): run it
                // through the normal URL path (streaming/HLS handling included).
                Some(Ok(ResolvedServiceStream::Url {
                    url,
                    format_hint,
                    seekable,
                })) => create_decoder_from_source_with_dsd_mode_and_metadata(
                    &AudioSource::Url {
                        url,
                        format_hint,
                        seekable,
                    },
                    dsd_output,
                ),
                // The service decodes internally and provides f32 PCM (e.g.
                // Spotify via librespot): wrap the reader in a PcmDecoder.
                Some(Ok(ResolvedServiceStream::Pcm {
                    sample_rate,
                    channels,
                    bits_per_sample,
                    total_frames,
                    reader,
                })) => {
                    // The resolver is higher-layer code; validate the spec at
                    // the trust boundary. `channels == 0` would silently decode
                    // as instant EOF, and `sample_rate == 0` makes
                    // `AudioSpec::duration()` panic on `Duration::from_secs_f64(inf)`.
                    // Upper bounds: `PcmDecoder::new` pre-allocates
                    // `1024 * channels` f32 per decoder (~268 MB at u16::MAX),
                    // and anything beyond the engine's channel ceiling cannot
                    // be routed through the processing graph anyway.
                    // `bits_per_sample` must be honest: the wire format is
                    // fixed f32, and the value lands in `AudioSpec` where
                    // `bytes_per_frame()` and UI bitrate displays trust it.
                    const MAX_SERVICE_SAMPLE_RATE: u32 = 384_000;
                    if sample_rate == 0
                        || sample_rate > MAX_SERVICE_SAMPLE_RATE
                        || channels == 0
                        || channels as usize > crate::EngineConfig::MAX_CHANNELS
                        || bits_per_sample != 32
                    {
                        return Err(AudioDecoderError::ServiceError(format!(
                            "resolver returned invalid PCM spec: {} Hz, {} ch, {} bits \
                             (want 1..={} Hz, 1..={} ch, 32 bits)",
                            sample_rate,
                            channels,
                            bits_per_sample,
                            MAX_SERVICE_SAMPLE_RATE,
                            crate::EngineConfig::MAX_CHANNELS,
                        )));
                    }
                    Ok((
                        Box::new(crate::decoder::PcmDecoder::new(
                            sample_rate,
                            channels,
                            bits_per_sample,
                            total_frames,
                            reader,
                        )),
                        None,
                    ))
                }
                Some(Err(err)) => Err(AudioDecoderError::ServiceError(err)),
                // No resolver installed: the service wasn't configured.
                None => Err(AudioDecoderError::ServiceError(format!(
                    "Service {} not configured — cannot stream {}",
                    service, track_id
                ))),
            }
        }
        AudioSource::Driver => Err(AudioDecoderError::ConfigError(
            "Driver source does not use a decoder".to_string(),
        )),
    }
}

#[cfg(feature = "streaming")]
fn is_hls_source(url: &str, format_hint: Option<&str>) -> bool {
    if format_hint.is_some_and(is_hls_format_hint) {
        return true;
    }

    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .to_ascii_lowercase();
    path.ends_with(".m3u8") || path.ends_with(".m3u")
}

#[cfg(feature = "streaming")]
fn is_hls_format_hint(format_hint: &str) -> bool {
    matches!(
        format_hint.trim().to_ascii_lowercase().as_str(),
        "hls"
            | "m3u8"
            | "m3u"
            | "application/vnd.apple.mpegurl"
            | "application/x-mpegurl"
            | "audio/mpegurl"
            | "audio/x-mpegurl"
    )
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

    #[test]
    fn test_create_decoder_recognizes_dsd_as_unsupported_decoder_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.dsf");
        std::fs::write(&path, []).unwrap();

        let result = create_decoder(&path);
        assert!(matches!(
            result,
            Err(AudioDecoderError::UnsupportedFormat(message))
                if message.contains("DSD output is disabled")
        ));
    }

    #[test]
    fn test_create_decoder_reports_required_dsd_bitstream_gap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.dsf");
        std::fs::write(&path, []).unwrap();

        let result = create_decoder_with_dsd_mode(&path, DsdOutputMode::DopRequired);
        assert!(matches!(
            result,
            Err(AudioDecoderError::UnsupportedFormat(message))
                if message.contains("cannot carry bit-perfect DoP frames")
        ));
    }

    #[cfg(feature = "streaming")]
    #[test]
    fn test_hls_source_detection() {
        assert!(is_hls_source("https://example.test/live/index.m3u8", None));
        assert!(is_hls_source(
            "https://example.test/live/index.M3U8?token=abc",
            None
        ));
        assert!(is_hls_source(
            "https://example.test/live",
            Some("application/vnd.apple.mpegurl")
        ));
        assert!(!is_hls_source(
            "https://example.test/track.flac",
            Some("flac")
        ));
    }

    #[test]
    fn test_create_decoder_from_source_with_metadata_file() {
        let (temp, _mono) = sotf_testkit::audio::temp_sine_wav(0.1, 48_000, 2, 440.0).unwrap();
        let source = AudioSource::File(temp.path().to_path_buf());

        let result =
            create_decoder_from_source_with_dsd_mode_and_metadata(&source, DsdOutputMode::Disabled);

        let (decoder, metadata_rx) = result.expect("WAV file source should decode");
        assert_eq!(decoder.spec().sample_rate, 48_000);
        assert_eq!(decoder.spec().channels, 2);
        assert!(
            metadata_rx.is_none(),
            "local files have no live metadata receiver"
        );
    }

    #[test]
    fn test_create_decoder_from_source_unsupported_extension() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.unsupported");
        std::fs::write(&path, b"").unwrap();

        let source = AudioSource::File(path);
        let result = create_decoder_from_source(&source);

        assert!(
            matches!(result, Err(AudioDecoderError::UnsupportedFormat(_))),
            "unexpected result"
        );
    }

    #[test]
    fn test_create_decoder_from_source_service_stream() {
        use crate::decoder::service_resolver::{
            ResolvedServiceStream, clear_service_stream_resolver, set_service_stream_resolver,
        };
        use std::io::Cursor;
        use std::sync::Arc;

        // Single test function for everything that touches the process-global
        // resolver, so tests cannot race on it. Any future resolver-touching
        // test must be folded in here for the same reason.
        struct ResolverGuard;
        impl Drop for ResolverGuard {
            fn drop(&mut self) {
                clear_service_stream_resolver();
            }
        }
        let _resolver_guard = ResolverGuard;

        // 1. No resolver installed: service is reported as not configured.
        clear_service_stream_resolver();
        assert!(!crate::decoder::has_service_stream_resolver());
        let source = AudioSource::ServiceStream {
            service: crate::ServiceId::Spotify,
            track_id: "test-track".to_string(),
        };
        // `Box<dyn AudioDecoder>` is not Debug; discard it for assertions.
        let result = create_decoder_from_source(&source).map(drop);
        assert!(
            matches!(&result, Err(AudioDecoderError::ServiceError(message)) if message.contains("not configured")),
            "unexpected result: {result:?}"
        );

        // 2. Resolver returning PCM: a PcmDecoder is produced and decodes.
        let pcm_samples: Vec<f32> = vec![0.5, -0.5, 0.25, -0.25];
        let pcm_bytes: Vec<u8> = pcm_samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        set_service_stream_resolver(Arc::new(move |service, track_id| {
            assert_eq!(service, crate::ServiceId::Spotify);
            assert_eq!(track_id, "test-track");
            Ok(ResolvedServiceStream::Pcm {
                sample_rate: 44100,
                channels: 2,
                bits_per_sample: 32,
                total_frames: Some(2),
                reader: Box::new(Cursor::new(pcm_bytes.clone())),
            })
        }));
        assert!(crate::decoder::has_service_stream_resolver());
        let (mut decoder, metadata_rx) =
            create_decoder_from_source_with_dsd_mode_and_metadata(&source, DsdOutputMode::Disabled)
                .expect("PCM service stream should decode");
        assert!(metadata_rx.is_none());
        assert_eq!(decoder.spec().sample_rate, 44100);
        assert_eq!(decoder.spec().channels, 2);
        let mut dest = DecodedAudio::new(decoder.spec().clone());
        let frames = decoder.decode_into(&mut dest).unwrap();
        assert_eq!(frames, 2);
        assert_eq!(dest.samples, pcm_samples);

        // 3. Resolver errors surface as ServiceError.
        set_service_stream_resolver(Arc::new(|_, _| Err("auth expired".to_string())));
        let result = create_decoder_from_source(&source).map(drop);
        assert!(
            matches!(&result, Err(AudioDecoderError::ServiceError(message)) if message.contains("auth expired")),
            "unexpected result: {result:?}"
        );

        // 3a. A panicking resolver is caught and reported, not unwound into
        // the decoder thread.
        set_service_stream_resolver(Arc::new(|_, _| panic!("boom")));
        let result = create_decoder_from_source(&source).map(drop);
        assert!(
            matches!(&result, Err(AudioDecoderError::ServiceError(message)) if message.contains("panicked")),
            "unexpected result: {result:?}"
        );

        // 3b. A resolver that re-enters the hook (self-uninstall on auth
        // failure) must not deadlock: the read guard is released before the
        // resolver runs. Pre-fix this hung the calling thread forever.
        set_service_stream_resolver(Arc::new(|_, _| {
            clear_service_stream_resolver();
            Err("auth expired".to_string())
        }));
        let result = create_decoder_from_source(&source).map(drop);
        assert!(
            matches!(&result, Err(AudioDecoderError::ServiceError(message)) if message.contains("auth expired")),
            "unexpected result: {result:?}"
        );

        // 3c. An invalid PCM spec (0 Hz / 0 channels) is rejected at the trust
        // boundary instead of producing a silently-empty or panicking decoder.
        set_service_stream_resolver(Arc::new(|_, _| {
            Ok(ResolvedServiceStream::Pcm {
                sample_rate: 0,
                channels: 0,
                bits_per_sample: 32,
                total_frames: Some(1),
                reader: Box::new(Cursor::new(Vec::<u8>::new())),
            })
        }));
        let result = create_decoder_from_source(&source).map(drop);
        assert!(
            matches!(&result, Err(AudioDecoderError::ServiceError(message)) if message.contains("invalid PCM spec")),
            "unexpected result: {result:?}"
        );

        // 3d. Out-of-range specs are rejected too: an absurd sample rate, a
        // channel count above the engine's ceiling (PcmDecoder pre-allocates
        // 1024 * channels f32 — ~268 MB at u16::MAX), and a dishonest
        // bits_per_sample (the wire format is fixed f32).
        for (sample_rate, channels, bits_per_sample) in
            [(384_001, 2, 32), (44_100, 17, 32), (44_100, 2, 16)]
        {
            set_service_stream_resolver(Arc::new(move |_, _| {
                Ok(ResolvedServiceStream::Pcm {
                    sample_rate,
                    channels,
                    bits_per_sample,
                    total_frames: Some(1),
                    reader: Box::new(Cursor::new(Vec::<u8>::new())),
                })
            }));
            let result = create_decoder_from_source(&source).map(drop);
            assert!(
                matches!(&result, Err(AudioDecoderError::ServiceError(message)) if message.contains("invalid PCM spec")),
                "spec {sample_rate} Hz / {channels} ch / {bits_per_sample} bits must be rejected: {result:?}"
            );
        }

        // 4. Resolver returning a URL goes through the URL path: without the
        // `streaming` feature that is an UnsupportedFormat error; with it, a
        // bogus loopback URL deterministically fails with a network error.
        set_service_stream_resolver(Arc::new(|_, _| {
            Ok(ResolvedServiceStream::Url {
                url: "http://127.0.0.1:1/none.flac".to_string(),
                format_hint: Some("flac".to_string()),
                seekable: true,
            })
        }));
        let result = create_decoder_from_source(&source).map(drop);
        #[cfg(not(feature = "streaming"))]
        assert!(
            matches!(result, Err(AudioDecoderError::UnsupportedFormat(_))),
            "unexpected result: {result:?}"
        );
        #[cfg(feature = "streaming")]
        assert!(
            matches!(result, Err(AudioDecoderError::NetworkError(_))),
            "unexpected result: {result:?}"
        );

        clear_service_stream_resolver();
        assert!(!crate::decoder::has_service_stream_resolver());
    }

    #[test]
    fn test_create_decoder_from_source_driver() {
        let result = create_decoder_from_source(&AudioSource::Driver);

        assert!(
            matches!(result, Err(AudioDecoderError::ConfigError(_))),
            "unexpected result"
        );
    }

    #[test]
    fn test_create_decoder_from_source_dsd_disabled_rejects_dsf() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.dsf");
        std::fs::write(&path, b"").unwrap();

        let source = AudioSource::File(path);
        let result = create_decoder_from_source_with_dsd_mode(&source, DsdOutputMode::Disabled);

        assert!(matches!(
            result,
            Err(AudioDecoderError::UnsupportedFormat(message))
                if message.contains("DSD output is disabled")
        ));
    }

    #[test]
    fn test_create_decoder_from_source_dsd_dop_required_rejects_dsf() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.dsf");
        std::fs::write(&path, b"").unwrap();

        let source = AudioSource::File(path);
        let result = create_decoder_from_source_with_dsd_mode(&source, DsdOutputMode::DopRequired);

        assert!(matches!(
            result,
            Err(AudioDecoderError::UnsupportedFormat(message))
                if message.contains("cannot carry bit-perfect DoP frames")
        ));
    }

    #[test]
    fn test_create_decoder_from_source_dsd_native_required_rejects_dsf() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.dsf");
        std::fs::write(&path, b"").unwrap();

        let source = AudioSource::File(path);
        let result =
            create_decoder_from_source_with_dsd_mode(&source, DsdOutputMode::NativeRequired);

        assert!(matches!(
            result,
            Err(AudioDecoderError::UnsupportedFormat(message))
                if message.contains("cannot carry native DSD frames")
        ));
    }
}
