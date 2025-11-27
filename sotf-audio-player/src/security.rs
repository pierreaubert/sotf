//! Security utilities for path validation
//!
//! This module provides functions to ensure that file operations
//! stay within allowed directories to prevent path traversal attacks.

use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};

/// Error type for security violations
#[derive(Debug)]
pub struct SecurityError {
    pub message: String,
    pub path: PathBuf,
    pub allowed_dirs: Vec<PathBuf>,
}

impl std::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Security violation: {} - path '{}' is not within allowed directories",
            self.message,
            self.path.display()
        )
    }
}

impl std::error::Error for SecurityError {}

impl From<SecurityError> for std::io::Error {
    fn from(e: SecurityError) -> Self {
        Error::new(ErrorKind::PermissionDenied, e.to_string())
    }
}

/// Canonicalize a path, resolving symlinks and removing ".." components.
/// Returns None if the path doesn't exist or can't be canonicalized.
fn safe_canonicalize(path: &Path) -> Option<PathBuf> {
    // First try to canonicalize the path directly
    if let Ok(canonical) = path.canonicalize() {
        return Some(canonical);
    }

    // If that fails (e.g., file doesn't exist yet), canonicalize the parent
    // and append the file name
    if let Some(parent) = path.parent() {
        if let Ok(canonical_parent) = parent.canonicalize() {
            if let Some(file_name) = path.file_name() {
                return Some(canonical_parent.join(file_name));
            }
        }
    }

    None
}

/// Check if a path is within an allowed directory.
/// This function canonicalizes both paths to prevent path traversal attacks.
fn is_path_within_dir(path: &Path, allowed_dir: &Path) -> bool {
    // Canonicalize both paths
    let canonical_path = match safe_canonicalize(path) {
        Some(p) => p,
        None => return false,
    };

    let canonical_allowed = match allowed_dir.canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };

    // Check if the path starts with the allowed directory
    canonical_path.starts_with(&canonical_allowed)
}

/// Validate that a path is within the configuration directory.
/// This should be used for all write operations.
pub fn validate_write_path(path: &Path) -> Result<PathBuf, SecurityError> {
    let config_dir = match crate::config::get_app_config_dir() {
        Some(dir) => dir,
        None => {
            return Err(SecurityError {
                message: "Could not determine config directory".to_string(),
                path: path.to_path_buf(),
                allowed_dirs: vec![],
            });
        }
    };

    if is_path_within_dir(path, &config_dir) {
        // Return the canonicalized path
        Ok(safe_canonicalize(path).unwrap_or_else(|| path.to_path_buf()))
    } else {
        Err(SecurityError {
            message: "Write operation not allowed outside config directory".to_string(),
            path: path.to_path_buf(),
            allowed_dirs: vec![config_dir],
        })
    }
}

/// Validate that a path is within allowed read directories.
/// Allowed directories include:
/// - The configuration directory (for config files, database, presets)
/// - Any registered music directories
pub fn validate_read_path(path: &Path, music_directories: &[PathBuf]) -> Result<PathBuf, SecurityError> {
    let config_dir = crate::config::get_app_config_dir();

    // Build list of allowed directories
    let mut allowed_dirs: Vec<PathBuf> = music_directories.to_vec();
    if let Some(ref config) = config_dir {
        allowed_dirs.push(config.clone());
    }

    // Check if path is within any allowed directory
    for allowed_dir in &allowed_dirs {
        if is_path_within_dir(path, allowed_dir) {
            return Ok(safe_canonicalize(path).unwrap_or_else(|| path.to_path_buf()));
        }
    }

    Err(SecurityError {
        message: "Read operation not allowed outside music or config directories".to_string(),
        path: path.to_path_buf(),
        allowed_dirs,
    })
}

/// Validate that a path is within the configuration directory for reading.
/// Use this for reading config files, database, presets, etc.
pub fn validate_config_read_path(path: &Path) -> Result<PathBuf, SecurityError> {
    let config_dir = match crate::config::get_app_config_dir() {
        Some(dir) => dir,
        None => {
            return Err(SecurityError {
                message: "Could not determine config directory".to_string(),
                path: path.to_path_buf(),
                allowed_dirs: vec![],
            });
        }
    };

    if is_path_within_dir(path, &config_dir) {
        Ok(safe_canonicalize(path).unwrap_or_else(|| path.to_path_buf()))
    } else {
        Err(SecurityError {
            message: "Read operation not allowed outside config directory".to_string(),
            path: path.to_path_buf(),
            allowed_dirs: vec![config_dir],
        })
    }
}

/// Validate that a path is within a music directory for reading audio files.
pub fn validate_music_read_path(path: &Path, music_directories: &[PathBuf]) -> Result<PathBuf, SecurityError> {
    if music_directories.is_empty() {
        return Err(SecurityError {
            message: "No music directories configured".to_string(),
            path: path.to_path_buf(),
            allowed_dirs: vec![],
        });
    }

    for music_dir in music_directories {
        if is_path_within_dir(path, music_dir) {
            return Ok(safe_canonicalize(path).unwrap_or_else(|| path.to_path_buf()));
        }
    }

    Err(SecurityError {
        message: "Read operation not allowed outside music directories".to_string(),
        path: path.to_path_buf(),
        allowed_dirs: music_directories.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_validate_write_path_within_config() {
        if let Some(config_dir) = crate::config::get_app_config_dir() {
            let valid_path = config_dir.join("test_file.json");
            assert!(validate_write_path(&valid_path).is_ok());
        }
    }

    #[test]
    fn test_validate_write_path_outside_config() {
        let invalid_path = PathBuf::from("/tmp/malicious_file.txt");
        assert!(validate_write_path(&invalid_path).is_err());
    }

    #[test]
    fn test_validate_write_path_traversal_attack() {
        if let Some(config_dir) = crate::config::get_app_config_dir() {
            // Try path traversal
            let malicious_path = config_dir.join("..").join("..").join("etc").join("passwd");
            assert!(validate_write_path(&malicious_path).is_err());
        }
    }

    #[test]
    fn test_validate_read_path_within_music_dir() {
        let temp_dir = env::temp_dir().join("test_music");
        std::fs::create_dir_all(&temp_dir).ok();

        let music_dirs = vec![temp_dir.clone()];
        let valid_path = temp_dir.join("song.flac");

        // Create the file so canonicalization works
        std::fs::write(&valid_path, b"test").ok();

        let result = validate_music_read_path(&valid_path, &music_dirs);
        assert!(result.is_ok());

        // Cleanup
        std::fs::remove_file(&valid_path).ok();
        std::fs::remove_dir(&temp_dir).ok();
    }

    #[test]
    fn test_validate_read_path_traversal_attack() {
        let temp_dir = env::temp_dir().join("test_music2");
        std::fs::create_dir_all(&temp_dir).ok();

        let music_dirs = vec![temp_dir.clone()];
        let malicious_path = temp_dir.join("..").join("..").join("etc").join("passwd");

        let result = validate_music_read_path(&malicious_path, &music_dirs);
        assert!(result.is_err());

        // Cleanup
        std::fs::remove_dir(&temp_dir).ok();
    }
}
