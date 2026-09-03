use super::audio_daemon::{AudioDaemon, wait_for_playback_observation};
use super::consts::MAX_HAL_CHANNELS;
use super::consts::SUPPORTED_SAMPLE_RATES;
use super::driver_manager::DriverManager;
use super::systemwide_state::SystemwideState;
use super::types::PipelineReconfigureOutcome;
use driver_common::DriverConfig;
use parking_lot::Mutex;
use sotf_audio::manager::AudioEngineManager;
use std::sync::Arc;
use std::time::Duration;

#[cfg(not(test))]
const DRIVER_RECONFIGURE_READY_TIMEOUT: Duration = Duration::from_secs(12);
#[cfg(test)]
const DRIVER_RECONFIGURE_READY_TIMEOUT: Duration = Duration::from_millis(200);

pub(super) fn wait_for_reconfigured_playback(
    audio_manager: &Arc<Mutex<AudioEngineManager>>,
    running: &Arc<Mutex<bool>>,
) -> Result<(), String> {
    wait_for_playback_observation(
        running,
        DRIVER_RECONFIGURE_READY_TIMEOUT,
        "driver reconfiguration",
        || {
            let state = audio_manager.lock().get_engine_state();
            AudioDaemon::playback_startup_observation(&state)
        },
    )
}

pub(super) fn acknowledged_config_for_outcome(
    requested_rate: u32,
    requested_frames: u32,
    requested_channels: u32,
    actual_rate: u32,
    outcome: PipelineReconfigureOutcome,
) -> (DriverConfig, driver_common::ConfigResult) {
    let active_channels = match outcome {
        PipelineReconfigureOutcome::Restored { input_channels } => input_channels as u32,
        PipelineReconfigureOutcome::IdleUpdated | PipelineReconfigureOutcome::Restarted => {
            requested_channels
        }
    };
    let actual = DriverConfig::new(actual_rate, requested_frames, active_channels);
    let result = if actual_rate != requested_rate || active_channels != requested_channels {
        driver_common::ConfigResult::negotiated(actual_rate, requested_frames, active_channels)
    } else {
        driver_common::ConfigResult::Accepted
    };
    (actual, result)
}

/// Publish HAL readiness only after the configuration acknowledgement has
/// completed. A failed callback observation records recovery and keeps HAL
/// unready so the caller can acknowledge the failure instead.
pub(super) fn publish_reconfigured_driver_readiness(
    driver_manager: &Arc<Mutex<DriverManager>>,
    system_state: &Arc<Mutex<SystemwideState>>,
    active_config: DriverConfig,
    readiness: Result<(), String>,
) -> Result<(), String> {
    if let Err(error) = readiness {
        system_state.lock().mark_pipeline_recovery(error.clone());
        driver_manager.lock().acknowledge_config_change(
            active_config,
            driver_common::ConfigResult::error(error.clone()),
        );
        return Err(error);
    }

    driver_manager.lock().set_engine_ready(true);
    Ok(())
}

/// Handle a driver-initiated config change
pub(super) fn handle_driver_config_change(
    driver_manager: &Arc<Mutex<DriverManager>>,
    audio_manager: &Arc<Mutex<AudioEngineManager>>,
    config: DriverConfig,
    system_state: &Arc<Mutex<SystemwideState>>,
    running: &Arc<Mutex<bool>>,
) {
    let requested_rate = config.sample_rate;
    let requested_frames = config.buffer_frames;
    let requested_channels = config.channel_count;

    log::info!(
        "Driver config change request: sample_rate={}, buffer_frames={}, channels={}",
        requested_rate,
        requested_frames,
        requested_channels
    );

    // Validate requested values
    if requested_rate == 0 {
        log::warn!("Invalid config request: sample_rate=0, ignoring");
        driver_manager.lock().acknowledge_config_change(
            DriverConfig::new(48000, requested_frames, config.channel_count),
            driver_common::ConfigResult::error(driver_common::DriverError::invalid_config(
                "sample_rate",
                "Invalid sample rate",
            )),
        );
        return;
    }
    if requested_frames == 0 || requested_frames > 65536 {
        log::warn!(
            "Invalid config request: buffer_frames={}, out of range",
            requested_frames
        );
        driver_manager.lock().acknowledge_config_change(
            DriverConfig::new(requested_rate, 512, config.channel_count),
            driver_common::ConfigResult::error(driver_common::DriverError::invalid_config(
                "buffer_frames",
                "Invalid buffer frames",
            )),
        );
        return;
    }
    if requested_channels == 0 || requested_channels as usize > MAX_HAL_CHANNELS {
        log::warn!(
            "Invalid config request: channel_count={}, out of range",
            requested_channels
        );
        driver_manager.lock().acknowledge_config_change(
            DriverConfig::new(requested_rate, requested_frames, 2),
            driver_common::ConfigResult::error(driver_common::DriverError::invalid_config(
                "channel_count",
                "Invalid channel count",
            )),
        );
        return;
    }

    // Determine actual rate to use
    let actual_rate = if SUPPORTED_SAMPLE_RATES.contains(&requested_rate) {
        requested_rate
    } else {
        SUPPORTED_SAMPLE_RATES
            .iter()
            .min_by_key(|&&r| (r as i32 - requested_rate as i32).abs())
            .copied()
            .unwrap_or(48000)
    };

    // Reconfigure audio pipeline
    // Stop publishing input frames while the old engine is being torn down.
    // A failed restart must not leave HAL believing that a stopped engine is
    // still ready to consume audio.
    driver_manager.lock().set_engine_ready(false);
    match reconfigure_audio_pipeline(
        audio_manager,
        system_state,
        actual_rate,
        requested_frames,
        requested_channels as usize,
    ) {
        Ok(outcome) => {
            let (active_config, result) = acknowledged_config_for_outcome(
                requested_rate,
                requested_frames,
                requested_channels,
                actual_rate,
                outcome,
            );
            if matches!(
                outcome,
                PipelineReconfigureOutcome::Restarted | PipelineReconfigureOutcome::Restored { .. }
            ) && let Err(error) = wait_for_reconfigured_playback(audio_manager, running)
            {
                log::error!("Driver reconfiguration readiness failed: {error}");
                let _ = publish_reconfigured_driver_readiness(
                    driver_manager,
                    system_state,
                    active_config,
                    Err(error.clone()),
                );
                return;
            }
            system_state.lock().clear_pipeline_recovery();
            if result != driver_common::ConfigResult::Accepted {
                log::info!(
                    "Config negotiated: requested {}Hz/{}ch, using {}Hz/{}ch",
                    requested_rate,
                    requested_channels,
                    active_config.sample_rate,
                    active_config.channel_count
                );
            }

            driver_manager
                .lock()
                .acknowledge_config_change(active_config, result);

            if matches!(
                outcome,
                PipelineReconfigureOutcome::Restarted | PipelineReconfigureOutcome::Restored { .. }
            ) {
                // Publish readiness only after HAL has acknowledged the exact
                // geometry consumed by the running engine.
                let _ = publish_reconfigured_driver_readiness(
                    driver_manager,
                    system_state,
                    active_config,
                    Ok(()),
                );
            }
            log::info!(
                "Config accepted: {}Hz, {} frames, {} channels, outcome={:?}",
                actual_rate,
                requested_frames,
                active_config.channel_count,
                outcome
            );
        }
        Err(e) => {
            log::error!("Pipeline reconfiguration failed: {}", e);
            system_state.lock().mark_pipeline_recovery(e.clone());
            driver_manager.lock().acknowledge_config_change(
                DriverConfig::new(actual_rate, requested_frames, config.channel_count),
                driver_common::ConfigResult::error(e),
            );
        }
    }
}

/// Reconfigure the audio pipeline with new sample rate and buffer size
pub(super) fn reconfigure_audio_pipeline(
    audio_manager: &Arc<Mutex<AudioEngineManager>>,
    system_state: &Arc<Mutex<SystemwideState>>,
    hal_sample_rate: u32,
    hal_buffer_frames: u32,
    input_channels: usize,
) -> Result<PipelineReconfigureOutcome, String> {
    let plan = {
        let state = system_state.lock();
        if let Some(graph) = state.user_graph() {
            state.prepare_graph_plan(
                graph,
                input_channels,
                state.output_channels(),
                input_channels,
            )?
        } else {
            state.prepare_plan(
                state.user_plugins(),
                input_channels,
                state.output_channels(),
                input_channels,
            )?
        }
    };

    // Keep a fully prepared copy of the last applied pipeline before
    // stopping the engine. If the new driver timing or graph cannot start,
    // the daemon can restore audio instead of leaving the process silent.
    let previous_plan = {
        let state = system_state.lock();
        state
            .applied_spec()
            .and_then(|spec| state.prepare_from_spec(spec, input_channels).ok())
    };

    let mut manager = audio_manager.lock();

    let state = manager.get_state();
    if state == sotf_audio::manager::StreamingState::Idle {
        log::debug!("No active playback, acknowledging config change");
        system_state.lock().commit_idle_reconfigure(&plan);
        return Ok(PipelineReconfigureOutcome::IdleUpdated);
    }

    log::info!("Reconfiguring driver playback pipeline");

    if let Err(e) = manager.stop() {
        log::warn!("Failed to stop current playback: {}", e);
    }

    log::info!(
        "Restarting driver playback with {} plugins (incl. 2 monitors), {} output channels, device: {:?}",
        plan.runtime_plugins.len(),
        plan.spec.output_channels,
        plan.spec.output_device
    );

    let result =
        AudioDaemon::start_pipeline_plan(&mut manager, &plan, hal_sample_rate, hal_buffer_frames);

    match result {
        Ok(_) => {
            system_state.lock().commit_applied(&plan);
            log::info!("Driver playback restarted successfully");
            Ok(PipelineReconfigureOutcome::Restarted)
        }
        Err(e) => {
            log::error!("Failed to restart driver playback: {}", e);

            let Some(previous_plan) = previous_plan else {
                return Err(format!("Failed to restart driver playback: {}", e));
            };

            log::warn!("Attempting to restore the last working driver pipeline");
            let restore = AudioDaemon::start_pipeline_plan(
                &mut manager,
                &previous_plan,
                hal_sample_rate,
                hal_buffer_frames,
            );
            if restore.is_ok() {
                log::warn!(
                    "Restored the last working driver pipeline after reconfiguration failure"
                );
                return Ok(PipelineReconfigureOutcome::Restored {
                    input_channels: previous_plan.spec.input_channels,
                });
            }

            Err(format!(
                "Failed to restart driver playback: {}; pipeline recovery also failed: {}",
                e,
                restore
                    .err()
                    .unwrap_or_else(|| "unknown recovery error".to_string())
            ))
        }
    }
}
