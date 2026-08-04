//! Assertions shared by deterministic audio and realtime-path tests.

/// Assert that an interleaved audio buffer contains only finite samples.
pub fn assert_finite_audio(samples: &[f32]) {
    if let Some((index, sample)) = samples
        .iter()
        .copied()
        .enumerate()
        .find(|(_, sample)| !sample.is_finite())
    {
        panic!("non-finite audio sample at index {index}: {sample}");
    }
}

/// Assert that an interleaved buffer contains a whole number of frames.
pub fn assert_frame_aligned(samples: &[f32], channels: usize) {
    assert!(channels > 0, "audio channel count must be positive");
    assert_eq!(
        samples.len() % channels,
        0,
        "{} samples are not aligned to {channels} channels",
        samples.len()
    );
}

/// Assert that all samples are inside the audio contract range with tolerance.
pub fn assert_audio_range(samples: &[f32], tolerance: f32) {
    assert!(tolerance.is_finite() && tolerance >= 0.0);
    assert_finite_audio(samples);
    let limit = 1.0 + tolerance;
    if let Some((index, sample)) = samples
        .iter()
        .copied()
        .enumerate()
        .find(|(_, sample)| sample.abs() > limit)
    {
        panic!("audio sample at index {index} exceeds {limit}: {sample}");
    }
}

/// Assert that two audio buffers have equal length and bounded sample error.
pub fn assert_audio_close(actual: &[f32], expected: &[f32], tolerance: f32) {
    assert_eq!(actual.len(), expected.len(), "audio buffer lengths differ");
    assert!(tolerance.is_finite() && tolerance >= 0.0);
    assert_finite_audio(actual);
    assert_finite_audio(expected);
    if let Some((index, (actual, expected))) = actual
        .iter()
        .copied()
        .zip(expected.iter().copied())
        .enumerate()
        .find(|(_, (actual, expected))| (actual - expected).abs() > tolerance)
    {
        panic!(
            "audio differs at index {index}: actual={actual}, expected={expected}, tolerance={tolerance}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_finite_aligned_audio() {
        let samples = [0.0, 0.5, -0.5, 1.0];
        assert_finite_audio(&samples);
        assert_frame_aligned(&samples, 2);
        assert_audio_range(&samples, 0.0);
        assert_audio_close(&samples, &samples, 0.0);
    }

    #[test]
    #[should_panic(expected = "non-finite audio sample")]
    fn rejects_non_finite_audio() {
        assert_finite_audio(&[0.0, f32::NAN]);
    }

    #[test]
    #[should_panic(expected = "not aligned")]
    fn rejects_partial_frame() {
        assert_frame_aligned(&[0.0, 0.0, 0.0], 2);
    }
}
