use super::consts::DFF_HEADER_LEN;
use super::consts::DSD_DECODE_CHUNK_FRAMES;
use super::consts::DSF_HEADER_LEN;
use super::consts::DSF_ROOT_CHUNK_SIZE;
use super::dff_pcm_decoder::DffPcmDecoder;
use super::dsf_pcm_decoder::{DsfPcmDecoder, dsf_sample};
use crate::decoder::core::{AudioDecoder, DecodedAudio};
use crate::decoder::error::AudioDecoderError;
use crate::decoder::formats::AudioFormat;
use std::io::Write;

fn chunk(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(DSF_HEADER_LEN + payload.len());
    bytes.extend_from_slice(id);
    bytes.extend_from_slice(&(DSF_HEADER_LEN as u64 + payload.len() as u64).to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

fn chunk_be(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(DFF_HEADER_LEN + payload.len() + (payload.len() & 1));
    bytes.extend_from_slice(id);
    bytes.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    bytes.extend_from_slice(payload);
    if !payload.len().is_multiple_of(2) {
        bytes.push(0);
    }
    bytes
}

fn minimal_dsf(channels: u32, sample_count: u64, block_size: u32, payload: &[u8]) -> Vec<u8> {
    minimal_dsf_with_bit_order(channels, sample_count, block_size, 1, payload)
}

fn minimal_dsf_with_bit_order(
    channels: u32,
    sample_count: u64,
    block_size: u32,
    bits_per_sample: u32,
    payload: &[u8],
) -> Vec<u8> {
    let mut fmt = Vec::new();
    fmt.extend_from_slice(&1u32.to_le_bytes());
    fmt.extend_from_slice(&0u32.to_le_bytes());
    fmt.extend_from_slice(&2u32.to_le_bytes());
    fmt.extend_from_slice(&channels.to_le_bytes());
    fmt.extend_from_slice(&2_822_400u32.to_le_bytes());
    fmt.extend_from_slice(&bits_per_sample.to_le_bytes());
    fmt.extend_from_slice(&sample_count.to_le_bytes());
    fmt.extend_from_slice(&block_size.to_le_bytes());
    fmt.extend_from_slice(&0u32.to_le_bytes());

    let fmt_chunk = chunk(b"fmt ", &fmt);
    let data_chunk = chunk(b"data", payload);
    let file_size = DSF_ROOT_CHUNK_SIZE + fmt_chunk.len() + data_chunk.len();

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"DSD ");
    bytes.extend_from_slice(&(DSF_ROOT_CHUNK_SIZE as u64).to_le_bytes());
    bytes.extend_from_slice(&(file_size as u64).to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes());
    bytes.extend_from_slice(&fmt_chunk);
    bytes.extend_from_slice(&data_chunk);
    bytes
}

fn minimal_dff(channels: u16, payload: &[u8]) -> Vec<u8> {
    let fs = chunk_be(b"FS  ", &2_822_400u32.to_be_bytes());

    let mut chnl = Vec::new();
    chnl.extend_from_slice(&channels.to_be_bytes());
    for channel in 0..channels {
        let id = if channel == 0 {
            *b"SLFT"
        } else if channel == 1 {
            *b"SRGT"
        } else {
            *b"C___"
        };
        chnl.extend_from_slice(&id);
    }
    let chnl = chunk_be(b"CHNL", &chnl);

    let mut cmpr = Vec::new();
    cmpr.extend_from_slice(b"DSD ");
    cmpr.push(0);
    let cmpr = chunk_be(b"CMPR", &cmpr);

    let mut prop_payload = Vec::new();
    prop_payload.extend_from_slice(b"SND ");
    prop_payload.extend_from_slice(&fs);
    prop_payload.extend_from_slice(&chnl);
    prop_payload.extend_from_slice(&cmpr);
    let prop = chunk_be(b"PROP", &prop_payload);
    let dsd = chunk_be(b"DSD ", payload);
    let form_size = 4 + prop.len() + dsd.len();

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"FRM8");
    bytes.extend_from_slice(&(form_size as u64).to_be_bytes());
    bytes.extend_from_slice(b"DSD ");
    bytes.extend_from_slice(&prop);
    bytes.extend_from_slice(&dsd);
    bytes
}

#[test]
fn dsf_pcm_decoder_converts_one_bit_samples_to_pcm_frames() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&[0xff; 8]);
    payload.extend_from_slice(&[0x00; 8]);
    let bytes = minimal_dsf(2, 64, 8, &payload);
    let mut decoder = DsfPcmDecoder::from_bytes(bytes).unwrap();
    let mut dest = DecodedAudio::new(decoder.spec().clone());

    let frames = decoder.decode_into(&mut dest).unwrap();

    assert_eq!(frames, 1);
    assert_eq!(dest.spec.sample_rate, 44_100);
    assert_eq!(dest.spec.channels, 2);
    assert!(dest.samples[0] > 0.0 && dest.samples[0] <= 1.0);
    assert!(dest.samples[1] < 0.0 && dest.samples[1] >= -1.0);
    assert_eq!(decoder.position(), 1);
}

#[test]
fn dsf_pcm_decoder_seek_and_eof_are_pcm_frame_based() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&[0xff; 16]);
    let bytes = minimal_dsf(1, 128, 16, &payload);
    let mut decoder = DsfPcmDecoder::from_bytes(bytes).unwrap();
    let mut dest = DecodedAudio::new(decoder.spec().clone());

    decoder.seek(1).unwrap();
    let frames = decoder.decode_into(&mut dest).unwrap();
    assert_eq!(frames, 1);
    assert_eq!(dest.frame_position, 1);
    assert!(dest.samples[0] > 0.0 && dest.samples[0] <= 1.0);
    assert!(decoder.is_eof());
    assert!(decoder.seek(3).is_err());
}

#[test]
fn dsf_pcm_decoder_clears_reused_destination_at_eof() {
    let payload = vec![0xff; 8];
    let bytes = minimal_dsf(1, 64, 8, &payload);
    let mut decoder = DsfPcmDecoder::from_bytes(bytes).unwrap();
    let mut dest = DecodedAudio::new(decoder.spec().clone());

    assert_eq!(decoder.decode_into(&mut dest).unwrap(), 1);
    dest.samples.extend_from_slice(&[0.25, -0.25]);
    dest.frame_position = 99;

    assert_eq!(decoder.decode_into(&mut dest).unwrap(), 0);
    assert!(dest.samples.is_empty());
    assert_eq!(dest.spec, *decoder.spec());
    assert_eq!(dest.frame_position, decoder.position());
}

#[test]
fn dff_pcm_decoder_converts_uncompressed_interleaved_dsd_bytes() {
    let mut payload = Vec::new();
    for _ in 0..8 {
        payload.push(0xff);
        payload.push(0x00);
    }
    let bytes = minimal_dff(2, &payload);
    let mut decoder = DffPcmDecoder::from_bytes(bytes).unwrap();
    let mut dest = DecodedAudio::new(decoder.spec().clone());

    let frames = decoder.decode_into(&mut dest).unwrap();

    assert_eq!(frames, 1);
    assert_eq!(dest.spec.sample_rate, 44_100);
    assert_eq!(dest.spec.channels, 2);
    assert!(dest.samples[0] > 0.0 && dest.samples[0] <= 1.0);
    assert!(dest.samples[1] < 0.0 && dest.samples[1] >= -1.0);
    assert_eq!(decoder.format(), AudioFormat::DsdDff);
}

#[test]
fn dff_pcm_decoder_clears_reused_destination_at_eof() {
    let bytes = minimal_dff(1, &[0xff; 8]);
    let mut decoder = DffPcmDecoder::from_bytes(bytes).unwrap();
    let mut dest = DecodedAudio::new(decoder.spec().clone());

    assert_eq!(decoder.decode_into(&mut dest).unwrap(), 1);
    dest.samples.extend_from_slice(&[0.25, -0.25]);
    dest.frame_position = 99;

    assert_eq!(decoder.decode_into(&mut dest).unwrap(), 0);
    assert!(dest.samples.is_empty());
    assert_eq!(dest.spec, *decoder.spec());
    assert_eq!(dest.frame_position, decoder.position());
}

#[test]
fn dff_pcm_decoder_rejects_dst_compression() {
    let mut bytes = minimal_dff(1, &[0xff; 8]);
    let pos = bytes
        .windows(4)
        .position(|window| window == b"CMPR")
        .expect("compression marker should exist");
    bytes[pos + DFF_HEADER_LEN..pos + DFF_HEADER_LEN + 4].copy_from_slice(b"DST ");

    let err = match DffPcmDecoder::from_bytes(bytes) {
        Ok(_) => panic!("DST-compressed DFF should be rejected"),
        Err(err) => err,
    };
    assert!(
        matches!(err, AudioDecoderError::UnsupportedFormat(message) if message.contains("DST"))
    );
}

#[test]
fn dsf_decoder_honors_lsb_and_msb_bit_order() {
    let payload = vec![0x01; 64];
    let lsb_bytes = minimal_dsf_with_bit_order(1, 512, 64, 1, &payload);
    let msb_bytes = minimal_dsf_with_bit_order(1, 512, 64, 8, &payload);
    let mut lsb = DsfPcmDecoder::from_bytes(lsb_bytes).unwrap();
    let mut msb = DsfPcmDecoder::from_bytes(msb_bytes).unwrap();
    let mut lsb_audio = DecodedAudio::new(lsb.spec().clone());
    let mut msb_audio = DecodedAudio::new(msb.spec().clone());

    lsb.decode_into(&mut lsb_audio).unwrap();
    msb.decode_into(&mut msb_audio).unwrap();

    assert!(lsb.lsb_first);
    assert!(!msb.lsb_first);
    assert_ne!(lsb_audio.samples, msb_audio.samples);

    assert_eq!(dsf_sample(&[0x01], 1, 1, true, 0, 0, 8), 1.0);
    assert_eq!(dsf_sample(&[0x01], 1, 1, true, 0, 1, 8), -1.0);
    assert_eq!(dsf_sample(&[0x01], 1, 1, false, 0, 0, 8), -1.0);
    assert_eq!(dsf_sample(&[0x01], 1, 1, false, 0, 7, 8), 1.0);
}

#[test]
fn dsf_seek_rebuilds_filter_history() {
    let payload: Vec<u8> = (0..1024)
        .map(|index| (index as u8).wrapping_mul(73).wrapping_add(19))
        .collect();
    let bytes = minimal_dsf(1, 8192, 1024, &payload);
    let mut sequential = DsfPcmDecoder::from_bytes(bytes.clone()).unwrap();
    let mut all_audio = DecodedAudio::new(sequential.spec().clone());
    sequential.decode_into(&mut all_audio).unwrap();

    let mut seeked = DsfPcmDecoder::from_bytes(bytes).unwrap();
    let mut seek_audio = DecodedAudio::new(seeked.spec().clone());
    seeked.seek(60).unwrap();
    seeked.decode_into(&mut seek_audio).unwrap();

    assert_eq!(seek_audio.frame_position, 60);
    assert!((seek_audio.samples[0] - all_audio.samples[60]).abs() < 1e-6);
}

#[test]
fn dsf_filter_state_is_continuous_across_decode_chunks() {
    let frames = DSD_DECODE_CHUNK_FRAMES + 1;
    let payload: Vec<u8> = (0..frames * 8)
        .map(|index| (index as u8).wrapping_mul(29).wrapping_add(7))
        .collect();
    let bytes = minimal_dsf(1, frames * 64, payload.len() as u32, &payload);
    let mut chunked = DsfPcmDecoder::from_bytes(bytes.clone()).unwrap();
    let mut first = DecodedAudio::new(chunked.spec().clone());
    let mut second = DecodedAudio::new(chunked.spec().clone());
    assert_eq!(
        chunked.decode_into(&mut first).unwrap() as u64,
        DSD_DECODE_CHUNK_FRAMES
    );
    assert_eq!(chunked.decode_into(&mut second).unwrap(), 1);

    let mut seeked = DsfPcmDecoder::from_bytes(bytes).unwrap();
    let mut reference = DecodedAudio::new(seeked.spec().clone());
    seeked.seek(DSD_DECODE_CHUNK_FRAMES).unwrap();
    seeked.decode_into(&mut reference).unwrap();

    assert!((second.samples[0] - reference.samples[0]).abs() < 1e-6);
}

#[test]
fn alternating_short_dsf_has_neutral_start_and_eof_padding() {
    // LSB-first 0x55 is a phase-stable 50%-density stream with zero baseband.
    let bytes = minimal_dsf(1, 64, 8, &[0x55; 8]);
    let mut decoder = DsfPcmDecoder::from_bytes(bytes).unwrap();
    let mut audio = DecodedAudio::new(decoder.spec().clone());

    assert_eq!(decoder.decode_into(&mut audio).unwrap(), 1);
    assert!(
        audio.samples[0].abs() < 0.01,
        "boundary padding injected {}",
        audio.samples[0]
    );
}

#[test]
fn dsd_parsers_reject_implausible_channel_counts() {
    let dsf_err = match DsfPcmDecoder::from_bytes(minimal_dsf(65, 64, 8, &[])) {
        Ok(_) => panic!("implausible DSF channel count should be rejected"),
        Err(err) => err,
    };
    assert!(
        matches!(dsf_err, AudioDecoderError::UnsupportedFormat(message) if message.contains("channel count"))
    );

    let dff_err = match DffPcmDecoder::from_bytes(minimal_dff(65, &[])) {
        Ok(_) => panic!("implausible DFF channel count should be rejected"),
        Err(err) => err,
    };
    assert!(
        matches!(dff_err, AudioDecoderError::UnsupportedFormat(message) if message.contains("channel count"))
    );
}

#[test]
fn dsf_rejects_truncated_declared_sample_data() {
    let err = match DsfPcmDecoder::from_bytes(minimal_dsf(2, 1024, 128, &[0xff; 16])) {
        Ok(_) => panic!("truncated DSF data should be rejected"),
        Err(err) => err,
    };
    assert!(
        matches!(err, AudioDecoderError::InvalidFile(message) if message.contains("truncated"))
    );
}

#[test]
fn dff_seek_rebuilds_filter_history() {
    let payload: Vec<u8> = (0..1024)
        .map(|index| (index as u8).wrapping_mul(41).wrapping_add(23))
        .collect();
    let bytes = minimal_dff(1, &payload);
    let mut sequential = DffPcmDecoder::from_bytes(bytes.clone()).unwrap();
    let mut all_audio = DecodedAudio::new(sequential.spec().clone());
    sequential.decode_into(&mut all_audio).unwrap();

    let mut seeked = DffPcmDecoder::from_bytes(bytes).unwrap();
    let mut seek_audio = DecodedAudio::new(seeked.spec().clone());
    seeked.seek(60).unwrap();
    seeked.decode_into(&mut seek_audio).unwrap();

    assert_eq!(seek_audio.frame_position, 60);
    assert!((seek_audio.samples[0] - all_audio.samples[60]).abs() < 1e-6);
}

#[test]
fn dsf_file_decoder_streams_sparse_multi_gigabyte_data_chunk() {
    const DATA_LEN: u64 = 2 * 1024 * 1024 * 1024;
    const CHANNELS: u32 = 2;
    const BLOCK_SIZE: u32 = 4096;
    let sample_count = (DATA_LEN / u64::from(CHANNELS)) * 8;
    let mut prefix = minimal_dsf(CHANNELS, sample_count, BLOCK_SIZE, &[]);
    let data_header = prefix
        .windows(4)
        .rposition(|window| window == b"data")
        .expect("DSF data header");
    let declared_chunk_size = DATA_LEN + DSF_HEADER_LEN as u64;
    prefix[data_header + 4..data_header + 12].copy_from_slice(&declared_chunk_size.to_le_bytes());
    let data_offset = (data_header + DSF_HEADER_LEN) as u64;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large-sparse.dsf");
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(&prefix[..data_header + DSF_HEADER_LEN])
        .unwrap();
    file.set_len(data_offset + DATA_LEN).unwrap();

    let decoder = DsfPcmDecoder::new(&path).expect("sparse DSF should open without slurping data");
    assert!(decoder.data.is_file_backed());
    assert_eq!(decoder.spec.total_frames, Some(sample_count / 64));
}

#[test]
fn dff_file_decoder_streams_sparse_multi_gigabyte_data_chunk() {
    const DATA_LEN: u64 = 2 * 1024 * 1024 * 1024;
    let mut prefix = minimal_dff(2, &[]);
    let data_header = prefix
        .windows(4)
        .rposition(|window| window == b"DSD ")
        .expect("DFF data header");
    prefix[data_header + 4..data_header + 12].copy_from_slice(&DATA_LEN.to_be_bytes());
    let data_offset = (data_header + DFF_HEADER_LEN) as u64;
    let file_len = data_offset + DATA_LEN;
    prefix[4..12].copy_from_slice(&(file_len - 12).to_be_bytes());

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large-sparse.dff");
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(&prefix[..data_header + DFF_HEADER_LEN])
        .unwrap();
    file.set_len(file_len).unwrap();

    let decoder = DffPcmDecoder::new(&path).expect("sparse DFF should open without slurping data");
    assert!(decoder.data.is_file_backed());
    assert_eq!(decoder.spec.total_frames, Some(DATA_LEN / 2 / 8));
}

#[test]
fn file_backed_dsd_decoders_match_in_memory_reference() {
    let dsf_bytes = minimal_dsf(2, 512, 64, &[0xa5; 128]);
    let dff_bytes = minimal_dff(2, &[0x5a; 128]);
    let dir = tempfile::tempdir().unwrap();
    let dsf_path = dir.path().join("reference.dsf");
    let dff_path = dir.path().join("reference.dff");
    std::fs::write(&dsf_path, &dsf_bytes).unwrap();
    std::fs::write(&dff_path, &dff_bytes).unwrap();

    let mut dsf_file = DsfPcmDecoder::new(&dsf_path).unwrap();
    let mut dsf_memory = DsfPcmDecoder::from_bytes(dsf_bytes).unwrap();
    let mut dsf_file_audio = DecodedAudio::new(dsf_file.spec().clone());
    let mut dsf_memory_audio = DecodedAudio::new(dsf_memory.spec().clone());
    assert_eq!(
        dsf_file.decode_into(&mut dsf_file_audio).unwrap(),
        dsf_memory.decode_into(&mut dsf_memory_audio).unwrap()
    );
    assert_eq!(dsf_file_audio.samples, dsf_memory_audio.samples);

    let mut dff_file = DffPcmDecoder::new(&dff_path).unwrap();
    let mut dff_memory = DffPcmDecoder::from_bytes(dff_bytes).unwrap();
    let mut dff_file_audio = DecodedAudio::new(dff_file.spec().clone());
    let mut dff_memory_audio = DecodedAudio::new(dff_memory.spec().clone());
    assert_eq!(
        dff_file.decode_into(&mut dff_file_audio).unwrap(),
        dff_memory.decode_into(&mut dff_memory_audio).unwrap()
    );
    assert_eq!(dff_file_audio.samples, dff_memory_audio.samples);
}
