use std::fmt;
use symphonia_core::errors::Error as SymphoniaError;

/// Audio decoder error types
#[derive(Debug, Clone)]
pub enum AudioDecoderError {
    /// File not found or inaccessible
    FileNotFound(String),
    /// Unsupported audio format
    UnsupportedFormat(String),
    /// File is corrupted or invalid
    InvalidFile(String),
    /// Decoding failed during playback
    DecodingFailed(String),
    /// Stream ended unexpectedly
    StreamEnded,
    /// I/O error during file operations
    IoError(String),
    /// Configuration error
    ConfigError(String),
    /// Seek operation failed
    SeekFailed(String),
}

impl fmt::Display for AudioDecoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioDecoderError::FileNotFound(path) => {
                write!(f, "Audio file not found: {}", path)
            }
            AudioDecoderError::UnsupportedFormat(format) => {
                write!(f, "Unsupported audio format: {}", format)
            }
            AudioDecoderError::InvalidFile(reason) => {
                write!(f, "Invalid audio file: {}", reason)
            }
            AudioDecoderError::DecodingFailed(reason) => {
                write!(f, "Audio decoding failed: {}", reason)
            }
            AudioDecoderError::StreamEnded => {
                write!(f, "Audio stream ended unexpectedly")
            }
            AudioDecoderError::IoError(reason) => {
                write!(f, "I/O error: {}", reason)
            }
            AudioDecoderError::ConfigError(reason) => {
                write!(f, "Configuration error: {}", reason)
            }
            AudioDecoderError::SeekFailed(reason) => {
                write!(f, "Seek operation failed: {}", reason)
            }
        }
    }
}

impl std::error::Error for AudioDecoderError {}

impl From<std::io::Error> for AudioDecoderError {
    fn from(err: std::io::Error) -> Self {
        AudioDecoderError::IoError(err.to_string())
    }
}

impl From<SymphoniaError> for AudioDecoderError {
    fn from(err: SymphoniaError) -> Self {
        match err {
            SymphoniaError::IoError(io_err) => AudioDecoderError::IoError(io_err.to_string()),
            SymphoniaError::DecodeError(_) => {
                AudioDecoderError::DecodingFailed("Symphonia decode error".to_string())
            }
            SymphoniaError::Unsupported(_) => {
                AudioDecoderError::UnsupportedFormat("Unsupported by Symphonia".to_string())
            }
            SymphoniaError::SeekError(_) => {
                AudioDecoderError::SeekFailed("Symphonia seek error".to_string())
            }
            _ => AudioDecoderError::DecodingFailed(format!("Symphonia error: {:?}", err)),
        }
    }
}

/// Result type for audio decoder operations
pub type AudioDecoderResult<T> = Result<T, AudioDecoderError>;

/// Helper function to create user-friendly error messages for the UI
pub fn user_friendly_error(error: &AudioDecoderError) -> String {
    match error {
        AudioDecoderError::FileNotFound(_) => {
            "The selected audio file could not be found. Please check if the file still exists.".to_string()
        }
        AudioDecoderError::UnsupportedFormat(_) => {
            format!(
                "This audio format is not supported. Currently supported formats: {}",
                crate::decoder::formats::AudioFormat::supported_formats_string()
            )
        }
        AudioDecoderError::InvalidFile(_) => {
            "The audio file appears to be corrupted or invalid. Please try a different file.".to_string()
        }
        AudioDecoderError::DecodingFailed(_) => {
            "Failed to decode the audio file. The file might be corrupted or use an unsupported codec variant.".to_string()
        }
        AudioDecoderError::StreamEnded => {
            "Playback ended unexpectedly. The audio stream was interrupted.".to_string()
        }
        AudioDecoderError::IoError(_) => {
            "Failed to read the audio file. Please check file permissions and disk space.".to_string()
        }
        AudioDecoderError::ConfigError(_) => {
            "Audio configuration error. Please check your audio settings.".to_string()
        }
        AudioDecoderError::SeekFailed(_) => {
            "Failed to seek to the requested position in the audio file.".to_string()
        }
    }
}
