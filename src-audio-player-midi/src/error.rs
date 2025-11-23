//! Error types for MIDI operations

use thiserror::Error;

/// Result type alias for MIDI operations
pub type Result<T> = std::result::Result<T, MidiError>;

/// Errors that can occur during MIDI operations
#[derive(Error, Debug)]
pub enum MidiError {
    /// Error initializing MIDI subsystem
    #[error("Failed to initialize MIDI: {0}")]
    InitError(String),

    /// Error connecting to a MIDI device
    #[error("Failed to connect to MIDI device: {0}")]
    ConnectionError(String),

    /// Error sending MIDI message
    #[error("Failed to send MIDI message: {0}")]
    SendError(String),

    /// Invalid device index
    #[error("Invalid device index: {0}")]
    InvalidDevice(usize),

    /// Device not connected
    #[error("Device not connected")]
    NotConnected,

    /// Invalid MIDI message
    #[error("Invalid MIDI message: {0}")]
    InvalidMessage(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl From<midir::InitError> for MidiError {
    fn from(err: midir::InitError) -> Self {
        MidiError::InitError(err.to_string())
    }
}

impl From<midir::ConnectError<midir::MidiInput>> for MidiError {
    fn from(err: midir::ConnectError<midir::MidiInput>) -> Self {
        MidiError::ConnectionError(err.to_string())
    }
}

impl From<midir::ConnectError<midir::MidiOutput>> for MidiError {
    fn from(err: midir::ConnectError<midir::MidiOutput>) -> Self {
        MidiError::ConnectionError(err.to_string())
    }
}

// Note: midir::SendError is generic over the message type
// We can't implement From directly due to type parameter constraints
// Use MidiError::SendError(err.to_string()) at call sites instead
