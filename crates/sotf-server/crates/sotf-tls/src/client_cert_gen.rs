//! Client certificate generation for mutual TLS pairing.
//!
//! Mobile and desktop clients generate self-signed certificates and present
//! their fingerprints to servers for pinning. No CA is involved.

use rcgen::{CertificateParams, DnType, KeyPair};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::path::{Path, PathBuf};

/// Generate a self-signed client certificate for mTLS authentication.
///
/// The certificate includes the client `name` in the Common Name field.
/// Returns `(DER-encoded certificate, DER-encoded private key)`.
///
/// # Errors
/// Returns an error if key generation or certificate signing fails.
pub fn generate_client_cert(
    name: &str,
) -> Result<(CertificateDer<'static>, PrivateKeyDer<'static>), String> {
    let mut params =
        CertificateParams::new(vec![]).map_err(|e| format!("invalid client cert params: {e}"))?;

    params
        .distinguished_name
        .push(DnType::CommonName, name.to_string());
    params
        .distinguished_name
        .push(DnType::OrganizationName, "SOTF Client");

    // Valid for 365 days
    let now = chrono::Utc::now();
    params.not_before = rcgen::date_time_ymd(
        now.format("%Y").to_string().parse().unwrap_or(2026),
        now.format("%m").to_string().parse().unwrap_or(1),
        now.format("%d").to_string().parse().unwrap_or(1),
    );
    let next_year = now + chrono::Duration::days(365);
    params.not_after = rcgen::date_time_ymd(
        next_year.format("%Y").to_string().parse().unwrap_or(2027),
        next_year.format("%m").to_string().parse().unwrap_or(1),
        next_year.format("%d").to_string().parse().unwrap_or(1),
    );

    let key_pair = KeyPair::generate().map_err(|e| format!("client key generation failed: {e}"))?;
    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| format!("client cert signing failed: {e}"))?;

    let cert_der = CertificateDer::from(cert.der().to_vec());
    let key_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_pair.serialize_der().clone()));

    Ok((cert_der, key_der))
}

/// Load or generate a persistent client certificate for this device.
///
/// Stores the cert and key under `{config_dir}/tls/client_{name}.der` and
/// `client_{name}.key.der`. If files exist, they are loaded; otherwise a new
/// cert is generated and persisted.
///
/// # Errors
/// Returns an error if the TLS directory cannot be created, files cannot be
/// read, or cert generation fails.
pub fn load_or_generate_client_cert(
    config_dir: &Path,
    name: &str,
) -> Result<(CertificateDer<'static>, PrivateKeyDer<'static>), String> {
    let tls_dir = config_dir.join("tls");
    std::fs::create_dir_all(&tls_dir)
        .map_err(|e| format!("failed to create TLS dir {}: {e}", tls_dir.display()))?;

    let cert_path = tls_dir.join(format!("client_{name}.der"));
    let key_path = tls_dir.join(format!("client_{name}.key.der"));

    if cert_path.exists() && key_path.exists() {
        let cert_bytes = std::fs::read(&cert_path).map_err(|e| format!("read client cert: {e}"))?;
        let key_bytes = std::fs::read(&key_path).map_err(|e| format!("read client key: {e}"))?;

        let cert = CertificateDer::from(cert_bytes);
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_bytes));
        return Ok((cert, key));
    }

    let (cert, key) = generate_client_cert(name)?;

    std::fs::write(&cert_path, cert.as_ref()).map_err(|e| format!("write client cert: {e}"))?;
    std::fs::write(&key_path, key_bytes(&key)).map_err(|e| format!("write client key: {e}"))?;

    // Restrict key file permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        let _ = std::fs::set_permissions(&key_path, perms);
    }

    Ok((cert, key))
}

/// Path to the client certificate file (for reference / deletion).
#[must_use]
pub fn client_cert_path(config_dir: &Path, name: &str) -> PathBuf {
    config_dir.join("tls").join(format!("client_{name}.der"))
}

/// Path to the client key file (for reference / deletion).
#[must_use]
pub fn client_key_path(config_dir: &Path, name: &str) -> PathBuf {
    config_dir
        .join("tls")
        .join(format!("client_{name}.key.der"))
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
    use crate::cert_gen::fingerprint;

    #[test]
    fn test_generate_client_cert() {
        let (cert, key) = generate_client_cert("Test-iPhone").expect("should succeed");

        assert!(!cert.as_ref().is_empty());
        match &key {
            PrivateKeyDer::Pkcs8(k) => assert!(!k.secret_pkcs8_der().is_empty()),
            _ => panic!("expected PKCS8 key"),
        }

        let fp = fingerprint(&cert);
        assert_eq!(fp.len(), 95); // SHA-256 = 32 bytes = 95 chars with colons
    }

    #[test]
    fn test_load_or_generate_persists_and_reloads() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let name = "unit-test-client";

        let (cert1, _key1) =
            load_or_generate_client_cert(tmp.path(), name).expect("first generation");
        let fp1 = fingerprint(&cert1);

        // Files should exist
        assert!(client_cert_path(tmp.path(), name).exists());
        assert!(client_key_path(tmp.path(), name).exists());

        // Reloading should return the same cert
        let (cert2, _key2) = load_or_generate_client_cert(tmp.path(), name).expect("reload");
        let fp2 = fingerprint(&cert2);
        assert_eq!(fp1, fp2);
    }
}
