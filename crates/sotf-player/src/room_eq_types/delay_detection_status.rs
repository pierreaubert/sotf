/// Status of the tone-burst delay detection measurement.
///
/// The measurement runs on a background thread (kicked off from the UI).
/// `Running` carries the wall-clock start time in ms so the UI can
/// render a progress estimate as `elapsed / estimated_total` without
/// requiring the engine to surface a progress callback. The estimated
/// total is computed by the UI from `probe_duration_ms` and
/// `silence_duration_ms` × channel count.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum DelayDetectionStatus {
    #[default]
    Idle,
    Running {
        /// Milliseconds since the Unix epoch when the measurement was
        /// spawned. Used purely for elapsed-time computation; if the
        /// system clock jumps backward the progress bar may briefly
        /// misreport but nothing else depends on this value.
        started_at_ms: u64,
    },
    Complete,
    Failed(String),
}

impl DelayDetectionStatus {
    /// Estimated fraction of the measurement completed, in `0.0..=1.0`.
    ///
    /// Returns `None` when the status is not `Running` or the estimated
    /// duration is zero. Callers should render a fallback (e.g. an
    /// indeterminate spinner) in that case.
    pub fn progress(&self, estimated_total_ms: u64, now_ms: u64) -> Option<f32> {
        match self {
            Self::Running { started_at_ms } if estimated_total_ms > 0 => {
                let elapsed = now_ms.saturating_sub(*started_at_ms);
                Some((elapsed as f32 / estimated_total_ms as f32).clamp(0.0, 1.0))
            }
            _ => None,
        }
    }
}
