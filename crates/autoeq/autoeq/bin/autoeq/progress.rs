//! Progress reporting for long-running optimizations.

#![allow(dead_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Progress reporter for optimization
pub struct ProgressReporter {
    start_time: Instant,
    name: String,
    total_iterations: usize,
    report_interval: Duration,
    last_report: Instant,
    current_iteration: Arc<AtomicUsize>,
}

impl ProgressReporter {
    /// Create a new progress reporter
    pub fn new(name: String, total_iterations: usize) -> Self {
        Self {
            start_time: Instant::now(),
            name,
            total_iterations,
            report_interval: Duration::from_secs(2),
            last_report: Instant::now(),
            current_iteration: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Get a clone of the iteration counter for use in optimizer
    pub fn iteration_counter(&self) -> Arc<AtomicUsize> {
        self.current_iteration.clone()
    }

    /// Report current progress
    pub fn report(&mut self, current_loss: f64) {
        let now = Instant::now();

        if now.duration_since(self.last_report) >= self.report_interval {
            let iter = self.current_iteration.load(Ordering::Relaxed);
            let progress = if self.total_iterations > 0 {
                iter as f64 / self.total_iterations as f64
            } else {
                0.0
            };

            let elapsed = now.duration_since(self.start_time);
            let eta = if progress > 0.01 {
                let per_iter = elapsed.as_secs_f64() / iter.max(1) as f64;
                let remaining = self.total_iterations.saturating_sub(iter);
                Duration::from_secs_f64(per_iter * remaining as f64)
            } else {
                Duration::from_secs(0)
            };

            eprintln!(
                "[{}] {:3.1}% | iter {}/{} | loss: {:.6} | elapsed: {:.1}s | ETA: {:.1}s",
                self.name,
                progress * 100.0,
                iter,
                self.total_iterations,
                current_loss,
                elapsed.as_secs_f64(),
                eta.as_secs_f64()
            );

            self.last_report = now;
        }
    }

    /// Final report
    pub fn finish(&self, final_loss: f64, converged: bool) {
        let elapsed = self.start_time.elapsed();
        let iter = self.current_iteration.load(Ordering::Relaxed);

        eprintln!(
            "[{}] Complete | iter: {} | final loss: {:.6} | converged: {} | time: {:.1}s",
            self.name,
            iter,
            final_loss,
            if converged { "Yes" } else { "No" },
            elapsed.as_secs_f64()
        );
    }
}
