use super::create::create_plugin;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::create::create_plugin_with_sandbox_grants;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::create::create_plugin_with_sandbox_grants_for_backend;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::create::create_plugin_with_sandbox_grants_for_backend_and_launcher;
use super::is::is_supported_plugin_type;
use super::parse::parse_external_plugin_descriptor;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::parse::parse_isolated_external_plugin_config;
use super::validate::validate_plugin_security_config;
use crate::{
    ExternalPluginSandboxMode, ExternalPluginState, PluginDescriptor, PluginFormat,
    PluginScanStatus,
};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use crate::{ExternalPluginSandboxTiming, ExternalPluginTrust};
use std::path::PathBuf;

use tempfile::tempdir;

mod misc;

#[test]
fn supported_plugin_type_list_covers_factory_aliases() {
    assert!(is_supported_plugin_type("gain"));
    assert!(is_supported_plugin_type("EQ"));
    assert!(is_supported_plugin_type("rnnoise"));
    assert!(is_supported_plugin_type("active_acoustic_enhancement"));
    assert!(is_supported_plugin_type("external"));
    assert!(is_supported_plugin_type("external_plugin"));
    assert!(!is_supported_plugin_type("definitely_missing"));
}

#[test]
fn create_external_plugin_from_path() {
    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-test-plugin.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();
    let params = serde_json::json!({
        "path": plugin_path.to_string_lossy(),
        "audio_inputs": 2,
        "audio_outputs": 2,
        "name": "External Test",
        "format": "clap",
    });

    let plugin = create_plugin("external", &params, 2, 48_000).unwrap();
    assert_eq!(plugin.input_channels(), 2);
}

#[test]
fn create_external_plugin_from_path_string() {
    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-test-plugin-string.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();

    let plugin = create_plugin(
        "external",
        &serde_json::json!({
            "path": plugin_path.to_string_lossy(),
            "audio_inputs": 2,
            "audio_outputs": 2,
            "name": "External Test",
            "format": "clap",
        }),
        2,
        48_000,
    )
    .unwrap();
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
}

#[test]
fn create_external_plugin_from_embedded_descriptor() {
    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-test-plugin.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();
    let descriptor = PluginDescriptor {
        id: "test.external".into(),
        name: "Embedded External Test".into(),
        vendor: "Test".into(),
        version: "0.1.0".into(),
        format: PluginFormat::Clap,
        path: plugin_path.clone(),
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec!["testing".into()],
        scan_status: PluginScanStatus::Discovered,
    };

    let plugin = create_plugin(
        "external_plugin",
        &serde_json::json!({"descriptor": descriptor}),
        2,
        48_000,
    )
    .unwrap();
    assert_eq!(plugin.output_channels(), 2);
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn factory_rejects_external_state_for_a_different_sandbox_mode() {
    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-state-mode.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();
    let descriptor = PluginDescriptor {
        id: "test.external.state-mode".into(),
        name: "External State Mode Test".into(),
        vendor: "Test".into(),
        version: "0.1.0".into(),
        format: PluginFormat::Clap,
        path: plugin_path,
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec!["testing".into()],
        scan_status: PluginScanStatus::Discovered,
    };
    let descriptor = parse_external_plugin_descriptor(&serde_json::json!({
        "descriptor": descriptor
    }))
    .unwrap();
    let state = ExternalPluginState::new(
        descriptor.clone(),
        ExternalPluginSandboxMode::InProcess,
        vec![1, 2, 3],
    );

    let error = match create_plugin(
        "external",
        &serde_json::json!({
            "descriptor": descriptor,
            "external_state": state,
            "start_worker": false,
        }),
        2,
        48_000,
    ) {
        Ok(_) => panic!("in-process state must not load into the default isolated host"),
        Err(error) => error,
    };
    assert!(error.contains("cannot restore isolated plugin"), "{error}");
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn create_external_plugin_defaults_to_isolated_when_trust_unknown() {
    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-test-plugin-isolated.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();
    let params = serde_json::json!({
        "path": plugin_path.to_string_lossy(),
        "audio_inputs": 2,
        "audio_outputs": 2,
        "name": "External Isolated Test",
        "format": "clap",
        "plugin_trust": "unknown",
        "start_worker": false,
        "deadline_micros": 0,
        "_sotf_instance_id": 37,
    });

    let mut plugin = create_plugin("external", &params, 2, 48_000).unwrap();
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
    let isolated = plugin
        .as_any()
        .and_then(|plugin| plugin.downcast_ref::<crate::IsolatedExternalPlugin>())
        .expect("factory must construct an isolated external plugin");
    assert_eq!(isolated.plugin_instance_id(), Some(37));

    let input = vec![0.25, -0.5, 1.0, -1.0];
    let mut output = vec![0.0; input.len()];
    let frames = plugin
        .process(
            &input,
            &mut output,
            &sotf_host::ProcessContext::new(48_000, 2),
        )
        .unwrap();
    assert_eq!(frames, 2);
    assert_eq!(output, input);
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn create_external_plugin_respects_backend_for_host_owned_sandbox_grants() {
    use crate::{
        PluginSandboxGrantStore, PluginSandboxIdentity, PluginSandboxNetworkGrant,
        PluginSandboxPermission, PluginSandboxUserGrant,
    };

    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-test-plugin-grants.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();
    let params = serde_json::json!({
        "path": plugin_path.to_string_lossy(),
        "audio_inputs": 2,
        "audio_outputs": 2,
        "name": "External Grant Test",
        "vendor": "Test Vendor",
        "id": "com.test.grants",
        "format": "clap",
        "plugin_trust": "unknown",
        "start_worker": false,
    });
    let descriptor = parse_external_plugin_descriptor(&params).unwrap();
    let identity = PluginSandboxIdentity::from_descriptor(&descriptor);
    let mut grants = PluginSandboxGrantStore::default();
    grants.remember(PluginSandboxUserGrant {
        identity,
        permission: PluginSandboxPermission::Network(PluginSandboxNetworkGrant::AnyOutbound),
    });

    let expected_policy = grants.strict_policy_for_plugin(&descriptor, dir.path().join("presets"));
    let backend_can_launch = expected_policy
        .current_backend_launch_plan()
        .validate_for_launch(&expected_policy)
        .is_ok();

    let result = create_plugin_with_sandbox_grants(
        "external",
        &params,
        2,
        48_000,
        &grants,
        dir.path().join("presets"),
    );

    if backend_can_launch {
        let plugin = result.unwrap();
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 2);
    } else {
        let err = match result {
            Ok(_) => panic!("expected unsupported sandbox backend to fail"),
            Err(err) => err,
        };
        assert!(err.contains("cannot satisfy required policy"));
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn create_external_plugin_accepts_host_selected_store_sandbox_backend() {
    use crate::{
        ExternalPluginWorkerCommand, PluginSandboxGrantStore, PluginSandboxLaunchBackend,
        PluginSandboxNetworkGrant, PluginSandboxPermission, PluginSandboxUserGrant,
    };

    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-test-plugin-store-helper.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();
    let params = serde_json::json!({
        "path": plugin_path.to_string_lossy(),
        "audio_inputs": 2,
        "audio_outputs": 2,
        "name": "External Store Helper Test",
        "vendor": "Test Vendor",
        "id": "com.test.store-helper",
        "format": "clap",
        "plugin_trust": "unknown",
        "start_worker": false,
    });
    let descriptor = parse_external_plugin_descriptor(&params).unwrap();
    let mut grants = PluginSandboxGrantStore::default();
    grants.remember(PluginSandboxUserGrant {
        identity: crate::PluginSandboxIdentity::from_descriptor(&descriptor),
        permission: PluginSandboxPermission::Network(PluginSandboxNetworkGrant::AnyOutbound),
    });

    let plugin = create_plugin_with_sandbox_grants_for_backend_and_launcher(
        "external",
        &params,
        2,
        48_000,
        &grants,
        dir.path().join("presets"),
        PluginSandboxLaunchBackend::MacosAppSandboxHelper,
        Some(ExternalPluginWorkerCommand::new("/tmp/sotf-sandbox-helper")),
    )
    .unwrap();

    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn create_external_plugin_rejects_helper_backend_without_launcher() {
    use crate::{PluginSandboxGrantStore, PluginSandboxLaunchBackend};

    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-test-plugin-no-helper.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();
    let params = serde_json::json!({
        "path": plugin_path.to_string_lossy(),
        "audio_inputs": 2,
        "audio_outputs": 2,
        "name": "External Missing Helper Test",
        "vendor": "Test Vendor",
        "id": "com.test.no-helper",
        "format": "clap",
        "plugin_trust": "unknown",
        "start_worker": false,
    });
    let grants = PluginSandboxGrantStore::default();

    let err = match create_plugin_with_sandbox_grants_for_backend(
        "external",
        &params,
        2,
        48_000,
        &grants,
        dir.path().join("presets"),
        PluginSandboxLaunchBackend::MacosAppSandboxHelper,
    ) {
        Ok(_) => panic!("expected helper backend to require launcher command"),
        Err(err) => err,
    };

    assert!(err.contains("requires a host-owned sandbox launcher command"));
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn create_external_plugin_rejects_unrepresentable_host_owned_sandbox_grants() {
    use crate::{
        PluginSandboxGrantStore, PluginSandboxIdentity, PluginSandboxNetworkGrant,
        PluginSandboxPermission, PluginSandboxUserGrant,
    };

    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-test-plugin-loopback.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();
    let params = serde_json::json!({
        "path": plugin_path.to_string_lossy(),
        "audio_inputs": 2,
        "audio_outputs": 2,
        "name": "External Loopback Test",
        "vendor": "Test Vendor",
        "id": "com.test.loopback",
        "format": "clap",
        "plugin_trust": "unknown",
        "start_worker": false,
    });
    let descriptor = parse_external_plugin_descriptor(&params).unwrap();
    let identity = PluginSandboxIdentity::from_descriptor(&descriptor);
    let mut grants = PluginSandboxGrantStore::default();
    grants.remember(PluginSandboxUserGrant {
        identity,
        permission: PluginSandboxPermission::Network(PluginSandboxNetworkGrant::LoopbackOnly),
    });

    let err = match create_plugin_with_sandbox_grants(
        "external",
        &params,
        2,
        48_000,
        &grants,
        dir.path().join("presets"),
    ) {
        Ok(_) => panic!("expected unrepresentable sandbox grant to fail"),
        Err(err) => err,
    };

    assert!(
        err.contains("cannot launch current worker policy")
            || err.contains("cannot satisfy required policy")
    );
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn create_external_plugin_rejects_worker_overrides_from_config() {
    let err = parse_isolated_external_plugin_config(
        &serde_json::json!({
            "worker_path": "/usr/bin/sotf-test-worker",
            "start_worker": false,
        }),
        ExternalPluginTrust::Unknown,
    )
    .unwrap_err();

    assert!(err.contains("worker_path"));
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn isolated_external_plugin_config_uses_bundled_worker() {
    let config = parse_isolated_external_plugin_config(
        &serde_json::json!({
            "start_worker": false,
            "deadline_micros": 250,
            "max_block_frames": 1024,
            "_sotf_instance_id": 37,
        }),
        ExternalPluginTrust::Unknown,
    )
    .unwrap();

    assert!(config.worker_command.program().is_absolute());
    assert!(config.worker_command.command_args().is_empty());
    assert!(config.worker_command.command_env().is_empty());
    assert!(!config.start_worker);
    assert_eq!(config.deadline, std::time::Duration::from_micros(250));
    assert_eq!(config.max_block_frames, 1024);
    assert_eq!(config.plugin_instance_id, Some(37));
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn isolated_external_plugin_config_maps_trust_to_sandbox_timing() {
    let signed = parse_isolated_external_plugin_config(
        &serde_json::json!({
            "sandbox_read_paths": ["/Library/Audio/Plug-Ins"],
            "sandbox_write_paths": ["/tmp/sotf-plugin-cache"],
        }),
        ExternalPluginTrust::Signed,
    )
    .unwrap();
    assert_eq!(
        signed.sandbox_policy.timing,
        ExternalPluginSandboxTiming::AfterPluginLoad
    );
    assert!(!signed.sandbox_policy.require_platform_sandbox);
    assert_eq!(
        signed.sandbox_policy.extra_read_paths,
        vec![PathBuf::from("/Library/Audio/Plug-Ins")]
    );

    let untrusted = parse_isolated_external_plugin_config(
        &serde_json::json!({
            "plugin_trust": "untrusted"
        }),
        ExternalPluginTrust::Untrusted,
    )
    .unwrap();
    assert_eq!(
        untrusted.sandbox_policy.timing,
        ExternalPluginSandboxTiming::BeforePluginLoad
    );
    assert_eq!(
        untrusted.sandbox_policy.require_platform_sandbox,
        cfg!(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "windows"
        ))
    );
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn external_plugin_security_rejects_self_declared_signed_trust() {
    let err = validate_plugin_security_config(
        "external",
        &serde_json::json!({
            "path": "/tmp/fake.clap",
            "plugin_trust": "signed"
        }),
    )
    .unwrap_err();

    assert!(err.contains("cannot mark external plugins as signed"));
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn external_plugin_security_rejects_untrusted_in_process() {
    let err = validate_plugin_security_config(
        "external",
        &serde_json::json!({
            "path": "/tmp/fake.clap",
            "isolated": false
        }),
    )
    .unwrap_err();

    assert!(err.contains("cannot disable process isolation"));
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn external_plugin_security_rejects_relaxed_untrusted_sandbox() {
    let err = validate_plugin_security_config(
        "external",
        &serde_json::json!({
            "path": "/tmp/fake.clap",
            "sandbox_timing": "disabled",
            "start_worker": false
        }),
    )
    .unwrap_err();

    assert!(err.contains("before plugin load"));
}

#[test]
fn create_external_plugin_reports_invalid_parameters() {
    let err = match create_plugin(
        "external",
        &serde_json::json!({"audio_inputs": 2}),
        2,
        48_000,
    ) {
        Ok(_) => panic!("external plugin creation should fail"),
        Err(err) => err,
    };
    assert!(
        err.contains("External plugin descriptor is missing required `path`")
            || err.contains("path")
    );
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn isolated_untrusted_config_rejects_broad_read_write_paths() {
    let err = parse_isolated_external_plugin_config(
        &serde_json::json!({
            "sandbox_read_paths": ["/"],
            "sandbox_write_paths": ["/tmp"],
        }),
        ExternalPluginTrust::Untrusted,
    )
    .unwrap_err();
    assert!(err.contains("cannot expand sandbox filesystem access"));
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn isolated_untrusted_config_rejects_network_grant() {
    let err = parse_isolated_external_plugin_config(
        &serde_json::json!({"sandbox_allow_network": true}),
        ExternalPluginTrust::Untrusted,
    )
    .unwrap_err();
    assert!(err.contains("cannot allow network access"));
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn isolated_untrusted_config_rejects_child_process_grant() {
    let err = parse_isolated_external_plugin_config(
        &serde_json::json!({"sandbox_allow_child_processes": true}),
        ExternalPluginTrust::Untrusted,
    )
    .unwrap_err();
    assert!(err.contains("cannot allow child processes"));
}

#[test]
fn create_external_plugin_rejects_missing_file_path() {
    let err = match create_plugin(
        "external",
        &serde_json::json!({
            "path": "/nonexistent/path/to/plugin.clap",
            "audio_inputs": 2,
            "audio_outputs": 2,
            "name": "Missing Plugin",
            "format": "clap",
        }),
        2,
        48_000,
    ) {
        Ok(_) => panic!("expected missing external plugin path to fail"),
        Err(err) => err,
    };
    assert!(
        err.to_ascii_lowercase().contains("path")
            || err.to_ascii_lowercase().contains("file")
            || err.to_ascii_lowercase().contains("no such")
    );
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn external_plugin_state_stays_consistent_after_invalid_parameter_changes() {
    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-param-corruption.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();
    let params = serde_json::json!({
        "path": plugin_path.to_string_lossy(),
        "audio_inputs": 2,
        "audio_outputs": 2,
        "name": "Param Corruption Test",
        "format": "clap",
        "plugin_trust": "unknown",
        "start_worker": false,
    });

    let mut plugin = create_plugin("external", &params, 2, 48_000).unwrap();
    plugin.initialize(48_000).unwrap();

    // Unknown parameter id should be ignored, not corrupt state.
    let _ = plugin.set_parameter(
        sotf_host::parameters::ParameterId::from("definitely_not_a_real_parameter"),
        sotf_host::parameters::ParameterValue::Float(1.0),
    );

    // Out-of-range value should be rejected, not corrupt state.
    let _ = plugin.set_parameter(
        sotf_host::parameters::ParameterId::from("mix"),
        sotf_host::parameters::ParameterValue::Float(f32::NAN),
    );

    let input = vec![0.25, -0.5, 1.0, -1.0];
    let mut output = vec![0.0; input.len()];
    let frames = plugin
        .process(
            &input,
            &mut output,
            &sotf_host::ProcessContext::new(48_000, 2),
        )
        .unwrap();
    assert_eq!(frames, 2);
}

#[test]
fn beamformer_factory_matches_fallible_constructor_validation() {
    for params in [
        serde_json::json!({"num_mics": 1}),
        serde_json::json!({"num_mics": 2, "mic_spacing_cm": 100.0}),
        serde_json::json!({"num_mics": 2, "steer_angle_deg": 200.0}),
        serde_json::json!({"num_mics": 2, "beamformer_type": "unknown"}),
    ] {
        assert!(create_plugin("beamformer", &params, 2, 48_000).is_err(), "{params}");
    }
    assert!(create_plugin(
        "beamformer",
        &serde_json::json!({"num_mics": 2, "beamformer_type": "Superdirective"}),
        2,
        48_000,
    )
    .is_ok());
}
#[test]
fn aec_catalog_factory_and_runtime_schema_are_canonical() {
    let entry = catalog_entry("aec").expect("AEC catalog entry");
    assert_eq!(entry.metadata.owning_crate, "sotf-plugin-aec");
    assert_eq!(
        entry.metadata.parameter_schema,
        super::catalog::PluginParameterSchema::Static("sotf_plugin_aec::params::PARAMS")
    );
    assert!(create_plugin("aec", &serde_json::json!({}), 1, 48_000).is_err());
    assert!(create_plugin("aec", &serde_json::json!({}), 3, 48_000).is_err());
    let plugin = create_plugin(
        "aec",
        &serde_json::json!({
            "echo_tail_ms": 100.0,
            "step_size": 0.4,
            "post_filter_enabled": false
        }),
        2,
        48_000,
    )
    .expect("canonical factory must construct AEC");
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 1);
    assert_eq!(
        plugin.get_parameter(&sotf_host::parameters::ParameterId::from("step_size")),
        Some(sotf_host::parameters::ParameterValue::Float(0.4))
    );
}

#[test]
fn ambisonics_catalog_admits_every_supported_order() {
    let entry = catalog_entry("ambisonics_decoder").unwrap();
    assert_eq!(
        entry.metadata.channel_layout.supported_inputs,
        super::catalog::PluginSupportedInputLayouts::Enumerated(&[4, 9, 16])
    );
    for (order, channels, layout) in [(1, 4, "5.1"), (2, 9, "7.1.4"), (3, 16, "9.1.6")] {
        let plugin = create_plugin(
            "ambisonics_decoder",
            &serde_json::json!({"order": order, "target_layout": layout}),
            channels,
            48_000,
        )
        .unwrap();
        assert_eq!(plugin.input_channels(), channels);
    }
}
