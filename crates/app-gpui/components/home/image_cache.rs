//! Image Cache for Album Art
//!
//! Provides efficient LRU-based tracking of album art access patterns
//! to optimize image loading when scrolling through large music libraries.
//!
//! Note: GPUI handles the actual image caching internally. This module
//! provides LRU tracking and preloading hints for better performance.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Maximum number of paths to track in the LRU cache
const MAX_CACHE_SIZE: usize = 200;

/// LRU tracker for album art image paths
///
/// This tracker monitors which album art paths have been accessed recently
/// to help with preloading decisions and cache eviction policies.
pub struct ImageAccessTracker {
    /// Paths that have been accessed, with access count
    access_counts: HashMap<PathBuf, u32>,
    /// LRU tracking - paths in order of recent access
    lru_order: Vec<PathBuf>,
    /// Maximum cache size
    max_size: usize,
    /// Paths that have been preloaded
    preloaded: HashSet<PathBuf>,
}

impl Default for ImageAccessTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageAccessTracker {
    /// Create a new empty tracker
    pub fn new() -> Self {
        Self {
            access_counts: HashMap::new(),
            lru_order: Vec::new(),
            max_size: MAX_CACHE_SIZE,
            preloaded: HashSet::new(),
        }
    }

    /// Create a tracker with custom size limit
    pub fn with_capacity(max_size: usize) -> Self {
        Self {
            access_counts: HashMap::new(),
            lru_order: Vec::new(),
            max_size,
            preloaded: HashSet::new(),
        }
    }

    /// Record an access to an image path
    pub fn record_access(&mut self, path: &Path) {
        let path_buf = path.to_path_buf();

        // Update access count
        *self.access_counts.entry(path_buf.clone()).or_insert(0) += 1;

        // Update LRU order
        self.touch_lru(&path_buf);

        // Evict if at capacity
        self.evict_if_needed();
    }

    /// Check if a path has been accessed recently
    pub fn was_accessed(&self, path: &Path) -> bool {
        self.access_counts.contains_key(path)
    }

    /// Get the access count for a path
    pub fn access_count(&self, path: &Path) -> u32 {
        self.access_counts.get(path).copied().unwrap_or(0)
    }

    /// Check if a path has been preloaded
    pub fn is_preloaded(&self, path: &Path) -> bool {
        self.preloaded.contains(path)
    }

    /// Mark a path as preloaded
    pub fn mark_preloaded(&mut self, path: &Path) {
        self.preloaded.insert(path.to_path_buf());
    }

    /// Get paths that should be preloaded based on access patterns
    ///
    /// Returns paths that haven't been preloaded yet but are likely to be needed soon.
    pub fn get_preload_candidates(&self, paths: &[PathBuf], count: usize) -> Vec<PathBuf> {
        paths
            .iter()
            .filter(|p| !self.preloaded.contains(*p))
            .take(count)
            .cloned()
            .collect()
    }

    /// Clear tracking data for paths no longer needed
    pub fn clear_stale(&mut self, keep_paths: &HashSet<PathBuf>) {
        self.access_counts.retain(|p, _| keep_paths.contains(p));
        self.lru_order.retain(|p| keep_paths.contains(p));
        self.preloaded.retain(|p| keep_paths.contains(p));
    }

    /// Get cache statistics
    pub fn stats(&self) -> TrackerStats {
        TrackerStats {
            tracked: self.access_counts.len(),
            preloaded: self.preloaded.len(),
            capacity: self.max_size,
        }
    }

    /// Clear all tracking data
    pub fn clear(&mut self) {
        self.access_counts.clear();
        self.lru_order.clear();
        self.preloaded.clear();
    }

    /// Get the most recently accessed paths
    pub fn recent_paths(&self, count: usize) -> Vec<&PathBuf> {
        self.lru_order.iter().rev().take(count).collect()
    }

    /// Update LRU order for a path
    fn touch_lru(&mut self, path: &PathBuf) {
        // Remove existing entry if present
        if let Some(pos) = self.lru_order.iter().position(|p| p == path) {
            self.lru_order.remove(pos);
        }
        // Add to end (most recent)
        self.lru_order.push(path.clone());
    }

    /// Evict least recently used entries if over capacity
    fn evict_if_needed(&mut self) {
        while self.lru_order.len() > self.max_size {
            if let Some(evict_path) = self.lru_order.first().cloned() {
                self.lru_order.remove(0);
                self.access_counts.remove(&evict_path);
                self.preloaded.remove(&evict_path);
            }
        }
    }
}

/// Tracker statistics
#[derive(Debug, Clone, Copy)]
pub struct TrackerStats {
    /// Number of tracked paths
    pub tracked: usize,
    /// Number of preloaded paths
    pub preloaded: usize,
    /// Maximum capacity
    pub capacity: usize,
}

impl TrackerStats {
    /// Get tracker utilization as percentage
    pub fn utilization(&self) -> f32 {
        if self.capacity > 0 {
            self.tracked as f32 / self.capacity as f32 * 100.0
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracker_creation() {
        let tracker = ImageAccessTracker::new();
        assert_eq!(tracker.stats().tracked, 0);
        assert_eq!(tracker.stats().capacity, MAX_CACHE_SIZE);
    }

    #[test]
    fn test_tracker_with_custom_capacity() {
        let tracker = ImageAccessTracker::with_capacity(50);
        assert_eq!(tracker.stats().capacity, 50);
    }

    #[test]
    fn test_tracker_record_and_check() {
        let mut tracker = ImageAccessTracker::new();
        let path = PathBuf::from("/test/image.jpg");

        tracker.record_access(&path);

        assert!(tracker.was_accessed(&path));
        assert_eq!(tracker.access_count(&path), 1);
    }

    #[test]
    fn test_tracker_eviction() {
        let mut tracker = ImageAccessTracker::with_capacity(3);

        for i in 0..5 {
            let path = PathBuf::from(format!("/test/image{}.jpg", i));
            tracker.record_access(&path);
        }

        // Only the last 3 should be tracked
        assert_eq!(tracker.stats().tracked, 3);

        // First two should be evicted
        assert!(!tracker.was_accessed(&PathBuf::from("/test/image0.jpg")));
        assert!(!tracker.was_accessed(&PathBuf::from("/test/image1.jpg")));

        // Last three should be present
        assert!(tracker.was_accessed(&PathBuf::from("/test/image2.jpg")));
        assert!(tracker.was_accessed(&PathBuf::from("/test/image3.jpg")));
        assert!(tracker.was_accessed(&PathBuf::from("/test/image4.jpg")));
    }

    #[test]
    fn test_tracker_lru_order() {
        let mut tracker = ImageAccessTracker::with_capacity(3);

        let path1 = PathBuf::from("/test/image1.jpg");
        let path2 = PathBuf::from("/test/image2.jpg");
        let path3 = PathBuf::from("/test/image3.jpg");
        let path4 = PathBuf::from("/test/image4.jpg");

        tracker.record_access(&path1);
        tracker.record_access(&path2);
        tracker.record_access(&path3);

        // Access path1 to make it recently used
        tracker.record_access(&path1);

        // Record path4, should evict path2 (least recently used)
        tracker.record_access(&path4);

        assert!(tracker.was_accessed(&path1)); // Recently accessed
        assert!(!tracker.was_accessed(&path2)); // Evicted
        assert!(tracker.was_accessed(&path3)); // Still present
        assert!(tracker.was_accessed(&path4)); // Newly added
    }

    #[test]
    fn test_tracker_clear() {
        let mut tracker = ImageAccessTracker::new();

        tracker.record_access(&PathBuf::from("/test/image1.jpg"));
        tracker.record_access(&PathBuf::from("/test/image2.jpg"));

        assert_eq!(tracker.stats().tracked, 2);

        tracker.clear();

        assert_eq!(tracker.stats().tracked, 0);
    }

    #[test]
    fn test_tracker_preload() {
        let mut tracker = ImageAccessTracker::new();
        let path = PathBuf::from("/test/image.jpg");

        assert!(!tracker.is_preloaded(&path));

        tracker.mark_preloaded(&path);

        assert!(tracker.is_preloaded(&path));
    }

    #[test]
    fn test_tracker_stats() {
        let tracker = ImageAccessTracker::with_capacity(100);
        let stats = tracker.stats();

        assert_eq!(stats.tracked, 0);
        assert_eq!(stats.capacity, 100);
        assert_eq!(stats.utilization(), 0.0);
    }

    #[test]
    fn test_tracker_recent_paths() {
        let mut tracker = ImageAccessTracker::new();

        tracker.record_access(&PathBuf::from("/test/image1.jpg"));
        tracker.record_access(&PathBuf::from("/test/image2.jpg"));
        tracker.record_access(&PathBuf::from("/test/image3.jpg"));

        let recent = tracker.recent_paths(2);
        assert_eq!(recent.len(), 2);
        // Most recent should be first (reversed order)
        assert_eq!(recent[0], &PathBuf::from("/test/image3.jpg"));
        assert_eq!(recent[1], &PathBuf::from("/test/image2.jpg"));
    }

    #[test]
    fn test_preload_candidates() {
        let mut tracker = ImageAccessTracker::new();

        let paths: Vec<PathBuf> = (0..5)
            .map(|i| PathBuf::from(format!("/test/image{}.jpg", i)))
            .collect();

        // Mark first two as preloaded
        tracker.mark_preloaded(&paths[0]);
        tracker.mark_preloaded(&paths[1]);

        // Should get next 3 as candidates (skipping preloaded ones)
        let candidates = tracker.get_preload_candidates(&paths, 3);
        assert_eq!(candidates.len(), 3);
        assert_eq!(candidates[0], paths[2]);
        assert_eq!(candidates[1], paths[3]);
        assert_eq!(candidates[2], paths[4]);
    }
}
