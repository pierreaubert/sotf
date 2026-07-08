use super::command::Command;
use super::configured::configured_output_device_from_env;
use super::consts::LEGACY_SOCKET_PATH;
use super::consts::empty_loudness_json;
use super::consts::get_socket_path;
use super::consts::metering_source_json;
use super::driver_manager::{DriverManager, get_driver_status};
use super::loudness::loudness_data_to_json;
use super::loudness::loudness_info_to_json;
use super::misc::bind_unix_socket;
use super::misc::is_safe_output_device_name;
use super::misc::list_audio_devices;
use super::misc::push_metering_faults;
use super::misc::socket_is_unix_socket;
use super::misc::transport_snapshot_and_faults;
use super::pipeline_spec::pipeline_spec_to_json;
use super::plugin::plugin_parameter_descriptors;
use super::plugin::plugin_type_category;
use super::plugin::plugin_type_to_engine_str;
use super::plugin_artifact::{PluginArtifactPlan, plan_plugin_artifact};
use super::response::Response;
use super::response::serialize_response_safely;
use super::security::{
    KeyManager, PeerClass, classify_peer, current_uid as security_current_uid,
    ensure_secure_socket_dir, peer_allows_command, verify_peer_credentials,
};
use super::systemwide_state::SystemwideState;
use super::systemwide_state::spawn_driver_config_watcher;
use super::types::IpcLine;
use super::types::PipelinePlan;
use super::types::read_ipc_line_bounded;
use driver_common::DriverConfig;
use parking_lot::Mutex;
use serde_json::Value;
use sotf_audio::PluginConfig;
use sotf_audio::manager::AudioEngineManager;
use sotf_audio::plugins::PluginType;
use std::io::{BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::Arc;

pub(super) fn pipeline_timing_after_config_request(
    result: &driver_common::ConfigResult,
    requested_sample_rate: u32,
    requested_buffer_frames: u32,
) -> (u32, u32) {
    match result {
        driver_common::ConfigResult::Negotiated {
            actual_rate,
            actual_frames,
            ..
        } => (*actual_rate, *actual_frames),
        driver_common::ConfigResult::Accepted | driver_common::ConfigResult::Error(_) => {
            (requested_sample_rate, requested_buffer_frames)
        }
    }
}

#[derive(Clone)]
pub(super) struct AudioDaemon {
    pub(super) manager: Arc<Mutex<AudioEngineManager>>,
    pub(super) running: Arc<Mutex<bool>>,
    pub(super) driver_manager: Arc<Mutex<DriverManager>>,
    /// Desired and applied systemwide daemon state.
    pub(super) system_state: Arc<Mutex<SystemwideState>>,
    /// Encryption key manager
    pub(super) key_manager: Arc<Mutex<KeyManager>>,
    /// Shared Tokio runtime for async operations
    pub(super) runtime: Arc<tokio::runtime::Runtime>,
}

impl AudioDaemon {
    pub(super) fn new() -> Self {
        let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

        Self {
            manager: Arc::new(Mutex::new(AudioEngineManager::new())),
            running: Arc::new(Mutex::new(true)),
            driver_manager: Arc::new(Mutex::new(DriverManager::new())),
            system_state: Arc::new(Mutex::new(SystemwideState::default())),
            key_manager: Arc::new(Mutex::new(KeyManager::default())),
            runtime: Arc::new(runtime),
        }
    }

    pub(super) fn spawn_initial_driver_playback(&self) {
        let daemon = self.clone();
        std::thread::spawn(move || {
            println!("Auto-starting driver playback (2ch)...");

            let output_device = configured_output_device_from_env();
            println!("   Output device: {:?}", output_device);

            if let Some(device) = output_device {
                if let Err(e) = daemon
                    .system_state
                    .lock()
                    .set_desired_output_device(Some(device.clone()))
                {
                    println!("   Ignoring configured output device {:?}: {}", device, e);
                }
            } else {
                println!("   No output device override; playback thread will choose a safe device");
            }

            let plugins: Vec<PluginConfig> = vec![];

            let result = daemon
                .runtime
                .block_on(daemon.handle_load_plugins_with_channels(plugins, 2, 2));
            if result.success {
                println!("   Driver playback started successfully");
            } else {
                println!("   Driver playback failed: {:?}", result.error);
            }
        });
    }

    pub(super) async fn handle_command(&self, cmd: Command) -> Response {
        match cmd {
            Command::Status => self.handle_status().await,
            Command::GetSnapshot => self.handle_get_snapshot().await,
            Command::DumpState => self.handle_dump_state().await,
            Command::Load { path } => self.handle_load(&path).await,
            Command::Play => self.handle_play().await,
            Command::Pause => self.handle_pause().await,
            Command::Stop => self.handle_stop().await,
            Command::Seek { position } => self.handle_seek(position).await,
            Command::SetVolume { volume } => self.handle_set_volume(volume).await,
            Command::ListDevices => self.handle_list_devices().await,
            Command::SetDevice { device } => self.handle_set_device(&device).await,
            Command::LoadPlugins {
                plugins,
                input_channels,
                output_channels,
            } => {
                self.handle_load_plugins_with_channels(plugins, input_channels, output_channels)
                    .await
            }
            Command::LoadPluginArtifact { artifact } => {
                self.handle_load_plugin_artifact(artifact).await
            }
            Command::SetInputChannels { channels } => {
                self.handle_set_pipeline_channels(Some(channels), None)
                    .await
            }
            Command::SetOutputChannels { channels } => {
                self.handle_set_pipeline_channels(None, Some(channels))
                    .await
            }
            Command::SetPipelineChannels {
                input_channels,
                output_channels,
            } => {
                self.handle_set_pipeline_channels(input_channels, output_channels)
                    .await
            }
            Command::GetLoudness => self.handle_get_loudness().await,
            Command::GetMetering => self.handle_get_metering().await,
            Command::GetPlugins => self.handle_get_plugins().await,
            Command::GetAvailablePlugins => self.handle_get_available_plugins().await,
            Command::AddPlugin { plugin, index } => self.handle_add_plugin(plugin, index).await,
            Command::RemovePlugin { index } => self.handle_remove_plugin(index).await,
            Command::UpdatePlugin { index, parameters } => {
                self.handle_update_plugin(index, parameters).await
            }
            Command::ReorderPlugins { order } => self.handle_reorder_plugins(order).await,
            Command::DriverStatus => self.handle_driver_status().await,
            Command::Shutdown => {
                *self.running.lock() = false;
                Response::ok_empty()
            }
            // Encryption commands
            Command::SetEncryption { enabled } => self.handle_set_encryption(enabled).await,
            Command::EncryptionStatus => self.handle_encryption_status().await,
            Command::RotateEncryptionKey => self.handle_rotate_encryption_key().await,
            // Driver config commands
            Command::SetSampleRate { rate } => self.handle_set_sample_rate(rate).await,
            Command::SetBufferFrames { frames } => self.handle_set_buffer_frames(frames).await,
            Command::GetDriverConfig => self.handle_get_driver_config().await,
        }
    }

    pub(super) fn metering_snapshot(&self) -> Value {
        let manager = self.manager.lock();
        let pipeline = self.system_state.lock();
        let input_idx = pipeline.input_loudness_index();
        let output_idx = pipeline.output_loudness_index();
        let fallback_input_channels = pipeline.input_channels();
        let fallback_output_channels = manager.get_engine_state().num_channels;
        drop(pipeline);

        let input_data = input_idx.and_then(|idx| {
            manager
                .get_cached_plugin_data(idx)
                .and_then(|data| data.downcast_ref::<sotf_audio::LoudnessData>().cloned())
        });

        let output_data = output_idx.and_then(|idx| {
            manager
                .get_cached_plugin_data(idx)
                .and_then(|data| data.downcast_ref::<sotf_audio::LoudnessData>().cloned())
        });

        let input_json = input_data
            .as_ref()
            .map(loudness_data_to_json)
            .unwrap_or_else(|| empty_loudness_json(fallback_input_channels));
        let output_json = output_data
            .as_ref()
            .map(loudness_data_to_json)
            .unwrap_or_else(|| empty_loudness_json(fallback_output_channels));

        serde_json::json!({
            "input": input_json,
            "output": output_json,
            "sources": {
                "input": metering_source_json(input_data.is_some(), fallback_input_channels),
                "output": metering_source_json(output_data.is_some(), fallback_output_channels),
            },
        })
    }

    pub(super) fn snapshot_json(&self) -> Value {
        let driver_status = self.driver_manager.lock().status();
        let key_status = self.key_manager.lock().status();

        let manager = self.manager.lock();
        let state = manager.get_state();
        let state_name = format!("{:?}", state);
        let engine_state = manager.get_engine_state();
        let volume = manager.get_volume();
        let muted = manager.is_muted();
        drop(manager);

        let pipeline = self.system_state.lock();
        let desired = pipeline.desired_spec();
        let applied = pipeline.applied_spec();
        let applied_generation = pipeline.applied_generation();
        let applied_output_device = pipeline.applied_output_device();
        drop(pipeline);

        let (transport, mut faults) =
            transport_snapshot_and_faults(&state_name, &driver_status, &engine_state);

        if desired
            .output_device
            .as_ref()
            .is_some_and(|device| !is_safe_output_device_name(device))
        {
            faults.push(serde_json::json!({
                "code": "unsafe_desired_output_device",
                "severity": "error",
                "message": "Desired output device is virtual/loopback and would create a feedback risk.",
            }));
        }
        if engine_state
            .playback_output_device
            .as_ref()
            .is_some_and(|device| !is_safe_output_device_name(device))
        {
            faults.push(serde_json::json!({
                "code": "unsafe_observed_output_device",
                "severity": "error",
                "message": "Observed playback output device is virtual/loopback and risks feedback.",
            }));
        }

        let metering = self.metering_snapshot();
        push_metering_faults(&state_name, &metering, &mut faults);
        let health = if faults
            .iter()
            .any(|fault| fault["severity"].as_str() == Some("error"))
        {
            "fault"
        } else if faults.is_empty() {
            "ok"
        } else {
            "warning"
        };

        serde_json::json!({
            "schema_version": 1,
            "desired": pipeline_spec_to_json(&desired),
            "applied": {
                "generation": applied_generation,
                "output_device": applied_output_device,
                "spec": applied.as_ref().map(pipeline_spec_to_json),
            },
            "observed": {
                "engine": {
                    "state": state_name,
                    "volume": volume,
                    "muted": muted,
                    "sample_rate": engine_state.sample_rate,
                    "channels": engine_state.num_channels,
                    "underruns": engine_state.underruns,
                    "playback_output_device": engine_state.playback_output_device,
                    "playback_callback_count": engine_state.playback_callback_count,
                    "playback_buffer_fill_percent": engine_state.playback_buffer_fill_percent,
                    "playback_stream_error_count": engine_state.playback_stream_error_count,
                    "playback_frames_received": engine_state.playback_frames_received,
                    "playback_frames_written": engine_state.playback_frames_written,
                    "playback_frames_dropped": engine_state.playback_frames_dropped,
                    "playback_effective_sample_rate": engine_state.playback_effective_sample_rate,
                    "last_error": engine_state.last_error,
                },
                "driver": {
                    "platform_supported": driver_status.platform_supported,
                    "driver_installed": driver_status.driver_installed,
                    "driver_ready": driver_status.driver_ready,
                    "capture_active": driver_status.capture_active,
                    "sample_rate": driver_status.sample_rate,
                    "channel_count": driver_status.channel_count,
                    "buffer_frames": driver_status.buffer_frames,
                    "driver_name": driver_status.driver_name,
                },
                "encryption": {
                    "enabled": key_status.enabled,
                    "fingerprint": key_status.fingerprint,
                    "key_path": key_status.key_path,
                },
                "transport": transport,
                "metering": metering,
            },
            "diagnostics": {
                "health": health,
                "faults": faults,
            },
        })
    }

    pub(super) async fn handle_get_snapshot(&self) -> Response {
        Response::ok(self.snapshot_json())
    }

    pub(super) async fn handle_dump_state(&self) -> Response {
        Response::ok(serde_json::json!({
            "snapshot": self.snapshot_json(),
            "plugins": self.system_state.lock().user_plugins(),
        }))
    }

    pub(super) async fn handle_status(&self) -> Response {
        let (state, engine_state, volume, muted) = {
            let manager = self.manager.lock();
            (
                manager.get_state(),
                manager.get_engine_state(),
                manager.get_volume(),
                manager.is_muted(),
            )
        };
        let (
            selected_device,
            input_channels,
            output_channels,
            pipeline_generation,
            pipeline_applied_output_device,
        ) = {
            let pipeline = self.system_state.lock();
            (
                pipeline.selected_output_device(),
                pipeline.input_channels(),
                pipeline.output_channels(),
                pipeline.applied_generation(),
                pipeline.applied_output_device(),
            )
        };
        let driver_status = self.driver_manager.lock().status();
        let key_status = self.key_manager.lock().status();

        let mut recovery_actions = Vec::<String>::new();
        if !driver_status.platform_supported {
            recovery_actions.push("driver_not_supported".to_string());
        }
        if !driver_status.driver_installed {
            recovery_actions.push("reinstall_driver".to_string());
        }
        if driver_status.driver_installed && !driver_status.driver_ready {
            recovery_actions.push("restart_daemon".to_string());
        }
        if selected_device.is_none()
            && pipeline_applied_output_device.is_none()
            && engine_state.playback_output_device.is_none()
        {
            recovery_actions.push("select_output_device".to_string());
        }
        if key_status.enabled && key_status.fingerprint.len() < 16 {
            recovery_actions.push("rotate_encryption_key".to_string());
        }
        if engine_state.underruns > 0 {
            recovery_actions.push("reset_shared_memory".to_string());
        }

        Response::ok(serde_json::json!({
            "state": format!("{:?}", state),
            "volume": volume,
            "muted": muted,
            "selected_device": selected_device,
            "pipeline_generation": pipeline_generation,
            "pipeline_applied_output_device": pipeline_applied_output_device,
            "sample_rate": engine_state.sample_rate,
            "input_channels": input_channels,
            "output_channels": output_channels,
            "channels": engine_state.num_channels,
            "underruns": engine_state.underruns,
            "playback_output_device": engine_state.playback_output_device,
            "playback_callback_count": engine_state.playback_callback_count,
            "playback_buffer_fill_percent": engine_state.playback_buffer_fill_percent,
            "playback_stream_error_count": engine_state.playback_stream_error_count,
            "playback_frames_received": engine_state.playback_frames_received,
            "playback_frames_written": engine_state.playback_frames_written,
            "playback_frames_dropped": engine_state.playback_frames_dropped,
            "playback_effective_sample_rate": engine_state.playback_effective_sample_rate,
            "last_error": engine_state.last_error,
            "driver": {
                "installed": driver_status.driver_installed,
                "ready": driver_status.driver_ready,
                "capture_active": driver_status.capture_active,
                "frame_size": driver_status.buffer_frames,
                "sample_rate": driver_status.sample_rate,
                "channel_count": driver_status.channel_count,
            },
            "encryption": {
                "enabled": key_status.enabled,
                "fingerprint": key_status.fingerprint,
            },
            "active_route": {
                "desired_output_device": selected_device,
                "applied_output_device": pipeline_applied_output_device,
                "playback_output_device": engine_state.playback_output_device,
                "capture_active": driver_status.capture_active,
            },
            "recovery_actions": recovery_actions,
        }))
    }

    pub(super) async fn handle_load(&self, path: &str) -> Response {
        let mut manager = self.manager.lock();
        match manager.load_file(path) {
            Ok(_) => Response::ok_empty(),
            Err(e) => Response::err(format!("Failed to load file: {}", e)),
        }
    }

    pub(super) async fn handle_play(&self) -> Response {
        let mut manager = self.manager.lock();
        let output_device = self.system_state.lock().selected_output_device();
        match manager.start_playback(output_device, vec![], 2) {
            Ok(_) => Response::ok_empty(),
            Err(e) => Response::err(format!("Failed to start playback: {}", e)),
        }
    }

    pub(super) async fn handle_pause(&self) -> Response {
        let manager = self.manager.lock();
        match manager.pause() {
            Ok(_) => Response::ok_empty(),
            Err(e) => Response::err(format!("Failed to pause: {}", e)),
        }
    }

    pub(super) async fn handle_stop(&self) -> Response {
        // Lock-order invariant: driver_manager -> manager. The config
        // watcher thread also acquires them in this order. Using the
        // `lock_order::lock_with_order_warning` helper turns silent
        // contention with the watcher into a logged warning so a future
        // contributor who introduces an inverse acquisition order has a
        // diagnostic to follow instead of an undetectable deadlock.
        super::lock_order::lock_with_order_warning(&self.driver_manager, "driver_manager")
            .set_engine_ready(false);
        log::debug!("Cleared engine_ready flag via driver");

        let mut manager = super::lock_order::lock_with_order_warning(&self.manager, "manager");
        match manager.stop() {
            Ok(_) => Response::ok_empty(),
            Err(e) => Response::err(format!("Failed to stop: {}", e)),
        }
    }

    pub(super) async fn handle_seek(&self, position: f64) -> Response {
        let manager = self.manager.lock();
        match manager.seek(position) {
            Ok(_) => Response::ok_empty(),
            Err(e) => Response::err(format!("Failed to seek: {}", e)),
        }
    }

    pub(super) async fn handle_set_volume(&self, volume: f32) -> Response {
        let manager = self.manager.lock();
        let _ = manager.set_volume(volume);
        Response::ok_empty()
    }

    pub(super) async fn handle_list_devices(&self) -> Response {
        match list_audio_devices() {
            Ok(devices) => Response::ok(serde_json::json!({ "devices": devices })),
            Err(e) => Response::err(format!("Failed to list devices: {}", e)),
        }
    }

    pub(super) async fn handle_set_device(&self, device: &str) -> Response {
        use cpal::traits::DeviceTrait;
        let is_asio = sotf_audio::devices::is_asio_device(device);
        let host = sotf_audio::devices::get_host_for_device(Some(device));
        let device_name = sotf_audio::devices::strip_asio_prefix(device);

        match sotf_audio::devices::find_device(&host, device_name, false) {
            Ok(cpal_device) => {
                let resolved_name = cpal_device
                    .description()
                    .map(|d| d.name().to_string())
                    .unwrap_or_else(|_| "Unknown Device".to_string());

                if !is_safe_output_device_name(&resolved_name) {
                    log::warn!(
                        "Rejected virtual output device '{}' (requested '{}') to prevent feedback",
                        resolved_name,
                        device
                    );
                    return Response::err(format!(
                        "'{}' is a virtual/loopback device and cannot be used as Systemwide speaker output. Select hardware speakers/headphones here, and select SotF Virtual Audio in macOS Sound Output.",
                        resolved_name
                    ));
                }

                // Store with ASIO prefix preserved so playback thread selects the right host
                let stored_name = if is_asio {
                    format!(
                        "{}{}",
                        sotf_audio::devices::ASIO_DEVICE_PREFIX,
                        resolved_name
                    )
                } else {
                    resolved_name.clone()
                };
                log::info!(
                    "Output device set to: {} (matched from '{}')",
                    resolved_name,
                    device
                );

                let driver_status = self.driver_manager.lock().status();
                let driver_sample_rate = if driver_status.sample_rate > 0 {
                    driver_status.sample_rate
                } else {
                    48_000
                };
                let driver_buffer_frames = if driver_status.buffer_frames > 0 {
                    driver_status.buffer_frames
                } else {
                    512
                };
                let plan = match self
                    .system_state
                    .lock()
                    .prepare_with_selected_device(stored_name.clone())
                {
                    Ok(plan) => plan,
                    Err(e) => return Response::err(e),
                };

                log::info!(
                    "Starting/restarting driver playback with output device: {}",
                    resolved_name
                );
                let resp = self.apply_pipeline_plan(
                    plan,
                    driver_status,
                    driver_sample_rate,
                    driver_buffer_frames,
                );
                if !resp.success {
                    return resp;
                }

                Response::ok_empty()
            }
            Err(e) => {
                log::warn!("Failed to set device '{}': {}", device, e);
                Response::err(format!("Device '{}' not found. {}", device, e))
            }
        }
    }

    pub(super) fn apply_pipeline_plan(
        &self,
        plan: PipelinePlan,
        driver_status: driver_common::DriverStatus,
        driver_sample_rate: u32,
        driver_buffer_frames: u32,
    ) -> Response {
        self.driver_manager.lock().set_engine_ready(false);

        {
            let mut manager = self.manager.lock();
            let _ = manager.stop();
        }

        let mut effective_driver_sample_rate = driver_sample_rate;
        let mut effective_driver_buffer_frames = driver_buffer_frames;

        if driver_status.driver_installed
            && driver_status.channel_count != plan.spec.input_channels as u32
        {
            let result = self.driver_manager.lock().request_config(DriverConfig {
                sample_rate: driver_sample_rate,
                buffer_frames: driver_buffer_frames,
                channel_count: plan.spec.input_channels as u32,
            });

            match result {
                driver_common::ConfigResult::Accepted
                | driver_common::ConfigResult::Negotiated { .. } => {
                    (effective_driver_sample_rate, effective_driver_buffer_frames) =
                        pipeline_timing_after_config_request(
                            &result,
                            driver_sample_rate,
                            driver_buffer_frames,
                        );
                    log::info!(
                        "HAL input channel count set to {} via driver config",
                        plan.spec.input_channels
                    );
                }
                driver_common::ConfigResult::Error(e) => {
                    log::error!("Failed to set HAL input channels: {}", e);
                    return Response::err(format!("Failed to set HAL input channels: {}", e));
                }
            }
        }

        log::info!(
            "Loading driver plugin chain: {} user plugins + 2 monitors = {} total, {}Hz {}ch input, {} output channels, device: {:?}",
            plan.spec.user_plugins.len(),
            plan.runtime_plugins.len(),
            effective_driver_sample_rate,
            plan.spec.input_channels,
            plan.spec.output_channels,
            plan.spec.output_device
        );

        let mut manager = self.manager.lock();
        manager.set_loudness_plugin_index(plan.output_loudness_index);
        let result = manager.start_hal_playback_with_driver_config(
            plan.spec.output_device.clone(),
            plan.runtime_plugins.clone(),
            plan.spec.output_channels,
            effective_driver_sample_rate,
            effective_driver_buffer_frames,
            plan.spec.input_channels,
        );
        drop(manager);

        match result {
            Ok(_) => {
                self.system_state.lock().commit_applied(&plan);
                log::info!("Driver plugin chain loaded successfully");

                self.driver_manager.lock().set_engine_ready(true);
                log::info!("Set engine_ready=true via driver");
                if let Err(e) = self.sync_encryption_to_shared_memory(false) {
                    log::warn!("{}", e);
                }

                Response::ok_empty()
            }
            Err(e) => {
                log::error!("Failed to load driver plugins: {}", e);
                Response::err(format!("Failed to load plugin chain: {}", e))
            }
        }
    }

    pub(super) async fn handle_load_plugins_with_channels(
        &self,
        plugins: Vec<PluginConfig>,
        input_channels: usize,
        output_channels: usize,
    ) -> Response {
        let driver_status = self.driver_manager.lock().status();
        let driver_sample_rate = if driver_status.sample_rate > 0 {
            driver_status.sample_rate
        } else {
            48_000
        };
        let driver_buffer_frames = if driver_status.buffer_frames > 0 {
            driver_status.buffer_frames
        } else {
            512
        };
        let stored_input_channels = self.system_state.lock().input_channels();
        let fallback_input_channels = if driver_status.channel_count > 0 {
            driver_status.channel_count as usize
        } else if stored_input_channels > 0 {
            stored_input_channels
        } else {
            2
        };

        let plan = match self.system_state.lock().prepare_plan(
            plugins,
            input_channels,
            output_channels,
            fallback_input_channels,
        ) {
            Ok(plan) => plan,
            Err(e) => return Response::err(e),
        };

        self.apply_pipeline_plan(
            plan,
            driver_status,
            driver_sample_rate,
            driver_buffer_frames,
        )
    }

    pub(super) async fn handle_load_plugin_artifact(&self, artifact: Value) -> Response {
        match plan_plugin_artifact(artifact) {
            Ok(PluginArtifactPlan::RackChain { plugins }) => {
                let (input_channels, output_channels) = {
                    let state = self.system_state.lock();
                    (state.input_channels(), state.output_channels())
                };
                self.handle_load_plugins_with_channels(plugins, input_channels, output_channels)
                    .await
            }
            Ok(PluginArtifactPlan::UnsupportedGraph { reason }) => Response::err(format!(
                "Unsupported graph plugin artifact: {}. Use a graph-aware loader instead of flattening it into the rack.",
                reason
            )),
            Err(e) => Response::err(format!("Invalid plugin artifact: {}", e)),
        }
    }

    pub(super) async fn handle_set_pipeline_channels(
        &self,
        input_channels: Option<usize>,
        output_channels: Option<usize>,
    ) -> Response {
        if input_channels.is_none() && output_channels.is_none() {
            return Response::err(
                "set_pipeline_channels requires input_channels or output_channels",
            );
        }

        let (plugins, current_input_channels, current_output_channels) = {
            let state = self.system_state.lock();
            (
                state.user_plugins(),
                state.input_channels(),
                state.output_channels(),
            )
        };

        let next_input_channels = input_channels.unwrap_or(current_input_channels);
        let next_output_channels = output_channels.unwrap_or(current_output_channels);

        self.handle_load_plugins_with_channels(plugins, next_input_channels, next_output_channels)
            .await
    }

    pub(super) async fn handle_get_loudness(&self) -> Response {
        let manager = self.manager.lock();
        match manager.get_loudness() {
            Some(loudness) => Response::ok(loudness_info_to_json(&loudness)),
            None => Response::err("Loudness monitoring not enabled"),
        }
    }

    pub(super) async fn handle_get_metering(&self) -> Response {
        Response::ok(self.metering_snapshot())
    }

    // =========================================================================
    // Plugin management handlers
    // =========================================================================

    pub(super) async fn handle_get_plugins(&self) -> Response {
        let plugins = self.system_state.lock().user_plugins();
        let result: Vec<Value> = plugins
            .iter()
            .enumerate()
            .map(|(i, p)| {
                serde_json::json!({
                    "index": i,
                    "plugin_type": p.plugin_type,
                    "parameters": p.parameters,
                })
            })
            .collect();
        Response::ok(serde_json::json!({ "plugins": result }))
    }

    pub(super) async fn handle_get_available_plugins(&self) -> Response {
        let excluded = [
            "loudness_monitor",
            "spectrum_analyzer",
            "resampler",
            "hal_input",
            "hal_output",
            "band_split",
            "band_merge",
            "ab_compare",
            "fletcher_munson",
        ];

        let available: Vec<Value> = PluginType::all()
            .into_iter()
            .filter(|pt| {
                let engine_type = plugin_type_to_engine_str(pt);
                !excluded.contains(&engine_type)
            })
            .map(|pt| {
                let engine_type = plugin_type_to_engine_str(&pt);
                let category = plugin_type_category(&pt);
                let default_settings = sotf_audio::PluginSettings::default_for(&pt);
                let default_parameters = default_settings.to_plugin_config(48_000.0).parameters;
                serde_json::json!({
                    "type": engine_type,
                    "name": pt.name(),
                    "description": pt.description(),
                    "category": category,
                    "maturity": format!("{:?}", pt.maturity()),
                    "default_parameters": default_parameters,
                    "parameters": plugin_parameter_descriptors(&default_settings),
                })
            })
            .collect();

        Response::ok(serde_json::json!({ "plugins": available }))
    }

    pub(super) async fn handle_add_plugin(
        &self,
        plugin: PluginConfig,
        index: Option<usize>,
    ) -> Response {
        let mut plugins = self.system_state.lock().user_plugins();
        match index {
            Some(i) if i <= plugins.len() => plugins.insert(i, plugin),
            _ => plugins.push(plugin),
        }
        self.reload_plugins_with_user_plugins(plugins).await
    }

    pub(super) async fn handle_remove_plugin(&self, index: usize) -> Response {
        let mut plugins = self.system_state.lock().user_plugins();
        if index >= plugins.len() {
            return Response::err(format!(
                "Plugin index {} out of range (have {})",
                index,
                plugins.len()
            ));
        }
        plugins.remove(index);
        self.reload_plugins_with_user_plugins(plugins).await
    }

    pub(super) async fn handle_update_plugin(&self, index: usize, parameters: Value) -> Response {
        let mut plugins = self.system_state.lock().user_plugins();
        if index >= plugins.len() {
            return Response::err(format!(
                "Plugin index {} out of range (have {})",
                index,
                plugins.len()
            ));
        }
        plugins[index].parameters = parameters;
        self.reload_plugins_with_user_plugins(plugins).await
    }

    pub(super) async fn handle_reorder_plugins(&self, order: Vec<usize>) -> Response {
        let plugins = self.system_state.lock().user_plugins();
        let n = plugins.len();

        if order.len() != n {
            return Response::err(format!(
                "Order length {} doesn't match plugin count {}",
                order.len(),
                n
            ));
        }
        let mut seen = vec![false; n];
        for &idx in &order {
            if idx >= n || seen[idx] {
                return Response::err(format!(
                    "Invalid order: duplicate or out-of-range index {}",
                    idx
                ));
            }
            seen[idx] = true;
        }

        let old = plugins.clone();
        let mut reordered = plugins;
        for (new_pos, &old_pos) in order.iter().enumerate() {
            reordered[new_pos] = old[old_pos].clone();
        }
        self.reload_plugins_with_user_plugins(reordered).await
    }

    pub(super) async fn reload_plugins_with_user_plugins(
        &self,
        plugins: Vec<PluginConfig>,
    ) -> Response {
        let prepared_plan = {
            let pipeline = self.system_state.lock();
            pipeline.prepare_plan(
                plugins,
                pipeline.input_channels(),
                pipeline.output_channels(),
                pipeline.input_channels(),
            )
        };
        let plan = match prepared_plan {
            Ok(plan) => plan,
            Err(e) => return Response::err(e),
        };

        let result = {
            let manager = self.manager.lock();
            manager.update_plugin_chain(&plan.runtime_plugins)
        };

        match result {
            Ok(()) => {
                self.manager
                    .lock()
                    .set_loudness_plugin_index(plan.output_loudness_index);
                self.system_state.lock().commit_applied(&plan);
                log::info!("Driver plugin chain hot-updated successfully");
                Response::ok_empty()
            }
            Err(e) if e == "No engine running" => {
                log::info!("No running driver engine; starting driver playback");
                self.handle_load_plugins_with_channels(
                    plan.spec.user_plugins.clone(),
                    plan.spec.input_channels,
                    plan.spec.output_channels,
                )
                .await
            }
            Err(e) => {
                log::error!("Failed to hot-update plugin chain: {}", e);
                Response::err(format!("Failed to update plugin chain: {}", e))
            }
        }
    }

    pub(super) async fn handle_driver_status(&self) -> Response {
        let status = get_driver_status(&self.driver_manager.lock());
        Response::ok(serde_json::json!({
            "platform_supported": status.platform_supported,
            "driver_installed": status.driver_installed,
            "capture_active": status.capture_active,
            "sample_rate": status.sample_rate,
            "channel_count": status.channel_count,
            "buffer_frames": status.buffer_frames,
            "driver_name": status.driver_name,
            // Legacy fields for backward compatibility
            "buffer_initialized": status.capture_active || status.driver_installed,
            "ready": status.platform_supported && status.driver_installed,
        }))
    }

    // =========================================================================
    // Encryption handlers
    // =========================================================================

    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(super) fn sync_encryption_to_shared_memory(&self, flush_audio: bool) -> Result<(), String> {
        let key_manager = self.key_manager.lock();
        Self::apply_encryption_to_shared_memory(&key_manager, flush_audio)
    }

    #[cfg(not(all(target_os = "macos", feature = "hal")))]
    pub(super) fn sync_encryption_to_shared_memory(
        &self,
        _flush_audio: bool,
    ) -> Result<(), String> {
        Ok(())
    }

    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(super) fn apply_encryption_to_shared_memory(
        key_manager: &KeyManager,
        flush_audio: bool,
    ) -> Result<(), String> {
        match driver_hal::SharedAudioBuffer::open_default() {
            Ok(buffer) => {
                if flush_audio {
                    buffer.flush_audio();
                }
                if key_manager.is_enabled() {
                    buffer.set_key_fingerprint(*key_manager.fingerprint());
                }
                buffer.set_encrypted(key_manager.is_enabled());
                buffer.set_config_changed();
                Ok(())
            }
            Err(e) => {
                let message = format!("Failed to sync encryption state to shared memory: {}", e);
                log::warn!("{}", message);
                Err(message)
            }
        }
    }

    pub(super) async fn handle_set_encryption(&self, enabled: bool) -> Response {
        let mut key_manager = self.key_manager.lock();
        key_manager.set_enabled(enabled);

        // On macOS with HAL, update shared memory encryption flag if the HAL
        // shared memory is available. Missing shared memory is normal when the
        // HAL driver is not currently running; the daemon-side encryption state
        // remains set and will be synced when the driver reconnects.
        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            if let Err(e) = Self::apply_encryption_to_shared_memory(&key_manager, true) {
                log::warn!(
                    "Failed to sync encryption state to shared memory (HAL may not be running): {}",
                    e
                );
            }
        }

        Response::ok(serde_json::json!({
            "enabled": key_manager.is_enabled(),
            "fingerprint": key_manager.fingerprint_hex(),
        }))
    }

    pub(super) async fn handle_encryption_status(&self) -> Response {
        let key_manager = self.key_manager.lock();
        let status = key_manager.status();

        Response::ok(serde_json::json!({
            "enabled": status.enabled,
            "fingerprint": status.fingerprint,
            "key_path": status.key_path,
        }))
    }

    pub(super) async fn handle_rotate_encryption_key(&self) -> Response {
        let mut key_manager = self.key_manager.lock();

        match key_manager.force_rotate() {
            Ok(()) => {
                // On macOS with HAL, update shared memory fingerprint if the HAL
                // shared memory is available. Missing shared memory is normal when
                // the HAL driver is not currently running.
                #[cfg(all(target_os = "macos", feature = "hal"))]
                {
                    if let Err(e) = Self::apply_encryption_to_shared_memory(&key_manager, true) {
                        log::warn!(
                            "Failed to sync rotated encryption key to shared memory (HAL may not be running): {}",
                            e
                        );
                    }
                }

                Response::ok(serde_json::json!({
                    "fingerprint": key_manager.fingerprint_hex(),
                }))
            }
            Err(e) => Response::err(format!("Failed to rotate key: {}", e)),
        }
    }

    // =========================================================================
    // Driver config handlers
    // =========================================================================

    pub(super) async fn handle_set_sample_rate(&self, rate: u32) -> Response {
        const SUPPORTED: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];

        if !SUPPORTED.contains(&rate) {
            return Response::err(format!(
                "Unsupported sample rate: {}. Supported: {:?}",
                rate, SUPPORTED
            ));
        }

        let manager = self.manager.lock();
        let state = manager.get_state();
        drop(manager);

        if state != sotf_audio::manager::StreamingState::Idle {
            log::warn!(
                "Cannot change sample rate during active playback, will apply on next start"
            );
        }

        let mut driver = self.driver_manager.lock();
        let result = driver.request_config(DriverConfig {
            sample_rate: rate,
            buffer_frames: 0, // Keep current
            channel_count: 0, // Keep current
        });

        match result {
            driver_common::ConfigResult::Accepted
            | driver_common::ConfigResult::Negotiated { .. } => {
                log::info!("Sample rate set to {}Hz via driver", rate);
                Response::ok(serde_json::json!({ "sample_rate": rate }))
            }
            driver_common::ConfigResult::Error(e) => {
                Response::err(format!("Failed to set sample rate: {}", e))
            }
        }
    }

    pub(super) async fn handle_set_buffer_frames(&self, frames: u32) -> Response {
        if !(64..=4096).contains(&frames) {
            return Response::err(format!(
                "Buffer frames must be between 64 and 4096, got: {}",
                frames
            ));
        }

        let mut driver = self.driver_manager.lock();
        let result = driver.request_config(DriverConfig {
            sample_rate: 0, // Keep current
            buffer_frames: frames,
            channel_count: 0, // Keep current
        });

        match result {
            driver_common::ConfigResult::Accepted
            | driver_common::ConfigResult::Negotiated { .. } => {
                log::info!("Buffer frames set to {} via driver", frames);
                Response::ok(serde_json::json!({ "buffer_frames": frames }))
            }
            driver_common::ConfigResult::Error(e) => {
                Response::err(format!("Failed to set buffer frames: {}", e))
            }
        }
    }

    pub(super) async fn handle_get_driver_config(&self) -> Response {
        let driver = self.driver_manager.lock();
        let status = driver.status();

        Response::ok(serde_json::json!({
            "sample_rate": status.sample_rate,
            "actual_sample_rate": status.sample_rate,
            "buffer_frames": status.buffer_frames,
            "actual_buffer_frames": status.buffer_frames,
            "channel_count": status.channel_count,
            "active": status.capture_active,
            "driver_name": status.driver_name,
            "driver_installed": status.driver_installed,
            "driver_ready": status.driver_ready,
            "platform_supported": status.platform_supported,
        }))
    }

    pub(super) fn handle_client(&self, mut stream: UnixStream, peer_class: PeerClass) {
        if let Err(e) = stream.set_read_timeout(Some(std::time::Duration::from_secs(
            super::consts::IPC_CLIENT_IDLE_TIMEOUT_SECS,
        ))) {
            log::warn!("Failed to set IPC client idle timeout: {}", e);
        }
        let reader_stream = match stream.try_clone() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to clone stream for reading: {}", e);
                return;
            }
        };
        let mut reader = BufReader::new(reader_stream);
        let mut line = Vec::new();

        loop {
            match read_ipc_line_bounded(&mut reader, &mut line) {
                Ok(IpcLine::Eof) => break,
                Ok(IpcLine::Empty) => continue,
                Ok(IpcLine::TooLarge) => {
                    let response = Response::err("Request too large");
                    let json = serialize_response_safely(&response);
                    let _ = writeln!(stream, "{}", json);
                    break;
                }
                Ok(IpcLine::InvalidUtf8) => {
                    let response = Response::err("Invalid UTF-8 in command");
                    let json = serialize_response_safely(&response);
                    if let Err(e) = writeln!(stream, "{}", json) {
                        log::error!("Failed to write response: {}", e);
                        break;
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    log::debug!("Closing idle IPC client after read timeout");
                    break;
                }
                Err(e) => {
                    log::warn!("IPC client read failed: {}", e);
                    break;
                }
                Ok(IpcLine::Line(command_line)) => {
                    let response = match serde_json::from_str::<Command>(&command_line) {
                        Ok(cmd) => {
                            // Defense-in-depth: gate which commands the
                            // peer's UID class may invoke. The macOS HAL
                            // (UID 202) is authenticated but should only
                            // be allowed to query status -- NOT issue
                            // arbitrary plugin loads, shutdowns, etc.
                            if !peer_allows_command(peer_class, cmd.name()) {
                                log::warn!(
                                    "Rejecting command '{}' from peer class {:?}: not allowed",
                                    cmd.name(),
                                    peer_class
                                );
                                Response::err(format!(
                                    "Command '{}' not permitted for this peer",
                                    cmd.name()
                                ))
                            } else {
                                self.runtime.block_on(self.handle_command(cmd))
                            }
                        }
                        Err(e) => Response::err(format!("Invalid command: {}", e)),
                    };

                    // Hot-path IPC writer: serialization can fail if a
                    // client managed to inject NaN / Infinity into a
                    // `Value::Number` via UpdatePlugin parameters that
                    // gets reflected back through get_plugins. Never
                    // panic the client thread -- emit a static, safe
                    // fallback instead.
                    let json = serialize_response_safely(&response);
                    if let Err(e) = writeln!(stream, "{}", json) {
                        log::error!("Failed to write response: {}", e);
                        break;
                    }
                }
            }
        }
    }

    pub(super) fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let socket_path = get_socket_path();

        // Ensure socket directory exists with secure permissions
        ensure_secure_socket_dir(&socket_path)?;

        // Start driver config watcher thread
        let config_watcher = {
            let driver_manager = Arc::clone(&self.driver_manager);
            let audio_manager = Arc::clone(&self.manager);
            let running = Arc::clone(&self.running);
            let pipeline = Arc::clone(&self.system_state);
            spawn_driver_config_watcher(driver_manager, audio_manager, running, pipeline)
        };

        // Bind the socket. To avoid a TOCTOU race window between an
        // existence check and a follow-up unlink (which would allow a
        // same-UID hostile actor to swap in their own socket or unrelated
        // file at the path), we try `bind()` first and only fall back to
        // unlinking when we have positively confirmed the existing entry
        // is a stale `AF_UNIX` socket -- never a regular file, FIFO, or
        // symlink. See `bind_unix_socket` below for the full strategy.
        let listener = bind_unix_socket(&socket_path)?;
        println!("Audio daemon listening on {}", socket_path.display());

        // NOTE: the legacy `/tmp/autoeq_audio.sock` symlink that previous
        // versions of the daemon created on each startup has been
        // removed. `/tmp` is world-writable on macOS/Linux, and the prior
        // `remove_file(LEGACY_SOCKET_PATH)` would happily unlink whatever
        // a same-host attacker pre-staged at that path (regular file,
        // FIFO, symlink-to-/etc/passwd, etc.). The `SOTF_LEGACY_SOCKET`
        // opt-in still works for callers that *must* use the legacy
        // path: they get a real socket bound at `LEGACY_SOCKET_PATH`,
        // not a symlink. New clients should use `get_secure_socket_path`.
        let _ = LEGACY_SOCKET_PATH; // keep the constant referenced

        // Accept connections (non-blocking so Ctrl-C can interrupt)
        listener.set_nonblocking(true)?;
        self.spawn_initial_driver_playback();

        loop {
            if !*self.running.lock() {
                println!("Shutdown requested, exiting");
                break;
            }

            match listener.accept() {
                Ok((stream, _addr)) => {
                    if let Err(e) = stream.set_nonblocking(false) {
                        log::error!("Failed to set client stream to blocking: {}", e);
                        continue;
                    }

                    let peer_class = match verify_peer_credentials(&stream) {
                        Ok(peer_uid) => {
                            let class = classify_peer(peer_uid, security_current_uid());
                            log::debug!(
                                "Accepted connection from UID {} (class {:?})",
                                peer_uid,
                                class
                            );
                            class
                        }
                        Err(e) => {
                            log::warn!("Rejected unauthorized connection: {}", e);
                            continue;
                        }
                    };

                    // Clone daemon for client thread
                    let daemon = AudioDaemon {
                        manager: Arc::clone(&self.manager),
                        running: Arc::clone(&self.running),
                        driver_manager: Arc::clone(&self.driver_manager),
                        system_state: Arc::clone(&self.system_state),
                        key_manager: Arc::clone(&self.key_manager),
                        runtime: Arc::clone(&self.runtime),
                    };

                    std::thread::spawn(move || {
                        daemon.handle_client(stream, peer_class);
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => {
                    log::error!("Failed to accept connection: {}", e);
                }
            }
        }

        // Cleanup -- only remove our own socket entry, after re-verifying
        // it is still a socket. We deliberately do NOT unlink the legacy
        // `/tmp/autoeq_audio.sock` here: if it exists and is not ours,
        // it's not our business to remove (avoid the prior TOCTOU /
        // symlink-following hazard at shutdown).
        if socket_is_unix_socket(&socket_path) {
            let _ = std::fs::remove_file(&socket_path);
        }

        let _ = config_watcher.join();

        Ok(())
    }
}
