use super::consts::{
    DFF_HEADER_LEN, DSD_TO_PCM_DECIMATION, DSF_FMT_CHUNK_SIZE, DSF_HEADER_LEN, DSF_ROOT_CHUNK_SIZE,
    MAX_DSD_CHANNELS,
};
use super::misc::checked_chunk_size;
use super::source::FILE_CACHE_BYTES;
use super::types::{DffFileMetadata, DffSoundProperties, DsfFileMetadata};
use crate::decoder::error::{AudioDecoderError, AudioDecoderResult};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

fn read_at(file: &mut File, offset: u64, dest: &mut [u8]) -> AudioDecoderResult<()> {
    file.seek(SeekFrom::Start(offset))?;
    file.read_exact(dest)?;
    Ok(())
}

fn invalid(message: impl Into<String>) -> AudioDecoderError {
    AudioDecoderError::InvalidFile(message.into())
}

fn validate_common(format: &str, sample_rate: u32, channels: u16) -> AudioDecoderResult<()> {
    if channels == 0 {
        return Err(invalid(format!("{format} file has zero channels")));
    }
    if channels > MAX_DSD_CHANNELS {
        return Err(AudioDecoderError::UnsupportedFormat(format!(
            "Unsupported {format} channel count {channels} (maximum {MAX_DSD_CHANNELS})"
        )));
    }
    if sample_rate < DSD_TO_PCM_DECIMATION as u32
        || !sample_rate.is_multiple_of(DSD_TO_PCM_DECIMATION as u32)
    {
        return Err(AudioDecoderError::UnsupportedFormat(format!(
            "Unsupported {format} sample rate {sample_rate}"
        )));
    }
    Ok(())
}

pub(super) fn parse_dsf_file(file: &mut File) -> AudioDecoderResult<DsfFileMetadata> {
    let file_len = file.metadata()?.len();
    let mut root = [0u8; DSF_ROOT_CHUNK_SIZE];
    read_at(file, 0, &mut root)
        .map_err(|error| invalid(format!("Unable to read DSF root chunk: {error}")))?;
    if &root[0..4] != b"DSD " {
        return Err(invalid("DSF file must start with a DSD chunk"));
    }
    let root_size = u64::from_le_bytes(root[4..12].try_into().unwrap());
    if root_size != DSF_ROOT_CHUNK_SIZE as u64 {
        return Err(invalid(format!(
            "Unexpected DSF root chunk size {root_size}"
        )));
    }

    let mut offset = DSF_ROOT_CHUNK_SIZE as u64;
    let mut sample_rate = None;
    let mut channels = None;
    let mut sample_count = None;
    let mut block_size_per_channel = None;
    let mut lsb_first = None;
    let mut data_chunk = None;

    while offset
        .checked_add(DSF_HEADER_LEN as u64)
        .is_some_and(|end| end <= file_len)
    {
        let mut header = [0u8; DSF_HEADER_LEN];
        read_at(file, offset, &mut header)?;
        let chunk_size = u64::from_le_bytes(header[4..12].try_into().unwrap());
        if chunk_size < DSF_HEADER_LEN as u64 {
            return Err(invalid(format!(
                "DSF chunk {:?} has invalid size {chunk_size}",
                String::from_utf8_lossy(&header[0..4])
            )));
        }
        let chunk_end = offset
            .checked_add(chunk_size)
            .ok_or_else(|| invalid("DSF chunk offset overflow"))?;
        if chunk_end > file_len {
            return Err(invalid(format!(
                "DSF chunk {:?} extends past end of file",
                String::from_utf8_lossy(&header[0..4])
            )));
        }
        let payload_start = offset + DSF_HEADER_LEN as u64;

        match &header[0..4] {
            b"fmt " => {
                if chunk_size < DSF_FMT_CHUNK_SIZE as u64 {
                    return Err(invalid(format!("DSF fmt chunk too small: {chunk_size}")));
                }
                let mut fmt = [0u8; DSF_FMT_CHUNK_SIZE - DSF_HEADER_LEN];
                read_at(file, payload_start, &mut fmt)?;
                let format_id = u32::from_le_bytes(fmt[4..8].try_into().unwrap());
                if format_id != 0 {
                    return Err(AudioDecoderError::UnsupportedFormat(format!(
                        "Unsupported DSF format id {format_id}"
                    )));
                }
                let channel_count = u32::from_le_bytes(fmt[12..16].try_into().unwrap());
                let bits_per_sample = u32::from_le_bytes(fmt[20..24].try_into().unwrap());
                if !matches!(bits_per_sample, 1 | 8) {
                    return Err(AudioDecoderError::UnsupportedFormat(format!(
                        "Unsupported DSF bits-per-sample {bits_per_sample}"
                    )));
                }
                sample_rate = Some(u32::from_le_bytes(fmt[16..20].try_into().unwrap()));
                channels = Some(u16::try_from(channel_count).map_err(|_| {
                    AudioDecoderError::UnsupportedFormat(format!(
                        "Unsupported DSF channel count {channel_count}"
                    ))
                })?);
                sample_count = Some(u64::from_le_bytes(fmt[24..32].try_into().unwrap()));
                block_size_per_channel = Some(
                    usize::try_from(u32::from_le_bytes(fmt[32..36].try_into().unwrap()))
                        .map_err(|_| invalid("DSF block size is too large"))?,
                );
                lsb_first = Some(bits_per_sample == 1);
            }
            b"data" => data_chunk = Some((payload_start, chunk_size - DSF_HEADER_LEN as u64)),
            _ => {}
        }
        offset = chunk_end;
    }

    let sample_rate = sample_rate.ok_or_else(|| invalid("Missing DSF fmt chunk"))?;
    let channels = channels.ok_or_else(|| invalid("Missing DSF channels"))?;
    let sample_count = sample_count.ok_or_else(|| invalid("Missing DSF sample count"))?;
    let block_size_per_channel =
        block_size_per_channel.ok_or_else(|| invalid("Missing DSF block size"))?;
    let lsb_first = lsb_first.ok_or_else(|| invalid("Missing DSF bit order"))?;
    let (data_offset, data_len) = data_chunk.ok_or_else(|| invalid("Missing DSF data chunk"))?;
    validate_common("DSF", sample_rate, channels)?;
    if block_size_per_channel == 0 {
        return Err(invalid("DSF file has zero block size"));
    }
    let block_group_bytes = block_size_per_channel
        .checked_mul(usize::from(channels))
        .ok_or_else(|| invalid("DSF block group size overflows"))?;
    if block_group_bytes > FILE_CACHE_BYTES {
        return Err(AudioDecoderError::UnsupportedFormat(format!(
            "Unsupported DSF block group size {block_group_bytes} bytes (maximum {FILE_CACHE_BYTES})"
        )));
    }
    let block_size = u64::try_from(block_size_per_channel)
        .map_err(|_| invalid("DSF block size is too large"))?;
    let required_data_len = sample_count
        .div_ceil(8)
        .div_ceil(block_size)
        .checked_mul(block_size)
        .and_then(|bytes| bytes.checked_mul(u64::from(channels)))
        .ok_or_else(|| invalid("DSF data size overflows"))?;
    if data_len < required_data_len {
        return Err(invalid(format!(
            "DSF data chunk is truncated: expected at least {required_data_len} bytes, found {data_len}"
        )));
    }

    Ok(DsfFileMetadata {
        sample_rate,
        channels,
        sample_count,
        block_size_per_channel,
        lsb_first,
        data_offset,
        data_len,
    })
}

fn parse_dff_properties(
    file: &mut File,
    payload_start: u64,
    payload_len: u64,
) -> AudioDecoderResult<DffSoundProperties> {
    if payload_len < 4 {
        return Ok(DffSoundProperties {
            sample_rate: None,
            channels: None,
            compression: None,
        });
    }
    let mut form = [0u8; 4];
    read_at(file, payload_start, &mut form)?;
    if &form != b"SND " {
        return Ok(DffSoundProperties {
            sample_rate: None,
            channels: None,
            compression: None,
        });
    }

    let payload_end = payload_start
        .checked_add(payload_len)
        .ok_or_else(|| invalid("DFF PROP chunk offset overflow"))?;
    let mut offset = payload_start + 4;
    let mut props = DffSoundProperties {
        sample_rate: None,
        channels: None,
        compression: None,
    };
    while offset
        .checked_add(DFF_HEADER_LEN as u64)
        .is_some_and(|end| end <= payload_end)
    {
        let mut header = [0u8; DFF_HEADER_LEN];
        read_at(file, offset, &mut header)?;
        let sub_len = u64::from_be_bytes(header[4..12].try_into().unwrap());
        let sub_start = offset + DFF_HEADER_LEN as u64;
        let sub_end = sub_start
            .checked_add(sub_len)
            .ok_or_else(|| invalid("DFF PROP subchunk offset overflow"))?;
        if sub_end > payload_end {
            return Err(invalid(format!(
                "DFF PROP subchunk {:?} extends past end of chunk",
                String::from_utf8_lossy(&header[0..4])
            )));
        }
        match &header[0..4] {
            b"FS  " if sub_len >= 4 => {
                let mut value = [0u8; 4];
                read_at(file, sub_start, &mut value)?;
                props.sample_rate = Some(u32::from_be_bytes(value));
            }
            b"CHNL" if sub_len >= 2 => {
                let mut value = [0u8; 2];
                read_at(file, sub_start, &mut value)?;
                props.channels = Some(u16::from_be_bytes(value));
            }
            b"CMPR" if sub_len >= 4 => {
                let mut value = [0u8; 4];
                read_at(file, sub_start, &mut value)?;
                props.compression = Some(value);
            }
            _ => {}
        }
        offset = sub_end
            .checked_add(sub_len & 1)
            .ok_or_else(|| invalid("DFF PROP padding offset overflow"))?;
    }
    Ok(props)
}

pub(super) fn parse_dff_file(file: &mut File) -> AudioDecoderResult<DffFileMetadata> {
    let file_len = file.metadata()?.len();
    let mut root = [0u8; 16];
    read_at(file, 0, &mut root)
        .map_err(|error| invalid(format!("Unable to read DFF root chunk: {error}")))?;
    if &root[0..4] != b"FRM8" || &root[12..16] != b"DSD " {
        return Err(invalid("DFF file must start with an FRM8 DSD form"));
    }
    let form_size = checked_chunk_size(u64::from_be_bytes(root[4..12].try_into().unwrap()))?;
    let form_end = 12u64
        .checked_add(form_size as u64)
        .ok_or_else(|| invalid("DFF form offset overflow"))?;
    if form_end > file_len {
        return Err(invalid(format!(
            "DFF form extends past end of file: declared {form_end} bytes, found {file_len}"
        )));
    }

    let mut offset = 16u64;
    let mut sample_rate = None;
    let mut channels = None;
    let mut compression = None;
    let mut data_chunk = None;
    while offset
        .checked_add(DFF_HEADER_LEN as u64)
        .is_some_and(|end| end <= form_end)
    {
        let mut header = [0u8; DFF_HEADER_LEN];
        read_at(file, offset, &mut header)?;
        let payload_size = u64::from_be_bytes(header[4..12].try_into().unwrap());
        let payload_start = offset + DFF_HEADER_LEN as u64;
        let payload_end = payload_start
            .checked_add(payload_size)
            .ok_or_else(|| invalid("DFF chunk offset overflow"))?;
        if payload_end > form_end {
            return Err(invalid(format!(
                "DFF chunk {:?} extends past end of form",
                String::from_utf8_lossy(&header[0..4])
            )));
        }
        match &header[0..4] {
            b"PROP" => {
                let props = parse_dff_properties(file, payload_start, payload_size)?;
                sample_rate = props.sample_rate.or(sample_rate);
                channels = props.channels.or(channels);
                compression = props.compression.or(compression);
            }
            b"DSD " => data_chunk = Some((payload_start, payload_size)),
            _ => {}
        }
        offset = payload_end
            .checked_add(payload_size & 1)
            .ok_or_else(|| invalid("DFF padding offset overflow"))?;
    }

    let sample_rate = sample_rate.ok_or_else(|| invalid("Missing DFF sample rate"))?;
    let channels = channels.ok_or_else(|| invalid("Missing DFF channels"))?;
    let compression = compression.ok_or_else(|| invalid("Missing DFF compression"))?;
    let (data_offset, data_len) =
        data_chunk.ok_or_else(|| invalid("Missing DFF DSD data chunk"))?;
    if compression != *b"DSD " {
        return Err(AudioDecoderError::UnsupportedFormat(format!(
            "Unsupported DFF compression {}",
            String::from_utf8_lossy(&compression)
        )));
    }
    validate_common("DFF", sample_rate, channels)?;
    let sample_count = data_len
        .checked_mul(8)
        .ok_or_else(|| invalid("DFF sample count overflows"))?
        / u64::from(channels);

    Ok(DffFileMetadata {
        sample_rate,
        channels,
        sample_count,
        data_offset,
        data_len,
    })
}
