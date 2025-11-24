use std::fmt;

/// Errors that can occur during binaural decoder operation
#[derive(Debug, Clone)]
pub enum BinauralError {
    /// SOFA file not loaded when required
    SofaNotLoaded,
    /// Sample rate mismatch between SOFA and engine
    SampleRateMismatch { sofa_rate: u32, engine_rate: u32 },
    /// Invalid FFT size (must be power of 2)
    InvalidFftSize(usize),
    /// SOFA file loading failed
    SofaLoadError(String),
    /// Resampling failed
    ResamplingError(String),
    /// HRTF preparation failed
    HrtfPreparationError(String),
    /// Invalid parameter value
    InvalidParameter { name: String, value: String },
    /// Input/output buffer size mismatch
    BufferSizeMismatch { expected: usize, got: usize },
}

impl fmt::Display for BinauralError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinauralError::SofaNotLoaded => write!(f, "SOFA file not loaded"),
            BinauralError::SampleRateMismatch {
                sofa_rate,
                engine_rate,
            } => {
                write!(
                    f,
                    "Sample rate mismatch: SOFA={}Hz, engine={}Hz",
                    sofa_rate, engine_rate
                )
            }
            BinauralError::InvalidFftSize(size) => {
                write!(f, "Invalid FFT size: {} (must be power of 2)", size)
            }
            BinauralError::SofaLoadError(msg) => write!(f, "SOFA load error: {}", msg),
            BinauralError::ResamplingError(msg) => write!(f, "Resampling error: {}", msg),
            BinauralError::HrtfPreparationError(msg) => {
                write!(f, "HRTF preparation error: {}", msg)
            }
            BinauralError::InvalidParameter { name, value } => {
                write!(f, "Invalid parameter '{}': {}", name, value)
            }
            BinauralError::BufferSizeMismatch { expected, got } => {
                write!(
                    f,
                    "Buffer size mismatch: expected {}, got {}",
                    expected, got
                )
            }
        }
    }
}

impl std::error::Error for BinauralError {}

pub type BinauralResult<T> = Result<T, BinauralError>;
