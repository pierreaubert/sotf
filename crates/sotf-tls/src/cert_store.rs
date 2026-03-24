use crate::cert_gen;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::path::{Path, PathBuf};

/// Manages persistent storage of server certificate and key.
///
/// Files stored in `{config_dir}/tls/`:
/// - `server.der` — DER-encoded certificate
/// - `server.key.der` — DER-encoded PKCS8 private key
#[derive(Debug)]
pub struct CertStore {
    tls_dir: PathBuf,
    cert: CertificateDer<'static>,
    key: PrivateKeyDer<'static>,
}

impl CertStore {
    /// Load existing cert/key from disk, or generate new ones.
    ///
    /// # Errors
    /// Returns an error if the TLS directory cannot be created, files cannot be read, or cert generation fails.
    pub fn load_or_generate(config_dir: &Path) -> Result<Self, String> {
        let tls_dir = config_dir.join("tls");
        std::fs::create_dir_all(&tls_dir)
            .map_err(|e| format!("failed to create TLS dir {}: {e}", tls_dir.display()))?;

        let cert_path = tls_dir.join("server.der");
        let key_path = tls_dir.join("server.key.der");

        if cert_path.exists() && key_path.exists() {
            log::info!("[TLS] Loading existing certificate from {}", tls_dir.display());
            let cert_bytes =
                std::fs::read(&cert_path).map_err(|e| format!("read cert: {e}"))?;
            let key_bytes =
                std::fs::read(&key_path).map_err(|e| format!("read key: {e}"))?;

            let cert = CertificateDer::from(cert_bytes);
            let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_bytes));

            Ok(Self { tls_dir, cert, key })
        } else {
            log::info!("[TLS] Generating new self-signed certificate");
            let store = Self::generate_new(&tls_dir)?;
            Ok(store)
        }
    }

    /// Force-regenerate the certificate (e.g., IP changed).
    ///
    /// # Errors
    /// Returns an error if cert generation or file writing fails.
    pub fn regenerate(&mut self) -> Result<(), String> {
        let new = Self::generate_new(&self.tls_dir)?;
        self.cert = new.cert;
        self.key = new.key;
        Ok(())
    }

    /// SHA-256 fingerprint of the server certificate.
    #[must_use] 
    pub fn server_fingerprint(&self) -> String {
        cert_gen::fingerprint(&self.cert)
    }

    /// Borrow the certificate.
    #[must_use] 
    pub fn cert(&self) -> &CertificateDer<'static> {
        &self.cert
    }

    /// Clone the certificate for use in TLS config.
    #[must_use] 
    pub fn cert_clone(&self) -> CertificateDer<'static> {
        self.cert.clone()
    }

    /// Clone the private key for use in TLS config.
    #[must_use] 
    pub fn key_clone(&self) -> PrivateKeyDer<'static> {
        self.key.clone_key()
    }

    fn generate_new(tls_dir: &Path) -> Result<Self, String> {
        let ips = cert_gen::local_ip_addresses();
        let hostnames = vec!["localhost".to_string(), "sotf.local".to_string()];

        let (cert, key) = cert_gen::generate_self_signed(&hostnames, &ips)?;

        // Write to disk
        let cert_path = tls_dir.join("server.der");
        let key_path = tls_dir.join("server.key.der");

        std::fs::write(&cert_path, cert.as_ref())
            .map_err(|e| format!("write cert: {e}"))?;
        std::fs::write(&key_path, key_bytes(&key))
            .map_err(|e| format!("write key: {e}"))?;

        // Restrict key file permissions on Unix
        #[cfg(unix)]
        Self::restrict_permissions(&key_path)?;

        let fp = cert_gen::fingerprint(&cert);
        log::info!("[TLS] Certificate fingerprint: {fp}");

        Ok(Self {
            tls_dir: tls_dir.to_path_buf(),
            cert,
            key,
        })
    }

    #[cfg(unix)]
    fn restrict_permissions(path: &Path) -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms).map_err(|e| format!("chmod key: {e}"))
    }
}

fn key_bytes<'a>(key: &'a PrivateKeyDer<'a>) -> &'a [u8] {
    match key {
        PrivateKeyDer::Pkcs8(k) => k.secret_pkcs8_der(),
        PrivateKeyDer::Pkcs1(k) => k.secret_pkcs1_der(),
        PrivateKeyDer::Sec1(k) => k.secret_sec1_der(),
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_or_generate_creates_files() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let store = CertStore::load_or_generate(tmp.path()).expect("should succeed");

        assert!(!store.cert().as_ref().is_empty());
        assert!(!store.server_fingerprint().is_empty());

        // Files should exist
        assert!(tmp.path().join("tls/server.der").exists());
        assert!(tmp.path().join("tls/server.key.der").exists());
    }

    #[test]
    fn test_load_existing() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let store1 = CertStore::load_or_generate(tmp.path()).expect("first");
        let fp1 = store1.server_fingerprint();

        // Loading again should return the same cert
        let store2 = CertStore::load_or_generate(tmp.path()).expect("second");
        assert_eq!(fp1, store2.server_fingerprint());
    }

    #[test]
    fn test_regenerate() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let mut store = CertStore::load_or_generate(tmp.path()).expect("first");
        let fp1 = store.server_fingerprint();

        store.regenerate().expect("regen");
        let fp2 = store.server_fingerprint();

        // New cert should have different fingerprint (overwhelmingly likely)
        assert_ne!(fp1, fp2);
    }
}
