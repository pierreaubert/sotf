#[cfg(not(target_os = "ios"))]
use super::misc::fill_measurement_output;
#[cfg(not(target_os = "ios"))]
use super::types::MeasurementOutputConfig;
#[cfg(not(target_os = "ios"))]
use super::write::write_capture_pairs_to_ring;
use crate::signals::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Build an octave-scaled sweep surrounded by silence windows.
///
/// Returns `[pre_silence | sweep | post_silence]` as a flat `Vec<f32>`.
/// This is the core of the GD-Opt v2 Phase GD-1b signal path.
pub(super) fn build_octave_sweep_with_silence(
    start_freq: f32,
    end_freq: f32,
    amp: f32,
    bass_octave_duration_s: f32,
    pre_silence_s: f32,
    post_silence_s: f32,
    sample_rate: u32,
) -> Vec<f32> {
    // Clamp bass duration per the design contract.
    let bass_dur = bass_octave_duration_s.clamp(1.0, 10.0);

    // The minimum total sweep duration is one full second so the upper octaves
    // remain usable even with a very narrow frequency range.
    let min_sweep_dur = 1.0_f32;

    let sweep = gen_log_sweep_octave_scaled(
        start_freq,
        end_freq,
        amp,
        sample_rate,
        bass_dur,
        min_sweep_dur,
    );

    let pre_n = (pre_silence_s.max(0.0) * sample_rate as f32).round() as usize;
    let post_n = (post_silence_s.max(0.0) * sample_rate as f32).round() as usize;

    let total = pre_n + sweep.len() + post_n;
    let mut out = vec![0.0_f32; total];
    out[pre_n..pre_n + sweep.len()].copy_from_slice(&sweep);
    out
}

#[cfg(not(target_os = "ios"))]
pub(super) fn build_measurement_output_stream(
    device: &cpal::Device,
    output_config: &MeasurementOutputConfig,
    playback: Arc<Vec<f32>>,
    cursor: Arc<std::sync::atomic::AtomicUsize>,
    log_tag: &str,
) -> Result<cpal::Stream, String> {
    let config = cpal::StreamConfig {
        channels: output_config.channels,
        sample_rate: output_config.sample_rate,
        buffer_size: cpal::BufferSize::Default,
    };
    macro_rules! build_typed {
        ($sample_ty:ty) => {
            build_measurement_output_stream_typed::<$sample_ty>(
                device, &config, playback, cursor, log_tag,
            )
        };
    }
    match output_config.sample_format {
        cpal::SampleFormat::F32 => build_typed!(f32),
        cpal::SampleFormat::F64 => build_typed!(f64),
        cpal::SampleFormat::I8 => build_typed!(i8),
        cpal::SampleFormat::I16 => build_typed!(i16),
        cpal::SampleFormat::I24 => build_typed!(cpal::I24),
        cpal::SampleFormat::I32 => build_typed!(i32),
        cpal::SampleFormat::I64 => build_typed!(i64),
        cpal::SampleFormat::U8 => build_typed!(u8),
        cpal::SampleFormat::U16 => build_typed!(u16),
        cpal::SampleFormat::U24 => build_typed!(cpal::U24),
        cpal::SampleFormat::U32 => build_typed!(u32),
        cpal::SampleFormat::U64 => build_typed!(u64),
        other => Err(format!(
            "[{log_tag}] Unsupported measurement output sample format: {other:?}"
        )),
    }
}

#[cfg(not(target_os = "ios"))]
pub(super) fn build_measurement_output_stream_typed<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    playback: Arc<Vec<f32>>,
    cursor: Arc<std::sync::atomic::AtomicUsize>,
    log_tag: &str,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    use cpal::traits::DeviceTrait;

    let log_tag_owned = log_tag.to_string();
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| fill_measurement_output(data, &playback, &cursor),
            move |err| log::debug!("[{log_tag_owned}] Output stream error: {}", err),
            None,
        )
        .map_err(|e| {
            format!(
                "[{log_tag}] Failed to build {:?} output stream: {}",
                T::FORMAT,
                e
            )
        })
}

#[cfg(not(target_os = "ios"))]
#[allow(clippy::too_many_arguments)]
pub(super) fn build_measurement_input_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_format: cpal::SampleFormat,
    capture_producer: rtrb::Producer<(f32, f32)>,
    capture_count_callback: Arc<AtomicUsize>,
    capture_overruns_callback: Arc<AtomicUsize>,
    hw_ch: usize,
    input_ch_idx: usize,
    loopback_ch_idx: Option<usize>,
    log_tag: &str,
) -> Result<cpal::Stream, String> {
    macro_rules! build_typed {
        ($sample_ty:ty) => {
            build_measurement_input_stream_typed::<$sample_ty>(
                device,
                config,
                capture_producer,
                capture_count_callback,
                capture_overruns_callback,
                hw_ch,
                input_ch_idx,
                loopback_ch_idx,
                log_tag,
            )
        };
    }
    match sample_format {
        cpal::SampleFormat::F32 => build_typed!(f32),
        cpal::SampleFormat::F64 => build_typed!(f64),
        cpal::SampleFormat::I8 => build_typed!(i8),
        cpal::SampleFormat::I16 => build_typed!(i16),
        cpal::SampleFormat::I24 => build_typed!(cpal::I24),
        cpal::SampleFormat::I32 => build_typed!(i32),
        cpal::SampleFormat::I64 => build_typed!(i64),
        cpal::SampleFormat::U8 => build_typed!(u8),
        cpal::SampleFormat::U16 => build_typed!(u16),
        cpal::SampleFormat::U24 => build_typed!(cpal::U24),
        cpal::SampleFormat::U32 => build_typed!(u32),
        cpal::SampleFormat::U64 => build_typed!(u64),
        other => Err(format!(
            "[{log_tag}] Unsupported measurement input sample format: {other:?}"
        )),
    }
}

#[cfg(not(target_os = "ios"))]
#[allow(clippy::too_many_arguments)]
pub(super) fn build_measurement_input_stream_typed<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut capture_producer: rtrb::Producer<(f32, f32)>,
    capture_count_callback: Arc<AtomicUsize>,
    capture_overruns_callback: Arc<AtomicUsize>,
    hw_ch: usize,
    input_ch_idx: usize,
    loopback_ch_idx: Option<usize>,
    log_tag: &str,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    use cpal::traits::DeviceTrait;

    let log_tag_for_data = log_tag.to_string();
    let log_tag_for_error = log_tag.to_string();
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                let frames = data.len() / hw_ch;
                let written = write_capture_pairs_to_ring(
                    &mut capture_producer,
                    data,
                    hw_ch,
                    input_ch_idx,
                    loopback_ch_idx,
                );
                capture_count_callback.fetch_add(written, Ordering::Relaxed);
                if written < frames {
                    capture_overruns_callback.fetch_add(1, Ordering::Relaxed);
                    crate::rate_limited_log!(
                        warn,
                        5,
                        "[{log_tag_for_data}] Input capture ring buffer overrun"
                    );
                }
            },
            move |err| {
                crate::rate_limited_log!(
                    warn,
                    5,
                    "[{log_tag_for_error}] Input stream error: {}",
                    err
                )
            },
            None,
        )
        .map_err(|e| {
            format!(
                "[{log_tag}] Failed to build {:?} input stream: {}",
                T::FORMAT,
                e
            )
        })
}
