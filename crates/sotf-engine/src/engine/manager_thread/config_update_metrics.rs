/// Metrics for config update operations
#[derive(Default, Debug, Clone)]
pub(super) struct ConfigUpdateMetrics {
    /// Total number of update attempts
    pub(super) total_updates: u64,
    /// Number of successful updates
    pub(super) successful_updates: u64,
    /// Number of failed updates
    pub(super) failed_updates: u64,
    /// Number of updates rejected (validation or queue full)
    pub(super) rejected_updates: u64,
    /// Number of rollbacks attempted
    pub(super) rollback_attempts: u64,
    /// Number of successful rollbacks
    pub(super) successful_rollbacks: u64,
    /// Total time spent on updates (milliseconds)
    pub(super) total_update_time_ms: u64,
    /// Maximum queue depth observed
    pub(super) max_queue_depth: usize,
    /// Last update timestamp
    pub(super) last_update_time: Option<std::time::Instant>,
}

impl ConfigUpdateMetrics {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn record_success(&mut self, duration: std::time::Duration) {
        self.total_updates += 1;
        self.successful_updates += 1;
        self.total_update_time_ms += duration.as_millis() as u64;
        self.last_update_time = Some(std::time::Instant::now());
    }

    pub(super) fn record_failure(&mut self) {
        self.total_updates += 1;
        self.failed_updates += 1;
    }

    pub(super) fn record_rejection(&mut self) {
        self.rejected_updates += 1;
    }

    pub(super) fn update_queue_depth(&mut self, depth: usize) {
        self.max_queue_depth = self.max_queue_depth.max(depth);
    }

    pub(super) fn success_rate(&self) -> f64 {
        if self.total_updates == 0 {
            return 1.0;
        }
        self.successful_updates as f64 / self.total_updates as f64
    }

    pub(super) fn avg_update_time_ms(&self) -> f64 {
        if self.successful_updates == 0 {
            return 0.0;
        }
        self.total_update_time_ms as f64 / self.successful_updates as f64
    }
}
