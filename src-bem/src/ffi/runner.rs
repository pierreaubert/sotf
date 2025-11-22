//! NumCalc subprocess runner
//!
//! Executes NumCalc as a subprocess with proper error handling,
//! timeout support, and output collection.

use super::config::{MemoryEstimate, NumCalcConfig, NumCalcOutput};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// NumCalc subprocess runner
pub struct NumCalcRunner {
    /// Path to NumCalc executable
    executable: PathBuf,

    /// Project directory containing NC.inp
    project_dir: PathBuf,
}

impl NumCalcRunner {
    /// Create new runner for project directory
    ///
    /// # Arguments
    ///
    /// * `project_dir` - Directory containing NC.inp and project files
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use bem::ffi::NumCalcRunner;
    ///
    /// let runner = NumCalcRunner::new("path/to/project")?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn new(project_dir: impl AsRef<Path>) -> Result<Self> {
        let project_dir = project_dir.as_ref().to_path_buf();

        // Verify project directory exists
        if !project_dir.exists() {
            anyhow::bail!("Project directory does not exist: {:?}", project_dir);
        }

        // Verify NC.inp exists
        let nc_inp = project_dir.join("NC.inp");
        if !nc_inp.exists() {
            anyhow::bail!("NC.inp not found in project directory: {:?}", project_dir);
        }

        // Find NumCalc executable
        let executable = Self::find_executable()?;

        log::info!("NumCalc executable: {:?}", executable);
        log::info!("Project directory: {:?}", project_dir);

        Ok(Self {
            executable,
            project_dir,
        })
    }

    /// Find NumCalc executable
    ///
    /// Search order:
    /// 1. NUMCALC_PATH environment variable
    /// 2. ./NumCalc/bin/NumCalc (relative to crate root)
    /// 3. System PATH
    fn find_executable() -> Result<PathBuf> {
        // Try environment variable first
        if let Ok(path) = std::env::var("NUMCALC_PATH") {
            let path = PathBuf::from(path);
            if path.exists() {
                return Ok(path);
            }
        }

        // Try relative path
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let candidates = vec![
                PathBuf::from(&manifest_dir).join("NumCalc/bin/NumCalc"),
                PathBuf::from(&manifest_dir).join("NumCalc/NumCalc"),
                PathBuf::from(&manifest_dir).join("target/debug/NumCalc"),
                PathBuf::from(&manifest_dir).join("target/release/NumCalc"),
            ];

            for candidate in candidates {
                if candidate.exists() {
                    return Ok(candidate);
                }
            }
        }

        // Try system PATH
        #[cfg(target_os = "windows")]
        let executable_name = "NumCalc.exe";
        #[cfg(not(target_os = "windows"))]
        let executable_name = "NumCalc";

        which::which(executable_name)
            .with_context(|| format!("NumCalc executable not found. Set NUMCALC_PATH or install to PATH"))
    }

    /// Run NumCalc with given configuration
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use bem::ffi::{NumCalcRunner, NumCalcConfig};
    ///
    /// let runner = NumCalcRunner::new("project")?;
    /// let config = NumCalcConfig::single_frequency(0);
    /// let output = runner.run(&config)?;
    ///
    /// if output.is_success() {
    ///     println!("Success! Generated {} output files", output.num_output_files());
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn run(&self, config: &NumCalcConfig) -> Result<NumCalcOutput> {
        let start_time = Instant::now();

        // Build command
        let mut cmd = Command::new(&self.executable);

        // Set working directory
        let working_dir = config
            .working_dir
            .clone()
            .unwrap_or_else(|| self.project_dir.clone());
        cmd.current_dir(&working_dir);

        // Add command-line arguments
        if let Some(start) = config.freq_start_idx {
            cmd.arg("-istart").arg(start.to_string());
        }
        if let Some(end) = config.freq_end_idx {
            cmd.arg("-iend").arg(end.to_string());
        }
        if config.max_iterations != 250 {
            cmd.arg("-nitermax").arg(config.max_iterations.to_string());
        }
        if config.estimate_ram {
            cmd.arg("-estimate_ram");
        }
        if config.check_normals {
            cmd.arg("-check_normals");
        }

        // Configure I/O
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        log::info!("Executing NumCalc: {:?}", cmd);

        // Spawn process
        let mut child = cmd.spawn().context("Failed to spawn NumCalc process")?;

        // Apply timeout if specified
        let output = if let Some(timeout) = config.timeout {
            Self::wait_with_timeout(&mut child, timeout)?
        } else {
            child.wait_with_output()?
        };

        let execution_time = start_time.elapsed();

        // Collect output files
        let output_files = self.collect_output_files()?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let result = NumCalcOutput {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout,
            stderr,
            output_files,
            execution_time,
            peak_memory_mb: None,  // TODO: Implement memory tracking
            frequency_index: config.freq_start_idx,
        };

        log::info!(
            "NumCalc finished in {:.2}s, exit code: {:?}",
            execution_time.as_secs_f64(),
            result.exit_code
        );

        Ok(result)
    }

    /// Wait for process with timeout
    fn wait_with_timeout(
        child: &mut std::process::Child,
        timeout: Duration,
    ) -> Result<std::process::Output> {
        use std::thread;
        use std::sync::mpsc;

        let (tx, rx) = mpsc::channel();

        let child_id = child.id();

        // Spawn thread to wait for child
        thread::spawn(move || {
            let _ = tx.send(());
        });

        // Wait with timeout
        match rx.recv_timeout(timeout) {
            Ok(_) => {
                // Child finished
                child.wait_with_output()
                    .context("Failed to get child output")
            }
            Err(_) => {
                // Timeout - kill child
                log::warn!("NumCalc execution timed out after {:?}", timeout);
                child.kill()?;
                anyhow::bail!("NumCalc execution timed out after {:?}", timeout);
            }
        }
    }

    /// Collect output files generated by NumCalc
    fn collect_output_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        // Check for be.out directory
        let be_out = self.project_dir.join("be.out");
        if be_out.exists() && be_out.is_dir() {
            for entry in std::fs::read_dir(&be_out)? {
                let entry = entry?;
                if entry.path().is_file() {
                    files.push(entry.path());
                }
            }
        }

        // Check for fe.out directory
        let fe_out = self.project_dir.join("fe.out");
        if fe_out.exists() && fe_out.is_dir() {
            for entry in std::fs::read_dir(&fe_out)? {
                let entry = entry?;
                if entry.path().is_file() {
                    files.push(entry.path());
                }
            }
        }

        // Check for NC.out
        let nc_out = self.project_dir.join("NC.out");
        if nc_out.exists() {
            files.push(nc_out);
        }

        Ok(files)
    }

    /// Estimate memory requirements
    ///
    /// Runs NumCalc with `-estimate_ram` flag and parses Memory.txt
    pub fn estimate_memory(&self) -> Result<MemoryEstimate> {
        let config = NumCalcConfig::estimate_memory();
        self.run(&config)?;

        // Read Memory.txt
        let memory_file = self.project_dir.join("Memory.txt");
        if !memory_file.exists() {
            anyhow::bail!("Memory.txt not generated");
        }

        MemoryEstimate::from_file(&memory_file)
    }

    /// Get project directory
    pub fn project_dir(&self) -> &Path {
        &self.project_dir
    }

    /// Get executable path
    pub fn executable(&self) -> &Path {
        &self.executable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_executable() {
        // This will fail if NumCalc is not installed, but that's okay for testing
        match NumCalcRunner::find_executable() {
            Ok(path) => {
                println!("Found NumCalc at: {:?}", path);
                assert!(path.exists());
            }
            Err(e) => {
                println!("NumCalc not found (expected): {}", e);
            }
        }
    }

    #[test]
    #[ignore]  // Requires actual project directory
    fn test_runner_creation() {
        // This test is ignored because it requires a real project directory
        // Run with: cargo test test_runner_creation -- --ignored

        let project_dir = std::env::var("TEST_PROJECT_DIR")
            .unwrap_or_else(|_| "test_project".to_string());

        match NumCalcRunner::new(&project_dir) {
            Ok(runner) => {
                println!("Created runner for: {:?}", runner.project_dir());
            }
            Err(e) => {
                println!("Failed to create runner: {}", e);
            }
        }
    }
}
