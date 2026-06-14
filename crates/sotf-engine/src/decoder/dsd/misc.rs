use crate::decoder::error::{AudioDecoderError, AudioDecoderResult};

pub(super) fn checked_chunk_size(size: u64) -> AudioDecoderResult<usize> {
    usize::try_from(size).map_err(|_| {
        AudioDecoderError::InvalidFile(format!("DSF chunk size {} is too large", size))
    })
}
