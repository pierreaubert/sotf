use super::audio_daemon::AudioDaemon;
use super::consts::MAX_HAL_CHANNELS;
use super::consts::SUPPORTED_SAMPLE_RATES;
use super::driver_manager::DriverManager;
use super::systemwide_state::SystemwideState;
use super::types::PipelineReconfigureOutcome;
use driver_common::DriverConfig;
use parking_lot::Mutex;
use sotf_audio::manager::AudioEngineManager;
use std::sync::Arc;

/// Handle a driver-initiated config change
pub(super) fn handle_driver_config_change(
    driver_manager: &Arc<Mutex<DriverManager>>,
    audio_manager: &Arc<Mutex<AudioEngineManager>>,
    config: DriverConfig,
    system_state: &Arc<Mutex<SystemwideState>>,
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

    let negotiated = actual_rate != requested_rate;

    // Reconfigure audio pipeline
    match reconfigure_audio_pipeline(
        audio_manager,
        system_state,
        actual_rate,
        requested_frames,
        requested_channels as usize,
    ) {
        Ok(outcome) => {
            if matches!(
                outcome,
                PipelineReconfigureOutcome::Restarted | PipelineReconfigureOutcome::Restored
            ) {
                // Set engine_ready so driver continues sending audio.
                driver_manager.lock().set_engine_ready(true);
            }

            let result = if negotiated {
                log::info!(
                    "Config negotiated: requested {}Hz, using {}Hz",
                    requested_rate,
                    actual_rate
                );
                driver_common::ConfigResult::negotiated(
                    actual_rate,
                    requested_frames,
                    requested_channels,
                )
            } else {
                driver_common::ConfigResult::Accepted
            };

            driver_manager.lock().acknowledge_config_change(
                DriverConfig::new(actual_rate, requested_frames, config.channel_count),
                result,
            );
            log::info!(
                "Config accepted: {}Hz, {} frames, {} channels, outcome={:?}",
                actual_rate,
                requested_frames,
                requested_channels,
                outcome
            );
        }
        Err(e) => {
            log::error!("Pipeline reconfiguration failed: {}", e);
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
                return Ok(PipelineReconfigureOutcome::Restored);
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
