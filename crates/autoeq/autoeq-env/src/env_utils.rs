//! Environment variable utilities for AutoEQ
//!
//! This module provides utilities for handling environment variables,
//! particularly the AUTOEQ_DIR variable that points to the AutoEQ project root.

use crate::constants::DATA_GENERATED;
use std::env;
use std::path::PathBuf;

/// Error type for environment variable issues
#[derive(Debug, thiserror::Error)]
pub enum EnvError {
    #[error(
        "AUTOEQ_DIR environment variable is not set and no autoeq/sotf directory found in $HOME or $HOME/src. Please set it to the AutoEQ project root directory (e.g., export AUTOEQ_DIR=/path/to/autoeq)"
    )]
    AutoeqDirNotSet,

    #[error("AUTOEQ_DIR points to a non-existent directory: {0}")]
    AutoeqDirNotFound(PathBuf),

    #[error("Failed to create data_generated directory: {0}")]
    DataGeneratedCreationFailed(std::io::Error),

    #[error(
        "Multiple autoeq/sotf directories found: {0:?}. Please set AUTOEQ_DIR to specify which one to use"
    )]
    MultipleAutoEqDirsFound(Vec<PathBuf>),
}

/// Get the AUTOEQ_DIR environment variable and validate it exists
///
/// If AUTOEQ_DIR is not set, this function will attempt to automatically
/// find the autoeq/sotf directory by searching in:
/// - $HOME/autoeq
/// - $HOME/sotf
/// - $HOME/src/autoeq
/// - $HOME/src/sotf
///
/// # Returns
///
/// Returns the path to the AutoEQ project root directory.
///
/// # Errors
///
/// Returns an error if:
/// - AUTOEQ_DIR is not set and no autoeq/sotf directory can be found
/// - AUTOEQ_DIR is not set and multiple autoeq/sotf directories are found
/// - AUTOEQ_DIR points to a non-existent directory
///
/// # Example
///
/// ```no_run
/// use autoeq_env::env_utils::get_autoeq_dir;
///
/// let autoeq_dir = get_autoeq_dir()?;
/// println!("AutoEQ directory: {}", autoeq_dir.display());
/// # Ok::<(), autoeq_env::env_utils::EnvError>(())
/// ```
pub fn get_autoeq_dir() -> Result<PathBuf, EnvError> {
    // First try the environment variable
    if let Ok(autoeq_dir) = env::var("AUTOEQ_DIR") {
        let path = PathBuf::from(autoeq_dir);
        if !path.exists() {
            return Err(EnvError::AutoeqDirNotFound(path));
        }
        return Ok(path);
    }

    // Try current directory and its parent
    if let Ok(current_dir) = env::current_dir() {
        // Check if current dir looks like AutoEQ root (has Cargo.toml and autoeq directory or crate)
        let candidates = vec![current_dir.clone(), current_dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("/"))];
        
        for dir in candidates {
            if dir.join("Cargo.toml").exists() {
                // Check for workspace members or specific file structure
                if dir.join("autoeq").exists() || dir.join("GEMINI.md").exists() {
                    return Ok(dir);
                }
            }
        }
    }

    // If AUTOEQ_DIR is not set and current dir is not it, try to guess the location
    let home = env::var("HOME").map_err(|_| EnvError::AutoeqDirNotSet)?;
    let home_path = PathBuf::from(home);

    let candidates = vec![
        home_path.join("autoeq"),
        home_path.join("sotf"),
        home_path.join("src").join("autoeq"),
        home_path.join("src").join("sotf"),
    ];

    let mut found_dirs = Vec::new();
    for candidate in candidates {
        if candidate.exists() && candidate.is_dir() {
            found_dirs.push(candidate);
        }
    }

    match found_dirs.len() {
        0 => Err(EnvError::AutoeqDirNotSet),
        1 => Ok(found_dirs.into_iter().next().unwrap()),
        _ => Err(EnvError::MultipleAutoEqDirsFound(found_dirs)),
    }
}

/// Get the path to the data_generated directory, creating it if necessary
///
/// This function:
/// 1. Gets the AUTOEQ_DIR from environment
/// 2. Constructs the path to data_generated
/// 3. Creates the directory if it doesn't exist
///
/// # Returns
///
/// Returns the path to the data_generated directory.
///
/// # Errors
///
/// Returns an error if:
/// - AUTOEQ_DIR is not set or invalid
/// - Cannot create the data_generated directory
///
/// # Example
///
/// ```no_run
/// use autoeq_env::env_utils::get_data_generated_dir;
///
/// let data_dir = get_data_generated_dir()?;
/// println!("Data directory: {}", data_dir.display());
/// # Ok::<(), autoeq_env::env_utils::EnvError>(())
/// ```
pub fn get_data_generated_dir() -> Result<PathBuf, EnvError> {
    let autoeq_dir = get_autoeq_dir()?;
    let data_generated = autoeq_dir.join(DATA_GENERATED);

    // Create the directory if it doesn't exist
    if !data_generated.exists() {
        std::fs::create_dir_all(&data_generated).map_err(EnvError::DataGeneratedCreationFailed)?;
    }

    Ok(data_generated)
}

/// Get the path to the records subdirectory within data_generated
///
/// This is a convenience function for the common case of writing
/// optimization records.
///
/// # Returns
///
/// Returns the path to the data_generated/records directory.
///
/// # Errors
///
/// Returns an error if:
/// - AUTOEQ_DIR is not set or invalid
/// - Cannot create the directories
///
/// # Example
///
/// ```no_run
/// use autoeq_env::env_utils::get_records_dir;
///
/// let records_dir = get_records_dir()?;
/// println!("Records directory: {}", records_dir.display());
/// # Ok::<(), autoeq_env::env_utils::EnvError>(())
/// ```
pub fn get_records_dir() -> Result<PathBuf, EnvError> {
    let data_generated = get_data_generated_dir()?;
    let records_dir = data_generated.join("records");

    // Create the records directory if it doesn't exist
    if !records_dir.exists() {
        std::fs::create_dir_all(&records_dir).map_err(EnvError::DataGeneratedCreationFailed)?;
    }

    Ok(records_dir)
}

/// Check if AUTOEQ_DIR is properly configured and print helpful information
///
/// This function is useful for diagnostic purposes and can be called
/// at the start of applications to provide clear error messages.
///
/// # Example
///
/// ```no_run
/// use autoeq_env::env_utils::check_autoeq_env;
///
/// // At the start of your application
/// if let Err(e) = check_autoeq_env() {
///     eprintln!("Environment setup error: {}", e);
///     eprintln!("Please set AUTOEQ_DIR to your AutoEQ project root directory.");
///     eprintln!("Example: export AUTOEQ_DIR=/path/to/your/autoeq/project");
///     std::process::exit(1);
/// }
/// ```
pub fn check_autoeq_env() -> Result<(), EnvError> {
    let autoeq_dir = get_autoeq_dir()?;
    let data_generated = get_data_generated_dir()?;

    println!("✓ AUTOEQ_DIR: {}", autoeq_dir.display());
    println!("✓ Data directory: {}", data_generated.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_autoeq_dir() {
        // This test verifies that get_autoeq_dir works when AUTOEQ_DIR is set
        // If AUTOEQ_DIR is not set, the test will fail with a helpful message
        let result = get_autoeq_dir();

        match result {
            Ok(path) => {
                assert!(path.exists(), "AUTOEQ_DIR points to non-existent directory");
                println!("✓ AUTOEQ_DIR is set to: {}", path.display());
            }
            Err(e) => {
                panic!(
                    "Test requires AUTOEQ_DIR to be set. Error: {}\nPlease run: export AUTOEQ_DIR=/path/to/autoeq",
                    e
                );
            }
        }
    }

    #[test]
    fn test_get_data_generated_dir() {
        // This test verifies that get_data_generated_dir works and creates the directory
        let result = get_data_generated_dir();

        match result {
            Ok(path) => {
                assert!(
                    path.exists(),
                    "data_generated directory should exist after calling get_data_generated_dir"
                );
                println!("✓ data_generated directory: {}", path.display());
            }
            Err(e) => {
                panic!(
                    "Test requires AUTOEQ_DIR to be set. Error: {}\nPlease run: export AUTOEQ_DIR=/path/to/autoeq",
                    e
                );
            }
        }
    }
}
