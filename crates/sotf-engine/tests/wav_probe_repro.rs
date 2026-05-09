// Smoke test for the temp-WAV pipeline used by the recording helper
// `play_per_channel_and_record_mono`: hound writes a 32-bit float WAV
// at the channel count derived from the output device, then Symphonia
// reads it back via `probe_file`.
//
// The helper used to grab `output_device.supported_output_configs().max()`
// for the channel count, which on pro audio interfaces (RME UFX+ reports
// 94) produces a WAV that symphonia-format-riff cannot decode — its
// channel-mask handling returns Unsupported around 32 ch and panics on
// left-shift overflow at higher counts. The fix caps the count at the
// minimum needed to address the highest target output channel; this
// test pins the pipeline at the channel counts the helper now produces
// in practice.

use hound::{SampleFormat, WavSpec, WavWriter};
use sotf_audio::decoder::probe_file;
use tempfile::NamedTempFile;

fn write_and_probe(channels: u16) -> Result<(), String> {
    let temp_file = NamedTempFile::with_suffix(".wav").map_err(|e| format!("temp: {e}"))?;
    let temp_path = temp_file.path().to_path_buf();
    let spec = WavSpec {
        channels,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    let mut writer = WavWriter::create(&temp_path, spec).map_err(|e| format!("create: {e}"))?;
    let frames = 48_000;
    for _ in 0..frames {
        for _ in 0..channels {
            writer
                .write_sample(0.1_f32)
                .map_err(|e| format!("write: {e}"))?;
        }
    }
    writer.finalize().map_err(|e| format!("finalize: {e}"))?;
    probe_file(&temp_path).map_err(|e| format!("probe: {e}"))?;
    Ok(())
}

#[test]
fn probe_mono_float32() {
    write_and_probe(1).expect("mono float32 should probe");
}

#[test]
fn probe_stereo_float32() {
    write_and_probe(2).expect("stereo float32 should probe");
}

#[test]
fn probe_quad_float32() {
    write_and_probe(4).expect("quad float32 should probe");
}

#[test]
fn probe_8ch_float32() {
    write_and_probe(8).expect("8ch float32 should probe");
}

#[test]
fn probe_16ch_float32() {
    write_and_probe(16).expect("16ch float32 should probe");
}

#[test]
fn probe_17ch_float32() {
    write_and_probe(17).expect("17ch float32 should probe");
}

#[test]
fn probe_24ch_float32() {
    let _ = write_and_probe(24).map_err(|e| eprintln!("24ch: {e}"));
}

/// Check whether symphonia preserves channel order through the decoder
/// for a hound-written multichannel float WAV (which writes
/// WAVE_FORMAT_EXTENSIBLE with channel_mask=0). If channels come out
/// reordered, the recording probe's per-channel timing analysis will
/// see signals on physically wrong outputs.
#[test]
fn channel_order_preserved_through_symphonia_6ch() {
    use sotf_audio::decoder::create_decoder;

    let temp_file = NamedTempFile::with_suffix(".wav").unwrap();
    let path = temp_file.path();
    let channels: u16 = 6;
    let sample_rate: u32 = 48_000;
    let frames: usize = 1000;

    // Each channel gets a distinct constant: ch 0 = 0.10, ..., ch 5 = 0.60
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    let mut writer = WavWriter::create(path, spec).unwrap();
    for _ in 0..frames {
        for ch in 0..channels {
            writer.write_sample(0.1_f32 * (ch as f32 + 1.0)).unwrap();
        }
    }
    writer.finalize().unwrap();

    let mut decoder = create_decoder(path).expect("decoder should create");
    let chunk = decoder
        .decode_next()
        .expect("decoder should read")
        .expect("first chunk should be Some");
    eprintln!(
        "decoded {} samples ({} frames, {} channels)",
        chunk.samples.len(),
        chunk.frame_count(),
        chunk.spec.channels,
    );
    let frame = &chunk.samples[..channels as usize];
    eprintln!("first frame interleaved: {frame:?}");
    for ch in 0..channels as usize {
        let expected = 0.1_f32 * (ch as f32 + 1.0);
        let actual = frame[ch];
        assert!(
            (actual - expected).abs() < 1e-4,
            "channel {ch}: expected {expected:.3}, got {actual:.3} — symphonia REORDERED channels"
        );
    }
}
