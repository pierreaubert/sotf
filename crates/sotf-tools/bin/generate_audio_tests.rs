//! Generate audio test files for end-to-end audio validation.
//!
//! Generates WAV files in multiple channel counts, sample rates and bit depths.
//! Signals:
//! - id: per-channel identification tones (unique frequency per channel)
//! - thd1k: single-tone 1 kHz @ -3 dBFS (for THD)
//! - thd100: single-tone 100 Hz @ -3 dBFS (low-frequency THD)
//! - imd_smpte: SMPTE two-tone 60 Hz + 7 kHz (4:1 power ratio, 2:1 amplitude ratio)
//! - imd_ccif: CCIF two-tone 19 kHz + 20 kHz (equal amplitudes)
//! - sweep: logarithmic frequency sweep from 20 Hz to 20 kHz (10s fixed duration)
//! - white_noise: white noise (flat spectrum)
//! - pink_noise: pink noise (1/f spectrum, -3dB/octave)
//! - m_noise: M-weighted noise (ITU-R 468 weighting for acoustic measurements)

use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use sotf_audio::signals::*;
use std::fs;
use std::path::{Path, PathBuf};

// Constants
const AMP_STD: f32 = 0.707; // ~-3 dBFS
const SMPTE_LOW_AMP: f32 = 0.8; // 60 Hz amplitude
const SMPTE_HIGH_AMP: f32 = 0.4; // 7 kHz amplitude, 2:1 amp = 4:1 power
const SMPTE_POWER_RATIO: u8 = 4;
const CCIF_AMP: f32 = 0.5; // 19/20 kHz equal amplitudes
const ID_BASE_FREQ: f32 = 300.0;
const ID_STEP_FREQ: f32 = 300.0;
const ID_MAX_FREQ: f32 = 6000.0;
const SWEEP_DURATION: f32 = 30.0; // Fixed duration for sweep

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SignalKind {
    Id,
    Thd1k,
    Thd100,
    ImdSmpte,
    ImdCcif,
    Sweep,
    WhiteNoise,
    PinkNoise,
    MNoise,
}

impl SignalKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Thd1k => "thd1k",
            Self::Thd100 => "thd100",
            Self::ImdSmpte => "imd_smpte",
            Self::ImdCcif => "imd_ccif",
            Self::Sweep => "sweep",
            Self::WhiteNoise => "white_noise",
            Self::PinkNoise => "pink_noise",
            Self::MNoise => "m_noise",
        }
    }

    fn all() -> Vec<Self> {
        vec![
            Self::Id,
            Self::Thd1k,
            Self::Thd100,
            Self::ImdSmpte,
            Self::ImdCcif,
            Self::Sweep,
            Self::WhiteNoise,
            Self::PinkNoise,
            Self::MNoise,
        ]
    }
}

#[derive(Parser)]
#[command(name = "generate_audio_tests")]
#[command(about = "Generate audio test files for validation", long_about = None)]
struct Cli {
    /// Output directory
    #[arg(long, default_value = "data_generated/test-audio")]
    out_dir: PathBuf,

    /// Number of channels (comma-separated, mono stereo 5.1 and 9.1.6)
    #[arg(long, value_delimiter = ',', default_values_t = vec![1, 2, 6, 16])]
    channels: Vec<u16>,

    /// Sample rates in Hz (comma-separated, should be enough to test most cases)
    #[arg(long = "sample-rates", value_delimiter = ',', default_values_t = vec![44100, 48000, 96000])]
    sample_rates: Vec<u32>,

    /// Bit depths (comma-separated, 16 or 24 only)
    #[arg(long, value_delimiter = ',', default_values_t = vec![16, 24])]
    bits: Vec<u16>,

    /// Signal types to generate (comma-separated)
    #[arg(long, value_delimiter = ',')]
    signals: Vec<SignalKind>,

    /// Duration in seconds (default 3.0, does not apply to sweep which is fixed at 10s)
    #[arg(long, default_value_t = 10.0)]
    duration: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Sidecar {
    format: String,
    channels: u16,
    sample_rate: u32,
    bits: u16,
    duration: f32,
    signal: SignalMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum SignalMetadata {
    Id {
        freqs: Vec<f32>,
    },
    Thd1k {
        freq: f32,
    },
    Thd100 {
        freq: f32,
    },
    ImdSmpte {
        freqs: [f32; 2],
        power_ratio: u8,
    },
    ImdCcif {
        freqs: [f32; 2],
    },
    Sweep {
        freq_start: f32,
        freq_end: f32,
        kind: String,
    },
    WhiteNoise {
        description: String,
    },
    PinkNoise {
        description: String,
    },
    MNoise {
        description: String,
        weighting: String,
    },
}

#[derive(Debug)]
struct GenerationStats {
    generated: usize,
    skipped: usize,
    failed: usize,
}

impl GenerationStats {
    fn new() -> Self {
        Self {
            generated: 0,
            skipped: 0,
            failed: 0,
        }
    }
}

fn id_frequency(channel: u16) -> f32 {
    (ID_BASE_FREQ + ID_STEP_FREQ * channel as f32).min(ID_MAX_FREQ)
}

fn ensure_below_nyquist(freq: f32, sr: u32, label: &str) -> Result<(), String> {
    let nyquist = sr as f32 / 2.0;
    if freq >= nyquist {
        Err(format!(
            "Nyquist violation: {label} {freq} Hz >= {nyquist} Hz (skipped)"
        ))
    } else {
        Ok(())
    }
}

fn gen_tone_checked(freq: f32, amp: f32, sr: u32, duration: f32) -> Result<Vec<f32>, String> {
    ensure_below_nyquist(freq, sr, "tone")?;
    Ok(gen_tone(freq, amp, sr, duration))
}

fn gen_two_tone_checked(
    f1: f32,
    a1: f32,
    f2: f32,
    a2: f32,
    sr: u32,
    duration: f32,
) -> Result<Vec<f32>, String> {
    ensure_below_nyquist(f1, sr, "first tone")?;
    ensure_below_nyquist(f2, sr, "second tone")?;
    Ok(gen_two_tone(f1, a1, f2, a2, sr, duration))
}

fn gen_log_sweep_checked(
    f_start: f32,
    f_end: f32,
    amp: f32,
    sr: u32,
    duration: f32,
) -> Result<Vec<f32>, String> {
    ensure_below_nyquist(f_end, sr, "sweep end")?;
    Ok(gen_log_sweep(f_start, f_end, amp, sr, duration))
}

fn main() {
    let cli = Cli::parse();

    // Validate bit depths
    for &bits in &cli.bits {
        if bits != 16 && bits != 24 {
            eprintln!("Error: Bit depth must be 16 or 24, got {}", bits);
            std::process::exit(1);
        }
    }

    // If no signals specified, use all
    let signals = if cli.signals.is_empty() {
        SignalKind::all()
    } else {
        cli.signals.clone()
    };

    // Create output directory
    if let Err(e) = fs::create_dir_all(&cli.out_dir) {
        eprintln!("Error: Failed to create output directory: {}", e);
        std::process::exit(1);
    }

    let mut stats = GenerationStats::new();
    let mut manifest_files = Vec::new();

    // Generate all combinations
    for &signal in &signals {
        for &channels in &cli.channels {
            if !(1..=16).contains(&channels) {
                eprintln!(
                    "Warning: Channel count {} out of range [1,16], skipping",
                    channels
                );
                stats.skipped += 1;
                continue;
            }

            for &sr in &cli.sample_rates {
                for &bits in &cli.bits {
                    let duration = if signal == SignalKind::Sweep {
                        SWEEP_DURATION
                    } else {
                        cli.duration
                    };

                    match generate_one(&cli.out_dir, signal, channels, sr, bits, duration) {
                        Ok(path) => {
                            manifest_files.push(path.to_string_lossy().to_string());
                            stats.generated += 1;
                        }
                        Err(e) => {
                            if e.contains("Nyquist") || e.contains("skipped") {
                                stats.skipped += 1;
                            } else {
                                eprintln!(
                                    "Warning: Failed to generate {} ch{} sr{} b{}: {}",
                                    signal.as_str(),
                                    channels,
                                    sr,
                                    bits,
                                    e
                                );
                                stats.failed += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    // Write manifest
    let manifest_path = cli.out_dir.join("manifest.json");
    match write_manifest(&manifest_path, &manifest_files) {
        Ok(_) => {
            println!(
                "\nGenerated {} files. Manifest: {}",
                stats.generated,
                manifest_path.display()
            );
        }
        Err(e) => {
            eprintln!("Warning: Failed to write manifest: {}", e);
        }
    }

    println!(
        "Summary: Generated: {}, Skipped: {}, Failed: {}",
        stats.generated, stats.skipped, stats.failed
    );
}

fn generate_one(
    out_dir: &Path,
    signal: SignalKind,
    channels: u16,
    sr: u32,
    bits: u16,
    duration: f32,
) -> Result<PathBuf, String> {
    let nyquist = sr as f32 / 2.0;

    // Check Nyquist violations
    match signal {
        SignalKind::Thd1k if 1000.0 >= nyquist => {
            return Err(format!(
                "Nyquist violation: 1000 Hz >= {} Hz (skipped)",
                nyquist
            ));
        }
        SignalKind::Thd100 if 100.0 >= nyquist => {
            return Err(format!(
                "Nyquist violation: 100 Hz >= {} Hz (skipped)",
                nyquist
            ));
        }
        SignalKind::ImdSmpte if 7000.0 >= nyquist => {
            return Err(format!(
                "Nyquist violation: 7000 Hz >= {} Hz (skipped)",
                nyquist
            ));
        }
        SignalKind::ImdCcif if 20000.0 >= nyquist => {
            return Err(format!(
                "Nyquist violation: 20 kHz >= {} Hz (skipped)",
                nyquist
            ));
        }
        SignalKind::Sweep if 20000.0 >= nyquist => {
            return Err(format!(
                "Nyquist violation: sweep end 20 kHz >= {} Hz (skipped)",
                nyquist
            ));
        }
        SignalKind::Id => {
            let max_id_freq = id_frequency(channels - 1);
            if max_id_freq >= nyquist {
                return Err(format!(
                    "Nyquist violation: max ID freq {} Hz >= {} Hz (skipped)",
                    max_id_freq, nyquist
                ));
            }
        }
        _ => {}
    }

    // Generate signal data
    let (audio_data, metadata) = match signal {
        SignalKind::Id => {
            let mut freqs = Vec::new();
            let mut per_channel = Vec::new();
            for ch in 0..channels {
                let freq = id_frequency(ch);
                freqs.push(freq);
                per_channel.push(gen_tone_checked(freq, AMP_STD, sr, duration)?);
            }
            let data = interleave_per_channel(&per_channel);
            (data, SignalMetadata::Id { freqs })
        }
        SignalKind::Thd1k => {
            let mono = gen_tone_checked(1000.0, AMP_STD, sr, duration)?;
            let data = replicate_mono(&mono, channels);
            (data, SignalMetadata::Thd1k { freq: 1000.0 })
        }
        SignalKind::Thd100 => {
            let mono = gen_tone_checked(100.0, AMP_STD, sr, duration)?;
            let data = replicate_mono(&mono, channels);
            (data, SignalMetadata::Thd100 { freq: 100.0 })
        }
        SignalKind::ImdSmpte => {
            let mono =
                gen_two_tone_checked(60.0, SMPTE_LOW_AMP, 7000.0, SMPTE_HIGH_AMP, sr, duration)?;
            let data = replicate_mono(&mono, channels);
            (
                data,
                SignalMetadata::ImdSmpte {
                    freqs: [60.0, 7000.0],
                    power_ratio: SMPTE_POWER_RATIO,
                },
            )
        }
        SignalKind::ImdCcif => {
            let mono = gen_two_tone_checked(19000.0, CCIF_AMP, 20000.0, CCIF_AMP, sr, duration)?;
            let data = replicate_mono(&mono, channels);
            (
                data,
                SignalMetadata::ImdCcif {
                    freqs: [19000.0, 20000.0],
                },
            )
        }
        SignalKind::Sweep => {
            let mono = gen_log_sweep_checked(20.0, 20000.0, AMP_STD, sr, duration)?;
            let data = replicate_mono(&mono, channels);
            (
                data,
                SignalMetadata::Sweep {
                    freq_start: 20.0,
                    freq_end: 20000.0,
                    kind: "log".to_string(),
                },
            )
        }
        SignalKind::WhiteNoise => {
            let mono = gen_white_noise(AMP_STD, sr, duration);
            let data = replicate_mono(&mono, channels);
            (
                data,
                SignalMetadata::WhiteNoise {
                    description: "Flat spectrum (white noise)".to_string(),
                },
            )
        }
        SignalKind::PinkNoise => {
            let mono = gen_pink_noise(AMP_STD, sr, duration);
            let data = replicate_mono(&mono, channels);
            (
                data,
                SignalMetadata::PinkNoise {
                    description: "1/f spectrum, -3dB/octave (pink noise)".to_string(),
                },
            )
        }
        SignalKind::MNoise => {
            let mono = gen_m_noise(AMP_STD, sr, duration);
            let data = replicate_mono(&mono, channels);
            (
                data,
                SignalMetadata::MNoise {
                    description: "M-weighted noise for acoustic measurements".to_string(),
                    weighting: "ITU-R 468".to_string(),
                },
            )
        }
    };

    // Create output directory structure
    let subdir = out_dir.join("wav").join(signal.as_str());
    fs::create_dir_all(&subdir).map_err(|e| format!("Failed to create directory: {}", e))?;

    // Build filename
    let filename = format!("{}_ch{}_sr{}_b{}.wav", signal.as_str(), channels, sr, bits);
    let wav_path = subdir.join(&filename);

    // Write WAV file with metadata tags before data chunk
    let tags: &[(&[u8; 4], &str)] = &[(b"IART", "SotF"), (b"IPRD", "SotF")];
    write_wav(&wav_path, &audio_data, sr, channels, bits, tags)?;

    // Write sidecar JSON
    let sidecar = Sidecar {
        format: "wav".to_string(),
        channels,
        sample_rate: sr,
        bits,
        duration,
        signal: metadata,
    };

    let sidecar_path = wav_path.with_extension("wav.json");
    write_sidecar(&sidecar_path, &sidecar)?;

    Ok(wav_path)
}

// WAV writing

fn clip(sample: f32) -> f32 {
    sample.clamp(-1.0, 1.0)
}

/// Build a RIFF LIST INFO chunk containing the given tag pairs.
fn build_info_chunk(tags: &[(&[u8; 4], &str)]) -> Vec<u8> {
    fn info_subchunk(id: &[u8; 4], value: &str) -> Vec<u8> {
        let mut v = value.as_bytes().to_vec();
        v.push(0); // null terminator
        if !v.len().is_multiple_of(2) {
            v.push(0); // RIFF word-alignment pad
        }
        let mut buf = Vec::with_capacity(8 + v.len());
        buf.extend_from_slice(id);
        buf.extend_from_slice(&(v.len() as u32).to_le_bytes());
        buf.extend_from_slice(&v);
        buf
    }

    let sub_chunks: Vec<u8> = tags
        .iter()
        .flat_map(|(id, val)| info_subchunk(id, val))
        .collect();
    let list_data_len = 4 + sub_chunks.len(); // "INFO" + sub-chunks
    let mut chunk = Vec::with_capacity(8 + list_data_len);
    chunk.extend_from_slice(b"LIST");
    chunk.extend_from_slice(&(list_data_len as u32).to_le_bytes());
    chunk.extend_from_slice(b"INFO");
    chunk.extend_from_slice(&sub_chunks);
    chunk
}

/// Write a WAV file with RIFF INFO tags placed before the data chunk so that
/// Symphonia (which stops parsing at the data chunk) can read them.
///
/// Layout: RIFF header | fmt chunk | LIST INFO chunk | data chunk
fn write_wav(
    path: &Path,
    interleaved: &[f32],
    sr: u32,
    channels: u16,
    bits: u16,
    tags: &[(&[u8; 4], &str)],
) -> Result<(), String> {
    use std::io::{BufWriter, Write};

    if !matches!(bits, 16 | 24) {
        return Err(format!("Unsupported bit depth: {}", bits));
    }

    let bytes_per_sample = (bits / 8) as u32;
    let block_align = channels as u32 * bytes_per_sample;
    let byte_rate = sr * block_align;
    let data_size = (interleaved.len() as u64)
        .checked_mul(u64::from(bytes_per_sample))
        .ok_or_else(|| "WAV data size overflow".to_string())?;
    if data_size > u64::from(u32::MAX) {
        return Err(format!("WAV data too large: {data_size} bytes"));
    }

    // Build fmt chunk (16 bytes payload for PCM)
    let mut fmt_chunk = Vec::with_capacity(24);
    fmt_chunk.extend_from_slice(b"fmt ");
    fmt_chunk.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    fmt_chunk.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    fmt_chunk.extend_from_slice(&channels.to_le_bytes());
    fmt_chunk.extend_from_slice(&sr.to_le_bytes());
    fmt_chunk.extend_from_slice(&byte_rate.to_le_bytes());
    fmt_chunk.extend_from_slice(&(block_align as u16).to_le_bytes());
    fmt_chunk.extend_from_slice(&bits.to_le_bytes());

    // Build LIST INFO chunk
    let info_chunk = if tags.is_empty() {
        Vec::new()
    } else {
        build_info_chunk(tags)
    };

    // Build data chunk header
    let mut data_header = Vec::with_capacity(8);
    data_header.extend_from_slice(b"data");
    data_header.extend_from_slice(&(data_size as u32).to_le_bytes());

    // RIFF header: total size = 4 ("WAVE") + fmt + info + data_header + data bytes
    let riff_size = 4_u64
        + fmt_chunk.len() as u64
        + info_chunk.len() as u64
        + data_header.len() as u64
        + data_size;
    if riff_size > u64::from(u32::MAX) {
        return Err(format!("RIFF size too large: {riff_size} bytes"));
    }

    let file =
        std::fs::File::create(path).map_err(|e| format!("Failed to create WAV file: {}", e))?;
    let mut file = BufWriter::new(file);

    file.write_all(b"RIFF")
        .map_err(|e| format!("Write failed: {}", e))?;
    file.write_all(&(riff_size as u32).to_le_bytes())
        .map_err(|e| format!("Write failed: {}", e))?;
    file.write_all(b"WAVE")
        .map_err(|e| format!("Write failed: {}", e))?;
    file.write_all(&fmt_chunk)
        .map_err(|e| format!("Write failed: {}", e))?;
    file.write_all(&info_chunk)
        .map_err(|e| format!("Write failed: {}", e))?;
    file.write_all(&data_header)
        .map_err(|e| format!("Write failed: {}", e))?;

    match bits {
        16 => {
            for &sample in interleaved {
                let pcm = (clip(sample) * 32767.0).round() as i16;
                file.write_all(&pcm.to_le_bytes())
                    .map_err(|e| format!("Write failed: {}", e))?;
            }
        }
        24 => {
            for &sample in interleaved {
                let pcm = (clip(sample) * 8388607.0).round() as i32;
                let bytes = pcm.to_le_bytes();
                file.write_all(&bytes[..3])
                    .map_err(|e| format!("Write failed: {}", e))?;
            }
        }
        _ => return Err(format!("Unsupported bit depth: {}", bits)),
    }

    Ok(())
}

// JSON writing

fn write_sidecar(path: &Path, sidecar: &Sidecar) -> Result<(), String> {
    let json = serde_json::to_string_pretty(sidecar)
        .map_err(|e| format!("Failed to serialize sidecar: {}", e))?;
    fs::write(path, json).map_err(|e| format!("Failed to write sidecar: {}", e))?;
    Ok(())
}

fn write_manifest(path: &Path, files: &[String]) -> Result<(), String> {
    let manifest = serde_json::json!({ "files": files });
    let json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
    fs::write(path, json).map_err(|e| format!("Failed to write manifest: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn id_frequency_uses_shared_step_formula() {
        assert_eq!(id_frequency(0), ID_BASE_FREQ);
        assert_eq!(id_frequency(1), ID_BASE_FREQ + ID_STEP_FREQ);
        assert_eq!(id_frequency(32), ID_MAX_FREQ);
    }

    #[test]
    fn checked_tone_rejects_nyquist_frequency() {
        let err = gen_tone_checked(24_000.0, AMP_STD, 48_000, 0.01).unwrap_err();
        assert!(err.contains("Nyquist violation"), "unexpected error: {err}");
    }

    #[test]
    fn smpte_metadata_uses_power_ratio() {
        let dir = tempfile::tempdir().unwrap();
        let wav = generate_one(dir.path(), SignalKind::ImdSmpte, 1, 48_000, 16, 0.01).unwrap();
        let sidecar = fs::read_to_string(wav.with_extension("wav.json")).unwrap();
        let sidecar: Value = serde_json::from_str(&sidecar).unwrap();

        assert_eq!(sidecar["signal"]["type"], "imd_smpte");
        assert_eq!(sidecar["signal"]["power_ratio"], SMPTE_POWER_RATIO);
        assert!(sidecar["signal"].get("ratio").is_none());
    }

    #[test]
    fn write_wav_rejects_unsupported_bit_depth_before_writing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.wav");
        let err = write_wav(&path, &[0.0], 48_000, 1, 12, &[]).unwrap_err();
        assert!(
            err.contains("Unsupported bit depth"),
            "unexpected error: {err}"
        );
        assert!(!path.exists());
    }

    #[test]
    fn info_chunk_is_word_aligned() {
        let chunk = build_info_chunk(&[(b"IART", "SotF"), (b"IPRD", "Odd")]);
        assert_eq!(chunk.len() % 2, 0);
        assert!(chunk.starts_with(b"LIST"));
        assert_eq!(&chunk[8..12], b"INFO");
    }
}
