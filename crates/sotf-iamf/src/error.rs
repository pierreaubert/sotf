// ============================================================================
// IAMF Error Types
// ============================================================================

use thiserror::Error;

#[derive(Debug, Error)]
pub enum IamfError {
    /// Invalid or missing IA Sequence Header magic bytes
    #[error("Invalid IAMF magic bytes (expected 'iamf')")]
    InvalidMagic,
    /// Unsupported IAMF profile
    #[error("Unsupported IAMF profile: {0}")]
    UnsupportedProfile(u8),
    /// Unknown or invalid OBU type
    #[error("Invalid OBU type: {0}")]
    InvalidObuType(u8),
    /// OBU payload size exceeds remaining data
    #[error("Truncated OBU: expected {expected} bytes, {available} available")]
    TruncatedObu { expected: usize, available: usize },
    /// Referenced codec config ID not found
    #[error("Unknown codec config ID: {0}")]
    UnknownCodecConfig(u32),
    /// Referenced audio element ID not found
    #[error("Unknown audio element ID: {0}")]
    UnknownAudioElement(u32),
    /// Referenced mix presentation ID not found
    #[error("Unknown mix presentation ID: {0}")]
    UnknownMixPresentation(u32),
    /// Unsupported codec
    #[error("Unsupported codec: {0}")]
    UnsupportedCodec(String),
    /// Unsupported audio element type
    #[error("Unsupported audio element type: {0}")]
    UnsupportedElementType(u8),
    /// No mix presentations found
    #[error("No mix presentations in IAMF stream")]
    NoMixPresentations,
    /// Codec decoding error
    #[error("Codec error: {0}")]
    CodecError(String),
    /// Invalid parameter value
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
    /// I/O error during reading
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    /// Seek failed
    #[error("Seek error: {0}")]
    SeekError(String),
    /// End of stream
    #[error("End of IAMF stream")]
    EndOfStream,
    /// Bitstream parsing error
    #[error("Parse error: {0}")]
    ParseError(String),
}

pub type IamfResult<T> = Result<T, IamfError>;
