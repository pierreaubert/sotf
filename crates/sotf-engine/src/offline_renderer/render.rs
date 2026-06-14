use crate::decoder::core::DecodedAudio;
use crate::decoder::source::AudioSource;
use crate :: engine :: { build_plugin_host } ;
use std :: path :: { Path } ;
use super::offline_render_config::OfflineRenderConfig;
use super::render_progress::RenderProgress;
use super::timeline_render_state_guard::TimelineRenderStateGuard;
use super::types::OutputFormat;

/// Render audio offline at maximum CPU speed.
///
/// Decodes the source, processes through the plugin chain, and writes to the
/// output file. No real-time constraints — runs as fast as the CPU allows.
///
/// # Arguments
/// * `config` — Render configuration (source, output, plugins, format)
/// * `on_progress` — Optional callback invoked after each decoded chunk
pub fn render_offline(
    config: &OfflineRenderConfig,
    mut on_progress: Option<&mut dyn FnMut(&RenderProgress)>,
) -> Result<(), String> {
    // 1. Create decoder
    let mut decoder = crate::decoder::core::create_decoder_from_source(&config.source)
        .map_err(|e| format!("Failed to open source: {e}"))?;
    let spec = decoder.spec().clone();

    let sample_rate = config.output_sample_rate.unwrap_or(spec.sample_rate);
    let input_channels = spec.channels as usize;

    // 2. Build plugin host
    let (mut host, _warnings) = build_plugin_host(&config.plugins, sample_rate, input_channels)?;
    host.build()?;
    let output_channels = host.output_channels();

    // 3. Create WAV writer
    let wav_spec = match config.format {
        OutputFormat::Wav { bits_per_sample } => hound::WavSpec {
            channels: output_channels as u16,
            sample_rate,
            bits_per_sample: if bits_per_sample == 32 {
                32
            } else {
                bits_per_sample
            },
            sample_format: if bits_per_sample == 32 {
                hound::SampleFormat::Float
            } else {
                hound::SampleFormat::Int
            },
        },
    };
    let mut writer = hound::WavWriter::create(&config.output_path, wav_spec)
        .map_err(|e| format!("Failed to create output file: {e}"))?;

    // 4. Pre-allocate buffers
    let frame_size = config.frame_size;
    let mut decode_buf = DecodedAudio::new(spec.clone());
    let mut process_output = vec![0.0f32; frame_size * output_channels * 2];
    let mut frames_processed: u64 = 0;

    // 5. Decode → process → write loop
    loop {
        decode_buf.clear();
        let decoded_frames = decoder
            .decode_into(&mut decode_buf)
            .map_err(|e| format!("Decode error: {e}"))?;

        if decoded_frames == 0 {
            break;
        }

        let samples = &decode_buf.samples;

        // Process in frame_size chunks
        let mut offset = 0;
        while offset < decoded_frames {
            let chunk_frames = (decoded_frames - offset).min(frame_size);
            let in_start = offset * input_channels;
            let in_end = in_start + chunk_frames * input_channels;
            let out_len = chunk_frames * output_channels;

            if process_output.len() < out_len {
                process_output.resize(out_len, 0.0);
            }

            let actual_frames =
                host.process(&samples[in_start..in_end], &mut process_output[..out_len])?;

            // Write to WAV
            let write_samples = actual_frames * output_channels;
            match wav_spec.sample_format {
                hound::SampleFormat::Float => {
                    for &s in &process_output[..write_samples] {
                        writer
                            .write_sample(s.clamp(-1.0, 1.0))
                            .map_err(|e| format!("Write error: {e}"))?;
                    }
                }
                hound::SampleFormat::Int => {
                    let scale = ((1i64 << (wav_spec.bits_per_sample - 1)) - 1) as f32;
                    for &s in &process_output[..write_samples] {
                        let i = (s.clamp(-1.0, 1.0) * scale) as i32;
                        writer
                            .write_sample(i)
                            .map_err(|e| format!("Write error: {e}"))?;
                    }
                }
            }

            frames_processed += actual_frames as u64;
            offset += chunk_frames;
        }

        // Progress callback
        if let Some(ref mut cb) = on_progress {
            cb(&RenderProgress {
                frames_processed,
                total_frames: spec.total_frames,
            });
        }
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize output: {e}"))?;
    Ok(())
}

/// Convenience: render a file with no plugins (passthrough / format conversion).
pub fn render_passthrough(
    input_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    bits_per_sample: u16,
) -> Result<(), String> {
    let config = OfflineRenderConfig {
        source: AudioSource::File(input_path.as_ref().to_path_buf()),
        output_path: output_path.as_ref().to_path_buf(),
        format: OutputFormat::Wav { bits_per_sample },
        plugins: Vec::new(),
        frame_size: 1024,
        output_sample_rate: None,
    };
    render_offline(&config, None)
}

/// Render a Timeline to a WAV file at max CPU speed.
///
/// Processes the entire timeline from start to end (or loop if enabled)
/// and writes the output to the specified path.
pub fn render_timeline(
    timeline: &mut crate::timeline::Timeline,
    output_path: impl AsRef<Path>,
    format: &OutputFormat,
    mut on_progress: Option<&mut dyn FnMut(&RenderProgress)>,
) -> Result<(), String> {
    let sr = timeline.transport.sample_rate;
    let ch = timeline.output_channels;
    let nf = timeline.frame_size;
    let duration = timeline.duration_samples();

    let wav_spec = match format {
        OutputFormat::Wav { bits_per_sample } => hound::WavSpec {
            channels: ch as u16,
            sample_rate: sr,
            bits_per_sample: if *bits_per_sample == 32 {
                32
            } else {
                *bits_per_sample
            },
            sample_format: if *bits_per_sample == 32 {
                hound::SampleFormat::Float
            } else {
                hound::SampleFormat::Int
            },
        },
    };

    let mut writer = hound::WavWriter::create(output_path.as_ref(), wav_spec)
        .map_err(|e| format!("Failed to create output: {e}"))?;

    // Disable looping for bounce — we render exactly once through the timeline.
    let mut render_state = TimelineRenderStateGuard::new(timeline);

    let mut output = vec![0.0f32; nf * ch];
    let mut frames_processed: u64 = 0;

    while frames_processed < duration {
        let frames = render_state.timeline.process(&mut output)?;
        let write_samples = frames * ch;

        match wav_spec.sample_format {
            hound::SampleFormat::Float => {
                for &s in &output[..write_samples] {
                    writer
                        .write_sample(s.clamp(-1.0, 1.0))
                        .map_err(|e| format!("Write error: {e}"))?;
                }
            }
            hound::SampleFormat::Int => {
                let scale = ((1i64 << (wav_spec.bits_per_sample - 1)) - 1) as f32;
                for &s in &output[..write_samples] {
                    let i = (s.clamp(-1.0, 1.0) * scale) as i32;
                    writer
                        .write_sample(i)
                        .map_err(|e| format!("Write error: {e}"))?;
                }
            }
        }

        frames_processed += frames as u64;
        if let Some(ref mut cb) = on_progress {
            cb(&RenderProgress {
                frames_processed,
                total_frames: Some(duration),
            });
        }
    }

    writer
        .finalize()
        .map_err(|e| format!("Finalize error: {e}"))?;
    Ok(())
}

