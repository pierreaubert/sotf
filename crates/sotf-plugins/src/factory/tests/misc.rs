use super::super::create::create_plugin;
use super::super::parse::parse_external_plugin_descriptor;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::super::sandboxed_plugin_creation_options::SandboxedPluginCreationOptions;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::super::sandboxed_plugin_creation_options::set_default_sandboxed_plugin_creation_options;

use std::sync::{Mutex, MutexGuard, OnceLock};
use tempfile::tempdir;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn sandbox_options_test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn create_plugin_uses_installed_authorized_runtime_sandbox_options() {
    use crate::{
        ExternalPluginWorkerCommand, PluginSandboxGrantStore, PluginSandboxIdentity,
        PluginSandboxLaunchBackend, PluginSandboxNetworkGrant, PluginSandboxPermission,
        PluginSandboxUserGrant,
    };

    let _guard = sandbox_options_test_lock();
    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-test-plugin-default-options.clap");
    let media_path = dir.path().join("Music");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();
    std::fs::create_dir_all(&media_path).unwrap();
    let params = serde_json::json!({
        "path": plugin_path.to_string_lossy(),
        "audio_inputs": 2,
        "audio_outputs": 2,
        "name": "External Default Options Test",
        "vendor": "Test Vendor",
        "id": "com.test.default-options",
        "format": "clap",
        "plugin_trust": "unknown",
        "start_worker": false,
    });
    let descriptor = parse_external_plugin_descriptor(&params).unwrap();
    let mut grants = PluginSandboxGrantStore::default();
    grants.remember(PluginSandboxUserGrant {
        identity: PluginSandboxIdentity::from_descriptor(&descriptor),
        permission: PluginSandboxPermission::Network(PluginSandboxNetworkGrant::AnyOutbound),
    });
    let options = SandboxedPluginCreationOptions::authorized_runtime(
        grants,
        dir.path().join("presets"),
        vec![media_path],
    )
    .with_backend(PluginSandboxLaunchBackend::MacosAppSandboxHelper)
    .with_launcher(Some(ExternalPluginWorkerCommand::new(
        "/tmp/sotf-sandbox-helper",
    )));
    set_default_sandboxed_plugin_creation_options(Some(options));

    let plugin = create_plugin("external", &params, 2, 48_000).unwrap();

    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
    set_default_sandboxed_plugin_creation_options(None);
}
