// ============================================================================
// Plugin Error Types
// ============================================================================

use thiserror::Error;

/// Comprehensive error type for plugin operations.
///
/// This enum provides standardized error handling across all plugins,
/// replacing the previous `String`-based error messages.
///
/// # Example
/// ```rust,ignore
/// use sotf_plugins::PluginError;
///
/// fn load_plugin(path: &str) -> Result<Box<dyn Plugin>, PluginError> {
///     // ...
/// }
/// ```
#[derive(Debug, Error)]
pub enum PluginError {
    #[error("Invalid sample rate: {0} Hz (valid range: {1}-{2} Hz)")]
    InvalidSampleRate(u32, u32, u32),

    #[error("Channel configuration not supported: {inputs} inputs → {outputs} outputs")]
    UnsupportedChannelConfig { inputs: usize, outputs: usize },

    #[error("FFT size {0} is not a power of 2")]
    InvalidFftSize(usize),

    #[error("FFT size {0} exceeds maximum allowed size {1}")]
    FftSizeTooLarge(usize, usize),

    #[error("HRTF file not found: {0}")]
    HrtfFileNotFound(String),

    #[error("Failed to parse HRTF file: {0}")]
    HrtfParseError(String),

    #[error("Invalid HRTF data: {0}")]
    HrtfInvalidData(String),

    #[error("Impulse response file not found: {0}")]
    IrFileNotFound(String),

    #[error("Failed to load impulse response: {0}")]
    IrLoadError(String),

    #[error("Impulse response sample rate mismatch: expected {expected}, got {actual}")]
    IrSampleRateMismatch { expected: u32, actual: u32 },

    #[error("Parameter '{0}' not found")]
    ParameterNotFound(String),

    #[error("Parameter value out of range: '{0}' = {1} (valid range: {2}-{3})")]
    ParameterOutOfRange(String, f32, f32, f32),

    #[error("Plugin is not initialized")]
    NotInitialized,

    #[error("Plugin initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Audio processing failed: {0}")]
    ProcessingError(String),

    #[error("Plugin locked by another thread")]
    LockPoisoned,

    #[error("Plugin not found: {0}")]
    PluginNotFound(String),

    #[error("Invalid plugin configuration: {0}")]
    InvalidConfiguration(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Audio device error: {0}")]
    DeviceError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Result type for plugin operations using PluginError.
pub type PluginResult<T> = Result<T, PluginError>;

impl From<String> for PluginError {
    fn from(s: String) -> Self {
        PluginError::Unknown(s)
    }
}

impl From<&str> for PluginError {
    fn from(s: &str) -> Self {
        PluginError::Unknown(s.to_string())
    }
}

impl<T> From<std::sync::PoisonError<T>> for PluginError {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        PluginError::LockPoisoned
    }
}
