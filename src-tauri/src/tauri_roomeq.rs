//! Tauri commands for multi-channel room EQ optimization
//!
//! This module provides commands for optimizing complete room audio setups
//! including stereo pairs, multi-channel systems (5.1, 7.1), and multi-way
//! speakers with crossovers.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

use crate::tauri_optim::{
    CancellationState, OptimizationParams, OptimizationResult, ProgressCallback, ProgressUpdate,
    run_optimization_internal,
};

// ============================================================================
// RoomEQ Configuration Types
// ============================================================================

/// Type of room EQ configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RoomConfigType {
    /// Single speaker or headphone (uses existing single optimization)
    Single,
    /// Stereo pair (2 channels)
    StereoPair {
        mirror: bool, // If true, optimize once and mirror
    },
    /// Multi-channel system (3+ channels)
    MultiChannel {
        channel_count: usize,
        parallel: bool, // If true, optimize all channels in parallel
    },
    /// Multi-way speaker with crossover
    MultiWay {
        driver_count: usize,
        optimize_crossover: bool,
    },
}

/// Measurement source for a channel
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "source_type", rename_all = "snake_case")]
pub enum MeasurementSource {
    /// CSV file path
    File { path: String },
    /// Spinorama database
    Database {
        speaker: String,
        version: String,
        measurement: String,
    },
    /// Captured audio data
    Captured {
        frequencies: Vec<f64>,
        magnitudes: Vec<f64>,
    },
}

/// Multi-way driver configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverConfig {
    pub name: String,
    pub measurement: MeasurementSource,
}

/// Crossover configuration for multi-way speakers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossoverConfig {
    pub crossover_type: String, // "LR24", "LR48", "Butterworth24", etc.
    pub frequency: Option<f64>, // Fixed frequency (if not optimizing)
    pub optimize: bool,         // Whether to optimize crossover frequency
}

/// Channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub channel_name: String,
    pub measurement: Option<MeasurementSource>, // For simple channels
    pub drivers: Option<Vec<DriverConfig>>,     // For multi-way speakers
    pub crossover: Option<CrossoverConfig>,     // For multi-way speakers
    pub target: Option<MeasurementSource>,      // Optional target curve
}

/// Complete room EQ configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEQConfig {
    pub config_type: RoomConfigType,
    pub channels: Vec<ChannelConfig>,
    pub optimizer_params: OptimizationParams, // Base optimization parameters
}

/// Progress update for multi-channel optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEQProgress {
    pub channel_index: usize,
    pub channel_name: String,
    pub stage: String, // "crossover", "eq", "complete"
    pub progress: ProgressUpdate,
}

/// Result for a single channel optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelResult {
    pub channel_name: String,
    pub success: bool,
    pub error_message: Option<String>,
    pub optimization_result: Option<OptimizationResult>,
}

/// Complete room EQ optimization result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEQResult {
    pub success: bool,
    pub error_message: Option<String>,
    pub channel_results: Vec<ChannelResult>,
    pub dsp_chain_json: Option<String>, // JSON output compatible with roomeq binary
}

// ============================================================================
// Tauri Progress Callback for Room EQ
// ============================================================================

struct RoomEQProgressCallback {
    app_handle: AppHandle,
    channel_index: usize,
    channel_name: String,
    stage: String,
}

impl ProgressCallback for RoomEQProgressCallback {
    fn on_progress(&self, update: ProgressUpdate) -> bool {
        let room_progress = RoomEQProgress {
            channel_index: self.channel_index,
            channel_name: self.channel_name.clone(),
            stage: self.stage.clone(),
            progress: update,
        };

        match self.app_handle.emit("roomeq_progress", &room_progress) {
            Ok(_) => true,
            Err(e) => {
                eprintln!("[ROOMEQ] Failed to emit progress: {}", e);
                true // Continue even if emit fails
            }
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert MeasurementSource to OptimizationParams fields
fn apply_measurement_source(
    params: &mut OptimizationParams,
    source: &MeasurementSource,
) -> Result<(), String> {
    match source {
        MeasurementSource::File { path } => {
            params.curve_path = Some(path.clone());
            params.speaker = None;
            params.version = None;
            params.measurement = None;
            params.captured_frequencies = None;
            params.captured_magnitudes = None;
        }
        MeasurementSource::Database {
            speaker,
            version,
            measurement,
        } => {
            params.speaker = Some(speaker.clone());
            params.version = Some(version.clone());
            params.measurement = Some(measurement.clone());
            params.curve_path = None;
            params.captured_frequencies = None;
            params.captured_magnitudes = None;
        }
        MeasurementSource::Captured {
            frequencies,
            magnitudes,
        } => {
            params.captured_frequencies = Some(frequencies.clone());
            params.captured_magnitudes = Some(magnitudes.clone());
            params.speaker = None;
            params.version = None;
            params.measurement = None;
            params.curve_path = None;
        }
    }
    Ok(())
}

/// Optimize a single channel
async fn optimize_channel(
    channel: &ChannelConfig,
    channel_index: usize,
    base_params: &OptimizationParams,
    app_handle: AppHandle,
    cancellation_state: Arc<CancellationState>,
) -> Result<ChannelResult, String> {
    println!(
        "[ROOMEQ] Optimizing channel {} ({}/{})",
        channel.channel_name,
        channel_index + 1,
        "total"
    );

    // Check if this is a multi-way speaker (has drivers and crossover)
    if let (Some(drivers), Some(crossover)) = (&channel.drivers, &channel.crossover) {
        // Multi-way speaker optimization
        return optimize_multiway_channel(
            channel,
            channel_index,
            drivers,
            crossover,
            base_params,
            app_handle,
            cancellation_state,
        )
        .await;
    }

    // Simple channel optimization
    if let Some(measurement) = &channel.measurement {
        let mut params = base_params.clone();
        apply_measurement_source(&mut params, measurement)?;

        // Apply target if specified
        if let Some(target) = &channel.target {
            match target {
                MeasurementSource::File { path } => {
                    params.target_path = Some(path.clone());
                }
                MeasurementSource::Captured {
                    frequencies,
                    magnitudes,
                } => {
                    params.target_frequencies = Some(frequencies.clone());
                    params.target_magnitudes = Some(magnitudes.clone());
                }
                _ => {}
            }
        }

        let progress_callback = Arc::new(RoomEQProgressCallback {
            app_handle,
            channel_index,
            channel_name: channel.channel_name.clone(),
            stage: "eq".to_string(),
        });

        match run_optimization_internal(params, progress_callback, cancellation_state).await {
            Ok(result) => Ok(ChannelResult {
                channel_name: channel.channel_name.clone(),
                success: true,
                error_message: None,
                optimization_result: Some(result),
            }),
            Err(e) => Ok(ChannelResult {
                channel_name: channel.channel_name.clone(),
                success: false,
                error_message: Some(e.to_string()),
                optimization_result: None,
            }),
        }
    } else {
        Err(format!(
            "Channel {} has no measurement source",
            channel.channel_name
        ))
    }
}

/// Optimize a multi-way speaker channel with crossover
async fn optimize_multiway_channel(
    channel: &ChannelConfig,
    channel_index: usize,
    drivers: &[DriverConfig],
    crossover: &CrossoverConfig,
    base_params: &OptimizationParams,
    app_handle: AppHandle,
    cancellation_state: Arc<CancellationState>,
) -> Result<ChannelResult, String> {
    println!(
        "[ROOMEQ] Optimizing multi-way channel {} with {} drivers",
        channel.channel_name,
        drivers.len()
    );

    // Build multi-driver optimization parameters
    let mut params = base_params.clone();
    params.loss = "drivers-flat".to_string();
    params.crossover_type = Some(crossover.crossover_type.clone());

    // Set driver paths
    if drivers.len() >= 2 {
        if let MeasurementSource::File { path } = &drivers[0].measurement {
            params.driver1_path = Some(path.clone());
        }
        if let MeasurementSource::File { path } = &drivers[1].measurement {
            params.driver2_path = Some(path.clone());
        }
    }
    if drivers.len() >= 3 {
        if let MeasurementSource::File { path } = &drivers[2].measurement {
            params.driver3_path = Some(path.clone());
        }
    }
    if drivers.len() >= 4 {
        if let MeasurementSource::File { path } = &drivers[3].measurement {
            params.driver4_path = Some(path.clone());
        }
    }

    // Stage 1: Optimize crossover (if enabled)
    if crossover.optimize {
        let crossover_callback = Arc::new(RoomEQProgressCallback {
            app_handle: app_handle.clone(),
            channel_index,
            channel_name: channel.channel_name.clone(),
            stage: "crossover".to_string(),
        });

        // TODO: Implement crossover-only optimization
        // For now, the full optimization includes crossover
        println!(
            "[ROOMEQ] Crossover optimization for {}",
            channel.channel_name
        );
    }

    // Stage 2: Optimize EQ on combined response
    let eq_callback = Arc::new(RoomEQProgressCallback {
        app_handle,
        channel_index,
        channel_name: channel.channel_name.clone(),
        stage: "eq".to_string(),
    });

    match run_optimization_internal(params, eq_callback, cancellation_state).await {
        Ok(result) => Ok(ChannelResult {
            channel_name: channel.channel_name.clone(),
            success: true,
            error_message: None,
            optimization_result: Some(result),
        }),
        Err(e) => Ok(ChannelResult {
            channel_name: channel.channel_name.clone(),
            success: false,
            error_message: Some(e.to_string()),
            optimization_result: None,
        }),
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Run multi-channel room EQ optimization
#[tauri::command]
pub async fn run_roomeq_optimization(
    config: RoomEQConfig,
    app_handle: AppHandle,
    cancellation_state: State<'_, CancellationState>,
) -> Result<RoomEQResult, String> {
    println!("[ROOMEQ] Starting room EQ optimization");
    println!("[ROOMEQ] Config type: {:?}", config.config_type);
    println!("[ROOMEQ] Channels: {}", config.channels.len());

    // Reset cancellation state
    cancellation_state.reset();

    let mut channel_results = Vec::new();

    match &config.config_type {
        RoomConfigType::Single => {
            // Single channel - just optimize once
            if config.channels.is_empty() {
                return Err("No channels specified".to_string());
            }

            let result = optimize_channel(
                &config.channels[0],
                0,
                &config.optimizer_params,
                app_handle,
                Arc::new((*cancellation_state).clone()),
            )
            .await?;

            channel_results.push(result);
        }

        RoomConfigType::StereoPair { mirror } => {
            if config.channels.len() < 2 {
                return Err("Stereo pair requires at least 2 channels".to_string());
            }

            if *mirror {
                // Optimize left channel only, mirror to right
                let left_result = optimize_channel(
                    &config.channels[0],
                    0,
                    &config.optimizer_params,
                    app_handle.clone(),
                    Arc::new((*cancellation_state).clone()),
                )
                .await?;

                // Mirror the result to right channel
                let right_result = ChannelResult {
                    channel_name: config.channels[1].channel_name.clone(),
                    success: left_result.success,
                    error_message: left_result.error_message.clone(),
                    optimization_result: left_result.optimization_result.clone(),
                };

                channel_results.push(left_result);
                channel_results.push(right_result);
            } else {
                // Optimize both channels independently
                for (i, channel) in config.channels.iter().enumerate() {
                    let result = optimize_channel(
                        channel,
                        i,
                        &config.optimizer_params,
                        app_handle.clone(),
                        Arc::new((*cancellation_state).clone()),
                    )
                    .await?;
                    channel_results.push(result);
                }
            }
        }

        RoomConfigType::MultiChannel {
            channel_count: _,
            parallel,
        } => {
            if *parallel {
                // Parallel optimization - spawn all channels concurrently
                let mut handles = Vec::new();

                for (i, channel) in config.channels.iter().enumerate() {
                    let channel = channel.clone();
                    let params = config.optimizer_params.clone();
                    let app_handle = app_handle.clone();
                    let cancellation = Arc::new((*cancellation_state).clone());

                    let handle = tokio::spawn(async move {
                        optimize_channel(&channel, i, &params, app_handle, cancellation).await
                    });

                    handles.push(handle);
                }

                // Wait for all to complete
                for handle in handles {
                    match handle.await {
                        Ok(Ok(result)) => channel_results.push(result),
                        Ok(Err(e)) => return Err(format!("Channel optimization failed: {}", e)),
                        Err(e) => return Err(format!("Task join error: {}", e)),
                    }
                }
            } else {
                // Sequential optimization
                for (i, channel) in config.channels.iter().enumerate() {
                    let result = optimize_channel(
                        channel,
                        i,
                        &config.optimizer_params,
                        app_handle.clone(),
                        Arc::new((*cancellation_state).clone()),
                    )
                    .await?;
                    channel_results.push(result);
                }
            }
        }

        RoomConfigType::MultiWay {
            driver_count: _,
            optimize_crossover: _,
        } => {
            // Multi-way speakers - each channel has multiple drivers
            for (i, channel) in config.channels.iter().enumerate() {
                let result = optimize_channel(
                    channel,
                    i,
                    &config.optimizer_params,
                    app_handle.clone(),
                    Arc::new((*cancellation_state).clone()),
                )
                .await?;
                channel_results.push(result);
            }
        }
    }

    // Check if all channels succeeded
    let all_success = channel_results.iter().all(|r| r.success);

    // TODO: Generate DSP chain JSON in roomeq format
    let dsp_chain_json = None;

    Ok(RoomEQResult {
        success: all_success,
        error_message: if all_success {
            None
        } else {
            Some("Some channels failed optimization".to_string())
        },
        channel_results,
        dsp_chain_json,
    })
}

/// Cancel room EQ optimization
#[tauri::command]
pub fn cancel_roomeq_optimization(
    cancellation_state: State<CancellationState>,
) -> Result<(), String> {
    println!("[ROOMEQ] Cancellation requested");
    cancellation_state.cancel();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_measurement_source_serialization() {
        let file_source = MeasurementSource::File {
            path: "test.csv".to_string(),
        };
        let json = serde_json::to_string(&file_source).unwrap();
        assert!(json.contains("\"source_type\":\"file\""));

        let db_source = MeasurementSource::Database {
            speaker: "KEF LS50".to_string(),
            version: "v1.0".to_string(),
            measurement: "On Axis".to_string(),
        };
        let json = serde_json::to_string(&db_source).unwrap();
        assert!(json.contains("\"source_type\":\"database\""));
    }

    #[test]
    fn test_room_config_type_serialization() {
        let stereo = RoomConfigType::StereoPair { mirror: true };
        let json = serde_json::to_string(&stereo).unwrap();
        assert!(json.contains("\"type\":\"stereo_pair\""));
    }
}
