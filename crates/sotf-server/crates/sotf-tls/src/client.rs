use crate::tofu::{TofuResult, TofuStore};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, Error, SignatureScheme};
use std::sync::{Arc, Mutex};

/// Canonicalize a `ServerName` + port into a stable TOFU key.
///
/// - DNS names are lowercased.
/// - IP addresses are normalized via `IpAddr::to_string()` (no `Debug` form),
///   IPv6 is bracketed.
/// - Port is always appended so that pinning `host:6600` does NOT authorize
///   `host:22`.
#[must_use]
pub fn canonical_host_port(server_name: &ServerName<'_>, port: u16) -> String {
    match server_name {
        ServerName::DnsName(dns) => format!("{}:{port}", dns.as_ref().to_ascii_lowercase()),
        ServerName::IpAddress(ip) => {
            let ip: std::net::IpAddr = (*ip).into();
            match ip {
                std::net::IpAddr::V4(v4) => format!("{v4}:{port}"),
                std::net::IpAddr::V6(v6) => format!("[{v6}]:{port}"),
            }
        }
        // ServerName is `#[non_exhaustive]`; fall back to the (host-only)
        // debug form to remain available rather than crash.
        _ => format!("{server_name:?}:{port}"),
    }
}

/// Custom certificate verifier using Trust-On-First-Use.
///
/// On first connection to an unknown host, returns an error containing the fingerprint.
/// The caller (UI layer) should catch this error, prompt the user, and call
/// `TofuStore::accept()` if the user approves.
///
/// On subsequent connections, the stored fingerprint is checked.
///
/// The TOFU key is `(canonical-host-or-ip, port)` so pinning a host on one
/// port doesn't auto-pin every other port on the same host.
#[derive(Debug)]
pub struct TofuVerifier {
    store: Arc<Mutex<TofuStore>>,
    target_port: u16,
}

impl TofuVerifier {
    /// Create a verifier without a known target port. Kept for backward
    /// compatibility — prefer `with_port` so the TOFU key is endpoint-scoped.
    pub fn new(store: Arc<Mutex<TofuStore>>) -> Self {
        Self {
            store,
            target_port: 0,
        }
    }

    /// Create a verifier that scopes TOFU entries to the given target port.
    pub fn with_port(store: Arc<Mutex<TofuStore>>, target_port: u16) -> Self {
        Self { store, target_port }
    }
}

/// Custom certificate verifier that accepts and pins unknown certificates.
///
/// This is the client-side "TOFU" mode used by SOTF-to-SOTF links: the first
/// certificate seen for `(host, port)` is persisted, and later changes are
/// rejected as potential impersonation.
#[derive(Debug)]
pub struct AutoAcceptTofuVerifier {
    store: Arc<Mutex<TofuStore>>,
    target_port: u16,
}

impl AutoAcceptTofuVerifier {
    /// Create a verifier that scopes TOFU entries to the given target port.
    pub fn with_port(store: Arc<Mutex<TofuStore>>, target_port: u16) -> Self {
        Self { store, target_port }
    }
}

/// Error message prefix used to signal TOFU decisions to the UI layer.
pub const TOFU_UNKNOWN_PREFIX: &str = "TOFU_UNKNOWN:";
pub const TOFU_CHANGED_PREFIX: &str = "TOFU_CHANGED:";

fn verify_tls12_signature(
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &DigitallySignedStruct,
) -> Result<HandshakeSignatureValid, Error> {
    // TOFU replaces CA-chain validation, NOT signature verification.
    // We must still verify that the server possesses the private key matching
    // the certificate.
    let provider = rustls::crypto::CryptoProvider::get_default()
        .ok_or_else(|| Error::General("no crypto provider installed".into()))?;
    rustls::crypto::verify_tls12_signature(
        message,
        cert,
        dss,
        &provider.signature_verification_algorithms,
    )
}

fn verify_tls13_signature(
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &DigitallySignedStruct,
) -> Result<HandshakeSignatureValid, Error> {
    let provider = rustls::crypto::CryptoProvider::get_default()
        .ok_or_else(|| Error::General("no crypto provider installed".into()))?;
    rustls::crypto::verify_tls13_signature(
        message,
        cert,
        dss,
        &provider.signature_verification_algorithms,
    )
}

fn supported_verify_schemes() -> Vec<SignatureScheme> {
    vec![
        SignatureScheme::ECDSA_NISTP256_SHA256,
        SignatureScheme::ECDSA_NISTP384_SHA384,
        SignatureScheme::RSA_PSS_SHA256,
        SignatureScheme::RSA_PSS_SHA384,
        SignatureScheme::RSA_PSS_SHA512,
        SignatureScheme::RSA_PKCS1_SHA256,
        SignatureScheme::RSA_PKCS1_SHA384,
        SignatureScheme::RSA_PKCS1_SHA512,
        SignatureScheme::ED25519,
    ]
}

impl ServerCertVerifier for TofuVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let fp = crate::cert_gen::fingerprint(end_entity);
        let key = canonical_host_port(server_name, self.target_port);

        let store = self
            .store
            .lock()
            .map_err(|e| Error::General(format!("TOFU store lock poisoned: {e}")))?;

        match store.check(&key, &fp) {
            TofuResult::Trusted => Ok(ServerCertVerified::assertion()),
            TofuResult::Unknown { fingerprint } => Err(Error::General(format!(
                "{TOFU_UNKNOWN_PREFIX}{fingerprint}"
            ))),
            TofuResult::Changed {
                old_fingerprint,
                new_fingerprint,
            } => Err(Error::General(format!(
                "{TOFU_CHANGED_PREFIX}{old_fingerprint}|{new_fingerprint}"
            ))),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        supported_verify_schemes()
    }
}

impl ServerCertVerifier for AutoAcceptTofuVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let fp = crate::cert_gen::fingerprint(end_entity);
        let key = canonical_host_port(server_name, self.target_port);

        let mut store = self
            .store
            .lock()
            .map_err(|e| Error::General(format!("TOFU store lock poisoned: {e}")))?;

        match store.check(&key, &fp) {
            TofuResult::Trusted => Ok(ServerCertVerified::assertion()),
            TofuResult::Unknown { fingerprint } => {
                store.accept(&key, &fingerprint, &key).map_err(|e| {
                    Error::General(format!("failed to persist TOFU certificate pin: {e}"))
                })?;
                Ok(ServerCertVerified::assertion())
            }
            TofuResult::Changed {
                old_fingerprint,
                new_fingerprint,
            } => Err(Error::General(format!(
                "{TOFU_CHANGED_PREFIX}{old_fingerprint}|{new_fingerprint}"
            ))),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        supported_verify_schemes()
    }
}

/// Build a client TLS config that uses TOFU verification (no client certificate).
///
/// # Errors
/// This function currently always succeeds but returns `Result` for forward compatibility.
pub fn build_client_tls_config(
    tofu_store: Arc<Mutex<TofuStore>>,
) -> Result<Arc<rustls::ClientConfig>, String> {
    let verifier = Arc::new(TofuVerifier::new(tofu_store));

    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();

    Ok(Arc::new(config))
}

/// Build an owned client TLS config that auto-pins unknown certificates.
///
/// Reqwest consumes an owned `rustls::ClientConfig` through
/// `use_preconfigured_tls`, so this variant intentionally does not wrap the
/// config in `Arc`.
///
/// # Errors
/// This function currently always succeeds but returns `Result` for forward compatibility.
pub fn build_auto_accept_client_tls_config_for_port(
    tofu_store: Arc<Mutex<TofuStore>>,
    target_port: u16,
) -> Result<rustls::ClientConfig, String> {
    let verifier = Arc::new(AutoAcceptTofuVerifier::with_port(tofu_store, target_port));

    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();

    Ok(config)
}

/// Build a client TLS config that uses TOFU verification AND presents a client certificate.
///
/// The client certificate is used for mutual TLS — the server verifies our identity
/// via our certificate fingerprint instead of a password.
///
/// # Errors
/// Returns an error if the client certificate or key is invalid.
pub fn build_client_tls_config_with_cert(
    tofu_store: Arc<Mutex<TofuStore>>,
    client_cert: rustls::pki_types::CertificateDer<'static>,
    client_key: rustls::pki_types::PrivateKeyDer<'static>,
) -> Result<Arc<rustls::ClientConfig>, String> {
    let verifier = Arc::new(TofuVerifier::new(tofu_store));

    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_client_auth_cert(vec![client_cert], client_key)
        .map_err(|e| format!("client cert config error: {e}"))?;

    Ok(Arc::new(config))
}

/// Parse a TOFU error message to extract the decision type and fingerprint(s).
///
/// Returns `Some(TofuResult)` if the error is a TOFU decision, `None` otherwise.
#[must_use]
pub fn parse_tofu_error(error_msg: &str) -> Option<TofuResult> {
    if let Some(fp) = error_msg.strip_prefix(TOFU_UNKNOWN_PREFIX) {
        Some(TofuResult::Unknown {
            fingerprint: fp.to_string(),
        })
    } else if let Some(rest) = error_msg.strip_prefix(TOFU_CHANGED_PREFIX) {
        let parts: Vec<&str> = rest.splitn(2, '|').collect();
        (parts.len() == 2).then(|| TofuResult::Changed {
            old_fingerprint: parts[0].to_string(),
            new_fingerprint: parts[1].to_string(),
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tofu_error_unknown() {
        let msg = format!("{TOFU_UNKNOWN_PREFIX}AA:BB:CC");
        let result = parse_tofu_error(&msg);
        assert_eq!(
            result,
            Some(TofuResult::Unknown {
                fingerprint: "AA:BB:CC".to_string()
            })
        );
    }

    #[test]
    fn test_parse_tofu_error_changed() {
        let msg = format!("{TOFU_CHANGED_PREFIX}AA:BB|CC:DD");
        let result = parse_tofu_error(&msg);
        assert_eq!(
            result,
            Some(TofuResult::Changed {
                old_fingerprint: "AA:BB".to_string(),
                new_fingerprint: "CC:DD".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_tofu_error_not_tofu() {
        assert_eq!(parse_tofu_error("some other error"), None);
    }

    #[test]
    fn test_build_client_config() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let store = TofuStore::load(tmp.path()).expect("load");
        let store = Arc::new(Mutex::new(store));

        let config = build_client_tls_config(store);
        config.unwrap();
    }
}
