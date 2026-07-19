use super::ExternalPlugin;
use super::external_hosting_backend::plan_external_plugin_hosting;
use super::external_hosting_backend::select_hosting_backend;
use super::external_plugin_state::ExternalPluginState;
use super::misc::EXTERNAL_PLUGIN_PRESET_ID;
use super::plugin::plugin_format_capabilities;
use super::plugin_descriptor::PluginDescriptor;
use super::plugin_format::PluginFormat;
use super::plugin_scan_summary::PluginScanSummary;
use super::plugin_scanner::PluginScanner;
use super::types::ExternalHostingBackend;
use super::types::ExternalPluginSandboxMode;
use super::types::PluginScanStatus;
use super::types::PluginScanStatusMode;
use crate::error::PluginError;
use crate::parameters::{ParameterId, ParameterValue};
use crate::plugin::{Plugin, ProcessContext};
use crate::serialization::{PluginPreset, SerializablePlugin};
use std::fs;
use std::path::{Path, PathBuf};

use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

fn unavailable_test_plugin(descriptor: &PluginDescriptor, sample_rate: u32) -> ExternalPlugin {
    ExternalPlugin::unavailable_placeholder(
        descriptor.clone(),
        sample_rate,
        "intentional test placeholder".to_string(),
    )
}

#[test]
fn test_plugin_scanner_search_paths() {
    // Verify search paths are non-empty for at least one format
    let paths = PluginScanner::search_paths(PluginFormat::Clap);
    assert!(!paths.is_empty(), "Should have CLAP search paths");
}

#[test]
fn test_plugin_scanner_scan_nonexistent() {
    let mut scanner = PluginScanner::new();
    scanner.scan_directory(Path::new("/nonexistent/path"), PluginFormat::Clap);
    assert!(scanner.plugins.is_empty());
}

#[test]
fn test_plugin_scanner_scan_path_single_bundle() {
    let dir = tempfile::tempdir().unwrap();
    let plugin_path = dir.path().join("scan-path-single.clap");
    fs::write(&plugin_path, b"stub plugin").unwrap();
    let mut scanner = PluginScanner::new();

    scanner.scan_path(&plugin_path, None).unwrap();

    assert_eq!(scanner.plugins.len(), 1);
    assert_eq!(scanner.plugins[0].format, PluginFormat::Clap);
    assert_eq!(scanner.plugins[0].name, "scan-path-single");
}

#[test]
fn test_plugin_scanner_scan_path_directory_recursive() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("nested");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("recursive-test.vst3"), b"stub plugin").unwrap();
    let mut scanner = PluginScanner::new();

    scanner.scan_path(dir.path(), None).unwrap();

    assert_eq!(scanner.plugins.len(), 1);
    assert_eq!(scanner.plugins[0].format, PluginFormat::Vst3);
    assert_eq!(scanner.plugins[0].name, "recursive-test");
}

#[test]
fn test_plugin_scanner_scan_path_rejects_format_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let plugin_path = dir.path().join("mismatch.clap");
    fs::write(&plugin_path, b"stub plugin").unwrap();
    let mut scanner = PluginScanner::new();

    let err = scanner
        .scan_path(&plugin_path, Some(PluginFormat::Vst3))
        .unwrap_err();

    assert!(err.contains("not Vst3"));
    assert!(scanner.plugins.is_empty());
}

#[test]
fn test_external_plugin_passthrough() {
    let mut tmp_path = env::temp_dir();
    tmp_path.push(format!(
        "sotf-external-plugin-fake-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&tmp_path).unwrap();
    let plugin_path = tmp_path.join("fake.clap");
    fs::write(&plugin_path, b"stub plugin").unwrap();

    let desc = PluginDescriptor {
        id: "test.plugin".into(),
        name: "Test Plugin".into(),
        vendor: "Test".into(),
        version: "1.0".into(),
        format: PluginFormat::Clap,
        path: plugin_path.clone(),
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec![],
        scan_status: PluginScanStatus::Discovered,
    };

    let mut plugin = unavailable_test_plugin(&desc, 48_000);
    let input = vec![0.5f32; 2048];
    let mut output = vec![0.0f32; 2048];
    let ctx = ProcessContext::new(48000, 1024);

    let frames = plugin.process(&input, &mut output, &ctx).unwrap();
    assert_eq!(frames, 1024);
    // Passthrough: first matching channels should match input
    for i in 0..2048 {
        assert!((output[i] - input[i]).abs() < 1e-6);
    }

    fs::remove_file(plugin_path).unwrap();
    fs::remove_dir_all(tmp_path).unwrap();
}

#[test]
fn test_external_plugin_scan_recursive_and_dedup() {
    let root = env::temp_dir().join(format!(
        "sotf-external-plugin-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let nested = root.join("nested");
    let plugin_file = nested.join("my-plugin.clap");

    fs::create_dir_all(&nested).unwrap();
    fs::write(&plugin_file, b"stub").unwrap();

    let mut scanner = PluginScanner::new();
    scanner.scan_directory(&root, PluginFormat::Clap);
    assert_eq!(scanner.plugins.len(), 1);
    assert_eq!(scanner.plugins[0].name, "my-plugin");
    assert_eq!(
        scanner.plugins[0].scan_status,
        PluginFormat::Clap.build_scan_status()
    );
    scanner.scan_directory(&root, PluginFormat::Clap);
    assert_eq!(scanner.plugins.len(), 1);

    fs::remove_file(&plugin_file).unwrap();
    fs::remove_dir_all(&nested).unwrap();
    fs::remove_dir_all(&root).unwrap_or(());
}

#[test]
fn test_external_plugin_scanner_can_preserve_discovered_status() {
    let root = env::temp_dir().join(format!(
        "sotf-external-plugin-discovered-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let plugin_file = root.join("raw-discovery.clap");

    fs::create_dir_all(&root).unwrap();
    fs::write(&plugin_file, b"stub").unwrap();

    let mut scanner = PluginScanner::with_scan_status_mode(PluginScanStatusMode::DiscoveryOnly);
    scanner.scan_directory(&root, PluginFormat::Clap);

    assert_eq!(scanner.plugins.len(), 1);
    assert_eq!(scanner.plugins[0].scan_status, PluginScanStatus::Discovered);

    fs::remove_file(&plugin_file).unwrap();
    fs::remove_dir_all(&root).unwrap_or(());
}

#[test]
fn test_external_plugin_scan_summary_counts_statuses() {
    let descriptor = |id: &str, status: PluginScanStatus| PluginDescriptor {
        id: id.into(),
        name: id.into(),
        vendor: "Test".into(),
        version: "1.0".into(),
        format: PluginFormat::Clap,
        path: PathBuf::from(format!("/tmp/{id}.clap")),
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec![],
        scan_status: status,
    };
    let mut scanner = PluginScanner::new();
    scanner.plugins.push(descriptor(
        "discovered.plugin",
        PluginScanStatus::Discovered,
    ));
    scanner
        .plugins
        .push(descriptor("loadable.plugin", PluginScanStatus::Loadable));
    scanner.plugins.push(descriptor(
        "unsupported.plugin",
        PluginScanStatus::UnsupportedByBuild,
    ));

    let summary = scanner.summary();

    assert_eq!(
        summary,
        PluginScanSummary {
            total: 3,
            discovered: 1,
            loadable: 1,
            unsupported_by_build: 1,
        }
    );
}

#[test]
fn test_external_plugin_capability_matrix_reports_build_support() {
    let matrix = plugin_format_capabilities();
    assert_eq!(matrix.len(), 3);
    let clap = matrix
        .iter()
        .find(|capability| capability.format == PluginFormat::Clap)
        .unwrap();
    assert_eq!(clap.feature, "external-plugin-clap");
    assert_eq!(clap.scan_status, PluginFormat::Clap.build_scan_status());
    assert_eq!(clap.backend, select_hosting_backend(PluginFormat::Clap));
    assert_eq!(
        clap.native_backend_available,
        clap.backend != ExternalHostingBackend::Passthrough
    );
    if clap.native_backend_available {
        assert_eq!(clap.reason, None);
    } else {
        assert!(
            clap.reason
                .as_deref()
                .unwrap()
                .contains("unsupported-by-build")
        );
    }
}

#[test]
fn test_external_plugin_hosting_plan_reports_feature_gate() {
    let desc = PluginDescriptor {
        id: "planned.plugin".into(),
        name: "Planned Plugin".into(),
        vendor: "Test".into(),
        version: "1.0".into(),
        format: PluginFormat::Clap,
        path: PathBuf::from("/tmp/planned-plugin.clap"),
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec![],
        scan_status: PluginScanStatus::Discovered,
    };

    let plan = plan_external_plugin_hosting(&desc);

    assert_eq!(plan.format, PluginFormat::Clap);
    assert_eq!(plan.feature, "external-plugin-clap");
    assert_eq!(plan.scan_status, PluginFormat::Clap.build_scan_status());
    assert_eq!(plan.backend, select_hosting_backend(PluginFormat::Clap));
    if plan.backend == ExternalHostingBackend::Passthrough {
        assert!(!plan.native_backend_available);
        assert!(
            plan.reason
                .as_deref()
                .unwrap()
                .contains("deterministic passthrough")
        );
    } else {
        assert!(plan.native_backend_available);
        assert_eq!(plan.reason, None);
    }
}

#[test]
fn test_external_plugin_set_parameter_unknown() {
    let mut tmp_path = env::temp_dir();
    tmp_path.push(format!(
        "sotf-external-plugin-setparam-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&tmp_path).unwrap();
    let plugin_path = tmp_path.join("fake.clap");
    fs::write(&plugin_path, b"stub plugin").unwrap();
    let desc = PluginDescriptor {
        id: "test.plugin".into(),
        name: "Test Plugin".into(),
        vendor: "Test".into(),
        version: "1.0".into(),
        format: PluginFormat::Clap,
        path: plugin_path.clone(),
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec![],
        scan_status: PluginScanStatus::Discovered,
    };

    let mut plugin = unavailable_test_plugin(&desc, 48_000);
    let result = plugin.set_parameter(ParameterId::from("unknown"), ParameterValue::Float(1.0));
    assert!(result.is_err());

    fs::remove_file(plugin_path).unwrap();
    fs::remove_dir_all(tmp_path).unwrap();
}

#[test]
fn test_external_plugin_placeholder_state_round_trips() {
    let tmp_path = env::temp_dir().join(format!(
        "sotf-external-plugin-state-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&tmp_path).unwrap();
    let plugin_path = tmp_path.join("state-test.clap");
    fs::write(&plugin_path, b"stub plugin").unwrap();
    let desc = PluginDescriptor {
        id: "test.state".into(),
        name: "State Test".into(),
        vendor: "Test".into(),
        version: "1.0".into(),
        format: PluginFormat::Clap,
        path: plugin_path.clone(),
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec!["state".into()],
        scan_status: PluginScanStatus::Discovered,
    };
    let plugin = unavailable_test_plugin(&desc, 48_000);
    let mut state = plugin.placeholder_state();
    state.opaque_state = vec![1, 2, 3, 4];

    let json = serde_json::to_string(&state).unwrap();
    let decoded: ExternalPluginState = serde_json::from_str(&json).unwrap();
    let restored = ExternalPlugin::from_placeholder_state(&decoded, 48_000).unwrap();

    assert_eq!(decoded, state);
    assert_eq!(restored.descriptor(), &desc);
    assert_eq!(decoded.sandbox_mode, ExternalPluginSandboxMode::InProcess);
    assert_eq!(decoded.opaque_state, vec![1, 2, 3, 4]);

    let mut incompatible = decoded;
    incompatible.sandbox_mode = ExternalPluginSandboxMode::Isolated;
    let error = ExternalPlugin::from_placeholder_state(&incompatible, 48_000)
        .err()
        .expect("isolated state must not restore in process");
    assert!(error.contains("cannot restore in-process plugin"));

    fs::remove_file(plugin_path).unwrap();
    fs::remove_dir_all(tmp_path).unwrap();
}

#[test]
fn test_external_plugin_placeholder_state_restores_missing_plugin_as_unavailable() {
    let missing_path = env::temp_dir().join(format!(
        "sotf-external-plugin-missing-{}.clap",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let desc = PluginDescriptor {
        id: "test.missing".into(),
        name: "Missing Test".into(),
        vendor: "Test".into(),
        version: "1.0".into(),
        format: PluginFormat::Clap,
        path: missing_path,
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec!["state".into()],
        scan_status: PluginScanStatus::Discovered,
    };
    let state = ExternalPluginState::new(
        desc.clone(),
        ExternalPluginSandboxMode::InProcess,
        vec![1, 2, 3],
    );

    let mut restored = ExternalPlugin::from_placeholder_state(&state, 48_000).unwrap();

    assert_eq!(restored.descriptor(), &desc);
    assert_eq!(
        restored.hosting_backend(),
        ExternalHostingBackend::Passthrough
    );
    assert!(
        restored
            .restore_error()
            .unwrap()
            .contains("plugin path does not exist")
    );

    let input = vec![0.25, -0.5, 1.0, -1.0];
    let mut output = vec![0.0; input.len()];
    let frames = restored
        .process(&input, &mut output, &ProcessContext::new(48_000, 2))
        .unwrap();
    assert_eq!(frames, 2);
    assert_eq!(output, input);
}

#[test]
fn test_external_plugin_serializable_preset_round_trips_placeholder_state() {
    let tmp_path = env::temp_dir().join(format!(
        "sotf-external-plugin-serializable-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&tmp_path).unwrap();
    let plugin_path = tmp_path.join("serializable.clap");
    fs::write(&plugin_path, b"stub plugin").unwrap();
    let desc = PluginDescriptor {
        id: "test.serializable".into(),
        name: "Serializable Test".into(),
        vendor: "Test".into(),
        version: "1.0".into(),
        format: PluginFormat::Clap,
        path: plugin_path.clone(),
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec!["state".into()],
        scan_status: PluginScanStatus::Discovered,
    };
    let mut plugin = unavailable_test_plugin(&desc, 48_000);

    let preset = SerializablePlugin::serialize(&plugin).unwrap();
    let restored_state = preset.external_plugin_state().unwrap().unwrap();

    assert_eq!(preset.plugin_id, EXTERNAL_PLUGIN_PRESET_ID);
    assert_eq!(restored_state.descriptor, desc);
    assert_eq!(
        restored_state.sandbox_mode,
        ExternalPluginSandboxMode::InProcess
    );
    assert!(restored_state.opaque_state.is_empty());
    SerializablePlugin::deserialize(&mut plugin, &preset).unwrap();

    let mut isolated_state = restored_state;
    isolated_state.sandbox_mode = ExternalPluginSandboxMode::Isolated;
    let mut incompatible_preset = preset.clone();
    incompatible_preset
        .set_external_plugin_state(&isolated_state)
        .unwrap();
    let error = SerializablePlugin::deserialize(&mut plugin, &incompatible_preset)
        .expect_err("isolated preset must not restore into in-process plugin");
    assert!(
        error
            .to_string()
            .contains("cannot restore in-process plugin")
    );

    fs::remove_file(plugin_path).unwrap();
    fs::remove_dir_all(tmp_path).unwrap();
}

#[test]
fn test_external_plugin_deserialize_rejects_different_descriptor() {
    let tmp_path = env::temp_dir().join(format!(
        "sotf-external-plugin-mismatch-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&tmp_path).unwrap();
    let plugin_path = tmp_path.join("mismatch.clap");
    fs::write(&plugin_path, b"stub plugin").unwrap();
    let desc = PluginDescriptor {
        id: "test.mismatch".into(),
        name: "Mismatch Test".into(),
        vendor: "Test".into(),
        version: "1.0".into(),
        format: PluginFormat::Clap,
        path: plugin_path.clone(),
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec![],
        scan_status: PluginScanStatus::Discovered,
    };
    let mut plugin = unavailable_test_plugin(&desc, 48_000);
    let mut state = plugin.placeholder_state();
    state.plugin_id = "other.plugin".into();
    state.descriptor.id = "other.plugin".into();

    let mut preset = PluginPreset::new(
        "Other".into(),
        EXTERNAL_PLUGIN_PRESET_ID.into(),
        env!("CARGO_PKG_VERSION").into(),
    );
    preset.set_external_plugin_state(&state).unwrap();

    assert!(matches!(
        SerializablePlugin::deserialize(&mut plugin, &preset),
        Err(PluginError::InvalidConfiguration(_))
    ));

    fs::remove_file(plugin_path).unwrap();
    fs::remove_dir_all(tmp_path).unwrap();
}

#[test]
fn test_plugin_format_extension() {
    assert_eq!(PluginFormat::Clap.extension(), "clap");
    assert_eq!(PluginFormat::Vst3.extension(), "vst3");
    assert_eq!(PluginFormat::AudioUnit.extension(), "component");
}

#[test]
fn test_external_plugin_backend_selection_is_feature_gated() {
    assert_eq!(
        select_hosting_backend(PluginFormat::Clap),
        if cfg!(feature = "external-plugin-clap") {
            ExternalHostingBackend::Clap
        } else {
            ExternalHostingBackend::Passthrough
        }
    );
    assert_eq!(
        select_hosting_backend(PluginFormat::Vst3),
        if cfg!(feature = "external-plugin-vst3") {
            ExternalHostingBackend::Vst3
        } else {
            ExternalHostingBackend::Passthrough
        }
    );
    assert_eq!(
        select_hosting_backend(PluginFormat::AudioUnit),
        if cfg!(feature = "external-plugin-au") {
            ExternalHostingBackend::AudioUnit
        } else {
            ExternalHostingBackend::Passthrough
        }
    );
}
