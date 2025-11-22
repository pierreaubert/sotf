//! System resource monitoring for BEM execution
//!
//! Monitors RAM and CPU usage to prevent system overload during
//! parallel BEM simulations.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// System resource information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemResources {
    /// Total system RAM (MB)
    pub total_ram_mb: f64,

    /// Available RAM (MB)
    pub available_ram_mb: f64,

    /// Used RAM (MB)
    pub used_ram_mb: f64,

    /// CPU usage (0.0 to 100.0)
    pub cpu_usage_percent: f64,

    /// Number of CPU cores
    pub num_cpus: usize,

    /// Load average (1 minute, if available)
    pub load_average: Option<f64>,
}

impl SystemResources {
    /// Get current system resources
    pub fn current() -> anyhow::Result<Self> {
        use sysinfo::{System, SystemExt};

        let mut sys = System::new_all();
        sys.refresh_all();

        let total_ram_mb = sys.total_memory() as f64 / 1024.0 / 1024.0;
        let used_ram_mb = sys.used_memory() as f64 / 1024.0 / 1024.0;
        let available_ram_mb = total_ram_mb - used_ram_mb;

        // Get CPU usage (requires refresh)
        std::thread::sleep(Duration::from_millis(200));
        sys.refresh_cpu();
        let cpu_usage_percent = sys.global_cpu_info().cpu_usage() as f64;

        // Get load average (Unix only)
        let load_average = sys.load_average().one;
        let load_average = if load_average > 0.0 {
            Some(load_average)
        } else {
            None
        };

        Ok(Self {
            total_ram_mb,
            available_ram_mb,
            used_ram_mb,
            cpu_usage_percent,
            num_cpus: num_cpus::get(),
            load_average,
        })
    }

    /// Get percentage of RAM used
    pub fn ram_usage_percent(&self) -> f64 {
        (self.used_ram_mb / self.total_ram_mb) * 100.0
    }

    /// Check if resources are available for task
    pub fn can_run_task(&self, required_ram_mb: f64, max_cpu_percent: f64) -> bool {
        self.available_ram_mb > required_ram_mb && self.cpu_usage_percent < max_cpu_percent
    }

    /// Print resource summary
    pub fn print_summary(&self) {
        println!("System Resources:");
        println!("  RAM: {:.1} / {:.1} MB ({:.1}% used)",
                 self.used_ram_mb, self.total_ram_mb, self.ram_usage_percent());
        println!("  Available RAM: {:.1} MB", self.available_ram_mb);
        println!("  CPU Usage: {:.1}%", self.cpu_usage_percent);
        println!("  CPU Cores: {}", self.num_cpus);
        if let Some(load) = self.load_average {
            println!("  Load Average (1m): {:.2}", load);
        }
    }
}

/// Resource monitor for tracking usage over time
pub struct ResourceMonitor {
    /// Sampling interval
    interval: Duration,

    /// Maximum CPU usage threshold (0-100)
    max_cpu_percent: f64,

    /// Maximum RAM usage threshold (0-100)
    max_ram_percent: f64,

    /// History of samples
    samples: Vec<SystemResources>,
}

impl ResourceMonitor {
    /// Create new monitor with defaults
    pub fn new() -> Self {
        Self {
            interval: Duration::from_secs(1),
            max_cpu_percent: 90.0,
            max_ram_percent: 85.0,
            samples: Vec::new(),
        }
    }

    /// Set CPU threshold
    pub fn with_max_cpu(mut self, max_percent: f64) -> Self {
        self.max_cpu_percent = max_percent;
        self
    }

    /// Set RAM threshold
    pub fn with_max_ram(mut self, max_percent: f64) -> Self {
        self.max_ram_percent = max_percent;
        self
    }

    /// Set sampling interval
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Take a resource snapshot
    pub fn sample(&mut self) -> anyhow::Result<SystemResources> {
        let resources = SystemResources::current()?;
        self.samples.push(resources.clone());
        Ok(resources)
    }

    /// Check if system is overloaded
    pub fn is_overloaded(&self) -> anyhow::Result<bool> {
        let resources = SystemResources::current()?;

        let cpu_overload = resources.cpu_usage_percent > self.max_cpu_percent;
        let ram_overload = resources.ram_usage_percent() > self.max_ram_percent;

        Ok(cpu_overload || ram_overload)
    }

    /// Wait until resources become available
    pub fn wait_for_resources(&mut self, required_ram_mb: f64) -> anyhow::Result<()> {
        loop {
            let resources = self.sample()?;

            if resources.can_run_task(required_ram_mb, self.max_cpu_percent) {
                return Ok(());
            }

            log::debug!(
                "Waiting for resources: RAM {:.1}/{:.1} MB, CPU {:.1}%",
                resources.available_ram_mb,
                required_ram_mb,
                resources.cpu_usage_percent
            );

            std::thread::sleep(self.interval);
        }
    }

    /// Get average CPU usage from samples
    pub fn avg_cpu_usage(&self) -> Option<f64> {
        if self.samples.is_empty() {
            return None;
        }

        let sum: f64 = self.samples.iter().map(|s| s.cpu_usage_percent).sum();
        Some(sum / self.samples.len() as f64)
    }

    /// Get peak memory usage from samples
    pub fn peak_memory_mb(&self) -> Option<f64> {
        self.samples
            .iter()
            .map(|s| s.used_ram_mb)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
    }

    /// Clear sample history
    pub fn clear_samples(&mut self) {
        self.samples.clear();
    }

    /// Get number of samples
    pub fn num_samples(&self) -> usize {
        self.samples.len()
    }
}

impl Default for ResourceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_resources() {
        let resources = SystemResources::current().unwrap();

        assert!(resources.total_ram_mb > 0.0);
        assert!(resources.num_cpus > 0);
        assert!(resources.ram_usage_percent() >= 0.0);
        assert!(resources.ram_usage_percent() <= 100.0);

        resources.print_summary();
    }

    #[test]
    fn test_resource_monitor() {
        let mut monitor = ResourceMonitor::new()
            .with_max_cpu(95.0)
            .with_max_ram(90.0);

        let resources = monitor.sample().unwrap();
        assert_eq!(monitor.num_samples(), 1);

        assert!(resources.total_ram_mb > 0.0);

        monitor.clear_samples();
        assert_eq!(monitor.num_samples(), 0);
    }

    #[test]
    fn test_can_run_task() {
        let resources = SystemResources::current().unwrap();

        // Should be able to run a small task
        assert!(resources.can_run_task(10.0, 99.0));

        // Should NOT be able to run a task requiring more RAM than exists
        assert!(!resources.can_run_task(resources.total_ram_mb * 2.0, 99.0));
    }
}
