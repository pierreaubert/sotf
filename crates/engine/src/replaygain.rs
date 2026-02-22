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

/// Extended ReplayGain data including EBU R128 gating block statistics.
/// Used for computing album-level gain by accumulating across tracks.
#[derive(Debug, Clone)]
pub struct ReplayGainTrackData {
    pub gain: f64,
    pub peak: f64,
    pub gating_block_count: u64,
    pub energy: f64,
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

/// Analyze an audio file and return extended ReplayGain data including
/// gating block count and energy, needed for album-level gain computation.
pub fn analyze_file_extended<P: AsRef<Path>>(path: P) -> AudioDecoderResult<ReplayGainTrackData> {
    let path = path.as_ref();
    let mut decoder = create_decoder(path)?;
    let spec = decoder.spec();
    let channels = spec.channels as u32;
    let sample_rate = spec.sample_rate;

    let mut ebur128 = EbuR128::new(channels, sample_rate, Mode::all()).map_err(|e| {
        AudioDecoderError::ConfigError(format!("Failed to create EBU R128 analyzer: {:?}", e))
    })?;

    while let Some(decoded) = decoder.decode_next()? {
        if decoded.is_empty() {
            continue;
        }
        ebur128.add_frames_f32(&decoded.samples).map_err(|e| {
            AudioDecoderError::DecodingFailed(format!("Failed to add frames to EBU R128: {:?}", e))
        })?;
    }

    let loudness = ebur128.loudness_global().map_err(|e| {
        AudioDecoderError::DecodingFailed(format!("Failed to calculate loudness: {:?}", e))
    })?;

    let mut peak = 0.0f64;
    for ch in 0..channels {
        let ch_peak = ebur128.sample_peak(ch).map_err(|e| {
            AudioDecoderError::DecodingFailed(format!("Failed to get peak for channel {}: {:?}", ch, e))
        })?;
        peak = peak.max(ch_peak);
    }

    let gain = REPLAYGAIN2_REFERENCE_LUFS - loudness;

    let (gating_block_count, energy) = ebur128.gating_block_count_and_energy().ok_or_else(|| {
        AudioDecoderError::DecodingFailed("Failed to get gating block count and energy".to_string())
    })?;

    Ok(ReplayGainTrackData {
        gain,
        peak,
        gating_block_count,
        energy,
    })
}

/// Compute album-level ReplayGain from accumulated per-track gating block data.
///
/// `tracks` contains `(peak, gating_block_count, energy)` for each track in the album.
/// Returns `(album_gain_db, album_peak)`.
pub fn compute_album_gain(tracks: &[(f64, u64, f64)]) -> Option<(f64, f64)> {
    if tracks.is_empty() {
        return None;
    }

    let mut total_blocks: u64 = 0;
    let mut total_energy: f64 = 0.0;
    let mut album_peak: f64 = 0.0;

    for &(peak, blocks, energy) in tracks {
        total_blocks += blocks;
        total_energy += energy;
        album_peak = album_peak.max(peak);
    }

    if total_blocks == 0 {
        return None;
    }

    let album_loudness = ebur128::energy_to_loudness(total_energy / total_blocks as f64);
    let album_gain = REPLAYGAIN2_REFERENCE_LUFS - album_loudness;

    Some((album_gain, album_peak))
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
