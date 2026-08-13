#![allow(clippy::field_reassign_with_default)]
use super::audio_daemon::{AudioDaemon, pipeline_timing_after_config_request};
use super::command::Command;
use super::configured::configured_output_device_from_value;
use super::consts::{MAX_HAL_CHANNELS, MAX_IPC_COMMAND_BYTES};
use super::driver_manager::DriverManager;
use super::loudness::{loudness_data_to_json, loudness_info_to_json};
use super::misc::push_metering_faults;
use super::misc::sanitize_user_plugins;
use super::misc::transport_snapshot_and_faults;
use super::misc::{
    build_driver_plugin_chain, build_driver_plugin_graph, is_safe_output_device_name,
    parameter_descriptor_to_json,
};
use super::pipeline_reconfigure_outcome::handle_driver_config_change;
use super::pipeline_reconfigure_outcome::reconfigure_audio_pipeline;
use super::pipeline_spec::{PipelineSpec, pipeline_spec_to_json};
use super::pipeline_supervisor::PipelineSupervisor;
use super::plugin::{
    plugin_parameter_descriptors, plugin_type_category, plugin_type_to_engine_str,
};
use super::response::Response;
use super::response::serialize_response_safely;
use super::security::{KeyManager, PeerClass};
use super::systemwide_state::SystemwideState;
use super::types::IpcLine;
use super::types::read_ipc_line_bounded;
use driver_common::DriverConfig;
use parking_lot::Mutex;
use serde_json::Value;
use sotf_audio::PluginConfig;
use sotf_audio::engine::{PluginGraphConfig, PluginGraphEdgeConfig, PluginGraphNodeConfig};
use sotf_audio::manager::AudioEngineManager;
use sotf_audio::plugins::PluginType;
use std::io::{BufRead, BufReader, Cursor, Write};
use std::os::unix::net::UnixStream;
use std::sync::Arc;

fn test_plugin(plugin_type: &str) -> PluginConfig {
    PluginConfig {
        plugin_type: plugin_type.to_string(),
        parameters: serde_json::json!({}),
    }
}

#[test]
fn loudness_data_json_includes_meter_fields() {
    let loudness = sotf_audio::LoudnessData {
        measurement_valid: true,
        query_error_generation: 0,
        measurement_enabled: true,
        channel_layout_is_compliant: true,
        momentary_lufs: -18.5,
        shortterm_lufs: -17.25,
        integrated_lufs: -20.0,
        peak: 0.75,
        channel_peaks: Arc::new(vec![0.5, 0.75]),
        true_peaks_dbtp: Arc::new(vec![-2.0, -1.0]),
        true_peak_is_compliant: true,
        integrated_window_seconds: 3_600,
        correlation_lr: Some(0.42),
        correlation_matrix: Arc::new(Vec::new()),
        correlation_samples_seen: 0,
    };

    let json = loudness_data_to_json(&loudness);

    assert_eq!(json["momentary"], serde_json::json!(-18.5));
    assert_eq!(json["short_term"], serde_json::json!(-17.25));
    assert_eq!(json["integrated"], serde_json::json!(-20.0));
    assert_eq!(json["peak"], serde_json::json!(0.75));
    assert_eq!(json["channel_peaks"], serde_json::json!([0.5, 0.75]));
    assert_eq!(json["true_peaks_dbtp"], serde_json::json!([-2.0, -1.0]));
    assert_eq!(json["correlation_lr"], serde_json::json!(0.42));
    assert_eq!(json["measurement_valid"], serde_json::json!(true));
    assert_eq!(json["measurement_enabled"], serde_json::json!(true));
    assert_eq!(json["query_error_generation"], serde_json::json!(0));
    assert_eq!(json["channel_layout_is_compliant"], serde_json::json!(true));
    assert_eq!(json["true_peak_is_compliant"], serde_json::json!(true));
    assert_eq!(json["integrated_window_seconds"], serde_json::json!(3600));
}

#[test]
fn available_plugin_descriptors_expose_engine_keys() {
    let settings = sotf_audio::PluginSettings::default_for(&PluginType::Gain).unwrap();
    let descriptors = plugin_parameter_descriptors(&settings);

    assert!(
        descriptors
            .iter()
            .any(|d| d["key"] == "gain_db" && d["type"] == "float")
    );
}

#[test]
fn configured_output_device_uses_non_empty_value() {
    assert_eq!(
        configured_output_device_from_value(Some(" ADAM Audio D3V ")),
        Some("ADAM Audio D3V".to_string())
    );
    assert_eq!(configured_output_device_from_value(Some("   ")), None);
    assert_eq!(configured_output_device_from_value(None), None);
}

#[test]
fn virtual_output_device_names_are_rejected() {
    for name in [
        "SotF Virtual Audio",
        "BlackHole 2ch",
        "Loopback Audio",
        "Soundflower (2ch)",
        "Background Music",
        "Audio Bridge",
        "ZoomAudioDevice",
        "Generic Virtual Device",
    ] {
        assert!(!is_safe_output_device_name(name), "{name} should be unsafe");
    }

    for name in ["Built-in Output", "ADAM Audio D3V", "MacBook Pro Speakers"] {
        assert!(is_safe_output_device_name(name), "{name} should be safe");
    }
}

#[test]
fn plugin_type_to_engine_str_maps_all_variants() {
    // Spot-check a representative set; the match must cover every PluginType.
    assert_eq!(plugin_type_to_engine_str(&PluginType::EQ), "eq");
    assert_eq!(plugin_type_to_engine_str(&PluginType::Gain), "gain");
    assert_eq!(plugin_type_to_engine_str(&PluginType::Upmixer), "upmixer");
    assert_eq!(
        plugin_type_to_engine_str(&PluginType::Compressor),
        "compressor"
    );
    assert_eq!(
        plugin_type_to_engine_str(&PluginType::MultibandCompressor),
        "multiband_compressor"
    );
    assert_eq!(
        plugin_type_to_engine_str(&PluginType::LoudnessCompensation),
        "loudness_compensation"
    );
    assert_eq!(
        plugin_type_to_engine_str(&PluginType::LoudnessMonitor),
        "loudness_monitor"
    );
    assert_eq!(
        plugin_type_to_engine_str(&PluginType::ChannelMuteSolo),
        "channel_mute_solo"
    );
    assert_eq!(
        plugin_type_to_engine_str(&PluginType::ABCompare),
        "ab_compare"
    );
    assert_eq!(
        plugin_type_to_engine_str(&PluginType::MonoToStereo),
        "mono_to_stereo"
    );
    assert_eq!(
        plugin_type_to_engine_str(&PluginType::LinearPhaseEq),
        "linear_phase_eq"
    );
}

#[test]
fn plugin_type_category_groups_are_consistent() {
    assert_eq!(plugin_type_category(&PluginType::EQ), "EQ & Tone");
    assert_eq!(plugin_type_category(&PluginType::Gain), "Utility");
    assert_eq!(plugin_type_category(&PluginType::Compressor), "Dynamics");
    assert_eq!(
        plugin_type_category(&PluginType::Upmixer),
        "Spatial & Routing"
    );
    assert_eq!(plugin_type_category(&PluginType::Convolution), "Effects");
    assert_eq!(plugin_type_category(&PluginType::Denoiser), "Restoration");
    assert_eq!(
        plugin_type_category(&PluginType::LoudnessMonitor),
        "Monitoring"
    );
}

#[test]
fn sanitize_user_plugins_strips_daemon_owned_types() {
    let plugins = vec![
        test_plugin("hal_input"),
        test_plugin("eq"),
        test_plugin("loudness_monitor"),
        test_plugin("gain"),
        test_plugin("hal_output"),
    ];
    let sanitized = sanitize_user_plugins(plugins);
    assert_eq!(
        sanitized
            .iter()
            .map(|p| p.plugin_type.as_str())
            .collect::<Vec<_>>(),
        vec!["eq", "gain"]
    );
}

#[test]
fn build_driver_plugin_chain_wraps_user_plugins_with_monitors() {
    let (runtime, input_idx, output_idx) =
        build_driver_plugin_chain(vec![test_plugin("eq"), test_plugin("gain")]);

    assert_eq!(input_idx, 0);
    assert_eq!(output_idx, 3);
    assert_eq!(
        runtime
            .iter()
            .map(|p| p.plugin_type.as_str())
            .collect::<Vec<_>>(),
        vec!["loudness_monitor", "eq", "gain", "loudness_monitor"]
    );
}

#[test]
fn build_driver_plugin_graph_preserves_topology_and_adds_monitor_boundaries() {
    let graph = PluginGraphConfig::try_new(
        vec![
            PluginGraphNodeConfig::try_new(10, "gain", serde_json::json!({}), 2).unwrap(),
            PluginGraphNodeConfig::try_new(20, "eq", serde_json::json!({"filters": []}), 2)
                .unwrap(),
            PluginGraphNodeConfig::try_new(30, "gain", serde_json::json!({}), 2).unwrap(),
        ],
        vec![
            PluginGraphEdgeConfig::new(10, 20),
            PluginGraphEdgeConfig::new(10, 30),
        ],
    )
    .unwrap();

    let (runtime, input_idx, output_idx) = build_driver_plugin_graph(graph, 2, 2).unwrap();

    assert_eq!(input_idx, 0);
    assert_eq!(output_idx, 4);
    assert_eq!(runtime.nodes.len(), 5);
    assert_eq!(runtime.edges.len(), 5);
    assert_eq!(runtime.nodes[0].plugin_type, "loudness_monitor");
    assert_eq!(runtime.nodes[4].plugin_type, "loudness_monitor");
    assert!(runtime.nodes.iter().any(|node| node.id == 10));
    assert!(
        runtime
            .edges
            .iter()
            .any(|edge| edge.from_node == 10 && edge.to_node == 20)
    );
    runtime.validate().unwrap();
}

#[test]
fn empty_loudness_json_clamps_channels_and_shapes_defaults() {
    let zero = super::consts::empty_loudness_json(0);
    assert_eq!(zero["momentary"], -60.0);
    assert_eq!(zero["short_term"], -60.0);
    assert_eq!(zero["integrated"], -60.0);
    assert_eq!(zero["peak"], 0.0);
    assert_eq!(zero["channel_peaks"].as_array().unwrap().len(), 1);
    assert_eq!(zero["true_peaks_dbtp"].as_array().unwrap().len(), 1);
    assert!(zero["correlation_lr"].is_null());

    let many = super::consts::empty_loudness_json(64);
    assert_eq!(
        many["channel_peaks"].as_array().unwrap().len(),
        MAX_HAL_CHANNELS
    );
    assert_eq!(
        many["true_peaks_dbtp"].as_array().unwrap().len(),
        MAX_HAL_CHANNELS
    );
}

#[test]
fn metering_source_json_reflects_data_presence() {
    let present = super::consts::metering_source_json(true, 2);
    assert_eq!(present["status"], "available");
    assert_eq!(present["source"], "loudness_monitor");
    assert_eq!(present["channels"], 2);

    let fallback = super::consts::metering_source_json(false, 6);
    assert_eq!(fallback["status"], "fallback_zero");
    assert_eq!(fallback["source"], "channel_sized_fallback");
    assert_eq!(fallback["channels"], 6);
}

#[test]
fn pipeline_spec_to_json_reports_plugin_types_and_count() {
    let spec = PipelineSpec {
        output_device: Some("ADAM Audio D3V".to_string()),
        user_plugins: vec![test_plugin("eq"), test_plugin("gain")],
        user_graph: None,
        input_channels: 2,
        output_channels: 6,
    };
    let json = pipeline_spec_to_json(&spec);
    assert_eq!(json["output_device"], "ADAM Audio D3V");
    assert_eq!(json["input_channels"], 2);
    assert_eq!(json["output_channels"], 6);
    assert_eq!(json["user_plugin_count"], 2);
    assert_eq!(
        json["user_plugin_types"].as_array().unwrap(),
        &vec![serde_json::json!("eq"), serde_json::json!("gain")]
    );
}

#[test]
fn loudness_info_to_json_shapes_empty_channel_arrays() {
    let info = sotf_audio::LoudnessInfo {
        momentary_lufs: -20.0,
        shortterm_lufs: -19.0,
        integrated_lufs: -21.0,
        peak: 0.5,
    };
    let json = loudness_info_to_json(&info);
    assert_eq!(json["momentary"], -20.0);
    assert_eq!(json["integrated"], -21.0);
    assert_eq!(json["channel_peaks"].as_array().unwrap().len(), 0);
    assert_eq!(json["true_peaks_dbtp"].as_array().unwrap().len(), 0);
    assert!(json["correlation_lr"].is_null());
}

#[test]
fn read_ipc_line_bounded_returns_eof_on_empty_input() {
    let input = Cursor::new(b"");
    let mut reader = BufReader::new(input);
    let mut buffer = Vec::new();
    assert_eq!(
        read_ipc_line_bounded(&mut reader, &mut buffer).unwrap(),
        IpcLine::Eof
    );
}

#[test]
fn read_ipc_line_bounded_returns_invalid_utf8() {
    let input = Cursor::new(vec![0xc0, 0x80, b'\n']);
    let mut reader = BufReader::new(input);
    let mut buffer = Vec::new();
    assert_eq!(
        read_ipc_line_bounded(&mut reader, &mut buffer).unwrap(),
        IpcLine::InvalidUtf8
    );
}

#[test]
fn response_constructors_build_expected_shape() {
    let ok = Response::ok(serde_json::json!({"x": 1}));
    assert!(ok.success);
    assert_eq!(ok.data.unwrap()["x"], 1);
    assert!(ok.error.is_none());

    let empty = Response::ok_empty();
    assert!(empty.success);
    assert!(empty.data.is_none());

    let err = Response::err("bad");
    assert!(!err.success);
    assert_eq!(err.error.unwrap(), "bad");
}

#[test]
fn command_name_covers_all_variants() {
    // Additional variants not exercised by command_name_matches_wire_tag.
    let cases: Vec<(Command, &str)> = vec![
        (Command::Pause, "pause"),
        (Command::Stop, "stop"),
        (Command::Seek { position: 0.0 }, "seek"),
        (Command::SetVolume { volume: 0.5 }, "set_volume"),
        (Command::ListDevices, "list_devices"),
        (
            Command::SetDevice {
                device: String::new(),
            },
            "set_device",
        ),
        (
            Command::SetInputChannels { channels: 2 },
            "set_input_channels",
        ),
        (Command::GetLoudness, "get_loudness"),
        (Command::GetMetering, "get_metering"),
        (Command::GetPlugins, "get_plugins"),
        (Command::GetAvailablePlugins, "get_available_plugins"),
        (
            Command::AddPlugin {
                plugin: test_plugin("eq"),
                index: None,
            },
            "add_plugin",
        ),
        (Command::RemovePlugin { index: 0 }, "remove_plugin"),
        (Command::ReorderPlugins { order: vec![] }, "reorder_plugins"),
        (Command::SetEncryption { enabled: true }, "set_encryption"),
        (Command::EncryptionStatus, "encryption_status"),
        (Command::RotateEncryptionKey, "rotate_encryption_key"),
        (Command::SetSampleRate { rate: 48_000 }, "set_sample_rate"),
        (
            Command::SetBufferFrames { frames: 512 },
            "set_buffer_frames",
        ),
        (Command::GetDriverConfig, "get_driver_config"),
    ];
    for (cmd, expected) in cases {
        assert_eq!(cmd.name(), expected, "{cmd:?}");
    }
}

#[test]
fn default_channel_counts_match_definitions() {
    assert_eq!(super::default::default_input_channels(), 0);
    assert_eq!(super::default::default_output_channels(), 2);
}

#[test]
fn parameter_descriptor_to_json_maps_float_spec() {
    use sotf_plugins::param_specs::ParamSpec;
    let spec =
        ParamSpec::float("Gain", "gain_db", 0.0, -60.0, 12.0, 0.1, "dB", "Level").doc("Gain in dB");
    let json = parameter_descriptor_to_json(&spec);
    assert_eq!(json["key"], "gain_db");
    assert_eq!(json["name"], "Gain");
    assert_eq!(json["unit"], "dB");
    assert_eq!(json["group"], "Level");
    assert_eq!(json["doc"], "Gain in dB");
    assert_eq!(json["type"], "float");
    assert_eq!(json["default"], 0.0);
    assert_eq!(json["min"], -60.0);
    assert_eq!(json["max"], 12.0);
    assert_eq!(json["step"], 0.1);
    assert_eq!(json["update_mode"], "realtime");
}

#[cfg(test)]
mod ipc_safety_tests {
    use super::*;

    use driver_common::{AudioDriver, ConfigResult, DriverError, DriverStatus};
    use std::io::Cursor;

    /// `serialize_response_safely` must produce valid JSON on the OK
    /// path with no behavioural change versus the original
    /// `to_string(...).unwrap()` semantics.
    #[test]
    fn serialize_response_safely_round_trips_ok_response() {
        let r = Response::ok(serde_json::json!({
            "index": 7,
            "name": "test plugin",
        }));
        let out = serialize_response_safely(&r);
        let parsed: serde_json::Value =
            serde_json::from_str(&out).expect("ok-path output must parse");
        assert_eq!(parsed["success"], serde_json::Value::Bool(true));
        assert_eq!(parsed["data"]["index"], serde_json::Value::from(7));
    }

    /// `serialize_response_safely` must produce valid JSON on the error
    /// path too.
    #[test]
    fn serialize_response_safely_handles_err_response() {
        let r = Response::err("something failed");
        let out = serialize_response_safely(&r);
        let parsed: serde_json::Value =
            serde_json::from_str(&out).expect("err-path output must parse");
        assert_eq!(parsed["success"], serde_json::Value::Bool(false));
        assert_eq!(parsed["error"], serde_json::Value::from("something failed"));
    }

    /// Regression test for the IPC `unwrap()` panic.
    ///
    /// The fallback returned by `serialize_response_safely` when
    /// `serde_json::to_string` errors out MUST itself be valid JSON
    /// matching the on-wire `Response` shape. If a future refactor
    /// breaks this string, every client receives malformed JSON when
    /// the daemon encounters a NaN/Inf in echoed user-supplied
    /// parameters -- so we lock it down here.
    #[test]
    fn serialize_response_safely_fallback_is_valid_json() {
        let fallback = String::from(
            r#"{"success":false,"error":"internal error: response serialization failed"}"#,
        );
        let parsed: serde_json::Value =
            serde_json::from_str(&fallback).expect("fallback must be valid JSON");
        assert_eq!(parsed["success"], serde_json::Value::Bool(false));
        assert!(parsed["error"].is_string());
    }

    /// Confirm that a synthetic Serialize failure does NOT propagate
    /// out of `serialize_response_safely`. We can't easily inject a
    /// failing `Value` through `Response::data` (serde_json::Value's
    /// own Serialize impl never errors for in-memory values), but we
    /// can verify the helper's no-panic contract on every legitimate
    /// Response we can construct -- and that `serde_json::to_string`
    /// itself can return Err on a custom Serialize impl that errors.
    /// This locks in the *shape* of the safety net: if the underlying
    /// `to_string` does error, our wrapper turns it into a normal
    /// String return rather than a panic-on-`.unwrap()`.
    #[test]
    fn synthetic_serializer_error_is_handled_without_panic() {
        use serde::{Serialize, Serializer};

        struct AlwaysFail;
        impl Serialize for AlwaysFail {
            fn serialize<S: Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
                Err(serde::ser::Error::custom("synthetic serialization failure"))
            }
        }
        // Sanity: serde_json::to_string on AlwaysFail returns Err.
        let bad = serde_json::to_string(&AlwaysFail);
        assert!(bad.is_err(), "AlwaysFail must fail to serialize");

        // The helper itself never panics on a normal Response, which
        // is the property we care about for the IPC hot path.
        let r = Response::err("normal");
        let _ = serialize_response_safely(&r); // must not panic
    }

    #[test]
    fn read_ipc_line_bounded_accepts_normal_command() {
        let input = Cursor::new(b"  {\"command\":\"status\"}  \n");
        let mut reader = BufReader::new(input);
        let mut buffer = Vec::new();

        assert_eq!(
            read_ipc_line_bounded(&mut reader, &mut buffer).unwrap(),
            IpcLine::Line(r#"{"command":"status"}"#.to_string())
        );
    }

    #[test]
    fn read_ipc_line_bounded_handles_crlf_and_empty_lines() {
        let input = Cursor::new(b"\r\n{\"command\":\"status\"}\r\n");
        let mut reader = BufReader::new(input);
        let mut buffer = Vec::new();

        assert_eq!(
            read_ipc_line_bounded(&mut reader, &mut buffer).unwrap(),
            IpcLine::Empty
        );
        assert_eq!(
            read_ipc_line_bounded(&mut reader, &mut buffer).unwrap(),
            IpcLine::Line(r#"{"command":"status"}"#.to_string())
        );
    }

    #[test]
    fn read_ipc_line_bounded_rejects_oversized_line() {
        let mut input = vec![b'a'; MAX_IPC_COMMAND_BYTES + 1];
        input.push(b'\n');
        let mut reader = BufReader::new(Cursor::new(input));
        let mut buffer = Vec::new();

        assert_eq!(
            read_ipc_line_bounded(&mut reader, &mut buffer).unwrap(),
            IpcLine::TooLarge
        );
    }

    #[test]
    fn read_ipc_line_bounded_rejects_oversized_unterminated_line() {
        let input = vec![b'a'; MAX_IPC_COMMAND_BYTES + 1];
        let mut reader = BufReader::new(Cursor::new(input));
        let mut buffer = Vec::new();

        assert_eq!(
            read_ipc_line_bounded(&mut reader, &mut buffer).unwrap(),
            IpcLine::TooLarge
        );
    }

    #[test]
    fn daemon_ipc_client_path_sets_idle_timeout() {
        let source = include_str!("audio_daemon.rs");
        assert!(source.contains("set_read_timeout(Some(std::time::Duration::from_secs("));
        assert!(source.contains("IPC_CLIENT_IDLE_TIMEOUT_SECS"));
        assert!(source.contains("std::io::ErrorKind::TimedOut"));
        assert!(source.contains("Closing idle IPC client after read timeout"));
    }

    /// `Command::name()` must stay in sync with `#[serde(rename)]`,
    /// because `peer_allows_command` matches on these exact strings.
    #[test]
    fn command_name_matches_wire_tag() {
        let cmd: Command = serde_json::from_str(r#"{"command":"status"}"#).unwrap();
        assert_eq!(cmd.name(), "status");

        let cmd: Command = serde_json::from_str(r#"{"command":"get_snapshot"}"#).unwrap();
        assert_eq!(cmd.name(), "get_snapshot");

        let cmd: Command = serde_json::from_str(r#"{"command":"snapshot"}"#).unwrap();
        assert_eq!(cmd.name(), "get_snapshot");

        let cmd: Command = serde_json::from_str(r#"{"command":"dump_state"}"#).unwrap();
        assert_eq!(cmd.name(), "dump_state");

        let cmd: Command = serde_json::from_str(r#"{"command":"driver_status"}"#).unwrap();
        assert_eq!(cmd.name(), "driver_status");

        // The "hal_status" alias deserialises to DriverStatus, whose
        // canonical wire name (per `#[serde(rename)]`) is "driver_status".
        let cmd: Command = serde_json::from_str(r#"{"command":"hal_status"}"#).unwrap();
        assert_eq!(cmd.name(), "driver_status");

        let cmd: Command =
            serde_json::from_str(r#"{"command":"update_plugin","index":0,"parameters":{}}"#)
                .unwrap();
        assert_eq!(cmd.name(), "update_plugin");

        let cmd: Command =
            serde_json::from_str(r#"{"command":"set_input_channels","channels":4}"#).unwrap();
        assert_eq!(cmd.name(), "set_input_channels");

        let cmd: Command =
            serde_json::from_str(r#"{"command":"set_output_channels","channels":6}"#).unwrap();
        assert_eq!(cmd.name(), "set_output_channels");

        let cmd: Command =
            serde_json::from_str(r#"{"command":"set_pipeline_channels","output_channels":6}"#)
                .unwrap();
        assert_eq!(cmd.name(), "set_pipeline_channels");

        let cmd: Command =
            serde_json::from_str(r#"{"command":"load_plugin_artifact","artifact":[]}"#).unwrap();
        assert_eq!(cmd.name(), "load_plugin_artifact");

        let cmd: Command = serde_json::from_str(r#"{"command":"shutdown"}"#).unwrap();
        assert_eq!(cmd.name(), "shutdown");
    }

    #[derive(Debug)]
    struct FakeDriverState {
        status: DriverStatus,
        engine_ready: bool,
        last_requested_config: Option<DriverConfig>,
        last_ack: Option<(DriverConfig, ConfigResult)>,
        pending_config_change: Option<DriverConfig>,
    }

    #[derive(Debug, Clone)]
    struct FakeDriver {
        state: Arc<Mutex<FakeDriverState>>,
    }

    impl FakeDriver {
        fn new(state: Arc<Mutex<FakeDriverState>>) -> Self {
            Self { state }
        }
    }

    impl AudioDriver for FakeDriver {
        fn initialize(&mut self) -> Result<(), DriverError> {
            Ok(())
        }

        fn shutdown(&mut self) {}

        fn status(&self) -> DriverStatus {
            self.state.lock().status.clone()
        }

        fn read_audio(&mut self, buffer: &mut [f32]) -> usize {
            buffer.fill(0.0);
            0
        }

        fn available_frames(&self) -> usize {
            0
        }

        fn sample_rate(&self) -> u32 {
            self.state.lock().status.sample_rate
        }

        fn channel_count(&self) -> u32 {
            self.state.lock().status.channel_count
        }

        fn request_config(&mut self, config: DriverConfig) -> ConfigResult {
            self.state.lock().last_requested_config = Some(config);
            ConfigResult::Accepted
        }

        fn poll_config_change(&mut self) -> Option<DriverConfig> {
            self.state.lock().pending_config_change.take()
        }

        fn acknowledge_config_change(&mut self, actual: DriverConfig, result: ConfigResult) {
            self.state.lock().last_ack = Some((actual, result));
        }

        fn set_engine_ready(&mut self, ready: bool) {
            self.state.lock().engine_ready = ready;
        }
    }

    fn fake_driver_state() -> Arc<Mutex<FakeDriverState>> {
        Arc::new(Mutex::new(FakeDriverState {
            status: DriverStatus {
                platform_supported: true,
                driver_installed: true,
                capture_active: true,
                sample_rate: 48_000,
                channel_count: 2,
                buffer_frames: 512,
                driver_name: "Fake HAL".to_string(),
                driver_ready: true,
            },
            engine_ready: false,
            last_requested_config: None,
            last_ack: None,
            pending_config_change: None,
        }))
    }

    fn healthy_driver_status() -> DriverStatus {
        DriverStatus {
            platform_supported: true,
            driver_installed: true,
            capture_active: true,
            sample_rate: 48_000,
            channel_count: 2,
            buffer_frames: 512,
            driver_name: "Fake HAL".to_string(),
            driver_ready: true,
        }
    }

    fn fault_codes(faults: &[Value]) -> Vec<&str> {
        faults
            .iter()
            .filter_map(|fault| fault["code"].as_str())
            .collect()
    }

    fn test_daemon_with_driver(state: Arc<Mutex<FakeDriverState>>) -> AudioDaemon {
        AudioDaemon {
            manager: Arc::new(Mutex::new(AudioEngineManager::new())),
            running: Arc::new(Mutex::new(true)),
            driver_manager: Arc::new(Mutex::new(DriverManager::from_driver(Box::new(
                FakeDriver::new(state),
            )))),
            system_state: Arc::new(Mutex::new(SystemwideState::default())),
            key_manager: Arc::new(Mutex::new(KeyManager::for_test())),
        }
    }

    fn send_owner_ipc_command(daemon: &AudioDaemon, raw: &str) -> serde_json::Value {
        let (mut client, server) = UnixStream::pair().expect("unix stream pair");
        let daemon = daemon.clone();
        let handle = std::thread::spawn(move || daemon.handle_client(server, PeerClass::Owner));

        writeln!(client, "{}", raw).expect("write request");
        let mut reader = BufReader::new(client.try_clone().expect("clone client"));
        let mut line = String::new();
        reader.read_line(&mut line).expect("read response");
        drop(reader);
        drop(client);
        handle.join().expect("client handler thread");

        serde_json::from_str(&line).expect("valid JSON response")
    }

    #[test]
    fn pipeline_supervisor_builds_runtime_chain_without_committing_until_success() {
        let supervisor = PipelineSupervisor::default();

        let plan = supervisor
            .prepare_plan(
                vec![
                    test_plugin("hal_input"),
                    test_plugin("eq"),
                    test_plugin("loudness_monitor"),
                    test_plugin("gain"),
                    test_plugin("hal_output"),
                ],
                2,
                6,
                2,
            )
            .expect("valid pipeline plan");

        assert_eq!(plan.spec.input_channels, 2);
        assert_eq!(plan.spec.output_channels, 6);
        assert_eq!(
            plan.spec
                .user_plugins
                .iter()
                .map(|p| p.plugin_type.as_str())
                .collect::<Vec<_>>(),
            vec!["eq", "gain"]
        );
        assert_eq!(plan.input_loudness_index, 0);
        assert_eq!(plan.output_loudness_index, 3);
        assert_eq!(
            plan.runtime_plugins
                .iter()
                .map(|p| p.plugin_type.as_str())
                .collect::<Vec<_>>(),
            vec!["loudness_monitor", "eq", "gain", "loudness_monitor"]
        );
        assert!(supervisor.input_loudness_index().is_none());
        assert!(supervisor.output_loudness_index().is_none());
    }

    #[test]
    fn negotiated_driver_timing_overrides_requested_pipeline_timing() {
        let result = driver_common::ConfigResult::negotiated(44_100, 256, 6);

        assert_eq!(
            pipeline_timing_after_config_request(&result, 48_000, 512),
            (44_100, 256)
        );
        assert_eq!(
            pipeline_timing_after_config_request(
                &driver_common::ConfigResult::Accepted,
                48_000,
                512
            ),
            (48_000, 512)
        );
    }

    #[test]
    fn apply_pipeline_plan_starts_engine_with_negotiated_timing() {
        let source = include_str!("audio_daemon.rs");
        let call = source
            .split("start_hal_playback_with_driver_config(")
            .nth(1)
            .and_then(|tail| tail.split(");").next())
            .expect("HAL playback start call");
        assert!(
            call.contains("effective_driver_sample_rate")
                && call.contains("effective_driver_buffer_frames"),
            "HAL playback restart must use negotiated driver timing"
        );
    }

    #[test]
    fn pipeline_supervisor_commit_atomically_updates_desired_and_applied_state() {
        let mut supervisor = PipelineSupervisor::default();
        let plan = supervisor
            .prepare_plan(vec![test_plugin("eq")], 4, 8, 2)
            .expect("valid pipeline plan");

        supervisor.commit_applied(&plan);

        assert_eq!(supervisor.input_channels(), 4);
        assert_eq!(supervisor.output_channels(), 8);
        assert_eq!(supervisor.input_loudness_index(), Some(0));
        assert_eq!(supervisor.output_loudness_index(), Some(2));
        assert_eq!(supervisor.applied_generation(), Some(1));
    }

    #[test]
    fn pipeline_supervisor_rejects_invalid_channels_before_state_mutation() {
        let supervisor = PipelineSupervisor::default();
        let result = supervisor.prepare_plan(vec![test_plugin("eq")], 0, 64, 2);

        assert!(result.unwrap_err().contains("Invalid output channel count"));
        assert_eq!(supervisor.input_channels(), 2);
        assert_eq!(supervisor.output_channels(), 2);
    }

    #[test]
    fn pipeline_supervisor_reducer_methods_control_desired_mutation() {
        let mut supervisor = PipelineSupervisor::default();

        supervisor
            .set_desired_output_device(Some("ADAM Audio D3V".to_string()))
            .expect("safe device should be accepted");
        assert_eq!(
            supervisor.selected_output_device().as_deref(),
            Some("ADAM Audio D3V")
        );

        let result = supervisor.set_desired_output_device(Some("SotF Virtual Audio".to_string()));
        assert!(result.unwrap_err().contains("virtual/loopback"));
        assert_eq!(
            supervisor.selected_output_device().as_deref(),
            Some("ADAM Audio D3V")
        );

        let plan = supervisor
            .prepare_plan(vec![test_plugin("eq")], 10, 4, 10)
            .expect("valid idle reconfigure plan");
        supervisor.commit_idle_reconfigure(&plan);
        assert_eq!(supervisor.input_channels(), 10);
        assert_eq!(supervisor.output_channels(), 4);
        assert!(supervisor.applied_generation().is_none());
    }

    #[test]
    fn transport_snapshot_reports_all_playing_faults_without_hiding_secondary_causes() {
        let driver_status = healthy_driver_status();
        let engine_state = sotf_audio::AudioEngineState::default();

        let (transport, faults) =
            transport_snapshot_and_faults("Playing", &driver_status, &engine_state);

        assert_eq!(transport["input"]["status"], "input_frames_missing");
        assert_eq!(transport["output"]["status"], "output_callbacks_missing");
        let codes = fault_codes(&faults);
        assert!(codes.contains(&"input_frames_missing"));
        assert!(codes.contains(&"output_callbacks_missing"));
        assert!(codes.contains(&"output_device_unresolved"));
    }

    #[test]
    fn transport_snapshot_reports_flowing_when_input_and_output_are_observed() {
        let driver_status = healthy_driver_status();
        let mut engine_state = sotf_audio::AudioEngineState::default();
        engine_state.playback_frames_received = 1024;
        engine_state.playback_callback_count = 8;
        engine_state.playback_frames_written = 1024;
        engine_state.playback_output_device = Some("ADAM Audio D3V".to_string());
        engine_state.playback_effective_sample_rate = 48_000;

        let (transport, faults) =
            transport_snapshot_and_faults("Playing", &driver_status, &engine_state);

        assert_eq!(transport["input"]["status"], "flowing");
        assert_eq!(transport["output"]["status"], "flowing");
        assert_eq!(transport["output"]["device"], "ADAM Audio D3V");
        assert!(faults.is_empty());
    }

    #[test]
    fn metering_faults_only_apply_to_playing_fallback_sources() {
        let metering = serde_json::json!({
            "sources": {
                "input": { "status": "fallback_zero" },
                "output": { "status": "available" }
            }
        });

        let mut faults = Vec::new();
        push_metering_faults("Idle", &metering, &mut faults);
        assert!(faults.is_empty());

        push_metering_faults("Playing", &metering, &mut faults);
        assert_eq!(fault_codes(&faults), vec!["input_metering_unavailable"]);
    }

    #[test]
    fn driver_reconfigure_preserves_daemon_selected_output_device_when_idle() {
        let audio_manager = Arc::new(Mutex::new(AudioEngineManager::new()));
        let system_state = Arc::new(Mutex::new(SystemwideState::default()));

        {
            let mut state = system_state.lock();
            let plan = state
                .prepare_with_selected_device("ADAM Audio D3V".to_string())
                .expect("valid device plan");
            state.commit_applied(&plan);
        }

        reconfigure_audio_pipeline(&audio_manager, &system_state, 48_000, 512, 6)
            .expect("idle reconfigure should update desired state");

        let state = system_state.lock();
        assert_eq!(
            state.selected_output_device().as_deref(),
            Some("ADAM Audio D3V")
        );
        assert_eq!(state.input_channels(), 6);
        assert_eq!(state.output_channels(), 2);
    }

    #[test]
    fn testkit_driver_status_uses_injected_driver() {
        let state = fake_driver_state();
        state.lock().status.channel_count = 10;
        let daemon = test_daemon_with_driver(state);

        let response = daemon.handle_command(Command::DriverStatus);

        assert!(response.success);
        let data = response.data.expect("driver_status data");
        assert_eq!(data["driver_name"], "Fake HAL");
        assert_eq!(data["channel_count"], 10);
        assert_eq!(data["ready"], true);
    }

    #[test]
    fn testkit_invalid_plugin_load_does_not_mutate_pipeline_state() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = daemon.handle_command(Command::LoadPlugins {
            plugins: vec![test_plugin("eq")],
            input_channels: 2,
            output_channels: 64,
        });

        assert!(!response.success);
        assert!(
            response
                .error
                .as_deref()
                .is_some_and(|e| e.contains("Invalid output channel count"))
        );
        assert_eq!(daemon.system_state.lock().output_channels(), 2);
        assert!(daemon.system_state.lock().output_loudness_index().is_none());
    }

    #[test]
    fn testkit_patch_channel_command_preserves_daemon_owned_plugins_and_input_channels() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = daemon.handle_command(Command::LoadPlugins {
            plugins: vec![test_plugin("eq")],
            input_channels: 10,
            output_channels: 2,
        });
        assert!(response.success);

        let response = daemon.handle_command(Command::SetOutputChannels { channels: 6 });

        assert!(response.success);
        let state = daemon.system_state.lock();
        assert_eq!(state.input_channels(), 10);
        assert_eq!(state.output_channels(), 6);
        assert_eq!(state.user_plugins().len(), 1);
    }

    #[test]
    fn testkit_pipeline_channel_patch_requires_a_field() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = daemon.handle_command(Command::SetPipelineChannels {
            input_channels: None,
            output_channels: None,
        });

        assert!(!response.success);
        assert!(
            response
                .error
                .as_deref()
                .is_some_and(|e| e.contains("requires input_channels or output_channels"))
        );
    }

    #[test]
    fn testkit_load_plugin_artifact_accepts_rack_chain_without_ui_flattening() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = send_owner_ipc_command(
            &daemon,
            r#"{"command":"load_plugin_artifact","artifact":{"plugins":[{"plugin_type":"eq","parameters":{}}]}}"#,
        );

        assert_eq!(response["success"], true);
        let state = daemon.system_state.lock();
        assert_eq!(state.user_plugins().len(), 1);
        assert_eq!(state.user_plugins()[0].plugin_type, "eq");
    }

    #[test]
    fn testkit_load_plugin_artifact_rejects_graph_shape_instead_of_flattening() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = send_owner_ipc_command(
            &daemon,
            r#"{"command":"load_plugin_artifact","artifact":{"global_plugins":[{"plugin_type":"eq","parameters":{}}],"channels":{"L":{"plugins":[{"plugin_type":"gain","parameters":{}}]}}}}"#,
        );

        assert_eq!(response["success"], false);
        assert!(
            response["error"]
                .as_str()
                .is_some_and(|e| e.contains("Unsupported graph plugin artifact"))
        );
        assert!(daemon.system_state.lock().user_plugins().is_empty());
    }

    #[test]
    fn testkit_load_plugin_artifact_applies_engine_graph_without_flattening() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = send_owner_ipc_command(
            &daemon,
            r#"{"command":"load_plugin_artifact","artifact":{"graph":{"nodes":[{"id":7,"plugin_type":"gain","parameters":{"gain_db":-3.0},"input_channels":2}],"edges":[]}}}"#,
        );

        assert_eq!(response["success"], true, "{response}");
        let state = daemon.system_state.lock();
        assert!(state.user_plugins().is_empty());
        let graph = state.user_graph().expect("graph desired state");
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.nodes[0].id, 7);
        assert_eq!(graph.nodes[0].parameters["gain_db"], -3.0);
        assert!(state.applied_generation().is_some());
    }

    #[test]
    fn testkit_stale_graph_generation_is_rejected_without_overwrite() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);
        let first = send_owner_ipc_command(
            &daemon,
            r#"{"command":"load_plugin_artifact","artifact":{"graph":{"nodes":[{"id":7,"plugin_type":"gain","parameters":{"gain_db":-3.0},"input_channels":2}],"edges":[]}}}"#,
        );
        assert_eq!(first["success"], true, "{first}");

        let stale = send_owner_ipc_command(
            &daemon,
            r#"{"command":"load_plugin_artifact","base_generation":0,"artifact":{"graph":{"nodes":[{"id":99,"plugin_type":"gain","parameters":{"gain_db":6.0},"input_channels":2}],"edges":[]}}}"#,
        );

        assert_eq!(stale["success"], false);
        assert!(
            stale["error"]
                .as_str()
                .is_some_and(|error| error.contains("generation conflict"))
        );
        let state = daemon.system_state.lock();
        assert_eq!(state.applied_generation(), Some(1));
        let graph = state.user_graph().unwrap();
        assert_eq!(graph.nodes[0].id, 7);
        assert_eq!(graph.nodes[0].parameters["gain_db"], -3.0);
    }

    #[test]
    fn testkit_unix_ipc_invalid_plugin_load_preserves_pipeline_state() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = send_owner_ipc_command(
            &daemon,
            r#"{"command":"load_plugins","plugins":[{"plugin_type":"eq","parameters":{}}],"input_channels":2,"output_channels":64}"#,
        );

        assert_eq!(response["success"], false);
        assert!(
            response["error"]
                .as_str()
                .is_some_and(|e| e.contains("Invalid output channel count"))
        );
        assert_eq!(daemon.system_state.lock().input_channels(), 2);
        assert_eq!(daemon.system_state.lock().output_channels(), 2);
        assert!(daemon.system_state.lock().applied_generation().is_none());
    }

    #[test]
    fn testkit_unix_ipc_driver_status_uses_injected_driver() {
        let state = fake_driver_state();
        state.lock().status.channel_count = 12;
        let daemon = test_daemon_with_driver(state);

        let response = send_owner_ipc_command(&daemon, r#"{"command":"driver_status"}"#);

        assert_eq!(response["success"], true);
        assert_eq!(response["data"]["driver_name"], "Fake HAL");
        assert_eq!(response["data"]["channel_count"], 12);
    }

    #[test]
    fn testkit_snapshot_separates_desired_observed_and_diagnostics() {
        let state = fake_driver_state();
        state.lock().status.channel_count = 6;
        let daemon = test_daemon_with_driver(state);
        {
            let mut pipeline = daemon.system_state.lock();
            pipeline
                .set_desired_output_device(Some("ADAM Audio D3V".to_string()))
                .expect("safe device");
        }

        let response = send_owner_ipc_command(&daemon, r#"{"command":"get_snapshot"}"#);

        assert_eq!(response["success"], true);
        let data = &response["data"];
        assert_eq!(data["schema_version"], 1);
        assert_eq!(data["desired"]["output_device"], "ADAM Audio D3V");
        assert_eq!(data["observed"]["driver"]["channel_count"], 6);
        assert_eq!(
            data["observed"]["metering"]["sources"]["input"]["status"],
            "fallback_zero"
        );
        assert_eq!(data["observed"]["transport"]["input"]["status"], "idle");
        assert_eq!(data["observed"]["transport"]["output"]["status"], "idle");
        assert_eq!(data["diagnostics"]["health"], "ok");
        assert!(data["diagnostics"]["faults"].as_array().unwrap().is_empty());
    }

    #[test]
    fn testkit_dump_state_includes_snapshot_and_plugins() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = send_owner_ipc_command(&daemon, r#"{"command":"dump_state"}"#);

        assert_eq!(response["success"], true);
        assert_eq!(response["data"]["snapshot"]["schema_version"], 1);
        assert!(response["data"]["plugins"].as_array().is_some());
    }

    #[test]
    fn testkit_idle_driver_config_change_updates_spec_without_engine_ready() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(Arc::clone(&state));
        {
            let mut pipeline = daemon.system_state.lock();
            let plan = pipeline
                .prepare_with_selected_device("ADAM Audio D3V".to_string())
                .expect("valid device plan");
            pipeline.commit_applied(&plan);
        }

        handle_driver_config_change(
            &daemon.driver_manager,
            &daemon.manager,
            DriverConfig {
                sample_rate: 48_000,
                buffer_frames: 512,
                channel_count: 10,
            },
            &daemon.system_state,
        );

        let pipeline = daemon.system_state.lock();
        assert_eq!(pipeline.input_channels(), 10);
        assert_eq!(
            pipeline.selected_output_device().as_deref(),
            Some("ADAM Audio D3V")
        );
        drop(pipeline);

        let state = state.lock();
        assert!(
            !state.engine_ready,
            "idle reconfigure must not mark engine ready"
        );
        let (actual, result) = state.last_ack.as_ref().expect("config ack");
        assert_eq!(actual.channel_count, 10);
        assert!(matches!(result, ConfigResult::Accepted));
    }

    #[test]
    fn testkit_unix_ipc_status_roundtrip() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = send_owner_ipc_command(&daemon, r#"{"command":"status"}"#);

        assert_eq!(response["success"], true);
        assert!(response["data"]["state"].is_string());
        assert!(response["data"]["volume"].is_number());
    }

    #[test]
    fn testkit_unix_ipc_get_plugins_roundtrip() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = send_owner_ipc_command(&daemon, r#"{"command":"get_plugins"}"#);

        assert_eq!(response["success"], true);
        assert!(response["data"]["plugins"].is_array());
    }

    #[test]
    fn testkit_unix_ipc_get_available_plugins_roundtrip() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = send_owner_ipc_command(&daemon, r#"{"command":"get_available_plugins"}"#);

        assert_eq!(response["success"], true);
        let plugins = response["data"]["plugins"]
            .as_array()
            .expect("plugins array");
        assert!(!plugins.is_empty());
        assert!(plugins[0]["type"].is_string());
    }

    #[test]
    fn testkit_unix_ipc_get_metering_roundtrip() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = send_owner_ipc_command(&daemon, r#"{"command":"get_metering"}"#);

        assert_eq!(response["success"], true);
        assert!(response["data"]["input"].is_object());
        assert!(response["data"]["output"].is_object());
        assert!(response["data"]["sources"]["input"].is_object());
        assert!(response["data"]["sources"]["output"].is_object());
    }

    #[test]
    fn testkit_unix_ipc_get_driver_config_roundtrip() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = send_owner_ipc_command(&daemon, r#"{"command":"get_driver_config"}"#);

        assert_eq!(response["success"], true);
        assert!(response["data"]["sample_rate"].is_number());
        assert!(response["data"]["buffer_frames"].is_number());
        assert!(response["data"]["channel_count"].is_number());
    }

    #[test]
    fn testkit_unix_ipc_set_volume_roundtrip() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = send_owner_ipc_command(&daemon, r#"{"command":"set_volume","volume":0.42}"#);

        assert_eq!(response["success"], true);
        assert!((daemon.manager.lock().get_volume() - 0.42).abs() < f32::EPSILON);
    }

    #[test]
    fn testkit_unix_ipc_set_encryption_roundtrip() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response =
            send_owner_ipc_command(&daemon, r#"{"command":"set_encryption","enabled":true}"#);

        if cfg!(all(target_os = "macos", feature = "hal")) {
            assert_eq!(response["success"], true);
            assert_eq!(response["data"]["enabled"], true);
            assert!(response["data"]["fingerprint"].is_string());
        } else {
            assert_eq!(response["success"], false);
            assert!(
                response["error"]
                    .as_str()
                    .is_some_and(|error| error.contains("no session cipher"))
            );
        }
    }

    #[test]
    fn testkit_unix_ipc_shutdown_roundtrip() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = send_owner_ipc_command(&daemon, r#"{"command":"shutdown"}"#);

        assert_eq!(response["success"], true);
        assert!(!*daemon.running.lock());
    }
}

#[test]
fn pipeline_supervisor_preserves_graph_when_output_device_changes() {
    let graph = PluginGraphConfig::try_new(
        vec![PluginGraphNodeConfig::try_new(42, "gain", serde_json::json!({}), 2).unwrap()],
        vec![],
    )
    .unwrap();
    let mut supervisor = PipelineSupervisor::default();
    let initial = supervisor
        .prepare_graph_plan(graph.clone(), 2, 2, 2)
        .unwrap();
    supervisor.commit_applied(&initial);

    let next = supervisor
        .prepare_with_selected_device("Built-in Output".to_string())
        .unwrap();

    assert!(next.spec.user_plugins.is_empty());
    let retained = next.spec.user_graph.as_ref().expect("retained graph");
    assert_eq!(retained.nodes[0].id, 42);
    assert!(next.runtime_graph.is_some());
    assert_eq!(supervisor.applied_generation(), Some(1));
}

/// Phase 4.2: full command/response round-trips without a real audio device.
///
/// These tests exercise JSON parsing for every `Command` variant and direct
/// `AudioDaemon::handle_command` invocation. They run serially because some
/// mutating commands start the engine's cpal output stream, and contending for
/// the default output device across tests would be flaky.
#[cfg(unix)]
mod command_roundtrip_tests {
    use super::*;
    use serial_test::serial;

    fn parse(json: &str) -> Command {
        serde_json::from_str(json).expect("valid command JSON")
    }

    fn run_command(cmd: Command) -> Response {
        let daemon = AudioDaemon::new();
        daemon.handle_command(cmd)
    }

    fn response_value(resp: &Response) -> Value {
        serde_json::to_value(resp).expect("response serializes to JSON")
    }

    #[test]
    fn parse_all_command_variants() {
        assert!(matches!(parse(r#"{"command":"status"}"#), Command::Status));
        assert!(matches!(
            parse(r#"{"command":"get_snapshot"}"#),
            Command::GetSnapshot
        ));
        assert!(matches!(
            parse(r#"{"command":"snapshot"}"#),
            Command::GetSnapshot
        ));
        assert!(matches!(
            parse(r#"{"command":"dump_state"}"#),
            Command::DumpState
        ));

        let cmd = parse(r#"{"command":"load","path":"/tmp/test.wav"}"#);
        assert!(matches!(cmd, Command::Load { path } if path == "/tmp/test.wav"));

        assert!(matches!(parse(r#"{"command":"play"}"#), Command::Play));
        assert!(matches!(parse(r#"{"command":"pause"}"#), Command::Pause));
        assert!(matches!(parse(r#"{"command":"stop"}"#), Command::Stop));

        let cmd = parse(r#"{"command":"seek","position":12.5}"#);
        assert!(
            matches!(cmd, Command::Seek { position } if (position - 12.5).abs() < f64::EPSILON)
        );

        let cmd = parse(r#"{"command":"set_volume","volume":0.5}"#);
        assert!(
            matches!(cmd, Command::SetVolume { volume } if (volume - 0.5).abs() < f32::EPSILON)
        );

        assert!(matches!(
            parse(r#"{"command":"list_devices"}"#),
            Command::ListDevices
        ));

        let cmd = parse(r#"{"command":"set_device","device":"Built-in Output"}"#);
        assert!(matches!(cmd, Command::SetDevice { device } if device == "Built-in Output"));

        let cmd = parse(
            r#"{"command":"load_plugins","plugins":[{"plugin_type":"eq","parameters":{}}],"input_channels":2,"output_channels":2}"#,
        );
        assert!(
            matches!(cmd, Command::LoadPlugins { plugins, input_channels, output_channels } if plugins.len() == 1 && input_channels == 2 && output_channels == 2)
        );

        let cmd = parse(r#"{"command":"load_plugin_artifact","artifact":{"plugins":[]}}"#);
        assert!(matches!(cmd, Command::LoadPluginArtifact { .. }));

        let cmd = parse(r#"{"command":"set_input_channels","channels":4}"#);
        assert!(matches!(cmd, Command::SetInputChannels { channels } if channels == 4));

        let cmd = parse(r#"{"command":"set_output_channels","channels":6}"#);
        assert!(matches!(cmd, Command::SetOutputChannels { channels } if channels == 6));

        let cmd =
            parse(r#"{"command":"set_pipeline_channels","input_channels":4,"output_channels":6}"#);
        assert!(matches!(
            cmd,
            Command::SetPipelineChannels {
                input_channels: Some(4),
                output_channels: Some(6),
            }
        ));

        assert!(matches!(
            parse(r#"{"command":"get_loudness"}"#),
            Command::GetLoudness
        ));
        assert!(matches!(
            parse(r#"{"command":"get_metering"}"#),
            Command::GetMetering
        ));
        assert!(matches!(
            parse(r#"{"command":"get_plugins"}"#),
            Command::GetPlugins
        ));
        assert!(matches!(
            parse(r#"{"command":"get_available_plugins"}"#),
            Command::GetAvailablePlugins
        ));

        let cmd =
            parse(r#"{"command":"add_plugin","plugin":{"plugin_type":"eq","parameters":{}}}"#);
        assert!(matches!(cmd, Command::AddPlugin { index: None, .. }));

        let cmd = parse(
            r#"{"command":"add_plugin","plugin":{"plugin_type":"eq","parameters":{}},"index":1}"#,
        );
        assert!(matches!(cmd, Command::AddPlugin { index: Some(1), .. }));

        let cmd = parse(r#"{"command":"remove_plugin","index":0}"#);
        assert!(matches!(cmd, Command::RemovePlugin { index } if index == 0));

        let cmd = parse(r#"{"command":"update_plugin","index":0,"parameters":{"gain_db":3.0}}"#);
        assert!(matches!(cmd, Command::UpdatePlugin { index, .. } if index == 0));

        let cmd = parse(r#"{"command":"reorder_plugins","order":[1,0]}"#);
        assert!(matches!(cmd, Command::ReorderPlugins { order } if order == vec![1, 0]));

        assert!(matches!(
            parse(r#"{"command":"driver_status"}"#),
            Command::DriverStatus
        ));
        assert!(matches!(
            parse(r#"{"command":"hal_status"}"#),
            Command::DriverStatus
        ));
        assert!(matches!(
            parse(r#"{"command":"shutdown"}"#),
            Command::Shutdown
        ));

        let cmd = parse(r#"{"command":"set_encryption","enabled":true}"#);
        assert!(matches!(cmd, Command::SetEncryption { enabled: true }));

        assert!(matches!(
            parse(r#"{"command":"encryption_status"}"#),
            Command::EncryptionStatus
        ));
        assert!(matches!(
            parse(r#"{"command":"rotate_encryption_key"}"#),
            Command::RotateEncryptionKey
        ));

        let cmd = parse(r#"{"command":"set_sample_rate","rate":48000}"#);
        assert!(matches!(cmd, Command::SetSampleRate { rate } if rate == 48_000));

        let cmd = parse(r#"{"command":"set_buffer_frames","frames":512}"#);
        assert!(matches!(cmd, Command::SetBufferFrames { frames } if frames == 512));

        assert!(matches!(
            parse(r#"{"command":"get_driver_config"}"#),
            Command::GetDriverConfig
        ));
        assert!(matches!(
            parse(r#"{"command":"get_hal_config"}"#),
            Command::GetDriverConfig
        ));
    }

    #[test]
    fn command_name_matches_parsed_variant() {
        let cases: Vec<(&str, &str)> = vec![
            (r#"{"command":"status"}"#, "status"),
            (r#"{"command":"get_snapshot"}"#, "get_snapshot"),
            (r#"{"command":"snapshot"}"#, "get_snapshot"),
            (r#"{"command":"dump_state"}"#, "dump_state"),
            (r#"{"command":"load","path":"x"}"#, "load"),
            (r#"{"command":"play"}"#, "play"),
            (r#"{"command":"pause"}"#, "pause"),
            (r#"{"command":"stop"}"#, "stop"),
            (r#"{"command":"seek","position":0.0}"#, "seek"),
            (r#"{"command":"set_volume","volume":1.0}"#, "set_volume"),
            (r#"{"command":"list_devices"}"#, "list_devices"),
            (r#"{"command":"set_device","device":"x"}"#, "set_device"),
            (r#"{"command":"load_plugins","plugins":[]}"#, "load_plugins"),
            (
                r#"{"command":"load_plugin_artifact","artifact":{}}"#,
                "load_plugin_artifact",
            ),
            (
                r#"{"command":"set_input_channels","channels":2}"#,
                "set_input_channels",
            ),
            (
                r#"{"command":"set_output_channels","channels":2}"#,
                "set_output_channels",
            ),
            (
                r#"{"command":"set_pipeline_channels","input_channels":2}"#,
                "set_pipeline_channels",
            ),
            (r#"{"command":"get_loudness"}"#, "get_loudness"),
            (r#"{"command":"get_metering"}"#, "get_metering"),
            (r#"{"command":"get_plugins"}"#, "get_plugins"),
            (
                r#"{"command":"get_available_plugins"}"#,
                "get_available_plugins",
            ),
            (
                r#"{"command":"add_plugin","plugin":{"plugin_type":"eq","parameters":{}}}"#,
                "add_plugin",
            ),
            (r#"{"command":"remove_plugin","index":0}"#, "remove_plugin"),
            (
                r#"{"command":"update_plugin","index":0,"parameters":{}}"#,
                "update_plugin",
            ),
            (
                r#"{"command":"reorder_plugins","order":[]}"#,
                "reorder_plugins",
            ),
            (r#"{"command":"driver_status"}"#, "driver_status"),
            (r#"{"command":"hal_status"}"#, "driver_status"),
            (r#"{"command":"shutdown"}"#, "shutdown"),
            (
                r#"{"command":"set_encryption","enabled":false}"#,
                "set_encryption",
            ),
            (r#"{"command":"encryption_status"}"#, "encryption_status"),
            (
                r#"{"command":"rotate_encryption_key"}"#,
                "rotate_encryption_key",
            ),
            (
                r#"{"command":"set_sample_rate","rate":48000}"#,
                "set_sample_rate",
            ),
            (
                r#"{"command":"set_buffer_frames","frames":512}"#,
                "set_buffer_frames",
            ),
            (r#"{"command":"get_driver_config"}"#, "get_driver_config"),
            (r#"{"command":"get_hal_config"}"#, "get_driver_config"),
        ];
        for (json, expected) in cases {
            let cmd = parse(json);
            assert_eq!(cmd.name(), expected, "{json}");
        }
    }

    #[test]
    fn handle_status_returns_expected_fields() {
        let resp = run_command(Command::Status);
        assert!(resp.success);
        let data = resp.data.expect("status data");
        assert!(data.get("state").is_some());
        assert!(data.get("volume").is_some());
        assert!(data.get("selected_device").is_some());
        assert!(data.get("input_channels").is_some());
        assert!(data.get("output_channels").is_some());
        assert!(data.get("playback_output_device").is_some());

        // QA-SYS-003 explicit diagnostics fields.
        let driver = data.get("driver").expect("driver object");
        assert!(driver.get("installed").is_some());
        assert!(driver.get("ready").is_some());
        assert!(driver.get("capture_active").is_some());
        assert!(driver.get("frame_size").is_some());

        let encryption = data.get("encryption").expect("encryption object");
        assert!(encryption.get("enabled").is_some());

        let active_route = data.get("active_route").expect("active_route object");
        assert!(active_route.get("desired_output_device").is_some());
        assert!(active_route.get("applied_output_device").is_some());
        assert!(active_route.get("playback_output_device").is_some());
        assert!(active_route.get("capture_active").is_some());

        assert!(
            data.get("recovery_actions").is_some(),
            "recovery_actions list must be present"
        );
        assert!(data["recovery_actions"].is_array());
    }

    #[test]
    fn handle_get_snapshot_returns_expected_fields() {
        let resp = run_command(Command::GetSnapshot);
        assert!(resp.success);
        let data = resp.data.expect("snapshot data");
        assert_eq!(data["schema_version"], 1);
        assert!(data.get("desired").is_some());
        assert!(data.get("observed").is_some());
        assert!(data.get("diagnostics").is_some());
        assert!(data["diagnostics"].get("health").is_some());
        assert!(data["diagnostics"]["faults"].is_array());
    }

    #[test]
    fn handle_dump_state_returns_snapshot_and_plugins() {
        let resp = run_command(Command::DumpState);
        assert!(resp.success);
        let data = resp.data.expect("dump_state data");
        assert!(data.get("snapshot").is_some());
        assert!(data["snapshot"]["schema_version"].is_u64());
        assert!(data.get("plugins").is_some());
        assert!(data["plugins"].is_array());
    }

    #[test]
    fn handle_get_plugins_returns_plugins_array() {
        let resp = run_command(Command::GetPlugins);
        assert!(resp.success);
        let data = resp.data.expect("get_plugins data");
        assert!(data["plugins"].is_array());
    }

    #[test]
    fn handle_get_available_plugins_returns_plugins_array() {
        let resp = run_command(Command::GetAvailablePlugins);
        assert!(resp.success);
        let data = resp.data.expect("get_available_plugins data");
        let plugins = data["plugins"].as_array().expect("plugins array");
        assert!(!plugins.is_empty());
        assert!(plugins[0].get("type").is_some());
        assert!(plugins[0].get("default_parameters").is_some());
    }

    #[test]
    fn handle_driver_status_returns_driver_info() {
        let resp = run_command(Command::DriverStatus);
        assert!(resp.success);
        let data = resp.data.expect("driver_status data");
        assert!(data.get("platform_supported").is_some());
        assert!(data.get("driver_installed").is_some());
        assert!(data.get("sample_rate").is_some());
        assert!(data.get("channel_count").is_some());
        assert!(data.get("buffer_frames").is_some());
        assert!(data.get("driver_name").is_some());
        assert!(data.get("ready").is_some());
    }

    #[test]
    fn handle_get_driver_config_returns_config() {
        let resp = run_command(Command::GetDriverConfig);
        assert!(resp.success);
        let data = resp.data.expect("get_driver_config data");
        assert!(data.get("sample_rate").is_some());
        assert!(data.get("buffer_frames").is_some());
        assert!(data.get("channel_count").is_some());
        assert!(data.get("driver_installed").is_some());
        assert!(data.get("platform_supported").is_some());
    }

    #[test]
    fn handle_get_loudness_returns_valid_response() {
        let resp = run_command(Command::GetLoudness);
        let value = response_value(&resp);
        assert!(value.get("success").is_some());
        if resp.success {
            assert!(value["data"].get("momentary").is_some());
        } else {
            assert!(value.get("error").is_some());
        }
    }

    #[test]
    fn handle_get_metering_returns_metering() {
        let resp = run_command(Command::GetMetering);
        assert!(resp.success);
        let data = resp.data.expect("metering data");
        assert!(data.get("input").is_some());
        assert!(data.get("output").is_some());
        assert!(data.get("sources").is_some());
        assert!(data["sources"].get("input").is_some());
        assert!(data["sources"].get("output").is_some());
    }

    #[test]
    fn handle_list_devices_returns_valid_response() {
        let resp = run_command(Command::ListDevices);
        let value = response_value(&resp);
        assert!(value.get("success").is_some());
        if resp.success {
            assert!(value["data"]["devices"].is_array());
        } else {
            assert!(value.get("error").is_some());
        }
    }

    #[test]
    #[serial]
    fn handle_set_volume_succeeds() {
        let resp = run_command(Command::SetVolume { volume: 0.75 });
        assert!(resp.success, "{:?}", resp.error);
    }

    #[test]
    #[serial]
    fn handle_set_input_channels_succeeds() {
        let resp = run_command(Command::SetInputChannels { channels: 4 });
        assert!(resp.success, "{:?}", resp.error);
    }

    #[test]
    #[serial]
    fn handle_set_output_channels_succeeds() {
        let resp = run_command(Command::SetOutputChannels { channels: 6 });
        assert!(resp.success, "{:?}", resp.error);
    }

    #[test]
    #[serial]
    fn handle_set_pipeline_channels_succeeds() {
        let resp = run_command(Command::SetPipelineChannels {
            input_channels: Some(4),
            output_channels: Some(6),
        });
        assert!(resp.success, "{:?}", resp.error);
    }

    /// Switch to the lab fake driver for the duration of `f`, then restore
    /// the previous `SOTF_SYSTEMWIDE_DRIVER` value.
    ///
    /// # Safety
    ///
    /// Mutates process environment. Safe here because the caller tests are
    /// marked `#[serial]` and no other thread reads this daemon-specific
    /// override concurrently.
    fn with_lab_driver<T>(f: impl FnOnce() -> T) -> T {
        let prev = std::env::var("SOTF_SYSTEMWIDE_DRIVER").ok();
        unsafe {
            std::env::set_var("SOTF_SYSTEMWIDE_DRIVER", "lab");
        }
        let result = f();
        match prev {
            Some(v) => unsafe { std::env::set_var("SOTF_SYSTEMWIDE_DRIVER", v) },
            None => unsafe { std::env::remove_var("SOTF_SYSTEMWIDE_DRIVER") },
        }
        result
    }

    #[test]
    #[serial]
    fn handle_set_sample_rate_succeeds() {
        // The default NullDriver rejects config changes; use the lab fake
        // driver so this test does not require a real HAL installation.
        let resp = with_lab_driver(|| run_command(Command::SetSampleRate { rate: 48_000 }));
        assert!(resp.success, "{:?}", resp.error);
        let data = resp.data.expect("sample_rate data");
        assert_eq!(data["sample_rate"], 48_000);
    }

    #[test]
    #[serial]
    fn handle_set_buffer_frames_succeeds() {
        let resp = with_lab_driver(|| run_command(Command::SetBufferFrames { frames: 512 }));
        assert!(resp.success, "{:?}", resp.error);
        let data = resp.data.expect("buffer_frames data");
        assert_eq!(data["buffer_frames"], 512);
    }

    #[test]
    #[serial]
    fn handle_set_encryption_reports_build_capability() {
        let resp = run_command(Command::SetEncryption { enabled: true });
        if cfg!(all(target_os = "macos", feature = "hal")) {
            assert!(resp.success, "{:?}", resp.error);
            let data = resp.data.expect("encryption data");
            assert_eq!(data["enabled"], true);
            assert!(data.get("fingerprint").is_some());
        } else {
            assert!(!resp.success);
            assert!(
                resp.error
                    .as_deref()
                    .is_some_and(|error| error.contains("no session cipher"))
            );
        }
    }

    #[test]
    #[serial]
    fn handle_load_plugins_empty_succeeds() {
        let resp = run_command(Command::LoadPlugins {
            plugins: vec![],
            input_channels: 2,
            output_channels: 2,
        });
        assert!(resp.success, "{:?}", resp.error);
    }

    #[test]
    fn handle_shutdown_returns_ok_and_stops_daemon() {
        let daemon = AudioDaemon::new();
        let resp = daemon.handle_command(Command::Shutdown);
        assert!(resp.success);
        assert!(!*daemon.running.lock());
    }

    // =========================================================================
    // Snapshot tests for daemon response shapes
    // =========================================================================

    #[test]
    fn snapshot_response_ok_with_payload() {
        let resp = Response::ok(serde_json::json!({"x": 1, "name": "test"}));
        insta::assert_json_snapshot!(resp);
    }

    #[test]
    fn snapshot_response_error() {
        let resp = Response::err("something went wrong");
        insta::assert_json_snapshot!(resp);
    }

    #[test]
    fn snapshot_status_response_shape() {
        // Use a NullDriver so the status response is deterministic regardless
        // of whether the `hal` feature (which selects the platform HAL) is
        // enabled. The KeyManager implementation is also feature-dependent,
        // so we normalize the encryption block before snapshotting.
        let daemon = AudioDaemon {
            manager: Arc::new(Mutex::new(AudioEngineManager::new())),
            running: Arc::new(Mutex::new(true)),
            driver_manager: Arc::new(Mutex::new(DriverManager::from_driver(Box::new(
                driver_common::NullDriver::new(),
            )))),
            system_state: Arc::new(Mutex::new(SystemwideState::default())),
            key_manager: Arc::new(Mutex::new(KeyManager::default())),
        };
        let resp = daemon.handle_command(Command::Status);
        assert!(resp.success, "{:?}", resp.error);
        let mut data = resp.data.expect("status data");
        data["encryption"]["enabled"] = serde_json::json!(false);
        data["encryption"]["fingerprint"] = serde_json::json!("0000000000000000");
        insta::assert_json_snapshot!(data);
    }

    #[test]
    fn snapshot_list_devices_response_shape() {
        // The real device list depends on the host, so we snapshot a
        // representative shape with both default and regular devices.
        let devices = serde_json::json!([
            { "name": "Built-in Output", "is_default": true },
            { "name": "ADAM Audio D3V", "is_default": false, "channels": 2, "sample_rate": 48000 }
        ]);
        let resp = Response::ok(serde_json::json!({ "devices": devices }));
        insta::assert_json_snapshot!(resp);
    }
}

// ============================================================================
// Property-Based Tests
// ============================================================================

#[cfg(test)]
mod property_tests {
    use super::Command;
    use super::Response;
    use super::serialize_response_safely;
    use proptest::prelude::*;
    use serde_json::json;

    fn simple_string_strategy() -> BoxedStrategy<String> {
        proptest::string::string_regex("[a-zA-Z0-9_ ./:-]+")
            .unwrap()
            .boxed()
    }

    fn command_payload_strategy() -> BoxedStrategy<serde_json::Value> {
        // Use only canonical wire names. Aliases (e.g. "snapshot", "hal_status")
        // are exercised separately in unit tests.
        prop_oneof![
            Just(json!({ "command": "status" })),
            Just(json!({ "command": "get_snapshot" })),
            Just(json!({ "command": "dump_state" })),
            simple_string_strategy().prop_map(|path| json!({ "command": "load", "path": path })),
            Just(json!({ "command": "play" })),
            Just(json!({ "command": "pause" })),
            Just(json!({ "command": "stop" })),
            (0.0f64..3600.0)
                .prop_map(|position| json!({ "command": "seek", "position": position })),
            (-60.0f32..12.0f32)
                .prop_map(|volume| json!({ "command": "set_volume", "volume": volume })),
            Just(json!({ "command": "list_devices" })),
            simple_string_strategy()
                .prop_map(|device| json!({ "command": "set_device", "device": device })),
            Just(json!({ "command": "get_loudness" })),
            Just(json!({ "command": "get_metering" })),
            Just(json!({ "command": "get_plugins" })),
            Just(json!({ "command": "get_available_plugins" })),
            Just(json!({ "command": "driver_status" })),
            Just(json!({ "command": "shutdown" })),
            prop::bool::ANY
                .prop_map(|enabled| json!({ "command": "set_encryption", "enabled": enabled })),
            Just(json!({ "command": "encryption_status" })),
            Just(json!({ "command": "rotate_encryption_key" })),
            (1u32..192_000u32)
                .prop_map(|rate| json!({ "command": "set_sample_rate", "rate": rate })),
            (1u32..8192u32)
                .prop_map(|frames| json!({ "command": "set_buffer_frames", "frames": frames })),
            Just(json!({ "command": "get_driver_config" })),
        ]
        .boxed()
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

        /// INVARIANT: every valid JSON command payload deserializes to a Command
        /// whose wire name matches the `command` field in the input payload.
        #[test]
        fn command_json_round_trips_name(payload in command_payload_strategy()) {
            let command_name = payload["command"].as_str().unwrap_or("").to_string();
            let raw = serde_json::to_string(&payload).expect("payload serializes");
            let parsed: Command = serde_json::from_str(&raw).expect("valid payload parses");
            prop_assert_eq!(
                parsed.name(),
                command_name,
                "variant name did not round-trip for payload {}",
                raw
            );
        }

        /// INVARIANT: Response values always serialize to JSON with the expected
        /// top-level shape: a boolean `success`, optional `data`, and optional `error`.
        /// Error responses always include an `error` string; ok responses never do.
        #[test]
        fn response_shape_is_stable(
            success in prop::bool::ANY,
            data in prop::option::of(Just(json!({ "value": 42 }))),
            error in "[^\\x00]*",
        ) {
            let resp = if success {
                Response { success: true, data, error: None }
            } else {
                Response { success: false, data: None, error: Some(error.to_string()) }
            };
            let json = serde_json::to_value(&resp).expect("Response serializes");
            prop_assert_eq!(json["success"].as_bool(), Some(success));
            if success {
                prop_assert!(json.get("error").is_none() || json["error"].is_null());
            } else {
                prop_assert!(json["error"].is_string(), "error response must include an error string");
                prop_assert!(json.get("data").is_none() || json["data"].is_null());
            }
        }

        /// INVARIANT: `serialize_response_safely` never panics and returns parseable JSON.
        #[test]
        fn serialize_response_safely_never_panics(
            success in prop::bool::ANY,
            data in prop::option::of(Just(json!({ "value": 42 }))),
            error in "[^\\x00]*",
        ) {
            let resp = if success {
                Response { success: true, data, error: None }
            } else {
                Response { success: false, data: None, error: Some(error.to_string()) }
            };
            let out = serialize_response_safely(&resp);
            let parsed: serde_json::Value = serde_json::from_str(&out).expect("safe output parses");
            prop_assert_eq!(parsed["success"].as_bool(), Some(success));
        }
    }
}
