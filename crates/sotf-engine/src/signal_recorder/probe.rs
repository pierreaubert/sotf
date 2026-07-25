use super::consts::DEFAULT_AUXILIARY_SIGNAL_LEVEL_DB;
use super::measurement::measurement_amplitude_from_level_db;
#[cfg(not(target_os = "ios"))]
use super::record::resample_reference_signal;
use super::types::CancelFlag;
use super::types::ProbeDelayChannelResult;
use super::types::ProbeDelayResults;
use super::types::pick_direct_arrival_from_envelope;
#[cfg(not(target_os = "ios"))]
use super::types::play_per_channel_and_record_mono;
use hound::{SampleFormat, WavSpec, WavWriter};
use math_audio_dsp::stft::RealFftProcessor;
use rustfft::num_complex::Complex;
use std::f32::consts::PI;

const MIN_PROBE_TONES: usize = 16;

/// Generate a periodic, band-limited multisine with Schroeder phases.
///
/// Equal-amplitude contiguous FFT bins and the quadratic phase progression
/// keep the probe crest factor bounded, preserving more RMS energy at a given
/// peak level than a random-phase multisine. Periodic construction also avoids
/// introducing a discontinuity at the analysis window boundary.
pub(super) fn gen_schroeder_narrowband_probe(
    n_frames: usize,
    sample_rate: u32,
    amplitude: f32,
    lo_hz: f32,
    hi_hz: f32,
) -> Result<Vec<f32>, String> {
    if n_frames == 0 {
        return Ok(Vec::new());
    }
    if sample_rate == 0
        || !amplitude.is_finite()
        || !lo_hz.is_finite()
        || !hi_hz.is_finite()
        || lo_hz < 0.0
        || hi_hz <= lo_hz
    {
        return Err("probe parameters must be finite and define a valid band".to_string());
    }

    let mut fft = RealFftProcessor::new_bidirectional(n_frames);
    let bin_hz = sample_rate as f32 / n_frames as f32;
    let first_bin = (lo_hz.max(0.0) / bin_hz).ceil() as usize;
    let last_real_bin = if n_frames.is_multiple_of(2) {
        fft.spectrum_size.saturating_sub(2)
    } else {
        fft.spectrum_size.saturating_sub(1)
    };
    let last_bin = ((hi_hz.max(0.0) / bin_hz).floor() as usize).min(last_real_bin);
    if first_bin > last_bin {
        return Err("probe band contains no usable FFT bins".to_string());
    }

    let tone_count = last_bin - first_bin + 1;
    if tone_count < MIN_PROBE_TONES {
        return Err(format!(
            "probe duration is too short for robust delay detection: \
             {tone_count} tones, need at least {MIN_PROBE_TONES}"
        ));
    }
    let tone_count_f32 = tone_count as f32;
    for (tone_index, bin) in (first_bin..=last_bin).enumerate() {
        let k = tone_index as f32;
        let phase = -PI * k * (k - 1.0) / tone_count_f32;
        fft.freq_buffer[bin] = Complex::from_polar(1.0, phase);
    }
    fft.inverse();

    let peak = fft.time_buffer[..n_frames]
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0_f32, f32::max);
    let bounded_amplitude = amplitude.clamp(0.0, 1.0);
    let scale = if peak > 1e-10 {
        bounded_amplitude / peak
    } else {
        0.0
    };
    Ok(fft.time_buffer[..n_frames]
        .iter()
        .map(|sample| sample * scale)
        .collect())
}

/// Run delay probing across all output channels in a single recording pass.
///
/// Builds a playback signal with the pattern:
///   `[silence][ch0_probe][silence][ch1_probe][silence]...[marker_probe][silence]`
///
/// Plays it while recording from the mic, then analyzes each segment
/// for arrival time using cross-correlation with analytic envelope.
/// The raw recording is discarded after analysis — use
/// [`probe_channel_delays_with_recording`] if you want to persist the
/// mono mic capture to a WAV file for inspection or re-analysis.
///
/// # Arguments
/// * `channel_indices` - Output channel indices to probe (0-based)
/// * `channel_names` - Human-readable name for each channel
/// * `sample_rate` - Sample rate in Hz
/// * `probe_duration_ms` - Duration of each probe signal in ms
/// * `silence_duration_ms` - Silence gap between probes in ms
/// * `output_device_name` - Playback device (None = default)
/// * `input_device_name` - Recording device (None = default)
/// * `input_channel` - Mic input channel index
#[cfg(not(target_os = "ios"))]
#[allow(clippy::too_many_arguments)]
pub fn probe_channel_delays(
    channel_indices: &[u16],
    channel_names: &[String],
    sample_rate: u32,
    probe_duration_ms: f32,
    silence_duration_ms: f32,
    output_device_name: Option<&str>,
    input_device_name: Option<&str>,
    input_channel: u16,
) -> Result<ProbeDelayResults, String> {
    // Thin wrapper around the shared core — drops the recorded audio.
    let (results, _recorded, _input_sr) = probe_channel_delays_core(
        channel_indices,
        channel_names,
        sample_rate,
        probe_duration_ms,
        silence_duration_ms,
        output_device_name,
        input_device_name,
        input_channel,
        DEFAULT_AUXILIARY_SIGNAL_LEVEL_DB,
        None,
    )?;
    Ok(results)
}

/// Run delay probing and persist the raw mono mic recording to a WAV
/// file. Identical to [`probe_channel_delays`] in every other respect.
///
/// The recording is written as a single-channel `f32` WAV at the
/// sample rate cpal negotiated for the input device (which may differ
/// from the requested `sample_rate` if the hardware doesn't support
/// it). The file can then be loaded with `hound`, played back, or
/// re-analyzed with the low-level helpers in
/// `autoeq::roomeq::time_align` if the original detection was flagged
/// as low-confidence.
///
/// The probe timing inside the recording follows the same layout as
/// `probe_channel_delays`:
///   `[silence][ch0_probe][silence][ch1_probe][silence]...[tail]`
/// so callers can re-analyze it by running `detect_delays_multi_channel`
/// against the same `channel_offsets` the core used.
///
/// # Arguments
/// Same as [`probe_channel_delays`], plus:
/// * `recording_wav_path` - Filesystem path to write the recorded
///   mono mic audio to. The parent directory must exist.
#[cfg(not(target_os = "ios"))]
#[allow(clippy::too_many_arguments)]
pub fn probe_channel_delays_with_recording(
    channel_indices: &[u16],
    channel_names: &[String],
    sample_rate: u32,
    probe_duration_ms: f32,
    silence_duration_ms: f32,
    output_device_name: Option<&str>,
    input_device_name: Option<&str>,
    input_channel: u16,
    recording_wav_path: &std::path::Path,
    signal_level_db: f32,
    cancel: Option<CancelFlag>,
) -> Result<ProbeDelayResults, String> {
    let (results, recorded, input_sr) = probe_channel_delays_core(
        channel_indices,
        channel_names,
        sample_rate,
        probe_duration_ms,
        silence_duration_ms,
        output_device_name,
        input_device_name,
        input_channel,
        signal_level_db,
        cancel.as_ref(),
    )?;

    // Write the mono recording as an f32 WAV. Matches the spec used by
    // `record_and_analyze` for consistency (same bits/sample_format).
    let spec = WavSpec {
        channels: 1,
        sample_rate: input_sr,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    let mut writer = WavWriter::create(recording_wav_path, spec)
        .map_err(|e| format!("Failed to create probe recording WAV: {}", e))?;
    for &s in &recorded {
        writer
            .write_sample(s)
            .map_err(|e| format!("Failed to write probe recording sample: {}", e))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize probe recording WAV: {}", e))?;
    log::info!(
        "[probe_channel_delays_with_recording] Saved {} samples ({:.2}s) to {}",
        recorded.len(),
        recorded.len() as f64 / input_sr as f64,
        recording_wav_path.display()
    );

    Ok(results)
}

/// Shared implementation behind [`probe_channel_delays`] and
/// [`probe_channel_delays_with_recording`]. Plays the sequential probe
/// pattern, records from the mic, estimates system latency from the
/// first probe, analyzes each segment, and returns the analyzed
/// results together with the raw mono recording buffer and the
/// **negotiated** input sample rate (which may differ from the
/// requested `sample_rate` argument if the hardware doesn't support
/// it).
#[cfg(not(target_os = "ios"))]
#[allow(clippy::too_many_arguments)]
fn probe_channel_delays_core(
    channel_indices: &[u16],
    channel_names: &[String],
    sample_rate: u32,
    probe_duration_ms: f32,
    silence_duration_ms: f32,
    output_device_name: Option<&str>,
    input_device_name: Option<&str>,
    input_channel: u16,
    signal_level_db: f32,
    cancel: Option<&CancelFlag>,
) -> Result<(ProbeDelayResults, Vec<f32>, u32), String> {
    let num_channels = channel_indices.len();
    if num_channels == 0 {
        return Err("No channels to probe".to_string());
    }
    if channel_names.len() != num_channels {
        return Err("channel_indices and channel_names must have the same length".to_string());
    }

    // Generate the narrowband probe (800-2000Hz per Johnston recommendation)
    // at the requested playback sample rate. If cpal negotiates a different
    // input rate the exact playback waveform is resampled for analysis below.
    let probe_samples = (probe_duration_ms / 1000.0 * sample_rate as f32) as usize;
    if !signal_level_db.is_finite() {
        return Err("probe signal level must be finite".to_string());
    }
    let amplitude = measurement_amplitude_from_level_db(signal_level_db).clamp(0.0, 1.0);
    let probe =
        gen_schroeder_narrowband_probe(probe_samples, sample_rate, amplitude, 800.0, 2000.0)?;

    log::info!(
        "[probe_channel_delays] Generated narrowband probe: {} samples ({:.1}ms), 800-2000Hz, level={:.1}dBFS",
        probe_samples,
        probe_duration_ms,
        signal_level_db.clamp(-40.0, 20.0)
    );

    // Play on each output channel in turn and capture the mic — shared
    // scaffolding handles device discovery, playback buffer layout,
    // WAV loading, and the recording stability loop.
    let capture = play_per_channel_and_record_mono(
        channel_indices,
        sample_rate,
        &probe,
        silence_duration_ms,
        output_device_name,
        input_device_name,
        input_channel,
        None,
        "probe_channel_delays",
        cancel,
    )?;

    // If the mic rate differs from the playback rate, resample the exact
    // playback waveform. Regenerating from duration can change an edge bin
    // after frame-count rounding, which changes every Schroeder phase and
    // destroys the matched-filter reference.
    let analysis_probe = if capture.input_sr == sample_rate {
        probe
    } else {
        log::warn!(
            "[probe_channel_delays] Input SR ({}) differs from playback SR ({}); \
             resampling exact probe for correct cross-correlation",
            capture.input_sr,
            sample_rate
        );
        resample_reference_signal(&probe, sample_rate, capture.input_sr)?
    };

    // --- Per-channel analysis via cross-correlation ---
    //
    // Peak position within the segment includes unknown hardware and
    // host latency. That common offset is not an acoustic distance, so
    // normalize the completed result set to the earliest detected
    // arrival before exposing it to the UI / optimizer. Alignment
    // delays are unchanged by this subtraction.

    let auto_result = math_audio_dsp::analysis::cross_correlate_envelope(
        &analysis_probe,
        &analysis_probe,
        capture.input_sr,
    )?;
    let auto_peak = auto_result.peak_value as f64;

    let analysis_segment_len = capture.analysis_silence_samples + capture.analysis_signal_samples;

    let mut arrivals_ms = Vec::with_capacity(num_channels);
    let mut channel_results = Vec::with_capacity(num_channels);

    for (i, &expected_offset) in capture.analysis_offsets.iter().enumerate() {
        let search_start = expected_offset;
        let search_end = (expected_offset + analysis_segment_len).min(capture.recorded.len());
        if search_start >= capture.recorded.len() {
            return Err(format!(
                "Channel {} expected offset {} exceeds recording length {}",
                i,
                expected_offset,
                capture.recorded.len()
            ));
        }
        let segment = &capture.recorded[search_start..search_end];

        let xcorr = math_audio_dsp::analysis::cross_correlate_envelope(
            &analysis_probe,
            segment,
            capture.input_sr,
        )?;

        let direct = pick_direct_arrival_from_envelope(
            &xcorr.envelope,
            capture.input_sr,
            segment
                .len()
                .min(capture.analysis_signal_samples + capture.analysis_silence_samples),
        );
        let peak_sample_refined = direct
            .map(|p| p.peak_sample_refined)
            .unwrap_or(xcorr.peak_sample_refined);
        let peak_value = direct.map(|p| p.peak_value).unwrap_or(xcorr.peak_value);
        let arrival_ms = peak_sample_refined / capture.input_sr as f64 * 1000.0;

        let gain_linear = if auto_peak > 1e-10 {
            peak_value as f64 / auto_peak
        } else {
            0.0
        };
        let gain_db = if gain_linear > 1e-10 {
            20.0 * gain_linear.log10()
        } else {
            -120.0
        };

        // SNR: peak / median of envelope
        let mut sorted_env = xcorr.envelope.to_vec();
        sorted_env.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = sorted_env[sorted_env.len() / 2].max(1e-10) as f64;
        let snr_db = 20.0 * (peak_value as f64 / median).log10();

        log::info!(
            "[probe] Ch {} '{}': arrival={:.3}ms, gain={:.1}dB, SNR={:.1}dB{}",
            i,
            channel_names[i],
            arrival_ms,
            gain_db,
            snr_db,
            if direct.is_some() {
                " (direct)"
            } else {
                " (strongest)"
            },
        );

        arrivals_ms.push(arrival_ms);
        channel_results.push(ProbeDelayChannelResult {
            channel_name: channel_names[i].clone(),
            channel_index: channel_indices[i] as usize,
            arrival_ms,
            gain_db,
            snr_db,
        });
    }

    let latency_floor_ms = arrivals_ms.iter().copied().fold(f64::INFINITY, f64::min);
    if latency_floor_ms.is_finite() {
        for arrival in &mut arrivals_ms {
            *arrival -= latency_floor_ms;
        }
        for result in &mut channel_results {
            result.arrival_ms -= latency_floor_ms;
        }
        log::info!(
            "[probe_channel_delays] Normalized arrivals by subtracting common latency floor {:.3}ms",
            latency_floor_ms
        );
    }

    // Compute alignment delays (align to the slowest relative arrival).
    let max_arrival = arrivals_ms
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let alignment_delays_ms: Vec<f64> = arrivals_ms.iter().map(|&a| max_arrival - a).collect();

    log::info!("[probe_channel_delays] Results:");
    for (i, cr) in channel_results.iter().enumerate() {
        log::info!(
            "  {}: arrival={:.3}ms, gain={:.1}dB, SNR={:.1}dB, alignment_delay={:.3}ms",
            cr.channel_name,
            cr.arrival_ms,
            cr.gain_db,
            cr.snr_db,
            alignment_delays_ms[i]
        );
    }

    let input_sr = capture.input_sr;
    let recorded = capture.recorded;
    Ok((
        ProbeDelayResults {
            channels: channel_results,
            sample_rate: input_sr,
            alignment_delays_ms,
        },
        recorded,
        input_sr,
    ))
}
