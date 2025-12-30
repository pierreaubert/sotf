// ============================================================================
// Audio Engine Integration
// ============================================================================
//
// Provides integration helpers for connecting head tracking to audio plugins,
// specifically the XTC (Crosstalk Cancellation) plugin.

use crate::types::HeadPosition;
use log::{debug, trace};

/// Parameters to send to the XTC plugin
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XtcHeadParams {
    /// Lateral head offset in meters (-0.5 to 0.5)
    pub head_offset_x: f32,
    /// Depth head offset in meters (-0.5 to 0.5)
    pub head_offset_z: f32,
}

impl Default for XtcHeadParams {
    fn default() -> Self {
        Self {
            head_offset_x: 0.0,
            head_offset_z: 0.0,
        }
    }
}

impl XtcHeadParams {
    /// Create from a head position
    pub fn from_head_position(pos: &HeadPosition) -> Self {
        Self {
            head_offset_x: pos.x.clamp(-0.5, 0.5),
            head_offset_z: pos.z.clamp(-0.5, 0.5),
        }
    }

    /// Check if params differ significantly from other params
    pub fn significantly_different(&self, other: &Self, threshold_m: f32) -> bool {
        (self.head_offset_x - other.head_offset_x).abs() > threshold_m
            || (self.head_offset_z - other.head_offset_z).abs() > threshold_m
    }
}

/// Trait for applying head tracking to an audio engine
///
/// This trait allows different audio engine implementations to receive
/// head tracking updates.
pub trait HeadTrackingTarget {
    /// Update XTC plugin parameters with new head position
    fn update_xtc_head_params(
        &mut self,
        plugin_index: usize,
        params: &XtcHeadParams,
    ) -> Result<(), String>;
}

/// Bridge between head tracker and audio engine
///
/// Handles the conversion of head positions to plugin parameters
/// and provides change detection to avoid unnecessary updates.
#[derive(Debug)]
pub struct HeadTrackingBridge {
    /// XTC plugin index in the audio engine
    plugin_index: usize,
    /// Last sent parameters (for change detection)
    last_params: XtcHeadParams,
    /// Minimum position change to trigger update (meters)
    update_threshold_m: f32,
    /// Counter for updates sent
    updates_sent: u64,
}

impl HeadTrackingBridge {
    /// Create a new bridge for the given XTC plugin index
    pub fn new(plugin_index: usize) -> Self {
        Self {
            plugin_index,
            last_params: XtcHeadParams::default(),
            update_threshold_m: 0.01, // 1cm default threshold
            updates_sent: 0,
        }
    }

    /// Set the minimum position change threshold for updates
    pub fn with_threshold(mut self, threshold_m: f32) -> Self {
        self.update_threshold_m = threshold_m.max(0.001);
        self
    }

    /// Get the plugin index
    pub fn plugin_index(&self) -> usize {
        self.plugin_index
    }

    /// Get the number of updates sent
    pub fn updates_sent(&self) -> u64 {
        self.updates_sent
    }

    /// Update the audio engine with a new head position
    ///
    /// Only sends updates if the position changed significantly.
    /// Returns true if an update was sent.
    pub fn update<T: HeadTrackingTarget>(
        &mut self,
        target: &mut T,
        position: &HeadPosition,
    ) -> Result<bool, String> {
        // Skip low-confidence positions
        if position.confidence < 0.3 {
            trace!(
                "Skipping low-confidence position (conf={:.2})",
                position.confidence
            );
            return Ok(false);
        }

        let new_params = XtcHeadParams::from_head_position(position);

        // Check if update is needed
        if !new_params.significantly_different(&self.last_params, self.update_threshold_m) {
            trace!("Position unchanged, skipping update");
            return Ok(false);
        }

        // Send update
        debug!(
            "Updating XTC: head_offset_x={:.3}m, head_offset_z={:.3}m",
            new_params.head_offset_x, new_params.head_offset_z
        );

        target.update_xtc_head_params(self.plugin_index, &new_params)?;

        self.last_params = new_params;
        self.updates_sent += 1;

        Ok(true)
    }

    /// Force update regardless of change threshold
    pub fn force_update<T: HeadTrackingTarget>(
        &mut self,
        target: &mut T,
        position: &HeadPosition,
    ) -> Result<(), String> {
        let new_params = XtcHeadParams::from_head_position(position);
        target.update_xtc_head_params(self.plugin_index, &new_params)?;
        self.last_params = new_params;
        self.updates_sent += 1;
        Ok(())
    }

    /// Reset to center position
    pub fn reset<T: HeadTrackingTarget>(&mut self, target: &mut T) -> Result<(), String> {
        let center = XtcHeadParams::default();
        target.update_xtc_head_params(self.plugin_index, &center)?;
        self.last_params = center;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockTarget {
        last_x: f32,
        last_z: f32,
        call_count: usize,
    }

    impl HeadTrackingTarget for MockTarget {
        fn update_xtc_head_params(
            &mut self,
            _plugin_index: usize,
            params: &XtcHeadParams,
        ) -> Result<(), String> {
            self.last_x = params.head_offset_x;
            self.last_z = params.head_offset_z;
            self.call_count += 1;
            Ok(())
        }
    }

    #[test]
    fn test_bridge_filters_small_changes() {
        let mut bridge = HeadTrackingBridge::new(0).with_threshold(0.01);
        let mut target = MockTarget {
            last_x: 0.0,
            last_z: 0.0,
            call_count: 0,
        };

        // First update should always go through
        let pos1 = HeadPosition {
            x: 0.05,
            z: 0.0,
            confidence: 0.9,
            ..Default::default()
        };
        assert!(bridge.update(&mut target, &pos1).unwrap());
        assert_eq!(target.call_count, 1);

        // Small change should be filtered
        let pos2 = HeadPosition {
            x: 0.055,
            z: 0.003,
            confidence: 0.9,
            ..Default::default()
        };
        assert!(!bridge.update(&mut target, &pos2).unwrap());
        assert_eq!(target.call_count, 1);

        // Large change should go through
        let pos3 = HeadPosition {
            x: 0.10,
            z: 0.05,
            confidence: 0.9,
            ..Default::default()
        };
        assert!(bridge.update(&mut target, &pos3).unwrap());
        assert_eq!(target.call_count, 2);
    }

    #[test]
    fn test_bridge_filters_low_confidence() {
        let mut bridge = HeadTrackingBridge::new(0);
        let mut target = MockTarget {
            last_x: 0.0,
            last_z: 0.0,
            call_count: 0,
        };

        let low_conf = HeadPosition {
            x: 0.20,
            z: 0.10,
            confidence: 0.1, // Too low
            ..Default::default()
        };
        assert!(!bridge.update(&mut target, &low_conf).unwrap());
        assert_eq!(target.call_count, 0);
    }

    #[test]
    fn test_xtc_params_clamping() {
        let pos = HeadPosition {
            x: 1.0,  // Out of range
            z: -1.0, // Out of range
            confidence: 0.9,
            ..Default::default()
        };
        let params = XtcHeadParams::from_head_position(&pos);
        assert_eq!(params.head_offset_x, 0.5);
        assert_eq!(params.head_offset_z, -0.5);
    }
}
