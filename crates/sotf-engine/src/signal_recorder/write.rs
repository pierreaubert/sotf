use hound::{SampleFormat, WavSpec, WavWriter};
#[cfg(not(target_os = "ios"))]
use std::io::Write as _;
use std::path::Path;
use tempfile::NamedTempFile;

#[cfg(not(target_os = "ios"))]
pub(super) fn write_selected_channel_to_ring(
    producer: &mut rtrb::Producer<f32>,
    data: &[f32],
    channels: usize,
    channel_idx: usize,
) -> usize {
    if channels == 0 || channel_idx >= channels {
        return 0;
    }

    let frames = data.len() / channels;
    let writable = producer.slots().min(frames);
    if writable == 0 {
        return 0;
    }

    let Ok(mut chunk) = producer.write_chunk_uninit(writable) else {
        return 0;
    };

    let (first, second) = chunk.as_mut_slices();
    for (frame_idx, slot) in first.iter_mut().chain(second.iter_mut()).enumerate() {
        slot.write(data[frame_idx * channels + channel_idx]);
    }
    unsafe { chunk.commit(writable) };
    writable
}

#[cfg(not(target_os = "ios"))]
pub(super) fn write_capture_pairs_to_ring<T>(
    producer: &mut rtrb::Producer<(f32, f32)>,
    data: &[T],
    channels: usize,
    input_idx: usize,
    loopback_idx: Option<usize>,
) -> usize
where
    T: cpal::Sample,
    f32: cpal::FromSample<T>,
{
    use cpal::Sample;

    if channels == 0 || input_idx >= channels || loopback_idx.is_some_and(|idx| idx >= channels) {
        return 0;
    }

    let frames = data.len() / channels;
    let writable = producer.slots().min(frames);
    if writable == 0 {
        return 0;
    }

    let Ok(mut chunk) = producer.write_chunk_uninit(writable) else {
        return 0;
    };

    let (first, second) = chunk.as_mut_slices();
    for (frame_idx, slot) in first.iter_mut().chain(second.iter_mut()).enumerate() {
        let base = frame_idx * channels;
        let mic_sample = f32::from_sample(data[base + input_idx]);
        let loopback_sample = loopback_idx
            .map(|idx| f32::from_sample(data[base + idx]))
            .unwrap_or(0.0);
        slot.write((mic_sample, loopback_sample));
    }
    unsafe { chunk.commit(writable) };
    writable
}

/// Write signal to a temporary WAV file
pub fn write_temp_wav(
    signal: &[f32],
    sample_rate: u32,
    channels: u16,
) -> Result<NamedTempFile, String> {
    let temp_file = NamedTempFile::with_suffix(".wav")
        .map_err(|e| format!("Failed to create temp file: {}", e))?;

    write_wav_file(temp_file.path(), signal, sample_rate, channels)?;

    Ok(temp_file)
}

/// Write signal to a WAV file
pub fn write_wav_file(
    path: &Path,
    signal: &[f32],
    sample_rate: u32,
    channels: u16,
) -> Result<(), String> {
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };

    let mut writer =
        WavWriter::create(path, spec).map_err(|e| format!("Failed to create WAV writer: {}", e))?;

    for &sample in signal {
        writer
            .write_sample(sample)
            .map_err(|e| format!("Failed to write sample: {}", e))?;
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV file: {}", e))?;

    Ok(())
}

/// Log-frequency interpolation of a curve sampled on `source_freqs_hz`
/// (ascending, e.g. a linear FFT bin grid) onto `target_freqs_hz` (e.g. the
/// log-spaced analysis CSV grid).
///
/// Targets outside the source range clamp to the nearest endpoint value. A
/// bracket spanning a DC (0 Hz) source bin has no log coordinate and falls
/// back to the upper sample. Empty sources yield zeros.
#[cfg(not(target_os = "ios"))]
pub fn interpolate_log_frequency_grid(
    source_freqs_hz: &[f32],
    source_values: &[f32],
    target_freqs_hz: &[f32],
) -> Vec<f32> {
    let n = source_freqs_hz.len().min(source_values.len());
    if n == 0 {
        return vec![0.0; target_freqs_hz.len()];
    }
    let source_freqs = &source_freqs_hz[..n];
    let source_values = &source_values[..n];

    target_freqs_hz
        .iter()
        .map(|&target| {
            if !target.is_finite() || target <= source_freqs[0] {
                return source_values[0];
            }
            if target >= source_freqs[n - 1] {
                return source_values[n - 1];
            }
            // Ascending grid: first index strictly above the target.
            let upper = source_freqs.partition_point(|&f| f <= target);
            let lower = upper - 1;
            let (f_lo, f_hi) = (source_freqs[lower], source_freqs[upper]);
            let (v_lo, v_hi) = (source_values[lower], source_values[upper]);
            if f_lo <= 0.0 || f_hi <= f_lo {
                return v_hi;
            }
            let t = (target.ln() - f_lo.ln()) / (f_hi.ln() - f_lo.ln());
            v_lo + t * (v_hi - v_lo)
        })
        .collect()
}

/// Write an [`crate::signal_analysis::AnalysisResult`] to CSV with the exact
/// column layout of math-dsp's `write_analysis_csv`, plus optional
/// `coherence` (γ²) and `noise_floor_db` columns appended last (B2).
///
/// Column order matters: math-dsp's `read_analysis_csv` parses columns ≥ 8
/// POSITIONALLY, so the original eight columns stay first and the new ones
/// are appended; autoeq's `load_driver_measurement` is header-name driven
/// and picks `coherence` / `noise_floor_db` up by name regardless of order.
/// math-dsp's own writer does not support extra columns and must not be
/// modified (it lives in the sibling math-audio repo), hence this wrapper.
///
/// `coherence` / `noise_floor_db`, when `Some`, must already be sampled on
/// `result.frequencies` (use [`interpolate_log_frequency_grid`] for FFT-grid
/// data). A `None` column is omitted from header and rows entirely rather
/// than fabricated — an all-ones coherence column would be a lie (B1-class)
/// and autoeq's gate degrades honestly when the column is missing.
#[cfg(not(target_os = "ios"))]
pub fn write_analysis_csv_extended(
    result: &crate::signal_analysis::AnalysisResult,
    output_path: &Path,
    compensation: Option<&crate::signal_analysis::MicrophoneCompensation>,
    coherence: Option<&[f32]>,
    noise_floor_db: Option<&[f32]>,
) -> Result<(), String> {
    for (name, column) in [("coherence", coherence), ("noise_floor_db", noise_floor_db)] {
        if let Some(values) = column
            && values.len() != result.frequencies.len()
        {
            return Err(format!(
                "write_analysis_csv_extended: {name} column has {} values for {} CSV rows",
                values.len(),
                result.frequencies.len()
            ));
        }
    }
    if result.frequencies.is_empty() {
        return Err("Cannot write CSV: Analysis result has no frequency points!".to_string());
    }

    let mut file =
        std::fs::File::create(output_path).map_err(|e| format!("Failed to create CSV file: {e}"))?;

    // The first eight columns mirror math-dsp's write_analysis_csv exactly.
    let mut header =
        "frequency_hz,spl_db,phase_deg,thd_percent,rt60_ms,c50_db,c80_db,group_delay_ms"
            .to_string();
    if coherence.is_some() {
        header.push_str(",coherence");
    }
    if noise_floor_db.is_some() {
        header.push_str(",noise_floor_db");
    }
    writeln!(file, "{header}").map_err(|e| format!("Failed to write header: {e}"))?;

    for i in 0..result.frequencies.len() {
        let freq = result.frequencies[i];
        let mut spl = result.spl_db[i];
        // Same inverse-compensation semantics as math-dsp's writer: subtract
        // the microphone's deviation to recover the true SPL.
        if let Some(comp) = compensation {
            spl -= comp.interpolate_at(freq);
        }
        let phase = result.phase_deg[i];
        let thd = result.thd_percent.get(i).copied().unwrap_or(0.0);
        let rt60 = result.rt60_ms.get(i).copied().unwrap_or(0.0);
        let c50 = result.clarity_c50_db.get(i).copied().unwrap_or(0.0);
        let c80 = result.clarity_c80_db.get(i).copied().unwrap_or(0.0);
        let gd = result.excess_group_delay_ms.get(i).copied().unwrap_or(0.0);

        let mut row = format!(
            "{freq:.6},{spl:.3},{phase:.6},{thd:.6},{rt60:.3},{c50:.3},{c80:.3},{gd:.6}"
        );
        if let Some(values) = coherence {
            row.push_str(&format!(",{:.4}", values[i]));
        }
        if let Some(values) = noise_floor_db {
            row.push_str(&format!(",{:.3}", values[i]));
        }
        writeln!(file, "{row}").map_err(|e| format!("Failed to write data: {e}"))?;
    }

    Ok(())
}
