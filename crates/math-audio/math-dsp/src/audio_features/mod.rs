//! Audio feature extraction for music similarity analysis.
//!
//! Pure Rust replacement for bliss-audio, producing a compatible 23-element
//! feature vector:
//!
//! | Index | Feature |
//! |-------|---------|
//! | 0 | Tempo (BPM) |
//! | 1 | Zero-crossing rate |
//! | 2-3 | Spectral centroid (mean, std) |
//! | 4-5 | Spectral rolloff (mean, std) |
//! | 6-7 | Spectral flatness (mean, std) |
//! | 8-9 | Loudness (mean, std) |
//! | 10-22 | Chroma interval features (13) |

pub mod chroma;
pub mod loudness;
pub mod spectral;
pub mod tempo;
pub mod utils;
pub mod zcr;

/// Number of features in the analysis vector (bliss v2 compatible).
pub const FEATURES_COUNT: usize = 23;

/// Minimum number of samples required for analysis (largest FFT window).
pub const MIN_SAMPLES: usize = 8192;

/// Error type for audio feature analysis.
#[derive(Debug, Clone)]
pub enum AnalysisError {
    TooShort,
    ChromaError(String),
    ThreadPanic(String),
}

impl std::fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalysisError::TooShort => write!(
                f,
                "audio too short for analysis (need >= {MIN_SAMPLES} samples)"
            ),
            AnalysisError::ChromaError(s) => write!(f, "chroma analysis error: {s}"),
            AnalysisError::ThreadPanic(s) => write!(f, "thread panic: {s}"),
        }
    }
}

impl std::error::Error for AnalysisError {}

/// Analyze audio samples and return a 23-element feature vector.
///
/// Input should be mono, 22050 Hz samples (same as bliss-audio requirements).
/// The returned vector is ordered identically to bliss v2 Analysis for database compatibility.
pub fn analyze_audio_features(
    samples: &[f32],
    sample_rate: u32,
) -> Result<Vec<f32>, AnalysisError> {
    if samples.len() < MIN_SAMPLES {
        return Err(AnalysisError::TooShort);
    }

    // Run descriptors in parallel using scoped threads
    std::thread::scope(|s| {
        let child_tempo = s.spawn(|| tempo::compute_tempo(samples, sample_rate));

        let child_zcr = s.spawn(|| zcr::compute_zcr(samples));

        let child_spectral = s.spawn(|| spectral::compute_spectral_features(samples, sample_rate));

        let child_loudness = s.spawn(|| loudness::compute_loudness(samples));

        let child_chroma = s.spawn(|| chroma::compute_chroma_features(samples, sample_rate));

        let tempo_val = child_tempo
            .join()
            .map_err(|e| AnalysisError::ThreadPanic(format!("{e:?}")))?;
        let zcr_val = child_zcr
            .join()
            .map_err(|e| AnalysisError::ThreadPanic(format!("{e:?}")))?;
        let spectral_vals = child_spectral
            .join()
            .map_err(|e| AnalysisError::ThreadPanic(format!("{e:?}")))?;
        let loudness_vals = child_loudness
            .join()
            .map_err(|e| AnalysisError::ThreadPanic(format!("{e:?}")))?;
        let chroma_vals = child_chroma
            .join()
            .map_err(|e| AnalysisError::ThreadPanic(format!("{e:?}")))?
            .map_err(|e| AnalysisError::ChromaError(e.0))?;

        // Assemble in bliss order:
        // [tempo, zcr, centroid(2), rolloff(2), flatness(2), loudness(2), chroma(13)]
        let mut result = Vec::with_capacity(FEATURES_COUNT);
        result.push(tempo_val);
        result.push(zcr_val);
        result.extend_from_slice(&spectral_vals); // 6 values: centroid(2), rolloff(2), flatness(2)
        result.extend_from_slice(&loudness_vals); // 2 values
        result.extend_from_slice(&chroma_vals); // 13 values

        assert_eq!(result.len(), FEATURES_COUNT);
        Ok(result)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_too_short() {
        let samples = vec![0.0; 100];
        assert!(analyze_audio_features(&samples, 22050).is_err());
    }

    #[test]
    fn test_analyze_features_count() {
        // Generate a simple tone long enough
        let sr = 22050u32;
        let signal: Vec<f32> = (0..sr as usize * 5)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin())
            .collect();

        let features = analyze_audio_features(&signal, sr).unwrap();
        assert_eq!(features.len(), FEATURES_COUNT);

        // All values should be in reasonable range [-1, 1]
        for (i, &f) in features.iter().enumerate() {
            assert!(
                (-1.5..=1.5).contains(&f),
                "feature[{i}] = {f} out of expected range"
            );
        }
    }

    #[test]
    fn test_analyze_silence() {
        // Silence should not crash
        let samples = vec![0.0; 22050 * 3];
        let features = analyze_audio_features(&samples, 22050).unwrap();
        assert_eq!(features.len(), FEATURES_COUNT);
    }

    #[test]
    fn test_thread_panic_error_format() {
        let err = AnalysisError::ThreadPanic("worker crashed".to_string());
        assert_eq!(format!("{err}"), "thread panic: worker crashed");
    }

    #[test]
    fn test_join_result_maps_panic_to_thread_panic() {
        let result: std::thread::Result<i32> = std::panic::catch_unwind(|| panic!("boom"));
        let mapped = result.map_err(|e| AnalysisError::ThreadPanic(format!("{e:?}")));
        assert!(matches!(mapped, Err(AnalysisError::ThreadPanic(_))));
    }
}
