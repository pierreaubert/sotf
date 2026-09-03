use super::consts::OUTPUT_DEVICE_ENV;
use super::misc::is_safe_output_device_name;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const STATE_PATH_ENV: &str = "SOTF_SYSTEMWIDE_STATE_PATH";
const STATE_VERSION: u32 = 1;
const MAX_STATE_FILE_BYTES: u64 = 16 * 1024;
static NEXT_STATE_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Deserialize, Serialize)]
struct PersistedSystemwideState {
    version: u32,
    output_device: String,
}

pub(super) fn configured_output_device() -> Option<String> {
    let environment_value = std::env::var(OUTPUT_DEVICE_ENV).ok();
    let state_path = systemwide_state_path();
    configured_output_device_from_sources(environment_value.as_deref(), state_path.as_deref())
}

fn configured_output_device_from_sources(
    environment_value: Option<&str>,
    state_path: Option<&Path>,
) -> Option<String> {
    configured_output_device_from_value(environment_value)
        .or_else(|| state_path.and_then(load_output_device_from_path))
        .filter(|device| {
            let safe = is_safe_output_device_name(device);
            if !safe {
                log::warn!(
                    "Ignoring persisted or environment-selected virtual output device '{}'",
                    device
                );
            }
            safe
        })
}

pub(super) fn configured_output_device_from_value(value: Option<&str>) -> Option<String> {
    value
        .map(|device| device.trim().to_string())
        .filter(|device| !device.is_empty())
}

pub(super) fn persist_output_device(device: &str) -> std::io::Result<()> {
    let path = systemwide_state_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "cannot determine the per-user Systemwide state directory",
        )
    })?;
    persist_output_device_to_path(&path, device)
}

#[cfg(test)]
fn systemwide_state_path() -> Option<PathBuf> {
    std::env::var_os(STATE_PATH_ENV)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

#[cfg(not(test))]
fn systemwide_state_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(STATE_PATH_ENV).filter(|path| !path.is_empty()) {
        return Some(PathBuf::from(path));
    }

    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library/Application Support/org.spinorama.sotf")
                .join("systemwide-state.json")
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
            })
            .map(|root| root.join("sotf/systemwide-state.json"))
    }
}

fn load_output_device_from_path(path: &Path) -> Option<String> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    }
    let file = options.open(path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > MAX_STATE_FILE_BYTES {
        return None;
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_STATE_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_STATE_FILE_BYTES {
        return None;
    }
    let state: PersistedSystemwideState = serde_json::from_slice(&bytes).ok()?;
    if state.version != STATE_VERSION {
        return None;
    }
    configured_output_device_from_value(Some(&state.output_device))
}

fn persist_output_device_to_path(path: &Path, device: &str) -> std::io::Result<()> {
    let output_device = configured_output_device_from_value(Some(device)).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "output device name must not be empty",
        )
    })?;
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Systemwide state path has no parent directory",
        )
    })?;
    std::fs::create_dir_all(parent)?;

    let state = PersistedSystemwideState {
        version: STATE_VERSION,
        output_device,
    };
    let bytes = serde_json::to_vec_pretty(&state)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let temp_path = parent.join(format!(
        ".systemwide-state-{}-{}.tmp",
        std::process::id(),
        NEXT_STATE_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));

    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| {
        let mut file = options.open(&temp_path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        std::fs::rename(&temp_path, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_output_device_round_trips() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("systemwide-state.json");

        persist_output_device_to_path(&path, " EVO8 ").unwrap();

        assert_eq!(load_output_device_from_path(&path).as_deref(), Some("EVO8"));
    }

    #[test]
    fn cold_start_uses_persisted_output_unless_environment_overrides_it() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("systemwide-state.json");
        persist_output_device_to_path(&path, "EVO8").unwrap();

        assert_eq!(
            configured_output_device_from_sources(None, Some(&path)).as_deref(),
            Some("EVO8")
        );
        assert_eq!(
            configured_output_device_from_sources(Some("Built-in Output"), Some(&path)).as_deref(),
            Some("Built-in Output")
        );
    }

    #[test]
    fn persisted_output_device_rejects_corrupt_or_oversized_state() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("systemwide-state.json");
        std::fs::write(&path, b"not-json").unwrap();
        assert_eq!(load_output_device_from_path(&path), None);

        std::fs::write(&path, vec![b'x'; MAX_STATE_FILE_BYTES as usize + 1]).unwrap();
        assert_eq!(load_output_device_from_path(&path), None);
    }
}
