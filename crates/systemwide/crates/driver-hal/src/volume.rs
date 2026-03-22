//! Volume Control Module for HAL Driver
//!
//! This module provides volume control functionality that integrates with
//! the DSP chain via the GainPlugin. It supports both global volume (all
//! channels) and per-channel volume control.
//!
//! # Example
//!
//! ```rust,ignore
//! use driver_hal::volume::{VolumeControl, VolumeConfig};
//!
//! // Create volume control for stereo
//! let mut volume = VolumeControl::new(2, 48000);
//!
//! // Set global volume to -6dB
//! volume.set_volume_db(-6.0);
//!
//! // Or set per-channel volumes
//! volume.set_channel_volume_db(0, 0.0);   // Left: 0dB
//! volume.set_channel_volume_db(1, -3.0);  // Right: -3dB
//!
//! // Process audio through the volume control
//! volume.process(&mut audio_buffer, num_frames);
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// Volume control configuration
#[derive(Debug, Clone)]
pub struct VolumeConfig {
    /// Global volume in dB (used when per-channel is not set)
    pub volume_db: f32,
    /// Per-channel volumes in dB (optional)
    /// If set, must have exactly one value per channel
    pub channel_volumes_db: Vec<f32>,
    /// Whether the output is muted
    pub muted: bool,
}

impl Default for VolumeConfig {
    fn default() -> Self {
        Self {
            volume_db: 0.0,
            channel_volumes_db: Vec::new(),
            muted: false,
        }
    }
}

impl VolumeConfig {
    /// Create a new volume config with global volume
    pub fn new(volume_db: f32) -> Self {
        Self {
            volume_db,
            channel_volumes_db: Vec::new(),
            muted: false,
        }
    }

    /// Create a new volume config with per-channel volumes
    pub fn with_channel_volumes(channel_volumes_db: Vec<f32>) -> Self {
        Self {
            volume_db: 0.0,
            channel_volumes_db,
            muted: false,
        }
    }
}

/// Thread-safe atomic volume value
///
/// This can be shared between threads for real-time safe volume updates.
#[derive(Debug)]
pub struct AtomicVolume {
    /// Volume stored as f32 bits in atomic u32
    volume_bits: AtomicU32,
    /// Mute state
    muted: std::sync::atomic::AtomicBool,
}

impl AtomicVolume {
    /// Create a new atomic volume at unity gain (0 dB)
    pub fn new() -> Self {
        Self {
            volume_bits: AtomicU32::new(1.0_f32.to_bits()),
            muted: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Create with initial linear gain value
    pub fn with_linear(linear_gain: f32) -> Self {
        Self {
            volume_bits: AtomicU32::new(linear_gain.to_bits()),
            muted: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Create with initial dB value
    pub fn with_db(db: f32) -> Self {
        Self::with_linear(db_to_linear(db))
    }

    /// Set volume from linear gain (0.0 to 1.0+)
    pub fn set_linear(&self, gain: f32) {
        self.volume_bits.store(gain.to_bits(), Ordering::Release);
    }

    /// Set volume from dB value (clamped to -60 to +20 dB range)
    pub fn set_db(&self, db: f32) {
        self.set_linear(db_to_linear(clamp_volume_db(db)));
    }

    /// Get current volume as linear gain
    pub fn get_linear(&self) -> f32 {
        f32::from_bits(self.volume_bits.load(Ordering::Acquire))
    }

    /// Get current volume in dB
    pub fn get_db(&self) -> f32 {
        linear_to_db(self.get_linear())
    }

    /// Set mute state
    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Release);
    }

    /// Get mute state
    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Acquire)
    }

    /// Get effective linear gain (0 if muted)
    pub fn effective_linear(&self) -> f32 {
        if self.is_muted() {
            0.0
        } else {
            self.get_linear()
        }
    }
}

impl Default for AtomicVolume {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for AtomicVolume {
    fn clone(&self) -> Self {
        Self {
            volume_bits: AtomicU32::new(self.volume_bits.load(Ordering::Acquire)),
            muted: std::sync::atomic::AtomicBool::new(self.muted.load(Ordering::Acquire)),
        }
    }
}

/// Per-channel atomic volume control
///
/// Provides thread-safe volume control for multiple channels.
#[derive(Debug)]
pub struct AtomicChannelVolumes {
    /// Per-channel volumes (linear gain stored as atomic u32 bits)
    channels: Vec<AtomicU32>,
    /// Global mute state
    muted: std::sync::atomic::AtomicBool,
}

impl AtomicChannelVolumes {
    /// Create with specified number of channels at unity gain
    pub fn new(num_channels: usize) -> Self {
        Self {
            channels: (0..num_channels)
                .map(|_| AtomicU32::new(1.0_f32.to_bits()))
                .collect(),
            muted: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Create with initial per-channel dB values (clamped to -60 to +20 dB range)
    pub fn with_db(channel_volumes_db: &[f32]) -> Self {
        Self {
            channels: channel_volumes_db
                .iter()
                .map(|&db| AtomicU32::new(db_to_linear(clamp_volume_db(db)).to_bits()))
                .collect(),
            muted: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Number of channels
    pub fn num_channels(&self) -> usize {
        self.channels.len()
    }

    /// Set volume for a specific channel in dB (clamped to -60 to +20 dB range)
    pub fn set_channel_db(&self, channel: usize, db: f32) {
        if channel < self.channels.len() {
            self.channels[channel].store(
                db_to_linear(clamp_volume_db(db)).to_bits(),
                Ordering::Release,
            );
        }
    }

    /// Set volume for a specific channel as linear gain
    pub fn set_channel_linear(&self, channel: usize, gain: f32) {
        if channel < self.channels.len() {
            self.channels[channel].store(gain.to_bits(), Ordering::Release);
        }
    }

    /// Get volume for a specific channel as linear gain
    pub fn get_channel_linear(&self, channel: usize) -> f32 {
        if channel < self.channels.len() {
            f32::from_bits(self.channels[channel].load(Ordering::Acquire))
        } else {
            1.0
        }
    }

    /// Get volume for a specific channel in dB
    pub fn get_channel_db(&self, channel: usize) -> f32 {
        linear_to_db(self.get_channel_linear(channel))
    }

    /// Set all channels to the same dB value (clamped to -60 to +20 dB range)
    pub fn set_all_db(&self, db: f32) {
        let linear = db_to_linear(clamp_volume_db(db));
        for ch in &self.channels {
            ch.store(linear.to_bits(), Ordering::Release);
        }
    }

    /// Set mute state
    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Release);
    }

    /// Get mute state
    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Acquire)
    }

    /// Get effective linear gain for a channel (0 if muted)
    pub fn effective_channel_linear(&self, channel: usize) -> f32 {
        if self.is_muted() {
            0.0
        } else {
            self.get_channel_linear(channel)
        }
    }

    /// Apply volumes to an interleaved audio buffer
    ///
    /// This is a simple non-smoothed application suitable for real-time
    /// processing when the volume doesn't change frequently.
    pub fn apply_to_buffer(&self, buffer: &mut [f32], num_frames: usize) {
        let num_channels = self.num_channels();
        if buffer.len() != num_frames * num_channels {
            return;
        }

        if self.is_muted() {
            buffer.fill(0.0);
            return;
        }

        // Pre-fetch all channel gains
        let gains: Vec<f32> = (0..num_channels)
            .map(|ch| self.get_channel_linear(ch))
            .collect();

        for frame in 0..num_frames {
            for (ch, &gain) in gains.iter().enumerate() {
                let idx = frame * num_channels + ch;
                buffer[idx] *= gain;
            }
        }
    }
}

/// Shared volume control that can be used across threads
pub type SharedVolume = Arc<AtomicVolume>;
pub type SharedChannelVolumes = Arc<AtomicChannelVolumes>;

/// Create a shared global volume control
pub fn create_shared_volume() -> SharedVolume {
    Arc::new(AtomicVolume::new())
}

/// Create a shared per-channel volume control
pub fn create_shared_channel_volumes(num_channels: usize) -> SharedChannelVolumes {
    Arc::new(AtomicChannelVolumes::new(num_channels))
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Convert dB to linear gain
#[inline]
pub fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

/// Convert linear gain to dB
#[inline]
pub fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.0 {
        f32::NEG_INFINITY
    } else {
        20.0 * linear.log10()
    }
}

/// Clamp volume to valid range
#[inline]
pub fn clamp_volume_db(db: f32) -> f32 {
    db.clamp(-60.0, 20.0)
}

/// Clamp linear gain to valid range
#[inline]
pub fn clamp_volume_linear(linear: f32) -> f32 {
    linear.clamp(0.0, db_to_linear(20.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_to_linear() {
        assert!((db_to_linear(0.0) - 1.0).abs() < 0.001);
        assert!((db_to_linear(-6.0) - 0.501).abs() < 0.01);
        assert!((db_to_linear(-12.0) - 0.251).abs() < 0.01);
        assert!((db_to_linear(6.0) - 1.995).abs() < 0.01);
        assert!((db_to_linear(-20.0) - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_linear_to_db() {
        assert!((linear_to_db(1.0) - 0.0).abs() < 0.001);
        assert!((linear_to_db(0.5) - (-6.02)).abs() < 0.1);
        assert!((linear_to_db(0.25) - (-12.04)).abs() < 0.1);
        assert!(linear_to_db(0.0).is_infinite());
    }

    #[test]
    fn test_atomic_volume() {
        let vol = AtomicVolume::new();
        assert!((vol.get_linear() - 1.0).abs() < 0.001);
        assert!((vol.get_db() - 0.0).abs() < 0.001);

        vol.set_db(-6.0);
        assert!((vol.get_db() - (-6.0)).abs() < 0.001);
        assert!((vol.get_linear() - 0.501).abs() < 0.01);

        vol.set_linear(0.5);
        assert!((vol.get_linear() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_atomic_volume_mute() {
        let vol = AtomicVolume::with_db(-6.0);
        assert!(!vol.is_muted());
        assert!((vol.effective_linear() - 0.501).abs() < 0.01);

        vol.set_muted(true);
        assert!(vol.is_muted());
        assert!((vol.effective_linear() - 0.0).abs() < 0.001);

        vol.set_muted(false);
        assert!((vol.effective_linear() - 0.501).abs() < 0.01);
    }

    #[test]
    fn test_channel_volumes() {
        let vols = AtomicChannelVolumes::new(4);
        assert_eq!(vols.num_channels(), 4);

        for ch in 0..4 {
            assert!((vols.get_channel_linear(ch) - 1.0).abs() < 0.001);
        }

        vols.set_channel_db(0, -6.0);
        vols.set_channel_db(1, -12.0);
        vols.set_channel_db(2, 0.0);
        vols.set_channel_db(3, 3.0);

        assert!((vols.get_channel_db(0) - (-6.0)).abs() < 0.001);
        assert!((vols.get_channel_db(1) - (-12.0)).abs() < 0.001);
        assert!((vols.get_channel_db(2) - 0.0).abs() < 0.001);
        assert!((vols.get_channel_db(3) - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_channel_volumes_apply() {
        let vols = AtomicChannelVolumes::with_db(&[0.0, -6.0]); // L: unity, R: half

        // Create stereo buffer: [L0, R0, L1, R1]
        let mut buffer = vec![1.0, 1.0, 1.0, 1.0];
        vols.apply_to_buffer(&mut buffer, 2);

        // Left should be unchanged, right should be ~0.5
        assert!((buffer[0] - 1.0).abs() < 0.001);
        assert!((buffer[1] - 0.501).abs() < 0.01);
        assert!((buffer[2] - 1.0).abs() < 0.001);
        assert!((buffer[3] - 0.501).abs() < 0.01);
    }

    #[test]
    fn test_channel_volumes_mute() {
        let vols = AtomicChannelVolumes::with_db(&[0.0, -6.0]);

        let mut buffer = vec![1.0, 1.0, 1.0, 1.0];
        vols.set_muted(true);
        vols.apply_to_buffer(&mut buffer, 2);

        // All should be zero when muted
        for &sample in &buffer {
            assert!((sample - 0.0).abs() < 0.001);
        }
    }

    #[test]
    fn test_shared_volume() {
        let vol = create_shared_volume();
        let vol2 = vol.clone();

        vol.set_db(-6.0);
        assert!((vol2.get_db() - (-6.0)).abs() < 0.001);

        vol2.set_db(-12.0);
        assert!((vol.get_db() - (-12.0)).abs() < 0.001);
    }

    #[test]
    fn test_multichannel_volumes() {
        // Test 6-channel (5.1) configuration
        let vols = AtomicChannelVolumes::new(6);

        // Set different volumes: FL, FR, C, LFE, SL, SR
        vols.set_channel_db(0, 0.0); // FL: unity
        vols.set_channel_db(1, 0.0); // FR: unity
        vols.set_channel_db(2, -3.0); // C: -3dB
        vols.set_channel_db(3, -10.0); // LFE: -10dB
        vols.set_channel_db(4, -6.0); // SL: -6dB
        vols.set_channel_db(5, -6.0); // SR: -6dB

        let mut buffer: Vec<f32> = (0..6).map(|_| 1.0).collect();
        vols.apply_to_buffer(&mut buffer, 1);

        assert!((buffer[0] - 1.0).abs() < 0.001); // FL
        assert!((buffer[1] - 1.0).abs() < 0.001); // FR
        assert!((buffer[2] - 0.708).abs() < 0.01); // C at -3dB
        assert!((buffer[3] - 0.316).abs() < 0.01); // LFE at -10dB
        assert!((buffer[4] - 0.501).abs() < 0.01); // SL at -6dB
        assert!((buffer[5] - 0.501).abs() < 0.01); // SR at -6dB
    }

    // ==========================================================================
    // Volume Validation Tests - catch out-of-range values and clamping issues
    // ==========================================================================

    #[test]
    fn test_clamp_volume_db_within_range() {
        // Values within range should pass through unchanged
        assert!((clamp_volume_db(0.0) - 0.0).abs() < 0.001);
        assert!((clamp_volume_db(-6.0) - (-6.0)).abs() < 0.001);
        assert!((clamp_volume_db(10.0) - 10.0).abs() < 0.001);
        assert!((clamp_volume_db(-60.0) - (-60.0)).abs() < 0.001);
        assert!((clamp_volume_db(20.0) - 20.0).abs() < 0.001);
    }

    #[test]
    fn test_clamp_volume_db_out_of_range() {
        // Values below minimum should clamp to -60
        assert!((clamp_volume_db(-100.0) - (-60.0)).abs() < 0.001);
        assert!((clamp_volume_db(-1000.0) - (-60.0)).abs() < 0.001);
        assert!((clamp_volume_db(f32::NEG_INFINITY) - (-60.0)).abs() < 0.001);

        // Values above maximum should clamp to +20
        assert!((clamp_volume_db(50.0) - 20.0).abs() < 0.001);
        assert!((clamp_volume_db(100.0) - 20.0).abs() < 0.001);
        // Note: f32::INFINITY.clamp() returns the max, which is correct
        assert!((clamp_volume_db(f32::INFINITY) - 20.0).abs() < 0.001);
    }

    #[test]
    fn test_clamp_volume_linear_within_range() {
        // Values within range should pass through unchanged
        assert!((clamp_volume_linear(0.0) - 0.0).abs() < 0.001);
        assert!((clamp_volume_linear(0.5) - 0.5).abs() < 0.001);
        assert!((clamp_volume_linear(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_clamp_volume_linear_out_of_range() {
        // Negative values should clamp to 0
        assert!((clamp_volume_linear(-1.0) - 0.0).abs() < 0.001);
        assert!((clamp_volume_linear(-100.0) - 0.0).abs() < 0.001);

        // Values above +20dB (10.0 linear) should clamp
        let max_linear = db_to_linear(20.0);
        assert!((clamp_volume_linear(100.0) - max_linear).abs() < 0.001);
    }

    #[test]
    fn test_atomic_volume_clamps_extreme_values() {
        let vol = AtomicVolume::new();

        // Test extremely low dB value gets clamped
        vol.set_db(-200.0);
        assert!(
            vol.get_db() >= -60.0,
            "Volume should be clamped to -60dB minimum"
        );
        assert!(
            vol.get_db() <= -59.9,
            "Volume should be at -60dB after clamping"
        );

        // Test extremely high dB value gets clamped
        vol.set_db(100.0);
        assert!(
            vol.get_db() <= 20.0,
            "Volume should be clamped to +20dB maximum"
        );
        assert!(
            vol.get_db() >= 19.9,
            "Volume should be at +20dB after clamping"
        );

        // Test infinity gets clamped
        vol.set_db(f32::INFINITY);
        assert!(vol.get_db() <= 20.0, "Infinity should be clamped to +20dB");
        assert!(!vol.get_db().is_infinite(), "Volume should not be infinite");
    }

    #[test]
    fn test_channel_volumes_clamps_extreme_values() {
        let vols = AtomicChannelVolumes::new(2);

        // Test extremely low dB value gets clamped
        vols.set_channel_db(0, -200.0);
        assert!(
            vols.get_channel_db(0) >= -60.0,
            "Channel volume should be clamped to -60dB"
        );

        // Test extremely high dB value gets clamped
        vols.set_channel_db(1, 100.0);
        assert!(
            vols.get_channel_db(1) <= 20.0,
            "Channel volume should be clamped to +20dB"
        );
    }

    #[test]
    fn test_channel_volumes_with_db_clamps_input() {
        // Initialize with extreme values - they should all be clamped
        let vols = AtomicChannelVolumes::with_db(&[-200.0, 100.0, 0.0]);

        assert!(
            vols.get_channel_db(0) >= -60.0,
            "Initial value should be clamped to -60dB"
        );
        assert!(
            vols.get_channel_db(1) <= 20.0,
            "Initial value should be clamped to +20dB"
        );
        assert!(
            (vols.get_channel_db(2) - 0.0).abs() < 0.001,
            "Normal value should pass through"
        );
    }

    #[test]
    fn test_set_all_db_clamps_value() {
        let vols = AtomicChannelVolumes::new(4);

        // Set all to extreme high value
        vols.set_all_db(100.0);
        for ch in 0..4 {
            assert!(
                vols.get_channel_db(ch) <= 20.0,
                "All channels should be clamped to +20dB"
            );
        }

        // Set all to extreme low value
        vols.set_all_db(-200.0);
        for ch in 0..4 {
            assert!(
                vols.get_channel_db(ch) >= -60.0,
                "All channels should be clamped to -60dB"
            );
        }
    }

    #[test]
    fn test_volume_with_nan_input() {
        let vol = AtomicVolume::new();

        // NaN should be handled gracefully (clamp returns NaN for NaN input)
        // This test documents current behavior - NaN passes through
        vol.set_db(f32::NAN);
        // We just verify it doesn't crash and the value is retrievable
        let _ = vol.get_db();
        let _ = vol.get_linear();
    }

    #[test]
    fn test_channel_out_of_bounds_access() {
        let vols = AtomicChannelVolumes::new(2);

        // Setting out-of-bounds channel should be no-op (not panic)
        vols.set_channel_db(99, -6.0); // Should not panic

        // Getting out-of-bounds channel should return unity gain
        assert!((vols.get_channel_linear(99) - 1.0).abs() < 0.001);
        assert!((vols.get_channel_db(99) - 0.0).abs() < 0.001);
    }
}
