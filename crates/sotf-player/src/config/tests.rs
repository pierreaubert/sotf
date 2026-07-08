use super::app_config::AppConfig;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::decide::decide_plugin_sandbox_permission_with_broker;
use super::get::delete_remote_server_token;
use super::get::get_app_config_dir;
use super::get::get_music_db_path;
use super::get::get_plugin_presets_dir;
use super::get::get_plugin_sandbox_grants_path;
use super::get::get_recordings_dir;
use super::get::get_remote_server_tokens_path;
use super::get::get_remote_servers_path;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::install::install_authorized_runtime_plugin_sandbox;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::load::load_plugin_sandbox_grants;
use super::load::load_remote_server_token;
#[cfg(any(target_os = "macos", test))]
use super::macos::macos_home_dir_from_env;
use super::misc::APP_BUNDLE_ID;
use super::misc::CONFIG_DIR_OVERRIDE;
#[cfg(test)]
use super::misc::test_config_dir;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::plugin::plugin_sandbox_media_paths;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::plugin_sandbox_permission_controller::PluginSandboxPermissionController;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::save::save_plugin_sandbox_grants;
use super::save::save_remote_server_token;
use std::path::PathBuf;
use std::sync::OnceLock;

use std::sync::Mutex;

fn plugin_sandbox_grants_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn remote_token_store_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn plugin_sandbox_identity(id: &str) -> sotf_plugins::PluginSandboxIdentity {
    sotf_plugins::PluginSandboxIdentity {
        plugin_id: id.into(),
        name: "Plugin".into(),
        vendor: "Vendor".into(),
        version: "1.0".into(),
        format: "Clap".into(),
        path: PathBuf::from("/tmp/plugin.clap"),
    }
}

fn plugin_sandbox_request(
    id: &str,
    permission: sotf_plugins::PluginSandboxPermission,
) -> sotf_plugins::PluginSandboxPermissionRequest {
    sotf_plugins::PluginSandboxPermissionRequest {
        identity: plugin_sandbox_identity(id),
        permission,
        reason: None,
    }
}

#[test]
fn test_config_dir_exists() {
    let config_dir = get_app_config_dir();
    assert!(config_dir.is_some());

    if let Some(dir) = config_dir {
        if CONFIG_DIR_OVERRIDE.get().is_some() {
            assert!(dir.exists());
            return;
        }

        // On macOS
        #[cfg(target_os = "macos")]
        assert!(
            dir.to_string_lossy()
                .contains("Library/Application Support/org.spinorama.sotf")
        );

        // On Linux
        #[cfg(target_os = "linux")]
        assert!(dir.to_string_lossy().contains(".config/sotf"));

        // On Windows (uses LOCALAPPDATA\sotf or USERPROFILE\.config\sotf)
        #[cfg(target_os = "windows")]
        assert!(dir.to_string_lossy().contains("sotf"));
    }
}

#[test]
fn test_music_db_path() {
    let db_path = get_music_db_path();
    assert!(db_path.is_some());

    if let Some(path) = db_path {
        assert!(path.to_string_lossy().ends_with("music.db"));
    }
}

#[test]
fn test_remote_servers_path() {
    let path = get_remote_servers_path();
    assert!(path.is_some());

    if let Some(path) = path {
        assert!(path.to_string_lossy().ends_with("remote_servers.json"));
    }
}

#[test]
fn test_remote_server_tokens_path() {
    let path = get_remote_server_tokens_path();
    assert!(path.is_some());

    if let Some(path) = path {
        assert!(
            path.to_string_lossy()
                .ends_with("remote_server_tokens.json")
        );
    }
}

#[test]
fn test_remote_server_token_internal_store_round_trip() {
    let _guard = remote_token_store_test_lock();
    let _config_dir = test_config_dir();
    let key = "org.spinorama.sotf.remote.test.bearer-token";

    delete_remote_server_token(key).unwrap();
    assert_eq!(load_remote_server_token(key).unwrap(), None);

    save_remote_server_token(key, " very-secret-token ").unwrap();
    assert_eq!(
        load_remote_server_token(key).unwrap().as_deref(),
        Some("very-secret-token")
    );

    delete_remote_server_token(key).unwrap();
    assert_eq!(load_remote_server_token(key).unwrap(), None);
}

#[test]
fn test_plugin_presets_dir() {
    let presets_dir = get_plugin_presets_dir();
    assert!(presets_dir.is_some());

    if let Some(dir) = presets_dir {
        assert!(dir.to_string_lossy().ends_with("plugin_presets"));
    }
}

#[test]
fn test_plugin_sandbox_grants_path() {
    let path = get_plugin_sandbox_grants_path();
    assert!(path.is_some());

    if let Some(path) = path {
        assert!(
            path.to_string_lossy()
                .ends_with("plugin_sandbox_grants.json")
        );
    }
}

#[test]
fn test_plugin_sandbox_grants_round_trip() {
    use sotf_plugins::{
        PluginSandboxGrantStore, PluginSandboxIdentity, PluginSandboxNetworkGrant,
        PluginSandboxPermission, PluginSandboxUserGrant,
    };

    let _guard = plugin_sandbox_grants_test_lock();
    let _config_dir = test_config_dir();
    let identity = PluginSandboxIdentity {
        plugin_id: "com.test.plugin".into(),
        name: "Plugin".into(),
        vendor: "Vendor".into(),
        version: "1.0".into(),
        format: "Clap".into(),
        path: PathBuf::from("/tmp/plugin.clap"),
    };
    let mut grants = PluginSandboxGrantStore::default();
    grants.remember(PluginSandboxUserGrant {
        identity,
        permission: PluginSandboxPermission::Network(PluginSandboxNetworkGrant::LoopbackOnly),
    });

    save_plugin_sandbox_grants(&grants).unwrap();
    let loaded = load_plugin_sandbox_grants().unwrap();

    assert_eq!(loaded, grants);
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn plugin_sandbox_media_paths_include_library_dirs() {
    let library_dir = PathBuf::from("/tmp/sotf-library");

    let paths = plugin_sandbox_media_paths(vec![library_dir.clone()]);

    assert!(paths.contains(&library_dir));
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn install_authorized_runtime_plugin_sandbox_sets_global_options() {
    use sotf_plugins::{
        PluginSandboxGrantStore, PluginSandboxIdentity, PluginSandboxNetworkGrant,
        PluginSandboxPermission, PluginSandboxUserGrant,
    };

    let _guard = plugin_sandbox_grants_test_lock();
    let _config_dir = test_config_dir();
    let library_dir = PathBuf::from("/tmp/sotf-runtime-media");
    let identity = PluginSandboxIdentity {
        plugin_id: "com.test.runtime-options".into(),
        name: "Plugin".into(),
        vendor: "Vendor".into(),
        version: "1.0".into(),
        format: "Clap".into(),
        path: PathBuf::from("/tmp/plugin.clap"),
    };
    let mut grants = PluginSandboxGrantStore::default();
    grants.remember(PluginSandboxUserGrant {
        identity,
        permission: PluginSandboxPermission::Network(PluginSandboxNetworkGrant::AnyOutbound),
    });
    save_plugin_sandbox_grants(&grants).unwrap();

    let status = install_authorized_runtime_plugin_sandbox(vec![library_dir.clone()]).unwrap();
    let options = sotf_plugins::default_sandboxed_plugin_creation_options().unwrap();

    assert!(status.runtime_external_access_disabled);
    assert_eq!(status.persistent_grant_count, 1);
    assert!(status.media_read_paths.contains(&library_dir));
    assert_eq!(
        options.lifecycle,
        sotf_plugins::PluginSandboxLifecycleMode::AuthorizedRuntime
    );
    assert!(options.media_read_paths.contains(&library_dir));
    assert_eq!(options.grants.grants.len(), 1);

    sotf_plugins::set_default_sandboxed_plugin_creation_options(None);
}

#[test]
fn plugin_sandbox_permission_controller_denies_without_broker() {
    let request = plugin_sandbox_request(
        "com.test.default-deny",
        sotf_plugins::PluginSandboxPermission::Network(
            sotf_plugins::PluginSandboxNetworkGrant::AnyOutbound,
        ),
    );
    let mut controller =
        PluginSandboxPermissionController::new(sotf_plugins::PluginSandboxGrantStore::default());

    let resolution = controller.decide_or_deny(request);

    assert_eq!(
        resolution.decision.outcome,
        sotf_plugins::PluginSandboxPermissionOutcome::Denied
    );
    assert!(!resolution.restart_required);
    assert!(!resolution.session_grants_changed);
    assert!(!resolution.persistent_grants_changed);
    assert!(controller.grants().grants.is_empty());
}

#[test]
fn plugin_sandbox_permission_controller_persists_remembered_grants() {
    struct RememberBroker;

    impl sotf_plugins::PluginSandboxPermissionBroker for RememberBroker {
        fn decide_permission(
            &mut self,
            request: sotf_plugins::PluginSandboxPermissionRequest,
        ) -> sotf_plugins::PluginSandboxPermissionDecision {
            request.grant_remembered()
        }
    }

    let _guard = plugin_sandbox_grants_test_lock();
    let _config_dir = test_config_dir();
    save_plugin_sandbox_grants(&sotf_plugins::PluginSandboxGrantStore::default()).unwrap();

    let request = plugin_sandbox_request(
        "com.test.remembered",
        sotf_plugins::PluginSandboxPermission::LocalAuthorization(
            sotf_plugins::PluginSandboxAuthorizationGrant::Pace,
        ),
    );
    let mut broker = RememberBroker;

    let resolution = decide_plugin_sandbox_permission_with_broker(request, &mut broker).unwrap();
    let loaded = load_plugin_sandbox_grants().unwrap();

    assert!(resolution.restart_required);
    assert!(resolution.session_grants_changed);
    assert!(resolution.persistent_grants_changed);
    assert_eq!(loaded.grants.len(), 1);
}

#[test]
fn plugin_sandbox_permission_controller_keeps_until_restart_grants_in_memory_only() {
    struct SessionBroker;

    impl sotf_plugins::PluginSandboxPermissionBroker for SessionBroker {
        fn decide_permission(
            &mut self,
            request: sotf_plugins::PluginSandboxPermissionRequest,
        ) -> sotf_plugins::PluginSandboxPermissionDecision {
            request.grant_until_restart()
        }
    }

    let _guard = plugin_sandbox_grants_test_lock();
    let _config_dir = test_config_dir();
    save_plugin_sandbox_grants(&sotf_plugins::PluginSandboxGrantStore::default()).unwrap();

    let request = plugin_sandbox_request(
        "com.test.session-only",
        sotf_plugins::PluginSandboxPermission::WritePath {
            path: PathBuf::from("/tmp/plugin-cache"),
        },
    );
    let mut controller =
        PluginSandboxPermissionController::new(sotf_plugins::PluginSandboxGrantStore::default());
    let mut broker = SessionBroker;

    let resolution = controller
        .decide_with_broker_and_save(request, &mut broker)
        .unwrap();
    let loaded = load_plugin_sandbox_grants().unwrap();

    assert!(resolution.restart_required);
    assert!(resolution.session_grants_changed);
    assert!(!resolution.persistent_grants_changed);
    assert_eq!(controller.grants().grants.len(), 1);
    assert!(loaded.grants.is_empty());
}

#[test]
fn plugin_sandbox_permission_controller_skips_prompt_for_existing_grant() {
    struct PanicBroker;

    impl sotf_plugins::PluginSandboxPermissionBroker for PanicBroker {
        fn decide_permission(
            &mut self,
            _request: sotf_plugins::PluginSandboxPermissionRequest,
        ) -> sotf_plugins::PluginSandboxPermissionDecision {
            panic!("broker should not be called for an already granted permission");
        }
    }

    let identity = plugin_sandbox_identity("com.test.already-granted");
    let mut grants = sotf_plugins::PluginSandboxGrantStore::default();
    grants.remember(sotf_plugins::PluginSandboxUserGrant {
        identity: identity.clone(),
        permission: sotf_plugins::PluginSandboxPermission::Network(
            sotf_plugins::PluginSandboxNetworkGrant::AnyOutbound,
        ),
    });
    let request = sotf_plugins::PluginSandboxPermissionRequest {
        identity,
        permission: sotf_plugins::PluginSandboxPermission::Network(
            sotf_plugins::PluginSandboxNetworkGrant::LoopbackOnly,
        ),
        reason: None,
    };
    let mut controller = PluginSandboxPermissionController::new(grants);
    let mut broker = PanicBroker;

    let resolution = controller.decide_with_broker(request, &mut broker);

    assert!(!resolution.restart_required);
    assert!(!resolution.session_grants_changed);
    assert!(!resolution.persistent_grants_changed);
}

#[test]
fn test_recordings_dir() {
    let recordings_dir = get_recordings_dir();
    assert!(recordings_dir.is_some());

    if let Some(dir) = recordings_dir {
        assert!(dir.to_string_lossy().ends_with("Recordings"));
    }
}

#[test]
fn test_macos_sandbox_home_uses_container() {
    let home = std::ffi::OsStr::new("/Users/alice");
    let sandbox_id = std::ffi::OsStr::new(APP_BUNDLE_ID);
    let dir = macos_home_dir_from_env(Some(home), None, Some(sandbox_id)).unwrap();

    assert_eq!(
        dir,
        PathBuf::from("/Users/alice")
            .join("Library")
            .join("Containers")
            .join(APP_BUNDLE_ID)
            .join("Data")
    );
}

#[test]
fn test_macos_sandbox_home_prefers_cf_fixed_home() {
    let home = std::ffi::OsStr::new("/Users/alice");
    let fixed_home =
        std::ffi::OsStr::new("/Users/alice/Library/Containers/org.spinorama.sotf/Data");
    let sandbox_id = std::ffi::OsStr::new(APP_BUNDLE_ID);
    let dir = macos_home_dir_from_env(Some(home), Some(fixed_home), Some(sandbox_id)).unwrap();

    assert_eq!(
        dir,
        PathBuf::from("/Users/alice/Library/Containers/org.spinorama.sotf/Data")
    );
}

// =========================================================================
// AppConfig schema / version compatibility tests (QA-CORE-001)
// =========================================================================

#[test]
fn app_config_default_deserialize() {
    let json = r#"{}"#;
    let config: AppConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.version, 1);
    assert_eq!(config.output_device, None);
    assert!(config.queue.is_empty());
    assert_eq!(config.queue_index, None);
    assert_eq!(config.track_index, 0);
    assert_eq!(config.plugin_preset, None);
}

#[test]
fn app_config_ignores_unknown_fields() {
    let json = r#"{
        "version": 1,
        "output_device": "Built-in Output",
        "queue": [],
        "queue_index": null,
        "track_index": 0,
        "plugin_preset": null,
        "future_field": "ignored",
        "unknown_nested": {"x": 1}
    }"#;

    let config: AppConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.output_device.as_deref(), Some("Built-in Output"));
    assert_eq!(config.version, 1);
}

#[test]
fn app_config_serde_roundtrip() {
    let config = AppConfig {
        version: 1,
        output_device: Some("Device".into()),
        queue: vec![("Artist".into(), "Album".into())],
        queue_index: Some(0),
        track_index: 3,
        plugin_preset: Some("preset".into()),
    };

    let json = serde_json::to_string(&config).unwrap();
    let decoded: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.version, config.version);
    assert_eq!(decoded.output_device, config.output_device);
    assert_eq!(decoded.queue, config.queue);
    assert_eq!(decoded.queue_index, config.queue_index);
    assert_eq!(decoded.track_index, config.track_index);
    assert_eq!(decoded.plugin_preset, config.plugin_preset);
}

#[test]
fn app_config_rejects_version_below_minimum() {
    let config = AppConfig {
        version: 0,
        ..Default::default()
    };
    let result = super::app_config::migrate_app_config(config);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Unsupported AppConfig version")
    );
}
