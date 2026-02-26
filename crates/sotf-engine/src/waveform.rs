pub use math_audio_dsp::waveform::{WAVEFORM_SAMPLES, compute_waveform};

use crate::decoder::{AudioDecoderResult, create_decoder};
use std::path::Path;

/// Analyze an audio file and compute a waveform representation.
///
/// Decodes the file, mixes to mono, then delegates to
/// [`math_audio_dsp::waveform::compute_waveform`].
pub fn analyze_waveform<P: AsRef<Path>>(path: P) -> AudioDecoderResult<Vec<u8>> {
    log::debug!(
        "[Waveform] Analyze: {}",
        path.as_ref().to_str().unwrap_or("unknown")
    );

    let mut decoder = create_decoder(path.as_ref())?;

    let mut mono_samples: Vec<f32> = Vec::new();

    while let Some(decoded) = decoder.decode_next()? {
        if decoded.is_empty() {
            continue;
        }
        let channels = decoded.spec.channels as usize;
        let samples = &decoded.samples;
        for frame_start in (0..samples.len()).step_by(channels) {
            let mut sum = 0.0f32;
            for ch in 0..channels {
                if frame_start + ch < samples.len() {
                    sum += samples[frame_start + ch];
                }
            }
            mono_samples.push(sum / channels as f32);
        }
    }

    Ok(compute_waveform(&mono_samples))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::AudioDecoderError;

    #[test]
    fn test_analyze_nonexistent_file() {
        let result = analyze_waveform("nonexistent_file.flac");
        assert!(matches!(result, Err(AudioDecoderError::FileNotFound(_))));
    }

    #[test]
    fn test_waveform_length() {
        assert_eq!(WAVEFORM_SAMPLES, 128);
    }
}
