use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;

pub const APP_BUNDLE_ID: &str = "org.spinorama.sotf";

/// Global override for the app config directory (set via `--qa` flag).
static CONFIG_DIR_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

/// Set a custom config directory, overriding the platform default.
/// Must be called before any `get_app_config_dir()` usage.
/// Creates the directory if it doesn't exist.
pub fn set_config_dir_override(path: PathBuf) {
    std::fs::create_dir_all(&path).expect("Failed to create config dir override");
    CONFIG_DIR_OVERRIDE
        .set(path)
        .expect("Config dir override already set");
}

#[cfg(test)]
pub(crate) fn test_config_dir() -> PathBuf {
    if let Some(dir) = CONFIG_DIR_OVERRIDE.get() {
        return dir.clone();
    }

    let dir = std::env::temp_dir()
        .join("sotf-player-test-config")
        .join(std::process::id().to_string());
    std::fs::create_dir_all(&dir).expect("Failed to create test config dir");
    let _ = CONFIG_DIR_OVERRIDE.set(dir.clone());
    CONFIG_DIR_OVERRIDE.get().cloned().unwrap_or(dir)
}

/// Application state that persists between sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Configuration version for migration support
    #[serde(default = "default_app_config_version")]
    pub version: u32,

    /// Currently selected output device name
    pub output_device: Option<String>,
    /// Queue of albums (artist, title pairs)
    pub queue: Vec<(String, String)>,
    /// Current position in queue
    pub queue_index: Option<usize>,
    /// Current track index in the current album
    pub track_index: usize,
    /// Currently loaded plugin preset name
    pub plugin_preset: Option<String>,
}

fn default_app_config_version() -> u32 {
    1
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: default_app_config_version(),
            output_device: None,
            queue: Vec::new(),
            queue_index: None,
            track_index: 0,
            plugin_preset: None,
        }
    }
}

#[cfg(any(target_os = "macos", test))]
fn macos_home_dir_from_env(
    home: Option<&std::ffi::OsStr>,
    cf_fixed_user_home: Option<&std::ffi::OsStr>,
    app_sandbox_container_id: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    let sandbox_id = app_sandbox_container_id
        .and_then(|id| id.to_str())
        .filter(|id| !id.is_empty());

    if sandbox_id.is_some()
        && let Some(fixed_home) = cf_fixed_user_home
        && !fixed_home.is_empty()
    {
        return Some(PathBuf::from(fixed_home));
    }

    let home = home.filter(|home| !home.is_empty()).map(PathBuf::from)?;

    if let Some(sandbox_id) = sandbox_id {
        let container_data_suffix = PathBuf::from("Library")
            .join("Containers")
            .join(sandbox_id)
            .join("Data");

        if home.ends_with(&container_data_suffix) {
            Some(home)
        } else {
            Some(home.join(container_data_suffix))
        }
    } else {
        Some(home)
    }
}

#[cfg(target_os = "macos")]
fn macos_home_dir() -> Option<PathBuf> {
    macos_home_dir_from_env(
        std::env::var_os("HOME").as_deref(),
        std::env::var_os("CFFIXED_USER_HOME").as_deref(),
        std::env::var_os("APP_SANDBOX_CONTAINER_ID").as_deref(),
    )
}

/// Get the application configuration directory.
/// - Linux: ~/.config/sotf
/// - macOS direct: ~/Library/Application Support/org.spinorama.sotf
/// - macOS sandbox: ~/Library/Containers/org.spinorama.sotf/Data/Library/Application Support/org.spinorama.sotf
/// - Windows: ~/.config/sotf (same as Linux)
/// - iOS: ~/Library/Application Support/org.spinorama.sotf (same as macOS)
pub fn get_app_config_dir() -> Option<PathBuf> {
    if let Some(dir) = CONFIG_DIR_OVERRIDE.get() {
        return Some(dir.clone());
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = macos_home_dir() {
            let config_dir = home
                .join("Library")
                .join("Application Support")
                .join(APP_BUNDLE_ID);
            std::fs::create_dir_all(&config_dir).ok()?;
            return Some(config_dir);
        }
    }

    #[cfg(target_os = "ios")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let config_dir = PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join(APP_BUNDLE_ID);
            std::fs::create_dir_all(&config_dir).ok()?;
            return Some(config_dir);
        }
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows, use LOCALAPPDATA (preferred) or USERPROFILE as fallback
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let config_dir = PathBuf::from(local_app_data).join("sotf");
            std::fs::create_dir_all(&config_dir).ok()?;
            return Some(config_dir);
        } else if let Ok(user_profile) = std::env::var("USERPROFILE") {
            let config_dir = PathBuf::from(user_profile).join(".config").join("sotf");
            std::fs::create_dir_all(&config_dir).ok()?;
            return Some(config_dir);
        }
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let config_dir = PathBuf::from(home).join(".config").join("sotf");
            std::fs::create_dir_all(&config_dir).ok()?;
            return Some(config_dir);
        }
    }

    // Fallback for any other platform
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "linux",
        target_os = "windows",
        target_os = "android"
    )))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let config_dir = PathBuf::from(home).join(".config").join("sotf");
            std::fs::create_dir_all(&config_dir).ok()?;
            return Some(config_dir);
        }
    }

    None
}

/// Get the path to the music database file
pub fn get_music_db_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("music.db"))
}

/// Get the path to the microphone presets config file
pub fn get_microphone_presets_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("microphones.json"))
}

/// Load microphone presets from disk
pub fn load_microphone_presets()
-> Result<crate::recording_types::MicrophonePresetsConfig, Box<dyn std::error::Error>> {
    if let Some(path) = get_microphone_presets_path() {
        if path.exists() {
            crate::security::validate_config_read_path(&path)?;
            let json = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&json)?)
        } else {
            Ok(crate::recording_types::MicrophonePresetsConfig::default())
        }
    } else {
        Err("Could not determine config directory".into())
    }
}

/// Save microphone presets to disk
pub fn save_microphone_presets(
    config: &crate::recording_types::MicrophonePresetsConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = get_microphone_presets_path() {
        crate::security::validate_write_path(&path)?;
        let json = serde_json::to_string_pretty(config)?;
        std::fs::write(path, json)?;
        Ok(())
    } else {
        Err("Could not determine config directory".into())
    }
}

/// Get the path to the plugin presets directory
pub fn get_plugin_presets_dir() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| {
        let presets_dir = dir.join("plugin_presets");
        std::fs::create_dir_all(&presets_dir).ok();
        presets_dir
    })
}

/// Get the path to the external plugin sandbox grant store.
pub fn get_plugin_sandbox_grants_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("plugin_sandbox_grants.json"))
}

/// Load external plugin sandbox grants from disk.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn load_plugin_sandbox_grants()
-> Result<sotf_plugins::PluginSandboxGrantStore, Box<dyn std::error::Error>> {
    if let Some(path) = get_plugin_sandbox_grants_path() {
        if path.exists() {
            crate::security::validate_config_read_path(&path)?;
            let json = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&json)?)
        } else {
            Ok(sotf_plugins::PluginSandboxGrantStore::default())
        }
    } else {
        Err("Could not determine config directory".into())
    }
}

/// Save external plugin sandbox grants to disk.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn save_plugin_sandbox_grants(
    grants: &sotf_plugins::PluginSandboxGrantStore,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = get_plugin_sandbox_grants_path() {
        crate::security::validate_write_path(&path)?;
        let json = serde_json::to_string_pretty(grants)?;
        std::fs::write(path, json)?;
        Ok(())
    } else {
        Err("Could not determine config directory".into())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSandboxRuntimeStatus {
    pub preset_root: PathBuf,
    pub media_read_paths: Vec<PathBuf>,
    pub protected_import_paths: Vec<PathBuf>,
    pub persistent_grant_count: usize,
    pub runtime_external_access_disabled: bool,
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn plugin_sandbox_media_paths(library_dirs: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut paths = sotf_plugins::default_plugin_sandbox_protected_media_paths();
    paths.extend(library_dirs);
    if let Some(recordings_dir) = get_recordings_dir() {
        paths.push(recordings_dir);
    }
    dedupe_paths(paths)
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn plugin_sandbox_runtime_status(
    library_dirs: impl IntoIterator<Item = PathBuf>,
) -> Result<PluginSandboxRuntimeStatus, Box<dyn std::error::Error>> {
    let grants = load_plugin_sandbox_grants()?;
    let preset_root = get_plugin_presets_dir()
        .ok_or_else(|| std::io::Error::other("Could not determine plugin preset root"))?;
    let media_read_paths = plugin_sandbox_media_paths(library_dirs);

    Ok(PluginSandboxRuntimeStatus {
        preset_root,
        protected_import_paths: media_read_paths.clone(),
        media_read_paths,
        persistent_grant_count: grants.grants.len(),
        runtime_external_access_disabled: true,
    })
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn install_authorized_runtime_plugin_sandbox(
    library_dirs: impl IntoIterator<Item = PathBuf>,
) -> Result<PluginSandboxRuntimeStatus, Box<dyn std::error::Error>> {
    let grants = load_plugin_sandbox_grants()?;
    let preset_root = get_plugin_presets_dir()
        .ok_or_else(|| std::io::Error::other("Could not determine plugin preset root"))?;
    let media_read_paths = plugin_sandbox_media_paths(library_dirs);
    let status = PluginSandboxRuntimeStatus {
        preset_root,
        protected_import_paths: media_read_paths.clone(),
        media_read_paths,
        persistent_grant_count: grants.grants.len(),
        runtime_external_access_disabled: true,
    };
    let options = sotf_plugins::SandboxedPluginCreationOptions::authorized_runtime(
        grants.clone(),
        status.preset_root.clone(),
        status.media_read_paths.clone(),
    );
    sotf_plugins::set_default_sandboxed_plugin_creation_options(Some(options));
    Ok(status)
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn install_import_plugin_sandbox(
    protected_media_paths: impl IntoIterator<Item = PathBuf>,
) -> Result<PluginSandboxRuntimeStatus, Box<dyn std::error::Error>> {
    let grants = load_plugin_sandbox_grants()?;
    let preset_root = get_plugin_presets_dir()
        .ok_or_else(|| std::io::Error::other("Could not determine plugin preset root"))?;
    let protected_import_paths = dedupe_paths(protected_media_paths);
    let options = sotf_plugins::SandboxedPluginCreationOptions::import(
        grants.clone(),
        preset_root.clone(),
        protected_import_paths.clone(),
    );
    sotf_plugins::set_default_sandboxed_plugin_creation_options(Some(options));

    Ok(PluginSandboxRuntimeStatus {
        preset_root,
        media_read_paths: Vec::new(),
        protected_import_paths,
        persistent_grant_count: grants.grants.len(),
        runtime_external_access_disabled: false,
    })
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn dedupe_paths(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut deduped = Vec::new();
    for path in paths {
        if !deduped.contains(&path) {
            deduped.push(path);
        }
    }
    deduped
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSandboxPermissionResolution {
    pub decision: sotf_plugins::PluginSandboxPermissionDecision,
    pub session_grants_changed: bool,
    pub persistent_grants_changed: bool,
    pub restart_required: bool,
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[derive(Debug, Clone, Default)]
pub struct PluginSandboxPermissionController {
    grants: sotf_plugins::PluginSandboxGrantStore,
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
impl PluginSandboxPermissionController {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self::new(load_plugin_sandbox_grants()?))
    }

    pub fn new(grants: sotf_plugins::PluginSandboxGrantStore) -> Self {
        Self { grants }
    }

    pub fn grants(&self) -> &sotf_plugins::PluginSandboxGrantStore {
        &self.grants
    }

    pub fn into_grants(self) -> sotf_plugins::PluginSandboxGrantStore {
        self.grants
    }

    pub fn decide_or_deny(
        &mut self,
        request: sotf_plugins::PluginSandboxPermissionRequest,
    ) -> PluginSandboxPermissionResolution {
        self.decide_with_optional_broker(request, None)
    }

    pub fn decide_with_broker(
        &mut self,
        request: sotf_plugins::PluginSandboxPermissionRequest,
        broker: &mut dyn sotf_plugins::PluginSandboxPermissionBroker,
    ) -> PluginSandboxPermissionResolution {
        self.decide_with_optional_broker(request, Some(broker))
    }

    pub fn decide_or_deny_and_save(
        &mut self,
        request: sotf_plugins::PluginSandboxPermissionRequest,
    ) -> Result<PluginSandboxPermissionResolution, Box<dyn std::error::Error>> {
        let resolution = self.decide_or_deny(request);
        self.save_persistent_changes(&resolution)?;
        Ok(resolution)
    }

    pub fn decide_with_broker_and_save(
        &mut self,
        request: sotf_plugins::PluginSandboxPermissionRequest,
        broker: &mut dyn sotf_plugins::PluginSandboxPermissionBroker,
    ) -> Result<PluginSandboxPermissionResolution, Box<dyn std::error::Error>> {
        let resolution = self.decide_with_broker(request, broker);
        self.save_persistent_changes(&resolution)?;
        Ok(resolution)
    }

    fn decide_with_optional_broker(
        &mut self,
        request: sotf_plugins::PluginSandboxPermissionRequest,
        broker: Option<&mut dyn sotf_plugins::PluginSandboxPermissionBroker>,
    ) -> PluginSandboxPermissionResolution {
        if self
            .grants
            .grants_permission(&request.identity, &request.permission)
        {
            let decision = request.grant_already_active();
            return PluginSandboxPermissionResolution {
                restart_required: decision.restart_required,
                decision,
                session_grants_changed: false,
                persistent_grants_changed: false,
            };
        }

        let decision = match broker {
            Some(broker) => broker.decide_permission(request),
            None => request.deny(),
        };

        let session_grants_changed = self.grants.apply_session_decision(&decision);
        let persistent_grants_changed = session_grants_changed
            && matches!(
                &decision.outcome,
                sotf_plugins::PluginSandboxPermissionOutcome::Granted {
                    persistence: sotf_plugins::PluginSandboxGrantPersistence::RememberForPlugin,
                    ..
                }
            );

        PluginSandboxPermissionResolution {
            restart_required: decision.restart_required,
            decision,
            session_grants_changed,
            persistent_grants_changed,
        }
    }

    fn save_persistent_changes(
        &self,
        resolution: &PluginSandboxPermissionResolution,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if resolution.persistent_grants_changed {
            save_plugin_sandbox_grants(&self.grants)?;
        }
        Ok(())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn decide_plugin_sandbox_permission_or_deny(
    request: sotf_plugins::PluginSandboxPermissionRequest,
) -> Result<PluginSandboxPermissionResolution, Box<dyn std::error::Error>> {
    let mut controller = PluginSandboxPermissionController::load()?;
    controller.decide_or_deny_and_save(request)
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn decide_plugin_sandbox_permission_with_broker(
    request: sotf_plugins::PluginSandboxPermissionRequest,
    broker: &mut dyn sotf_plugins::PluginSandboxPermissionBroker,
) -> Result<PluginSandboxPermissionResolution, Box<dyn std::error::Error>> {
    let mut controller = PluginSandboxPermissionController::load()?;
    controller.decide_with_broker_and_save(request, broker)
}

/// Get the default recording directory.
///
/// This lives under the app support directory so Mac App Store builds can
/// record without relying on a security-scoped external folder.
pub fn get_recordings_dir() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| {
        let recordings_dir = dir.join("Recordings");
        std::fs::create_dir_all(&recordings_dir).ok();
        recordings_dir
    })
}

/// Get the path to the EQ directory (for headphone/speaker EQ curves)
pub fn get_eq_dir() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| {
        let eq_dir = dir.join("EQ");
        std::fs::create_dir_all(&eq_dir).ok();
        eq_dir
    })
}

/// Get the path to the app state config file (deprecated - use app-specific paths)
#[deprecated(note = "Use get_tui_state_path() or get_gpui_state_path() instead")]
pub fn get_app_state_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("app_state.json"))
}

/// Get the path to the TUI app state config file
pub fn get_tui_state_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("app_state_tui.json"))
}

/// Get the path to the GPUI app state config file
pub fn get_gpui_state_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("app_state_gpui.json"))
}

/// Get the path to the TUI log file
pub fn get_tui_log_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("sotf_tui_player.log"))
}

/// Get the path to the GPUI log file
pub fn get_gpui_log_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("sotf_gpui_player.log"))
}

/// Get the path to the server configuration file
pub fn get_server_config_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("servers.json"))
}

/// Get the path to the native remote server store.
pub fn get_remote_servers_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("remote_servers.json"))
}

/// Get the path to the internal native remote token store.
pub fn get_remote_server_tokens_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("remote_server_tokens.json"))
}

/// Load server configuration from disk.
///
/// # Errors
/// Returns an error if the file exists but cannot be read or parsed.
pub fn load_server_config()
-> Result<crate::federation_config::ServerConfig, Box<dyn std::error::Error>> {
    if let Some(path) = get_server_config_path() {
        if path.exists() {
            crate::security::validate_config_read_path(&path)?;
            let json = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&json)?)
        } else {
            Ok(crate::federation_config::ServerConfig::default())
        }
    } else {
        Err("Could not determine config directory".into())
    }
}

/// Save server configuration to disk.
///
/// # Errors
/// Returns an error if the config directory cannot be determined or the file cannot be written.
pub fn save_server_config(
    config: &crate::federation_config::ServerConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = get_server_config_path() {
        crate::security::validate_write_path(&path)?;
        let json = serde_json::to_string_pretty(config)?;
        std::fs::write(path, json)?;
        Ok(())
    } else {
        Err("Could not determine config directory".into())
    }
}

/// Load native remote server records from disk.
///
/// Bearer tokens are intentionally not stored in this file. Use the
/// credential store keyed by `SotfRemoteServer::token_secret_key`.
pub fn load_remote_server_store()
-> Result<crate::sotf_remote::SotfRemoteServerStore, Box<dyn std::error::Error>> {
    if let Some(path) = get_remote_servers_path() {
        if path.exists() {
            crate::security::validate_config_read_path(&path)?;
        }
        Ok(crate::sotf_remote::SotfRemoteServerStore::load_from_path(
            &path,
        )?)
    } else {
        Err("Could not determine config directory".into())
    }
}

/// Save native remote server records to disk.
///
/// Bearer tokens are intentionally not stored in this file. Use the
/// credential store keyed by `SotfRemoteServer::token_secret_key`.
pub fn save_remote_server_store(
    store: &crate::sotf_remote::SotfRemoteServerStore,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = get_remote_servers_path() {
        crate::security::validate_write_path(&path)?;
        Ok(store.save_to_path(&path)?)
    } else {
        Err("Could not determine config directory".into())
    }
}

/// Load a bearer token from the internal native remote token store.
///
/// This fallback is used on platforms without a system credential store.
pub fn load_remote_server_token(key: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
    if let Some(path) = get_remote_server_tokens_path() {
        if path.exists() {
            crate::security::validate_config_read_path(&path)?;
        }
        let store = crate::sotf_remote::SotfRemoteTokenStore::load_from_path(&path)?;
        Ok(store.get(key).map(ToString::to_string))
    } else {
        Err("Could not determine config directory".into())
    }
}

/// Save a bearer token to the internal native remote token store.
///
/// This fallback is used on platforms without a system credential store.
pub fn save_remote_server_token(key: &str, token: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = get_remote_server_tokens_path() {
        if path.exists() {
            crate::security::validate_config_read_path(&path)?;
        }
        crate::security::validate_write_path(&path)?;
        let mut store = crate::sotf_remote::SotfRemoteTokenStore::load_from_path(&path)?;
        store.set(key, token);
        Ok(store.save_to_path(&path)?)
    } else {
        Err("Could not determine config directory".into())
    }
}

/// Delete a bearer token from the internal native remote token store.
///
/// This fallback is used on platforms without a system credential store.
pub fn delete_remote_server_token(key: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = get_remote_server_tokens_path() {
        if path.exists() {
            crate::security::validate_config_read_path(&path)?;
        }
        crate::security::validate_write_path(&path)?;
        let mut store = crate::sotf_remote::SotfRemoteTokenStore::load_from_path(&path)?;
        store.remove(key);
        Ok(store.save_to_path(&path)?)
    } else {
        Err("Could not determine config directory".into())
    }
}

/// Save TUI app configuration to disk
pub fn save_app_config(config: &AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = get_tui_state_path() {
        // Validate that we're writing within the config directory
        crate::security::validate_write_path(&path)?;

        let json = serde_json::to_string_pretty(config)?;
        std::fs::write(path, json)?;
        Ok(())
    } else {
        Err("Could not determine config directory".into())
    }
}

/// Load TUI app configuration from disk, applying migrations if needed
pub fn load_app_config() -> Result<AppConfig, Box<dyn std::error::Error>> {
    if let Some(path) = get_tui_state_path() {
        if path.exists() {
            // Validate that we're reading from within the config directory
            crate::security::validate_config_read_path(&path)?;

            let json = std::fs::read_to_string(&path)?;
            let mut config: AppConfig = serde_json::from_str(&json)?;

            // Check if migration is needed
            const LATEST_VERSION: u32 = 1;
            let original_version = config.version;

            if config.version < LATEST_VERSION {
                log::info!(
                    "Migrating AppConfig from version {} to {}",
                    original_version,
                    LATEST_VERSION
                );

                // Apply migrations
                config = migrate_app_config(config)?;

                // Save upgraded config back to disk
                save_app_config(&config)?;

                log::info!(
                    "Successfully migrated AppConfig from version {} to {}",
                    original_version,
                    LATEST_VERSION
                );
            }

            Ok(config)
        } else {
            Ok(AppConfig::default())
        }
    } else {
        Err("Could not determine config directory".into())
    }
}

/// Apply all necessary migrations to bring AppConfig to the latest version
fn migrate_app_config(config: AppConfig) -> Result<AppConfig, Box<dyn std::error::Error>> {
    const LATEST_VERSION: u32 = 1;

    // Reject corrupt configs with version below minimum
    if config.version < LATEST_VERSION {
        return Err(format!(
            "Unsupported AppConfig version {} (minimum: {})",
            config.version, LATEST_VERSION
        )
        .into());
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

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
        let mut controller = PluginSandboxPermissionController::new(
            sotf_plugins::PluginSandboxGrantStore::default(),
        );

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

        let resolution =
            decide_plugin_sandbox_permission_with_broker(request, &mut broker).unwrap();
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
        let mut controller = PluginSandboxPermissionController::new(
            sotf_plugins::PluginSandboxGrantStore::default(),
        );
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
}
