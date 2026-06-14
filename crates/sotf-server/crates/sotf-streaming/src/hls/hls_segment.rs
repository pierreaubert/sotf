use super::hls_byte_range::HlsByteRange;
use url::Url;

#[derive(Clone, Debug)]
pub(super) struct HlsSegment {
    pub(super) url: Url,
    pub(super) byte_range: Option<HlsByteRange>,
}

impl HlsSegment {
    pub(super) fn new(url: Url, byte_range: Option<HlsByteRange>) -> Self {
        Self { url, byte_range }
    }

    pub(super) fn key(&self) -> String {
        match self.byte_range {
            Some(range) => format!("{}#{}+{}", self.url, range.offset, range.length),
            None => self.url.as_str().to_string(),
        }
    }
}
