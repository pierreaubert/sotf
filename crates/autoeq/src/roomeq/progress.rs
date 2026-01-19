//! Progress reporting for long-running optimizations.
//!
//! Provides progress tracking and ETA estimation for optimization operations.

#![allow(dead_code)]

use std::time::{Duration, Instant};

/// Progress reporter for optimization operations.
///
/// Tracks elapsed time, reports progress at configurable intervals,
/// and estimates time remaining based on current progress.
pub struct ProgressReporter {
    /// When the operation started
    start_time: Instant,
    /// Name of the operation (for display)
    name: String,
    /// Total expected iterations
    total_iterations: usize,
    /// How often to report progress
    report_interval: Duration,
    /// When we last reported
    last_report: Instant,
    /// Current iteration
    current_iteration: usize,
    /// Best loss seen so far
    best_loss: f64,
    /// Whether to print to stderr
    verbose: bool,
}

impl ProgressReporter {
    /// Create a new progress reporter
    ///
    /// # Arguments
    /// * `name` - Name of the operation (e.g., "Left Channel EQ")
    /// * `total_iterations` - Expected total iterations
    pub fn new(name: impl Into<String>, total_iterations: usize) -> Self {
        Self {
            start_time: Instant::now(),
            name: name.into(),
            total_iterations,
            report_interval: Duration::from_secs(5),
            last_report: Instant::now(),
            current_iteration: 0,
            best_loss: f64::INFINITY,
            verbose: true,
        }
    }

    /// Set the report interval
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.report_interval = interval;
        self
    }

    /// Set whether to print progress
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Report progress for current iteration.
    ///
    /// Only prints if enough time has passed since last report.
    pub fn report(&mut self, current_iter: usize, current_loss: f64) {
        self.current_iteration = current_iter;
        if current_loss < self.best_loss {
            self.best_loss = current_loss;
        }

        if !self.verbose {
            return;
        }

        let now = Instant::now();

        if now.duration_since(self.last_report) >= self.report_interval {
            let elapsed = now.duration_since(self.start_time);
            let progress = current_iter as f64 / self.total_iterations as f64;

            let eta = if progress > 0.01 && current_iter > 0 {
                let per_iter = elapsed.as_secs_f64() / current_iter as f64;
                let remaining = self.total_iterations.saturating_sub(current_iter);
                Duration::from_secs_f64(per_iter * remaining as f64)
            } else {
                Duration::from_secs(0)
            };

            eprintln!(
                "[{}] {:3.1}% | iter {}/{} | loss: {:.6} (best: {:.6}) | elapsed: {:.1}s | ETA: {:.1}s",
                self.name,
                progress * 100.0,
                current_iter,
                self.total_iterations,
                current_loss,
                self.best_loss,
                elapsed.as_secs_f64(),
                eta.as_secs_f64()
            );

            self.last_report = now;
        }
    }

    /// Force a progress report regardless of interval
    pub fn force_report(&mut self, current_iter: usize, current_loss: f64) {
        self.current_iteration = current_iter;
        if current_loss < self.best_loss {
            self.best_loss = current_loss;
        }

        if !self.verbose {
            return;
        }

        let elapsed = self.start_time.elapsed();
        let progress = current_iter as f64 / self.total_iterations as f64;

        let eta = if progress > 0.01 && current_iter > 0 {
            let per_iter = elapsed.as_secs_f64() / current_iter as f64;
            let remaining = self.total_iterations.saturating_sub(current_iter);
            Duration::from_secs_f64(per_iter * remaining as f64)
        } else {
            Duration::from_secs(0)
        };

        eprintln!(
            "[{}] {:3.1}% | iter {}/{} | loss: {:.6} (best: {:.6}) | elapsed: {:.1}s | ETA: {:.1}s",
            self.name,
            progress * 100.0,
            current_iter,
            self.total_iterations,
            current_loss,
            self.best_loss,
            elapsed.as_secs_f64(),
            eta.as_secs_f64()
        );

        self.last_report = Instant::now();
    }

    /// Final report with total statistics.
    pub fn finish(&self, final_loss: f64) {
        if !self.verbose {
            return;
        }

        let elapsed = self.start_time.elapsed();
        let improvement = if self.best_loss < f64::INFINITY && self.best_loss > 0.0 {
            let imp = (1.0 - final_loss / self.best_loss) * 100.0;
            format!(" ({:+.1}% from initial)", imp)
        } else {
            String::new()
        };

        eprintln!(
            "[{}] Complete | final loss: {:.6}{} | total time: {:.1}s | iters: {}",
            self.name,
            final_loss,
            improvement,
            elapsed.as_secs_f64(),
            self.current_iteration
        );
    }

    /// Get elapsed time since start
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get the best loss seen so far
    pub fn best_loss(&self) -> f64 {
        self.best_loss
    }

    /// Get current iteration
    pub fn current_iteration(&self) -> usize {
        self.current_iteration
    }
}

/// Multi-stage progress reporter for operations with multiple phases
pub struct MultiStageProgress {
    /// Overall operation name
    name: String,
    /// Stage names and weights (for ETA calculation)
    stages: Vec<(String, f64)>,
    /// Current stage index
    current_stage: usize,
    /// Overall start time
    start_time: Instant,
    /// Whether to print
    verbose: bool,
}

impl MultiStageProgress {
    /// Create a new multi-stage progress reporter
    pub fn new(name: impl Into<String>, stages: Vec<(String, f64)>) -> Self {
        Self {
            name: name.into(),
            stages,
            current_stage: 0,
            start_time: Instant::now(),
            verbose: true,
        }
    }

    /// Set verbose mode
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Start a new stage
    pub fn start_stage(&mut self, stage_name: &str) {
        // Find the stage and update state (always, regardless of verbose)
        for (i, (name, _)) in self.stages.iter().enumerate() {
            if name == stage_name {
                self.current_stage = i;
                break;
            }
        }

        if !self.verbose {
            return;
        }

        eprintln!(
            "[{}] Stage {}/{}: {}",
            self.name,
            self.current_stage + 1,
            self.stages.len(),
            stage_name
        );
    }

    /// Report completion of current stage
    pub fn complete_stage(&mut self) {
        if self.current_stage >= self.stages.len() {
            return;
        }

        let stage_name = self.stages[self.current_stage].0.clone();

        // Increment stage counter (always, regardless of verbose)
        self.current_stage += 1;

        if !self.verbose {
            return;
        }

        let elapsed = self.start_time.elapsed();
        eprintln!(
            "[{}] Stage '{}' complete | elapsed: {:.1}s",
            self.name,
            stage_name,
            elapsed.as_secs_f64()
        );
    }

    /// Report overall completion
    pub fn finish(&self) {
        if !self.verbose {
            return;
        }

        let elapsed = self.start_time.elapsed();
        eprintln!(
            "[{}] All stages complete | total time: {:.1}s",
            self.name,
            elapsed.as_secs_f64()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_reporter_new() {
        let reporter = ProgressReporter::new("Test", 1000);
        assert_eq!(reporter.total_iterations, 1000);
        assert_eq!(reporter.best_loss, f64::INFINITY);
    }

    #[test]
    fn test_progress_reporter_tracks_best_loss() {
        let mut reporter = ProgressReporter::new("Test", 100).with_verbose(false);

        reporter.report(10, 5.0);
        assert_eq!(reporter.best_loss(), 5.0);

        reporter.report(20, 3.0);
        assert_eq!(reporter.best_loss(), 3.0);

        // Best shouldn't increase
        reporter.report(30, 4.0);
        assert_eq!(reporter.best_loss(), 3.0);
    }

    #[test]
    fn test_progress_reporter_elapsed() {
        let reporter = ProgressReporter::new("Test", 100);
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(reporter.elapsed().as_millis() >= 10);
    }

    #[test]
    fn test_multi_stage_progress() {
        let stages = vec![
            ("Load".to_string(), 0.1),
            ("Optimize".to_string(), 0.8),
            ("Save".to_string(), 0.1),
        ];

        let mut progress = MultiStageProgress::new("Test", stages).with_verbose(false);

        progress.start_stage("Load");
        assert_eq!(progress.current_stage, 0);

        progress.complete_stage();
        progress.start_stage("Optimize");
        assert_eq!(progress.current_stage, 1);
    }
}
