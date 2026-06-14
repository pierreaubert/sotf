use audioadapter_buffers::direct::SequentialSliceOfVecs;
use math_audio_dsp::audio_features;
use rubato::{Fft, FixedSync, Resampler};
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

    let mut resampler = Fft::<f32>::new(
        source_rate as usize,
        target_rate as usize,
        chunk_size,
        2,
        1,
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
