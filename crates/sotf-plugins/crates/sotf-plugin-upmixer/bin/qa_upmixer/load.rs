use super::misc::decode_audio_with_ffmpeg;
use super::types::InputAudio;
use hound::{SampleFormat, WavReader};
use std::io::Cursor;
use std::path::Path;

pub(super) fn load_audio_stereo(path: &Path) -> Result<InputAudio, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
    match load_wav_stereo_from_bytes(path, bytes) {
        Ok(input) => Ok(input),
        Err(wav_err) => {
            let wav_bytes = decode_audio_with_ffmpeg(path, &wav_err)?;
            load_wav_stereo_from_bytes(path, wav_bytes)
        }
    }
}

pub(super) fn load_wav_stereo_from_bytes(
    path: &Path,
    mut bytes: Vec<u8>,
) -> Result<InputAudio, String> {
    if !bytes.starts_with(b"RIFF") {
        let riff_start = bytes
            .windows(4)
            .position(|w| w == b"RIFF")
            .ok_or_else(|| format!("could not find RIFF header in {}", path.display()))?;
        bytes.drain(..riff_start);
    }

    let mut reader = WavReader::new(Cursor::new(bytes))
        .map_err(|e| format!("could not parse {}: {e}", path.display()))?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;

    let raw = match spec.sample_format {
        SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("could not read float samples: {e}"))?,
        SampleFormat::Int => {
            let bits = spec.bits_per_sample.clamp(1, 32);
            let denom = (1_i64 << (bits - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| (v as f32 / denom).clamp(-1.0, 1.0)))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("could not read integer samples: {e}"))?
        }
    };

    let frames = raw.len() / channels;
    let mut stereo = Vec::with_capacity(frames * 2);
    for frame in raw.chunks_exact(channels) {
        let left = frame[0];
        let right = if channels > 1 { frame[1] } else { left };
        stereo.push(left);
        stereo.push(right);
    }

    Ok(InputAudio {
        sample_rate: spec.sample_rate,
        samples: stereo,
    })
}
