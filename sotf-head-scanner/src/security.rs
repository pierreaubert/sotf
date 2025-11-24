//! Security utilities for path validation and sanitization
//!
//! This module provides secure path validation to prevent path traversal attacks,
//! symlink attacks, and other file system security vulnerabilities.

use crate::error::{ScannerError, ScannerResult};
use std::path::{Path, PathBuf};

/// Validate and canonicalize a file path within an allowed base directory
///
/// This function prevents:
/// - Path traversal attacks (../, URL-encoded variants)
/// - Absolute paths outside allowed directories
/// - Symlink attacks
/// - Windows drive letter attacks (C:, \\share\)
///
/// # Security
///
/// Uses path canonicalization which resolves:
/// - Relative components (. and ..)
/// - Symbolic links
/// - Redundant separators
///
/// # Arguments
///
/// * `path` - The path to validate
/// * `base_dir` - Optional base directory that the path must be within
///
/// # Returns
///
/// Returns the canonicalized absolute path if valid
///
/// # Example
///
/// ```no_run
/// use head_scanner::security::validate_path;
/// use std::path::PathBuf;
///
/// let base = PathBuf::from("/var/data/models");
/// let user_path = "my_model.onnx";
///
/// // Safe: resolves to /var/data/models/my_model.onnx
/// let safe_path = validate_path(user_path, Some(&base)).unwrap();
///
/// // BLOCKED: would resolve outside base directory
/// let bad_path = "../../../etc/passwd";
/// assert!(validate_path(bad_path, Some(&base)).is_err());
/// ```
pub fn validate_path(path: &str, base_dir: Option<&Path>) -> ScannerResult<PathBuf> {
    // Check for null bytes (C string terminator attacks)
    if path.contains('\0') {
        return Err(ScannerError::InvalidConfig(
            "Null byte detected in path".to_string(),
        ));
    }

    // Check for URL-encoded path traversal attempts
    if path.contains("%2e%2e") || path.contains("%2E%2E") || path.contains("..") {
        return Err(ScannerError::InvalidConfig(
            "Path traversal detected".to_string(),
        ));
    }

    // Check for backslash traversal (Windows)
    if path.contains("..\\") || path.contains("..\\\\") {
        return Err(ScannerError::InvalidConfig(
            "Path traversal detected (Windows style)".to_string(),
        ));
    }

    // Create path object
    let path_obj = Path::new(path);

    // Reject absolute paths if a base directory is specified
    if let Some(base) = base_dir {
        if path_obj.is_absolute() {
            return Err(ScannerError::InvalidConfig(
                "Absolute paths not allowed when base directory is specified".to_string(),
            ));
        }

        // Construct full path
        let full_path = base.join(path_obj);

        // Canonicalize to resolve symlinks and .. components
        let canonical = full_path
            .canonicalize()
            .map_err(|e| ScannerError::IoError(format!("Failed to canonicalize path: {}", e)))?;

        // Canonicalize base directory
        let canonical_base = base.canonicalize().map_err(|e| {
            ScannerError::IoError(format!("Failed to canonicalize base directory: {}", e))
        })?;

        // Ensure the canonical path is within the base directory
        if !canonical.starts_with(&canonical_base) {
            return Err(ScannerError::InvalidConfig(format!(
                "Path escapes base directory: {} not in {}",
                canonical.display(),
                canonical_base.display()
            )));
        }

        Ok(canonical)
    } else {
        // No base directory specified - just canonicalize and return
        path_obj
            .canonicalize()
            .map_err(|e| ScannerError::IoError(format!("Invalid path: {}", e)))
    }
}

/// Validate a path for export operations (writing files)
///
/// This is a more lenient version that:
/// - Allows creating new files (path doesn't need to exist)
/// - Still prevents path traversal
/// - Validates the parent directory exists
///
/// # Arguments
///
/// * `path` - The output path to validate
/// * `base_dir` - Optional base directory for relative paths
///
/// # Returns
///
/// Returns the validated path (not canonicalized if file doesn't exist yet)
pub fn validate_export_path(path: &str, base_dir: Option<&Path>) -> ScannerResult<PathBuf> {
    // Check for null bytes
    if path.contains('\0') {
        return Err(ScannerError::InvalidConfig(
            "Null byte detected in path".to_string(),
        ));
    }

    // Check for path traversal
    if path.contains("%2e%2e") || path.contains("%2E%2E") || path.contains("..") {
        return Err(ScannerError::InvalidConfig(
            "Path traversal detected in export path".to_string(),
        ));
    }

    // Check for backslash traversal (Windows)
    if path.contains("..\\") {
        return Err(ScannerError::InvalidConfig(
            "Path traversal detected (Windows style)".to_string(),
        ));
    }

    let path_obj = Path::new(path);

    // Build full path with base directory if provided
    let full_path = if let Some(base) = base_dir {
        if path_obj.is_absolute() {
            return Err(ScannerError::InvalidConfig(
                "Absolute paths not allowed when base directory is specified".to_string(),
            ));
        }
        base.join(path_obj)
    } else {
        path_obj.to_path_buf()
    };

    // Get parent directory
    let parent = full_path.parent().ok_or_else(|| {
        ScannerError::InvalidConfig("Invalid path: no parent directory".to_string())
    })?;

    // If parent doesn't exist, try to create it (optional - may want to require it exists)
    if !parent.exists() {
        std::fs::create_dir_all(parent).map_err(|e| {
            ScannerError::IoError(format!("Failed to create parent directory: {}", e))
        })?;
    }

    // Canonicalize parent to prevent symlink attacks
    let canonical_parent = parent.canonicalize().map_err(|e| {
        ScannerError::IoError(format!("Failed to canonicalize parent directory: {}", e))
    })?;

    // If base directory specified, ensure parent is within it
    if let Some(base) = base_dir {
        let canonical_base = base.canonicalize().map_err(|e| {
            ScannerError::IoError(format!("Failed to canonicalize base directory: {}", e))
        })?;

        if !canonical_parent.starts_with(&canonical_base) {
            return Err(ScannerError::InvalidConfig(format!(
                "Export path escapes base directory: {} not in {}",
                canonical_parent.display(),
                canonical_base.display()
            )));
        }
    }

    // Return the full path (filename may not exist yet)
    Ok(canonical_parent.join(full_path.file_name().unwrap()))
}

/// Validate a model path for loading ML models
///
/// This is stricter and requires:
/// - File must exist
/// - File must be within allowed model directory
/// - Symlinks are resolved and checked
///
/// # Arguments
///
/// * `path` - The model path to validate
/// * `allowed_model_dirs` - List of allowed base directories for models
///
/// # Returns
///
/// Returns the canonicalized path if valid
pub fn validate_model_path(path: &str, allowed_model_dirs: &[PathBuf]) -> ScannerResult<PathBuf> {
    // Basic checks
    if path.contains('\0') {
        return Err(ScannerError::InvalidConfig(
            "Null byte detected in model path".to_string(),
        ));
    }

    if path.contains("%2e%2e") || path.contains("%2E%2E") || path.contains("..") {
        return Err(ScannerError::InvalidConfig(
            "Path traversal detected in model path".to_string(),
        ));
    }

    let path_obj = Path::new(path);

    // File must exist
    if !path_obj.exists() {
        return Err(ScannerError::InvalidConfig(format!(
            "Model file does not exist: {}",
            path
        )));
    }

    // Canonicalize to resolve symlinks
    let canonical = path_obj
        .canonicalize()
        .map_err(|e| ScannerError::IoError(format!("Failed to canonicalize model path: {}", e)))?;

    // If allowed directories specified, check the path is within one of them
    if !allowed_model_dirs.is_empty() {
        let mut allowed = false;

        for base_dir in allowed_model_dirs {
            let canonical_base = base_dir.canonicalize().map_err(|e| {
                ScannerError::IoError(format!("Failed to canonicalize base directory: {}", e))
            })?;

            if canonical.starts_with(&canonical_base) {
                allowed = true;
                break;
            }
        }

        if !allowed {
            return Err(ScannerError::InvalidConfig(format!(
                "Model path not in allowed directories: {}",
                canonical.display()
            )));
        }
    }

    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_path_traversal_detection() {
        let base = PathBuf::from("/tmp/test");

        // These should all be rejected
        assert!(validate_path("../etc/passwd", Some(&base)).is_err());
        assert!(validate_path("..\\windows\\system32", Some(&base)).is_err());
        assert!(validate_path("%2e%2e/etc/passwd", Some(&base)).is_err());
        assert!(validate_path("file\0name", Some(&base)).is_err());
    }

    #[test]
    fn test_export_path_validation() {
        // Create temp directory
        let temp_dir = std::env::temp_dir().join("head_scanner_test");
        fs::create_dir_all(&temp_dir).unwrap();

        // Valid export path
        let result = validate_export_path("output.obj", Some(&temp_dir));
        assert!(result.is_ok());

        // Path traversal should fail
        let result = validate_export_path("../etc/passwd", Some(&temp_dir));
        assert!(result.is_err());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
