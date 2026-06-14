use super::consts::DEFAULT_AUXILIARY_SIGNAL_LEVEL_DB;
use super::measurement::measurement_amplitude_from_level_db;
use super::types::BassAnchorResults;
use super::types::CancelFlag;
use super::types::SplCalibrationResult;
use super::types::analyze_bass_anchor_recording;
#[cfg(not(target_os = "ios"))]
use super::types::play_per_channel_and_record_mono;
use crate::signals::*;
use hound::{SampleFormat, WavSpec, WavWriter};

/// Run the bass-anchor capture across all output channels with
/// steady-state lock-in detection. See `run_bass_anchor_with_recording`
/// for the persisting variant.
///
/// # Arguments
/// * `channel_indices` - Output channel indices to probe (0-based)
/// * `channel_names`   - Human-readable name for each channel
/// * `sample_rate`     - Desired playback / capture sample rate in Hz
/// * `bass_freq_hz`    - Steady-state tone frequency in Hz
/// * `bass_duration_s` - Tone duration in seconds (≥ 2 · `fade_ms`)
/// * `fade_ms`         - Half-Hann fade-in / fade-out length in ms
/// * `num_windows`     - Sub-window count for circular-mean / std analysis
/// * `silence_duration_ms` - Silence gap between channels in ms
/// * `output_device_name`  - Playback device (None = default)
/// * `input_device_name`   - Recording device (None = default)
/// * `input_channel`       - Mic input channel index
/// * `loopback_input_channel` - Optional second input that captures the
///   raw playback signal. When `Some`, the per-channel reported phase
///   is `phase_mic − phase_loopback`, which cancels DAC delay / cpal
///   pre-roll / clock skew. Reusing `RecordingDeviceConfig.ctc_loopback_input_channel`
///   keeps the wiring identical to the CTC raw-sweep path.
#[cfg(not(target_os = "ios"))]
#[allow(clippy::too_many_arguments)]
pub fn run_bass_anchor(
    channel_indices: &[u16],
    channel_names: &[String],
    sample_rate: u32,
    bass_freq_hz: f32,
    bass_duration_s: f32,
    fade_ms: f32,
    num_windows: u16,
    silence_duration_ms: f32,
    output_device_name: Option<&str>,
    input_device_name: Option<&str>,
    input_channel: u16,
    loopback_input_channel: Option<u16>,
    cancel: Option<CancelFlag>,
) -> Result<BassAnchorResults, String> {
    let (results, _recorded, _loopback, _input_sr) = run_bass_anchor_core(
        channel_indices,
        channel_names,
        sample_rate,
        bass_freq_hz,
        bass_duration_s,
        fade_ms,
        num_windows,
        silence_duration_ms,
        output_device_name,
        input_device_name,
        input_channel,
        loopback_input_channel,
        DEFAULT_AUXILIARY_SIGNAL_LEVEL_DB,
        cancel.as_ref(),
    )?;
    Ok(results)
}

/// Run the bass-anchor capture and persist the raw mic recording.
///
/// Behaves like [`run_bass_anchor`] otherwise. When
/// `loopback_input_channel` is `Some`, the recording WAV is written as
/// a 2-channel f32 file (channel 0 = mic, channel 1 = loopback) so the
/// session can be re-analysed offline with
/// [`analyze_bass_anchor_recording`]; otherwise it is single-channel.
#[cfg(not(target_os = "ios"))]
#[allow(clippy::too_many_arguments)]
pub fn run_bass_anchor_with_recording(
    channel_indices: &[u16],
    channel_names: &[String],
    sample_rate: u32,
    bass_freq_hz: f32,
    bass_duration_s: f32,
    fade_ms: f32,
    num_windows: u16,
    silence_duration_ms: f32,
    output_device_name: Option<&str>,
    input_device_name: Option<&str>,
    input_channel: u16,
    loopback_input_channel: Option<u16>,
    recording_wav_path: &std::path::Path,
    signal_level_db: f32,
    cancel: Option<CancelFlag>,
) -> Result<BassAnchorResults, String> {
    let (results, recorded, loopback_recorded, input_sr) = run_bass_anchor_core(
        channel_indices,
        channel_names,
        sample_rate,
        bass_freq_hz,
        bass_duration_s,
        fade_ms,
        num_windows,
        silence_duration_ms,
        output_device_name,
        input_device_name,
        input_channel,
        loopback_input_channel,
        signal_level_db,
        cancel.as_ref(),
    )?;

    let channels_out: u16 = if loopback_recorded.is_some() { 2 } else { 1 };
    let spec = WavSpec {
        channels: channels_out,
        sample_rate: input_sr,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    let mut writer = WavWriter::create(recording_wav_path, spec)
        .map_err(|e| format!("Failed to create bass-anchor recording WAV: {}", e))?;
    if let Some(ref lb) = loopback_recorded {
        if lb.len() != recorded.len() {
            crate::rate_limited_log!(
                warn,
                5,
                "[run_bass_anchor_with_recording] Mic/loopback length mismatch (mic={}, lb={}) — \
                 padding shorter side with zeros so the WAV stays interleaved. \
                 Live result is consistent; offline re-analysis of the WAV may report \
                 phase shifts in the padded region.",
                recorded.len(),
                lb.len()
            );
        }
        let n = recorded.len().max(lb.len());
        for i in 0..n {
            let mic_sample = recorded.get(i).copied().unwrap_or(0.0);
            let lb_sample = lb.get(i).copied().unwrap_or(0.0);
            writer
                .write_sample(mic_sample)
                .map_err(|e| format!("Failed to write bass-anchor mic sample: {}", e))?;
            writer
                .write_sample(lb_sample)
                .map_err(|e| format!("Failed to write bass-anchor loopback sample: {}", e))?;
        }
    } else {
        for &s in &recorded {
            writer
                .write_sample(s)
                .map_err(|e| format!("Failed to write bass-anchor sample: {}", e))?;
        }
    }
    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize bass-anchor WAV: {}", e))?;
    log::info!(
        "[run_bass_anchor_with_recording] Saved {} samples ({:.2}s, {}ch) to {}",
        recorded.len(),
        recorded.len() as f64 / input_sr as f64,
        channels_out,
        recording_wav_path.display()
    );
    Ok(results)
}

#[cfg(not(target_os = "ios"))]
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn run_bass_anchor_core(
    channel_indices: &[u16],
    channel_names: &[String],
    sample_rate: u32,
    bass_freq_hz: f32,
    bass_duration_s: f32,
    fade_ms: f32,
    num_windows: u16,
    silence_duration_ms: f32,
    output_device_name: Option<&str>,
    input_device_name: Option<&str>,
    input_channel: u16,
    loopback_input_channel: Option<u16>,
    signal_level_db: f32,
    cancel: Option<&CancelFlag>,
) -> Result<(BassAnchorResults, Vec<f32>, Option<Vec<f32>>, u32), String> {
    let num_channels = channel_indices.len();
    if num_channels == 0 {
        return Err("No channels for bass anchor".to_string());
    }
    if channel_names.len() != num_channels {
        return Err("channel_indices and channel_names must have the same length".to_string());
    }
    if bass_freq_hz <= 0.0 || bass_duration_s <= 0.0 || num_windows == 0 {
        return Err(format!(
            "Invalid bass-anchor params: freq={bass_freq_hz}, duration_s={bass_duration_s}, num_windows={num_windows}"
        ));
    }

    // Generate the steady-state tone at the playback sample rate.
    let amplitude = measurement_amplitude_from_level_db(signal_level_db);
    let tone = math_audio_dsp::signals::gen_steady_tone(
        bass_freq_hz,
        bass_duration_s,
        fade_ms,
        sample_rate,
        amplitude,
    );
    if tone.is_empty() {
        return Err(format!(
            "gen_steady_tone returned empty (freq={bass_freq_hz}, duration_s={bass_duration_s}, fade_ms={fade_ms})"
        ));
    }
    let tone_samples = tone.len();

    log::info!(
        "[run_bass_anchor] Generated {:.1} Hz × {:.2} s ({} samples, fade {:.0} ms, {} sub-windows) at {} Hz, level={:.1}dBFS{}",
        bass_freq_hz,
        bass_duration_s,
        tone_samples,
        fade_ms,
        num_windows,
        sample_rate,
        signal_level_db.clamp(-40.0, 20.0),
        if loopback_input_channel.is_some() {
            " + loopback ref"
        } else {
            ""
        }
    );

    // Play + record via the shared scaffolding.
    let capture = play_per_channel_and_record_mono(
        channel_indices,
        sample_rate,
        &tone,
        silence_duration_ms,
        output_device_name,
        input_device_name,
        input_channel,
        loopback_input_channel,
        "run_bass_anchor",
        cancel,
    )?;

    // Per-channel analysis via the pure helper.
    // `extract_tone_phase_windowed` operates at the recording's sample
    // rate directly, so no regeneration step is needed even when cpal
    // negotiates a different input rate.
    let results = analyze_bass_anchor_recording(
        &capture.recorded,
        capture.loopback_recorded.as_deref(),
        channel_names,
        channel_indices,
        capture.input_sr,
        bass_freq_hz,
        bass_duration_s,
        num_windows,
        &capture.analysis_offsets,
        capture.analysis_signal_samples,
    )?;

    for cr in &results.channels {
        log::info!(
            "[run_bass_anchor] {}: phase={:.2}°  mag={:.4}  stability={:.2}°{}{}",
            cr.channel_name,
            cr.bass_anchor_phase_deg,
            cr.bass_anchor_magnitude,
            cr.bass_anchor_stability_deg,
            cr.bass_anchor_loopback_phase_deg
                .map(|p| format!("  lb_phase={p:.2}°"))
                .unwrap_or_default(),
            cr.bass_anchor_coherence
                .map(|c| format!("  γ²={c:.3}"))
                .unwrap_or_default()
        );
    }

    Ok((
        results,
        capture.recorded,
        capture.loopback_recorded,
        capture.input_sr,
    ))
}

/// Play a single-frequency sine wave through `output_channel` while
/// recording the mic. Returns the mic peak / RMS sample level over
/// the stable portion of the tone (skipping the first 200 ms of
/// attack and the last 200 ms of release).
#[cfg(not(target_os = "ios"))]
#[allow(clippy::too_many_arguments)]
pub fn run_spl_calibration(
    output_channel: u16,
    sample_rate: u32,
    reference_freq_hz: f32,
    amp: f32,
    duration_s: f32,
    output_device_name: Option<&str>,
    input_device_name: Option<&str>,
    input_channel: u16,
    cancel: Option<CancelFlag>,
) -> Result<SplCalibrationResult, String> {
    if !reference_freq_hz.is_finite() || reference_freq_hz <= 0.0 {
        return Err(format!(
            "Invalid SPL cal reference frequency: {reference_freq_hz}"
        ));
    }
    if !duration_s.is_finite() || duration_s <= 0.3 {
        return Err(format!(
            "SPL cal duration must be > 0.3 s, got {duration_s}"
        ));
    }
    if !amp.is_finite() || !(0.0..=1.0).contains(&amp) {
        return Err(format!("SPL cal amplitude must be in (0, 1], got {amp}"));
    }

    // Generate a Hann-windowed tone so the start/end don't click.
    // The 200 ms skirt on each end is ignored during analysis, so the
    // Hann just avoids hardware popping during attack/release; it
    // doesn't affect the measured level in the stable window.
    let tone = gen_tone(reference_freq_hz, amp, sample_rate, duration_s);
    if tone.is_empty() {
        return Err("gen_tone returned empty".to_string());
    }

    log::info!(
        "[run_spl_calibration] Generated {:.0} Hz tone @ amp={:.3}, {:.1}s ({} samples)",
        reference_freq_hz,
        amp,
        duration_s,
        tone.len()
    );

    let capture = play_per_channel_and_record_mono(
        &[output_channel],
        sample_rate,
        &tone,
        // Short silence so the tone starts cleanly but the whole
        // capture finishes in a couple of seconds. The stability
        // window analysis below excludes the first 200 ms anyway.
        200.0,
        output_device_name,
        input_device_name,
        input_channel,
        None,
        "run_spl_calibration",
        cancel.as_ref(),
    )?;

    // Slice the mic recording to the stable portion of the tone.
    // `analysis_offsets[0]` is the first sample of the tone at the
    // mic's rate; skip an additional 200 ms to let any DAC+mic
    // transient die and stop 200 ms before the tone ends.
    let skirt = (0.2 * capture.input_sr as f32) as usize;
    let start = capture.analysis_offsets[0] + skirt;
    let end = (capture.analysis_offsets[0] + capture.analysis_signal_samples)
        .saturating_sub(skirt)
        .min(capture.recorded.len());
    if end <= start + skirt {
        return Err(format!(
            "[run_spl_calibration] capture too short for stable-window analysis \
             (start={start}, end={end}, recording={})",
            capture.recorded.len()
        ));
    }
    let stable = &capture.recorded[start..end];

    let peak = stable.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
    let rms = {
        let sum_sq: f64 = stable.iter().map(|&s| (s as f64) * (s as f64)).sum();
        ((sum_sq / stable.len() as f64).sqrt()) as f32
    };

    log::info!(
        "[run_spl_calibration] Stable window [{start}..{end}) → peak={peak:.4}, rms={rms:.4}"
    );

    Ok(SplCalibrationResult {
        sample_rate: capture.input_sr,
        peak_sample_level: peak,
        rms_sample_level: rms,
        reference_freq_hz,
        output_channel,
    })
}
