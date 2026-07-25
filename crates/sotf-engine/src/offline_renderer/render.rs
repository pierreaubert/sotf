use super::offline_render_config::OfflineRenderConfig;
use super::render_progress::RenderProgress;
use super::timeline_render_state_guard::TimelineRenderStateGuard;
use super::types::OutputFormat;
use crate::decoder::core::DecodedAudio;
use crate::decoder::source::AudioSource;
use crate::engine::build_plugin_host;
use crate::engine::output_dither::TpdfDither;
use hound::{SampleFormat, WavSpec, WavWriter};
use sotf_plugins::{DawHost, Plugin, ProcessContext, ResamplerPlugin};
use std::io::{Seek, Write};
use std::path::Path;

const OFFLINE_DITHER_SEED: u64 = 0x6f66_666c_696e_655f;

struct HostOutputState {
    latency_remaining: usize,
    frames_written: usize,
}

impl HostOutputState {
    fn new(latency_samples: usize) -> Self {
        Self {
            latency_remaining: latency_samples,
            frames_written: 0,
        }
    }
}

/// Render audio offline at maximum CPU speed.
pub fn render_offline(
    config: &OfflineRenderConfig,
    mut on_progress: Option<&mut dyn FnMut(&RenderProgress)>,
) -> Result<(), String> {
    if config.frame_size == 0 {
        return Err("Offline render frame_size must be greater than zero".to_string());
    }

    let mut decoder = crate::decoder::core::create_decoder_from_source(&config.source)
        .map_err(|e| format!("Failed to open source: {e}"))?;
    let source_spec = decoder.spec().clone();
    let source_rate = source_spec.sample_rate;
    let output_rate = config.output_sample_rate.unwrap_or(source_rate);
    let input_channels = source_spec.channels as usize;
    if output_rate == 0 {
        return Err("Offline render output sample rate must be greater than zero".to_string());
    }

    let (mut host, _warnings) = build_plugin_host(&config.plugins, output_rate, input_channels)
        .map_err(|diagnostic| diagnostic.message)?;
    host.build()?;
    let output_channels = host.output_channels();
    let mut host_output = HostOutputState::new(host.total_latency_samples());

    let wav_spec = wav_spec(&config.format, output_channels, output_rate)?;
    let mut writer = WavWriter::create(&config.output_path, wav_spec)
        .map_err(|e| format!("Failed to create output file: {e}"))?;
    let mut dither = TpdfDither::new(OFFLINE_DITHER_SEED);

    let mut resampler = if source_rate == output_rate {
        None
    } else {
        Some(ResamplerPlugin::new(
            input_channels,
            source_rate,
            output_rate,
            config.frame_size,
        )?)
    };
    let resampler_delay = resampler
        .as_ref()
        .map_or(0, ResamplerPlugin::output_delay_frames);
    let mut resampler_delay_remaining = resampler_delay;
    let mut resampled_frames_written = 0usize;

    let mut decode_buf = DecodedAudio::new(source_spec.clone());
    let mut resample_output = Vec::<f32>::new();
    let mut process_output = Vec::<f32>::new();
    let mut source_frames_decoded = 0usize;
    let progress_total = source_spec
        .total_frames
        .map(|frames| ((frames as f64 * output_rate as f64 / source_rate as f64).ceil()) as u64);

    loop {
        decode_buf.clear();
        let decoded_frames = decoder
            .decode_into(&mut decode_buf)
            .map_err(|e| format!("Decode error: {e}"))?;
        if decoded_frames == 0 {
            break;
        }
        source_frames_decoded = source_frames_decoded.saturating_add(decoded_frames);

        if let Some(resampler) = resampler.as_mut() {
            let max_frames = resampler.output_frames_for_input(decoded_frames);
            resample_output.resize(max_frames.saturating_mul(input_channels), 0.0);
            let produced = resampler.process(
                &decode_buf.samples,
                &mut resample_output,
                &ProcessContext::new(source_rate, decoded_frames),
            )?;
            process_resampled_frames(
                &resample_output[..produced * input_channels],
                produced,
                input_channels,
                &mut resampler_delay_remaining,
                usize::MAX,
                &mut host,
                config.frame_size,
                &mut process_output,
                &mut writer,
                wav_spec,
                &mut dither,
                &mut resampled_frames_written,
                &mut host_output,
            )?;
        } else {
            process_and_write(
                &decode_buf.samples,
                decoded_frames,
                input_channels,
                &mut host,
                config.frame_size,
                &mut process_output,
                &mut writer,
                wav_spec,
                &mut dither,
                &mut host_output,
                usize::MAX,
            )?;
        }

        if let Some(ref mut callback) = on_progress {
            callback(&RenderProgress {
                frames_processed: host_output.frames_written as u64,
                total_frames: progress_total,
            });
        }
    }

    if let Some(resampler) = resampler.as_mut() {
        let expected_frames = ((source_frames_decoded as f64 * output_rate as f64
            / source_rate as f64)
            .ceil()) as usize;
        let silence = vec![0.0f32; config.frame_size * input_channels];
        while resampled_frames_written < expected_frames {
            let max_frames = resampler.output_frames_for_input(config.frame_size);
            resample_output.resize(max_frames.saturating_mul(input_channels), 0.0);
            let produced = resampler.process(
                &silence,
                &mut resample_output,
                &ProcessContext::new(source_rate, config.frame_size),
            )?;
            if produced == 0 {
                continue;
            }
            let remaining = expected_frames - resampled_frames_written;
            process_resampled_frames(
                &resample_output[..produced * input_channels],
                produced,
                input_channels,
                &mut resampler_delay_remaining,
                remaining,
                &mut host,
                config.frame_size,
                &mut process_output,
                &mut writer,
                wav_spec,
                &mut dither,
                &mut resampled_frames_written,
                &mut host_output,
            )?;
        }
    }

    let program_frames = if resampler.is_some() {
        resampled_frames_written
    } else {
        source_frames_decoded
    };
    drain_host_to_duration(
        &mut host,
        input_channels,
        config.frame_size,
        &mut process_output,
        &mut writer,
        wav_spec,
        &mut dither,
        &mut host_output,
        program_frames,
    )?;

    if let Some(ref mut callback) = on_progress {
        callback(&RenderProgress {
            frames_processed: host_output.frames_written as u64,
            total_frames: progress_total,
        });
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize output: {e}"))
}

#[allow(clippy::too_many_arguments)]
fn process_resampled_frames<W: Write + Seek>(
    samples: &[f32],
    frames: usize,
    channels: usize,
    delay_remaining: &mut usize,
    maximum_frames: usize,
    host: &mut DawHost,
    frame_size: usize,
    process_output: &mut Vec<f32>,
    writer: &mut WavWriter<W>,
    wav_spec: WavSpec,
    dither: &mut TpdfDither,
    frames_written: &mut usize,
    host_output: &mut HostOutputState,
) -> Result<(), String> {
    let skipped = frames.min(*delay_remaining);
    *delay_remaining -= skipped;
    let available = frames.saturating_sub(skipped).min(maximum_frames);
    if available == 0 {
        return Ok(());
    }
    let start = skipped * channels;
    let end = start + available * channels;
    process_and_write(
        &samples[start..end],
        available,
        channels,
        host,
        frame_size,
        process_output,
        writer,
        wav_spec,
        dither,
        host_output,
        usize::MAX,
    )?;
    *frames_written = frames_written.saturating_add(available);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_and_write<W: Write + Seek>(
    input: &[f32],
    input_frames: usize,
    input_channels: usize,
    host: &mut DawHost,
    frame_size: usize,
    process_output: &mut Vec<f32>,
    writer: &mut WavWriter<W>,
    wav_spec: WavSpec,
    dither: &mut TpdfDither,
    state: &mut HostOutputState,
    maximum_total_frames: usize,
) -> Result<usize, String> {
    let output_channels = host.output_channels();
    let mut offset = 0usize;
    let mut total_written = 0usize;
    while offset < input_frames {
        let chunk_frames = (input_frames - offset).min(frame_size);
        let input_start = offset * input_channels;
        let input_end = input_start + chunk_frames * input_channels;
        let output_capacity = host
            .output_frames_for_input(chunk_frames)
            .max(chunk_frames)
            .saturating_mul(output_channels);
        process_output.resize(output_capacity, 0.0);
        let actual_frames = host.process(
            &input[input_start..input_end],
            process_output.as_mut_slice(),
        )?;
        let skipped = actual_frames.min(state.latency_remaining);
        state.latency_remaining -= skipped;
        let available = actual_frames
            .saturating_sub(skipped)
            .min(maximum_total_frames.saturating_sub(state.frames_written));
        if available > 0 {
            let start = skipped * output_channels;
            let end = start + available * output_channels;
            write_wav_samples(writer, wav_spec, &process_output[start..end], dither)?;
            state.frames_written = state.frames_written.saturating_add(available);
            total_written = total_written.saturating_add(available);
        }
        offset += chunk_frames;
    }
    Ok(total_written)
}

#[allow(clippy::too_many_arguments)]
fn drain_host_to_duration<W: Write + Seek>(
    host: &mut DawHost,
    input_channels: usize,
    frame_size: usize,
    process_output: &mut Vec<f32>,
    writer: &mut WavWriter<W>,
    wav_spec: WavSpec,
    dither: &mut TpdfDither,
    state: &mut HostOutputState,
    target_frames: usize,
) -> Result<(), String> {
    if state.frames_written >= target_frames {
        return Ok(());
    }

    let silence = vec![0.0f32; frame_size * input_channels];
    let frames_to_flush = state
        .latency_remaining
        .saturating_add(target_frames - state.frames_written);
    let maximum_blocks = frames_to_flush.div_ceil(frame_size).saturating_add(8);
    for _ in 0..maximum_blocks {
        process_and_write(
            &silence,
            frame_size,
            input_channels,
            host,
            frame_size,
            process_output,
            writer,
            wav_spec,
            dither,
            state,
            target_frames,
        )?;
        if state.frames_written >= target_frames {
            return Ok(());
        }
    }

    Err(format!(
        "Plugin host did not drain to the requested {target_frames} frames (wrote {})",
        state.frames_written
    ))
}

fn wav_spec(format: &OutputFormat, channels: usize, sample_rate: u32) -> Result<WavSpec, String> {
    let channels = u16::try_from(channels)
        .map_err(|_| format!("Offline output channel count {channels} exceeds WAV limits"))?;
    match *format {
        OutputFormat::Wav { bits_per_sample } => match bits_per_sample {
            16 | 24 => Ok(WavSpec {
                channels,
                sample_rate,
                bits_per_sample,
                sample_format: SampleFormat::Int,
            }),
            32 => Ok(WavSpec {
                channels,
                sample_rate,
                bits_per_sample,
                sample_format: SampleFormat::Float,
            }),
            other => Err(format!(
                "Unsupported WAV depth {other}; expected 16-bit, 24-bit, or 32-bit float"
            )),
        },
    }
}

fn write_wav_samples<W: Write + Seek>(
    writer: &mut WavWriter<W>,
    wav_spec: WavSpec,
    samples: &[f32],
    dither: &mut TpdfDither,
) -> Result<(), String> {
    match (wav_spec.sample_format, wav_spec.bits_per_sample) {
        (SampleFormat::Float, 32) => {
            for &sample in samples {
                writer
                    .write_sample(sample)
                    .map_err(|e| format!("Write error: {e}"))?;
            }
        }
        (SampleFormat::Int, 16) => {
            for &sample in samples {
                writer
                    .write_sample(dither.quantize_signed(sample, 16) as i16)
                    .map_err(|e| format!("Write error: {e}"))?;
            }
        }
        (SampleFormat::Int, 24) => {
            for &sample in samples {
                writer
                    .write_sample(dither.quantize_signed(sample, 24))
                    .map_err(|e| format!("Write error: {e}"))?;
            }
        }
        _ => return Err("Invalid offline WAV sample format".to_string()),
    }
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

/// Render a timeline to a WAV file at maximum CPU speed.
pub fn render_timeline(
    timeline: &mut crate::timeline::Timeline,
    output_path: impl AsRef<Path>,
    format: &OutputFormat,
    mut on_progress: Option<&mut dyn FnMut(&RenderProgress)>,
) -> Result<(), String> {
    let sample_rate = timeline.transport.sample_rate;
    let channels = timeline.output_channels;
    let frame_size = timeline.frame_size;
    let duration = timeline.duration_samples();
    let mut latency_remaining = timeline.output_latency_samples();
    let wav_spec = wav_spec(format, channels, sample_rate)?;
    let mut writer = WavWriter::create(output_path.as_ref(), wav_spec)
        .map_err(|e| format!("Failed to create output: {e}"))?;
    let mut dither = TpdfDither::new(OFFLINE_DITHER_SEED);
    let render_state = TimelineRenderStateGuard::new(timeline);
    let mut output = vec![0.0f32; frame_size * channels];
    let mut frames_processed = 0u64;

    while frames_processed < duration {
        let produced = render_state.timeline.process(&mut output)?;
        if produced == 0 {
            return Err("Timeline renderer made no progress".to_string());
        }
        let skipped = produced.min(latency_remaining);
        latency_remaining -= skipped;
        let remaining = usize::try_from(duration - frames_processed).unwrap_or(usize::MAX);
        let written = produced.saturating_sub(skipped).min(remaining);
        let start = skipped * channels;
        let end = start + written * channels;
        write_wav_samples(&mut writer, wav_spec, &output[start..end], &mut dither)?;
        frames_processed += written as u64;
        if let Some(ref mut callback) = on_progress {
            callback(&RenderProgress {
                frames_processed,
                total_frames: Some(duration),
            });
        }
    }

    writer
        .finalize()
        .map_err(|e| format!("Finalize error: {e}"))
}
