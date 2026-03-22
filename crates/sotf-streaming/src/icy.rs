/// Parsed ICY (Icecast/SHOUTcast) metadata from an internet radio stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IcyMetadata {
    /// Current track title (e.g. "Artist - Song Title")
    pub stream_title: Option<String>,
    /// Stream URL if provided
    pub stream_url: Option<String>,
}

impl IcyMetadata {
    /// Parse ICY metadata from the raw metadata block.
    ///
    /// ICY metadata format: `StreamTitle='Artist - Title';StreamUrl='http://...';`
    pub fn parse(raw: &[u8]) -> Self {
        let text = String::from_utf8_lossy(raw);
        let text = text.trim_end_matches('\0');

        let stream_title = Self::extract_field(text, "StreamTitle");
        let stream_url = Self::extract_field(text, "StreamUrl");

        IcyMetadata {
            stream_title,
            stream_url,
        }
    }

    fn extract_field(text: &str, field: &str) -> Option<String> {
        let prefix = format!("{}='", field);
        let start = text.find(&prefix)?;
        let value_start = start + prefix.len();
        let rest = &text[value_start..];
        let end = rest.find("';")?;
        let value = rest[..end].to_string();
        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_typical_metadata() {
        let raw = b"StreamTitle='Pink Floyd - Comfortably Numb';StreamUrl='http://radio.example.com';";
        let meta = IcyMetadata::parse(raw);
        assert_eq!(
            meta.stream_title.as_deref(),
            Some("Pink Floyd - Comfortably Numb")
        );
        assert_eq!(
            meta.stream_url.as_deref(),
            Some("http://radio.example.com")
        );
    }

    #[test]
    fn test_parse_title_only() {
        let raw = b"StreamTitle='Jazz FM';";
        let meta = IcyMetadata::parse(raw);
        assert_eq!(meta.stream_title.as_deref(), Some("Jazz FM"));
        assert_eq!(meta.stream_url, None);
    }

    #[test]
    fn test_parse_empty_metadata() {
        let raw = b"StreamTitle='';";
        let meta = IcyMetadata::parse(raw);
        assert_eq!(meta.stream_title, None);
        assert_eq!(meta.stream_url, None);
    }

    #[test]
    fn test_parse_null_padded() {
        let mut raw = b"StreamTitle='Test';".to_vec();
        raw.extend_from_slice(&[0u8; 100]);
        let meta = IcyMetadata::parse(&raw);
        assert_eq!(meta.stream_title.as_deref(), Some("Test"));
    }

    #[test]
    fn test_parse_garbage() {
        let raw = b"this is not icy metadata";
        let meta = IcyMetadata::parse(raw);
        assert_eq!(meta.stream_title, None);
        assert_eq!(meta.stream_url, None);
    }
}
