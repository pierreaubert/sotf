//! Shared callback utilities for optimization progress tracking
//!
//! This module provides common callback functionality used by different
//! optimization backends (DE, metaheuristics).

use crate::param_utils;

/// Tracks optimization progress and detects stalls.
pub struct ProgressTracker {
    /// Best fitness value seen so far
    last_fitness: f64,
    /// Number of iterations without improvement
    stall_count: usize,
    /// Threshold for reporting stall warnings
    stall_threshold: usize,
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self::new(50)
    }
}

impl ProgressTracker {
    /// Create a new progress tracker with the specified stall threshold.
    pub fn new(stall_threshold: usize) -> Self {
        Self {
            last_fitness: f64::INFINITY,
            stall_count: 0,
            stall_threshold,
        }
    }

    /// Update tracker with new fitness value and return improvement info.
    ///
    /// Returns a tuple of (improvement_string, is_stalling).
    pub fn update(&mut self, fitness: f64) -> (String, bool) {
        if fitness < self.last_fitness {
            let delta = self.last_fitness - fitness;
            self.last_fitness = fitness;
            self.stall_count = 0;
            (format!("(-{:.2e})", delta), false)
        } else {
            self.stall_count += 1;
            let is_stalling = self.stall_count >= self.stall_threshold;
            let msg = if is_stalling {
                format!("(STALL:{})", self.stall_count)
            } else {
                "(--) ".to_string()
            };
            (msg, is_stalling)
        }
    }

    /// Check if we just started stalling (stall_count == 1)
    pub fn just_started_stalling(&self) -> bool {
        self.stall_count == 1
    }

    /// Check if stall count is a multiple of the given interval
    pub fn stall_at_interval(&self, interval: usize) -> bool {
        self.stall_count > 0 && self.stall_count.is_multiple_of(interval)
    }
}

/// Format parameter summary for progress display.
///
/// Assumes 3 parameters per filter (freq, Q, gain) which is the common case.
/// For models with 4 parameters per filter, the display will be approximate.
pub fn format_param_summary(params: &[f64], params_per_filter: usize) -> String {
    let num_filters = params.len() / params_per_filter;
    let summaries: Vec<String> = (0..num_filters)
        .map(|i| {
            let offset = i * params_per_filter;
            // For 4-param models, freq is at offset+1; for 3-param, it's at offset
            let freq_idx = if params_per_filter == 4 {
                offset + 1
            } else {
                offset
            };
            let freq = param_utils::freq_from_log10(params[freq_idx]);
            let q = params[freq_idx + 1];
            let gain = params[freq_idx + 2];
            format!("[f{:.0}Hz Q{:.2} G{:.2}dB]", freq, q, gain)
        })
        .collect();
    summaries.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_tracker_improvement() {
        let mut tracker = ProgressTracker::new(50);

        // First update should show improvement from infinity
        let (msg, stalling) = tracker.update(100.0);
        assert!(msg.contains("-"), "Should show improvement");
        assert!(!stalling);

        // Better value should show improvement
        let (msg, stalling) = tracker.update(50.0);
        assert!(msg.contains("-"), "Should show improvement");
        assert!(!stalling);
    }

    #[test]
    fn test_progress_tracker_stall() {
        let mut tracker = ProgressTracker::new(3);

        tracker.update(100.0);

        // Same or worse value should increment stall
        let (_, stalling) = tracker.update(100.0);
        assert!(!stalling, "Not stalling yet");
        assert!(tracker.just_started_stalling());

        let (_, stalling) = tracker.update(101.0);
        assert!(!stalling, "Not at threshold yet");

        let (msg, stalling) = tracker.update(102.0);
        assert!(stalling, "Should be stalling now");
        assert!(msg.contains("STALL"));
    }

    #[test]
    fn test_format_param_summary() {
        // 3 params per filter: freq (log10), Q, gain
        let params = vec![
            3.0, 1.0, -3.0, // 1000 Hz, Q=1.0, -3dB
            3.301, 2.0, 2.0, // ~2000 Hz, Q=2.0, +2dB
        ];
        let summary = format_param_summary(&params, 3);
        assert!(summary.contains("f1000Hz"));
        assert!(summary.contains("Q1.00"));
        assert!(summary.contains("G-3.00dB"));
    }
}
