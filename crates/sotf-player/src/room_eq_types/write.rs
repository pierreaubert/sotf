use super::types::read_first_wav_channel_f32;
use std::path::Path;

pub(super) fn write_stereo_ir_wav(
    path: &Path,
    left: &[f32],
    right: &[f32],
    sample_rate: u32,
) -> Result<(), String> {
    let len = left.len().max(right.len());
    let mut interleaved = Vec::with_capacity(len * 2);
    for idx in 0..len {
        interleaved.push(left.get(idx).copied().unwrap_or(0.0));
        interleaved.push(right.get(idx).copied().unwrap_or(0.0));
    }
    sotf_audio::signal_recorder::write_wav_file(path, &interleaved, sample_rate, 2)
        .map_err(|e| format!("failed to write CTC IR WAV '{}': {}", path.display(), e))
}

pub(super) fn write_stereo_wav_from_mono_wavs(
    path: &Path,
    left_path: &Path,
    right_path: &Path,
    expected_sample_rate: u32,
) -> Result<(), String> {
    let (left, left_sample_rate) = read_first_wav_channel_f32(left_path)?;
    let (right, right_sample_rate) = read_first_wav_channel_f32(right_path)?;
    if left_sample_rate != expected_sample_rate || right_sample_rate != expected_sample_rate {
        return Err(format!(
            "CTC raw sweep sample-rate mismatch: left={}Hz, right={}Hz, expected={}Hz",
            left_sample_rate, right_sample_rate, expected_sample_rate
        ));
    }
    write_stereo_ir_wav(path, &left, &right, expected_sample_rate)
}
