use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

pub(super) const CD_SERVICE: &str = "urn:schemas-upnp-org:service:ContentDirectory:1";

pub(super) const CM_SERVICE: &str = "urn:schemas-upnp-org:service:ConnectionManager:1";

pub(super) async fn stream_file_range(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    path: &std::path::Path,
    start: u64,
    len: u64,
) -> Result<(), String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| format!("open media file: {e}"))?;
    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(|e| format!("seek media file: {e}"))?;

    let mut remaining = len;
    let mut buf = vec![0u8; 64 * 1024];
    while remaining > 0 {
        let to_read = remaining.min(buf.len() as u64) as usize;
        let n = file
            .read(&mut buf[..to_read])
            .await
            .map_err(|e| format!("read media file: {e}"))?;
        if n == 0 {
            break;
        }
        writer
            .write_all(&buf[..n])
            .await
            .map_err(|e| e.to_string())?;
        remaining -= n as u64;
    }
    Ok(())
}

pub(super) fn parse_range_header(
    range_header: Option<&str>,
    file_len: u64,
) -> Result<Option<(u64, u64)>, ()> {
    let Some(raw) = range_header else {
        return Ok(None);
    };
    let Some(spec) = raw.trim().strip_prefix("bytes=") else {
        return Err(());
    };
    if spec.contains(',') || file_len == 0 {
        return Err(());
    }
    let Some((start_raw, end_raw)) = spec.split_once('-') else {
        return Err(());
    };

    let (start, end) = if start_raw.is_empty() {
        let suffix_len = end_raw.parse::<u64>().map_err(|_| ())?;
        if suffix_len == 0 {
            return Err(());
        }
        let start = file_len.saturating_sub(suffix_len);
        (start, file_len - 1)
    } else {
        let start = start_raw.parse::<u64>().map_err(|_| ())?;
        if start >= file_len {
            return Err(());
        }
        let end = if end_raw.is_empty() {
            file_len - 1
        } else {
            end_raw.parse::<u64>().map_err(|_| ())?.min(file_len - 1)
        };
        if end < start {
            return Err(());
        }
        (start, end)
    };

    Ok(Some((start, end)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_parse_range_header_none() {
        assert_eq!(parse_range_header(None, 100), Ok(None));
    }

    #[test]
    fn test_parse_range_header_full_range() {
        assert_eq!(parse_range_header(Some("bytes=0-99"), 100), Ok(Some((0, 99))));
    }

    #[test]
    fn test_parse_range_header_open_ended() {
        assert_eq!(parse_range_header(Some("bytes=10-"), 100), Ok(Some((10, 99))));
    }

    #[test]
    fn test_parse_range_header_suffix() {
        assert_eq!(parse_range_header(Some("bytes=-10"), 100), Ok(Some((90, 99))));
    }

    #[test]
    fn test_parse_range_header_clamps_end() {
        assert_eq!(parse_range_header(Some("bytes=0-200"), 100), Ok(Some((0, 99))));
    }

    #[test]
    fn test_parse_range_header_zero_suffix_is_error() {
        assert_eq!(parse_range_header(Some("bytes=-0"), 100), Err(()));
    }

    #[test]
    fn test_parse_range_header_start_beyond_file() {
        assert_eq!(parse_range_header(Some("bytes=100-"), 100), Err(()));
    }

    #[test]
    fn test_parse_range_header_end_before_start() {
        assert_eq!(parse_range_header(Some("bytes=50-10"), 100), Err(()));
    }

    #[test]
    fn test_parse_range_header_missing_bytes_prefix() {
        assert_eq!(parse_range_header(Some("0-10"), 100), Err(()));
    }

    #[test]
    fn test_parse_range_header_multiple_ranges() {
        assert_eq!(parse_range_header(Some("bytes=0-10,20-30"), 100), Err(()));
    }

    #[test]
    fn test_parse_range_header_empty_file() {
        assert_eq!(parse_range_header(Some("bytes=0-"), 0), Err(()));
    }

    proptest! {
        /// INVARIANT: a single closed byte range is parsed consistently with the
        /// HTTP Range spec: bounds are inclusive and clamped to the file length.
        #[test]
        fn parse_range_header_closed_range(
            start in 0u64..1_000_000u64,
            end in 0u64..1_000_000u64,
            file_len in 1u64..1_000_000u64,
        ) {
            let header = format!("bytes={}-{}", start, end);
            let result = parse_range_header(Some(&header), file_len);
            if end < start {
                prop_assert_eq!(result, Err(()));
            } else if start >= file_len {
                prop_assert_eq!(result, Err(()));
            } else {
                prop_assert_eq!(result, Ok(Some((start, end.min(file_len - 1)))));
            }
        }

        /// INVARIANT: an open-ended byte range clamps the end to file_len - 1.
        #[test]
        fn parse_range_header_open_ended(
            start in 0u64..1_000_000u64,
            file_len in 1u64..1_000_000u64,
        ) {
            let header = format!("bytes={}-", start);
            let result = parse_range_header(Some(&header), file_len);
            if start >= file_len {
                prop_assert_eq!(result, Err(()));
            } else {
                prop_assert_eq!(result, Ok(Some((start, file_len - 1))));
            }
        }

        /// INVARIANT: a suffix range returns the last N bytes when N > 0.
        #[test]
        fn parse_range_header_suffix(
            suffix_len in 1u64..1_000_000u64,
            file_len in 1u64..1_000_000u64,
        ) {
            let header = format!("bytes=-{}", suffix_len);
            let result = parse_range_header(Some(&header), file_len);
            let start = file_len.saturating_sub(suffix_len);
            prop_assert_eq!(result, Ok(Some((start, file_len - 1))));
        }

        /// INVARIANT: multi-range and zero-suffix headers are rejected.
        #[test]
        fn parse_range_header_rejects_multi_and_zero_suffix(
            a in 0u64..100u64,
            b in 0u64..100u64,
            file_len in 1u64..1_000_000u64,
        ) {
            let multi = format!("bytes={}-{},{}-{}", a, b, b + 1, b + 2);
            prop_assert_eq!(parse_range_header(Some(&multi), file_len), Err(()));
            prop_assert_eq!(parse_range_header(Some("bytes=-0"), file_len), Err(()));
        }
    }
}
