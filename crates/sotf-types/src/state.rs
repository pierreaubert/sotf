//! Playback state and audio frame types.

use super::AudioSource;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A chunk of interleaved audio samples
#[derive(Clone, Debug)]
pub struct AudioFrame {
    /// Interleaved samples: [L0, R0, L1, R1, ...] for stereo
    pub data: Vec<f32>,
    /// Number of frames (not samples!)
    pub num_frames: usize,
    /// Number of channels
    pub num_channels: usize,
    /// Sample rate
    pub sample_rate: u32,
}

impl AudioFrame {
    /// Create a new audio frame
    pub fn new(data: Vec<f32>, num_frames: usize, num_channels: usize, sample_rate: u32) -> Self {
        debug_assert_eq!(data.len(), num_frames * num_channels);
        Self {
            data,
            num_frames,
            num_channels,
            sample_rate,
        }
    }

    /// Create an empty (silent) audio frame
    pub fn silent(num_frames: usize, num_channels: usize, sample_rate: u32) -> Self {
        Self {
            data: vec![0.0; num_frames * num_channels],
            num_frames,
            num_channels,
            sample_rate,
        }
    }

    /// Total number of samples (frames x channels)
    pub fn num_samples(&self) -> usize {
        self.num_frames * self.num_channels
    }

    /// Clear the frame (set to silence)
    pub fn clear(&mut self) {
        self.data.fill(0.0);
    }
}

/// Playback state
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

/// Complete audio engine state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioEngineState {
    /// Current playback state
    pub playback_state: PlaybackState,
    /// Currently playing source
    pub current_source: Option<AudioSource>,
    /// Currently playing file path (convenience accessor, populated for File sources)
    pub current_file: Option<PathBuf>,
    /// Current position in seconds
    pub position: f64,
    /// Total duration in seconds (if known)
    pub duration: Option<f64>,
    /// Sample rate
    pub sample_rate: u32,
    /// Number of channels
    pub num_channels: usize,
    /// Output volume (linear)
    pub volume: f32,
    /// Muted flag
    pub muted: bool,
    /// Processing bypassed flag
    pub processing_bypassed: bool,
    /// Number of buffer underruns
    pub underruns: u64,
    /// Total plugin chain latency in samples (for position compensation)
    pub plugin_latency_samples: usize,
    /// Last error message, if any
    pub last_error: Option<String>,
    /// Seek in progress flag
    pub seeking: bool,
}

impl Default for AudioEngineState {
    fn default() -> Self {
        Self {
            playback_state: PlaybackState::Stopped,
            current_source: None,
            current_file: None,
            position: 0.0,
            duration: None,
            sample_rate: 48000,
            num_channels: 2,
            volume: 1.0,
            muted: false,
            processing_bypassed: false,
            underruns: 0,
            plugin_latency_samples: 0,
            last_error: None,
            seeking: false,
        }
    }
}
