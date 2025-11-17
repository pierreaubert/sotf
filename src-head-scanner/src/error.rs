//! Error types for the head scanner

use thiserror::Error;

/// Result type for scanner operations
pub type ScannerResult<T> = Result<T, ScannerError>;

/// Errors that can occur during head scanning
#[derive(Debug, Error)]
pub enum ScannerError {
    /// Camera-related errors
    #[error("Camera error: {0}")]
    Camera(String),

    /// Camera not initialized
    #[error("Camera not initialized")]
    CameraNotInitialized,

    /// Vision model errors
    #[error("Vision model error: {0}")]
    VisionModel(String),

    /// Point cloud processing errors
    #[error("Point cloud error: {0}")]
    PointCloud(String),

    /// Mesh generation errors
    #[error("Mesh generation error: {0}")]
    MeshGeneration(String),

    /// Convex hull computation errors
    #[error("Convex hull error: {0}")]
    ConvexHull(String),

    /// File I/O errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// OpenCV errors
    #[error("OpenCV error: {0}")]
    OpenCV(String),

    /// ONNX Runtime errors
    #[error("ONNX Runtime error: {0}")]
    OnnxRuntime(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Insufficient data
    #[error("Insufficient data: {0}")]
    InsufficientData(String),

    /// Invalid input
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Feature not implemented
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// IO error with custom message
    #[error("IO error: {0}")]
    IoError(String),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl From<opencv::Error> for ScannerError {
    fn from(err: opencv::Error) -> Self {
        ScannerError::OpenCV(err.to_string())
    }
}

impl From<ort::Error> for ScannerError {
    fn from(err: ort::Error) -> Self {
        ScannerError::OnnxRuntime(err.to_string())
    }
}
