use crate::decoder::error::{AudioDecoderError, AudioDecoderResult};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

/// Large enough to cover a conventional DSF block group for all channels,
/// while keeping decoder memory independent of the source file size.
pub(super) const FILE_CACHE_BYTES: usize = 1024 * 1024;

/// Bounded storage for a DSD data chunk.
///
/// Production decoders use the file-backed variant so opening a multi-gigabyte
/// DSF/DFF file does not allocate an equally large `Vec`. The memory variant is
/// retained for small parser fixtures and callers constructing a decoder from
/// bytes in unit tests.
pub(super) enum DsdDataSource {
    #[cfg(test)]
    Memory(Vec<u8>),
    File {
        file: File,
        data_offset: u64,
        data_len: u64,
        cache: Box<[u8]>,
        cache_start: u64,
        cache_len: usize,
    },
}

impl DsdDataSource {
    #[cfg(test)]
    pub(super) fn memory(data: Vec<u8>) -> Self {
        Self::Memory(data)
    }

    pub(super) fn file(file: File, data_offset: u64, data_len: u64) -> Self {
        Self::File {
            file,
            data_offset,
            data_len,
            cache: vec![0; FILE_CACHE_BYTES].into_boxed_slice(),
            cache_start: 0,
            cache_len: 0,
        }
    }

    pub(super) fn read_exact_at(
        &mut self,
        relative_offset: u64,
        dest: &mut [u8],
    ) -> AudioDecoderResult<()> {
        let read_len = u64::try_from(dest.len()).map_err(|_| {
            AudioDecoderError::DecodingFailed("DSD read length is too large".to_string())
        })?;
        match self {
            #[cfg(test)]
            Self::Memory(data) => {
                let start = usize::try_from(relative_offset).map_err(|_| {
                    AudioDecoderError::DecodingFailed(
                        "DSD data offset is too large to address".to_string(),
                    )
                })?;
                let end = start.checked_add(dest.len()).ok_or_else(|| {
                    AudioDecoderError::DecodingFailed("DSD data offset overflow".to_string())
                })?;
                let source = data.get(start..end).ok_or_else(|| {
                    AudioDecoderError::DecodingFailed(format!(
                        "DSD data is truncated at byte {relative_offset}"
                    ))
                })?;
                dest.copy_from_slice(source);
                Ok(())
            }
            Self::File {
                file,
                data_offset,
                data_len,
                cache,
                cache_start,
                cache_len,
            } => {
                let relative_end = relative_offset.checked_add(read_len).ok_or_else(|| {
                    AudioDecoderError::DecodingFailed("DSD data offset overflow".to_string())
                })?;
                if relative_end > *data_len {
                    return Err(AudioDecoderError::DecodingFailed(format!(
                        "DSD data is truncated at byte {relative_offset} (chunk length {data_len})"
                    )));
                }
                let cached_end = cache_start.saturating_add(*cache_len as u64);
                if relative_offset >= *cache_start && relative_end <= cached_end {
                    let cache_offset = usize::try_from(relative_offset - *cache_start)
                        .map_err(|_| {
                            AudioDecoderError::DecodingFailed(format!(
                                "DSD cached offset is too large to address at byte {relative_offset}"
                            ))
                        })?;
                    let cache_end = cache_offset.checked_add(dest.len()).ok_or_else(|| {
                        AudioDecoderError::DecodingFailed(format!(
                            "DSD cached read length overflows at byte {relative_offset}"
                        ))
                    })?;
                    let cached = cache.get(cache_offset..cache_end).ok_or_else(|| {
                        AudioDecoderError::DecodingFailed(format!(
                            "DSD cache is inconsistent at byte {relative_offset}"
                        ))
                    })?;
                    dest.copy_from_slice(cached);
                    return Ok(());
                }

                let absolute_offset =
                    data_offset.checked_add(relative_offset).ok_or_else(|| {
                        AudioDecoderError::DecodingFailed("DSD file offset overflow".to_string())
                    })?;
                file.seek(SeekFrom::Start(absolute_offset))
                    .map_err(|error| {
                        AudioDecoderError::IoError(format!(
                            "Failed to seek DSD data at byte {relative_offset}: {error}"
                        ))
                    })?;
                if dest.len() > cache.len() {
                    // Oversized read: larger than the whole cache, so bypass it
                    // and read straight into the destination. The cache is
                    // invalidated (a later small read simply refills it).
                    file.read_exact(dest).map_err(|error| {
                        AudioDecoderError::IoError(format!(
                            "Failed to read DSD data at byte {relative_offset}: {error}"
                        ))
                    })?;
                    *cache_start = relative_offset;
                    *cache_len = 0;
                    return Ok(());
                }
                let available =
                    usize::try_from((*data_len - relative_offset).min(cache.len() as u64))
                        .map_err(|_| {
                            AudioDecoderError::DecodingFailed(format!(
                                "DSD read at byte {relative_offset} exceeds addressable length"
                            ))
                        })?;
                file.read_exact(&mut cache[..available]).map_err(|error| {
                    AudioDecoderError::IoError(format!(
                        "Failed to read DSD data at byte {relative_offset}: {error}"
                    ))
                })?;
                *cache_start = relative_offset;
                *cache_len = available;
                let cached = cache.get(..dest.len()).ok_or_else(|| {
                    AudioDecoderError::DecodingFailed(format!(
                        "DSD cache refill is short at byte {relative_offset}"
                    ))
                })?;
                dest.copy_from_slice(cached);
                Ok(())
            }
        }
    }

    #[cfg(test)]
    pub(super) fn is_file_backed(&self) -> bool {
        matches!(self, Self::File { .. })
    }
}
