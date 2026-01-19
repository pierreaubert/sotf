//! Optimization state save/restore for resume capability.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Saved optimization state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizerState {
    /// Best parameters found so far
    pub best_params: Vec<f64>,
    /// Current parameters
    pub current_params: Vec<f64>,
    /// Best loss value
    pub best_loss: f64,
    /// Current iteration
    pub iteration: usize,
    /// Total iterations requested
    pub total_iterations: usize,
    /// Whether optimization was converging
    pub converged: bool,
    /// Random seed used
    pub seed: Option<u64>,
    /// Timestamp of save
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Save optimizer state to file
pub fn save_optimizer_state(
    state: &OptimizerState,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(state)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Load optimizer state from file
pub fn load_optimizer_state(
    path: &Path,
) -> Result<Option<OptimizerState>, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(None);
    }

    let json = std::fs::read_to_string(path)?;
    let state = serde_json::from_str(&json)?;
    Ok(Some(state))
}

/// Get default state file path for a given output directory
pub fn get_state_file_path(output_dir: &Path) -> PathBuf {
    output_dir.join("optimizer_state.json")
}
