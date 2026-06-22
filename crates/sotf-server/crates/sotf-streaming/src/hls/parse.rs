use super::consts::DEFAULT_TARGET_DURATION;
use super::hls_segment::HlsSegment;
use super::resolve::resolve_byte_range;
use super::types::PendingByteRange;
use super::types::ResolvedPlaylist;
use std::collections::HashSet;
use std::io::{self};
use std::time::Duration;
use url::Url;

pub(super) fn parse_master_playlist(base_url: &Url, playlist: &str) -> io::Result<Option<Url>> {
    if let Ok(m3u8_rs::Playlist::MasterPlaylist(master)) =
        m3u8_rs::parse_playlist_res(playlist.as_bytes())
    {
        return master
            .variants
            .iter()
            .filter(|variant| !variant.is_i_frame)
            .max_by_key(|variant| variant.bandwidth)
            .map(|variant| {
                base_url
                    .join(&variant.uri)
                    .map_err(|e| io::Error::other(e.to_string()))
            })
            .transpose();
    }

    let mut best: Option<(u64, Url)> = None;
    let mut pending_bandwidth: Option<u64> = None;

    for raw_line in playlist.lines() {
        let line = raw_line.trim();
        if line.starts_with("#EXT-X-STREAM-INF:") {
            pending_bandwidth = parse_attribute(line, "BANDWIDTH")
                .and_then(|value| value.parse::<u64>().ok())
                .or(Some(0));
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(bandwidth) = pending_bandwidth.take() {
            let url = base_url
                .join(line)
                .map_err(|e| io::Error::other(e.to_string()))?;
            if best
                .as_ref()
                .is_none_or(|(best_bw, _)| bandwidth > *best_bw)
            {
                best = Some((bandwidth, url));
            }
        }
    }

    Ok(best.map(|(_, url)| url))
}

pub(super) fn parse_media_playlist(base_url: &Url, playlist: &str) -> io::Result<ResolvedPlaylist> {
    let mut segments = Vec::new();
    let mut end_list = false;
    let mut target_duration = DEFAULT_TARGET_DURATION;
    let mut current_map: Option<HlsSegment> = None;
    let mut emitted_maps = HashSet::new();
    let mut pending_byte_range: Option<PendingByteRange> = None;
    let mut last_byte_range_end: Option<u64> = None;
    let mut last_map_byte_range_end: Option<u64> = None;
    let mut encrypted_segments = false;

    for raw_line in playlist.lines() {
        let line = raw_line.trim();
        if let Some(value) = line.strip_prefix("#EXT-X-TARGETDURATION:") {
            if let Ok(seconds) = value.trim().parse::<u64>() {
                target_duration = Duration::from_secs(seconds.max(1));
            }
            continue;
        }
        if line == "#EXT-X-ENDLIST" {
            end_list = true;
            continue;
        }
        if line.starts_with("#EXT-X-KEY:") {
            encrypted_segments = parse_attribute(line, "METHOD")
                .is_some_and(|method| !method.eq_ignore_ascii_case("NONE"));
            continue;
        }
        if let Some(value) = line.strip_prefix("#EXT-X-BYTERANGE:") {
            pending_byte_range = Some(parse_byte_range(value)?);
            continue;
        }
        if line.starts_with("#EXT-X-MAP:") {
            let uri = parse_attribute(line, "URI").ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "HLS EXT-X-MAP missing URI")
            })?;
            let byte_range = parse_attribute(line, "BYTERANGE")
                .map(parse_byte_range)
                .transpose()?
                .map(|range| resolve_byte_range(range, &mut last_map_byte_range_end))
                .transpose()?;
            let url = base_url
                .join(uri)
                .map_err(|e| io::Error::other(e.to_string()))?;
            current_map = Some(HlsSegment::new(url, byte_range));
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if encrypted_segments {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "HLS encrypted media segments are not supported",
            ));
        }

        if let Some(map) = current_map.clone()
            && emitted_maps.insert(map.key())
        {
            segments.push(map);
        }

        let byte_range = match pending_byte_range.take() {
            Some(range) => Some(resolve_byte_range(range, &mut last_byte_range_end)?),
            None => {
                last_byte_range_end = None;
                None
            }
        };
        let url = base_url
            .join(line)
            .map_err(|e| io::Error::other(e.to_string()))?;
        segments.push(HlsSegment::new(url, byte_range));
    }

    if segments.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HLS media playlist contains no segments",
        ));
    }

    Ok(ResolvedPlaylist {
        playlist_url: base_url.clone(),
        segments,
        end_list,
        target_duration,
    })
}

pub(super) fn parse_attribute<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let attrs = line.split_once(':')?.1;
    let mut start = 0;
    let mut in_quotes = false;

    for (idx, ch) in attrs.char_indices() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                if let Some(value) = parse_attribute_pair(&attrs[start..idx], name) {
                    return Some(value);
                }
                start = idx + 1;
            }
            _ => {}
        }
    }

    parse_attribute_pair(&attrs[start..], name)
}

pub(super) fn parse_attribute_pair<'a>(attr: &'a str, name: &str) -> Option<&'a str> {
    let (key, value) = attr.split_once('=')?;
    if key.trim() == name {
        Some(value.trim().trim_matches('"'))
    } else {
        None
    }
}

/// Format a single HLS attribute pair for round-trip testing.
///
/// The output is a full tag line (`#EXT-X-FOO:KEY="VALUE"`) so that
/// `parse_attribute(line, key)` returns the original `value` for values that
/// do not contain commas, quotes, or leading/trailing whitespace.
#[cfg(test)]
pub(super) fn format_attribute(tag: &str, name: &str, value: &str) -> String {
    format!("#EXT-X-{}:{}=\"{}\"", tag, name, value)
}

pub(super) fn parse_byte_range(value: &str) -> io::Result<PendingByteRange> {
    let value = value.trim().trim_matches('"');
    let (length, offset) = match value.split_once('@') {
        Some((length, offset)) => (length, Some(offset)),
        None => (value, None),
    };
    let length = length.trim().parse::<u64>().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid HLS byte range length: {}", e),
        )
    })?;
    if length == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HLS byte range length must be greater than zero",
        ));
    }
    let offset = offset
        .map(|offset| {
            offset.trim().parse::<u64>().map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid HLS byte range offset: {}", e),
                )
            })
        })
        .transpose()?;

    Ok(PendingByteRange { length, offset })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_attribute_finds_quoted_and_unquoted_values() {
        assert_eq!(
            parse_attribute(
                r#"#EXT-X-STREAM-INF:BANDWIDTH=128000,CODECS="mp4a.40.2""#,
                "BANDWIDTH"
            ),
            Some("128000")
        );
        assert_eq!(
            parse_attribute(
                r#"#EXT-X-STREAM-INF:BANDWIDTH=128000,CODECS="mp4a.40.2""#,
                "CODECS"
            ),
            Some("mp4a.40.2")
        );
        assert_eq!(
            parse_attribute(r#"#EXT-X-KEY:METHOD=AES-128,URI="key.bin""#, "METHOD"),
            Some("AES-128")
        );
        assert_eq!(parse_attribute("#EXT-X-FOO:BAR=1", "MISSING"), None);
    }

    #[test]
    fn parse_attribute_handles_commas_inside_quotes() {
        let line = r#"#EXT-X-STREAM-INF:BANDWIDTH=128000,NAME="A,B,C",CODECS="mp4a.40.2""#;
        assert_eq!(parse_attribute(line, "NAME"), Some("A,B,C"));
        assert_eq!(parse_attribute(line, "CODECS"), Some("mp4a.40.2"));
    }

    #[test]
    fn parse_attribute_pair_trims_quotes() {
        assert_eq!(
            parse_attribute_pair("NAME=\"value\"", "NAME"),
            Some("value")
        );
        assert_eq!(parse_attribute_pair("NAME=value", "NAME"), Some("value"));
        assert_eq!(parse_attribute_pair("OTHER=1", "NAME"), None);
    }

    #[test]
    fn parse_byte_range_length_only() {
        let range = parse_byte_range("1024").unwrap();
        assert_eq!(range.length, 1024);
        assert_eq!(range.offset, None);
    }

    #[test]
    fn parse_byte_range_length_and_offset() {
        let range = parse_byte_range("1024@2048").unwrap();
        assert_eq!(range.length, 1024);
        assert_eq!(range.offset, Some(2048));
    }

    #[test]
    fn parse_byte_range_rejects_zero_length() {
        assert!(parse_byte_range("0").is_err());
    }

    #[test]
    fn parse_byte_range_rejects_invalid_numbers() {
        assert!(parse_byte_range("abc").is_err());
        assert!(parse_byte_range("1024@abc").is_err());
    }

    #[test]
    fn parse_master_playlist_selects_highest_bandwidth() {
        let base = Url::parse("http://example.com/").unwrap();
        let playlist = "#EXTM3U\n\
            #EXT-X-STREAM-INF:BANDWIDTH=128000\n\
            low.m3u8\n\
            #EXT-X-STREAM-INF:BANDWIDTH=256000\n\
            high.m3u8\n";
        let selected = parse_master_playlist(&base, playlist).unwrap().unwrap();
        assert_eq!(selected.as_str(), "http://example.com/high.m3u8");
    }

    #[test]
    fn parse_master_playlist_returns_none_for_empty() {
        let base = Url::parse("http://example.com/").unwrap();
        assert!(parse_master_playlist(&base, "#EXTM3U\n").unwrap().is_none());
    }

    #[test]
    fn parse_media_playlist_extracts_segments() {
        let base = Url::parse("http://example.com/").unwrap();
        let playlist = "#EXTM3U\n\
            #EXT-X-TARGETDURATION:10\n\
            #EXTINF:9.009,\n\
            segment0.ts\n\
            #EXTINF:9.009,\n\
            segment1.ts\n\
            #EXT-X-ENDLIST\n";
        let resolved = parse_media_playlist(&base, playlist).unwrap();
        assert_eq!(resolved.segments.len(), 2);
        assert!(resolved.end_list);
        assert_eq!(resolved.target_duration, Duration::from_secs(10));
        assert_eq!(
            resolved.segments[0].url.as_str(),
            "http://example.com/segment0.ts"
        );
    }

    #[test]
    fn parse_media_playlist_rejects_encrypted() {
        let base = Url::parse("http://example.com/").unwrap();
        let playlist = "#EXTM3U\n\
            #EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\"\n\
            segment.ts\n";
        let err = parse_media_playlist(&base, playlist).unwrap_err();
        assert!(err.to_string().contains("encrypted"));
    }

    #[test]
    fn parse_media_playlist_rejects_empty() {
        let base = Url::parse("http://example.com/").unwrap();
        assert!(parse_media_playlist(&base, "#EXTM3U\n").is_err());
    }

    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        /// Values that are safe to wrap in double quotes for a round-trip.
        fn safe_value_strategy() -> BoxedStrategy<String> {
            proptest::string::string_regex("[a-zA-Z0-9_.:/-]+")
                .unwrap()
                .boxed()
        }

        fn attribute_name_strategy() -> BoxedStrategy<String> {
            proptest::string::string_regex("[A-Z]+").unwrap().boxed()
        }

        fn tag_name_strategy() -> BoxedStrategy<String> {
            proptest::string::string_regex("[A-Z]+").unwrap().boxed()
        }

        proptest! {
            #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

            /// INVARIANT: arbitrary key/value pairs that do not need escaping round-trip
            /// through `format_attribute` and `parse_attribute`.
            #[test]
            fn format_parse_attribute_roundtrip(
                tag in tag_name_strategy(),
                name in attribute_name_strategy(),
                value in safe_value_strategy(),
            ) {
                let line = format_attribute(&tag, &name, &value);
                let parsed = parse_attribute(&line, &name);
                prop_assert_eq!(
                    parsed,
                    Some(value.as_str()),
                    "round-trip failed for line {}",
                    line
                );
            }

            /// INVARIANT: `parse_attribute` never panics on arbitrary HLS-like lines.
            #[test]
            fn parse_attribute_never_panics(
                prefix in "#[A-Za-z0-9-]*",
                body in "[a-zA-Z0-9_,=\\\"\\n\\r\\t [:space:]]{0,128}",
            ) {
                let line = format!("{}:{}", prefix, body);
                let _ = parse_attribute(&line, "KEY");
            }

            /// INVARIANT: `parse_byte_range` never panics on arbitrary input.
            #[test]
            fn parse_byte_range_never_panics(
                value in "[a-zA-Z0-9_@,=\\\"\\n\\r\\t [:space:]]{0,64}",
            ) {
                let _ = parse_byte_range(&value);
            }
        }
    }
}
