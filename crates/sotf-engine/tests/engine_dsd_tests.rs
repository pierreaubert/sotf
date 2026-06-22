//! DSD format switching tests.
//!
//! These tests focus on decoder configuration and planning logic. They do not
//! require a DSD-capable audio device; bitstream output modes are expected to
//! report that the current backend cannot carry DSD frames and fall back or
//! error accordingly.

use sotf_audio::decoder::AudioSource;
use sotf_audio::decoder::core::create_decoder_from_source_with_dsd_mode_and_metadata;
use sotf_audio::engine::{
    DsdOutputBackend, DsdOutputMode, DsdOutputPlan, DsdOutputStatus, plan_dsd_output,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DSF_FILE_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Write a minimal valid DSF file under `CARGO_TARGET_TMPDIR`.
///
/// The file contains a tiny amount of DSD data (stereo, DSD64). It is valid
/// enough for the DSF parser to recognize the format and for the PCM decoder
/// to produce a few frames.
fn make_minimal_dsf() -> PathBuf {
    let tmp_dir = std::env::var_os("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let path = tmp_dir.join(format!(
        "sotf_engine_dsd_test_{}_{}.dsf",
        std::process::id(),
        DSF_FILE_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));

    let channels: u32 = 2;
    let sample_rate: u32 = 2_822_400; // DSD64
    let bits_per_sample: u32 = 1;
    // One full PCM decode frame needs 64 DSD samples per channel.
    let sample_count: u64 = 64 * 64; // 64 PCM frames worth
    let block_size_per_channel: u32 = 64;

    // DSD data bytes per channel = sample_count / 8
    let bytes_per_channel = (sample_count / 8) as usize;
    let blocks = bytes_per_channel / block_size_per_channel as usize;
    let data_len = blocks * block_size_per_channel as usize * channels as usize;
    let data = vec![0u8; data_len];

    let mut bytes = Vec::new();

    // Root chunk: "DSD " + chunk_size(28) + file_size(8) + metadata_ptr(8)
    bytes.extend_from_slice(b"DSD ");
    bytes.extend_from_slice(&28u64.to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes()); // file size (unused by parser)
    bytes.extend_from_slice(&0u64.to_le_bytes()); // metadata pointer (0 = none)

    // fmt chunk: "fmt " + chunk_size(52) + payload(40 bytes)
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&52u64.to_le_bytes());
    // fmt payload
    let mut fmt_payload = vec![0u8; 40];
    // format id at offset 4
    fmt_payload[4..8].copy_from_slice(&0u32.to_le_bytes());
    // channel count at offset 12
    fmt_payload[12..16].copy_from_slice(&channels.to_le_bytes());
    // sample rate at offset 16
    fmt_payload[16..20].copy_from_slice(&sample_rate.to_le_bytes());
    // bits per sample at offset 20
    fmt_payload[20..24].copy_from_slice(&bits_per_sample.to_le_bytes());
    // sample count at offset 24
    fmt_payload[24..32].copy_from_slice(&sample_count.to_le_bytes());
    // block size per channel at offset 32
    fmt_payload[32..36].copy_from_slice(&block_size_per_channel.to_le_bytes());
    bytes.extend_from_slice(&fmt_payload);

    // data chunk: "data" + chunk_size(12 + data_len) + payload
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&((12 + data_len) as u64).to_le_bytes());
    bytes.extend_from_slice(&data);

    std::fs::write(&path, bytes).expect("failed to write minimal DSF file");
    path
}

#[test]
fn dsf_file_is_recognized_and_decodes_to_pcm() {
    let dsf_path = make_minimal_dsf();
    let source = AudioSource::File(dsf_path);

    let (decoder, metadata_rx) =
        create_decoder_from_source_with_dsd_mode_and_metadata(&source, DsdOutputMode::PcmDecode)
            .expect("PcmDecode should produce a PCM decoder for a DSF source");

    assert_eq!(decoder.spec().sample_rate, 2_822_400 / 64);
    assert_eq!(decoder.spec().channels, 2);
    assert!(
        metadata_rx.is_none(),
        "local files have no live metadata receiver"
    );
}

#[test]
fn dsd_disabled_rejects_dsf() {
    let dsf_path = make_minimal_dsf();
    let source = AudioSource::File(dsf_path);

    let result =
        create_decoder_from_source_with_dsd_mode_and_metadata(&source, DsdOutputMode::Disabled);

    assert!(
        matches!(result, Err(sotf_audio::AudioDecoderError::UnsupportedFormat(ref m)) if m.contains("DSD output is disabled")),
        "unexpected result: {:?}",
        result.as_ref().err()
    );
}

#[test]
fn dsd_preferred_modes_fallback_to_pcm_decode() {
    let dsf_path = make_minimal_dsf();
    let source = AudioSource::File(dsf_path);

    for mode in [DsdOutputMode::DopPreferred, DsdOutputMode::NativePreferred] {
        let (decoder, _) = create_decoder_from_source_with_dsd_mode_and_metadata(&source, mode)
            .unwrap_or_else(|e| panic!("{:?} should fall back to PCM: {}", mode, e));
        assert_eq!(decoder.spec().sample_rate, 2_822_400 / 64);
        assert_eq!(decoder.spec().channels, 2);
    }
}

#[test]
fn dsd_required_bitstream_modes_error_for_dsf() {
    let dsf_path = make_minimal_dsf();
    let source = AudioSource::File(dsf_path);

    let dop =
        create_decoder_from_source_with_dsd_mode_and_metadata(&source, DsdOutputMode::DopRequired);
    assert!(
        matches!(dop, Err(sotf_audio::AudioDecoderError::UnsupportedFormat(ref m)) if m.contains("cannot carry bit-perfect DoP frames")),
        "DoPRequired should error: {:?}",
        dop.as_ref().err()
    );

    let native = create_decoder_from_source_with_dsd_mode_and_metadata(
        &source,
        DsdOutputMode::NativeRequired,
    );
    assert!(
        matches!(native, Err(sotf_audio::AudioDecoderError::UnsupportedFormat(ref m)) if m.contains("cannot carry native DSD frames")),
        "NativeRequired should error: {:?}",
        native.as_ref().err()
    );
}

#[test]
fn plan_dsd_output_maps_modes_to_expected_backend_and_status() {
    let cases: &[(DsdOutputMode, DsdOutputBackend, DsdOutputStatus)] = &[
        (
            DsdOutputMode::Disabled,
            DsdOutputBackend::Disabled,
            DsdOutputStatus::Disabled,
        ),
        (
            DsdOutputMode::PcmDecode,
            DsdOutputBackend::PcmDecoder,
            DsdOutputStatus::PcmDecodeAvailable,
        ),
        (
            DsdOutputMode::DopPreferred,
            DsdOutputBackend::PcmDecoder,
            DsdOutputStatus::DopFallbackPcm,
        ),
        (
            DsdOutputMode::DopRequired,
            DsdOutputBackend::DopBitstream,
            DsdOutputStatus::DopUnavailable,
        ),
        (
            DsdOutputMode::NativePreferred,
            DsdOutputBackend::PcmDecoder,
            DsdOutputStatus::NativeFallbackPcm,
        ),
        (
            DsdOutputMode::NativeRequired,
            DsdOutputBackend::NativeBitstream,
            DsdOutputStatus::NativeUnavailable,
        ),
    ];

    for &(mode, expected_backend, expected_status) in cases {
        let plan: DsdOutputPlan = plan_dsd_output(mode);
        assert_eq!(
            plan.requested, mode,
            "requested mode should be preserved in plan"
        );
        assert_eq!(
            plan.backend, expected_backend,
            "unexpected backend for {:?}",
            mode
        );
        assert_eq!(
            plan.status, expected_status,
            "unexpected status for {:?}",
            mode
        );

        // Preferred/Required modes should always carry a human-readable reason.
        if !matches!(mode, DsdOutputMode::Disabled) {
            assert!(
                plan.reason.as_deref().unwrap_or("").len() > 5,
                "{:?} should have a descriptive reason",
                mode
            );
        }
    }
}
