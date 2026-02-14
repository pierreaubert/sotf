//! AutoEQ Environment and Constants
//!
//! This crate provides shared environment utilities and constants for the AutoEQ workspace.
//! It centralizes environment variable handling and common constants that are used across
//! multiple workspace members.

pub mod constants;
pub mod env_utils;

// Include the log macros module to make macros available
#[macro_use]
pub mod log;

// Re-export commonly used items
pub use constants::{DATA_CACHED, DATA_GENERATED};
pub use env_utils::{
    EnvError, check_autoeq_env, get_autoeq_dir, get_data_generated_dir, get_records_dir,
};
