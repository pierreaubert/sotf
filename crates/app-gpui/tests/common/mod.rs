//! Common test helpers for GPUI component tests
//!
//! Provides shared test utilities. For test data builders that produce
//! real `Album`/`Track` types, see `factories`.
//!
//! NOTE: This module contains ONLY:
//! - Shared factories for building real production types
//! - Pure math utilities for parameter normalization (no production equivalent)
//! - EQ filter validation utilities (test-only boundary checking)
//!
//! All production types should be imported directly from `sotf_audio_player_gpui`
//! or `sotf_audio_player`. Do NOT add mirror types here.

pub mod factories;
pub mod state_builder;

use std::path::{Path, PathBuf};

// ============================================================================
// Test File System Utilities
// ============================================================================

/// RAII temporary directory for tests that need file system fixtures.
/// Automatically cleaned up on drop via `tempfile::TempDir`.
pub struct TestTempDir {
    dir: tempfile::TempDir,
}

impl TestTempDir {
    pub fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("create temp dir"),
        }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Create an empty file at the given relative path, creating parent dirs as needed.
    pub fn create_file(&self, relative_path: &str) -> PathBuf {
        let full_path = self.dir.path().join(relative_path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).expect("create parent dirs");
        }
        std::fs::write(&full_path, b"").expect("create file");
        full_path
    }

    /// Create a minimal WAV file at the given relative path (for tests that need
    /// a parseable audio file rather than just a path).
    pub fn create_fake_audio(&self, relative_path: &str) -> PathBuf {
        let full_path = self.dir.path().join(relative_path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).expect("create parent dirs");
        }
        // Minimal 44-byte WAV header (silent, 0 samples)
        let header: [u8; 44] = [
            0x52, 0x49, 0x46, 0x46, // "RIFF"
            0x24, 0x00, 0x00, 0x00, // file size - 8
            0x57, 0x41, 0x56, 0x45, // "WAVE"
            0x66, 0x6D, 0x74, 0x20, // "fmt "
            0x10, 0x00, 0x00, 0x00, // chunk size (16)
            0x01, 0x00, // PCM format
            0x02, 0x00, // 2 channels
            0x44, 0xAC, 0x00, 0x00, // 44100 Hz
            0x10, 0xB1, 0x02, 0x00, // byte rate
            0x04, 0x00, // block align
            0x10, 0x00, // 16 bits per sample
            0x64, 0x61, 0x74, 0x61, // "data"
            0x00, 0x00, 0x00, 0x00, // data size (0)
        ];
        std::fs::write(&full_path, header).expect("create fake audio");
        full_path
    }

    /// Create an album directory structure with N fake tracks.
    pub fn create_album_dir(&self, album_name: &str, track_count: usize) -> PathBuf {
        let album_dir = self.dir.path().join(album_name);
        std::fs::create_dir_all(&album_dir).expect("create album dir");
        for i in 1..=track_count {
            let track_path = album_dir.join(format!("track_{:02}.wav", i));
            self.create_fake_audio(
                track_path
                    .strip_prefix(self.dir.path())
                    .unwrap()
                    .to_str()
                    .unwrap(),
            );
        }
        album_dir
    }
}

// ============================================================================
// Pure Math Utilities (test-only — no production equivalent)
// ============================================================================

/// Verify a parameter value is within valid range
pub fn clamp_parameter(value: f64, min: f64, max: f64) -> f64 {
    value.clamp(min, max)
}

/// Calculate normalized value (0.0-1.0) from absolute value
pub fn normalize_parameter(value: f64, min: f64, max: f64) -> f64 {
    if max == min {
        return 0.0;
    }
    (value - min) / (max - min)
}

/// Calculate absolute value from normalized value (0.0-1.0)
pub fn denormalize_parameter(normalized: f64, min: f64, max: f64) -> f64 {
    min + normalized * (max - min)
}

/// Calculate logarithmic normalized value for Hz parameters
pub fn normalize_parameter_log(value: f64, min: f64, max: f64) -> f64 {
    if max == min || min <= 0.0 {
        return 0.0;
    }
    (value.ln() - min.ln()) / (max.ln() - min.ln())
}

/// Calculate absolute value from logarithmic normalized value
pub fn denormalize_parameter_log(normalized: f64, min: f64, max: f64) -> f64 {
    if min <= 0.0 {
        return min;
    }
    (min.ln() + normalized * (max.ln() - min.ln())).exp()
}

// ============================================================================
// EQ Filter Validation (test-only boundary checking)
// ============================================================================

/// Filter type for EQ bands (test-only, used by validation utilities)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterType {
    Peak,
    LowShelf,
    HighShelf,
    LowPass,
    HighPass,
    BandPass,
    Notch,
}

/// EQ Filter representation for test validation
#[derive(Debug, Clone)]
pub struct TestEQFilter {
    pub filter_type: FilterType,
    pub frequency: f64,
    pub q: f64,
    pub gain_db: f64,
}

impl TestEQFilter {
    pub fn new(filter_type: FilterType, frequency: f64, q: f64, gain_db: f64) -> Self {
        Self {
            filter_type,
            frequency,
            q,
            gain_db,
        }
    }

    /// Create a default peak filter at 1kHz (matches add_eq_band behavior)
    pub fn default_peak() -> Self {
        Self::new(FilterType::Peak, 1000.0, 1.0, 0.0)
    }
}

/// Add a new EQ band to the filter list. Returns the new filter count.
pub fn add_eq_band(filters: &mut Vec<TestEQFilter>) -> usize {
    filters.push(TestEQFilter::default_peak());
    filters.len()
}

/// Remove an EQ band at the given index.
/// Returns Ok(new count) if successful, Err if index out of bounds.
pub fn remove_eq_band(filters: &mut Vec<TestEQFilter>, index: usize) -> Result<usize, String> {
    if index >= filters.len() {
        return Err(format!(
            "Invalid band index {} for {} bands",
            index,
            filters.len()
        ));
    }
    if filters.len() <= 1 {
        return Err("Cannot remove the last EQ band".to_string());
    }
    filters.remove(index);
    Ok(filters.len())
}

/// Validate EQ filter parameters are within acceptable ranges
pub fn validate_eq_filter(filter: &TestEQFilter) -> Result<(), String> {
    if filter.frequency < 20.0 || filter.frequency > 20000.0 {
        return Err(format!(
            "Frequency {} Hz is outside valid range (20-20000 Hz)",
            filter.frequency
        ));
    }
    if filter.q <= 0.0 {
        return Err(format!("Q factor {} must be positive", filter.q));
    }
    if filter.q < 0.1 || filter.q > 10.0 {
        return Err(format!(
            "Q factor {} is outside reasonable range (0.1-10.0)",
            filter.q
        ));
    }
    if filter.gain_db < -24.0 || filter.gain_db > 24.0 {
        return Err(format!(
            "Gain {} dB is outside valid range (-24 to +24 dB)",
            filter.gain_db
        ));
    }
    Ok(())
}
