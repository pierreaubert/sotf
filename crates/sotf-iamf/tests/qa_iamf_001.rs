// ============================================================================
// QA-IAMF-001: Malformed input, bounds, and synthetic decode tests
// ============================================================================
//
// Covers the public parser/decode API of `sotf-iamf` with adversarial and
// edge-case inputs. All tests use only the public crate surface.

use sotf_iamf::IamfDecoder;
use sotf_iamf::error::IamfError;
use sotf_iamf::obu::ObuType;
use sotf_iamf::obu::parser::{
    parse_audio_element, parse_codec_config, parse_descriptors, parse_mix_presentation,
    parse_obu_header, parse_parameter_block, parse_sequence_header, parse_temporal_unit,
};
use std::io::Cursor;

// =============================================================================
// Bitstream construction helpers
// =============================================================================

fn leb128_u32(value: u32) -> Vec<u8> {
    let mut value = value;
    let mut out = Vec::new();
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
    out
}

fn obu_header(obu_type: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + payload.len());
    out.push(obu_type << 3);
    out.extend_from_slice(&leb128_u32(payload.len() as u32));
    out.extend_from_slice(payload);
    out
}

fn seq_header_payload() -> Vec<u8> {
    vec![b'i', b'a', b'm', b'f', 0, 0]
}

fn seq_header_obu() -> Vec<u8> {
    obu_header(31, &seq_header_payload())
}

fn codec_config_lpcm_payload() -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&leb128_u32(0)); // codec_config_id
    payload.extend_from_slice(b"ipcm");
    payload.extend_from_slice(&leb128_u32(2)); // num_samples_per_frame
    payload.extend_from_slice(&0_i16.to_be_bytes()); // audio_roll_distance
    payload.push(0); // format_flags
    payload.push(16); // sample_size
    payload.extend_from_slice(&48_000u32.to_be_bytes()); // sample_rate
    payload
}

fn codec_config_lpcm_obu() -> Vec<u8> {
    obu_header(0, &codec_config_lpcm_payload())
}

fn audio_element_stereo_payload() -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&leb128_u32(0)); // audio_element_id
    payload.push(0x00); // type byte: Channel (top 3 bits = 0)
    payload.extend_from_slice(&leb128_u32(0)); // codec_config_id
    payload.extend_from_slice(&leb128_u32(1)); // num_substreams
    payload.extend_from_slice(&leb128_u32(0)); // substream_id 0
    payload.extend_from_slice(&leb128_u32(0)); // num_parameters

    // Scalable channel config: 1 layer
    payload.push(0x20); // num_layers=1 (top 3 bits)
    // Layer: layout_idx=1 (Stereo), no gain flags, substream_count=1, coupled=1
    payload.push(0x10);
    payload.push(0x01);
    payload.push(0x01);
    payload
}

fn audio_element_stereo_obu() -> Vec<u8> {
    obu_header(1, &audio_element_stereo_payload())
}

fn mix_presentation_stereo_payload() -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&leb128_u32(0)); // mix_presentation_id
    payload.extend_from_slice(&leb128_u32(1)); // count_label
    payload.extend_from_slice(b"en\0"); // language
    payload.extend_from_slice(b"Stereo\0"); // label

    payload.extend_from_slice(&leb128_u32(1)); // num_sub_mixes
    payload.extend_from_slice(&leb128_u32(1)); // num_audio_elements
    payload.extend_from_slice(&leb128_u32(0)); // audio_element_id
    payload.extend_from_slice(b"Stereo\0"); // element label

    // rendering_config
    payload.push(0x00); // headphones_rendering_mode
    payload.extend_from_slice(&leb128_u32(0)); // rendering_config_extension_size

    // element mix gain
    payload.extend_from_slice(&leb128_u32(0)); // parameter_id
    payload.extend_from_slice(&leb128_u32(0)); // parameter_rate
    payload.push(0x80); // param_definition_mode=true
    payload.extend_from_slice(&0_i16.to_be_bytes()); // default_mix_gain_db

    // output mix gain
    payload.extend_from_slice(&leb128_u32(1)); // parameter_id
    payload.extend_from_slice(&leb128_u32(0)); // parameter_rate
    payload.push(0x80); // param_definition_mode=true
    payload.extend_from_slice(&0_i16.to_be_bytes()); // default_mix_gain_db

    // output layout: loudspeaker (layout_type=0), sound_system=1 (Stereo)
    payload.push(0x00);
    payload.push(0x10);

    // loudness info
    payload.push(0x00); // info_type
    payload.extend_from_slice(&0_i16.to_be_bytes()); // integrated_loudness
    payload.extend_from_slice(&0_i16.to_be_bytes()); // digital_peak

    payload
}

fn mix_presentation_stereo_obu() -> Vec<u8> {
    obu_header(2, &mix_presentation_stereo_payload())
}

fn temporal_delimiter_obu() -> Vec<u8> {
    obu_header(4, &[])
}

fn audio_frame_obu(substream_id: u32, samples: &[i16]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&leb128_u32(substream_id));
    for s in samples {
        payload.extend_from_slice(&s.to_be_bytes());
    }
    obu_header(5, &payload)
}

fn minimal_lpcm_stream() -> Vec<u8> {
    let mut stream = Vec::new();
    stream.extend_from_slice(&seq_header_obu());
    stream.extend_from_slice(&codec_config_lpcm_obu());
    stream.extend_from_slice(&audio_element_stereo_obu());
    stream.extend_from_slice(&mix_presentation_stereo_obu());
    stream.extend_from_slice(&temporal_delimiter_obu());
    // 2 frames of stereo silence = 4 samples = 8 bytes
    stream.extend_from_slice(&audio_frame_obu(0, &[0i16; 4]));
    stream
}

// =============================================================================
// Positive / end-to-end decode tests
// =============================================================================

#[test]
fn parse_descriptors_extracts_temporal_offset() {
    let data = minimal_lpcm_stream();
    let (desc, offset) = parse_descriptors(&data).unwrap();
    assert_eq!(desc.codec_configs.len(), 1);
    assert_eq!(desc.audio_elements.len(), 1);
    assert_eq!(desc.mix_presentations.len(), 1);
    assert!(offset < data.len());
    // The offset should point at the temporal delimiter.
    assert_eq!(data[offset], 4 << 3);
}

#[test]
fn open_and_decode_minimal_lpcm_stream() {
    let data = minimal_lpcm_stream();
    let mut decoder = IamfDecoder::open(Cursor::new(&data)).unwrap();
    assert_eq!(decoder.spec().sample_rate, 48000);
    assert_eq!(decoder.spec().bit_depth, 16);
    assert_eq!(decoder.spec().output_channels, 2);
    assert_eq!(decoder.spec().num_samples_per_frame, 2);

    let mut output = vec![0.0_f32; 4];
    let frames = decoder.decode_next(&mut output).unwrap();
    assert_eq!(frames, 2);
    for s in &output {
        assert!(s.abs() < 1e-6, "expected silence, got {s}");
    }
    assert!(decoder.is_eof());
}

// =============================================================================
// Malformed header / descriptor tests
// =============================================================================

#[test]
fn parse_obu_header_truncated_leb128_payload_size() {
    // OBU type=31, payload_size leb128 never terminates.
    let data = [31 << 3, 0xff];
    assert!(parse_obu_header(&data).is_err());
}

#[test]
fn parse_obu_header_zero_length_payload() {
    let data = [31 << 3, 0];
    let (header, size) = parse_obu_header(&data).unwrap();
    assert_eq!(header.obu_type, ObuType::SequenceHeader);
    assert_eq!(header.payload_size, 0);
    assert_eq!(size, 2);
}

#[test]
fn parse_obu_header_oversized_descriptor() {
    let mut data = vec![31 << 3];
    data.extend_from_slice(&leb128_u32(1000)); // payload_size=1000
    data.extend_from_slice(&seq_header_payload()); // only 6 bytes payload
    let err = parse_obu_header(&data).unwrap_err();
    assert!(matches!(err, IamfError::TruncatedObu { .. }));
}

#[test]
fn parse_sequence_header_bad_magic() {
    assert!(matches!(
        parse_sequence_header(b"xxxx\0\0"),
        Err(IamfError::InvalidMagic)
    ));
}

#[test]
fn parse_descriptors_rejects_bad_magic() {
    let mut data = seq_header_obu();
    data[3] = b'x'; // corrupt "iamf"
    assert!(matches!(
        parse_descriptors(&data).unwrap_err(),
        IamfError::InvalidMagic
    ));
}

#[test]
fn parse_descriptors_rejects_missing_sequence_header() {
    // Only a codec config OBU, no sequence header.
    let data = codec_config_lpcm_obu();
    assert!(matches!(
        parse_descriptors(&data).unwrap_err(),
        IamfError::InvalidMagic
    ));
}

#[test]
fn parse_codec_config_unknown_codec() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&leb128_u32(0)); // codec_config_id
    payload.extend_from_slice(b"xxxx"); // unsupported codec_id
    let err = parse_codec_config(&payload).unwrap_err();
    assert!(matches!(err, IamfError::UnsupportedCodec(_)));
}

#[test]
fn parse_codec_config_truncated() {
    // codec_config_id only, no codec_id.
    let payload = leb128_u32(0);
    assert!(parse_codec_config(&payload).is_err());
}

// =============================================================================
// Out-of-range count / unbounded allocation tests
// =============================================================================

#[test]
fn parse_audio_element_rejects_out_of_range_substream_count() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&leb128_u32(0)); // audio_element_id
    payload.push(0x00); // Channel
    payload.extend_from_slice(&leb128_u32(0)); // codec_config_id
    payload.extend_from_slice(&leb128_u32(100)); // 100 substreams, only 4 bytes remain
    payload.extend_from_slice(&[0u8; 4]);
    let err = parse_audio_element(&payload).unwrap_err();
    assert!(matches!(err, IamfError::ParseError(_)));
}

#[test]
fn parse_audio_element_rejects_unbounded_parameter_subblocks() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&leb128_u32(0)); // audio_element_id
    payload.push(0x00); // Channel
    payload.extend_from_slice(&leb128_u32(0)); // codec_config_id
    payload.extend_from_slice(&leb128_u32(1)); // num_substreams
    payload.extend_from_slice(&leb128_u32(0)); // substream_id
    payload.extend_from_slice(&leb128_u32(1)); // num_parameters
    payload.extend_from_slice(&leb128_u32(0)); // parameter_definition_type = MixGain
    payload.extend_from_slice(&leb128_u32(0)); // parameter_id
    payload.extend_from_slice(&leb128_u32(0)); // parameter_rate
    payload.push(0x00); // param_definition_mode=false
    payload.extend_from_slice(&leb128_u32(10)); // duration
    payload.extend_from_slice(&leb128_u32(0)); // constant_subblock_duration=0
    payload.extend_from_slice(&leb128_u32(0xffff_ffff)); // huge num_subblocks
    payload.extend_from_slice(&[0u8; 4]);
    let err = parse_audio_element(&payload).unwrap_err();
    assert!(matches!(err, IamfError::ParseError(_)));
}

#[test]
fn parse_mix_presentation_rejects_out_of_range_count_label() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&leb128_u32(0)); // mix_presentation_id
    payload.extend_from_slice(&leb128_u32(100)); // 100 labels, only 4 bytes remain
    payload.extend_from_slice(&[0u8; 4]);
    let err = parse_mix_presentation(&payload).unwrap_err();
    assert!(matches!(err, IamfError::ParseError(_)));
}

#[test]
fn parse_mix_presentation_rejects_unbounded_rendering_extension() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&leb128_u32(0)); // mix_presentation_id
    payload.extend_from_slice(&leb128_u32(1)); // count_label
    payload.extend_from_slice(b"en\0");
    payload.extend_from_slice(b"Stereo\0");
    payload.extend_from_slice(&leb128_u32(1)); // num_sub_mixes
    payload.extend_from_slice(&leb128_u32(1)); // num_audio_elements
    payload.extend_from_slice(&leb128_u32(0)); // audio_element_id
    payload.extend_from_slice(b"Stereo\0");
    payload.push(0x00); // headphones_rendering_mode
    payload.extend_from_slice(&leb128_u32(0xffff_ffff)); // huge extension size
    let err = parse_mix_presentation(&payload).unwrap_err();
    assert!(matches!(err, IamfError::ParseError(_)));
}

#[test]
fn parse_parameter_block_rejects_out_of_range_subblocks() {
    // MixGain parameter block with num_subblocks far larger than payload.
    let mut payload = Vec::new();
    payload.extend_from_slice(&leb128_u32(0)); // parameter_id
    payload.extend_from_slice(&leb128_u32(10)); // duration
    payload.extend_from_slice(&leb128_u32(0)); // constant_subblock_duration=0
    payload.extend_from_slice(&leb128_u32(0xffff_ffff)); // huge num_subblocks
    payload.extend_from_slice(&[0u8; 4]);
    let err = parse_parameter_block(&payload).unwrap_err();
    assert!(matches!(err, IamfError::ParseError(_)));
}

// =============================================================================
// Temporal unit malformed-input tests
// =============================================================================

#[test]
fn parse_temporal_unit_truncated_audio_frame() {
    // AudioFrame OBU declares payload_size=10 but only provides 2 bytes.
    let mut data = vec![5 << 3];
    data.extend_from_slice(&leb128_u32(10));
    data.extend_from_slice(&[0x00, 0x00]);
    assert!(parse_temporal_unit(&data).is_err());
}

#[test]
fn parse_temporal_unit_zero_length_audio_frame_payload_errors() {
    // Raw AudioFrame OBU with payload_size=0 cannot provide a substream_id.
    let data = [5 << 3, 0];
    assert!(matches!(
        parse_temporal_unit(&data).unwrap_err(),
        IamfError::ParseError(_)
    ));
}

#[test]
fn parse_temporal_unit_zero_length_delimiter_means_end_of_stream() {
    // A lone temporal delimiter marks the start of the temporal-unit section,
    // but with no following audio data the unit is empty -> EndOfStream.
    let data = temporal_delimiter_obu();
    assert!(matches!(
        parse_temporal_unit(&data).unwrap_err(),
        IamfError::EndOfStream
    ));
}

// =============================================================================
// Cross-reference validation via the public decoder API
// =============================================================================

#[test]
fn open_rejects_invalid_codec_config_id() {
    let mut stream = Vec::new();
    stream.extend_from_slice(&seq_header_obu());
    stream.extend_from_slice(&codec_config_lpcm_obu());

    // Audio element references codec_config_id=99, which does not exist.
    let mut bad_element = Vec::new();
    bad_element.extend_from_slice(&leb128_u32(0)); // audio_element_id
    bad_element.push(0x00); // Channel
    bad_element.extend_from_slice(&leb128_u32(99)); // invalid codec_config_id
    bad_element.extend_from_slice(&leb128_u32(1)); // num_substreams
    bad_element.extend_from_slice(&leb128_u32(0)); // substream_id
    bad_element.extend_from_slice(&leb128_u32(0)); // num_parameters
    bad_element.push(0x20);
    bad_element.push(0x10);
    bad_element.push(0x01);
    bad_element.push(0x01);
    stream.extend_from_slice(&obu_header(1, &bad_element));

    stream.extend_from_slice(&mix_presentation_stereo_obu());

    let err = IamfDecoder::open(Cursor::new(&stream)).unwrap_err();
    assert!(matches!(err, IamfError::UnknownCodecConfig(99)));
}

#[test]
fn open_rejects_no_mix_presentations() {
    // Descriptors only: sequence header + codec config + audio element, no mix.
    let mut stream = Vec::new();
    stream.extend_from_slice(&seq_header_obu());
    stream.extend_from_slice(&codec_config_lpcm_obu());
    stream.extend_from_slice(&audio_element_stereo_obu());
    let err = IamfDecoder::open(Cursor::new(&stream)).unwrap_err();
    assert!(matches!(err, IamfError::NoMixPresentations));
}
