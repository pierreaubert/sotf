use super::misc::build_info_chunk;
use super::misc::clip;
use super::types::Sidecar;
use std::fs;
use std::path::Path;

/// Write a WAV file with RIFF INFO tags placed before the data chunk so that
/// Symphonia (which stops parsing at the data chunk) can read them.
///
/// Layout: RIFF header | fmt chunk | LIST INFO chunk | data chunk
pub(super) fn write_wav(
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

pub(super) fn write_sidecar(path: &Path, sidecar: &Sidecar) -> Result<(), String> {
    let json = serde_json::to_string_pretty(sidecar)
        .map_err(|e| format!("Failed to serialize sidecar: {}", e))?;
    fs::write(path, json).map_err(|e| format!("Failed to write sidecar: {}", e))?;
    Ok(())
}

pub(super) fn write_manifest(path: &Path, files: &[String]) -> Result<(), String> {
    let manifest = serde_json::json!({ "files": files });
    let json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
    fs::write(path, json).map_err(|e| format!("Failed to write manifest: {}", e))?;
    Ok(())
}
