use super::super::validate::validate_gapless_source_compatible;
use hound::{WavSpec, WavWriter};
use tempfile::NamedTempFile;

fn create_test_wav_with_channels(channels: u16) -> NamedTempFile {
    let spec = WavSpec {
        channels,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let temp_file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    let mut writer = WavWriter::create(temp_file.path(), spec).unwrap();
    for _ in 0..128 {
        for _ in 0..channels {
            writer.write_sample(0i16).unwrap();
        }
    }
    writer.finalize().unwrap();
    temp_file
}

#[test]
fn validate_gapless_source_rejects_channel_mismatch() {
    let stereo = create_test_wav_with_channels(2);
    let mono = create_test_wav_with_channels(1);

    assert!(validate_gapless_source_compatible(&stereo.path().to_path_buf().into(), 2).is_ok());

    let err = validate_gapless_source_compatible(&mono.path().to_path_buf().into(), 2).unwrap_err();
    assert!(err.contains("channel"));
    assert!(err.contains("expected 2"));
    assert!(err.contains("got 1"));
}
