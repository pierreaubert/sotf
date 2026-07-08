use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Result of checking a host's certificate fingerprint against the TOFU store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TofuResult {
    /// Fingerprint matches the stored value.
    Trusted,
    /// Host not seen before — user must decide whether to trust.
    Unknown { fingerprint: String },
    /// Fingerprint changed since last acceptance — possible MITM.
    Changed {
        old_fingerprint: String,
        new_fingerprint: String,
    },
}

/// A trusted host entry persisted in `known_hosts.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedHost {
    pub fingerprint: String,
    pub first_seen: String,
    pub last_seen: String,
    pub display_name: String,
}

/// Trust-On-First-Use store, similar to SSH `known_hosts`.
///
/// Stored as `{config_dir}/tls/known_hosts.toml`.
#[derive(Debug)]
pub struct TofuStore {
    path: PathBuf,
    hosts: BTreeMap<String, TrustedHost>,
}

impl TofuStore {
    /// Load the TOFU store from disk, creating an empty one if it doesn't exist.
    ///
    /// # Errors
    /// Returns an error if the file exists but cannot be read or parsed.
    pub fn load(config_dir: &Path) -> Result<Self, String> {
        let path = config_dir.join("tls").join("known_hosts.toml");

        let hosts = if path.exists() {
            let content =
                std::fs::read_to_string(&path).map_err(|e| format!("read known_hosts: {e}"))?;
            toml::from_str(&content).map_err(|e| format!("parse known_hosts: {e}"))?
        } else {
            BTreeMap::new()
        };

        Ok(Self { path, hosts })
    }

    /// Check whether a host's fingerprint is trusted.
    #[must_use]
    pub fn check(&self, host_port: &str, fingerprint: &str) -> TofuResult {
        match self.hosts.get(host_port) {
            Some(entry) if entry.fingerprint == fingerprint => TofuResult::Trusted,
            Some(entry) => TofuResult::Changed {
                old_fingerprint: entry.fingerprint.clone(),
                new_fingerprint: fingerprint.to_string(),
            },
            None => TofuResult::Unknown {
                fingerprint: fingerprint.to_string(),
            },
        }
    }

    /// Accept a host's fingerprint (first-time or updated).
    ///
    /// # Errors
    /// Returns an error if the store cannot be written to disk.
    pub fn accept(
        &mut self,
        host_port: &str,
        fingerprint: &str,
        display_name: &str,
    ) -> Result<(), String> {
        let now = chrono::Utc::now().format("%Y-%m-%d").to_string();

        let entry = self
            .hosts
            .entry(host_port.to_string())
            .or_insert_with(|| TrustedHost {
                fingerprint: fingerprint.to_string(),
                first_seen: now.clone(),
                last_seen: now.clone(),
                display_name: display_name.to_string(),
            });
        entry.fingerprint = fingerprint.to_string();
        entry.last_seen = now;
        if !display_name.is_empty() {
            entry.display_name = display_name.to_string();
        }

        self.save()
    }

    /// Remove a host from the trust store.
    ///
    /// # Errors
    /// Returns an error if the store cannot be written to disk after removal.
    pub fn remove(&mut self, host_port: &str) -> Result<bool, String> {
        let removed = self.hosts.remove(host_port).is_some();
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// List all trusted hosts.
    #[must_use]
    pub fn list(&self) -> Vec<(&str, &TrustedHost)> {
        self.hosts.iter().map(|(k, v)| (k.as_str(), v)).collect()
    }

    fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create tls dir: {e}"))?;
        }
        let content = toml::to_string_pretty(&self.hosts)
            .map_err(|e| format!("serialize known_hosts: {e}"))?;

        // Atomic save: write to a tmp file then rename. Prevents a torn
        // known_hosts.toml if the process is killed mid-write.
        let tmp_path = self.path.with_extension("toml.tmp");
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let _ = opts.mode(0o600);
        };
        let mut f = opts
            .open(&tmp_path)
            .map_err(|e| format!("open known_hosts tmp: {e}"))?;
        f.write_all(content.as_bytes())
            .map_err(|e| format!("write known_hosts tmp: {e}"))?;
        f.sync_all()
            .map_err(|e| format!("sync known_hosts tmp: {e}"))?;
        std::fs::rename(&tmp_path, &self.path).map_err(|e| format!("rename known_hosts: {e}"))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Re-assert 0600 in case rename inherited different mode.
            let _ = std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_store_returns_unknown() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let store = TofuStore::load(tmp.path()).expect("load");

        let result = store.check("example.com:6600", "AA:BB:CC");
        assert_eq!(
            result,
            TofuResult::Unknown {
                fingerprint: "AA:BB:CC".to_string()
            }
        );
    }

    #[test]
    fn test_accept_then_check_trusted() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let mut store = TofuStore::load(tmp.path()).expect("load");

        store
            .accept("example.com:6600", "AA:BB:CC", "Test Server")
            .expect("accept");

        assert_eq!(
            store.check("example.com:6600", "AA:BB:CC"),
            TofuResult::Trusted
        );
    }

    #[test]
    fn test_changed_fingerprint() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let mut store = TofuStore::load(tmp.path()).expect("load");

        store
            .accept("example.com:6600", "AA:BB:CC", "Test Server")
            .expect("accept");

        assert_eq!(
            store.check("example.com:6600", "DD:EE:FF"),
            TofuResult::Changed {
                old_fingerprint: "AA:BB:CC".to_string(),
                new_fingerprint: "DD:EE:FF".to_string(),
            }
        );
    }

    #[test]
    fn test_remove() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let mut store = TofuStore::load(tmp.path()).expect("load");

        store
            .accept("host:1234", "AB:CD", "My Host")
            .expect("accept");
        assert!(store.remove("host:1234").expect("remove"));
        assert!(!store.remove("host:1234").expect("remove again"));

        assert!(matches!(
            store.check("host:1234", "AB:CD"),
            TofuResult::Unknown { .. }
        ));
    }

    #[test]
    #[allow(
        clippy::semicolon_outside_block,
        reason = "conflicts with semicolon_if_nothing_returned"
    )]
    fn test_persistence() {
        let tmp = tempfile::tempdir().expect("tmpdir");

        {
            let mut store = TofuStore::load(tmp.path()).expect("load");
            store
                .accept("host:9999", "FF:00", "Persistent")
                .expect("accept");
        }

        // Reload from disk
        let store = TofuStore::load(tmp.path()).expect("reload");
        assert_eq!(store.check("host:9999", "FF:00"), TofuResult::Trusted);
        let list = store.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].1.display_name, "Persistent");
    }

    #[test]
    fn test_list() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let mut store = TofuStore::load(tmp.path()).expect("load");

        store.accept("a:1", "AA", "Alpha").expect("accept");
        store.accept("b:2", "BB", "Beta").expect("accept");

        let list = store.list();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_load_malformed_file_returns_error() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let path = tmp.path().join("tls").join("known_hosts.toml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "this is not toml [}").unwrap();

        let err = TofuStore::load(tmp.path()).expect_err("expected parse error");
        assert!(err.contains("parse known_hosts"), "error: {err}");
    }

    #[test]
    fn test_remove_nonexistent_returns_false() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let mut store = TofuStore::load(tmp.path()).expect("load");
        assert!(!store.remove("no-such:1").expect("remove"));
        // Removing a missing entry must not create the store file.
        assert!(!tmp.path().join("tls").join("known_hosts.toml").exists());
    }

    #[test]
    fn test_accept_updates_existing_host_and_persists() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        {
            let mut store = TofuStore::load(tmp.path()).expect("load");
            store.accept("host:1", "AA", "Old").expect("accept");
            store.accept("host:1", "BB", "New").expect("update");
            assert_eq!(store.check("host:1", "BB"), TofuResult::Trusted);
        }

        let store = TofuStore::load(tmp.path()).expect("reload");
        assert_eq!(store.check("host:1", "BB"), TofuResult::Trusted);
        assert_eq!(
            store.check("host:1", "AA"),
            TofuResult::Changed {
                old_fingerprint: "BB".to_string(),
                new_fingerprint: "AA".to_string(),
            }
        );
        assert_eq!(store.list()[0].1.display_name, "New");
    }

    #[test]
    fn test_list_sorted_by_host() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let mut store = TofuStore::load(tmp.path()).expect("load");
        store.accept("z:9", "ZZ", "Zulu").expect("accept");
        store.accept("a:1", "AA", "Alpha").expect("accept");
        store.accept("m:5", "MM", "Mike").expect("accept");

        let keys: Vec<_> = store.list().iter().map(|(k, _)| k.to_string()).collect();
        assert_eq!(keys, vec!["a:1", "m:5", "z:9"]);
    }

    #[test]
    fn test_corrupted_trust_store_load_fails_cleanly() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let path = tmp.path().join("tls").join("known_hosts.toml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "[[invalid toml").unwrap();

        let err = TofuStore::load(tmp.path()).expect_err("expected parse error");
        let err_text = err.to_string();
        assert!(
            err_text.contains("parse known_hosts"),
            "error should describe the parse failure, got: {}",
            err_text
        );
        // The error is surfaced to callers; it must not contain the absolute
        // filesystem path to the trust store.
        assert!(
            !err_text.contains(tmp.path().to_str().unwrap()),
            "error must not leak trust-store path: {}",
            err_text
        );
    }
}
