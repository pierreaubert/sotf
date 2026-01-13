use crate::decoder::{AudioDecoderError, AudioDecoderResult, create_decoder};
use ebur128::{EbuR128, Mode};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// ReplayGain 2.0 Reference Gain
///
/// See the [ReplayGain 2.0 specification][rg2spec] for details.
///
/// [rg2spec]: https://wiki.hydrogenaud.io/index.php?title=ReplayGain_2.0_specification#Reference_level
const REPLAYGAIN2_REFERENCE_LUFS: f64 = -18.0;

/// ReplayGain analysis result containing gain and peak information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayGainInfo {
    /// ReplayGain 2.0 Track Gain in dB
    /// This value indicates how much the track should be adjusted to reach the reference level
    pub gain: f64,

    /// ReplayGain 2.0 Track Peak (0.0 to 1.0+)
    /// The maximum sample peak across all channels
    pub peak: f64,
}

/// Analyze an audio file and compute ReplayGain values
///
/// This function decodes an audio file using Symphonia and computes ReplayGain 2.0
/// loudness and peak values according to the EBU R128 standard.
///
/// # Arguments
///
/// * `path` - Path to the audio file to analyze
///
/// # Returns
///
/// Returns `ReplayGainInfo` containing the gain (in dB) and peak values.
///
/// # Errors
///
/// Returns an `AudioDecoderError` if:
/// - The file cannot be found or opened
/// - The file format is unsupported
/// - Decoding fails
/// - EBU R128 analysis fails
///
/// # Example
///
/// ```no_run
/// use sotf_audio::replaygain::analyze_file;
///
/// let info = analyze_file("track.flac").unwrap();
/// log::info!("ReplayGain: {:.2} dB", info.gain);
/// log::info!("Peak: {:.6}", info.peak);
/// ```
pub fn analyze_file<P: AsRef<Path>>(path: P) -> AudioDecoderResult<ReplayGainInfo> {
    log::info!(
        "[Replay Gain] Analyze : {}",
        path.as_ref().to_str().unwrap()
    );

    let path = path.as_ref();

    // Create decoder for the audio file
    let mut decoder = create_decoder(path)?;

    // Get audio specifications
    let spec = decoder.spec();
    let channels = spec.channels as u32;
    let sample_rate = spec.sample_rate;

    // Create EBU R128 analyzer with all measurement modes
    let mut ebur128 = EbuR128::new(channels, sample_rate, Mode::all()).map_err(|e| {
        AudioDecoderError::ConfigError(format!("Failed to create EBU R128 analyzer: {:?}", e))
    })?;

    // Process audio in chunks
    while let Some(decoded) = decoder.decode_next()? {
        if decoded.is_empty() {
            continue;
        }

        // Add samples to EBU R128 analyzer
        // Samples are already in f32 format normalized to [-1.0, 1.0]
        ebur128.add_frames_f32(&decoded.samples).map_err(|e| {
            AudioDecoderError::DecodingFailed(format!("Failed to add frames to EBU R128: {:?}", e))
        })?;
    }

    // Calculate global loudness
    let loudness = ebur128.loudness_global().map_err(|e| {
        AudioDecoderError::DecodingFailed(format!("Failed to calculate loudness: {:?}", e))
    })?;

    // Calculate peak across all channels
    let mut peak = 0.0f64;
    for channel_index in 0..channels {
        let channel_peak = ebur128.sample_peak(channel_index).map_err(|e| {
            AudioDecoderError::DecodingFailed(format!(
                "Failed to get peak for channel {}: {:?}",
                channel_index, e
            ))
        })?;
        peak = peak.max(channel_peak);
    }

    // Calculate ReplayGain: reference level minus the measured loudness
    let gain = REPLAYGAIN2_REFERENCE_LUFS - loudness;

    log::debug!("[Replay Gain] Gain: {}dB Peak: {}dB", gain, peak);
    Ok(ReplayGainInfo { gain, peak })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_replaygain_info_serialization() {
        let info = ReplayGainInfo {
            gain: -5.5,
            peak: 0.95,
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("-5.5"));
        assert!(json.contains("0.95"));

        let deserialized: ReplayGainInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.gain, info.gain);
        assert_eq!(deserialized.peak, info.peak);
    }
}
