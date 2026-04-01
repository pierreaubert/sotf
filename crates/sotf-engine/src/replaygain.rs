pub use math_audio_dsp::replaygain::{
    ReplayGainAnalyzer, ReplayGainInfo, ReplayGainTrackData, compute_album_gain,
};

use crate::decoder::{AudioDecoderError, AudioDecoderResult, create_decoder};
use std::path::Path;

/// Analyze an audio file and compute ReplayGain values.
///
/// Decodes the file and feeds frames to [`ReplayGainAnalyzer`].
pub fn analyze_file<P: AsRef<Path>>(path: P) -> AudioDecoderResult<ReplayGainInfo> {
    analyze_file_limited(path, None)
}

/// Analyze an audio file with an optional sample limit.
///
/// When `max_samples` is `Some(n)`, decoding stops after processing at least `n`
/// interleaved samples (e.g. 131072 ≈ 0.5 MB of f32 data). This is useful for
/// fast approximate analysis in tests.
pub fn analyze_file_limited<P: AsRef<Path>>(
    path: P,
    max_samples: Option<usize>,
) -> AudioDecoderResult<ReplayGainInfo> {
    log::info!(
        "[Replay Gain] Analyze : {}",
        path.as_ref().to_str().unwrap()
    );

    let mut decoder = create_decoder(path.as_ref())?;
    let spec = decoder.spec();

    let mut analyzer = ReplayGainAnalyzer::new(spec.channels as u32, spec.sample_rate)
        .map_err(AudioDecoderError::ConfigError)?;

    let mut total_samples = 0usize;
    while let Some(decoded) = decoder.decode_next()? {
        if decoded.is_empty() {
            continue;
        }
        analyzer
            .add_frames_f32(&decoded.samples)
            .map_err(AudioDecoderError::DecodingFailed)?;
        total_samples += decoded.samples.len();
        if let Some(limit) = max_samples
            && total_samples >= limit
        {
            break;
        }
    }

    analyzer
        .finalize()
        .map_err(AudioDecoderError::DecodingFailed)
}

/// Analyze an audio file and return extended ReplayGain data including
/// gating block count and energy, needed for album-level gain computation.
pub fn analyze_file_extended<P: AsRef<Path>>(path: P) -> AudioDecoderResult<ReplayGainTrackData> {
    let mut decoder = create_decoder(path.as_ref())?;
    let spec = decoder.spec();

    let mut analyzer = ReplayGainAnalyzer::new(spec.channels as u32, spec.sample_rate)
        .map_err(AudioDecoderError::ConfigError)?;

    while let Some(decoded) = decoder.decode_next()? {
        if decoded.is_empty() {
            continue;
        }
        analyzer
            .add_frames_f32(&decoded.samples)
            .map_err(AudioDecoderError::DecodingFailed)?;
    }

    analyzer
        .finalize_extended()
        .map_err(AudioDecoderError::DecodingFailed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::AudioDecoderError;

    #[test]
    fn test_analyze_nonexistent_file() {
        let result = analyze_file("nonexistent_file.flac");
        assert!(matches!(result, Err(AudioDecoderError::FileNotFound(_))));
    }

    #[test]
    fn test_analyze_unsupported_format() {
        let result = analyze_file("test.unsupported");
        assert!(matches!(
            result,
            Err(AudioDecoderError::UnsupportedFormat(_))
        ));
    }
}
