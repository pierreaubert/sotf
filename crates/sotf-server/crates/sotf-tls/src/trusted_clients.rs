//! Trusted client certificate store for mutual TLS.
//!
//! Servers persist SHA-256 fingerprints of paired client certificates.
//! This is the server-side equivalent of TOFU: the admin pins known clients
//! instead of relying on a CA.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A trusted client entry persisted in `trusted_clients.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustedClient {
    pub fingerprint: String,
    pub name: String,
    pub paired_at: String,
}

/// Server-side store of trusted client certificate fingerprints.
///
/// Stored as `{config_dir}/tls/trusted_clients.toml`.
#[derive(Debug)]
pub struct TrustedClientStore {
    path: PathBuf,
    clients: BTreeMap<String, TrustedClient>,
}

impl TrustedClientStore {
    /// Load the trusted client store from disk, creating an empty one if it doesn't exist.
    ///
    /// # Errors
    /// Returns an error if the file exists but cannot be read or parsed.
    pub fn load(config_dir: &Path) -> Result<Self, String> {
        let path = config_dir.join("tls").join("trusted_clients.toml");

        let clients = if path.exists() {
            let content =
                std::fs::read_to_string(&path).map_err(|e| format!("read trusted_clients: {e}"))?;
            toml::from_str(&content).map_err(|e| format!("parse trusted_clients: {e}"))?
        } else {
            BTreeMap::new()
        };

        Ok(Self { path, clients })
    }

    /// Check whether a client fingerprint is trusted.
    #[must_use]
    pub fn contains(&self, fingerprint: &str) -> bool {
        self.clients.contains_key(fingerprint)
    }

    /// Add or update a trusted client.
    ///
    /// # Errors
    /// Returns an error if the store cannot be written to disk.
    pub fn add(&mut self, fingerprint: &str, name: &str) -> Result<(), String> {
        let now = chrono::Utc::now().format("%Y-%m-%d").to_string();

        let entry = TrustedClient {
            fingerprint: fingerprint.to_string(),
            name: name.to_string(),
            paired_at: now,
        };
        self.clients.insert(fingerprint.to_string(), entry);
        self.save()
    }

    /// Remove a client from the trust store.
    ///
    /// # Errors
    /// Returns an error if the store cannot be written to disk after removal.
    pub fn remove(&mut self, fingerprint: &str) -> Result<bool, String> {
        let removed = self.clients.remove(fingerprint).is_some();
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// List all trusted clients.
    #[must_use]
    pub fn list(&self) -> Vec<&TrustedClient> {
        self.clients.values().collect()
    }

    /// Return the number of trusted clients.
    #[must_use]
    pub fn len(&self) -> usize {
        self.clients.len()
    }

    /// True if no clients are trusted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }

    /// Collect all fingerprints into a `HashSet` for use with `PinnedClientVerifier`.
    #[must_use]
    pub fn fingerprint_set(&self) -> std::collections::HashSet<String> {
        self.clients.keys().cloned().collect()
    }

    fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create tls dir: {e}"))?;
        }
        let content = toml::to_string_pretty(&self.clients)
            .map_err(|e| format!("serialize trusted_clients: {e}"))?;

        // Atomic save: write to a tmp file then rename
        let tmp_path = self.path.with_extension("toml.tmp");
        {
            use std::io::Write;
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            let mut f = opts
                .open(&tmp_path)
                .map_err(|e| format!("open trusted_clients tmp: {e}"))?;
            f.write_all(content.as_bytes())
                .map_err(|e| format!("write trusted_clients tmp: {e}"))?;
            f.sync_all()
                .map_err(|e| format!("sync trusted_clients tmp: {e}"))?;
        }
        std::fs::rename(&tmp_path, &self.path)
            .map_err(|e| format!("rename trusted_clients: {e}"))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_store() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let store = TrustedClientStore::load(tmp.path()).expect("load");
        assert!(store.is_empty());
        assert!(!store.contains("AA:BB:CC"));
        assert!(store.list().is_empty());
    }

    #[test]
    fn test_add_contains_list() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let mut store = TrustedClientStore::load(tmp.path()).expect("load");

        store.add("AA:BB:CC", "iPhone").expect("add");
        assert!(store.contains("AA:BB:CC"));
        assert!(!store.contains("DD:EE:FF"));
        assert_eq!(store.len(), 1);

        let list = store.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "iPhone");
        assert_eq!(list[0].fingerprint, "AA:BB:CC");
    }

    #[test]
    fn test_remove() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let mut store = TrustedClientStore::load(tmp.path()).expect("load");

        store.add("AA:BB:CC", "iPhone").expect("add");
        assert!(store.remove("AA:BB:CC").expect("remove"));
        assert!(!store.contains("AA:BB:CC"));
        assert!(!store.remove("AA:BB:CC").expect("remove again"));
    }

    #[test]
    fn test_persistence() {
        let tmp = tempfile::tempdir().expect("tmpdir");

        {
            let mut store = TrustedClientStore::load(tmp.path()).expect("load");
            store.add("FF:00", "Persistent").expect("add");
        }

        // Reload from disk
        let store = TrustedClientStore::load(tmp.path()).expect("reload");
        assert!(store.contains("FF:00"));
        let list = store.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Persistent");
    }

    #[test]
    fn test_fingerprint_set() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let mut store = TrustedClientStore::load(tmp.path()).expect("load");

        store.add("AA", "Alpha").expect("add");
        store.add("BB", "Beta").expect("add");

        let set = store.fingerprint_set();
        assert_eq!(set.len(), 2);
        assert!(set.contains("AA"));
        assert!(set.contains("BB"));
    }

    #[test]
    fn test_update_existing_client() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let mut store = TrustedClientStore::load(tmp.path()).expect("load");

        store.add("AA:BB", "Old Name").expect("add");
        store.add("AA:BB", "New Name").expect("update");

        let list = store.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "New Name");
    }
}
