use math_audio_dsp::audio_features;
use rubato::audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{Fft, FixedSync, Resampler, WindowFunction};
use sotf_audio::decoder::create_decoder;
use std::path::Path;

/// Number of audio analysis features stored
pub const BLISS_FEATURES_COUNT: usize = audio_features::FEATURES_COUNT;

/// Analysis sample rate (matches bliss convention)
pub(super) const ANALYSIS_SAMPLE_RATE: u32 = 22050;

/// Decode an audio file to mono 22050 Hz samples for analysis
pub(super) fn decode_for_analysis(path: &Path) -> Result<Vec<f32>, String> {
    let mut decoder = create_decoder(path).map_err(|e| e.to_string())?;

    let spec = decoder.spec().clone();
    let channels = spec.channels as usize;
    let source_sample_rate = spec.sample_rate;

    let mut all_samples: Vec<f32> = Vec::new();

    loop {
        match decoder.decode_next() {
            Ok(Some(audio)) => {
                all_samples.extend_from_slice(&audio.samples);
            }
            Ok(None) => break,
            Err(e) => return Err(e.to_string()),
        }
    }

    if all_samples.is_empty() {
        return Err("No audio samples decoded".to_string());
    }

    // Convert to mono
    let mono_samples: Vec<f32> = if channels == 1 {
        all_samples
    } else {
        let frame_count = all_samples.len() / channels;
        (0..frame_count)
            .map(|i| {
                let start = i * channels;
                let sum: f32 = (0..channels).map(|ch| all_samples[start + ch]).sum();
                sum / channels as f32
            })
            .collect()
    };

    // Resample to ANALYSIS_SAMPLE_RATE if needed
    if source_sample_rate == ANALYSIS_SAMPLE_RATE {
        Ok(mono_samples)
    } else {
        resample(&mono_samples, source_sample_rate, ANALYSIS_SAMPLE_RATE)
    }
}

/// Resample audio to the target sample rate using rubato
pub(super) fn resample(
    samples: &[f32],
    source_rate: u32,
    target_rate: u32,
) -> Result<Vec<f32>, String> {
    if source_rate == target_rate {
        return Ok(samples.to_vec());
    }

    let resample_ratio = target_rate as f64 / source_rate as f64;
    let chunk_size = 1024;

    // Match Rubato 1's two sub-chunks and Blackman-Harris filter explicitly.
    let mut resampler = Fft::<f32>::new_custom(
        source_rate as usize,
        target_rate as usize,
        chunk_size,
        2,
        1,
        WindowFunction::BlackmanHarris2,
        FixedSync::Both,
    )
    .map_err(|e| format!("Failed to create resampler: {e}"))?;

    let input_frames_needed = resampler.input_frames_next();
    let output_frames_per_chunk = resampler.output_frames_next();
    let estimated_output_len =
        ((samples.len() as f64 * resample_ratio) as usize) + output_frames_per_chunk;
    let mut output = Vec::with_capacity(estimated_output_len);
    let mut output_channels = vec![vec![0.0f32; output_frames_per_chunk]];

    let mut pos = 0;
    while pos < samples.len() {
        let end = (pos + input_frames_needed).min(samples.len());
        let chunk = &samples[pos..end];

        let input_chunk: Vec<f32> = if chunk.len() < input_frames_needed {
            let mut padded = chunk.to_vec();
            padded.resize(input_frames_needed, 0.0);
            padded
        } else {
            chunk.to_vec()
        };

        let input_channels = vec![input_chunk];
        let input_adapter = SequentialSliceOfVecs::new(&input_channels, 1, input_frames_needed)
            .map_err(|e| format!("Input adapter error: {e}"))?;
        let mut output_adapter =
            SequentialSliceOfVecs::new_mut(&mut output_channels, 1, output_frames_per_chunk)
                .map_err(|e| format!("Output adapter error: {e}"))?;

        match resampler.process_into_buffer(&input_adapter, &mut output_adapter, None) {
            Ok((_, written)) => {
                output.extend_from_slice(&output_channels[0][..written]);
            }
            Err(e) => {
                return Err(format!("Resampling error: {e}"));
            }
        }

        pos += input_frames_needed;
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::resample;
    use rubato::{Fft, FixedSync, Resampler, WindowFunction};

    #[test]
    fn resampling_preserves_duration_for_partial_final_chunk() {
        let samples = vec![0.25_f32; 44_100 + 37];
        let output = resample(&samples, 44_100, 22_050).unwrap();
        let reference = Fft::<f32>::new_custom(
            44_100,
            22_050,
            1024,
            2,
            1,
            WindowFunction::BlackmanHarris2,
            FixedSync::Both,
        )
        .unwrap();
        let chunks = samples.len().div_ceil(reference.input_frames_next());
        assert_eq!(output.len(), chunks * reference.output_frames_next());
        assert!(output.len() > 22_069); // The final input chunk remains zero-padded.
        assert!(output.iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn resampling_keeps_a_tone_at_its_physical_frequency() {
        let source_rate = 44_100;
        let target_rate = 22_050;
        let tone_hz = 1_000.0_f32;
        let samples: Vec<f32> = (0..source_rate)
            .map(|index| {
                (std::f32::consts::TAU * tone_hz * index as f32 / source_rate as f32).sin()
            })
            .collect();
        let output = resample(&samples, source_rate, target_rate).unwrap();
        let middle = &output[1_000..output.len() - 1_000];
        let rms =
            (middle.iter().map(|value| value * value).sum::<f32>() / middle.len() as f32).sqrt();
        assert!((rms - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.04);
        let crossings = middle
            .windows(2)
            .filter(|pair| pair[0] <= 0.0 && pair[1] > 0.0)
            .count();
        let measured_hz = crossings as f32 * target_rate as f32 / middle.len() as f32;
        assert!((measured_hz - tone_hz).abs() < 5.0);
    }
}
