use crate::icy::IcyMetadata;

/// Stream metadata updates delivered from the HTTP source.
#[derive(Debug, Clone)]
pub enum StreamMetadata {
    /// ICY metadata update (title/url change).
    Icy(IcyMetadata),
    /// Content type detected from HTTP headers.
    ContentType(String),
    /// Bitrate detected from ICY headers (kbps).
    Bitrate(u32),
}
