use std::io;

#[derive(Debug, thiserror::Error)]
pub enum SofaError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Not a valid HDF5 file (bad magic signature)")]
    NotHdf5,

    #[error("Unsupported HDF5 superblock version: {0}")]
    UnsupportedSuperblock(u8),

    #[error("Unsupported object header version: {0}")]
    UnsupportedObjectHeader(u8),

    #[error("Missing required SOFA attribute: {0}")]
    MissingAttribute(String),

    #[error("Missing required SOFA dimension: {0}")]
    MissingDimension(String),

    #[error("Missing required SOFA variable: {0}")]
    MissingVariable(String),

    #[error("Data type mismatch: expected {expected}, got {got}")]
    TypeMismatch { expected: String, got: String },

    #[error("Invalid HDF5 structure: {0}")]
    InvalidStructure(String),

    #[error("Truncated data at offset {offset}: need {need} bytes, have {have}")]
    Truncated { offset: u64, need: u64, have: u64 },

    #[error("Unsupported feature: {0}")]
    Unsupported(String),
}

pub type Result<T> = std::result::Result<T, SofaError>;
